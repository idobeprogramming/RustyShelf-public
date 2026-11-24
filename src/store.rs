use crate::model::{Game, Installation, Launcher};
use rusqlite::{params, Connection, OptionalExtension};
use std::{fs, path::PathBuf};

pub struct Store {
    connection: Connection,
}

#[derive(Clone, Copy)]
pub enum ReviewAction {
    Confirm,
    Ignore,
}

impl Store {
    pub fn open_default() -> rusqlite::Result<Self> {
        let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
        // Reuse the previous library in place, including any SQLite WAL files.
        let legacy = base.join("GameShelf/gameshelf.sqlite3");
        if legacy.is_file() {
            return Self::open(legacy);
        }
        let root = base.join("RustyShelf");
        fs::create_dir_all(&root)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        Self::open(root.join("rustyshelf.sqlite3"))
    }

    pub fn open(path: impl Into<PathBuf>) -> rusqlite::Result<Self> {
        let connection = Connection::open(path.into())?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS games (
               id TEXT PRIMARY KEY, title TEXT NOT NULL, category TEXT NOT NULL,
               favorite INTEGER NOT NULL DEFAULT 0, confirmed INTEGER NOT NULL DEFAULT 0,
               cover_path TEXT, category_custom INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS installations (
               game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
               path TEXT NOT NULL, launcher TEXT NOT NULL, profile TEXT,
               available INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(game_id, path)
             );
             CREATE INDEX IF NOT EXISTS installations_path ON installations(path);
             CREATE TABLE IF NOT EXISTS excluded_paths (path TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS ignored_games (game_id TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS title_overrides (game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE, title TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS detected_consoles (game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE, console TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS emulators (id TEXT PRIMARY KEY, name TEXT NOT NULL, executable TEXT NOT NULL UNIQUE);
             CREATE TABLE IF NOT EXISTS game_emulators (game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE, emulator_id TEXT NOT NULL REFERENCES emulators(id) ON DELETE CASCADE);
             CREATE TABLE IF NOT EXISTS categories (name TEXT PRIMARY KEY);",
        )?;
        #[cfg(windows)]
        connection.execute_batch("CREATE INDEX IF NOT EXISTS installations_path_windows ON installations(REPLACE(path,char(92),'/') COLLATE NOCASE);")?;
        let store = Self { connection };
        store.merge_legacy_duplicates()?;
        store.split_legacy_image_groups()?;
        Ok(store)
    }

    fn split_legacy_image_groups(&self) -> rusqlite::Result<()> {
        let games = self.games()?;
        let tx = self.connection.unchecked_transaction()?;
        let mut overrides: std::collections::HashMap<String, String> = self
            .preference("console_overrides")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let mut changed = false;
        for game in games {
            if game.installations.len() < 2 {
                continue;
            }
            let has_pc_install = game
                .installations
                .iter()
                .any(|installation| !crate::rom::is_image(&installation.path));
            let images = game
                .installations
                .iter()
                .filter(|installation| crate::rom::is_image(&installation.path));
            for installation in images.skip(if has_pc_install { 0 } else { 1 }) {
                let id = crate::model::game_id(&game.title, &installation.path);
                if id == game.id {
                    continue;
                }
                tx.execute("INSERT OR IGNORE INTO games(id,title,category,favorite,confirmed,cover_path,category_custom) SELECT ?1,title,category,favorite,confirmed,cover_path,category_custom FROM games WHERE id=?2", params![id, game.id])?;
                tx.execute("INSERT OR IGNORE INTO title_overrides(game_id,title) SELECT ?1,title FROM title_overrides WHERE game_id=?2", params![id, game.id])?;
                tx.execute("INSERT OR IGNORE INTO installations(game_id,path,launcher,profile,available) SELECT ?1,path,launcher,profile,available FROM installations WHERE game_id=?2 AND path=?3", params![id, game.id, installation.path.to_string_lossy()])?;
                tx.execute(
                    "DELETE FROM installations WHERE game_id=?1 AND path=?2",
                    params![game.id, installation.path.to_string_lossy()],
                )?;
                if let Some(console) = overrides.get(&game.id).cloned() {
                    overrides.entry(id).or_insert(console);
                    changed = true;
                }
            }
        }
        if changed {
            if let Ok(json) = serde_json::to_string(&overrides) {
                tx.execute("INSERT INTO preferences(key,value) VALUES('console_overrides',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json])?;
            }
        }
        tx.commit()
    }

    fn merge_legacy_duplicates(&self) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let mut overrides: std::collections::HashMap<String, String> = self
            .preference("console_overrides")?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let mut overrides_changed = false;
        let records = {
            let mut query = tx.prepare(
                "SELECT id,title,category,favorite,confirmed,cover_path,category_custom FROM games",
            )?;
            let rows = query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (old_id, title, category, favorite, confirmed, cover, custom_category) in records {
            // Scoped ROM IDs remain stable even while their drive is disconnected.
            if old_id.starts_with("rom-") {
                continue;
            }
            let path: Option<String> = tx
                .query_row(
                    "SELECT path FROM installations WHERE game_id=?1 ORDER BY path LIMIT 1",
                    [&old_id],
                    |row| row.get(0),
                )
                .optional()?;
            let new_id =
                crate::model::game_id(&title, std::path::Path::new(path.as_deref().unwrap_or("")));
            if old_id == new_id {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO games(id,title,category,favorite,confirmed,cover_path,category_custom) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![new_id, title, category, favorite, confirmed, cover, custom_category],
            )?;
            tx.execute(
                "UPDATE games SET favorite=MAX(favorite,?2), cover_path=COALESCE(cover_path,?3),
                 category=CASE WHEN category_custom=1 THEN category WHEN ?4=1 THEN ?5 WHEN ?6=1 AND confirmed=0 THEN ?5 ELSE category END,
                 confirmed=MAX(confirmed,?6), category_custom=MAX(category_custom,?4) WHERE id=?1",
                params![new_id, favorite, cover, custom_category, category, confirmed],
            )?;
            tx.execute(
                "INSERT INTO installations(game_id,path,launcher,profile,available)
                 SELECT ?1,path,launcher,profile,available FROM installations WHERE game_id=?2 AND 1
                 ON CONFLICT(game_id,path) DO UPDATE SET
                 launcher=CASE WHEN installations.launcher='unknown' THEN excluded.launcher ELSE installations.launcher END,
                 profile=COALESCE(installations.profile,excluded.profile), available=MAX(installations.available,excluded.available)",
                params![new_id, old_id],
            )?;
            tx.execute("INSERT OR IGNORE INTO game_emulators(game_id,emulator_id) SELECT ?1,emulator_id FROM game_emulators WHERE game_id=?2", params![new_id, old_id])?;
            tx.execute("INSERT OR IGNORE INTO title_overrides(game_id,title) SELECT ?1,title FROM title_overrides WHERE game_id=?2", params![new_id, old_id])?;
            tx.execute("INSERT OR IGNORE INTO detected_consoles(game_id,console) SELECT ?1,console FROM detected_consoles WHERE game_id=?2", params![new_id, old_id])?;
            tx.execute("INSERT OR IGNORE INTO ignored_games(game_id) SELECT ?1 FROM ignored_games WHERE game_id=?2", params![new_id, old_id])?;
            if let Some(console) = overrides.remove(&old_id) {
                // A correction already attached to the surviving record wins.
                overrides.entry(new_id).or_insert(console);
                overrides_changed = true;
            }
            tx.execute("DELETE FROM ignored_games WHERE game_id=?1", [&old_id])?;
            tx.execute("DELETE FROM games WHERE id=?1", [&old_id])?;
        }
        if overrides_changed {
            let json = serde_json::to_string(&overrides)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            tx.execute("INSERT INTO preferences(key,value) VALUES('console_overrides',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json])?;
        }
        tx.commit()
    }

    pub fn preference(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row("SELECT value FROM preferences WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn backup_to(&self, path: &std::path::Path) -> Result<(), String> {
        if let (Some(source), Ok(destination)) = (self.connection.path(), path.canonicalize()) {
            if std::path::Path::new(source).canonicalize().ok().as_ref() == Some(&destination) {
                return Err("Choisis un fichier différent de la bibliothèque active.".into());
            }
        }
        self.connection
            .backup(rusqlite::DatabaseName::Main, path, None)
            .map_err(|error| error.to_string())
    }

    pub fn set_preference(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn set_preferences(&self, values: &[(&str, &str)]) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        for (key, value) in values {
            tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        }
        tx.commit()
    }

    pub fn save_emulator(&self, emulator: &crate::model::Emulator) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT INTO emulators(id,name,executable) VALUES(?1,?2,?3) ON CONFLICT(executable) DO UPDATE SET name=excluded.name",
            params![emulator.id, emulator.name, emulator.executable.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn emulators(&self) -> rusqlite::Result<Vec<crate::model::Emulator>> {
        let mut query = self
            .connection
            .prepare("SELECT id,name,executable FROM emulators ORDER BY name COLLATE NOCASE")?;
        let rows = query.query_map([], |row| {
            Ok(crate::model::Emulator {
                id: row.get(0)?,
                name: row.get(1)?,
                executable: PathBuf::from(row.get::<_, String>(2)?),
            })
        })?;
        rows.collect()
    }

    pub fn reuse_installation_identity(&self, game: &mut Game) -> rusqlite::Result<()> {
        #[cfg(windows)]
        let query = "SELECT game_id,path FROM installations JOIN games ON games.id=installations.game_id WHERE REPLACE(path,char(92),'/')=?1 COLLATE NOCASE ORDER BY confirmed DESC, favorite DESC LIMIT 1";
        #[cfg(not(windows))]
        let query = "SELECT game_id,path FROM installations JOIN games ON games.id=installations.game_id WHERE path=?1 ORDER BY confirmed DESC, favorite DESC LIMIT 1";
        let mut identity_found = false;
        for installation in &mut game.installations {
            let path = installation.path.to_string_lossy().into_owned();
            #[cfg(windows)]
            let path = path.replace('\\', "/");
            let existing: Option<(String, String)> = self
                .connection
                .query_row(query, [path], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()?;
            if let Some((id, path)) = existing {
                if !identity_found {
                    game.id = id.clone();
                    identity_found = true;
                }
                if game.id == id {
                    installation.path = path.into();
                }
            }
        }
        Ok(())
    }
    pub fn save_game(&self, game: &Game) -> rusqlite::Result<()> {
        self.save_game_with_visibility(game, false)
    }

    pub fn save_manual_game(&self, game: &mut Game) -> rusqlite::Result<()> {
        self.reuse_installation_identity(game)?;
        self.save_game_with_visibility(game, true)
    }

    fn save_game_with_visibility(
        &self,
        game: &Game,
        restore_ignored: bool,
    ) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let preserve_launcher_metadata = restore_ignored && tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM installations WHERE game_id=?1 AND launcher NOT IN ('unknown','manual'))",
            [&game.id], |row| row.get::<_, bool>(0),
        )?;
        let existing: Option<(bool, Option<String>, String, bool, bool)> = tx
            .query_row(
                "SELECT favorite, cover_path, category, category_custom, confirmed FROM games WHERE id=?1",
                [&game.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        let favorite = existing.as_ref().map_or(game.favorite, |row| row.0);
        // Discovery must never replace a cover already chosen in the library.
        // Explicit replacements go through set_cover instead.
        let cover = existing.as_ref().and_then(|row| row.1.clone()).or_else(|| {
            game.cover_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
        });
        let category = existing
            .as_ref()
            .filter(|row| {
                if row.3 {
                    return true;
                }
                if row.2.starts_with("ROM · ") && game.category.starts_with("ROM · ") {
                    if game.detected_console.is_some() {
                        return false;
                    }
                    if game.category.strip_prefix("ROM · ") == Some(crate::rom::UNCLASSIFIED) {
                        return true;
                    }
                }
                row.4 && (!game.confirmed || preserve_launcher_metadata)
            })
            .map_or_else(|| game.category.clone(), |row| row.2.clone());
        tx.execute(
            "INSERT INTO games(id,title,category,favorite,confirmed,cover_path) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET title=CASE WHEN games.confirmed=1 AND (excluded.confirmed=0 OR ?7) THEN games.title ELSE excluded.title END, category=excluded.category,
             confirmed=MAX(excluded.confirmed,games.confirmed), cover_path=COALESCE(excluded.cover_path,games.cover_path)",
            params![game.id, game.title, category, favorite, game.confirmed, cover, preserve_launcher_metadata],
        )?;
        if let Some(console) = &game.detected_console {
            tx.execute(
                "INSERT INTO detected_consoles(game_id,console) VALUES(?1,?2)
                        ON CONFLICT(game_id) DO UPDATE SET console=excluded.console",
                params![game.id, console],
            )?;
        }
        for installation in &game.installations {
            let profile = installation
                .launch_profile
                .as_ref()
                .and_then(|p| serde_json::to_string(p).ok());
            tx.execute(
                "INSERT INTO installations(game_id,path,launcher,profile,available) VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(game_id,path) DO UPDATE SET
                 launcher=CASE WHEN (excluded.launcher='unknown' AND installations.launcher<>'unknown') OR (excluded.launcher='manual' AND installations.launcher NOT IN ('unknown','manual')) THEN installations.launcher ELSE excluded.launcher END,
                 profile=CASE WHEN excluded.launcher='unknown' AND installations.launcher<>'unknown' THEN installations.profile WHEN excluded.launcher='manual' AND installations.launcher NOT IN ('unknown','manual') THEN COALESCE(installations.profile,excluded.profile) ELSE COALESCE(excluded.profile,installations.profile) END, available=excluded.available",
                params![game.id, installation.path.to_string_lossy(), launcher_name(&installation.launcher), profile, installation.available],
            )?;
        }
        if restore_ignored {
            tx.execute("DELETE FROM ignored_games WHERE game_id=?1", [&game.id])?;
        }
        tx.commit()
    }

    pub fn games(&self) -> rusqlite::Result<Vec<Game>> {
        self.read_games(false, false)
    }

    pub fn scan_candidates(&self) -> rusqlite::Result<Vec<Game>> {
        self.read_games(false, true)
    }

    pub fn ignored_games(&self) -> rusqlite::Result<Vec<Game>> {
        self.read_games(true, false)
    }

    fn read_games(&self, ignored: bool, candidates_only: bool) -> rusqlite::Result<Vec<Game>> {
        // One ordered join replaces one query per game. LEFT JOIN retains games
        // without an installation (for example after importing an older library).
        let mut statement = self.connection.prepare(
            "SELECT g.id,COALESCE(t.title,g.title),g.category,g.favorite,g.confirmed,g.cover_path,
                    i.path,i.launcher,i.profile,i.available,
                    (g.category_custom OR t.game_id IS NOT NULL OR
                     EXISTS(SELECT 1 FROM game_emulators e WHERE e.game_id=g.id)), d.console
             FROM games g LEFT JOIN installations i ON i.game_id=g.id
             LEFT JOIN title_overrides t ON t.game_id=g.id
             LEFT JOIN detected_consoles d ON d.game_id=g.id
             WHERE EXISTS (SELECT 1 FROM ignored_games WHERE game_id=g.id)=?1
               AND (?2=0 OR g.confirmed=0)
             ORDER BY COALESCE(t.title,g.title) COLLATE NOCASE,g.id,i.path",
        )?;
        let mut rows = statement.query([ignored, candidates_only])?;
        let mut games: Vec<Game> = Vec::new();
        let mut customized_ids = std::collections::HashSet::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            if games.last().is_none_or(|game| game.id != id) {
                let cover: Option<String> = row.get(5)?;
                if row.get::<_, bool>(10)? {
                    customized_ids.insert(id.clone());
                }
                games.push(Game {
                    id,
                    title: row.get(1)?,
                    category: row.get(2)?,
                    detected_console: row.get(11)?,
                    favorite: row.get(3)?,
                    confirmed: row.get(4)?,
                    cover_path: cover.map(PathBuf::from),
                    installations: Vec::new(),
                });
            }
            let path: Option<String> = row.get(6)?;
            if let Some(path) = path {
                let profile: Option<String> = row.get(8)?;
                games.last_mut().unwrap().installations.push(Installation {
                    path: path.into(),
                    launcher: parse_launcher(&row.get::<_, String>(7)?),
                    launch_profile: profile.and_then(|json| serde_json::from_str(&json).ok()),
                    available: row.get(9)?,
                });
            }
        }
        if !ignored && !candidates_only {
            games.retain_mut(|game| {
                if game.installations.is_empty() {
                    return true;
                }
                game.installations
                    .retain(|installation| !crate::sources::is_launcher_component(installation));
                // Keep personal entries, even when all their old paths pointed
                // to installer metadata. A Steam cache image is automatic artwork.
                !game.installations.is_empty()
                    || game.favorite
                    || customized_ids.contains(&game.id)
                    || game
                        .cover_path
                        .as_ref()
                        .is_some_and(|path| !crate::sources::is_shared_runtime_art(path))
            });
        }
        Ok(games)
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET favorite=?2 WHERE id=?1",
            params![id, favorite],
        )?;
        Ok(())
    }

    pub fn custom_title(&self, id: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT title FROM title_overrides WHERE game_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn set_title(&self, id: &str, title: &str) -> rusqlite::Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Le nom du jeu ne peut pas être vide.",
                ),
            )));
        }
        self.connection.execute("INSERT INTO title_overrides(game_id,title) VALUES(?1,?2) ON CONFLICT(game_id) DO UPDATE SET title=excluded.title", params![id, title])?;
        Ok(())
    }

    pub fn reset_title(&self, id: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM title_overrides WHERE game_id=?1", [id])?;
        Ok(())
    }

    pub fn set_confirmed(&self, id: &str, confirmed: bool) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET confirmed=?2 WHERE id=?1",
            params![id, confirmed],
        )?;
        Ok(())
    }

    pub fn review_games(&self, ids: &[String], action: ReviewAction) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let sql = match action {
            ReviewAction::Confirm => "UPDATE games SET confirmed=1 WHERE id=?1 AND confirmed=0 AND NOT EXISTS (SELECT 1 FROM ignored_games WHERE game_id=games.id)",
            ReviewAction::Ignore => "INSERT OR IGNORE INTO ignored_games(game_id) SELECT id FROM games WHERE id=?1 AND confirmed=0",
        };
        {
            let mut statement = tx.prepare(sql)?;
            for id in ids {
                statement.execute([id])?;
            }
        }
        tx.commit()
    }

    pub fn ignore_game(&self, id: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO ignored_games(game_id) VALUES(?1)",
            [id],
        )?;
        Ok(())
    }

    pub fn ignore_obsolete_candidates<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> rusqlite::Result<usize> {
        let tx = self.connection.unchecked_transaction()?;
        let mut changed = 0;
        {
            let mut statement = tx.prepare(
                "INSERT OR IGNORE INTO ignored_games(game_id) SELECT id FROM games
             WHERE id=?1 AND confirmed=0 AND favorite=0 AND category_custom=0 AND cover_path IS NULL
             AND NOT EXISTS(SELECT 1 FROM title_overrides WHERE game_id=?1)
             AND NOT EXISTS(SELECT 1 FROM game_emulators WHERE game_id=?1)
             AND NOT EXISTS(SELECT 1 FROM installations WHERE game_id=?1 AND launcher<>'unknown')",
            )?;
            for id in ids {
                changed += statement.execute([id])?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn is_ignored(&self, id: &str) -> rusqlite::Result<bool> {
        self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM ignored_games WHERE game_id=?1)",
            [id],
            |row| row.get(0),
        )
    }

    pub fn restore_ignored_games(&self) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM ignored_games", [])?;
        Ok(())
    }

    pub fn restore_ignored_game(&self, id: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM ignored_games WHERE game_id=?1", [id])?;
        Ok(())
    }

    pub fn set_cover(&self, id: &str, path: Option<&std::path::Path>) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET cover_path=?2 WHERE id=?1",
            params![id, path.map(|p| p.to_string_lossy().to_string())],
        )?;
        Ok(())
    }

    pub fn set_missing_cover(&self, id: &str, path: &std::path::Path) -> rusqlite::Result<bool> {
        // The user may have selected a cover after the background search started.
        self.connection.execute(
            "UPDATE games SET cover_path=?2 WHERE id=?1 AND cover_path IS NULL AND NOT EXISTS(SELECT 1 FROM ignored_games WHERE game_id=?1)",
            params![id, path.to_string_lossy()],
        ).map(|changed| changed != 0)
    }

    pub fn set_category(&self, id: &str, category: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET category=?2, category_custom=1 WHERE id=?1",
            params![id, category],
        )?;
        Ok(())
    }

    pub fn add_exclusion(&self, path: &std::path::Path) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO excluded_paths(path) VALUES(?1)",
            [path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn remove_exclusion(&self, path: &std::path::Path) -> rusqlite::Result<()> {
        self.connection.execute(
            "DELETE FROM excluded_paths WHERE path=?1",
            [path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn exclusions(&self) -> rusqlite::Result<Vec<PathBuf>> {
        let mut query = self
            .connection
            .prepare("SELECT path FROM excluded_paths ORDER BY path")?;
        let rows = query.query_map([], |row| Ok(PathBuf::from(row.get::<_, String>(0)?)))?;
        rows.collect()
    }

    pub fn mark_unseen_under_roots_unavailable(
        &self,
        roots: &[PathBuf],
        exclusions: &[PathBuf],
        seen_paths: &[PathBuf],
    ) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let paths = {
            let mut query = tx.prepare("SELECT DISTINCT path FROM installations")?;
            let paths = query
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            paths
        };
        let seen_paths: std::collections::HashSet<_> = seen_paths.iter().collect();
        for value in paths {
            let path = PathBuf::from(value);
            if roots
                .iter()
                .any(|root| crate::scan::path_is_within(&path, root))
                && !exclusions
                    .iter()
                    .any(|excluded| crate::scan::path_is_within(&path, excluded))
            {
                let available = seen_paths.contains(&path) || path.exists();
                tx.execute(
                    "UPDATE installations SET available=?2 WHERE path=?1",
                    params![path.to_string_lossy(), available],
                )?;
            }
        }
        tx.commit()
    }
}

fn launcher_name(launcher: &Launcher) -> &'static str {
    match launcher {
        Launcher::Steam => "steam",
        Launcher::Epic => "epic",
        Launcher::Heroic => "heroic",
        Launcher::Lutris => "lutris",
        Launcher::Gog => "gog",
        Launcher::Manual => "manual",
        Launcher::Unknown => "unknown",
    }
}
fn parse_launcher(value: &str) -> Launcher {
    match value {
        "steam" => Launcher::Steam,
        "epic" => Launcher::Epic,
        "heroic" => Launcher::Heroic,
        "lutris" => Launcher::Lutris,
        "gog" => Launcher::Gog,
        "manual" => Launcher::Manual,
        _ => Launcher::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn steam_rescan_repairs_a_saved_placeholder_and_keeps_edits_after_restart() {
        let temp = tempdir().unwrap();
        let db = temp.path().join("library.db");
        let library = temp.path().join("steamapps");
        let install = library.join("common/Lune");
        fs::create_dir_all(&install).unwrap();
        let manifest = library.join("appmanifest_4294967294.acf");
        fs::write(
            &manifest,
            r#""AppState" { "appid" "4294967294" "name" "APP_ID_4294967294" "installdir" "Lune" }"#,
        )
        .unwrap();
        let mut rescanned = crate::sources::from_manifest(&manifest).unwrap().unwrap();
        assert_eq!(rescanned.title, "Lune");
        let old = Game::from_installation(
            "appid_4294967294",
            "Steam",
            rescanned.installations[0].clone(),
        );
        let store = Store::open(&db).unwrap();
        store.save_game(&old).unwrap();
        store.set_favorite(&old.id, true).unwrap();
        store.set_title(&old.id, "Ma lune").unwrap();
        store.reuse_installation_identity(&mut rescanned).unwrap();
        store.save_game(&rescanned).unwrap();
        drop(store);

        let store = Store::open(db).unwrap();
        let games = store.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Ma lune");
        assert!(games[0].favorite);
        assert_eq!(games[0].installations, rescanned.installations);
        store.reset_title(&games[0].id).unwrap();
        assert_eq!(store.games().unwrap()[0].title, "Lune");
    }

    #[test]
    fn only_real_launcher_installations_enter_the_library_after_rescan_and_restart() {
        let temp = tempdir().unwrap();
        let db = temp.path().join("library.db");
        let cache = temp.path().join("GOG.com/supportInstaller");
        let install = temp.path().join("Jeux/Lune");
        fs::create_dir_all(&install).unwrap();
        let gog = |title, location| {
            Game::from_installation(
                title,
                "GOG",
                Installation {
                    path: location,
                    launcher: Launcher::Gog,
                    launch_profile: None,
                    available: true,
                },
            )
        };
        let store = Store::open(&db).unwrap();
        store.save_game(&gog("Lune", cache.join("10"))).unwrap();
        let mut old = gog("Lune - Extension", cache.join("20"));
        old.installations
            .extend(gog("Lune - Extension", install.clone()).installations);
        store.save_game(&old).unwrap();
        let corrected = gog("Autre extension", cache.join("30"));
        store.save_game(&corrected).unwrap();
        store.set_title(&corrected.id, "À conserver").unwrap();
        let shared = temp.path().join("steamapps/common/Steamworks Shared");
        fs::create_dir_all(&shared).unwrap();
        let manifest = temp.path().join("steamapps/appmanifest_228980.acf");
        fs::write(&manifest, r#""AppState" { "appid" "228980" "name" "Steamworks" "installdir" "Steamworks Shared" }"#).unwrap();
        assert!(crate::sources::from_manifest(&manifest).unwrap().is_none());
        let mut runtime = Game::from_installation(
            "Steamworks",
            "Steam",
            Installation {
                path: shared,
                launcher: Launcher::Steam,
                launch_profile: Some(crate::model::LaunchProfile {
                    command: PathBuf::new(),
                    arguments: vec![],
                    working_directory: None,
                    launcher_command: Some("steam://rungameid/228980".into()),
                }),
                available: true,
            },
        );
        runtime.cover_path = Some(temp.path().join("appcache/librarycache/228980/header.jpg"));
        store.save_game(&runtime).unwrap();
        let mut rediscovered = gog("Lune", install.clone());
        store
            .reuse_installation_identity(&mut rediscovered)
            .unwrap();
        store.save_game(&rediscovered).unwrap();
        drop(store);
        let store = Store::open(&db).unwrap();
        let games = store.games().unwrap();
        assert_eq!(games.len(), 2);
        let base = games.iter().find(|g| g.title == "Lune").unwrap();
        assert_eq!(base.installations.len(), 1);
        assert_eq!(base.installations[0].path, install);
        assert!(games
            .iter()
            .any(|g| g.title == "À conserver" && g.installations.is_empty()));
        store
            .set_cover(&runtime.id, Some(&temp.path().join("Perso.png")))
            .unwrap();
        assert!(store.games().unwrap().iter().any(|g| g.id == runtime.id));
    }

    #[test]
    fn automatic_candidate_cleanup_preserves_user_changes_and_is_reversible() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for protection in [
            "none",
            "confirmed",
            "favorite",
            "title",
            "category",
            "cover",
            "manual",
        ] {
            let mut game = Game::from_installation(
                protection,
                "À confirmer",
                Installation {
                    path: temp.path().join(protection),
                    launcher: Launcher::Unknown,
                    launch_profile: None,
                    available: true,
                },
            );
            if protection == "manual" {
                game.installations[0].launcher = Launcher::Manual;
            }
            store.save_game(&game).unwrap();
            match protection {
                "confirmed" => store.set_confirmed(&game.id, true).unwrap(),
                "favorite" => store.set_favorite(&game.id, true).unwrap(),
                "title" => store.set_title(&game.id, "Mon titre").unwrap(),
                "category" => store.set_category(&game.id, "Mes jeux").unwrap(),
                "cover" => store
                    .set_cover(&game.id, Some(&temp.path().join("cover.png")))
                    .unwrap(),
                _ => {}
            }
            assert_eq!(
                store
                    .ignore_obsolete_candidates([game.id.as_str()])
                    .unwrap(),
                usize::from(protection == "none"),
                "{protection}"
            );
            assert_eq!(store.is_ignored(&game.id).unwrap(), protection == "none");
            assert_eq!(
                store
                    .ignore_obsolete_candidates([game.id.as_str()])
                    .unwrap(),
                0
            );
            store.restore_ignored_game(&game.id).unwrap();
            assert!(!store.is_ignored(&game.id).unwrap());
        }
        assert_eq!(store.games().unwrap().len(), 7);
    }

    #[test]
    fn every_installation_path_reuses_its_stored_spelling() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let mut game = Game::from_installation(
            "Game",
            "Steam",
            Installation {
                path: temp.path().join("Games/First"),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        game.installations.push(Installation {
            path: temp.path().join("Games/Second"),
            ..game.installations[0].clone()
        });
        store.save_game(&game).unwrap();
        let mut scanned = game.clone();
        #[cfg(windows)]
        {
            scanned.installations[0].path = temp.path().join("games/first");
            scanned.installations[1].path = temp.path().join("games/second");
        }
        store.reuse_installation_identity(&mut scanned).unwrap();
        assert_eq!(scanned.installations, game.installations);
        store.save_game(&scanned).unwrap();
        assert_eq!(store.games().unwrap()[0].installations.len(), 2);
    }

    #[test]
    fn manual_readdition_does_not_replace_an_official_launcher_profile() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let profile = crate::model::LaunchProfile {
            command: PathBuf::new(),
            arguments: vec![],
            working_directory: None,
            launcher_command: Some("steam://rungameid/42".into()),
        };
        let official = Game::from_installation(
            "Official title",
            "Steam",
            Installation {
                path: temp.path().join("game"),
                launcher: Launcher::Steam,
                launch_profile: Some(profile.clone()),
                available: false,
            },
        );
        store.save_game(&official).unwrap();
        let mut manual = Game::from_installation(
            "run-game",
            "Ajout manuel",
            Installation {
                path: temp.path().join("game"),
                launcher: Launcher::Manual,
                launch_profile: Some(crate::model::LaunchProfile {
                    command: temp.path().join("game/run.exe"),
                    arguments: vec![],
                    working_directory: None,
                    launcher_command: None,
                }),
                available: true,
            },
        );
        store.save_manual_game(&mut manual).unwrap();
        let games = store.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Official title");
        assert_eq!(games[0].category, "Steam");
        assert_eq!(games[0].installations[0].launcher, Launcher::Steam);
        assert_eq!(
            games[0].installations[0].launch_profile.as_ref(),
            Some(&profile)
        );
        assert!(games[0].installations[0].available);
    }

    #[test]
    fn manual_addition_can_supply_a_missing_official_launch_profile() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let game = Game::from_installation(
            "Official title",
            "GOG",
            Installation {
                path: temp.path().join("game"),
                launcher: Launcher::Gog,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&game).unwrap();
        let mut manual = game.clone();
        manual.installations[0].launcher = Launcher::Manual;
        let profile = crate::model::LaunchProfile {
            command: temp.path().join("game/run.exe"),
            arguments: vec![],
            working_directory: None,
            launcher_command: None,
        };
        manual.installations[0].launch_profile = Some(profile.clone());
        store.save_manual_game(&mut manual).unwrap();
        let loaded = store.games().unwrap();
        assert_eq!(loaded[0].installations[0].launcher, Launcher::Gog);
        assert_eq!(
            loaded[0].installations[0].launch_profile.as_ref(),
            Some(&profile)
        );
    }

    #[test]
    fn manual_readdition_restores_an_ignored_game_atomically_and_keeps_corrections() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let mut game = Game::from_installation(
            "Old",
            "ROM",
            Installation {
                path: temp.path().join("game.iso"),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: false,
            },
        );
        store.save_game(&game).unwrap();
        store.set_title(&game.id, "Mon jeu").unwrap();
        store.set_favorite(&game.id, true).unwrap();
        store.set_category(&game.id, "Mes aventures").unwrap();
        store.ignore_game(&game.id).unwrap();
        let original_id = game.id.clone();
        game.id = "another-id".into();
        game.confirmed = true;
        game.installations[0].launcher = Launcher::Manual;
        game.installations[0].available = true;
        store.connection.execute_batch("CREATE TRIGGER prevent_restore BEFORE DELETE ON ignored_games BEGIN SELECT RAISE(ABORT,'refus'); END;").unwrap();
        assert!(store.save_manual_game(&mut game).is_err());
        let ignored = store.ignored_games().unwrap();
        assert_eq!(ignored.len(), 1);
        assert!(!ignored[0].confirmed);
        assert!(!ignored[0].installations[0].available);
        store
            .connection
            .execute_batch("DROP TRIGGER prevent_restore;")
            .unwrap();
        store.save_manual_game(&mut game).unwrap();
        assert_eq!(game.id, original_id);
        let restored = store.games().unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].title, "Mon jeu");
        assert_eq!(restored[0].category, "Mes aventures");
        assert!(restored[0].favorite && restored[0].confirmed);
        assert!(restored[0].installations[0].available);
        assert!(store.ignored_games().unwrap().is_empty());
    }

    #[test]
    fn custom_titles_survive_scan_reopen_and_can_restore_the_detected_name() {
        let temp = tempdir().unwrap();
        let database = temp.path().join("library.db");
        let game = Game::from_installation(
            "raw-file-name",
            "ROM",
            Installation {
                path: temp.path().join("game.iso"),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: false,
            },
        );
        {
            let store = Store::open(&database).unwrap();
            store.save_game(&game).unwrap();
            store.set_title(&game.id, "  L’Étoile  ").unwrap();
            store.set_favorite(&game.id, true).unwrap();
            store.save_game(&game).unwrap();
            assert_eq!(store.games().unwrap()[0].title, "L’Étoile");
            assert!(store.set_title(&game.id, " \n\t ").is_err());
            assert_eq!(store.games().unwrap()[0].title, "L’Étoile");
        }
        let store = Store::open(&database).unwrap();
        let loaded = store.games().unwrap().remove(0);
        assert_eq!(loaded.id, game.id);
        assert_eq!(loaded.title, "L’Étoile");
        assert!(loaded.favorite);
        store.reset_title(&game.id).unwrap();
        assert_eq!(store.games().unwrap()[0].title, "raw-file-name");
    }

    #[test]
    fn adding_custom_title_storage_preserves_old_libraries_and_distinct_games() {
        let temp = tempdir().unwrap();
        let database = temp.path().join("library.db");
        {
            let store = Store::open(&database).unwrap();
            for name in ["First", "Second"] {
                store
                    .save_game(&Game::from_installation(
                        name,
                        "Steam",
                        Installation {
                            path: temp.path().join(name),
                            launcher: Launcher::Steam,
                            launch_profile: None,
                            available: true,
                        },
                    ))
                    .unwrap();
            }
            store
                .connection
                .execute_batch("DROP TABLE title_overrides;")
                .unwrap();
        }
        let store = Store::open(&database).unwrap();
        let games = store.games().unwrap();
        assert_eq!(games.len(), 2);
        for game in &games {
            store.set_title(&game.id, "Identical display name").unwrap();
        }
        drop(store);
        let store = Store::open(&database).unwrap();
        let loaded = store.games().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded
            .iter()
            .all(|game| game.title == "Identical display name"));
        assert!(games
            .iter()
            .all(|game| loaded.iter().any(|other| other.id == game.id)));
    }

    #[test]
    fn migrating_legacy_ids_preserves_console_overrides_and_ignored_state() {
        let temp = tempdir().unwrap();
        let database = temp.path().join("library.db");
        let mut game = Game::from_installation(
            "Adventure",
            "ROM",
            Installation {
                path: temp.path().join("Adventure.iso"),
                launcher: Launcher::Manual,
                launch_profile: None,
                available: false,
            },
        );
        let new_id = game.id.clone();
        game.id = "legacy-image-id".into();
        {
            let store = Store::open(&database).unwrap();
            store.save_game(&game).unwrap();
            store.set_favorite(&game.id, true).unwrap();
            store.ignore_game(&game.id).unwrap();
            store.set_title(&game.id, "Mon ancienne aventure").unwrap();
            store
                .set_preference(
                    "console_overrides",
                    &serde_json::json!({game.id.clone(): "PlayStation 2"}).to_string(),
                )
                .unwrap();
        }
        for _ in 0..2 {
            let store = Store::open(&database).unwrap();
            assert!(store.games().unwrap().is_empty());
            let ignored = store.ignored_games().unwrap();
            assert_eq!(ignored.len(), 1);
            assert_eq!(ignored[0].id, new_id);
            assert_eq!(ignored[0].title, "Mon ancienne aventure");
            assert!(ignored[0].favorite);
            let overrides: std::collections::HashMap<String, String> =
                serde_json::from_str(&store.preference("console_overrides").unwrap().unwrap())
                    .unwrap();
            assert_eq!(
                overrides.get(&new_id).map(String::as_str),
                Some("PlayStation 2")
            );
            assert!(!overrides.contains_key(&game.id));
        }
    }

    #[test]
    fn batch_review_is_atomic_and_never_modifies_confirmed_games() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        store
            .connection
            .execute_batch(
                "INSERT INTO games(id,title,category,confirmed) VALUES
            ('a','A','Test',0),('b','B','Test',0),('c','C','Test',1);",
            )
            .unwrap();
        let ids = vec!["a".into(), "b".into(), "c".into(), "missing".into()];
        store.review_games(&ids, ReviewAction::Ignore).unwrap();
        assert_eq!(
            store
                .games()
                .unwrap()
                .iter()
                .map(|game| game.id.as_str())
                .collect::<Vec<_>>(),
            ["c"]
        );
        store.restore_ignored_games().unwrap();
        store.connection.execute_batch("CREATE TRIGGER reject_b BEFORE UPDATE ON games WHEN NEW.id='b' BEGIN SELECT RAISE(ABORT,'refus'); END;").unwrap();
        assert!(store.review_games(&ids, ReviewAction::Confirm).is_err());
        assert!(!store.games().unwrap()[0].confirmed);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_b;")
            .unwrap();
        store.review_games(&ids, ReviewAction::Confirm).unwrap();
        assert!(store.games().unwrap().iter().all(|game| game.confirmed));
    }

    #[test]
    fn large_library_keeps_installations_and_ignored_games_consistent() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        store.connection.execute_batch("BEGIN;
            WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<3000)
            INSERT INTO games(id,title,category) SELECT CAST(x AS TEXT),'Game '||x,'Test' FROM n;
            INSERT INTO installations(game_id,path,launcher,available) SELECT id,'/games/'||id,'manual',1 FROM games;
            INSERT INTO installations(game_id,path,launcher,available) SELECT id,'/backup/'||id,'manual',0 FROM games;
            INSERT INTO ignored_games(game_id) VALUES('10');
            INSERT INTO games(id,title,category) VALUES('empty','No installation','Test');
            COMMIT;").unwrap();
        let mut durations = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let games = store.games().unwrap();
            durations.push(start.elapsed());
            assert_eq!(games.len(), 3000);
            assert!(!games.iter().any(|game| game.id == "10"));
            assert!(games
                .iter()
                .find(|game| game.id == "empty")
                .unwrap()
                .installations
                .is_empty());
            assert!(games.iter().filter(|game| game.id != "empty").all(|game| {
                game.installations.len() == 2
                    && game.installations[0].path.starts_with("/backup")
                    && !game.installations[0].available
                    && game.installations[1].available
            }));
        }
        durations.sort();
        eprintln!(
            "Catalogue 3000 jeux / 6000 installations : médiane {:?}",
            durations[2]
        );
    }

    #[test]
    fn installation_identity_uses_platform_case_rules() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let install = Installation {
            path: temp.path().join("Games/Game"),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        };
        let old = Game::from_installation("Old title", "À confirmer", install.clone());
        store.save_game(&old).unwrap();
        let mut new = Game::from_installation(
            "New title",
            "Steam",
            Installation {
                path: temp.path().join("games/game"),
                launcher: Launcher::Steam,
                ..install
            },
        );
        store.reuse_installation_identity(&mut new).unwrap();
        #[cfg(windows)]
        {
            assert_eq!(new.id, old.id);
            assert_eq!(new.installations[0].path, old.installations[0].path);
            store.save_game(&new).unwrap();
            assert_eq!(store.games().unwrap()[0].installations.len(), 1);
        }
        #[cfg(not(windows))]
        assert_ne!(new.id, old.id);
    }

    #[test]
    fn recognized_title_reuses_unknown_installation_and_preserves_edits() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let install = Installation {
            path: temp.path().join("game"),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        };
        let unknown = Game::from_installation("game-win64", "À confirmer", install.clone());
        store.save_game(&unknown).unwrap();
        store.set_category(&unknown.id, "Mes jeux").unwrap();
        store.set_favorite(&unknown.id, true).unwrap();
        let mut known = Game::from_installation(
            "Official title",
            "Steam",
            Installation {
                launcher: Launcher::Steam,
                ..install
            },
        );
        store.reuse_installation_identity(&mut known).unwrap();
        assert_eq!(known.id, unknown.id);
        store.save_game(&known).unwrap();
        let games = store.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Official title");
        assert_eq!(games[0].category, "Mes jeux");
        assert!(games[0].favorite && games[0].confirmed);
    }

    #[test]
    fn rescans_preserve_user_cover_even_when_the_image_is_offline() {
        let temp = tempdir().unwrap();
        let db = temp.path().join("library.db");
        let store = Store::open(&db).unwrap();
        let mut game = Game::from_installation(
            "Adventure",
            "Steam",
            Installation {
                path: temp.path().join("game"),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        game.cover_path = Some(temp.path().join("cache.png"));
        store.save_game(&game).unwrap();
        let chosen = temp.path().join("disconnected/Été.png");
        store.set_cover(&game.id, Some(&chosen)).unwrap();
        store.save_game(&game).unwrap();
        drop(store);
        let store = Store::open(&db).unwrap();
        assert_eq!(store.games().unwrap()[0].cover_path.as_ref(), Some(&chosen));
        let replacement = temp.path().join("replacement.png");
        store.set_cover(&game.id, Some(&replacement)).unwrap();
        assert_eq!(
            store.games().unwrap()[0].cover_path.as_ref(),
            Some(&replacement)
        );
    }

    #[test]
    fn separates_legacy_image_groups_without_losing_favorites_or_manual_categories() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("library.db");
        let store = Store::open(&path).unwrap();
        let mut game = Game::from_installation(
            "Adventure",
            "ROM · PlayStation 2",
            Installation {
                path: temp.path().join("one.iso"),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        game.id = "rom-old-group".into();
        game.favorite = true;
        game.installations.push(Installation {
            path: temp.path().join("two.iso"),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        });
        store.save_game(&game).unwrap();
        store
            .set_preference("console_overrides", "{\"rom-old-group\":\"PlayStation 2\"}")
            .unwrap();
        drop(store);
        for _ in 0..2 {
            let reopened = Store::open(&path).unwrap();
            let games = reopened.games().unwrap();
            assert_eq!(games.len(), 2);
            assert!(games
                .iter()
                .all(|game| game.favorite && game.installations.len() == 1));
            let categories: std::collections::HashMap<String, String> =
                serde_json::from_str(&reopened.preference("console_overrides").unwrap().unwrap())
                    .unwrap();
            assert!(games
                .iter()
                .all(|game| categories.get(&game.id).map(String::as_str) == Some("PlayStation 2")));
        }
    }

    #[test]
    fn rescanning_an_old_image_preserves_its_identity_and_corrections() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let install = Installation {
            path: temp.path().join("game.iso"),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        };
        let mut old = Game::from_installation("Same title", "ROM · PlayStation 2", install.clone());
        old.id = "rom-legacy-id".into();
        store.save_game(&old).unwrap();
        store.set_favorite(&old.id, true).unwrap();
        let mut rescanned =
            Game::from_installation("Same title", "ROM · Images à classer", install);
        assert_ne!(rescanned.id, old.id);
        store.reuse_installation_identity(&mut rescanned).unwrap();
        assert_eq!(rescanned.id, old.id);
        store.save_game(&rescanned).unwrap();
        let library = store.games().unwrap();
        assert_eq!(library.len(), 1);
        assert!(library[0].favorite);
        assert_eq!(
            crate::rom::game_console(&library[0]),
            Some(crate::rom::UNCLASSIFIED)
        );
    }

    #[test]
    fn backup_preserves_preferences_and_rejects_the_active_database() {
        let temp = tempdir().unwrap();
        let active = temp.path().join("active.sqlite3");
        let backup = temp.path().join("backup.sqlite3");
        let store = Store::open(&active).unwrap();
        store
            .set_preference("console_emulators", "{\"NES\":\"retroarch\"}")
            .unwrap();
        assert!(store.backup_to(&active).is_err());
        store.backup_to(&backup).unwrap();
        let restored = Store::open(&backup).unwrap();
        assert_eq!(
            restored.preference("console_emulators").unwrap(),
            Some("{\"NES\":\"retroarch\"}".into())
        );
    }

    #[test]
    fn same_title_on_two_consoles_stays_separate_after_reopening() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("library.db");
        let store = Store::open(&path).unwrap();
        for extension in ["nes", "sfc"] {
            let game = Game::from_installation(
                "Adventure",
                "ROM",
                Installation {
                    path: temp.path().join(format!("adventure.{extension}")),
                    launcher: Launcher::Unknown,
                    launch_profile: None,
                    available: false,
                },
            );
            store.save_game(&game).unwrap();
        }
        drop(store);
        assert_eq!(Store::open(&path).unwrap().games().unwrap().len(), 2);
    }

    #[test]
    fn ignored_candidates_stay_hidden_after_rescan_and_can_be_restored() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let candidate = Game::from_installation(
            "Utility",
            "À confirmer",
            Installation {
                path: temp.path().join("utility"),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&candidate).unwrap();
        store.ignore_game(&candidate.id).unwrap();
        store.save_game(&candidate).unwrap();
        assert!(store.is_ignored(&candidate.id).unwrap());
        assert!(store.games().unwrap().is_empty());
        store.restore_ignored_games().unwrap();
        assert_eq!(store.games().unwrap().len(), 1);
    }
    #[test]
    fn round_trip_keeps_favorite_and_multiple_installs() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let mut game = Game::from_installation(
            "Test",
            "Steam",
            Installation {
                path: PathBuf::from("/games/a"),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&game).unwrap();
        store.set_favorite(&game.id, true).unwrap();
        game.installations.push(Installation {
            path: PathBuf::from("/backup/a"),
            launcher: Launcher::Manual,
            launch_profile: None,
            available: false,
        });
        store.save_game(&game).unwrap();
        let loaded = store.games().unwrap();
        assert!(loaded[0].favorite);
        assert_eq!(loaded[0].installations.len(), 2);
    }

    #[test]
    fn unknown_scan_does_not_downgrade_known_metadata_or_user_category() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let install = Installation {
            path: PathBuf::from("/games/celeste"),
            launcher: Launcher::Steam,
            launch_profile: Some(crate::model::LaunchProfile {
                command: PathBuf::new(),
                arguments: vec![],
                working_directory: None,
                launcher_command: Some("steam://rungameid/42".into()),
            }),
            available: true,
        };
        let game = Game::from_installation("Celeste", "Steam", install.clone());
        store.save_game(&game).unwrap();
        store.set_category(&game.id, "Plateforme").unwrap();
        store
            .save_game(&Game::from_installation(
                "celeste",
                "À confirmer",
                Installation {
                    path: install.path.clone(),
                    launcher: Launcher::Unknown,
                    launch_profile: Some(crate::model::LaunchProfile {
                        command: PathBuf::from("wrong.exe"),
                        arguments: vec![],
                        working_directory: None,
                        launcher_command: None,
                    }),
                    available: true,
                },
            ))
            .unwrap();
        let loaded_games = store.games().unwrap();
        let loaded = &loaded_games[0];
        assert_eq!(loaded.category, "Plateforme");
        assert_eq!(loaded.title, "Celeste");
        assert_eq!(
            loaded.installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .launcher_command
                .as_deref(),
            Some("steam://rungameid/42")
        );
        assert_eq!(loaded.installations[0].launcher, Launcher::Steam);
        assert!(loaded.confirmed);
    }

    #[test]
    fn same_named_emulators_and_existing_manual_installations_survive_rescans() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for folder in ["stable", "nightly"] {
            store
                .save_emulator(&crate::model::Emulator::new(
                    "Dolphin",
                    temp.path().join(folder).join("dolphin"),
                ))
                .unwrap();
        }
        assert_eq!(store.emulators().unwrap().len(), 2);
        let game = Game::from_installation(
            "Manual",
            "Ajout manuel",
            Installation {
                path: temp.path().to_path_buf(),
                launcher: Launcher::Manual,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&game).unwrap();
        store
            .mark_unseen_under_roots_unavailable(&[temp.path().to_path_buf()], &[], &[])
            .unwrap();
        assert!(store.games().unwrap()[0].installations[0].available);
    }

    #[test]
    fn rescanning_one_root_marks_only_unseen_installations_unavailable() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let inside = temp.path().join("games/a");
        let outside = PathBuf::from("/other-drive/games/b");
        let mut game = Game::from_installation(
            "A",
            "Steam",
            Installation {
                path: inside.clone(),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        game.installations.push(Installation {
            path: outside.clone(),
            launcher: Launcher::Manual,
            launch_profile: None,
            available: true,
        });
        store.save_game(&game).unwrap();
        store
            .mark_unseen_under_roots_unavailable(&[temp.path().to_path_buf()], &[], &[])
            .unwrap();
        let loaded = store.games().unwrap();
        assert!(
            !loaded[0]
                .installations
                .iter()
                .find(|i| i.path == inside)
                .unwrap()
                .available
        );
        assert!(
            loaded[0]
                .installations
                .iter()
                .find(|i| i.path == outside)
                .unwrap()
                .available
        );
    }

    #[test]
    fn opening_the_library_merges_legacy_per_installation_ids() {
        use std::hash::{Hash, Hasher};
        let temp = tempdir().unwrap();
        let db = temp.path().join("library.db");
        let store = Store::open(&db).unwrap();
        let first = Game::from_installation(
            "Moonlight Arcade",
            "Steam",
            Installation {
                path: PathBuf::from("/games/one"),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&first).unwrap();
        drop(store);

        let old_path = "/backup/moonlight";
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        "moonlight arcade".hash(&mut hasher);
        old_path.hash(&mut hasher);
        let old_id = format!("{:016x}", hasher.finish());
        let connection = Connection::open(&db).unwrap();
        connection.execute("INSERT INTO games(id,title,category,favorite,confirmed) VALUES(?1,'Moonlight Arcade','À confirmer',1,0)", [&old_id]).unwrap();
        connection.execute("INSERT INTO installations(game_id,path,launcher,available) VALUES(?1,?2,'unknown',1)", params![old_id, old_path]).unwrap();
        drop(connection);

        let migrated = Store::open(&db).unwrap().games().unwrap();
        assert_eq!(migrated.len(), 1);
        assert!(migrated[0].favorite);
        assert_eq!(migrated[0].installations.len(), 2);
        assert!(migrated[0].confirmed);
    }
}
