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
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut signature = [0; 4];
    file.seek(SeekFrom::Start(0x100)).is_ok()
        && file.read_exact(&mut signature).is_ok()
        && &signature == b"SEGA"
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
