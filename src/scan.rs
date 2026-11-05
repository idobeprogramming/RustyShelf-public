use crate::{
    emulator,
    model::{Game, Installation, LaunchProfile, Launcher},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use sysinfo::Disks;
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub root: PathBuf,
    pub name: String,
    pub filesystem: String,
    pub removable: bool,
}

pub fn mounted_volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .map(|disk| Volume {
            root: disk.mount_point().to_path_buf(),
            name: disk.name().to_string_lossy().trim().to_owned(),
            filesystem: disk.file_system().to_string_lossy().into_owned(),
            removable: disk.is_removable(),
        })
        .filter(|disk| disk.root.exists())
        .collect()
}

#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub visited: usize,
    pub current: PathBuf,
}

#[derive(Clone, Debug)]
pub enum ScanEvent {
    Candidate(Game),
    Error(PathBuf, String),
    Progress(ScanProgress),
    Finished { visited: usize, cancelled: bool },
}

#[derive(Clone, Default)]
pub struct ScanControl(Arc<AtomicBool>);
impl ScanControl {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub fn scan_roots(
    roots: &[PathBuf],
    exclusions: &[PathBuf],
    control: &ScanControl,
    mut emit: impl FnMut(ScanEvent),
) {
    let mut visited = 0;
    for root in roots {
        if control.is_cancelled() {
            break;
        }
        if !root.exists() {
            emit(ScanEvent::Error(
                root.clone(),
                "Disque ou dossier indisponible".into(),
            ));
            continue;
        }
        let walker = WalkDir::new(root)
            .follow_links(false)
            .max_open(16)
            .into_iter()
            .filter_entry(|entry| should_descend(entry, exclusions));
        for entry in walker {
            if control.is_cancelled() {
                break;
            }
            match entry {
                Ok(entry) => {
                    visited += 1;
                    if visited % 128 == 0 {
                        emit(ScanEvent::Progress(ScanProgress {
                            visited,
                            current: entry.path().to_path_buf(),
                        }));
                    }
                    if !entry.file_type().is_file() || !is_candidate_executable(entry.path()) {
                        if entry.file_type().is_file() && emulator::is_disc_image(entry.path()) {
                            if let Some(game) = candidate_from_disc_image(entry.path()) {
                                emit(ScanEvent::Candidate(game));
                            }
                        }
                        continue;
                    }
                    if let Some(game) = candidate_from_executable(entry.path()) {
                        emit(ScanEvent::Candidate(game));
                    }
                }
                Err(error) => {
                    let path = error.path().unwrap_or(root).to_path_buf();
                    emit(ScanEvent::Error(path, error.to_string()));
                }
            }
        }
    }
    emit(ScanEvent::Finished {
        visited,
        cancelled: control.is_cancelled(),
    });
}

fn should_descend(entry: &DirEntry, exclusions: &[PathBuf]) -> bool {
    let path = entry.path();
    if exclusions.iter().any(|excluded| path.starts_with(excluded)) {
        return false;
    }
    if !entry.file_type().is_dir() {
        return true;
    }
    #[cfg(target_os = "linux")]
    if [
        "/proc",
        "/sys",
        "/dev",
        "/run",
        "/tmp",
        "/usr/bin",
        "/usr/lib",
        "/bin",
        "/sbin",
        "/etc",
        "/var/lib",
        "/var/cache",
    ]
    .iter()
    .any(|system| path == Path::new(system) || path.starts_with(system))
    {
        return false;
    }
    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    !matches!(
        name.as_str(),
        "$recycle.bin"
            | "system volume information"
            | "windows"
            | "node_modules"
            | ".git"
            | "proc"
            | "sys"
            | "dev"
            | ".cache"
            | "target"
            | "temp"
    )
}

fn is_candidate_executable(path: &Path) -> bool {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    #[cfg(windows)]
    {
        extension.eq_ignore_ascii_case("exe")
    }
    #[cfg(not(windows))]
    {
        extension.eq_ignore_ascii_case("desktop")
            || extension.eq_ignore_ascii_case("sh")
            || std::fs::metadata(path).is_ok_and(|metadata| {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            })
    }
}

fn candidate_from_executable(path: &Path) -> Option<Game> {
    let is_desktop = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("desktop"));
    let desktop_text = is_desktop
        .then(|| std::fs::read_to_string(path).ok())
        .flatten();
    let title = desktop_text
        .as_deref()
        .and_then(|text| desktop_value(text, "Name"))
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })?;
    if !looks_like_game(&title) {
        return None;
    }
    let folder = path.parent()?.to_path_buf();
    let (command, arguments, working_directory) = if let Some(text) = desktop_text.as_deref() {
        let exec = desktop_value(text, "Exec")?;
        let mut pieces = desktop_exec_arguments(&exec)
            .into_iter()
            .filter(|part| !part.starts_with('%'));
        let command = PathBuf::from(pieces.next()?);
        let arguments = pieces.collect();
        let working_directory = desktop_value(text, "Path")
            .map(PathBuf::from)
            .or_else(|| command.parent().map(PathBuf::from))
            .or_else(|| Some(folder.clone()));
        (command, arguments, working_directory)
    } else {
        (path.to_path_buf(), Vec::new(), Some(folder.clone()))
    };
    let profile = Some(LaunchProfile {
        command,
        arguments,
        working_directory,
        launcher_command: None,
    });
    let installation = Installation {
        path: folder,
        launcher: Launcher::Unknown,
        launch_profile: profile,
        available: true,
    };
    Some(Game::from_installation(title, "À confirmer", installation))
}

