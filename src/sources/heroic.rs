use crate::model::{Game, Installation, LaunchProfile, Launcher};
use crate::scan::{ScanControl, ScanEvent};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn discover(control: &ScanControl, report: &mut dyn FnMut(ScanEvent)) -> Vec<Game> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        for base in [
            home.join(".config/heroic"),
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
        ] {
            roots.extend(
                ["store", "store_cache", "legendary", "gog_store"].map(|folder| base.join(folder)),
            );
        }
    }
    #[cfg(windows)]
    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(app_data).join("heroic"));
    }
    discover_roots(&roots, control, report)
}

fn discover_roots(
    roots: &[PathBuf],
    control: &ScanControl,
    report: &mut dyn FnMut(ScanEvent),
) -> Vec<Game> {
    let mut result = Vec::new();
    for root in roots {
        if control.is_cancelled() {
            break;
        }
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
        {
            if control.is_cancelled() {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    report(ScanEvent::Error(
                        error.path().unwrap_or(root).to_owned(),
                        error.to_string(),
                    ));
                    continue;
                }
            };
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
            let gog = is_gog_metadata(entry.path(), root);
            collect_games(&json, gog, None, &mut result);
        }
    }
    result
}

fn is_gog_metadata(path: &Path, root: &Path) -> bool {
    root.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("gog_store"))
        || path.strip_prefix(root).is_ok_and(|relative| {
            relative
                .components()
                .any(|component| component.as_os_str().eq_ignore_ascii_case("gog_store"))
        })
        || path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("gog_")
        })
}

// A title and installation path must belong to the same record. Searching each
// field recursively can combine the title of one game with another game's path.
fn collect_games(
    value: &serde_json::Value,
    gog: bool,
    record_id: Option<&str>,
    result: &mut Vec<Game>,
) {
    if value.get("is_installed").and_then(|value| value.as_bool()) == Some(false) {
        return;
    }
    let gog = match field(value, &["runner"]) {
        Some("gog") => true,
        Some("legendary") => false,
        Some(_) => return, // Do not route unsupported stores through Legendary.
        None => gog,
    };
    let title = field(value, &["title", "app_title", "displayName", "name"]);
    let install_data = value.get("install").unwrap_or(value);
    let location = field(value, &["install_path", "installPath", "path"])
        .or_else(|| field(install_data, &["install_path", "installPath", "path"]));
    if let (Some(title), Some(location)) = (title, location) {
        let install = PathBuf::from(location);
        if !install.is_absolute() || !install.is_dir() {
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
        let executable = field(value, &["executable", "launchExecutable", "exe"])
            .or_else(|| field(install_data, &["executable", "launchExecutable", "exe"]));
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
    fn nested_install_records_keep_their_own_title_and_runner() {
        let temp = tempdir().unwrap();
        let json = serde_json::json!({"games":[
            {"title":"GOG Game", "app_name":"gog-id", "runner":"gog", "is_installed":true, "install":{"install_path":temp.path()}},
            {"title":"Epic Game", "app_name":"epic-id", "runner":"legendary", "install":{"install_path":temp.path()}},
            {"title":"Uninstalled", "is_installed":false, "install":{"install_path":temp.path()}},
            {"title":"Unrelated title"},
            {"install":{"install_path":temp.path()}},
            {"title":"Unsupported", "runner":"nile", "install":{"install_path":temp.path()}}
        ]});
        let mut games = Vec::new();
        collect_games(&json, false, None, &mut games);
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].installations[0].launcher, Launcher::Gog);
        assert_eq!(games[1].installations[0].launcher, Launcher::Heroic);
        assert_eq!(
            games[0].installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .launcher_command
                .as_deref(),
            Some("heroic://launch?appName=gog-id&runner=gog")
        );
        assert_eq!(
            games[1].installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .launcher_command
                .as_deref(),
            Some("heroic://launch?appName=epic-id&runner=legendary")
        );
    }

    #[test]
    fn metadata_discovery_does_not_guess_the_store_from_the_user_folder() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("gog-fan/heroic");
        fs::create_dir_all(root.join("legendary")).unwrap();
        fs::create_dir_all(root.join("gog_store")).unwrap();
        let record = serde_json::json!({"game-id":{"title":"Été", "install_path":temp.path()}});
        fs::write(root.join("legendary/installed.json"), record.to_string()).unwrap();
        fs::write(root.join("gog_store/installed.json"), record.to_string()).unwrap();
        let games = discover_roots(
            std::slice::from_ref(&root),
            &ScanControl::default(),
            &mut |event| panic!("Unexpected: {event:?}"),
        );
        assert_eq!(games.len(), 2);
        assert!(games
            .iter()
            .any(|game| game.installations[0].launcher == Launcher::Gog));
        assert!(games
            .iter()
            .any(|game| game.installations[0].launcher == Launcher::Heroic));
        assert!(is_gog_metadata(
            &root.join("store_cache/gog_library.json"),
            &root
        ));
        assert!(is_gog_metadata(
            &root.join("gog_store/installed.json"),
            &root.join("gog_store")
        ));
    }

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
