use crate::model::{Game, Installation, LaunchProfile, Launcher};
use crate::scan::{ScanControl, ScanEvent};
use std::{fs, path::PathBuf};

pub fn discover(control: &ScanControl, report: &mut dyn FnMut(ScanEvent)) -> Vec<Game> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        if let Some(program_data) = std::env::var_os("ProgramData") {
            roots.push(PathBuf::from(program_data).join("Epic/EpicGamesLauncher/Data/Manifests"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".config/heroic/legendary/metadata"));
            roots.push(
                home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/legendary/metadata"),
            );
        }
    }
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
            if path.extension().and_then(|s| s.to_str()) != Some("item")
                && path.extension().and_then(|s| s.to_str()) != Some("json")
            {
                continue;
            }
            let Some(text) = super::read_or_report(&path, report) else {
                continue;
            };
            let json = match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(json) => json,
                Err(error) => {
                    report(ScanEvent::Error(path.clone(), error.to_string()));
                    continue;
                }
            };
            let Some(title) = json
                .get("DisplayName")
                .or_else(|| json.get("title"))
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            let Some(location) = json
                .get("InstallLocation")
                .or_else(|| json.get("install_path"))
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            let install = PathBuf::from(location);
            if !install.is_dir() {
                continue;
            }
            let executable = json
                .get("LaunchExecutable")
                .and_then(|v| v.as_str())
                .map(|exe| install.join(exe));
            let profile = executable.map(|command| LaunchProfile {
                command,
                arguments: Vec::new(),
                working_directory: Some(install.clone()),
                launcher_command: None,
            });
            result.push(Game::from_installation(
                title,
                "Epic Games",
                Installation {
                    path: install,
                    launcher: Launcher::Epic,
                    launch_profile: profile,
                    available: true,
                },
            ));
        }
    }
    result
}
