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
        .find_map(|installation| {
            // The ROM identity is assigned only after classification. Preserve
            // accepted ambiguous cartridges without rereading their header every frame
            // or relying on a category the user may have renamed.
            if game.id.starts_with("rom-") && !game.id.starts_with("rom-image-") {
                let extension = installation
                    .path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("");
                if extension.eq_ignore_ascii_case("md") {
                    return Some("Mega Drive");
                }
                if extension.eq_ignore_ascii_case("3ds") {
                    return Some("Nintendo 3DS");
                }
            }
            console(&installation.path)
        })
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
        "3ds" if has_ncsd_header(path) => "Nintendo 3DS",
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

fn has_ncsd_header(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    // NCSD has a 512-byte header, with its magic after the RSA signature.
    let mut header = [0u8; 0x200];
    file.read_exact(&mut header).is_ok() && &header[0x100..0x104] == b"NCSD"
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
    match checked_header(path) {
        Ok(Some(valid)) => return valid,
        Err(_) => return false,
        Ok(None) => {}
    }
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    match extension.to_ascii_lowercase().as_str() {
        "mdf" => {
            in_rom_folder(path)
                && (path.with_extension("mds").is_file() || path.with_extension("MDS").is_file())
        }
        // These formats lack the checks above: require an explicit ROM collection.
        "cdi" | "nrg" | "cue" => in_rom_folder(path),
        "iso" => in_rom_folder(path),
        _ => console(path).is_some(),
    }
}

/// Only reject an old entry when a local file was actually readable and its
/// known header is invalid. An offline or inaccessible file is inconclusive.
pub fn invalid_existing_header(path: &Path) -> bool {
    matches!(checked_header(path), Ok(Some(false)))
}

// Share one bounded check between discovery and legacy cleanup. None means an
// unchecked format; an IO error is inconclusive for a previously saved game.
fn checked_header(path: &Path) -> std::io::Result<Option<bool>> {
    use std::io::Read;
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let minimum = match extension.as_str() {
        "mdf" => return Ok(mdf_is_not_disc(path)?.then_some(false)),
        "nes" => 16,
        "n64" | "z64" | "v64" => 64,
        "3ds" | "cci" => 512,
        "gen" | "md" => 260,
        "cso" | "pbp" => 4,
        "chd" => 8,
        "smd" => 0x4200,
        "rvz" => 0x48,
        "gcz" => 32,
        "wbfs" => 512,
        _ => return Ok(None),
    };
    let file = std::fs::File::open(path)?;
    let mut header = Vec::new();
    file.take(minimum).read_to_end(&mut header)?;
    if extension == "smd" {
        // Accept raw dumps mislabeled .smd as well as interleaved 16 KiB blocks
        // after the 512-byte copier header. Only inspect the SEGA system ID.
        return Ok(Some(
            header.get(0x100..0x104) == Some(b"SEGA")
                || (header.len() == 0x4200
                    && [header[0x2280], header[0x280], header[0x2281], header[0x281]] == *b"SEGA"),
        ));
    }
    if header.len() < minimum as usize {
        return Ok(Some(false));
    }
    Ok(Some(match extension.as_str() {
        "nes" => header.starts_with(b"NES\x1a"),
        "n64" | "z64" | "v64" => matches!(
            header[..4],
            [0x80, 0x37, 0x12, 0x40] | [0x37, 0x80, 0x40, 0x12] | [0x40, 0x12, 0x37, 0x80]
        ),
        "3ds" | "cci" => &header[0x100..0x104] == b"NCSD",
        "gen" | "md" => &header[0x100..0x104] == b"SEGA",
        "cso" => header.starts_with(b"CISO"),
        "chd" => header.starts_with(b"MComprHD"),
        "pbp" => header.starts_with(b"\0PBP"),
        "rvz" => header.starts_with(b"RVZ\x01"),
        "gcz" => header.starts_with(&[0x01, 0xc0, 0x0b, 0xb1]),
        "wbfs" => header.starts_with(b"WBFS"),
        _ => unreachable!(),
    }))
}

