use crate::model::{Game, Installation, Launcher};
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
    if ["/proc", "/sys", "/dev", "/run", "/tmp"]
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
        extension.eq_ignore_ascii_case("desktop") || extension.eq_ignore_ascii_case("sh")
    }
}

fn candidate_from_executable(path: &Path) -> Option<Game> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    if !looks_like_game(&stem) {
        return None;
    }
    let folder = path.parent()?.to_path_buf();
    let installation = Installation {
        path: folder,
        launcher: Launcher::Unknown,
        launch_profile: None,
        available: true,
    };
    Some(Game::from_installation(stem, "À confirmer", installation))
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
}
