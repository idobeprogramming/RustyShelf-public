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

pub struct SearchQuery {
    terms: Vec<String>,
}

impl SearchQuery {
    pub fn new(query: &str) -> Self {
        Self {
            terms: normalize_search(query)
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
        }
    }

    fn matches(&self, game: &Game, console: Option<&str>) -> bool {
        if self.terms.is_empty() {
            return true;
        }
        let category = if console.is_some() && game.category.starts_with("ROM · ") {
            ""
        } else {
            &game.category
        };
        let mut searchable = format!("{} {} {}", game.title, category, console.unwrap_or(""));
        for installation in &game.installations {
            searchable.push(' ');
            searchable.push_str(installation.launcher.label());
        }
        let searchable = normalize_search(&searchable);
        self.terms.iter().all(|term| searchable.contains(term))
    }
}

fn normalize_search(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for character in text.chars().flat_map(char::to_lowercase) {
        normalized.push_str(match character {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'î' | 'ï' | 'í' | 'ì' => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' => "u",
            'ç' => "c",
            'ñ' => "n",
            'œ' => "oe",
            'æ' => "ae",
            '\u{0300}'..='\u{036f}' => "", // Decomposed accents from copied titles.
            _ => {
                normalized.push(character);
                continue;
            }
        });
    }
    normalized
}

impl Filters {
    pub fn matches(&self, game: &Game, query: &SearchQuery, console: Option<&str>) -> bool {
        let matching_installations = game.installations.iter().filter(|installation| {
            self.source
                .as_ref()
                .is_none_or(|source| &installation.launcher == source)
        });
        let has_source = self.source.is_none() || matching_installations.clone().next().is_some();
        let available = matching_installations
            .into_iter()
            .any(|installation| installation.available);
        query.matches(game, console)
            && match self.kind {
                GameKind::All => true,
                GameKind::Pc => console.is_none(),
                GameKind::Rom => console.is_some(),
            }
            && (!self.favorites_only || game.favorite)
            && self.available.is_none_or(|expected| available == expected)
            && has_source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Installation;

    #[test]
    fn availability_is_evaluated_for_the_selected_launcher() {
        let mut game = Game::from_installation(
            "Game",
            "Steam",
            Installation {
                path: "/offline/game".into(),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: false,
            },
        );
        game.installations.push(Installation {
            path: "/online/game".into(),
            launcher: Launcher::Epic,
            launch_profile: None,
            available: true,
        });
        let query = SearchQuery::new("");
        let mut filters = Filters {
            source: Some(Launcher::Steam),
            available: Some(true),
            ..Default::default()
        };
        assert!(!filters.matches(&game, &query, None));
        filters.available = Some(false);
        assert!(filters.matches(&game, &query, None));
        filters.source = Some(Launcher::Gog);
        assert!(!filters.matches(&game, &query, None));
        filters.source = None;
        assert!(!filters.matches(&game, &query, None));
        filters.available = Some(true);
        assert!(filters.matches(&game, &query, None));
        // A second available Steam installation makes that source available.
        game.installations[1].launcher = Launcher::Steam;
        filters.source = Some(Launcher::Steam);
        assert!(filters.matches(&game, &query, None));
        filters.available = Some(false);
        assert!(!filters.matches(&game, &query, None));
    }

    #[test]
    fn multiword_search_ignores_accents_and_combines_title_console_and_source() {
        let game = Game::from_installation(
            "L’Étoile du cœur",
            "ROM · PlayStation 2",
            Installation {
                path: "/roms/adventure.iso".into(),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        let filters = Filters::default();
        assert!(filters.matches(
            &game,
            &SearchQuery::new("  STEAM coeur etoile NES  "),
            Some("NES")
        ));
        assert!(filters.matches(&game, &SearchQuery::new("E\u{0301}TOILE"), Some("NES")));
        assert!(!filters.matches(&game, &SearchQuery::new("etoile xbox"), Some("NES")));
        assert!(!filters.matches(&game, &SearchQuery::new("PlayStation"), Some("NES")));
        assert!(filters.matches(&game, &SearchQuery::new("  "), Some("NES")));
    }

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
        assert!(filters.matches(
            &game,
            &SearchQuery::new("advance"),
            Some("Game Boy Advance")
        ));
        filters.available = Some(true);
        assert!(!filters.matches(
            &game,
            &SearchQuery::new("advance"),
            Some("Game Boy Advance")
        ));
        filters.available = None;
        filters.kind = GameKind::Pc;
        assert!(!filters.matches(
            &game,
            &SearchQuery::new("adventure"),
            Some("Game Boy Advance")
        ));
    }
}