fn mdf_is_not_disc(path: &Path) -> std::io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};

    const DISC_SECTOR: u64 = 2048;
    const BOOT_PAGE_OFFSET: u64 = 9 * 8192;
    const PAGE_ID_OFFSET: usize = 32;
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length < DISC_SECTOR {
        // An optical image must contain at least one sector.
        return Ok(true);
    }

    // SQL Server MDF shares the extension. Its primary file starts with a
    // file header page (type 15) and has a boot page (type 13) at page 9.
    // Require both typed headers and their page IDs to avoid guessing from
    // the filename or classifying an image with a missing MDS as invalid.
    let mut header = [0; PAGE_ID_OFFSET + 6];
    if length < BOOT_PAGE_OFFSET + header.len() as u64 {
        return Ok(false);
    }
    file.read_exact(&mut header)?;
    if !header.starts_with(&[1, 15]) || header[PAGE_ID_OFFSET..] != [0, 0, 0, 0, 1, 0] {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(BOOT_PAGE_OFFSET))?;
    file.read_exact(&mut header)?;
    Ok(header.starts_with(&[1, 13]) && header[PAGE_ID_OFFSET..] == [9, 0, 0, 0, 1, 0])
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
    fn smd_detection_uses_cartridge_content_instead_of_rom_folder_names() {
        let temp = tempfile::tempdir().unwrap();
        for folder in [
            temp.path().join("roms"),
            temp.path().join("XboxGames/Minecraft"),
        ] {
            std::fs::create_dir_all(&folder).unwrap();
            let path = folder.join("Étoiles.SMD");
            for bytes in [b"version 1\nnodes\nend\n".to_vec(), vec![0; 0x4200]] {
                std::fs::write(&path, bytes).unwrap();
                assert!(!scan_candidate(&path));
                assert!(invalid_existing_header(&path));
            }
            let mut raw = vec![0; 0x4000];
            raw[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
            let mut interleaved = vec![0; 512];
            interleaved.extend(raw.iter().skip(1).step_by(2));
            interleaved.extend(raw.iter().step_by(2));
            for bytes in [&raw, &interleaved] {
                std::fs::write(&path, bytes).unwrap();
                assert!(scan_candidate(&path));
                assert!(!invalid_existing_header(&path));
            }
            std::fs::write(&path, &interleaved[..0x4200 - 1]).unwrap();
            assert!(!scan_candidate(&path), "truncated first block");
            std::fs::remove_file(&path).unwrap();
            assert!(!scan_candidate(&path));
            assert!(
                !invalid_existing_header(&path),
                "offline file is inconclusive"
            );
            assert_eq!(console(&path), Some("Mega Drive"));
        }
    }

    #[test]
    fn legacy_header_check_only_rejects_readable_invalid_known_formats() {
        let temp = tempfile::tempdir().unwrap();
        for (ext, size, offset, signature) in [
            ("nes", 16, 0, b"NES\x1a".as_slice()),
            ("z64", 64, 0, &[0x80, 0x37, 0x12, 0x40]),
            ("v64", 64, 0, &[0x37, 0x80, 0x40, 0x12]),
            ("n64", 64, 0, &[0x40, 0x12, 0x37, 0x80]),
            ("3DS", 512, 256, b"NCSD"),
            ("gen", 260, 256, b"SEGA"),
            ("md", 260, 256, b"SEGA"),
            ("cso", 4, 0, b"CISO"),
            ("chd", 8, 0, b"MComprHD"),
            ("pbp", 4, 0, b"\0PBP"),
            ("RVZ", 0x48, 0, b"RVZ\x01"),
            ("gcz", 32, 0, &[0x01, 0xc0, 0x0b, 0xb1]),
            ("wbfs", 512, 0, b"WBFS"),
        ] {
            let path = temp.path().join(format!("Étoiles.{ext}"));
            assert!(!invalid_existing_header(&path), "missing {ext}");
            std::fs::write(&path, []).unwrap();
            assert!(invalid_existing_header(&path), "empty {ext}");
            let mut bytes = vec![0; size];
            std::fs::write(&path, &bytes).unwrap();
            assert!(invalid_existing_header(&path), "invalid {ext}");
            assert!(!scan_candidate(&path), "invalid discovery {ext}");
            bytes[offset..offset + signature.len()].copy_from_slice(signature);
            std::fs::write(&path, &bytes[..size - 1]).unwrap();
            assert!(!scan_candidate(&path), "truncated discovery {ext}");
            std::fs::write(&path, &bytes).unwrap();
            assert!(!invalid_existing_header(&path), "valid {ext}");
            assert!(scan_candidate(&path), "valid discovery {ext}");
        }
        let iso = temp.path().join("Unknown.iso");
        std::fs::write(&iso, []).unwrap();
        assert!(!invalid_existing_header(&iso));
    }

    #[test]
    fn nintendo_3ds_detection_uses_the_container_not_the_folder_name() {
        let temp = tempfile::tempdir().unwrap();
        let roms = temp.path().join("roms");
        std::fs::create_dir(&roms).unwrap();
        for folder in [temp.path(), roms.as_path()] {
            for extension in ["3DS", "cci"] {
                let path = folder.join(format!("Étoiles.{extension}"));
                assert!(!scan_candidate(&path));
                for bytes in [b"3D model".to_vec(), {
                    let mut truncated = vec![0; 0x104];
                    truncated[0x100..].copy_from_slice(b"NCSD");
                    truncated
                }] {
                    std::fs::write(&path, bytes).unwrap();
                    assert!(!scan_candidate(&path), "{}", path.display());
                }
                let mut header = [0u8; 0x200];
                header[0x100..0x104].copy_from_slice(b"NCSD");
                std::fs::write(&path, header).unwrap();
                assert!(scan_candidate(&path));
                assert_eq!(console(&path), Some("Nintendo 3DS"));
                let mut game = Game::from_installation(
                    "Étoiles",
                    "Ajout manuel",
                    crate::model::Installation {
                        path: path.clone(),
                        launcher: crate::model::Launcher::Manual,
                        launch_profile: None,
                        available: true,
                    },
                );
                game.category = "Aventures".into();
                std::fs::remove_file(&path).unwrap();
                assert_eq!(game_console(&game), Some("Nintendo 3DS"));
            }
        }
    }

    #[test]
    fn accepted_md_rom_keeps_its_console_with_custom_category_and_offline_disk() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Adventure.md");
        let mut bytes = [0u8; 0x104];
        bytes[0x100..].copy_from_slice(b"SEGA");
        std::fs::write(&path, bytes).unwrap();
        let mut game = Game::from_installation(
            "Adventure",
            "ROM · Mega Drive",
            crate::model::Installation {
                path: path.clone(),
                launcher: crate::model::Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        game.category = "Mes favoris rétro".into();
        assert_eq!(game_console(&game), Some("Mega Drive"));
        std::fs::remove_file(path).unwrap();
        assert_eq!(game_console(&game), Some("Mega Drive"));
        let document = Game::from_installation(
            "README",
            "Documents",
            crate::model::Installation {
                path: temp.path().join("README.md"),
                launcher: crate::model::Launcher::Manual,
                launch_profile: None,
                available: false,
            },
        );
        assert_eq!(game_console(&document), None);
    }

    #[test]
    fn cartridge_detection_checks_headers_without_trusting_extensions() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["NES", "n64", "z64", "v64"] {
            let path = temp.path().join(format!("Été.{extension}"));
            assert!(!scan_candidate(&path));
            for bytes in [b"".as_slice(), b"NES\x1a", b"ordinary application data"] {
                std::fs::write(&path, bytes).unwrap();
                assert!(!scan_candidate(&path));
            }
            if extension == "NES" {
                let mut header = [0; 16];
                header[..4].copy_from_slice(b"NES\x1a");
                std::fs::write(&path, header).unwrap();
                assert!(scan_candidate(&path));
            } else {
                for signature in [
                    [0x80, 0x37, 0x12, 0x40],
                    [0x37, 0x80, 0x40, 0x12],
                    [0x40, 0x12, 0x37, 0x80],
                ] {
                    let mut header = [0; 64];
                    header[..4].copy_from_slice(&signature);
                    std::fs::write(&path, header).unwrap();
                    assert!(scan_candidate(&path));
                }
            }
            // Offline classification must not forget an accepted game's console.
            std::fs::remove_file(&path).unwrap();
            assert!(console(&path).is_some());
        }
    }

    #[test]
    fn ambiguous_formats_require_rom_context_and_mdf_requires_a_descriptor() {
        let temp = tempfile::tempdir().unwrap();
        let roms = temp.path().join("roms");
        let apps = temp.path().join("Apps");
        std::fs::create_dir(&roms).unwrap();
        std::fs::create_dir(&apps).unwrap();
        for folder in [&roms, &apps] {
            std::fs::write(folder.join("Database.mdf"), [0; 2048]).unwrap();
            assert!(!scan_candidate(&folder.join("Database.mdf")));
            std::fs::write(folder.join("Database.mds"), b"descriptor").unwrap();
        }
        assert!(scan_candidate(&roms.join("Database.mdf")));
        assert!(!scan_candidate(&apps.join("Database.mdf")));
        assert!(!scan_candidate(&apps.join("generated.smd")));
        assert!(!scan_candidate(&roms.join("Adventure.smd")));
        assert!(!scan_candidate(&temp.path().join("Games/Setup.iso")));
        assert!(scan_candidate(&roms.join("Adventure.iso")));
    }

    #[test]
    fn mdf_cleanup_requires_database_headers_or_a_file_too_short_for_a_sector() {
        let temp = tempfile::tempdir().unwrap();
        let roms = temp.path().join("roms");
        std::fs::create_dir(&roms).unwrap();
        let path = roms.join("Été.mdf");
        assert!(!invalid_existing_header(&path));

        let mut bytes = vec![0; 10 * 8192];
        bytes[..2].copy_from_slice(&[1, 15]);
        bytes[32..38].copy_from_slice(&[0, 0, 0, 0, 1, 0]);
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            !invalid_existing_header(&path),
            "one matching page is insufficient"
        );
        assert!(
            !scan_candidate(&path),
            "an unfamiliar MDF still needs a companion"
        );
        std::fs::write(path.with_extension("MDS"), b"descriptor").unwrap();
        assert!(scan_candidate(&path));

        let boot_page = &mut bytes[9 * 8192..];
        boot_page[..2].copy_from_slice(&[1, 13]);
        boot_page[32..38].copy_from_slice(&[9, 0, 0, 0, 1, 0]);
        std::fs::write(&path, &bytes).unwrap();
        assert!(invalid_existing_header(&path));
        assert!(
            !scan_candidate(&path),
            "a companion cannot turn SQL data into a disc"
        );

        std::fs::write(&path, b"\r\n").unwrap();
        assert!(invalid_existing_header(&path));
        assert!(!scan_candidate(&path));
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
