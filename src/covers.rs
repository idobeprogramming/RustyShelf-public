use crate::{
    model::{Game, Launcher},
    scan::ScanControl,
};
use std::path::{Path, PathBuf};

const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "ico"];
const STEAM_IMAGES: &[&str] = &[
    "library_header",
    "header",
    "library_600x900",
    "library_capsule",
    "library_hero",
];

/// Called during discovery/import, never by the frame rendering loop.
/// Known names only: arbitrary artwork or files from other Steam IDs are ignored.
pub fn find(game: &Game, steam_caches: &[PathBuf], control: &ScanControl) -> Option<PathBuf> {
    if game.cover_path.is_some() || control.is_cancelled() {
        return game.cover_path.clone();
    }
    for installation in &game.installations {
        if control.is_cancelled() {
            return None;
        }
        if installation.launcher == Launcher::Steam {
            let id = installation
                .launch_profile
                .as_ref()
                .and_then(|profile| profile.launcher_command.as_deref())
                .and_then(|uri| uri.strip_prefix("steam://rungameid/"))
                .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
            if let Some(id) = id {
                let mut roots = steam_caches.to_vec();
                if let Some(root) = installation
                    .path
                    .ancestors()
                    .find(|path| {
                        path.file_name()
                            .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
                    })
                    .and_then(Path::parent)
                {
                    roots.push(root.join("appcache/librarycache"));
                }
                roots.sort();
                roots.dedup();
                for root in roots {
                    if let Some(path) = steam_cover(&root, id, control) {
                        return Some(path);
                    }
                    if control.is_cancelled() {
                        return None;
                    }
                }
            }
        }
        let path = &installation.path;
        let candidates = if path.is_dir() {
            ["cover", "poster", "boxart", "header"]
                .iter()
                .flat_map(|name| {
                    EXTENSIONS
                        .iter()
                        .map(move |ext| path.join(format!("{name}.{ext}")))
                })
                .collect::<Vec<_>>()
        } else {
            // ROM collections share a folder: never give every game its generic cover.png.
            let Some(parent) = path.parent() else {
                continue;
            };
            let Some(stem) = path.file_stem() else {
                continue;
            };
            [
                parent.to_path_buf(),
                parent.join("covers"),
                parent.join("images"),
                parent.join("boxart"),
            ]
            .iter()
            .flat_map(|folder| {
                EXTENSIONS.iter().map(move |ext| {
                    let mut name = stem.to_os_string();
                    name.push(format!(".{ext}"));
                    folder.join(name)
                })
            })
            .collect()
        };
        for path in candidates {
            if control.is_cancelled() {
                return None;
            }
            if usable_image(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn steam_cover(root: &Path, id: &str, control: &ScanControl) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    // Older clients store <appid>_<kind>.jpg directly in librarycache.
    for (rank, name) in STEAM_IMAGES.iter().enumerate() {
        for extension in EXTENSIONS {
            candidates.push((rank, root.join(format!("{id}_{name}.{extension}"))));
        }
    }
    // Current clients also use <appid>/<hash>/<kind>.jpg. Do not traverse the
    // entire cache or follow directory links, even if a cache is malformed.
    for entry in walkdir::WalkDir::new(root.join(id))
        .follow_links(false)
        .max_depth(2)
        .into_iter()
        .take(256)
    {
        if control.is_cancelled() {
            return None;
        }
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let name = stem.to_string_lossy().to_ascii_lowercase();
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if EXTENSIONS
            .iter()
            .any(|ext| extension.eq_ignore_ascii_case(ext))
        {
            if let Some(rank) = STEAM_IMAGES.iter().position(|known| *known == name) {
                candidates.push((rank, path.to_path_buf()));
            }
        }
    }
    candidates.sort();
    for (_, path) in candidates {
        if control.is_cancelled() {
            return None;
        }
        if usable_image(&path) {
            return Some(path);
        }
    }
    None
}

fn usable_image(path: &Path) -> bool {
    let Ok(metadata) = path.symlink_metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        return false;
    }
    image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .is_some_and(|(width, height)| width > 0 && height > 0 && width <= 4096 && height <= 4096)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installation, LaunchProfile};

    fn png(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::new(12, 8)
            .save_with_format(path, image::ImageFormat::Png)
            .unwrap();
    }

    fn game(path: PathBuf) -> Game {
        Game::from_installation(
            "Étoiles",
            "Test",
            Installation {
                path,
                launcher: Launcher::Manual,
                launch_profile: None,
                available: true,
            },
        )
    }

    #[test]
    fn steam_artwork_uses_exact_id_in_legacy_and_nested_caches() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Steam/appcache/librarycache");
        let mut game = game(
            temp.path()
                .join("Other drive/SteamLibrary/steamapps/common/Étoiles"),
        );
        game.installations[0].launcher = Launcher::Steam;
        game.installations[0].launch_profile = Some(LaunchProfile {
            command: PathBuf::new(),
            arguments: vec![],
            working_directory: None,
            launcher_command: Some("steam://rungameid/123".into()),
        });
        png(&root.join("1234_header.jpg"));
        png(&root.join("456/hash/library_header.jpg"));
        let control = ScanControl::default();
        assert_eq!(find(&game, std::slice::from_ref(&root), &control), None);
        let portrait = root.join("123/hash/library_600x900.jpg");
        png(&portrait);
        assert_eq!(
            find(&game, std::slice::from_ref(&root), &control),
            Some(portrait)
        );
        let header = root.join("123_header.jpg");
        png(&header);
        assert_eq!(
            find(&game, std::slice::from_ref(&root), &control),
            Some(header.clone())
        );
        std::fs::write(&header, b"broken image").unwrap();
        let modern = root.join("123/otherhash/LIBRARY_HEADER.JPG");
        png(&modern);
        assert_eq!(
            find(&game, std::slice::from_ref(&root), &control),
            Some(modern)
        );
        game.installations[0]
            .launch_profile
            .as_mut()
            .unwrap()
            .launcher_command = Some("steam://rungameid/../456".into());
        assert_eq!(find(&game, &[root], &control), None);
        game.installations[0]
            .launch_profile
            .as_mut()
            .unwrap()
            .launcher_command = Some("steam://rungameid/123".into());
        let own_cache = temp
            .path()
            .join("Other drive/SteamLibrary/appcache/librarycache/123/hash/library_header.jpg");
        png(&own_cache);
        assert_eq!(find(&game, &[], &control), Some(own_cache));
    }

    #[test]
    fn local_rom_artwork_is_specific_to_the_file_and_preserves_manual_choices() {
        let temp = tempfile::tempdir().unwrap();
        let mut game = game(temp.path().join("Étoiles.nes"));
        png(&temp.path().join("cover.png"));
        png(&temp.path().join("Another.png"));
        let control = ScanControl::default();
        assert_eq!(find(&game, &[], &control), None);
        let cover = temp.path().join("covers/Étoiles.png");
        png(&cover);
        assert_eq!(find(&game, &[], &control), Some(cover));
        let chosen = temp.path().join("unplugged/manual.png");
        game.cover_path = Some(chosen.clone());
        assert_eq!(find(&game, &[], &control), Some(chosen));
        game.cover_path = None;
        control.cancel();
        assert_eq!(find(&game, &[], &control), None);
    }

    #[test]
    fn local_game_artwork_skips_broken_and_oversized_images() {
        let temp = tempfile::tempdir().unwrap();
        let game = game(temp.path().to_path_buf());
        std::fs::write(temp.path().join("cover.png"), b"invalid").unwrap();
        image::RgbaImage::new(4097, 1)
            .save(temp.path().join("poster.png"))
            .unwrap();
        let file = std::fs::File::create(temp.path().join("cover.jpg")).unwrap();
        file.set_len(33 * 1024 * 1024).unwrap();
        let cover = temp.path().join("boxart.png");
        png(&cover);
        assert_eq!(find(&game, &[], &ScanControl::default()), Some(cover));
    }
}
