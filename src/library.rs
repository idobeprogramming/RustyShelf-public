use crate::model::{Game, Launcher};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameKind {
    #[default]
    All,
    Pc,
    Rom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Name,
    ReverseName,
    Favorites,
}

#[derive(Clone, Debug, Default)]
pub struct Filters {
    pub kind: GameKind,
    pub source: Option<Launcher>,
    pub available: Option<bool>,
    pub favorites_only: bool,
    pub sort: Sort,
}

impl Filters {
    pub fn matches(&self, game: &Game, query: &str, console: Option<&str>) -> bool {
        let available = game
            .installations
            .iter()
            .any(|installation| installation.available);
        let query = query.trim().to_lowercase();
        let searchable =
            format!("{} {} {}", game.title, game.category, console.unwrap_or("")).to_lowercase();
        searchable.contains(&query)
            && match self.kind {
                GameKind::All => true,
                GameKind::Pc => console.is_none(),
                GameKind::Rom => console.is_some(),
            }
            && (!self.favorites_only || game.favorite)
            && self.available.is_none_or(|expected| available == expected)
            && self.source.as_ref().is_none_or(|source| {
                game.installations
                    .iter()
                    .any(|installation| &installation.launcher == source)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Installation;

    #[test]
    fn combines_all_filters_and_search_for_roms() {
        let mut game = Game::from_installation(
            "Adventure",
            "ROM",
            Installation {
                path: "/roms/adventure.gba".into(),
                launcher: Launcher::Manual,
                launch_profile: None,
                available: false,
            },
        );
        game.favorite = true;
        let mut filters = Filters {
            kind: GameKind::Rom,
            source: Some(Launcher::Manual),
            available: Some(false),
            favorites_only: true,
            ..Default::default()
        };
        assert!(filters.matches(&game, "advance", Some("Game Boy Advance")));
        filters.available = Some(true);
        assert!(!filters.matches(&game, "advance", Some("Game Boy Advance")));
        filters.available = None;
        filters.kind = GameKind::Pc;
        assert!(!filters.matches(&game, "adventure", Some("Game Boy Advance")));
    }
}
