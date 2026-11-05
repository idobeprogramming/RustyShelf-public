use crate::{emulator, model::Game};
use std::path::Path;

pub const UNCLASSIFIED: &str = "Images à classer";

pub const CONSOLES: &[&str] = &[
    "NES",
    "Super Nintendo",
    "Game Boy",
    "Game Boy Color",
    "Game Boy Advance",
    "Nintendo DS",
    "Nintendo 3DS",
    "Nintendo 64",
    "Master System",
    "Mega Drive",
    "Game Gear",
    "Mega-CD",
    "Saturn",
    "Dreamcast",
    "PlayStation",
    "PlayStation 2",
    "PlayStation 3",
    "PSP",
    "GameCube",
    "Wii",
    "Xbox",
    "Xbox 360",
    UNCLASSIFIED,
];

pub fn game_console(game: &Game) -> Option<&'static str> {
    game.installations
        .iter()
        .find_map(|installation| console(&installation.path))
        .or_else(|| {
            game.category.strip_prefix("ROM · ").and_then(|category| {
                CONSOLES
                    .iter()
                    .copied()
                    .find(|console| *console == category)
            })
        })
}

pub fn console(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let console = match extension.as_str() {
        "nes" => "NES",
        "sfc" | "smc" => "Super Nintendo",
        "gb" => "Game Boy",
        "gbc" => "Game Boy Color",
        "gba" => "Game Boy Advance",
        "nds" => "Nintendo DS",
        "3ds"
            if path.ancestors().skip(1).any(|folder| {
                folder.file_name().is_some_and(|name| {
                    ["roms", "3ds", "nintendo 3ds"]
                        .contains(&name.to_string_lossy().to_ascii_lowercase().as_str())
                })
            }) =>
        {
            "Nintendo 3DS"
        }
        "cci" => "Nintendo 3DS",
        "n64" | "z64" | "v64" => "Nintendo 64",
        "gen" | "smd" => "Mega Drive",
        "md" if has_sega_header(path) => "Mega Drive",
        "sms" => "Master System",
        "gg" => "Game Gear",
        _ if is_image(path) => UNCLASSIFIED,
        _ => return None,
    };
    Some(console)
}

fn has_sega_header(path: &Path) -> bool {
    has_signature(path, 0x100, b"SEGA")
}

fn has_signature(path: &Path, offset: u64, expected: &[u8]) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut signature = vec![0; expected.len()];
    file.seek(SeekFrom::Start(offset)).is_ok()
        && file.read_exact(&mut signature).is_ok()
        && signature == expected
}

/// Discovery is stricter than classification of an already accepted/offline game.
/// In particular, .cso shaders, .gen generated files and .mdf databases are common.
pub fn scan_candidate(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    match extension.to_ascii_lowercase().as_str() {
        "gen" | "md" => has_sega_header(path),
        "cso" => has_signature(path, 0, b"CISO"),
        "chd" => has_signature(path, 0, b"MComprHD"),
        "pbp" => has_signature(path, 0, b"\0PBP"),
        "mdf" => {
            in_rom_folder(path)
                && (path.with_extension("mds").is_file() || path.with_extension("MDS").is_file())
        }
        // These formats lack the checks above: require an explicit ROM collection.
        "smd" | "cdi" | "nrg" | "cue" => in_rom_folder(path),
        "iso" => in_rom_folder(path),
        _ => console(path).is_some(),
    }
}

fn in_rom_folder(path: &Path) -> bool {
    path.ancestors().skip(1).any(|folder| {
        folder.file_name().is_some_and(|name| {
            let name = name.to_string_lossy().to_lowercase();
            [
                "rom",
                "roms",
                "iso",
                "isos",
                "emulation",
                "émulation",
                "ps1",
                "ps2",
                "ps3",
                "psp",
            ]
            .contains(&name.as_str())
                || CONSOLES
                    .iter()
                    .any(|console| console.to_lowercase() == name)
        })
    })
}

pub fn is_image(path: &Path) -> bool {
    emulator::is_disc_image(path)
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["rvz", "gcm"]
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            })
}

pub fn assignable_console(console: &str) -> bool {
    console != UNCLASSIFIED && CONSOLES.contains(&console)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_formats_require_rom_context_and_mdf_requires_a_descriptor() {
        let temp = tempfile::tempdir().unwrap();
        let roms = temp.path().join("roms");
        let apps = temp.path().join("Apps");
        std::fs::create_dir(&roms).unwrap();
        std::fs::create_dir(&apps).unwrap();
        for folder in [&roms, &apps] {
            std::fs::write(folder.join("Database.mdf"), b"data").unwrap();
            assert!(!scan_candidate(&folder.join("Database.mdf")));
            std::fs::write(folder.join("Database.mds"), b"descriptor").unwrap();
        }
        assert!(scan_candidate(&roms.join("Database.mdf")));
        assert!(!scan_candidate(&apps.join("Database.mdf")));
        assert!(!scan_candidate(&apps.join("generated.smd")));
        assert!(scan_candidate(&roms.join("Adventure.smd")));
        assert!(!scan_candidate(&temp.path().join("Games/Setup.iso")));
        assert!(scan_candidate(&roms.join("Adventure.iso")));
    }

    #[test]
    fn classifies_roms_without_guessing_ambiguous_discs() {
        assert_eq!(console(Path::new("/roms/zelda.NES")), Some("NES"));
        assert_eq!(
            console(Path::new("/roms/PS2/racing.iso")),
            Some(UNCLASSIFIED)
        );
        assert_eq!(console(Path::new("/roms/racing.chd")), Some(UNCLASSIFIED));
        assert_eq!(
            console(Path::new("/roms/GameCube/racing.rvz")),
            Some(UNCLASSIFIED)
        );
        assert_eq!(console(Path::new("/apps/readme.md")), None);
        assert_eq!(console(Path::new("/models/car.3ds")), None);
        assert_eq!(console(Path::new("/apps/tool.exe")), None);
        assert_eq!(console(Path::new("/apps/data.bin")), None);
    }
}
