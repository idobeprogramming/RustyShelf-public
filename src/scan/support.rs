use std::{ffi::OsStr, fs, path::Path};

fn named(name: Option<&OsStr>, choices: &[&str]) -> bool {
    name.is_some_and(|name| {
        choices
            .iter()
            .any(|choice| name.eq_ignore_ascii_case(choice))
    })
}

/// Returns the location that identifies a non-game installation or generated
/// build artifact. The caller may require this anchor to exist on disk.
pub(super) fn location(path: &Path) -> Option<&Path> {
    for folder in path.ancestors() {
        let Some(parent) = folder.parent() else {
            continue;
        };
        let name = folder.file_name();
        if named(
            parent.file_name(),
            &["Program Files", "Program Files (x86)"],
        ) && windows_application(path, folder)
        {
            return Some(parent);
        }
        if named(name, &["Chipset_Software", "RyzenMasterExtracted"])
            && named(parent.file_name(), &["AMD"])
            && parent
                .parent()
                .is_some_and(|root| root.has_root() && root.file_name().is_none())
        {
            return parent.parent();
        }
        if named(parent.file_name(), &["steamapps"])
            && named(name, &["temp", "downloading", "shadercache", "depotcache"])
            && parent.join("common").is_dir()
        {
            return Some(parent);
        }
        if named(name, &["Mods"])
            && named(parent.file_name(), &["BBLauncher"])
            && parent.join("LauncherSettings.toml").is_file()
            && folder.join("ActiveMods.txt").is_file()
        {
            return Some(parent);
        }
        if named(name, &["target", "dist", "runtime"])
            && parent.join("examples/rustyshelf.css").is_file()
            && parent.join("src/scan.rs").is_file()
        {
            return Some(parent);
        }
        if named(name, &["build", "deps", "incremental", ".fingerprint"]) {
            // Cargo inserts a target triple between `target` and the profile
            // for cross-compilation. A Cargo marker distinguishes these from
            // ordinary folders with names such as `target` and `build`.
            for target in parent.ancestors().skip(1).take(2) {
                if !named(target.file_name(), &["target"]) {
                    continue;
                }
                if target.join(".rustc_info.json").is_file() {
                    return Some(target);
                }
                if let Some(project) = target.parent().filter(|p| p.join("Cargo.toml").is_file()) {
                    return Some(project);
                }
            }
        }
        if named(name, &["CMakeFiles"]) && parent.join("CMakeCache.txt").is_file() {
            return Some(parent);
        }
        if named(name, &["toolchains"])
            && parent.join("settings.toml").is_file()
            && named(parent.file_name(), &["rustup", ".rustup"])
        {
            return Some(parent);
        }
        if named(name, &["bin"])
            && named(parent.file_name(), &[".cargo", ".cargo-home"])
            && parent.join("registry").is_dir()
        {
            return Some(parent);
        }
        if named(name, &["bin"])
            && named(parent.file_name(), &["jre", "jre64", "jdk", "jdk64"])
            && parent.join("release").is_file()
            && parent.join("lib").is_dir()
        {
            return Some(parent);
        }
    }
    None
}

fn windows_application(path: &Path, product: &Path) -> bool {
    if named(
        product.file_name(),
        &[
            "microsoft office",
            "microsoft office 15",
            "microsoft visual studio",
            "microsoft sdks",
            "windows kits",
            "amd",
            "gigabyte",
            "bravesoftware",
            "openssl-win64",
            "git",
            "libreoffice",
            "postgresql",
            "docker",
            "dotnet",
            "java",
            "microsoft sql server",
            "obs-studio",
            "mozilla firefox",
            "librewolf",
            "stirling pdf",
            "windows defender",
            "windows defender advanced threat protection",
            "windows media player",
            "wsl",
            "internet explorer",
            "microsoft gameinput",
            "windows mail",
            "iis express",
            "hyper-v",
            "razer",
            "razer chroma sdk",
            "realtek",
            "anticheatexpert",
            "nefarius software solutions",
            "cheat engine",
            "gamesave manager v3",
            "vortex",
            "keepassxc",
            "pgbouncer",
            "qbittorrent",
            "soulseekqt",
            "installshield installation information",
            "nodejs",
            "parallel-launcher",
        ],
    ) {
        return true;
    }
    let Ok(relative) = path.strip_prefix(product) else {
        return false;
    };
    let mut parts = relative.iter();
    let branch = parts.next();
    let child = parts.next();
    let name = product
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    match name.to_ascii_lowercase().as_str() {
        "microsoft" => named(
            branch,
            &["edge", "edgecore", "edgeupdate", "edgewebview", "copilot"],
        ),
        "electronic arts" => named(branch, &["ea desktop"]),
        "epic games" => named(branch, &["launcher", "epic online services"]),
        "steam" => named(branch, &["bin"]),
        "samsung" => named(branch, &["samsung magician"]),
        "common files" => {
            named(branch, &["steam", "java", "oracle"])
                || (named(branch, &["microsoft shared"])
                    && named(child, &["clicktorun", "ink", "msinfo"]))
        }
        _ => false,
    }
}

