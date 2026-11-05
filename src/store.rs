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
             CREATE TABLE IF NOT EXISTS categories (name TEXT PRIMARY KEY);",
        )?;
        Ok(Self { connection })
    }

    pub fn save_game(&self, game: &Game) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        let existing: Option<(bool, Option<String>)> = tx
            .query_row(
                "SELECT favorite, cover_path FROM games WHERE id=?1",
                [&game.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let favorite = existing.as_ref().map_or(game.favorite, |row| row.0);
        let cover = game
            .cover_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| existing.and_then(|row| row.1));
        tx.execute(
            "INSERT INTO games(id,title,category,favorite,confirmed,cover_path) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, category=excluded.category,
             confirmed=excluded.confirmed, cover_path=COALESCE(excluded.cover_path,games.cover_path)",
            params![game.id, game.title, game.category, favorite, game.confirmed, cover],
        )?;
        for installation in &game.installations {
            let profile = installation
                .launch_profile
                .as_ref()
                .and_then(|p| serde_json::to_string(p).ok());
            tx.execute(
                "INSERT INTO installations(game_id,path,launcher,profile,available) VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(game_id,path) DO UPDATE SET launcher=excluded.launcher, profile=excluded.profile, available=excluded.available",
                params![game.id, installation.path.to_string_lossy(), launcher_name(&installation.launcher), profile, installation.available],
            )?;
        }
        tx.commit()
    }

    pub fn games(&self) -> rusqlite::Result<Vec<Game>> {
        let mut statement = self.connection.prepare("SELECT id,title,category,favorite,confirmed,cover_path FROM games ORDER BY title COLLATE NOCASE")?;
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

    pub fn category_is_custom(&self, id: &str) -> rusqlite::Result<bool> {
        self.connection.query_row(
            "SELECT category_custom FROM games WHERE id=?1",
            [id],
            |row| row.get(0),
        )
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

    pub fn mark_unseen_unavailable(&self, seen_paths: &[PathBuf]) -> rusqlite::Result<()> {
        let tx = self.connection.unchecked_transaction()?;
        tx.execute("UPDATE installations SET available=0", [])?;
        for path in seen_paths {
            tx.execute(
                "UPDATE installations SET available=1 WHERE path=?1",
                [path.to_string_lossy()],
            )?;
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
}
