use crate::model::{Game, Installation, LaunchProfile, Launcher};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn discover(roots: &[PathBuf]) -> Vec<Game> {
    let mut steamapps = default_steamapps();
    steamapps.extend(
        roots
            .iter()
            .flat_map(|root| [root.join("SteamLibrary/steamapps"), root.join("steamapps")]),
    );
    let mut result = Vec::new();
    let mut index = 0;
    while index < steamapps.len() {
        let library = steamapps[index].clone();
        index += 1;
        if !library.is_dir() {
            continue;
        }
        if let Ok(vdf) = fs::read_to_string(library.join("libraryfolders.vdf")) {
            for line in vdf.lines().filter(|line| line.contains("\"path\"")) {
                if let Some(path) = value(line, "path") {
                    let candidate = PathBuf::from(path.replace("\\\\", "\\")).join("steamapps");
                    if !steamapps.contains(&candidate) {
                        steamapps.push(candidate);
                    }
                }
            }
        }
        let common = library.join("common");
        let manifest_root = library.clone();
        let Ok(entries) = fs::read_dir(&manifest_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.starts_with("appmanifest_") && name.ends_with(".acf"))
            {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(name) = value(&text, "name") else {
                continue;
            };
            let Some(install_dir) = value(&text, "installdir") else {
                continue;
            };
            let app_id = value(&text, "appid").unwrap_or_default();
            let install = common.join(install_dir);
            if !install.exists() {
                continue;
            }
            let profile = LaunchProfile {
                command: PathBuf::new(),
                arguments: Vec::new(),
                working_directory: Some(install.clone()),
                launcher_command: Some(format!("steam://rungameid/{app_id}")),
            };
            result.push(Game::from_installation(
                name,
                "Steam",
                Installation {
                    path: install,
                    launcher: Launcher::Steam,
                    launch_profile: Some(profile),
                    available: true,
                },
            ));
        }
    }
    result
}

fn value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line
            .trim()
            .split('"')
            .filter(|part| !part.trim().is_empty());
        let (Some(candidate), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        if candidate.trim() == key {
            return Some(value.trim().replace("\\\"", "\""));
        }
    }
    None
}

fn default_steamapps() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        paths.extend([
            home.join(".steam/steam/steamapps"),
            home.join(".local/share/Steam/steamapps"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"),
        ]);
    }
    #[cfg(windows)]
    {
        if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
            paths.push(PathBuf::from(program_files).join("Steam/steamapps"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            paths.push(PathBuf::from(program_files).join("Steam/steamapps"));
        }
    }
    paths
}

#[allow(dead_code)]
fn _is_child(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn finds_local_manifest_in_secondary_library() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("SteamLibrary/steamapps");
        let installed = library.join("common/Été Game");
        fs::create_dir_all(&installed).unwrap();
        fs::write(library.join("appmanifest_42.acf"), "\"AppState\" {\n\"appid\" \"42\"\n\"name\" \"Été Game\"\n\"installdir\" \"Été Game\"\n}").unwrap();
        let found = discover(&[temp.path().to_path_buf()]);
        assert!(found
            .iter()
            .any(|game| game.title == "Été Game"
                && game.installations[0].launcher == Launcher::Steam));
    }
}
