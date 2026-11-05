use crate::model::Emulator;
use std::path::{Path, PathBuf};

pub fn for_category<'a>(
    console: Option<&str>,
    associations: &std::collections::HashMap<String, String>,
    emulators: &'a [Emulator],
) -> Option<&'a Emulator> {
    let console = console.filter(|console| crate::rom::assignable_console(console))?;
    let id = associations.get(console)?;
    emulators.iter().find(|emulator| &emulator.id == id)
}

pub fn is_disc_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "iso" | "chd" | "cso" | "wbfs" | "gcz" | "cue" | "pbp" | "cdi" | "nrg" | "mdf"
    )
}

pub fn discover_installed() -> Vec<Emulator> {
    let mut result = Vec::new();
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(value) = std::env::var_os(variable) {
                roots.push(PathBuf::from(value));
            }
        }
        let candidates = [
            ("RetroArch", "RetroArch-Win64/retroarch.exe"),
            ("Dolphin", "Dolphin-x64/Dolphin.exe"),
            ("Dolphin", "Dolphin/Dolphin.exe"),
            ("PCSX2", "PCSX2/pcsx2-qt.exe"),
            (
                "DuckStation",
                "DuckStation/duckstation-qt-x64-ReleaseLTCG.exe",
            ),
            ("RPCS3", "RPCS3/rpcs3.exe"),
            ("PPSSPP", "PPSSPP/PPSSPPWindows64.exe"),
        ];
        for root in roots {
            for (name, relative) in candidates {
                let executable = root.join(relative);
                if executable.is_file() {
                    result.push(Emulator::new(name, executable));
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let candidates = [
            ("RetroArch", &["retroarch"][..]),
            ("Dolphin", &["dolphin-emu", "dolphin"][..]),
            ("PCSX2", &["pcsx2-qt", "pcsx2"][..]),
            ("DuckStation", &["duckstation-qt", "duckstation"][..]),
            ("RPCS3", &["rpcs3"][..]),
            ("PPSSPP", &["ppsspp-qt", "PPSSPPQt", "ppsspp"][..]),
        ];
        for (name, binaries) in candidates {
            if let Some(executable) = binaries.iter().find_map(|binary| find_on_path(binary)) {
                result.push(Emulator::new(name, executable));
            }
        }
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result.dedup_by(|a, b| a.executable == b.executable);
    result
}

#[cfg(target_os = "linux")]
fn find_on_path(binary: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(binary))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_inherits_an_emulator_from_another_category() {
        let emulators = [
            Emulator::new("PS2 runner", "/tools/ps2".into()),
            Emulator::new("GC runner", "/tools/gc".into()),
        ];
        let mut associations = std::collections::HashMap::new();
        associations.insert("PlayStation 2".into(), emulators[0].id.clone());
        associations.insert("GameCube".into(), emulators[1].id.clone());
        assert_eq!(
            for_category(Some("PlayStation 2"), &associations, &emulators),
            Some(&emulators[0])
        );
        assert_eq!(
            for_category(Some("GameCube"), &associations, &emulators),
            Some(&emulators[1])
        );
        assert!(for_category(Some("PSP"), &associations, &emulators).is_none());
        assert!(for_category(Some(crate::rom::UNCLASSIFIED), &associations, &emulators).is_none());
        assert!(for_category(None, &associations, &emulators).is_none());
    }
    #[test]
    fn recognizes_common_disc_image_formats_case_insensitively() {
        for path in ["game.ISO", "disc.chd", "game.wbfs", "disc.CSO", "game.gcz"] {
            assert!(is_disc_image(Path::new(path)), "{path}");
        }
        assert!(!is_disc_image(Path::new("game.exe")));
    }
}