pub(super) fn auxiliary_program(folder: &Path, name: &str) -> bool {
    let binary = |dir: &Path, stem: &str| {
        dir.join(stem).is_file() || dir.join(format!("{stem}.exe")).is_file()
    };
    match name.to_ascii_lowercase().as_str() {
        "eden" | "eden-cli" | "eden-room" => ["eden", "eden-cli", "eden-room"]
            .iter()
            .all(|stem| binary(folder, stem)),
        "rpcs3" => {
            binary(folder, "rpcs3")
                && folder
                    .ancestors()
                    .take(2)
                    .any(|dir| dir.join("dev_hdd0").is_dir() && dir.join("dev_flash").is_dir())
        }
        "shadps4" => {
            (binary(folder, "shadPS4") || binary(folder, "shadps4"))
                && ((folder.join("user/config.toml").is_file()
                    && folder.join("user/game_data").is_dir())
                    || folder.ancestors().skip(1).take(2).any(|dir| {
                        binary(dir, "BB_Launcher")
                            && dir.join("BBLauncher/LauncherSettings.toml").is_file()
                    }))
        }
        other => {
            (other.ends_with("_tests")
                && folder.join("CMakeCache.txt").is_file()
                && folder.join("CTestTestfile.cmake").is_file())
                || python_console_tool(folder, other)
        }
    }
}

fn python_console_tool(scripts: &Path, command: &str) -> bool {
    // A console entry point can also be a game. Only check known utilities.
    let package = match command {
        "fonttools" | "pyftmerge" | "pyftsubset" | "ttx" => "fonttools",
        "f2py" | "numpy-config" => "numpy",
        "pymupdf" => "pymupdf",
        _ => return false,
    };
    let Some(prefix) = scripts.parent() else {
        return false;
    };
    let exe = format!("{command}.exe");
    let mut wrappers = [command, &exe].into_iter().filter_map(|name| {
        let metadata = scripts.join(name).symlink_metadata().ok()?;
        metadata.is_file().then_some((name, metadata.len()))
    });
    let Some((filename, size)) = wrappers.next() else {
        return false;
    };
    if wrappers.next().is_some() {
        return false;
    }

    let registered = |site: &Path, record_dirs: &[&str]| {
        fs::read_dir(site)
            .into_iter()
            .flatten()
            .take(512)
            .flatten()
            .any(|entry| {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                name.strip_prefix(package)
                    .is_some_and(|suffix| suffix.starts_with('-'))
                    && name.ends_with(".dist-info")
                    && entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && python_registration(
                        &entry.path(),
                        command,
                        package,
                        filename,
                        size,
                        record_dirs,
                    )
            })
    };
    if named(scripts.file_name(), &["bin"]) && named(prefix.file_name(), &[".packages"]) {
        return registered(prefix, &["../../bin", "bin"]);
    }
    if !prefix.join("pyvenv.cfg").is_file() {
        return false;
    }
    if named(scripts.file_name(), &["Scripts"]) {
        return registered(&prefix.join("Lib/site-packages"), &["../../Scripts"]);
    }
    named(scripts.file_name(), &["bin"])
        && fs::read_dir(prefix.join("lib"))
            .into_iter()
            .flatten()
            .take(24)
            .flatten()
            .any(|entry| {
                entry.file_name().to_string_lossy().starts_with("python3.")
                    && entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && registered(&entry.path().join("site-packages"), &["../../../bin"])
            })
}

fn python_registration(
    info: &Path,
    command: &str,
    package: &str,
    file: &str,
    size: u64,
    dirs: &[&str],
) -> bool {
    let read = |name, limit| {
        let path = info.join(name);
        let metadata = path.symlink_metadata().ok()?;
        if !metadata.is_file() || metadata.len() > limit {
            return None;
        }
        crate::sources::read_manifest_text(&path).ok()
    };
    let Some(entries) = read("entry_points.txt", 16 * 1024) else {
        return false;
    };
    let mut console = false;
    let matches_entry = entries.lines().map(str::trim).any(|line| {
        if line.starts_with('[') {
            console = line == "[console_scripts]";
        }
        console
            && line.split_once('=').is_some_and(|(name, target)| {
                name.trim() == command
                    && target
                        .trim()
                        .split_once('.')
                        .is_some_and(|(module, _)| module.eq_ignore_ascii_case(package))
            })
    });
    if !matches_entry {
        return false;
    }
    let Some(record) = read("RECORD", 256 * 1024) else {
        return false;
    };
    record.lines().any(|line| {
        let Some((path, attributes)) = line.split_once(',') else {
            return false;
        };
        let Some((dir, basename)) = path.rsplit_once('/') else {
            return false;
        };
        basename == file
            && dirs.contains(&dir)
            && attributes
                .split_once(',')
                .and_then(|(_, size)| size.parse().ok())
                == Some(size)
    })
}
