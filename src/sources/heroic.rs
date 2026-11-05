use crate::model::{Game, Installation, LaunchProfile, Launcher};
use std::{fs, path::PathBuf};
use walkdir::WalkDir;

pub fn discover() -> Vec<Game> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.extend([
            home.join(".config/heroic/store"),
            home.join(".config/heroic/legendary"),
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/store"),
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/legendary"),
        ]);
    }
    #[cfg(windows)]
    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(app_data).join("heroic"));
    }
    let mut result = Vec::new();
    for root in roots {
        for entry in WalkDir::new(root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|s| s.to_str()) != Some("json")
            {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let title = find_string(&json, &["title", "app_title", "displayName", "name"]);
            let location = find_string(&json, &["install_path", "installPath", "path"]);
            let (Some(title), Some(location)) = (title, location) else {
                continue;
            };
            let install = PathBuf::from(location);
            if !install.is_dir() {
                continue;
            }
            let launcher = if entry
                .path()
                .to_string_lossy()
                .to_lowercase()
                .contains("gog")
            {
                Launcher::Gog
            } else {
                Launcher::Heroic
            };
            let executable = find_string(&json, &["executable", "launchExecutable", "exe"]);
            let profile = executable.map(|exe| {
                let command = PathBuf::from(exe);
                let command = if command.is_absolute() {
                    command
                } else {
                    install.join(command)
                };
                LaunchProfile {
                    command,
                    arguments: Vec::new(),
                    working_directory: Some(install.clone()),
                    launcher_command: None,
                }
            });
            result.push(Game::from_installation(
                title,
                launcher.label(),
                Installation {
                    path: install,
                    launcher,
                    launch_profile: profile,
                    available: true,
                },
            ));
        }
    }
    result
}

fn find_string<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(text) = object.get(*key).and_then(|v| v.as_str()) {
                return Some(text);
            }
        }
        for child in object.values() {
            if let Some(text) = find_string(child, keys) {
                return Some(text);
            }
        }
    }
    if let Some(array) = value.as_array() {
        for child in array {
            if let Some(text) = find_string(child, keys) {
                return Some(text);
            }
        }
    }
    None
}
