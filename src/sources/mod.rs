mod epic;
mod gog;
mod heroic;
mod lutris;
mod steam;

use crate::model::{Game, Installation, Launcher};
use crate::scan::{ScanControl, ScanEvent, ScanProgress};
use std::path::{Path, PathBuf};

const STEAM_SHARED_REDIST_ID: &str = "228980";

pub fn is_launcher_component(installation: &Installation) -> bool {
    match installation.launcher {
        Launcher::Gog => gog::installer_cache(&installation.path),
        Launcher::Steam => {
            installation
                .launch_profile
                .as_ref()
                .and_then(|profile| profile.launcher_command.as_deref())
                .and_then(|uri| uri.strip_prefix("steam://rungameid/"))
                == Some(STEAM_SHARED_REDIST_ID)
        }
        _ => false,
    }
}

pub fn is_shared_runtime_art(path: &Path) -> bool {
    path.ancestors().any(|entry| {
        entry
            .parent()
            .is_some_and(|parent| parent.ends_with("appcache/librarycache"))
            && entry
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| {
                    name == STEAM_SHARED_REDIST_ID
                        || name
                            .split_once('_')
                            .is_some_and(|(id, _)| id == STEAM_SHARED_REDIST_ID)
                })
    })
}

pub fn steam_cache_roots() -> Vec<PathBuf> {
    steam::default_steamapps()
        .iter()
        .filter_map(|path| path.parent().map(|root| root.join("appcache/librarycache")))
        .collect()
}

pub fn read_manifest_text(path: &Path) -> Result<String, String> {
    use std::io::Read;
    const MAX_BYTES: u64 = 4 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut text = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() as u64 > MAX_BYTES {
        return Err("Manifeste trop volumineux (limite de 4 Mo)".into());
    }
    Ok(text)
}

fn read_or_report(path: &Path, report: &mut dyn FnMut(ScanEvent)) -> Option<String> {
    match read_manifest_text(path) {
        Ok(text) => Some(text),
        Err(error) => {
            report(ScanEvent::Error(path.to_owned(), error));
            None
        }
    }
}

fn numeric_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().all(|byte| byte.is_ascii_digit())
        && id.parse::<u64>().is_ok_and(|id| id > 0)
}

fn relative_path(root: &Path, value: &str) -> Result<PathBuf, String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || normalized.contains(':')
        || path.is_absolute()
        || path.components().any(|part| {
            !matches!(
                part,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err("Chemin d’installation relatif invalide".into());
    }
    Ok(root.join(path))
}

pub fn is_manifest(path: &Path) -> bool {
    steam::is_manifest(path) || gog::is_manifest(path)
}

pub fn from_manifest(path: &Path) -> Result<Option<Game>, String> {
    if steam::is_manifest(path) {
        steam::read_manifest(path)
    } else if gog::is_manifest(path) {
        gog::read_manifest(path)
    } else {
        Ok(None)
    }
}

pub fn discover(
    roots: &[PathBuf],
    control: &ScanControl,
    report: &mut dyn FnMut(ScanEvent),
) -> Vec<Game> {
    let mut games = Vec::new();
    for source in ["Steam", "Epic Games", "Heroic", "Lutris", "GOG"] {
        if control.is_cancelled() {
            break;
        }
        report(ScanEvent::Progress(ScanProgress {
            visited: 0,
            current: source.into(),
        }));
        games.extend(match source {
            "Steam" => steam::discover(roots, control, report),
            "Epic Games" => epic::discover(control, report),
            "Heroic" => heroic::discover(control, report),
            "Lutris" => lutris::discover(control, report),
            _ => gog::discover(control, report),
        });
    }
    games.sort_by_key(|game| game.title.to_lowercase());
    // The store merges titles while preserving every installation and launcher profile.
    games
}

// Only user exclusions filter launcher records. Folders recognized by one
// launcher must not hide another launcher's metadata for the same installation.
pub fn within_scan_scope(
    mut games: Vec<Game>,
    roots: &[PathBuf],
    exclusions: &[PathBuf],
) -> Vec<Game> {
    for game in &mut games {
        game.installations.retain(|installation| {
            roots
                .iter()
                .any(|root| crate::scan::path_is_within(&installation.path, root))
                && !exclusions
                    .iter()
                    .any(|excluded| crate::scan::path_is_within(&installation.path, excluded))
        });
    }
    games.retain(|game| !game.installations.is_empty());
    games
}

// Split a Windows-style argument string without passing it to a shell.
fn split_arguments(text: &str) -> Result<Vec<String>, String> {
    let mut chars = text.chars().peekable();
    let mut result = Vec::new();
    let mut argument = String::new();
    let mut quoted = false;
    let mut started = false;
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() && !quoted {
            if started {
                result.push(std::mem::take(&mut argument));
                started = false;
            }
            continue;
        }
        started = true;
        match ch {
            '"' => quoted = !quoted,
            '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    argument.extend(std::iter::repeat_n('\\', count / 2));
                    chars.next();
                    if count % 2 == 0 {
                        quoted = !quoted;
                    } else {
                        argument.push('"');
                    }
                } else {
                    argument.extend(std::iter::repeat_n('\\', count));
                }
            }
            _ => argument.push(ch),
        }
    }
    if quoted {
        return Err("Guillemet non fermé dans les arguments de lancement".into());
    }
    if started {
        result.push(argument);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installation, Launcher};

    #[test]
    fn cancellation_stops_discovery_before_reading_any_user_library() {
        let control = ScanControl::default();
        control.cancel();
        assert!(discover(&[], &control, &mut |_| panic!(
            "Cancelled scan emitted an event"
        ))
        .is_empty());
        let control = ScanControl::default();
        let mut events = 0;
        let games = discover(&[], &control, &mut |_| {
            events += 1;
            control.cancel();
        });
        assert!(games.is_empty());
        assert_eq!(events, 1);
    }

    #[test]
    fn unreadable_and_oversized_manifests_are_reported_without_unbounded_reads() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.json");
        let mut errors = Vec::new();
        assert!(read_or_report(&path, &mut |event| {
            if let ScanEvent::Error(path, _) = event {
                errors.push(path);
            }
        })
        .is_none());
        assert_eq!(errors, [path]);
        let large = temp.path().join("large.json");
        std::fs::write(&large, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
        assert!(read_manifest_text(&large).unwrap_err().contains("4 Mo"));
    }

    #[test]
    fn shared_launcher_folders_do_not_hide_records_but_user_exclusions_do() {
        let root = PathBuf::from("/games");
        let make = |title: &str, path: &str, launcher| {
            Game::from_installation(
                title,
                "Games",
                Installation {
                    path: PathBuf::from(path),
                    launcher,
                    launch_profile: None,
                    available: true,
                },
            )
        };
        let mut shared = make("Shared", "/games/shared", Launcher::Heroic);
        shared.installations.push(Installation {
            path: "/other-drive/shared".into(),
            launcher: Launcher::Heroic,
            launch_profile: None,
            available: true,
        });
        let games = vec![
            make("Shared", "/games/shared", Launcher::Epic),
            shared,
            make("Excluded", "/games/private/game", Launcher::Steam),
            make("Outside", "/games-backup/game", Launcher::Steam),
        ];
        let scoped = within_scan_scope(games, &[root], &["/games/private".into()]);
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|game| game.installations.len() == 1));
        assert_eq!(scoped[1].installations[0].launcher, Launcher::Heroic);
    }
}
