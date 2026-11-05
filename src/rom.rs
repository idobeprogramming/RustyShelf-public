use crate::{emulator, model::Game};
use std::path::Path;

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
    "GameCube / Wii",
    "Xbox",
    "Xbox 360",
    "Console à préciser",
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
        "gcm" => "GameCube",
        "wbfs" => "Wii",
        "cso" | "pbp" => "PSP",
        "gcz" | "rvz" => folder_console(path).unwrap_or("GameCube / Wii"),
        _ if emulator::is_disc_image(path) => folder_console(path).unwrap_or("Console à préciser"),
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

fn folder_console(path: &Path) -> Option<&'static str> {
    path.ancestors().skip(1).find_map(|folder| {
        let name = folder.file_name()?.to_string_lossy().to_ascii_lowercase();
        match name.as_str() {
            "ps1" | "psx" | "playstation" => Some("PlayStation"),
            "ps2" | "playstation 2" => Some("PlayStation 2"),
            "ps3" | "playstation 3" => Some("PlayStation 3"),
            "psp" => Some("PSP"),
            "gamecube" | "ngc" => Some("GameCube"),
            "wii" => Some("Wii"),
            "dreamcast" => Some("Dreamcast"),
            "saturn" => Some("Saturn"),
            "segacd" | "sega cd" | "mega cd" => Some("Mega-CD"),
            "megadrive" | "mega drive" | "genesis" => Some("Mega Drive"),
            "xbox" => Some("Xbox"),
            "xbox360" | "xbox 360" => Some("Xbox 360"),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_roms_without_guessing_ambiguous_discs() {
        assert_eq!(console(Path::new("/roms/zelda.NES")), Some("NES"));
        assert_eq!(
            console(Path::new("/roms/PS2/racing.iso")),
            Some("PlayStation 2")
        );
        assert_eq!(
            console(Path::new("/roms/racing.chd")),
            Some("Console à préciser")
        );
        assert_eq!(
            console(Path::new("/roms/GameCube/racing.rvz")),
            Some("GameCube")
        );
        assert_eq!(console(Path::new("/apps/readme.md")), None);
        assert_eq!(console(Path::new("/models/car.3ds")), None);
        assert_eq!(console(Path::new("/apps/tool.exe")), None);
        assert_eq!(console(Path::new("/apps/data.bin")), None);
    }
}
