use crate::model::{Game, Installation, Launcher};
use rusqlite::{params, Connection, OptionalExtension};
use std::{fs, path::PathBuf};

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open_default() -> rusqlite::Result<Self> {
        let root = dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("GameShelf");
        fs::create_dir_all(&root)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        Self::open(root.join("gameshelf.sqlite3"))
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
             CREATE TABLE IF NOT EXISTS excluded_paths (path TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS ignored_games (game_id TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS emulators (id TEXT PRIMARY KEY, name TEXT NOT NULL, executable TEXT NOT NULL UNIQUE);
             CREATE TABLE IF NOT EXISTS game_emulators (game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE, emulator_id TEXT NOT NULL REFERENCES emulators(id) ON DELETE CASCADE);
             CREATE TABLE IF NOT EXISTS categories (name TEXT PRIMARY KEY);",
        )?;
        let store = Self { connection };
        store.merge_legacy_duplicates()?;
        Ok(store)
    }

    fn merge_legacy_duplicates(&self) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
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
            let new_id = crate::model::stable_id(&title, std::path::Path::new(""));
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
            tx.execute("DELETE FROM games WHERE id=?1", [&old_id])?;
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

    pub fn set_preference(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
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

    pub fn set_game_emulator(
        &self,
        game_id: &str,
        emulator_id: Option<&str>,
    ) -> rusqlite::Result<()> {
        if let Some(emulator_id) = emulator_id {
            self.connection.execute("INSERT INTO game_emulators(game_id,emulator_id) VALUES(?1,?2) ON CONFLICT(game_id) DO UPDATE SET emulator_id=excluded.emulator_id", params![game_id, emulator_id])?;
        } else {
            self.connection
                .execute("DELETE FROM game_emulators WHERE game_id=?1", [game_id])?;
        }
        Ok(())
    }

    pub fn game_emulator(&self, game_id: &str) -> rusqlite::Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT emulator_id FROM game_emulators WHERE game_id=?1",
                [game_id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn save_game(&self, game: &Game) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let existing: Option<(bool, Option<String>, String, bool, bool)> = tx
            .query_row(
                "SELECT favorite, cover_path, category, category_custom, confirmed FROM games WHERE id=?1",
                [&game.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        let favorite = existing.as_ref().map_or(game.favorite, |row| row.0);
        let cover = game
            .cover_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| existing.as_ref().and_then(|row| row.1.clone()));
        let category = existing
            .as_ref()
            .filter(|row| row.3 || (row.4 && !game.confirmed))
            .map_or_else(|| game.category.clone(), |row| row.2.clone());
        tx.execute(
            "INSERT INTO games(id,title,category,favorite,confirmed,cover_path) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, category=excluded.category,
             confirmed=MAX(excluded.confirmed,games.confirmed), cover_path=COALESCE(excluded.cover_path,games.cover_path)",
            params![game.id, game.title, category, favorite, game.confirmed, cover],
        )?;
        for installation in &game.installations {
            let profile = installation
                .launch_profile
                .as_ref()
                .and_then(|p| serde_json::to_string(p).ok());
            tx.execute(
                "INSERT INTO installations(game_id,path,launcher,profile,available) VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(game_id,path) DO UPDATE SET
                 launcher=CASE WHEN excluded.launcher='unknown' AND installations.launcher<>'unknown' THEN installations.launcher ELSE excluded.launcher END,
                 profile=COALESCE(excluded.profile,installations.profile), available=excluded.available",
                params![game.id, installation.path.to_string_lossy(), launcher_name(&installation.launcher), profile, installation.available],
            )?;
        }
        tx.commit()
    }

    pub fn games(&self) -> rusqlite::Result<Vec<Game>> {
        let mut statement = self.connection.prepare("SELECT id,title,category,favorite,confirmed,cover_path FROM games WHERE NOT EXISTS (SELECT 1 FROM ignored_games WHERE game_id=games.id) ORDER BY title COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| {
            let cover: Option<String> = row.get(5)?;
            Ok(Game {
                id: row.get(0)?,
                title: row.get(1)?,
                category: row.get(2)?,
                favorite: row.get(3)?,
                confirmed: row.get(4)?,
                cover_path: cover.map(PathBuf::from),
                installations: Vec::new(),
            })
        })?;
        let mut games: Vec<Game> = rows.collect::<rusqlite::Result<_>>()?;
        for game in &mut games {
            let mut stmt = self.connection.prepare("SELECT path,launcher,profile,available FROM installations WHERE game_id=?1 ORDER BY path")?;
            let rows = stmt.query_map([&game.id], |row| {
                let profile: Option<String> = row.get(2)?;
                Ok(Installation {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    launcher: parse_launcher(&row.get::<_, String>(1)?),
                    launch_profile: profile.and_then(|json| serde_json::from_str(&json).ok()),
                    available: row.get(3)?,
                })
            })?;
            game.installations = rows.collect::<rusqlite::Result<_>>()?;
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

    pub fn set_confirmed(&self, id: &str, confirmed: bool) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET confirmed=?2 WHERE id=?1",
            params![id, confirmed],
        )?;
        Ok(())
    }

    pub fn ignore_game(&self, id: &str) -> rusqlite::Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO ignored_games(game_id) VALUES(?1)",
            [id],
        )?;
        Ok(())
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

    pub fn set_cover(&self, id: &str, path: Option<&std::path::Path>) -> rusqlite::Result<()> {
        self.connection.execute(
            "UPDATE games SET cover_path=?2 WHERE id=?1",
            params![id, path.map(|p| p.to_string_lossy().to_string())],
        )?;
        Ok(())
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
            let mut query = tx.prepare("SELECT path FROM installations")?;
            let paths = query
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            paths
        };
        for value in paths {
            let path = PathBuf::from(value);
            if roots.iter().any(|root| path.starts_with(root))
                && !exclusions.iter().any(|excluded| path.starts_with(excluded))
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
            launch_profile: None,
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
                    launch_profile: None,
                    available: true,
                },
            ))
            .unwrap();
        let loaded_games = store.games().unwrap();
        let loaded = &loaded_games[0];
        assert_eq!(loaded.category, "Plateforme");
        assert_eq!(loaded.installations[0].launcher, Launcher::Steam);
        assert!(loaded.confirmed);
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

    #[test]
    fn stores_default_and_per_game_emulator_choices() {
        let temp = tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let game = Game::from_installation(
            "Disc Game",
            "Image disque",
            Installation {
                path: PathBuf::from("/roms/game.iso"),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        store.save_game(&game).unwrap();
        let emulator = crate::model::Emulator::new("Dolphin", PathBuf::from("/usr/bin/dolphin"));
        store.save_emulator(&emulator).unwrap();
        store
            .set_preference("preferred_emulator", &emulator.id)
            .unwrap();
        store
            .set_game_emulator(&game.id, Some(&emulator.id))
            .unwrap();
        assert_eq!(
            store.preference("preferred_emulator").unwrap().as_deref(),
            Some(emulator.id.as_str())
        );
        assert_eq!(
            store.game_emulator(&game.id).unwrap().as_deref(),
            Some(emulator.id.as_str())
        );
    }
}
