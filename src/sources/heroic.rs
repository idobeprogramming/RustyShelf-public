use crate::model::{Game, Installation, LaunchProfile, Launcher};
use crate::scan::{ScanControl, ScanEvent};
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn discover(control: &ScanControl, report: &mut dyn FnMut(ScanEvent)) -> Vec<Game> {
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
        if control.is_cancelled() {
            break;
        }
        for entry in WalkDir::new(root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if control.is_cancelled() {
                break;
            }
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|s| s.to_str()) != Some("json")
            {
                continue;
            }
            let Some(text) = super::read_or_report(entry.path(), report) else {
                continue;
            };
            let json = match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(json) => json,
                Err(error) => {
                    report(ScanEvent::Error(entry.path().to_owned(), error.to_string()));
                    continue;
                }
            };
            let gog = entry
                .path()
                .to_string_lossy()
                .to_lowercase()
                .contains("gog");
            collect_games(&json, gog, None, &mut result);
        }
    }
    result
}

// A title and installation path must belong to the same record. Searching each
// field recursively can combine the title of one game with another game's path.
fn collect_games(
    value: &serde_json::Value,
    gog: bool,
    record_id: Option<&str>,
    result: &mut Vec<Game>,
) {
    let title = field(value, &["title", "app_title", "displayName", "name"]);
    let location = field(value, &["install_path", "installPath", "path"]);
    if let (Some(title), Some(location)) = (title, location) {
        let install = PathBuf::from(location);
        if !install.is_dir() {
            return;
        }
        let launcher = if gog { Launcher::Gog } else { Launcher::Heroic };
        let app_name = field(value, &["app_name", "appName", "app_id", "appId"]).or(record_id);
        let runner = if gog { "gog" } else { "legendary" };
        let launch_uri = app_name.map(|id| {
            format!(
                "heroic://launch?appName={}&runner={runner}",
                uri_component(id)
            )
        });
        let executable = field(value, &["executable", "launchExecutable", "exe"]);
        let profile = if executable.is_some() || launch_uri.is_some() {
            let command = executable.map(PathBuf::from).unwrap_or_default();
            let command = if command.as_os_str().is_empty() || command.is_absolute() {
                command
            } else {
                install.join(command)
            };
            Some(LaunchProfile {
                command,
                arguments: Vec::new(),
                working_directory: Some(install.clone()),
                launcher_command: launch_uri,
            })
        } else {
            None
        };
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
        return;
    }
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            collect_games(child, gog, Some(key), result);
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            collect_games(child, gog, None, result);
        }
    }
}

fn field<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_str())
            .filter(|text| !text.is_empty())
    })
}

fn uri_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn collects_every_installed_record_without_mixing_metadata() {
        let temp = tempdir().unwrap();
        let first = temp.path().join("one");
        let second = temp.path().join("two");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let json = serde_json::json!({
            "first&game": {"title":"First", "install_path":first},
            "second": {"title":"Second", "install_path":second, "executable":"run.exe"},
            "incomplete": {"title":"Not installed"}
        });
        let mut result = Vec::new();
        collect_games(&json, false, None, &mut result);
        assert_eq!(result.len(), 2);
        let game = result.iter().find(|game| game.title == "First").unwrap();
        assert_eq!(game.installations[0].path, first);
        assert_eq!(
            game.installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .launcher_command
                .as_deref(),
            Some("heroic://launch?appName=first%26game&runner=legendary")
        );
    }
}