fn desktop_exec_arguments(exec: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in exec.chars() {
        if escaped {
            argument.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            escaped = true;
        } else if Some(character) == quote {
            quote = None;
        } else if quote.is_none() && matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if quote.is_none() && character.is_whitespace() {
            if !argument.is_empty() {
                arguments.push(std::mem::take(&mut argument));
            }
        } else {
            argument.push(character);
        }
    }
    if escaped {
        argument.push('\\');
    }
    if !argument.is_empty() {
        arguments.push(argument);
    }
    arguments
}

fn desktop_value(contents: &str, key: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::to_owned))
        .filter(|value| !value.trim().is_empty())
}

fn candidate_from_disc_image(path: &Path) -> Option<Game> {
    let title = path.file_stem()?.to_string_lossy().trim().to_string();
    if !looks_like_game(&title) {
        return None;
    }
    Some(Game::from_installation(
        title,
        "Image disque · À confirmer",
        Installation {
            path: path.to_path_buf(),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        },
    ))
}

fn looks_like_game(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    if name.len() < 3
        || [
            "unins",
            "setup",
            "crash",
            "report",
            "unitycrash",
            "vc_redist",
            "directx",
            "7za",
            "7z",
            "unrealcefsubprocess",
            "easyanticheat",
            "launcher",
            "updater",
            "helper",
            "service",
            "redistributable",
        ]
        .iter()
        .any(|skip| lowered.contains(skip))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc};
    use tempfile::tempdir;

    #[test]
    fn scan_reports_candidates_and_respects_exclusions() {
        let temp = tempdir().unwrap();
        let game = temp.path().join("Games").join("SuperGame");
        let ignored = temp.path().join("ignored").join("HiddenGame");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&ignored).unwrap();
        fs::write(game.join("SuperGame.exe"), []).unwrap();
        fs::write(ignored.join("HiddenGame.exe"), []).unwrap();
        let (tx, rx) = mpsc::channel();
        scan_roots(
            &[temp.path().to_path_buf()],
            &[temp.path().join("ignored")],
            &ScanControl::default(),
            |event| {
                tx.send(event).unwrap();
            },
        );
        drop(tx);
        let titles: Vec<_> = rx
            .into_iter()
            .filter_map(|event| {
                if let ScanEvent::Candidate(game) = event {
                    Some(game.title)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(titles, ["SuperGame"]);
    }

    #[test]
    fn cancellation_is_reported() {
        let control = ScanControl::default();
        control.cancel();
        let mut cancelled = false;
        scan_roots(&[], &[], &control, |event| {
            if let ScanEvent::Finished {
                cancelled: value, ..
            } = event
            {
                cancelled = value;
            }
        });
        assert!(cancelled);
    }

    #[test]
    fn common_runtime_helpers_are_not_game_candidates() {
        assert!(!looks_like_game("7za"));
        assert!(!looks_like_game("setup_game"));
        assert!(looks_like_game("Moonlight Valley"));
    }

    #[test]
    fn disc_images_are_added_to_the_review_queue() {
        let game = candidate_from_disc_image(Path::new("/roms/Super Mario.iso")).unwrap();
        assert_eq!(game.title, "Super Mario");
        assert!(!game.confirmed);
        assert_eq!(
            game.installations[0].path,
            PathBuf::from("/roms/Super Mario.iso")
        );
    }

    #[test]
    fn desktop_shortcuts_use_their_display_name_and_launch_command() {
        let temp = tempdir().unwrap();
        let shortcut = temp.path().join("moonlight.desktop");
        fs::write(&shortcut, "[Desktop Entry]\nName=Moonlight Arcade\nExec=\"/games/Moonlight Arcade/run game\" --fullscreen %U\nPath=/games/Moonlight Arcade").unwrap();
        let game = candidate_from_executable(&shortcut).unwrap();
        assert_eq!(game.title, "Moonlight Arcade");
        let profile = game.installations[0].launch_profile.as_ref().unwrap();
        assert_eq!(
            profile.command,
            PathBuf::from("/games/Moonlight Arcade/run game")
        );
        assert_eq!(profile.arguments, ["--fullscreen"]);
        assert_eq!(
            profile.working_directory,
            Some(PathBuf::from("/games/Moonlight Arcade"))
        );
    }
}
