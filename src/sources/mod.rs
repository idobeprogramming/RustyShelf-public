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
