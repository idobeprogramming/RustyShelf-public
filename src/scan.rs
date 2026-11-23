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
    Finished {
        visited: usize,
        cancelled: bool,
        obsolete_ids: Vec<String>,
    },
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
    let mut recognized_installations = Vec::new();
    let windows_directory = if cfg!(windows) {
        std::env::var_os("SystemRoot").map(PathBuf::from)
    } else {
        None
    };
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
        let mut walker = WalkDir::new(root)
            .follow_links(false)
            .max_open(16)
            // Inspect local launcher metadata before sibling files or subfolders,
            // regardless of the filesystem's enumeration order.
            .sort_by_key(|entry| {
                !entry.file_type().is_file() || !crate::sources::is_manifest(entry.path())
            })
            .into_iter();
        // Check cancellation even between excluded entries. filter_entry can
        // otherwise consume many of them before returning control to this loop.
        while !control.is_cancelled() {
            let Some(entry) = walker.next() else {
                break;
            };
            match entry {
                Ok(entry) => {
                    if !should_descend(&entry, root, exclusions, windows_directory.as_deref()) {
                        if entry.file_type().is_dir() {
                            walker.skip_current_dir();
                        }
                        continue;
                    }
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
                                    recognized_installations.extend(
                                        game.installations.iter().map(|item| item.path.clone()),
                                    );
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
                    // Still inspect manifests in a shared folder (e.g. several
                    // GOG records), but don't turn its assets into new games.
                    if recognized_installations
                        .iter()
                        .any(|install| path_is_within(entry.path(), install))
                    {
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
        obsolete_ids: Vec::new(),
    });
}

fn should_descend(
    entry: &DirEntry,
    root: &Path,
    exclusions: &[PathBuf],
    windows_directory: Option<&Path>,
) -> bool {
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
    if windows_directory.is_some_and(|system| path_is_within(path, system)) {
        return false;
    }
    #[cfg(target_os = "linux")]
    if linux_path_excluded(path, root) {
        return false;
    }
    // A folder explicitly chosen by the user may happen to be named "target"
    // or "temp". User exclusions and protected system locations still apply.
    if path == root {
        return true;
    }
    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    !matches!(
        name.as_str(),
        "$recycle.bin"
            | "system volume information"
            | "node_modules"
            | ".git"
            | ".cache"
            | "target"
    )
}

#[cfg(any(target_os = "linux", test))]
fn linux_path_excluded(path: &Path, root: &Path) -> bool {
    // Broad drive scans skip system trees. A deliberately chosen subtree is
    // still scannable (including temporary/import folders and /var/lib games).
    linux_system_path(path) && !linux_system_path(root)
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
    if support_path(path) {
        return None;
    }
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
            .any(|project| unreal_assets_match(executable, folder, project))
}

fn unreal_assets_match(executable: &Path, folder: &Path, project: &Path) -> bool {
    if !project.join("Content/Paks").is_dir() {
        return false;
    }
    // The staged launcher can live at the project root. Keep it, even when its
    // filename differs from the engine binary.
    if folder == project {
        return true;
    }
    let Ok(relative) = folder.strip_prefix(project) else {
        return false;
    };
    let mut parts = relative.components();
    let Some(binaries) = parts.next() else {
        return false;
    };
    if !binaries.as_os_str().eq_ignore_ascii_case("Binaries") {
        return false;
    }
    let (Some(platform), Some(project_name), Some(stem)) = (
        parts.next(),
        project.file_name().and_then(|name| name.to_str()),
        executable.file_stem().and_then(|name| name.to_str()),
    ) else {
        return true;
    };
    // Keep custom launch scripts. Their names need not match the packaged
    // binary, and the scanner already requires separate evidence elsewhere.
    if executable
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sh"))
    {
        return true;
    }
    let platform_name = platform.as_os_str().to_string_lossy();
    let platform_folder = project.join(binaries).join(platform);
    let mut has_identified_binary = false;
    for name in [project_name.to_owned(), format!("{project_name}Client")] {
        // A packaged development build can be named simply after the project.
        let build_names = ["", "Debug", "DebugGame", "Development", "Test", "Shipping"];
        for configuration in build_names {
            let target = if configuration.is_empty() {
                name.clone()
            } else {
                format!("{name}-{platform_name}-{configuration}")
            };
            if stem.eq_ignore_ascii_case(&target) {
                return true;
            }
            has_identified_binary = has_identified_binary
                || platform_folder.join(&target).is_file()
                || platform_folder.join(format!("{target}.exe")).is_file();
        }
    }
    // Renamed or nonstandard packages can still be found using their assets
    // when there is no identifiable project binary.
    !has_identified_binary
}

fn support_path(path: &Path) -> bool {
    if known_application_path(path) {
        return true;
    }
    path.ancestors().take(6).any(|folder| {
        folder.file_name().is_some_and(|name| {
            matches!(
                name.to_string_lossy().to_ascii_lowercase().as_str(),
                "_commonredist"
                    | "_redist"
                    | "redist"
                    | "redistributables"
                    | "__installer"
                    | "runtimes"
            )
        })
    }) || path.ancestors().any(|folder| {
        folder
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("Binaries"))
            && folder
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name.eq_ignore_ascii_case("Engine"))
    })
}

