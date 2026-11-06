use crate::model::{Game, Installation, LaunchProfile, Launcher};
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

pub fn volume_selection(volumes: &[Volume], saved: Option<&[PathBuf]>) -> Vec<bool> {
    volumes
        .iter()
        .map(|volume| {
            saved.is_none_or(|saved| saved.iter().any(|root| same_root(root, &volume.root)))
        })
        .collect()
}

pub fn updated_volume_selection(
    volumes: &[Volume],
    selected: &[bool],
    saved: &[PathBuf],
) -> Vec<PathBuf> {
    // Retain choices for disconnected drives when editing the currently mounted ones.
    let mut roots: Vec<_> = saved
        .iter()
        .filter(|root| !volumes.iter().any(|volume| same_root(root, &volume.root)))
        .cloned()
        .collect();
    roots.extend(
        volumes
            .iter()
            .zip(selected)
            .filter(|(_, selected)| **selected)
            .map(|(volume, _)| volume.root.clone()),
    );
    roots
}

fn same_root(left: &Path, right: &Path) -> bool {
    path_is_within(left, right) && path_is_within(right, left)
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

pub fn path_is_within(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    {
        let mut components = path.components();
        root.components().all(|expected| {
            components.next().is_some_and(|actual| {
                actual
                    .as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
            })
        })
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

pub fn unique_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = Vec::new();
    for root in roots {
        if root.as_os_str().is_empty() || result.iter().any(|parent| path_is_within(root, parent)) {
            continue;
        }
        result.retain(|child| !path_is_within(child, root));
        result.push(root.clone());
    }
    result
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
    Skipped(PathBuf, String),
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
    for root in &unique_roots(roots) {
        if control.is_cancelled() {
            break;
        }
        if !root.exists() {
            emit(ScanEvent::Skipped(
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
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    if crate::sources::is_manifest(entry.path()) {
                        match crate::sources::from_manifest(entry.path()) {
                            Ok(Some(mut game)) => {
                                game.installations.retain(|installation| {
                                    roots
                                        .iter()
                                        .any(|root| path_is_within(&installation.path, root))
                                        && !exclusions.iter().any(|excluded| {
                                            path_is_within(&installation.path, excluded)
                                        })
                                });
                                if !game.installations.is_empty() {
                                    emit(ScanEvent::Candidate(game));
                                }
                            }
                            Ok(None) => {}
                            Err(message) => {
                                emit(ScanEvent::Error(entry.path().to_owned(), message))
                            }
                        }
                        continue;
                    }
                    if crate::rom::console(entry.path()).is_some() {
                        if let Some(game) = candidate_from_disc_image(entry.path()) {
                            emit(ScanEvent::Candidate(game));
                        }
                        continue;
                    }
                    if is_candidate_executable(entry.path()) {
                        if let Some(game) = candidate_from_executable(entry.path()) {
                            emit(ScanEvent::Candidate(game));
                        }
                    }
                }
                Err(error) => {
                    let path = error.path().unwrap_or(root).to_path_buf();
                    if error.io_error().is_some_and(|error| {
                        matches!(
                            error.kind(),
                            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
                        )
                    }) {
                        emit(ScanEvent::Skipped(
                            path,
                            "Dossier protégé ou devenu indisponible".into(),
                        ));
                    } else {
                        emit(ScanEvent::Error(path, error.to_string()));
                    }
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
    if exclusions
        .iter()
        .any(|excluded| path_is_within(path, excluded))
    {
        return false;
    }
    if !entry.file_type().is_dir() {
        return true;
    }
    #[cfg(target_os = "linux")]
    if linux_system_path(path) {
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

#[cfg(any(target_os = "linux", test))]
fn linux_system_path(path: &Path) -> bool {
    // Desktop environments commonly mount removable disks under /run/media.
    // Keep /run traversable to reach those disks, excluding its other contents.
    if path.starts_with("/run") && path != Path::new("/run") {
        return !path.starts_with("/run/media");
    }
    [
        "/proc",
        "/sys",
        "/dev",
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
    .any(|system| path.starts_with(system))
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
    let desktop_text = if is_desktop {
        // An unreadable shortcut must not fall back to launching the .desktop
        // file as if it were an executable, even beside recognizable game assets.
        Some(crate::sources::read_manifest_text(path).ok()?)
    } else {
        None
    };
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
    if let Some(text) = desktop_text.as_deref() {
        if desktop_value(text, "Hidden").as_deref() == Some("true")
            || !desktop_value(text, "Categories")
                .is_some_and(|categories| categories.split(';').any(|category| category == "Game"))
        {
            return None;
        }
    } else if !has_game_assets(path) {
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
            .or_else(|| {
                command
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .map(PathBuf::from)
            })
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
        path: if is_desktop {
            path.to_path_buf()
        } else {
            folder
        },
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
    let mut in_entry = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry {
            if let Some(value) = line.strip_prefix(&format!("{key}=")) {
                return (!value.trim().is_empty()).then(|| value.to_owned());
            }
        }
    }
    None
}

fn candidate_from_disc_image(path: &Path) -> Option<Game> {
    let title = path.file_stem()?.to_string_lossy().trim().to_string();
    if title.is_empty() || !crate::rom::scan_candidate(path) {
        return None;
    }
    Some(Game::from_installation(
        title,
        format!(
            "ROM · {}",
            crate::rom::console(path).unwrap_or(crate::rom::UNCLASSIFIED)
        ),
        Installation {
            path: path.to_path_buf(),
            launcher: Launcher::Unknown,
            launch_profile: None,
            available: true,
        },
    ))
}

fn has_game_assets(executable: &Path) -> bool {
    let Some(folder) = executable.parent() else {
        return false;
    };
    let stem = executable.file_stem().unwrap_or_default().to_string_lossy();
    folder.join(format!("{stem}_Data")).is_dir()
        || executable.with_extension("pck").is_file()
        || folder.join("data.win").is_file()
        || folder.join("renpy").is_dir()
        || folder.join("www/data/System.json").is_file()
        || folder
            .ancestors()
            .take(4)
            .any(|parent| parent.join("Content/Paks").is_dir())
}

pub(crate) fn looks_like_game(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    if name.trim().is_empty()
        || [
            "unins",
            "setup_",
            "crashhandler",
            "crashreporter",
            "unitycrash",
            "vc_redist",
            "directx",
            "unrealcefsubprocess",
            "easyanticheat",
            "redistributable",
        ]
        .iter()
        .any(|skip| lowered.contains(skip))
    {
        return false;
    }
    !matches!(
        lowered.as_str(),
        "setup"
            | "install"
            | "installer"
            | "7za"
            | "7z"
            | "7zfm"
            | "launcher"
            | "updater"
            | "helper"
            | "service"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc};
    use tempfile::tempdir;

    #[test]
    fn drive_choices_survive_disconnects_and_do_not_select_new_drives() {
        let volume = |root: &str| Volume {
            root: root.into(),
            name: String::new(),
            filesystem: String::new(),
            removable: true,
        };
        let mounted = vec![volume("/disk/a"), volume("/disk/b")];
        assert_eq!(volume_selection(&mounted, None), [true, true]);
        let saved = updated_volume_selection(&mounted, &[false, true], &[]);
        assert_eq!(saved, [PathBuf::from("/disk/b")]);
        let disconnected = vec![volume("/disk/a"), volume("/disk/c")];
        assert_eq!(
            volume_selection(&disconnected, Some(&saved)),
            [false, false]
        );
        let saved = updated_volume_selection(&disconnected, &[true, false], &saved);
        assert_eq!(volume_selection(&mounted, Some(&saved)), [true, true]);
        assert_eq!(volume_selection(&mounted, Some(&[])), [false, false]);
    }

    #[test]
    fn invalid_or_oversized_shortcuts_cannot_fall_back_to_direct_execution() {
        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join("game_Data")).unwrap();
        let shortcut = temp.path().join("game.desktop");
        fs::write(&shortcut, [0xff, 0xfe]).unwrap();
        assert!(candidate_from_executable(&shortcut).is_none());
        fs::write(&shortcut, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
        assert!(candidate_from_executable(&shortcut).is_none());
    }

    #[test]
    fn overlapping_roots_are_scanned_once_and_cancellation_stops_after_a_candidate() {
        let temp = tempdir().unwrap();
        let roms = temp.path().join("roms");
        fs::create_dir(&roms).unwrap();
        for index in 0..5 {
            fs::write(roms.join(format!("game{index}.nes")), []).unwrap();
        }
        let roots = [roms, temp.path().to_owned(), temp.path().to_owned()];
        assert_eq!(unique_roots(&roots), [temp.path().to_owned()]);
        for cancel in [false, true] {
            let control = ScanControl::default();
            let mut count = 0;
            let mut finished = false;
            scan_roots(&roots, &[], &control, |event| match event {
                ScanEvent::Candidate(_) => {
                    count += 1;
                    if cancel {
                        control.cancel();
                    }
                }
                ScanEvent::Finished { cancelled, .. } => {
                    finished = true;
                    assert_eq!(cancelled, cancel);
                }
                _ => {}
            });
            assert!(finished);
            assert_eq!(count, if cancel { 1 } else { 5 });
        }
    }

    #[test]
    fn finds_custom_steam_libraries_and_reports_invalid_manifests() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("Mes jeux/Archive/steamapps");
        fs::create_dir_all(library.join("common/Été")).unwrap();
        fs::write(
            library.join("appmanifest_42.acf"),
            r#""AppState" { "appid" "42" "name" "Été" "installdir" "Été" }"#,
        )
        .unwrap();
        fs::write(library.join("appmanifest_43.acf"), "broken").unwrap();
        let mut titles = Vec::new();
        let mut errors = Vec::new();
        scan_roots(
            &[temp.path().to_owned()],
            &[],
            &ScanControl::default(),
            |event| match event {
                ScanEvent::Candidate(game) => {
                    assert!(game.confirmed);
                    assert_eq!(game.installations[0].launcher, Launcher::Steam);
                    titles.push(game.title);
                }
                ScanEvent::Error(path, _) => errors.push(path),
                _ => {}
            },
        );
        assert_eq!(titles, ["Été"]);
        assert_eq!(errors, [library.join("appmanifest_43.acf")]);
        let mut count = 0;
        scan_roots(
            &[temp.path().to_owned()],
            &[library.join("common")],
            &ScanControl::default(),
            |event| {
                if matches!(event, ScanEvent::Candidate(_)) {
                    count += 1;
                }
            },
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn rejects_shader_database_and_generated_files_even_in_rom_folders() {
        let temp = tempdir().unwrap();
        let roms = temp.path().join("roms");
        fs::create_dir(&roms).unwrap();
        for (name, bytes) in [
            ("D3D9Overlay.cso", b"DXBC compiled shader".as_slice()),
            ("Database.mdf", b"database content".as_slice()),
            (
                "7792D9CE-355A-493C-AFBD.gen",
                b"generated content".as_slice(),
            ),
            ("broken.chd", b"not an image".as_slice()),
        ] {
            fs::write(roms.join(name), bytes).unwrap();
        }
        // Keep actual container signatures regardless of the title chosen by the user.
        fs::write(roms.join("Database.cso"), b"CISO\0\0\0\0").unwrap();
        let mut sega = vec![0u8; 0x200];
        sega[0x100..0x104].copy_from_slice(b"SEGA");
        fs::write(roms.join("Adventure.gen"), sega).unwrap();
        let mut titles = Vec::new();
        scan_roots(
            &[temp.path().to_owned()],
            &[],
            &ScanControl::default(),
            |event| {
                if let ScanEvent::Candidate(game) = event {
                    titles.push(game.title);
                }
            },
        );
        titles.sort();
        assert_eq!(titles, ["Adventure", "Database"]);
    }

    #[test]
    fn linux_removable_volumes_remain_scannable_without_entering_runtime_dirs() {
        for path in [
            "/run",
            "/run/media",
            "/run/media/alice/Jeux",
            "/media/Jeux",
            "/mnt/Jeux",
        ] {
            assert!(!linux_system_path(Path::new(path)), "{path}");
        }
        for path in [
            "/run/user/1000",
            "/run/udev",
            "/run/media-backup",
            "/proc/1",
            "/sys",
            "/dev",
            "/var/cache",
        ] {
            assert!(linux_system_path(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn desktop_actions_cannot_supply_a_missing_game_command_or_category() {
        let temp = tempdir().unwrap();
        let shortcut = temp.path().join("game.desktop");
        fs::write(&shortcut, "[Desktop Entry]\nName=Game\nCategories=Game;\n[Desktop Action Remove]\nExec=uninstall-game\n").unwrap();
        assert!(candidate_from_executable(&shortcut).is_none());
        fs::write(
            &shortcut,
            "[Desktop Entry]\nName=Editor\nExec=editor\n[Desktop Action Play]\nCategories=Game;\n",
        )
        .unwrap();
        assert!(candidate_from_executable(&shortcut).is_none());
    }

    #[test]
    fn unavailable_roots_are_reported_as_skipped_not_failed() {
        let temp = tempdir().unwrap();
        let mut skipped = 0;
        scan_roots(
            &[temp.path().join("unplugged")],
            &[],
            &ScanControl::default(),
            |event| match event {
                ScanEvent::Skipped(_, _) => skipped += 1,
                ScanEvent::Error(_, message) => panic!("{message}"),
                _ => {}
            },
        );
        assert_eq!(skipped, 1);
    }

    #[test]
    fn scans_console_roms_and_ignores_markdown_documents() {
        let temp = tempdir().unwrap();
        let roms = temp.path().join("roms/NES");
        fs::create_dir_all(&roms).unwrap();
        fs::write(roms.join("Étoiles.nes"), []).unwrap();
        fs::write(roms.join("README.md"), b"documentation").unwrap();
        let mut games = Vec::new();
        scan_roots(
            &[temp.path().to_owned()],
            &[],
            &ScanControl::default(),
            |event| {
                if let ScanEvent::Candidate(game) = event {
                    games.push(game);
                }
            },
        );
        assert_eq!(games.len(), 1);
        assert_eq!(crate::rom::game_console(&games[0]), Some("NES"));
        assert!(!games[0].confirmed);
    }

    #[test]
    fn exclusions_match_path_components_and_platform_case_rules() {
        assert!(path_is_within(
            Path::new("/games/celeste"),
            Path::new("/games")
        ));
        assert!(!path_is_within(
            Path::new("/games-backup/celeste"),
            Path::new("/games")
        ));
        #[cfg(windows)]
        assert!(path_is_within(
            Path::new("C:/Games/Celeste"),
            Path::new("c:/games")
        ));
        #[cfg(unix)]
        assert!(!path_is_within(
            Path::new("/Games/Celeste"),
            Path::new("/games")
        ));
    }

    #[test]
    fn ordinary_apps_and_non_game_desktop_entries_are_rejected() {
        let temp = tempdir().unwrap();
        let executable = temp.path().join("TextEditor.exe");
        fs::write(&executable, []).unwrap();
        assert!(candidate_from_executable(&executable).is_none());
        fs::create_dir(temp.path().join("TextEditor_Data")).unwrap();
        assert!(candidate_from_executable(&executable).is_some());
        let shortcut = temp.path().join("editor.desktop");
        fs::write(
            &shortcut,
            "[Desktop Entry]\nName=Editor\nCategories=Utility;\nExec=editor",
        )
        .unwrap();
        assert!(candidate_from_executable(&shortcut).is_none());
        assert!(candidate_from_disc_image(&temp.path().join("Windows.iso")).is_none());
    }

    #[test]
    fn scan_reports_candidates_and_respects_exclusions() {
        let temp = tempdir().unwrap();
        let game = temp.path().join("Games").join("SuperGame");
        let ignored = temp.path().join("ignored").join("HiddenGame");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&ignored).unwrap();
        fs::write(game.join("SuperGame.exe"), []).unwrap();
        fs::create_dir(game.join("SuperGame_Data")).unwrap();
        // Linux scans native executables and scripts, not Windows .exe files.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                game.join("SuperGame.exe"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
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
        assert!(looks_like_game("Crash Bandicoot"));
        assert!(looks_like_game("FE"));
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
        fs::write(&shortcut, "[Desktop Entry]\nName=Moonlight Arcade\nCategories=Game;\nExec=\"/games/Moonlight Arcade/run game\" --fullscreen %U\nPath=/games/Moonlight Arcade").unwrap();
        let game = candidate_from_executable(&shortcut).unwrap();
        assert_eq!(game.installations[0].path, shortcut);
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
