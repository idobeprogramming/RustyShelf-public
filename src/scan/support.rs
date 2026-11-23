use std::{ffi::OsStr, path::Path};

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

pub(super) fn cmake_test(folder: &Path, name: &str) -> bool {
    name.to_ascii_lowercase().ends_with("_tests")
        && folder.join("CMakeCache.txt").is_file()
        && folder.join("CTestTestfile.cmake").is_file()
}
