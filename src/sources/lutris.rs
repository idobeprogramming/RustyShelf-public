use crate::model::{Game, Installation, LaunchProfile, Launcher};
use std::{fs, path::PathBuf};

pub fn discover() -> Vec<Game> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let roots = [
        home.join(".config/lutris/games"),
        home.join(".var/app/net.lutris.Lutris/config/lutris/games"),
    ];
    let mut result = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext == "yml" || ext == "yaml")
            {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let name = yaml_scalar(&text, "name")
                .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()));
            let Some(name) = name else { continue };
            let slug = yaml_scalar(&text, "game_slug")
                .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()));
            let executable = yaml_scalar(&text, "exe").or_else(|| yaml_scalar(&text, "executable"));
            let profile = executable.map(|exe| {
                let command = expand_home(exe);
                let working_directory = command.parent().map(PathBuf::from);
                LaunchProfile {
                    command,
                    arguments: Vec::new(),
                    working_directory,
                    launcher_command: slug.map(|slug| format!("lutris:{slug}")),
                }
            });
            let install = profile
                .as_ref()
                .and_then(|p| p.command.parent())
                .unwrap_or_else(|| path.parent().unwrap_or(&path))
                .to_path_buf();
            result.push(Game::from_installation(
                name,
                "Lutris",
                Installation {
                    path: install,
                    launcher: Launcher::Lutris,
                    launch_profile: profile,
                    available: true,
                },
            ));
        }
    }
    result
}

fn yaml_scalar(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| {
            let (candidate, value) = line.trim().split_once(':')?;
            if candidate != key {
                return None;
            }
            Some(value.trim().trim_matches(['\'', '"']).to_string())
        })
        .filter(|value| !value.is_empty())
}
fn expand_home(value: String) -> PathBuf {
    if let Some(suffix) = value.strip_prefix("~/") {
        return dirs::home_dir().unwrap_or_default().join(suffix);
    }
    PathBuf::from(value)
}
