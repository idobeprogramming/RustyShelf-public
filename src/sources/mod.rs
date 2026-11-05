mod epic;
mod gog;
mod heroic;
mod lutris;
mod steam;

use crate::model::Game;
use std::path::PathBuf;

pub fn discover(roots: &[PathBuf]) -> Vec<Game> {
    let mut games = Vec::new();
    games.extend(steam::discover(roots));
    games.extend(epic::discover());
    games.extend(heroic::discover());
    games.extend(lutris::discover());
    games.extend(gog::discover());
    games.sort_by_key(|game| game.title.to_lowercase());
    // The store merges titles while preserving every installation and launcher profile.
    games
}

// Only user exclusions filter launcher records. Folders recognized by one
// launcher must not hide another launcher's metadata for the same installation.
pub fn within_scan_scope(
    mut games: Vec<Game>,
    roots: &[PathBuf],
    exclusions: &[PathBuf],
) -> Vec<Game> {
    for game in &mut games {
        game.installations.retain(|installation| {
            roots
                .iter()
                .any(|root| crate::scan::path_is_within(&installation.path, root))
                && !exclusions
                    .iter()
                    .any(|excluded| crate::scan::path_is_within(&installation.path, excluded))
        });
    }
    games.retain(|game| !game.installations.is_empty());
    games
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installation, Launcher};

    #[test]
    fn shared_launcher_folders_do_not_hide_records_but_user_exclusions_do() {
        let root = PathBuf::from("/games");
        let make = |title: &str, path: &str, launcher| {
            Game::from_installation(
                title,
                "Games",
                Installation {
                    path: PathBuf::from(path),
                    launcher,
                    launch_profile: None,
                    available: true,
                },
            )
        };
        let mut shared = make("Shared", "/games/shared", Launcher::Heroic);
        shared.installations.push(Installation {
            path: "/other-drive/shared".into(),
            launcher: Launcher::Heroic,
            launch_profile: None,
            available: true,
        });
        let games = vec![
            make("Shared", "/games/shared", Launcher::Epic),
            shared,
            make("Excluded", "/games/private/game", Launcher::Steam),
            make("Outside", "/games-backup/game", Launcher::Steam),
        ];
        let scoped = within_scan_scope(games, &[root], &["/games/private".into()]);
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|game| game.installations.len() == 1));
        assert_eq!(scoped[1].installations[0].launcher, Launcher::Heroic);
    }
}
