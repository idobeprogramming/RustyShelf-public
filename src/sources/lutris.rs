use crate::model::{Game, Installation, LaunchProfile, Launcher};
use crate::scan::{ScanControl, ScanEvent};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn discover(control: &ScanControl, report: &mut dyn FnMut(ScanEvent)) -> Vec<Game> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let roots = [
        home.join(".config/lutris/games"),
        home.join(".var/app/net.lutris.Lutris/config/lutris/games"),
    ];
    let mut result = Vec::new();
    for root in roots {
        if control.is_cancelled() {
            break;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if control.is_cancelled() {
                break;
            }
            let path = entry.path();
            if !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext == "yml" || ext == "yaml")
            {
                continue;
            }
            let Some(text) = super::read_or_report(&path, report) else {
                continue;
            };
            match from_config(&text, &path) {
                Some(game) => result.push(game),
                None => report(ScanEvent::Skipped(
                    path,
                    "Profil Lutris sans exécutable local résolu".into(),
                )),
            }
        }
    }
    result
}

fn from_config(text: &str, path: &Path) -> Option<Game> {
    let name = yaml_scalar(text, "name")
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    let slug = yaml_scalar(text, "game_slug")
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    let executable = yaml_scalar(text, "exe").or_else(|| yaml_scalar(text, "executable"))?;
    let mut command = expand_home(executable);
    if !command.is_absolute() {
        let working_dir = expand_home(yaml_scalar(text, "working_dir")?);
        if !working_dir.is_absolute() {
            return None;
        }
        command = working_dir.join(command);
    }
    if !command.is_file() {
        return None;
    }
    let install = command.parent()?.to_owned();
    Some(Game::from_installation(
        name,
        "Lutris",
        Installation {
            path: install.clone(),
            launcher: Launcher::Lutris,
            available: true,
            launch_profile: Some(LaunchProfile {
                command,
                arguments: Vec::new(),
                working_directory: Some(install),
                launcher_command: Some(format!("lutris:{slug}")),
            }),
        },
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_lutris_profiles_do_not_turn_the_config_folder_into_a_game() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("game.yml");
        assert!(from_config("name: Incomplete\ngame_slug: incomplete", &config).is_none());
        assert!(from_config("name: Missing\nexe: missing.exe", &config).is_none());
        let executable = temp.path().join("Été.exe");
        fs::write(&executable, []).unwrap();
        let text = format!("name: Été\ngame_slug: ete\nexe: '{}'", executable.display());
        let game = from_config(&text, &config).unwrap();
        assert_eq!(game.installations[0].path, temp.path());
        assert_eq!(
            game.installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .launcher_command
                .as_deref(),
            Some("lutris:ete")
        );
    }
}
