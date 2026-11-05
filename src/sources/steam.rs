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
        let Ok(entries) = fs::read_dir(&library) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_manifest(&path) {
                if let Ok(Some(game)) = read_manifest(&path) {
                    result.push(game);
                }
            }
        }
    }
    result
}

pub fn is_manifest(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name.starts_with("appmanifest_") && name.ends_with(".acf")
            })
}

pub fn read_manifest(path: &Path) -> Result<Option<Game>, String> {
    let text = super::read_manifest_text(path)?;
    let tokens = quoted_tokens(&text)?;
    let field = |key: &str| {
        tokens
            .windows(2)
            .find(|pair| pair[0] == key)
            .map(|pair| pair[1].clone())
    };
    let name = field("name")
        .filter(|name| !name.trim().is_empty())
        .ok_or("Titre Steam manquant")?;
    let app_id = field("appid")
        .filter(|id| super::numeric_id(id))
        .ok_or("Identifiant Steam invalide")?;
    let install_dir = field("installdir").ok_or("Dossier Steam manquant")?;
    let common = path
        .parent()
        .ok_or("Dossier Steam manquant")?
        .join("common");
    let install = super::relative_path(&common, &install_dir)?;
    if !install.is_dir() {
        return Ok(None);
    }
    Ok(Some(Game::from_installation(
        name,
        "Steam",
        Installation {
            path: install.clone(),
            launcher: Launcher::Steam,
            launch_profile: Some(LaunchProfile {
                command: PathBuf::new(),
                arguments: Vec::new(),
                working_directory: Some(install),
                launcher_command: Some(format!("steam://rungameid/{app_id}")),
            }),
            available: true,
        },
    )))
}

fn value(text: &str, key: &str) -> Option<String> {
    quoted_tokens(text)
        .ok()?
        .windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1].clone())
}

// Valve files can contain multiple fields on one line and escaped quotes.
fn quoted_tokens(text: &str) -> Result<Vec<String>, String> {
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'/') {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
        } else if ch == '"' {
            let mut value = String::new();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == '"' {
                    closed = true;
                    break;
                }
                if ch == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                    value.push(chars.next().unwrap());
                } else {
                    value.push(ch);
                }
            }
            if !closed {
                return Err("Guillemet non fermé dans le manifeste Steam".into());
            }
            tokens.push(value);
        }
    }
    Ok(tokens)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn handles_escaped_titles_and_rejects_invalid_ids_and_paths() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("steamapps");
        fs::create_dir_all(library.join("common/Game")).unwrap();
        let path = library.join("appmanifest_42.acf");
        let manifest = r#""AppState" { "appid" "42" "name" "The \"Game\"" "installdir" "Game" }"#;
        fs::write(&path, manifest).unwrap();
        assert_eq!(read_manifest(&path).unwrap().unwrap().title, "The \"Game\"");
        for text in [
            manifest.replace("\"42\"", "\"42?bad\""),
            manifest.replace("\"installdir\" \"Game\"", "\"installdir\" \"../outside\""),
            manifest.replace("\"installdir\" \"Game\"", "\"installdir\" \"C:/outside\""),
            "\"name\" \"unterminated".into(),
        ] {
            fs::write(&path, text).unwrap();
            assert!(read_manifest(&path).is_err());
        }
    }
    #[test]
    fn finds_local_manifest_in_secondary_library() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("SteamLibrary/steamapps");
        let installed = library.join("common/Été Game");
        fs::create_dir_all(&installed).unwrap();
        fs::write(library.join("appmanifest_42.acf"), "\"AppState\" {\n\"appid\" \"42\"\n\"name\" \"Été Game\"\n\"installdir\" \"Été Game\"\n}").unwrap();
        let found = [read_manifest(&library.join("appmanifest_42.acf"))
            .unwrap()
            .unwrap()];
        assert!(found
            .iter()
            .any(|game| game.title == "Été Game"
                && game.installations[0].launcher == Launcher::Steam));
    }
}