fn known_application_path(path: &Path) -> bool {
    // Match an installed application directly under Program Files, not a game
    // title containing a vendor name or an arbitrary project folder.
    path.ancestors().any(|folder| {
        let Some(parent) = folder.parent().and_then(Path::file_name) else {
            return false;
        };
        if !parent.eq_ignore_ascii_case("Program Files")
            && !parent.eq_ignore_ascii_case("Program Files (x86)")
        {
            return false;
        }
        folder.file_name().is_some_and(|name| {
            matches!(
                name.to_string_lossy().to_ascii_lowercase().as_str(),
                "microsoft office"
                    | "microsoft visual studio"
                    | "microsoft sdks"
                    | "windows kits"
                    | "amd"
                    | "gigabyte"
                    | "bravesoftware"
                    | "openssl-win64"
                    | "git"
                    | "libreoffice"
                    | "postgresql"
                    | "docker"
                    | "dotnet"
                    | "java"
                    | "microsoft sql server"
                    | "obs-studio"
                    | "mozilla firefox"
                    | "librewolf"
                    | "stirling pdf"
                    | "windows defender"
                    | "windows defender advanced threat protection"
                    | "windows media player"
                    | "wsl"
                    | "internet explorer"
            )
        })
    })
}

pub fn obsolete_candidate(
    game: &Game,
    roots: &[PathBuf],
    exclusions: &[PathBuf],
    launcher_installations: &[PathBuf],
) -> bool {
    !game.confirmed
        && !game.favorite
        && !game.installations.is_empty()
        && game.installations.iter().all(|installation| {
            let path = &installation.path;
            if installation.launcher != Launcher::Unknown
                || !roots.iter().any(|root| path_is_within(path, root))
                || exclusions
                    .iter()
                    .any(|excluded| path_is_within(path, excluded))
            {
                return false;
            }
            let Ok(metadata) = path.metadata() else {
                return false;
            };
            if metadata.is_file() {
                if crate::rom::invalid_existing_header(path) {
                    return true;
                }
                if crate::rom::console(path).is_some() {
                    return false;
                }
            }
            // Older scans stored the folder of each executable as its own
            // installation. A strict child of a recognized game is redundant
            // unless its profile points outside that game.
            if metadata.is_dir()
                && !launcher_installations
                    .iter()
                    .any(|install| same_root(path, install))
                && path.parent().is_some_and(|parent| {
                    launcher_installations.iter().any(|install| {
                        path_is_within(parent, install)
                            && installation
                                .launch_profile
                                .as_ref()
                                .is_none_or(|profile| path_is_within(&profile.command, install))
                    })
                })
            {
                return true;
            }
            let name = installation
                .launch_profile
                .as_ref()
                .and_then(|profile| profile.command.file_stem())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| game.title.clone());
            !looks_like_game(&name)
                || support_path(path)
                || installation
                    .launch_profile
                    .as_ref()
                    .is_some_and(|profile| support_path(&profile.command))
        })
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
            "vcredist",
            "cefsharp.browsersubprocess",
            "crashpad_handler",
            "bugsplathd",
            "bssndrpt",
        ]
        .iter()
        .any(|skip| lowered.contains(skip))
    {
        return false;
    }
    if lowered.ends_with("installer") || lowered.ends_with("updater") {
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
            | "7zg"
            | "python"
            | "pythonw"
            | "unrealpak"
            | "shadercompileworker"
            | "dxsetup"
            | "oalinst"
            | "quicksfv"
            | "driver version checker"
            | "driverversionchecker"
            | "layerschecker"
            | "wowerror"
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
    fn local_manifests_take_priority_over_internal_game_candidates() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("Archive/steamapps");
        let steam = library.join("common/Été");
        let gog = temp.path().join("GOG Adventure");
        let neighbor = temp.path().join("GOG Adventure 2");
        for folder in [&steam, &gog, &neighbor] {
            fs::create_dir_all(folder.join("Bin")).unwrap();
            for name in ["A.nes", "Bin/Tool.nes"] {
                fs::write(folder.join(name), b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();
            }
        }
        fs::write(
            library.join("appmanifest_42.acf"),
            r#""AppState" { "appid" "42" "name" "Été" "installdir" "Été" }"#,
        )
        .unwrap();
        for (id, name) in [(1, "GOG Adventure"), (2, "GOG Bonus")] {
            fs::write(
                gog.join(format!("goggame-{id}.info")),
                format!(r#"{{"gameId":"{id}","name":"{name}"}}"#),
            )
            .unwrap();
        }
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
        assert_eq!(games.iter().filter(|game| game.confirmed).count(), 3);
        let candidates: Vec<_> = games.iter().filter(|game| !game.confirmed).collect();
        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|game| game.installations[0].path.starts_with(&neighbor)));
    }

    #[test]
    fn invalid_manifest_does_not_hide_independent_games_in_the_folder() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("goggame-42.info"), "broken").unwrap();
        fs::write(
            temp.path().join("Étoiles.nes"),
            b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0",
        )
        .unwrap();
        let mut candidates = 0;
        let mut errors = 0;
        scan_roots(
            &[temp.path().to_owned()],
            &[],
            &ScanControl::default(),
            |event| match event {
                ScanEvent::Candidate(_) => candidates += 1,
                ScanEvent::Error(_, _) => errors += 1,
                _ => {}
            },
        );
        assert_eq!((candidates, errors), (1, 1));
    }

    #[test]
    fn installed_productivity_and_driver_tools_are_not_game_candidates() {
        let temp = tempdir().unwrap();
        for relative in [
            "Program Files/Microsoft Office/root/Office16",
            "Program Files (x86)/Microsoft Visual Studio/18/BuildTools/Common7/IDE",
            "Program Files/Microsoft SDKs/NuGetPackages/compiler",
            "Program Files (x86)/Windows Kits/10/bin",
            "Program Files/AMD/CNext/CNext",
            "program files/GIGABYTE/Control Center/Lib",
            "Program Files (x86)/BraveSoftware/Update/1.0",
            "Program Files/OpenSSL-Win64/bin",
            "Program Files/Git/mingw64/bin",
            "Program Files/LibreOffice/program",
            "Program Files/PostgreSQL/17/bin",
            "Program Files/Docker/Docker/resources",
            "Program Files/dotnet/shared",
            "Program Files/Java/bin",
            "Program Files/Microsoft SQL Server/Tools",
            "Program Files/obs-studio/bin/64bit",
            "Program Files/Mozilla Firefox",
            "Program Files/LibreWolf",
            "Program Files/Stirling PDF",
            "Program Files/Windows Defender",
            "Program Files/Windows Defender Advanced Threat Protection",
            "Program Files (x86)/Windows Media Player",
            "Program Files/WSL",
            "Program Files (x86)/Internet Explorer",
        ] {
            let folder = temp.path().join(relative);
            fs::create_dir_all(&folder).unwrap();
            // Nearby engine files must not turn a known software component into a game.
            fs::write(folder.join("data.win"), []).unwrap();
            let executable = folder.join("Component.exe");
            fs::write(&executable, []).unwrap();
            assert!(
                candidate_from_executable(&executable).is_none(),
                "{relative}"
            );
            let game = Game::from_installation(
                "Component",
                "À confirmer",
                Installation {
                    path: folder,
                    launcher: Launcher::Unknown,
                    launch_profile: None,
                    available: true,
                },
            );
            assert!(
                obsolete_candidate(&game, &[temp.path().to_path_buf()], &[], &[]),
                "{relative}"
            );
        }
        for relative in [
            "Games/AMD",
            "Games/Microsoft Office",
            "Program Files/My Game",
            "Program Files/AMD Adventure",
            "Games/Git",
            "Program Files/Java Adventure",
        ] {
            let folder = temp.path().join(relative);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("data.win"), []).unwrap();
            assert!(
                candidate_from_executable(&folder.join("Adventure.exe")).is_some(),
                "{relative}"
            );
        }
    }

    #[test]
    fn old_candidates_are_rechecked_conservatively_within_scan_scope() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let obsolete = |game: &Game, roots: &[PathBuf], exclusions: &[PathBuf]| {
            obsolete_candidate(game, roots, exclusions, &[])
        };
        let mut game = Game::from_installation(
            "vcredist_x64",
            "À confirmer",
            Installation {
                path: root.clone(),
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        assert!(obsolete(&game, std::slice::from_ref(&root), &[]));
        assert!(!obsolete(&game, &[root.join("Other")], &[]));
        assert!(!obsolete(
            &game,
            std::slice::from_ref(&root),
            std::slice::from_ref(&root)
        ));
        game.confirmed = true;
        assert!(!obsolete(&game, std::slice::from_ref(&root), &[]));
        game.confirmed = false;
        game.favorite = true;
        assert!(!obsolete(&game, std::slice::from_ref(&root), &[]));
        game.favorite = false;
        game.installations[0].launcher = Launcher::Manual;
        assert!(!obsolete(&game, std::slice::from_ref(&root), &[]));
        game.installations[0].launcher = Launcher::Unknown;
        game.installations.push(Installation {
            path: root.join("unplugged"),
            ..game.installations[0].clone()
        });
        assert!(!obsolete(&game, std::slice::from_ref(&root), &[]));
        game.installations.pop();
        game.title = "Adventure".into();
        assert!(!obsolete(&game, std::slice::from_ref(&root), &[]));
        let path = root.join("Database.gen");
        fs::write(&path, b"not a ROM").unwrap();
        game.installations[0].path = path;
        assert!(obsolete(&game, std::slice::from_ref(&root), &[]));
        game.installations.push(Installation {
            path: root,
            ..game.installations[0].clone()
        });
        assert!(!obsolete(&game, &[temp.path().to_path_buf()], &[]));
    }

    #[test]
    fn engine_assets_do_not_turn_support_tools_into_games() {
        let temp = tempdir().unwrap();
        let game = temp.path().join("Adventure");
        fs::create_dir_all(game.join("Content/Paks")).unwrap();
        for relative in [
            "Binaries/Win64/Adventure-Win64-Shipping.exe",
            "Engine/Binaries/Win64/UnrealPak.exe",
            "Binaries/Win64/CefSharp.BrowserSubprocess.exe",
            "Binaries/Win64/BugSplatHD64.exe",
            "_CommonRedist/DirectX/DXSETUP.exe",
        ] {
            let path = game.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, []).unwrap();
            assert_eq!(
                candidate_from_executable(&path).is_some(),
                relative.contains("Adventure-Win64"),
                "{relative}"
            );
        }
        fs::write(game.join("data.win"), []).unwrap();
        for name in [
            "python",
            "pythonw",
            "crashpad_handler",
            "BsSndRpt64",
            "vcredist_x86",
            "BBEnhancedInstaller",
            "7zG",
            "oalinst",
        ] {
            let path = game.join(format!("{name}.exe"));
            fs::write(&path, []).unwrap();
            assert!(candidate_from_executable(&path).is_none(), "{name}");
        }
        assert!(candidate_from_executable(&game.join("Adventure.exe")).is_some());
    }

    #[test]
    fn unreal_internal_helpers_do_not_inherit_an_identified_game_binarys_assets() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("Étoiles");
        fs::create_dir_all(project.join("Content/Paks")).unwrap();
        for (platform, suffix) in [("Win64", ".exe"), ("Linux", "")] {
            let folder = project.join("Binaries").join(platform);
            fs::create_dir_all(folder.join("Tools")).unwrap();
            let shipping = folder.join(format!("Étoiles-{platform}-Shipping{suffix}"));
            let client = folder.join(format!("ÉtoilesClient-{platform}-Test{suffix}"));
            let tool = folder.join(format!("VideoConfig{suffix}"));
            let nested_tool = folder.join(format!("Tools/Calibrate{suffix}"));
            let standalone = folder.join(format!("Bonus{suffix}"));
            let wrapper = folder.join("play-with-overlay.sh");
            let staged = project.join(format!("Play Étoiles{suffix}"));
            for path in [
                &shipping,
                &client,
                &tool,
                &nested_tool,
                &standalone,
                &wrapper,
                &staged,
            ] {
                fs::write(path, []).unwrap();
            }
            fs::write(standalone.with_extension("pck"), []).unwrap();

            for path in [shipping, client, standalone, wrapper, staged] {
                assert!(
                    candidate_from_executable(&path).is_some(),
                    "{}",
                    path.display()
                );
            }
            for path in [tool, nested_tool] {
                assert!(
                    candidate_from_executable(&path).is_none(),
                    "{}",
                    path.display()
                );
            }
        }

        let renamed = temp.path().join("Archive restaurée");
        fs::create_dir_all(renamed.join("Content/Paks")).unwrap();
        let folder = renamed.join("Binaries/Win64");
        fs::create_dir_all(&folder).unwrap();
        let executable = folder.join("OriginalProject-Win64-Shipping.exe");
        fs::write(&executable, []).unwrap();
        assert!(candidate_from_executable(&executable).is_some());
    }

    #[test]
    fn game_folders_named_like_system_directories_are_not_lost() {
        let temp = tempdir().unwrap();
        for name in ["Windows", "dev", "sys", "proc", "temp", "node_modules"] {
            let folder = temp.path().join(name);
            fs::create_dir(&folder).unwrap();
            fs::write(
                folder.join(format!("{name}.nes")),
                b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0",
            )
            .unwrap();
        }
        let collect = |root: PathBuf, exclusions: &[PathBuf]| {
            let mut titles = Vec::new();
            scan_roots(&[root], exclusions, &ScanControl::default(), |event| {
                if let ScanEvent::Candidate(game) = event {
                    titles.push(game.title);
                }
            });
            titles.sort();
            titles
        };
        assert_eq!(
            collect(temp.path().to_owned(), &[]),
            ["Windows", "dev", "proc", "sys", "temp"]
        );
        let chosen = temp.path().join("node_modules");
        assert_eq!(collect(chosen.clone(), &[]), ["node_modules"]);
        assert!(collect(chosen.clone(), &[chosen]).is_empty());
    }

    #[test]
    fn real_windows_directory_is_excluded_without_hiding_other_windows_folders() {
        let temp = tempdir().unwrap();
        let system = temp.path().join("OperatingSystem");
        let game = temp.path().join("Games/Windows");
        fs::create_dir_all(&system).unwrap();
        fs::create_dir_all(&game).unwrap();
        for (folder, expected) in [(system.clone(), false), (game, true)] {
            let entry = WalkDir::new(&folder).into_iter().next().unwrap().unwrap();
            assert_eq!(
                should_descend(&entry, temp.path(), &[], Some(&system)),
                expected
            );
        }
    }

    #[test]
    fn explicit_linux_subtree_can_be_scanned_but_whole_drive_skips_system_paths() {
        for path in [
            "/tmp/import/roms",
            "/var/lib/games",
            "/run/user/1000/import",
        ] {
            assert!(linux_path_excluded(Path::new(path), Path::new("/")));
            assert!(!linux_path_excluded(Path::new(path), Path::new(path)));
        }
        assert!(!linux_path_excluded(
            Path::new("/run/media/user/Games/Windows"),
            Path::new("/")
        ));
    }

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
            fs::write(
                roms.join(format!("game{index}.nes")),
                b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0",
            )
            .unwrap();
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
    fn finds_3ds_images_outside_rom_folders_and_rejects_models_inside_them() {
        let temp = tempdir().unwrap();
        let roms = temp.path().join("roms");
        fs::create_dir(&roms).unwrap();
        fs::write(roms.join("Scene.3ds"), b"3D geometry").unwrap();
        fs::write(roms.join("Data.cci"), b"ordinary data").unwrap();
        let path = temp.path().join("Étoiles.3ds");
        let mut header = [0u8; 0x200];
        header[0x100..0x104].copy_from_slice(b"NCSD");
        fs::write(&path, header).unwrap();
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
        assert_eq!(games[0].installations[0].path, path);
        assert_eq!(crate::rom::game_console(&games[0]), Some("Nintendo 3DS"));
        assert!(!games[0].confirmed);
        assert!(games[0].installations[0].launch_profile.is_none());
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
        fs::write(roms.join("Étoiles.nes"), b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();
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
