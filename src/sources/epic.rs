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
            match from_manifest(&text) {
                Ok(Some(game)) => result.push(game),
                Ok(None) => {}
                Err(error) => report(ScanEvent::Error(path, error)),
            }
        }
    }
    result
}

fn from_manifest(text: &str) -> Result<Option<Game>, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    if json
        .get("bIsIncompleteInstall")
        .and_then(|value| value.as_bool())
        == Some(true)
        || json.get("bIsExecutable").and_then(|value| value.as_bool()) == Some(false)
    {
        return Ok(None);
    }
    let field = |primary: &str, fallback: &str| {
        json.get(primary)
            .or_else(|| json.get(fallback))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
    };
    let Some(title) = field("DisplayName", "title") else {
        return Ok(None);
    };
    let Some(location) = field("InstallLocation", "install_path") else {
        return Ok(None);
    };
    let install = PathBuf::from(location);
    if !install.is_absolute() {
        return Err("Chemin d’installation Epic non absolu".into());
    }
    if !install.is_dir() {
        return Ok(None);
    }
    let profile = if let Some(executable) = field("LaunchExecutable", "executable") {
        let command = super::relative_path(&install, executable)?;
        let arguments =
            super::split_arguments(field("LaunchCommand", "launch_parameters").unwrap_or(""))?;
        let windows_binary_on_linux = cfg!(target_os = "linux")
            && command
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
        if command.is_file() && !windows_binary_on_linux {
            Some(LaunchProfile {
                command,
                arguments,
                working_directory: Some(install.clone()),
                launcher_command: None,
            })
        } else {
            None
        }
    } else {
        None
    };
    Ok(Some(Game::from_installation(
        title,
        "Epic Games",
        Installation {
            path: install,
            launcher: Launcher::Epic,
            launch_profile: profile,
            available: true,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epic_profiles_preserve_arguments_and_require_an_existing_executable() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("Bin")).unwrap();
        fs::write(temp.path().join("Bin/run"), []).unwrap();
        let mut manifest = serde_json::json!({
            "DisplayName": "Été", "InstallLocation": temp.path(),
            "LaunchExecutable": "Bin\\run", "LaunchCommand": "--profile \"Mes jeux\" \"\""
        });
        let game = from_manifest(&manifest.to_string()).unwrap().unwrap();
        let profile = game.installations[0].launch_profile.as_ref().unwrap();
        assert_eq!(profile.command, temp.path().join("Bin/run"));
        assert_eq!(profile.arguments, ["--profile", "Mes jeux", ""]);
        assert_eq!(profile.working_directory.as_deref(), Some(temp.path()));
        manifest["LaunchExecutable"] = "missing.exe".into();
        assert!(from_manifest(&manifest.to_string())
            .unwrap()
            .unwrap()
            .installations[0]
            .launch_profile
            .is_none());
        manifest["LaunchExecutable"] = "../outside.exe".into();
        assert!(from_manifest(&manifest.to_string()).is_err());
    }

    #[test]
    fn epic_ignores_incomplete_installs_and_rejects_broken_commands() {
        let temp = tempfile::tempdir().unwrap();
        let mut manifest = serde_json::json!({"DisplayName":"Game", "InstallLocation":temp.path(), "bIsIncompleteInstall":true});
        assert!(from_manifest(&manifest.to_string()).unwrap().is_none());
        manifest["bIsIncompleteInstall"] = false.into();
        manifest["bIsExecutable"] = false.into();
        assert!(from_manifest(&manifest.to_string()).unwrap().is_none());
        manifest["bIsExecutable"] = true.into();
        manifest["LaunchExecutable"] = "run".into();
        manifest["LaunchCommand"] = "\"unterminated".into();
        assert!(from_manifest(&manifest.to_string()).is_err());
        assert!(from_manifest("not json").is_err());
        manifest["InstallLocation"] = "relative".into();
        assert!(from_manifest(&manifest.to_string()).is_err());
    }
}
