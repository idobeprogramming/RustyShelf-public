use crate::{
    diagnostics::ScanDiagnostics,
    library::{Filters, GameKind, SearchQuery, Sort},
    model::{Emulator, Game, Installation, LaunchProfile, Launcher},
    scan::{self, ScanControl, ScanEvent, Volume},
    sources,
    store::{ReviewAction, Store},
    style::CustomStyle,
};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
};

const LAST_SCAN_REVIEW: &str = "review:last-scan";

fn scan_outcome(language: Language, cancelled: bool) -> &'static str {
    if cancelled {
        tr(language, "Scan interrompu", "Scan interrupted")
    } else {
        tr(language, "Scan terminé", "Scan complete")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    French,
    English,
}

impl Language {
    fn parse(value: Option<String>) -> Self {
        if value.as_deref() == Some("en") {
            Self::English
        } else {
            Self::French
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::French => "fr",
            Self::English => "en",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
    Breeze,
    Horizon,
    Classic,
    Prism,
}

impl Theme {
    fn parse(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("light") => Self::Light,
            Some("breeze" | "vista") => Self::Breeze,
            Some("horizon" | "xp") => Self::Horizon,
            Some("retro" | "win98") => Self::Classic,
            Some("prism" | "liquid") => Self::Prism,
            _ => Self::Dark,
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Breeze => "breeze",
            Self::Horizon => "horizon",
            Self::Classic => "retro",
            Self::Prism => "prism",
        }
    }
    fn label(self, language: Language) -> &'static str {
        match (self, language) {
            (Self::Dark, Language::French) => "Nocturne",
            (Self::Dark, Language::English) => "Nocturne",
            (Self::Light, Language::French) => "Aurore",
            (Self::Light, Language::English) => "Dawn",
            (Self::Breeze, Language::French) => "Brise",
            (Self::Breeze, Language::English) => "Brise",
            (Self::Horizon, Language::French) => "Horizon",
            (Self::Horizon, Language::English) => "Horizon",
            (Self::Classic, Language::French) => "Rétro",
            (Self::Classic, Language::English) => "Rétro",
            (Self::Prism, Language::French) => "Prisme",
            (Self::Prism, Language::English) => "Prisme",
        }
    }
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    let mut visuals = match theme {
        Theme::Light | Theme::Breeze | Theme::Horizon | Theme::Classic => egui::Visuals::light(),
        Theme::Dark | Theme::Prism => egui::Visuals::dark(),
    };
    match theme {
        Theme::Dark => {
            visuals.panel_fill = Color32::from_rgb(21, 25, 31);
            visuals.window_fill = Color32::from_rgb(32, 38, 48);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(41, 49, 61);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 64, 79);
            visuals.widgets.active.bg_fill = Color32::from_rgb(229, 162, 112);
        }
        Theme::Light => {
            visuals.panel_fill = Color32::from_rgb(239, 242, 248);
            visuals.window_fill = Color32::from_rgb(250, 251, 254);
            visuals.widgets.active.bg_fill = Color32::from_rgb(102, 85, 190);
        }
        Theme::Breeze => {
            visuals.panel_fill = Color32::from_rgb(211, 229, 245);
            visuals.window_fill = Color32::from_rgb(237, 246, 255);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(220, 237, 251);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(188, 220, 246);
            visuals.widgets.active.bg_fill = Color32::from_rgb(89, 151, 204);
            visuals.window_corner_radius = egui::CornerRadius::same(5);
        }
        Theme::Horizon => {
            visuals.panel_fill = Color32::from_rgb(226, 236, 252);
            visuals.window_fill = Color32::from_rgb(246, 249, 255);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(233, 241, 253);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(219, 233, 252);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(192, 218, 250);
            visuals.widgets.active.bg_fill = Color32::from_rgb(49, 106, 197);
            visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
            visuals.window_corner_radius = egui::CornerRadius::same(8);
        }
        Theme::Classic => {
            let classic_gray = Color32::from_rgb(192, 192, 192);
            visuals.panel_fill = classic_gray;
            visuals.window_fill = classic_gray;
            visuals.widgets.noninteractive.bg_fill = classic_gray;
            visuals.widgets.inactive.bg_fill = classic_gray;
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(212, 208, 200);
            visuals.widgets.active.bg_fill = Color32::from_rgb(0, 0, 128);
            visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
            visuals.window_corner_radius = egui::CornerRadius::ZERO;
            visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::ZERO;
            visuals.widgets.inactive.corner_radius = egui::CornerRadius::ZERO;
            visuals.widgets.hovered.corner_radius = egui::CornerRadius::ZERO;
            visuals.widgets.active.corner_radius = egui::CornerRadius::ZERO;
        }
        Theme::Prism => {
            visuals.panel_fill = Color32::from_rgba_unmultiplied(18, 24, 35, 82);
            // Floating windows need stronger tint to keep the library below quiet.
            visuals.window_fill = Color32::from_rgba_unmultiplied(30, 37, 48, 224);
            visuals.window_stroke = egui::Stroke::new(1.0_f32, Color32::from_white_alpha(72));
            visuals.extreme_bg_color = Color32::from_black_alpha(38);
            visuals.faint_bg_color = Color32::from_white_alpha(12);
            visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
            visuals.widgets.noninteractive.bg_stroke =
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(28));
            visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(22);
            visuals.widgets.inactive.weak_bg_fill = Color32::from_white_alpha(22);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(48));
            visuals.widgets.hovered.bg_fill = Color32::from_white_alpha(40);
            visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(40);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(110));
            visuals.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(144, 199, 255, 105);
            visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
            visuals.widgets.open.bg_fill = Color32::from_rgba_unmultiplied(45, 55, 70, 240);
            visuals.widgets.open.weak_bg_fill = visuals.widgets.open.bg_fill;
            visuals.window_corner_radius = egui::CornerRadius::same(22);
            visuals.menu_corner_radius = egui::CornerRadius::same(16);
            for widget in [
                &mut visuals.widgets.inactive,
                &mut visuals.widgets.hovered,
                &mut visuals.widgets.active,
                &mut visuals.widgets.open,
            ] {
                widget.corner_radius = egui::CornerRadius::same(12);
            }
            visuals.window_shadow = egui::epaint::Shadow {
                offset: [0, 12],
                blur: 36,
                spread: 2,
                color: Color32::from_black_alpha(70),
            };
        }
    }
    visuals.widgets.active.fg_stroke.color = crate::style::text_on(visuals.widgets.active.bg_fill);
    visuals.widgets.noninteractive.fg_stroke.color = theme_text(theme);
    visuals.widgets.inactive.fg_stroke.color = theme_text(theme);
    visuals.selection.bg_fill = theme_accent(theme);
    visuals.selection.stroke.color = crate::style::text_on(theme_accent(theme));
    // The selected application theme must not switch when the OS sends its theme.
    ctx.set_theme(if visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
    });
}

fn tr(language: Language, french: &'static str, english: &'static str) -> &'static str {
    match language {
        Language::French => french,
        Language::English => english,
    }
}

fn load_mapping(store: Option<&Store>, key: &str) -> HashMap<String, String> {
    store
        .and_then(|store| store.preference(key).ok())
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn launcher_label(launcher: &Launcher, language: Language) -> &'static str {
    match launcher {
        Launcher::Steam => "Steam",
        Launcher::Epic => "Epic Games",
        Launcher::Heroic => "Heroic",
        Launcher::Lutris => "Lutris",
        Launcher::Gog => "GOG",
        Launcher::Manual => tr(language, "Ajout manuel", "Manual add"),
        Launcher::Unknown => tr(language, "À confirmer", "To review"),
    }
}

pub struct RustyShelfApp {
    selected_category: String,
    search: String,
    show_options: bool,
    show_ignored: bool,
    ignored_games: Vec<Game>,
    ignored_search: String,
    games: Vec<Game>,
    selected_game: Option<String>,
    review_selection: HashSet<String>,
    store: Option<Store>,
    status: String,
    volumes: Vec<Volume>,
    selected_volumes: Vec<bool>,
    saved_scan_volumes: Option<Vec<PathBuf>>,
    exclusions: Vec<PathBuf>,
    scan_control: Option<ScanControl>,
    scan_receiver: Option<Receiver<ScanEvent>>,
    scan_visited: usize,
    scan_current: String,
    scan_roots: Vec<PathBuf>,
    scan_exclusions: Vec<PathBuf>,
    scan_seen_paths: Vec<PathBuf>,
    diagnostics: ScanDiagnostics,
    cover_textures: HashMap<String, (PathBuf, Option<TextureHandle>)>,
    cover_refresh: Option<crate::covers::Refresh>,
    category_edit: String,
    category_edit_id: Option<String>,
    title_edit: String,
    title_edit_id: Option<String>,
    language: Language,
    theme: Theme,
    native_glass: Option<bool>,
    emulators: Vec<Emulator>,
    css_path: Option<PathBuf>,
    css_enabled: bool,
    custom_style: Option<CustomStyle>,
    css_error: Option<String>,
    filters: Filters,
    console_overrides: HashMap<String, String>,
    console_emulators: HashMap<String, String>,
}

impl RustyShelfApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.show_options = false;
            self.show_ignored = false;
        }
        if !self.show_options
            && !self.show_ignored
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::F))
        {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("library-search")));
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (store, store_error) = match Store::open_default() {
            Ok(store) => (Some(store), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let language = Language::parse(
            store
                .as_ref()
                .and_then(|s| s.preference("language").ok())
                .flatten(),
        );
        let theme = Theme::parse(
            store
                .as_ref()
                .and_then(|s| s.preference("theme").ok())
                .flatten(),
        );
        apply_theme(&cc.egui_ctx, theme);
        let css_path = store
            .as_ref()
            .and_then(|store| store.preference("css_path").ok())
            .flatten()
            .map(PathBuf::from);
        let css_enabled = store
            .as_ref()
            .and_then(|store| store.preference("css_enabled").ok())
            .flatten()
            .as_deref()
            == Some("true");
        let (custom_style, css_error) = if css_enabled {
            match css_path.as_deref().map(CustomStyle::load) {
                Some(Ok(style)) => {
                    style.apply(&cc.egui_ctx);
                    (Some(style), None)
                }
                Some(Err(error)) => (None, Some(error)),
                None => (None, None),
            }
        } else {
            (None, None)
        };
        let mut status = String::from("Hors ligne · Données locales");
        let games = store
            .as_ref()
            .and_then(|s| s.games().ok())
            .map(deduplicate_library)
            .unwrap_or_default();
        if let Some(store) = &store {
            for emulator in crate::emulator::discover_installed() {
                let _ = store.save_emulator(&emulator);
            }
        }
        let emulators = store
            .as_ref()
            .and_then(|s| s.emulators().ok())
            .unwrap_or_default();
        let exclusions = store
            .as_ref()
            .and_then(|s| s.exclusions().ok())
            .unwrap_or_default();
        if let Some(error) = store_error {
            status = format!("Base locale indisponible : {error}");
        }
        let volumes = scan::mounted_volumes();
        let mut console_overrides = load_mapping(store.as_ref(), "console_overrides");
        console_overrides.retain(|_, console| crate::rom::assignable_console(console));
        let mut console_emulators = load_mapping(store.as_ref(), "console_emulators");
        console_emulators.retain(|console, _| crate::rom::assignable_console(console));
        let diagnostics = store
            .as_ref()
            .and_then(|store| store.preference("last_scan_diagnostics").ok())
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let saved_scan_volumes = store
            .as_ref()
            .and_then(|store| store.preference("selected_scan_volumes").ok())
            .flatten()
            .and_then(|json| serde_json::from_str::<Vec<PathBuf>>(&json).ok());
        let selected_volumes = scan::volume_selection(&volumes, saved_scan_volumes.as_deref());
        Self {
            selected_category: "Tous les jeux".into(),
            search: String::new(),
            show_options: false,
            show_ignored: false,
            ignored_games: Vec::new(),
            ignored_search: String::new(),
            selected_game: games
                .iter()
                .find(|game| game.confirmed)
                .or_else(|| games.first())
                .map(|game| game.id.clone()),
            review_selection: HashSet::new(),
            games,
            store,
            status,
            volumes,
            selected_volumes,
            saved_scan_volumes,
            exclusions,
            scan_control: None,
            scan_receiver: None,
            scan_visited: 0,
            scan_current: String::new(),
            scan_roots: Vec::new(),
            scan_exclusions: Vec::new(),
            scan_seen_paths: Vec::new(),
            diagnostics,
            cover_textures: HashMap::new(),
            cover_refresh: None,
            category_edit: String::new(),
            category_edit_id: None,
            title_edit: String::new(),
            title_edit_id: None,
            language,
            theme,
            native_glass: None,
            emulators,
            css_path,
            css_enabled,
            custom_style,
            css_error,
            filters: Filters::default(),
            console_overrides,
            console_emulators,
        }
    }

    fn active_style(&self) -> Option<&CustomStyle> {
        self.custom_style.as_ref().filter(|_| self.css_enabled)
    }

    fn persist(&mut self, action: impl FnOnce(&Store) -> rusqlite::Result<()>) -> bool {
        let result = self
            .store
            .as_ref()
            .ok_or_else(|| {
                tr(
                    self.language,
                    "Base locale indisponible : modification non enregistrée.",
                    "Local database unavailable: change was not saved.",
                )
                .to_owned()
            })
            .and_then(|store| action(store).map_err(|error| error.to_string()));
        match result {
            Ok(()) => true,
            Err(error) => {
                self.status = format!(
                    "{} : {error}",
                    tr(self.language, "Échec de l’enregistrement", "Save failed")
                );
                false
            }
        }
    }

    fn update_library(&mut self, action: impl FnOnce(&Store) -> rusqlite::Result<()>) -> bool {
        if !self.persist(action) {
            return false;
        }
        // A failed reload must not turn an existing library into an empty one.
        if let Some(store) = &self.store {
            match store.games() {
                Ok(games) => self.games = deduplicate_library(games),
                Err(error) => {
                    self.status = format!(
                        "{} : {error}",
                        tr(
                            self.language,
                            "Modification enregistrée, rechargement impossible",
                            "Change saved, reload failed"
                        )
                    );
                    return false;
                }
            }
        }
        true
    }

    fn apply_review_selection(&mut self, action: ReviewAction) {
        let ids: Vec<String> = self
            .games
            .iter()
            .filter(|game| {
                !game.confirmed
                    && self.matches_category(game)
                    && self.review_selection.contains(&game.id)
            })
            .map(|game| game.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        if self.update_library(|store| store.review_games(&ids, action)) {
            self.review_selection.clear();
            self.status = format!(
                "{} {}",
                ids.len(),
                match action {
                    ReviewAction::Confirm =>
                        tr(self.language, "jeu(x) confirmé(s).", "game(s) confirmed."),
                    ReviewAction::Ignore => tr(
                        self.language,
                        "élément(s) ignoré(s). Aucun fichier supprimé.",
                        "item(s) ignored. No files deleted."
                    ),
                }
            );
        }
    }

    fn open_ignored_games(&mut self) {
        if let Some(store) = &self.store {
            match store.ignored_games() {
                Ok(games) => {
                    self.ignored_games = games;
                    self.show_ignored = true;
                    self.show_options = false;
                }
                Err(error) => self.status = error.to_string(),
            }
        } else {
            self.persist(|_| Ok(()));
        }
    }

    fn restore_ignored_item(&mut self, id: &str) {
        if self.update_library(|store| store.restore_ignored_game(id)) {
            self.ignored_games.retain(|game| game.id != id);
            self.status = tr(
                self.language,
                "Élément restauré dans la bibliothèque.",
                "Item restored to the library.",
            )
            .into();
        }
    }

    fn ignored_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_ignored;
        let mut restore = None;
        let mut restore_all = false;
        egui::Window::new(tr(self.language, "Éléments ignorés", "Ignored items"))
            .id(egui::Id::new("ignored-items-window")).open(&mut open)
            .collapsible(false).resizable(true).default_width(650.0)
            .min_width((ctx.screen_rect().width() - 48.0).clamp(280.0, 480.0))
            .show(ctx, |ui| {
                ui.label(tr(self.language, "Restaure uniquement les éléments souhaités. Aucun fichier de jeu n’a été supprimé.", "Restore only the items you want. No game files were deleted."));
                ui.add(egui::TextEdit::singleline(&mut self.ignored_search).hint_text(tr(self.language, "Rechercher un élément ignoré…", "Search ignored items…")));
                let query = SearchQuery::new(&self.ignored_search);
                let filters = Filters::default();
                let games: Vec<_> = self.ignored_games.iter().filter(|game| filters.matches(game, &query, self.game_console(game))).collect();
                ui.label(format!("{} / {}", games.len(), self.ignored_games.len()));
                egui::ScrollArea::vertical().id_salt("ignored-items-list").auto_shrink([false, true])
                    .max_height(360.0).show_rows(ui, 64.0, games.len(), |ui, range| {
                        for index in range {
                            let game = games[index];
                            ui.push_id(&game.id, |ui| {
                                ui.set_min_height(64.0);
                                ui.horizontal(|ui| {
                                    if ui.button(tr(self.language, "Restaurer", "Restore")).clicked() { restore = Some(game.id.clone()); }
                                    ui.add(egui::Label::new(RichText::new(&game.title).strong().color(self.foreground_color())).truncate()).on_hover_text(&game.title);
                                });
                                if let Some(installation) = game.installations.first() {
                                    let path = installation.path.display().to_string();
                                    ui.add(egui::Label::new(&path).truncate()).on_hover_text(&path);
                                }
                                ui.separator();
                            });
                        }
                    });
                if self.ignored_games.is_empty() { ui.label(tr(self.language, "Aucun élément ignoré.", "No ignored items.")); }
                if ui.add_enabled(!self.ignored_games.is_empty(), egui::Button::new(tr(self.language, "Tout restaurer", "Restore all"))).clicked() { restore_all = true; }
                ui.small(&self.status);
            });
        self.show_ignored = open;
        if let Some(id) = restore {
            self.restore_ignored_item(&id);
        }
        if restore_all && self.update_library(Store::restore_ignored_games) {
            self.ignored_games.clear();
        }
    }

    fn game_console<'a>(&'a self, game: &Game) -> Option<&'a str> {
        self.console_overrides
            .get(&game.id)
            .map(String::as_str)
            .or_else(|| crate::rom::game_console(game))
    }

    fn category_emulator(&self, game: &Game) -> Option<&Emulator> {
        crate::emulator::for_category(
            self.game_console(game),
            &self.console_emulators,
            &self.emulators,
        )
    }

    fn category_emulator_ui(&mut self, ui: &mut egui::Ui, console: &str) {
        if !crate::rom::assignable_console(console) {
            return;
        }
        let previous = self.console_emulators.get(console).cloned();
        let mut chosen = previous.clone();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} · {console}",
                tr(
                    self.language,
                    "Émulateur de cette catégorie",
                    "Category emulator"
                )
            ));
            egui::ComboBox::from_id_salt(("category-emulator", console))
                .selected_text(
                    chosen
                        .as_ref()
                        .and_then(|id| self.emulators.iter().find(|emulator| &emulator.id == id))
                        .map(|emulator| emulator.name.as_str())
                        .unwrap_or(tr(self.language, "Aucun · à choisir", "None · choose one")),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, None, tr(self.language, "Aucun", "None"));
                    for emulator in &self.emulators {
                        ui.selectable_value(&mut chosen, Some(emulator.id.clone()), &emulator.name);
                    }
                });
            if ui
                .button(tr(self.language, "Ajouter un émulateur…", "Add emulator…"))
                .clicked()
            {
                self.add_emulator();
            }
        });
        if previous != chosen {
            let mut mapping = self.console_emulators.clone();
            if let Some(id) = chosen {
                mapping.insert(console.into(), id);
            } else {
                mapping.remove(console);
            }
            if self.save_mapping("console_emulators", &mapping) {
                self.console_emulators = mapping;
            }
        }
    }
    fn save_mapping(&mut self, key: &str, mapping: &HashMap<String, String>) -> bool {
        match serde_json::to_string(mapping) {
            Ok(json) => self.persist(|store| store.set_preference(key, &json)),
            Err(error) => {
                self.status = error.to_string();
                false
            }
        }
    }

    fn refresh_appearance(&self, ctx: &egui::Context) {
        apply_theme(ctx, self.theme);
        if let Some(style) = self.active_style() {
            style.apply(ctx);
        }
    }

    fn load_css(&mut self, ctx: &egui::Context, path: PathBuf) {
        match CustomStyle::load(&path) {
            Ok(style) => {
                if !self.persist(|store| {
                    store.set_preferences(&[
                        ("css_path", &path.to_string_lossy()),
                        ("css_enabled", "true"),
                    ])
                }) {
                    self.css_error = Some(self.status.clone());
                    return;
                }
                self.css_path = Some(path);
                self.custom_style = Some(style);
                self.css_enabled = true;
                self.css_error = None;
                self.refresh_appearance(ctx);
            }
            Err(error) => self.css_error = Some(error),
        }
    }

    fn set_css_enabled(&mut self, ctx: &egui::Context, enabled: bool) {
        if enabled {
            if let Some(path) = self.css_path.clone() {
                self.load_css(ctx, path);
            } else {
                self.css_error = Some(
                    tr(
                        self.language,
                        "Choisis d’abord un fichier CSS.",
                        "Choose a CSS file first.",
                    )
                    .into(),
                );
            }
        } else if self.persist(|store| store.set_preference("css_enabled", "false")) {
            self.css_enabled = false;
            self.css_error = None;
            self.refresh_appearance(ctx);
        } else {
            self.css_error = Some(self.status.clone());
        }
    }

    fn start_scan(&mut self) {
        if let Some(control) = &self.scan_control {
            control.cancel();
            return;
        }
        if self.store.is_none() {
            self.persist(|_| Ok(()));
            return;
        }
        let roots: Vec<_> = self
            .volumes
            .iter()
            .zip(&self.selected_volumes)
            .filter(|(_, selected)| **selected)
            .map(|(volume, _)| volume.root.clone())
            .collect();
        if roots.is_empty() {
            self.status = tr(
                self.language,
                "Sélectionne au moins un disque à scanner.",
                "Select at least one drive to scan.",
            )
            .into();
            return;
        }
        self.begin_scan(roots, true);
    }

    fn save_scan_volumes(&mut self) -> bool {
        let roots = scan::updated_volume_selection(
            &self.volumes,
            &self.selected_volumes,
            self.saved_scan_volumes.as_deref().unwrap_or_default(),
        );
        match serde_json::to_string(&roots) {
            Ok(json)
                if self.persist(|store| store.set_preference("selected_scan_volumes", &json)) =>
            {
                self.saved_scan_volumes = Some(roots);
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.status = error.to_string();
                false
            }
        }
    }

    fn begin_scan(&mut self, roots: Vec<PathBuf>, include_launcher_catalogs: bool) {
        if self.scan_control.is_some() {
            return;
        }
        if self.store.is_none() {
            self.persist(|_| Ok(()));
            return;
        }
        let roots = scan::unique_roots(&roots);
        if roots.is_empty() {
            return;
        }
        let exclusions = self.exclusions.clone();
        self.scan_roots = roots.clone();
        self.scan_exclusions = exclusions.clone();
        self.scan_seen_paths.clear();
        let control = ScanControl::default();
        let worker_control = control.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        let previous_candidates: Vec<_> = self
            .games
            .iter()
            .filter(|game| !game.confirmed)
            .cloned()
            .collect();
        let cleanup_label = tr(
            self.language,
            "Vérification des anciens candidats…",
            "Rechecking old candidates…",
        );
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let cover_roots = if include_launcher_catalogs {
                    sources::steam_cache_roots()
                } else {
                    Vec::new()
                };
                let mut launcher_installations = Vec::new();
                let mut emit_candidate = |mut game: Game| {
                    launcher_installations.extend(
                        game.installations
                            .iter()
                            .filter(|installation| {
                                !matches!(
                                    installation.launcher,
                                    Launcher::Unknown | Launcher::Manual
                                ) && installation.available
                                    && installation.path.parent().is_some()
                                    && installation.path.is_dir()
                            })
                            .map(|installation| installation.path.clone()),
                    );
                    game.cover_path = crate::covers::find(&game, &cover_roots, &worker_control);
                    sender.send(ScanEvent::Candidate(game)).is_ok()
                };
                let discovered = if include_launcher_catalogs {
                    sources::discover(&roots, &worker_control, &mut |event| {
                        if sender.send(event).is_err() {
                            worker_control.cancel();
                        }
                    })
                } else {
                    Vec::new()
                };
                let games = sources::within_scan_scope(discovered, &roots, &exclusions);
                let mut scan_exclusions = exclusions.clone();
                for game in games {
                    if worker_control.is_cancelled() {
                        break;
                    }
                    scan_exclusions.extend(
                        game.installations
                            .iter()
                            .map(|installation| installation.path.clone()),
                    );
                    if !emit_candidate(game) {
                        return;
                    }
                }
                let mut scan_result = (0, true);
                scan::scan_roots(&roots, &scan_exclusions, &worker_control, |event| {
                    let result = match event {
                        ScanEvent::Candidate(game) => emit_candidate(game),
                        ScanEvent::Finished { visited, cancelled } => {
                            scan_result = (visited, cancelled);
                            true
                        }
                        event => sender.send(event).is_ok(),
                    };
                    if !result {
                        worker_control.cancel();
                    }
                });
                let (visited, cancelled) = scan_result;
                // Discovery must reach the store first: an old candidate can be
                // promoted by a manifest or reappear with a valid new profile.
                if !cancelled && !previous_candidates.is_empty() {
                    if sender
                        .send(ScanEvent::Progress(scan::ScanProgress {
                            visited,
                            current: PathBuf::from(cleanup_label),
                        }))
                        .is_err()
                    {
                        return;
                    }
                    for game in previous_candidates {
                        if worker_control.is_cancelled() {
                            break;
                        }
                        if scan::obsolete_candidate(
                            &game,
                            &roots,
                            &exclusions,
                            &launcher_installations,
                        ) && sender.send(ScanEvent::RejectedCandidate(game.id)).is_err()
                        {
                            return;
                        }
                    }
                }
                let _ = sender.send(ScanEvent::Finished {
                    visited,
                    cancelled: cancelled || worker_control.is_cancelled(),
                });
            }));
            if outcome.is_err() {
                let _ = sender.send(ScanEvent::Error(
                    PathBuf::new(),
                    "Le scan a rencontré une erreur inattendue.".into(),
                ));
                let _ = sender.send(ScanEvent::Finished {
                    visited: 0,
                    cancelled: true,
                });
            }
        });
        self.scan_control = Some(control);
        self.scan_receiver = Some(receiver);
        if self.selected_category == LAST_SCAN_REVIEW {
            self.selected_category = "À confirmer".into();
        }
        self.scan_visited = 0;
        self.scan_current.clear();
        self.diagnostics = ScanDiagnostics {
            roots: self.scan_roots.clone(),
            ..Default::default()
        };
        self.status = tr(
            self.language,
            "Analyse des disques en cours…",
            "Scanning drives…",
        )
        .into();
        self.show_options = false;
    }

    fn poll_scan(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.scan_receiver else {
            return;
        };
        let mut events = Vec::new();
        for _ in 0..8 {
            match receiver.try_recv() {
                Ok(event) => {
                    let finished = matches!(event, ScanEvent::Finished { .. });
                    events.push(event);
                    if finished {
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    events.push(ScanEvent::Error(
                        PathBuf::new(),
                        "Le processus de scan s’est arrêté sans terminer.".into(),
                    ));
                    events.push(ScanEvent::Finished {
                        visited: self.scan_visited,
                        cancelled: true,
                    });
                    break;
                }
            }
        }
        for event in events {
            match event {
                ScanEvent::RejectedCandidate(id) => {
                    if self.console_overrides.contains_key(&id)
                        || self
                            .games
                            .iter()
                            .find(|game| game.id == id)
                            .is_some_and(|game| {
                                game.installations.iter().any(|installation| {
                                    self.scan_seen_paths
                                        .iter()
                                        .any(|path| same_path(path, &installation.path))
                                })
                            })
                    {
                        continue;
                    }
                    if let Some(store) = &self.store {
                        match store.ignore_obsolete_candidate(&id) {
                            Ok(true) => {
                                self.games.retain(|game| game.id != id);
                                self.diagnostics.filtered_count += 1;
                            }
                            Ok(false) => {}
                            Err(error) => self.diagnostics.error(error.to_string()),
                        }
                    }
                }
                ScanEvent::Candidate(mut game) => {
                    self.scan_seen_paths.extend(
                        game.installations
                            .iter()
                            .map(|installation| installation.path.clone()),
                    );
                    if let Some(store) = &self.store {
                        if let Err(error) = store.reuse_installation_identity(&mut game) {
                            self.diagnostics.error(error.to_string());
                            continue;
                        }
                        match store.is_ignored(&game.id) {
                            Ok(true) => continue,
                            Ok(false) => {}
                            Err(error) => {
                                self.diagnostics.error(error.to_string());
                                continue;
                            }
                        }
                        if let Err(error) = store.save_game(&game) {
                            self.diagnostics.error(error.to_string());
                            continue;
                        }
                        match store.custom_title(&game.id) {
                            Ok(Some(title)) => game.title = title,
                            Ok(None) => {}
                            Err(error) => {
                                self.diagnostics.error(error.to_string());
                                continue;
                            }
                        }
                    }
                    let before = self.games.len();
                    let id = game.id.clone();
                    merge_game_in_memory(&mut self.games, game);
                    if self.games.len() > before {
                        self.diagnostics.added_game_ids.insert(id);
                    }
                }
                ScanEvent::Progress(progress) => {
                    self.scan_visited = progress.visited;
                    self.diagnostics.visited = progress.visited;
                    self.scan_current = progress.current.to_string_lossy().to_string();
                }
                ScanEvent::Error(path, message) => {
                    // An unreadable subtree is not proof that a game was removed.
                    if !path.as_os_str().is_empty() {
                        self.scan_exclusions.push(path.clone());
                    }
                    self.diagnostics
                        .error(format!("{} : {message}", path.display()));
                }
                ScanEvent::Skipped(path, reason) => {
                    self.scan_exclusions.push(path.clone());
                    self.diagnostics.skip(&path, &reason);
                }
                ScanEvent::Finished { visited, cancelled } => {
                    self.scan_visited = visited;
                    self.diagnostics.visited = visited;
                    self.diagnostics.cancelled = Some(cancelled);
                    if let Some(store) = &self.store {
                        if !cancelled {
                            if let Err(error) = store.mark_unseen_under_roots_unavailable(
                                &self.scan_roots,
                                &self.scan_exclusions,
                                &self.scan_seen_paths,
                            ) {
                                self.diagnostics.error(error.to_string());
                            }
                        }
                        // Even a cancelled scan saved partial results. Reload the
                        // preserved user metadata, but never mark unseen games missing.
                        match store.games() {
                            Ok(games) => self.games = deduplicate_library(games),
                            Err(error) => self.diagnostics.error(error.to_string()),
                        }
                    }
                    self.scan_control = None;
                    self.scan_receiver = None;
                    let visible_ids: HashSet<_> = self.games.iter().map(|game| &game.id).collect();
                    self.diagnostics
                        .added_game_ids
                        .retain(|id| visible_ids.contains(id));
                    if let Some(store) = &self.store {
                        if let Ok(json) = serde_json::to_string(&self.diagnostics) {
                            if let Err(error) = store.set_preference("last_scan_diagnostics", &json)
                            {
                                self.diagnostics.error(error.to_string());
                            }
                        }
                    }
                    self.status = scan_outcome(self.language, cancelled).into();
                }
            }
        }
        if self.scan_control.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn add_manual_game(&mut self) {
        let Some(executable) = rfd::FileDialog::new()
            .add_filter("Exécutable", &["exe", "sh", "x86_64", "AppImage"])
            .add_filter(
                "ROM / image disque",
                &[
                    "nes", "sfc", "smc", "gb", "gbc", "gba", "nds", "3ds", "n64", "z64", "iso",
                    "chd", "cso", "wbfs", "rvz", "gcm", "cue", "pbp", "gen", "sms", "gg", "v64",
                    "cci", "smd", "md", "gcz", "cdi", "nrg", "mdf",
                ],
            )
            .add_filter("Tous les fichiers", &["*"])
            .pick_file()
        else {
            return;
        };
        self.add_manual_path(executable);
    }

    fn add_manual_path(&mut self, executable: PathBuf) {
        if !executable.is_file() {
            self.status = tr(
                self.language,
                "Le fichier choisi est introuvable ou inaccessible.",
                "The selected file is missing or inaccessible.",
            )
            .into();
            return;
        }
        let title = executable
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Nouveau jeu".into());
        let is_rom = crate::rom::console(&executable).is_some();
        let install_path = if is_rom {
            executable.clone()
        } else {
            executable.parent().unwrap_or(&executable).to_path_buf()
        };
        let profile = LaunchProfile {
            command: executable,
            arguments: Vec::new(),
            working_directory: Some(install_path.clone()),
            launcher_command: None,
        };
        let mut game = Game::from_installation(
            title,
            "Ajout manuel",
            Installation {
                path: install_path,
                launcher: Launcher::Manual,
                launch_profile: if is_rom { None } else { Some(profile) },
                available: true,
            },
        );
        game.cover_path = crate::covers::find(&game, &[], &ScanControl::default());
        if self.update_library(|store| store.save_manual_game(&mut game)) {
            self.selected_game = Some(game.id);
            self.status = tr(
                self.language,
                "Jeu ajouté à la bibliothèque.",
                "Game added to your library.",
            )
            .into();
        }
    }

    fn add_emulator(&mut self) {
        let Some(executable) = rfd::FileDialog::new().pick_file() else {
            return;
        };
        let name = executable
            .file_stem()
            .map(|name| name.to_string_lossy().replace(['_', '-'], " "))
            .unwrap_or_else(|| "Émulateur".into());
        let emulator = Emulator::new(name, executable);
        if let Some(store) = &self.store {
            match store
                .save_emulator(&emulator)
                .and_then(|_| store.emulators())
            {
                Ok(emulators) => {
                    self.emulators = emulators;
                    self.status = tr(self.language, "Émulateur ajouté.", "Emulator added.").into();
                }
                Err(error) => {
                    self.status = format!("{}: {error}", tr(self.language, "Erreur", "Error"))
                }
            }
        }
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let accent = self
            .active_style()
            .and_then(|style| style.accent)
            .unwrap_or(theme_accent(self.theme));
        let muted = self
            .active_style()
            .and_then(|style| style.muted)
            .unwrap_or(theme_muted_text(self.theme));
        let mut consoles = std::collections::BTreeMap::<String, usize>::new();
        for game in self.games.iter().filter(|game| game.confirmed) {
            if let Some(console) = self.game_console(game) {
                *consoles.entry(console.to_owned()).or_default() += 1;
            }
        }
        let custom_categories: std::collections::BTreeSet<String> = self
            .games
            .iter()
            .filter(|game| {
                game.confirmed
                    && ![
                        "Steam",
                        "Epic Games",
                        "Heroic",
                        "GOG",
                        "Lutris",
                        "Ajout manuel",
                        "À confirmer",
                    ]
                    .contains(&game.category.as_str())
                    && !game.category.starts_with("ROM")
            })
            .map(|game| game.category.clone())
            .collect();
        egui::SidePanel::left("navigation")
            .exact_width(215.0)
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("▦").size(30.0).color(accent));
                    ui.label(
                        RichText::new("RustyShelf")
                            .size(21.0)
                            .strong()
                            .color(self.foreground_color()),
                    );
                });
                ui.label(
                    RichText::new(tr(
                        self.language,
                        "Chaque jeu à sa place.",
                        "Every game, in its place.",
                    ))
                    .size(11.0)
                    .color(muted),
                );
                ui.add_space(18.0);
                egui::ScrollArea::vertical()
                    .id_salt("navigation-scroll")
                    .max_height((ui.available_height() - 95.0).max(80.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(tr(self.language, "BIBLIOTHÈQUE", "LIBRARY"))
                                .size(10.0)
                                .color(muted),
                        );
                        for (key, english, icon, count) in [
                            (
                                "Tous les jeux",
                                "All games",
                                "▦",
                                self.games.iter().filter(|game| game.confirmed).count(),
                            ),
                            (
                                "Favoris",
                                "Favorites",
                                "★",
                                self.games
                                    .iter()
                                    .filter(|game| game.confirmed && game.favorite)
                                    .count(),
                            ),
                            (
                                "Installés",
                                "Available",
                                "●",
                                self.games
                                    .iter()
                                    .filter(|game| {
                                        game.confirmed
                                            && game
                                                .installations
                                                .iter()
                                                .any(|installation| installation.available)
                                    })
                                    .count(),
                            ),
                            (
                                "À confirmer",
                                "To review",
                                "?",
                                self.games.iter().filter(|game| !game.confirmed).count(),
                            ),
                        ] {
                            if ui
                                .selectable_label(
                                    self.selected_category == key
                                        || (key == "À confirmer"
                                            && self.selected_category == LAST_SCAN_REVIEW),
                                    format!("{icon}  {}  {count}", tr(self.language, key, english)),
                                )
                                .clicked()
                            {
                                self.selected_category = key.into();
                            }
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new("ROMS & CONSOLES").size(10.0).color(muted));
                        if ui
                            .selectable_label(
                                self.selected_category == "ROMs",
                                format!("▣  ROMs  {}", consoles.values().sum::<usize>()),
                            )
                            .clicked()
                        {
                            self.selected_category = "ROMs".into();
                        }
                        for (console, count) in consoles {
                            let key = format!("console:{console}");
                            if ui
                                .selectable_label(
                                    self.selected_category == key,
                                    format!("    {console}  {count}"),
                                )
                                .clicked()
                            {
                                self.selected_category = key;
                            }
                        }
                        ui.add_space(14.0);
                        ui.label(RichText::new("SOURCES").size(10.0).color(muted));
                        for name in [
                            "Steam",
                            "Epic Games",
                            "Heroic",
                            "Lutris",
                            "GOG",
                            "Ajout manuel",
                        ] {
                            let label = if name == "Ajout manuel" {
                                tr(self.language, name, "Manual")
                            } else {
                                name
                            };
                            if ui
                                .selectable_label(
                                    self.selected_category == name,
                                    format!("·  {label}"),
                                )
                                .clicked()
                            {
                                self.selected_category = name.into();
                            }
                        }
                        if !custom_categories.is_empty() {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new(tr(self.language, "MES CATÉGORIES", "MY CATEGORIES"))
                                    .size(10.0)
                                    .color(muted),
                            );
                            for category in custom_categories {
                                if ui
                                    .selectable_label(self.selected_category == category, &category)
                                    .clicked()
                                {
                                    self.selected_category = category;
                                }
                            }
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(RichText::new(&self.status).size(10.0).color(muted));
                    if ui
                        .button(tr(
                            self.language,
                            "⚙ Options et apparence",
                            "⚙ Settings and appearance",
                        ))
                        .clicked()
                    {
                        self.show_options = true;
                    }
                });
            });
    }
    fn open_last_scan_candidates(&mut self) {
        self.selected_category = LAST_SCAN_REVIEW.into();
        self.selected_game = None;
        self.review_selection.clear();
        self.search.clear();
        self.filters = Filters {
            sort: self.filters.sort,
            ..Filters::default()
        };
    }

    fn scan_summary(&mut self, ui: &mut egui::Ui) -> bool {
        if self.scan_control.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "{} {}",
                    self.scan_visited,
                    tr(self.language, "éléments parcourus", "items scanned")
                ));
                if !self.scan_current.is_empty() {
                    ui.add(egui::Label::new(&self.scan_current).truncate())
                        .on_hover_text(&self.scan_current);
                }
            });
            return false;
        }
        let Some(cancelled) = self.diagnostics.cancelled else {
            return false;
        };
        let pending = self
            .games
            .iter()
            .filter(|game| !game.confirmed && self.diagnostics.added_game_ids.contains(&game.id))
            .count();
        let mut navigate = false;
        ui.horizontal_wrapped(|ui| {
            ui.strong(scan_outcome(self.language, cancelled));
            ui.small(format!(
                "{} {} · {} {}",
                self.diagnostics.added_game_ids.len(),
                tr(self.language, "fiche(s) ajoutée(s)", "item(s) added"),
                self.diagnostics.filtered_count,
                tr(
                    self.language,
                    "candidat(s) masqué(s)",
                    "candidate(s) hidden"
                )
            ));
            if self.diagnostics.error_count > 0 {
                ui.small(format!(
                    "{} {}",
                    self.diagnostics.error_count,
                    tr(
                        self.language,
                        "erreur(s) dans le rapport",
                        "error(s) in report"
                    )
                ));
            }
            if self.selected_category != LAST_SCAN_REVIEW
                && pending > 0
                && ui
                    .small_button(format!(
                        "{} ({pending})",
                        tr(self.language, "Nouveaux à confirmer", "New candidates")
                    ))
                    .clicked()
            {
                self.open_last_scan_candidates();
                navigate = true;
            }
        });
        navigate
    }

    fn content(&mut self, ctx: &egui::Context) {
        let accent = self
            .active_style()
            .and_then(|style| style.accent)
            .unwrap_or(theme_accent(self.theme));
        let card = self
            .active_style()
            .and_then(|style| style.card)
            .unwrap_or(card_color(self.theme));
        let text = self
            .active_style()
            .and_then(|style| style.text)
            .unwrap_or(theme_text(self.theme));
        let muted = self
            .active_style()
            .and_then(|style| style.muted)
            .unwrap_or(theme_muted_text(self.theme));
        let radius = self
            .active_style()
            .and_then(|style| style.radius)
            .unwrap_or(match self.theme {
                Theme::Classic => 0,
                Theme::Prism => 20,
                _ => 12,
            });
        let gap = self
            .active_style()
            .and_then(|style| style.gap)
            .unwrap_or(12.0);
        let query = SearchQuery::new(&self.search);
        let mut filtered: Vec<Game> = self
            .games
            .iter()
            .filter(|game| {
                self.matches_category(game)
                    && self.filters.matches(game, &query, self.game_console(game))
            })
            .cloned()
            .collect();
        filtered.sort_by_cached_key(|game| {
            (
                self.filters.sort == Sort::Favorites && !game.favorite,
                game.title.to_lowercase(),
            )
        });
        if self.filters.sort == Sort::ReverseName {
            filtered.reverse();
        }
        let review_ids: HashSet<_> = filtered
            .iter()
            .filter(|game| !game.confirmed)
            .map(|game| &game.id)
            .collect();
        self.review_selection.retain(|id| review_ids.contains(id));
        if !filtered
            .iter()
            .any(|game| self.selected_game.as_ref() == Some(&game.id))
        {
            self.selected_game = filtered.first().map(|game| game.id.clone());
        }
        if !filtered.is_empty() {
            egui::SidePanel::right("game_details")
                .default_width(290.0)
                .width_range(250.0..=390.0)
                .frame(
                    egui::Frame::new()
                        .fill(
                            self.active_style()
                                .and_then(|style| style.surface)
                                .unwrap_or(card),
                        )
                        .inner_margin(18.0),
                )
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(tr(self.language, "FICHE DU JEU", "GAME DETAILS"))
                            .size(10.0)
                            .color(muted),
                    );
                    ui.add_space(14.0);
                    egui::ScrollArea::vertical()
                        .id_salt("details-scroll")
                        .show(ui, |ui| self.details_panel(ui, &filtered));
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            let title = match self.selected_category.as_str() {
                "Tous les jeux" => tr(self.language, "Ma collection", "My collection"),
                "Favoris" => tr(self.language, "Favoris", "Favorites"),
                "Installés" => tr(self.language, "Jeux disponibles", "Available games"),
                "À confirmer" => tr(self.language, "À confirmer", "To review"),
                LAST_SCAN_REVIEW => tr(self.language, "À confirmer · Dernier scan", "To review · Last scan"),
                other => other.strip_prefix("console:").unwrap_or(other),
            };
            ui.label(RichText::new(title).size(28.0).strong().color(text));
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    filtered.len(),
                    tr(self.language, "résultat(s)", "result(s)"),
                ))
                .size(12.0)
                .color(muted),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .id(egui::Id::new("library-search"))
                        .desired_width(210.0)
                        .hint_text(tr(
                            self.language,
                            "Titre, console, source, dossier…",
                            "Title, console, source, folder…",
                        )),
                );
                if ui
                    .button(tr(self.language, "＋ Ajouter", "＋ Add"))
                    .clicked()
                {
                    self.add_manual_game();
                }
                let scan_label = if self.scan_control.is_some() {
                    tr(self.language, "■ Arrêter", "■ Stop")
                } else {
                    tr(self.language, "Scanner", "Scan")
                };
                if ui.button(scan_label).clicked() {
                    if let Some(control) = &self.scan_control {
                        control.cancel();
                    } else {
                        self.show_options = true;
                    }
                }
            });
            self.filter_bar(ui);
            if self.scan_summary(ui) {
                ctx.request_repaint();
                return;
            }
            if matches!(self.selected_category.as_str(), "À confirmer" | LAST_SCAN_REVIEW) {
                ui.horizontal_wrapped(|ui| {
                    if self.selected_category == LAST_SCAN_REVIEW && ui.button(tr(self.language,
                        "Tous les candidats", "All candidates")).clicked() {
                        self.selected_category = "À confirmer".into();
                        self.review_selection.clear();
                    }
                    if ui.button(tr(self.language, "Sélectionner les résultats", "Select results")).clicked() {
                        self.review_selection = review_ids.iter().map(|id| (*id).clone()).collect();
                    }
                    if ui.button(tr(self.language, "Désélectionner", "Clear selection")).clicked() {
                        self.review_selection.clear();
                    }
                    let count = self.review_selection.len();
                    if ui.add_enabled(count > 0, egui::Button::new(format!("{} ({count})", tr(self.language, "Confirmer", "Confirm")))).clicked() {
                        self.apply_review_selection(ReviewAction::Confirm);
                    }
                    if ui.add_enabled(count > 0, egui::Button::new(format!("{} ({count})", tr(self.language, "Ignorer", "Ignore")))).clicked() {
                        self.apply_review_selection(ReviewAction::Ignore);
                    }
                });
                ui.small(tr(self.language, "Coche les éléments à traiter. Les éléments ignorés restent restaurables dans les options.", "Check items to process. Ignored items can be restored in settings."));
            }
            if let Some(console) = self
                .selected_category
                .strip_prefix("console:")
                .map(str::to_owned)
            {
                self.category_emulator_ui(ui, &console);
            }
            ui.add_space(8.0);
            ui.separator();
            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(55.0);
                    ui.label(RichText::new("▦").size(42.0).color(accent));
                    let has_games_in_view = self.games.iter().any(|game| self.matches_category(game));
                    let empty_title = if has_games_in_view {
                        tr(self.language, "Aucun résultat pour ces filtres", "No results with these filters")
                    } else if matches!(self.selected_category.as_str(), "À confirmer" | LAST_SCAN_REVIEW) {
                        tr(self.language, "Aucun candidat à confirmer", "No candidates to review")
                    } else {
                        tr(self.language, "Aucun jeu dans cette vue", "No games in this view")
                    };
                    ui.label(
                        RichText::new(empty_title)
                        .size(20.0)
                        .color(text),
                    );
                    if has_games_in_view && ui
                        .button(tr(
                            self.language,
                            "Réinitialiser les filtres",
                            "Reset filters",
                        ))
                        .clicked()
                    {
                        self.filters = Filters::default();
                        self.search.clear();
                    }
                    if !matches!(self.selected_category.as_str(), "À confirmer" | LAST_SCAN_REVIEW)
                        && self.games.iter().any(|game| !game.confirmed)
                        && ui
                            .button(tr(
                                self.language,
                                "Examiner les candidats",
                                "Review candidates",
                            ))
                            .clicked()
                    {
                        self.selected_category = "À confirmer".into();
                    }
                });
                return;
            }
            ui.scope(|ui| {
                let columns = ((ui.available_width() + gap) / (190.0 + gap))
                    .floor()
                    .max(1.0) as usize;
                let width = ((ui.available_width() - gap * (columns.saturating_sub(1)) as f32)
                    / columns as f32)
                    .clamp(150.0, 250.0);
                let card_height = 196.0;
                let rows = filtered.len().div_ceil(columns);
                egui::ScrollArea::vertical()
                    .id_salt("virtual-cards")
                    .show_rows(ui, card_height, rows, |ui, range| {
                        for row in range {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = gap;
                                for game in filtered.iter().skip(row * columns).take(columns) {
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(width, card_height),
                                        egui::Sense::click(),
                                    );
                                    if !ui.is_rect_visible(rect) {
                                        continue;
                                    }
                                    let selected = self.selected_game.as_deref() == Some(&game.id);
                                    let texture = self.load_cover(ctx, game);
                                    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                                    if self.theme == Theme::Prism {
                                        crate::glass::surface(&painter, rect, radius, card);
                                    } else {
                                        painter.rect_filled(rect, radius, card);
                                    }
                                    if selected || response.hovered() {
                                        painter.rect_stroke(
                                            rect,
                                            radius,
                                            egui::Stroke::new(
                                                if selected { 2.0_f32 } else { 1.0_f32 },
                                                accent,
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    let cover = egui::Rect::from_min_size(
                                        rect.min + Vec2::splat(8.0),
                                        Vec2::new(width - 16.0, 112.0),
                                    );
                                    painter.rect_filled(
                                        cover,
                                        radius.min(8),
                                        placeholder_color(self.theme),
                                    );
                                    if let Some(texture) = texture {
                                        let size = texture.size_vec2();
                                        let scale = (cover.width() / size.x).min(cover.height() / size.y);
                                        let image_rect = egui::Rect::from_center_size(cover.center(), size * scale);
                                        painter.image(
                                            texture.id(),
                                            image_rect,
                                            egui::Rect::from_min_max(
                                                egui::Pos2::ZERO,
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        let initials = game
                                            .title
                                            .chars()
                                            .take(2)
                                            .collect::<String>()
                                            .to_uppercase();
                                        painter.text(
                                            cover.center(),
                                            egui::Align2::CENTER_CENTER,
                                            initials,
                                            egui::FontId::proportional(32.0),
                                            accent,
                                        );
                                    }
                                    let title = truncate_title(&game.title, (width / 7.5) as usize);
                                    painter.text(
                                        rect.min + Vec2::new(12.0, 132.0),
                                        egui::Align2::LEFT_TOP,
                                        title,
                                        egui::FontId::proportional(15.0),
                                        text,
                                    );
                                    let category =
                                        self.game_console(game).unwrap_or(if game.confirmed {
                                            &game.category
                                        } else {
                                            tr(self.language, "À confirmer", "To review")
                                        });
                                    painter.text(
                                        rect.min + Vec2::new(12.0, 158.0),
                                        egui::Align2::LEFT_TOP,
                                        truncate_title(category, (width / 7.0) as usize),
                                        egui::FontId::proportional(11.0),
                                        muted,
                                    );
                                    if game.favorite {
                                        painter.text(
                                            rect.right_top() + Vec2::new(-14.0, 14.0),
                                            egui::Align2::RIGHT_TOP,
                                            "★",
                                            egui::FontId::proportional(18.0),
                                            accent,
                                        );
                                    }
                                    let available = game
                                        .installations
                                        .iter()
                                        .any(|installation| installation.available);
                                    painter.circle_filled(
                                        rect.left_bottom() + Vec2::new(14.0, -12.0),
                                        3.0,
                                        if available {
                                            Color32::from_rgb(113, 186, 142)
                                        } else {
                                            muted
                                        },
                                    );
                                    response.clone().on_hover_text(&game.title);
                                    if response.clicked() {
                                        self.selected_game = Some(game.id.clone());
                                    }
                                    if !game.confirmed {
                                        let mut checked = self.review_selection.contains(&game.id);
                                        let check_rect = egui::Rect::from_min_size(rect.min + Vec2::splat(12.0), Vec2::splat(24.0));
                                        let mut check_ui = ui.new_child(egui::UiBuilder::new()
                                            .id_salt(("review-select", &game.id)).max_rect(check_rect)
                                            .layout(egui::Layout::top_down(egui::Align::Min)));
                                        if check_ui.add(egui::Checkbox::without_text(&mut checked)).changed() {
                                            if checked { self.review_selection.insert(game.id.clone()); }
                                            else { self.review_selection.remove(&game.id); }
                                        }
                                    }
                                }
                            });
                        }
                    });
            });
        });
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("filter-kind")
                .width(95.0)
                .selected_text(match self.filters.kind {
                    GameKind::All => tr(self.language, "PC + ROMs", "PC + ROMs"),
                    GameKind::Pc => "PC",
                    GameKind::Rom => "ROMs",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filters.kind, GameKind::All, "PC + ROMs");
                    ui.selectable_value(&mut self.filters.kind, GameKind::Pc, "PC");
                    ui.selectable_value(&mut self.filters.kind, GameKind::Rom, "ROMs");
                });
            egui::ComboBox::from_id_salt("filter-source")
                .width(100.0)
                .selected_text(
                    self.filters
                        .source
                        .as_ref()
                        .map(|source| launcher_label(source, self.language))
                        .unwrap_or(tr(self.language, "Toutes sources", "All sources")),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.filters.source,
                        None,
                        tr(self.language, "Toutes sources", "All sources"),
                    );
                    for source in [
                        Launcher::Steam,
                        Launcher::Epic,
                        Launcher::Heroic,
                        Launcher::Lutris,
                        Launcher::Gog,
                        Launcher::Manual,
                        Launcher::Unknown,
                    ] {
                        let label = launcher_label(&source, self.language);
                        ui.selectable_value(&mut self.filters.source, Some(source), label);
                    }
                });
            egui::ComboBox::from_id_salt("filter-available")
                .width(105.0)
                .selected_text(match self.filters.available {
                    None => tr(self.language, "Tous les états", "All states"),
                    Some(true) => tr(self.language, "Disponibles", "Available"),
                    Some(false) => tr(self.language, "Indisponibles", "Unavailable"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.filters.available,
                        None,
                        tr(self.language, "Tous les états", "All states"),
                    );
                    ui.selectable_value(
                        &mut self.filters.available,
                        Some(true),
                        tr(self.language, "Disponibles", "Available"),
                    );
                    ui.selectable_value(
                        &mut self.filters.available,
                        Some(false),
                        tr(self.language, "Indisponibles", "Unavailable"),
                    );
                });
            egui::ComboBox::from_id_salt("filter-sort")
                .width(85.0)
                .selected_text(match self.filters.sort {
                    Sort::Name => "A → Z",
                    Sort::ReverseName => "Z → A",
                    Sort::Favorites => "★",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filters.sort, Sort::Name, "A → Z");
                    ui.selectable_value(&mut self.filters.sort, Sort::ReverseName, "Z → A");
                    ui.selectable_value(
                        &mut self.filters.sort,
                        Sort::Favorites,
                        tr(self.language, "Favoris d’abord", "Favorites first"),
                    );
                });
            ui.checkbox(&mut self.filters.favorites_only, "★");
            if ui
                .small_button(tr(self.language, "Effacer", "Clear"))
                .clicked()
            {
                self.filters = Filters::default();
                self.search.clear();
            }
        });
    }
    fn matches_category(&self, game: &Game) -> bool {
        if self.selected_category == LAST_SCAN_REVIEW {
            return !game.confirmed && self.diagnostics.added_game_ids.contains(&game.id);
        }
        // Discovery candidates belong exclusively to the review queue, including
        // old extension-only ROM detections already stored in the library.
        if !game.confirmed {
            return self.selected_category == "À confirmer";
        }
        let source = match self.selected_category.as_str() {
            "Steam" => Some(Launcher::Steam),
            "Epic Games" => Some(Launcher::Epic),
            "Heroic" => Some(Launcher::Heroic),
            "Lutris" => Some(Launcher::Lutris),
            "GOG" => Some(Launcher::Gog),
            "Ajout manuel" => Some(Launcher::Manual),
            _ => None,
        };
        if let Some(source) = source {
            return game
                .installations
                .iter()
                .any(|installation| installation.launcher == source);
        }
        if let Some(console) = self.selected_category.strip_prefix("console:") {
            return self.game_console(game) == Some(console);
        }
        match self.selected_category.as_str() {
            "ROMs" => self.game_console(game).is_some(),
            "Tous les jeux" => game.confirmed,
            "Favoris" => game.favorite,
            "Installés" => {
                game.confirmed
                    && game
                        .installations
                        .iter()
                        .any(|installation| installation.available)
            }
            "À confirmer" => !game.confirmed,
            category => game.category.eq_ignore_ascii_case(category),
        }
    }

    fn foreground_color(&self) -> Color32 {
        self.active_style()
            .and_then(|style| style.text)
            .unwrap_or(theme_text(self.theme))
    }

    fn details_panel(&mut self, ui: &mut egui::Ui, games: &[Game]) {
        let Some(id) = self.selected_game.clone() else {
            ui.label(tr(
                self.language,
                "Sélectionnez un jeu pour afficher ses détails.",
                "Select a game to view its details.",
            ));
            return;
        };
        let Some(game) = games.iter().find(|game| game.id == id).cloned() else {
            ui.label(tr(
                self.language,
                "Sélectionne un jeu pour afficher ses détails.",
                "Select a game to view its details.",
            ));
            return;
        };
        ui.label(
            RichText::new(&game.title)
                .size(21.0)
                .strong()
                .color(self.foreground_color()),
        );
        let category = if let Some(console) = self.game_console(&game) {
            console
        } else if game.category == "À confirmer" {
            tr(self.language, "À confirmer", "To review")
        } else {
            &game.category
        };
        let muted = self
            .active_style()
            .and_then(|style| style.muted)
            .unwrap_or(theme_muted_text(self.theme));
        ui.label(RichText::new(category).color(muted));
        ui.add_space(10.0);
        ui.label(
            RichText::new(tr(self.language, "INSTALLATIONS", "INSTALLATIONS"))
                .size(11.0)
                .color(muted),
        );
        let mut launch_profile: Option<LaunchProfile> = None;
        let mut launch_uri: Option<String> = None;
        let mut launch_image: Option<(PathBuf, PathBuf)> = None;
        for installation in &game.installations {
            let detected_path = installation
                .launch_profile
                .as_ref()
                .map(|profile| &profile.command)
                .filter(|command| !game.confirmed && command.is_absolute())
                .unwrap_or(&installation.path);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                let source = if installation.launcher == Launcher::Unknown {
                    tr(self.language, "Détecté sur le disque", "Found on disk")
                } else {
                    launcher_label(&installation.launcher, self.language)
                };
                ui.strong(source);
                ui.label(if installation.available {
                    tr(self.language, "Disponible", "Available")
                } else {
                    tr(self.language, "Indisponible", "Unavailable")
                });
            });
            ui.add(
                egui::Label::new(detected_path.display().to_string())
                    .wrap()
                    .selectable(true),
            );
            if crate::rom::console(&installation.path).is_some() && installation.available {
                if ui
                    .add_enabled(
                        self.category_emulator(&game)
                            .is_some_and(|emulator| emulator.executable.is_file()),
                        egui::Button::new(tr(
                            self.language,
                            "▶  Lancer avec l’émulateur",
                            "▶  Launch in emulator",
                        )),
                    )
                    .on_disabled_hover_text(tr(
                        self.language,
                        "Classe l’image, puis associe un émulateur installé à sa catégorie.",
                        "Classify the image, then assign an installed emulator to its category.",
                    ))
                    .clicked()
                {
                    if let Some(emulator) = self.category_emulator(&game) {
                        launch_image =
                            Some((emulator.executable.clone(), installation.path.clone()));
                    } else {
                        self.status = tr(
                            self.language,
                            "Choisis un émulateur dans la catégorie de cette image.",
                            "Choose an emulator in this image category.",
                        )
                        .into();
                    }
                }
            } else if installation.launch_profile.is_some()
                && installation.available
                && ui
                    .button(tr(self.language, "▶  Jouer", "▶  Play"))
                    .clicked()
            {
                if let Some(profile) = &installation.launch_profile {
                    if let Some(uri) = &profile.launcher_command {
                        launch_uri = Some(uri.clone());
                    } else {
                        launch_profile = Some(profile.clone());
                    }
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        installation.available,
                        egui::Button::new(tr(self.language, "Ouvrir le dossier", "Open folder")),
                    )
                    .clicked()
                {
                    let folder = if installation.path.is_file() {
                        installation.path.parent()
                    } else {
                        Some(installation.path.as_path())
                    };
                    if let Some(folder) = folder.filter(|path| path.is_absolute() && path.is_dir())
                    {
                        if let Err(error) = open::that_detached(folder) {
                            self.status = error.to_string();
                        }
                    } else {
                        self.status =
                            tr(self.language, "Dossier indisponible", "Folder unavailable").into();
                    }
                }
                if ui
                    .button(tr(self.language, "Copier le chemin", "Copy path"))
                    .clicked()
                {
                    ui.ctx().copy_text(detected_path.display().to_string());
                    self.status = tr(self.language, "Chemin copié.", "Path copied.").into();
                }
            });
        }
        if let Some(uri) = launch_uri.filter(|uri| !uri.is_empty()) {
            if let Err(error) = launch_uri_handler(&uri) {
                self.status = error.to_string();
            }
        }
        if let Some(profile) = launch_profile {
            if let Err(error) = launch_profile_command(&profile) {
                self.status = error.to_string();
            }
        }
        if let Some((emulator, image)) = launch_image {
            if let Err(error) = launch_emulator(&emulator, &image) {
                self.status = error.to_string();
            }
        }
        ui.add_space(10.0);
        if !game.confirmed
            && ui
                .button(tr(
                    self.language,
                    "✓ Confirmer ce jeu",
                    "✓ Confirm this game",
                ))
                .clicked()
        {
            self.update_library(|store| store.set_confirmed(&game.id, true));
        }
        if ui
            .button(tr(
                self.language,
                "Ce n’est pas un jeu · Ignorer",
                "Not a game · Ignore",
            ))
            .clicked()
        {
            if self.persist(|store| store.ignore_game(&game.id)) {
                self.games.retain(|item| item.id != game.id);
                self.selected_game = None;
            }
            return;
        }
        if ui
            .button(if game.favorite {
                tr(
                    self.language,
                    "★  Retirer des favoris",
                    "★  Remove favorite",
                )
            } else {
                tr(
                    self.language,
                    "☆  Ajouter aux favoris",
                    "☆  Add to favorites",
                )
            })
            .clicked()
        {
            self.update_library(|store| store.set_favorite(&game.id, !game.favorite));
        }
        let is_disc_game = self.game_console(&game).is_some();
        if is_disc_game {
            let current_console = self.console_overrides.get(&game.id).cloned();
            let mut chosen_console = current_console.clone();
            egui::ComboBox::from_id_salt(("rom-console", &game.id))
                .selected_text(self.game_console(&game).unwrap_or(crate::rom::UNCLASSIFIED))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut chosen_console,
                        None,
                        tr(self.language, "Détection automatique", "Auto detect"),
                    );
                    for console in crate::rom::CONSOLES {
                        ui.selectable_value(
                            &mut chosen_console,
                            Some((*console).to_owned()),
                            *console,
                        );
                    }
                });
            if chosen_console != current_console {
                let mut mapping = self.console_overrides.clone();
                if let Some(console) = chosen_console {
                    mapping.insert(game.id.clone(), console);
                } else {
                    mapping.remove(&game.id);
                }
                if self.save_mapping("console_overrides", &mapping) {
                    self.console_overrides = mapping;
                }
            }
            let console = self
                .game_console(&game)
                .unwrap_or(crate::rom::UNCLASSIFIED)
                .to_owned();
            if crate::rom::assignable_console(&console) {
                let emulator_name = self
                    .category_emulator(&game)
                    .map(|emulator| emulator.name.as_str())
                    .unwrap_or(tr(
                        self.language,
                        "Aucun émulateur associé",
                        "No emulator assigned",
                    ));
                ui.label(format!("{console} · {emulator_name}"));
                if ui
                    .button(tr(
                        self.language,
                        "Configurer cette catégorie",
                        "Configure category",
                    ))
                    .clicked()
                {
                    self.selected_category = format!("console:{console}");
                }
            } else {
                ui.label(tr(
                    self.language,
                    "Choisis d’abord la catégorie de cette image.",
                    "Choose this image's category first.",
                ));
            }
        }
        ui.collapsing(tr(self.language, "Modifier le nom", "Edit name"), |ui| {
            if self.title_edit_id.as_deref() != Some(&game.id) {
                self.title_edit = game.title.clone();
                self.title_edit_id = Some(game.id.clone());
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.title_edit)
                    .desired_width(ui.available_width()),
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.title_edit.trim().is_empty(),
                        egui::Button::new(tr(self.language, "Enregistrer le nom", "Save name")),
                    )
                    .clicked()
                {
                    let title = self.title_edit.trim().to_owned();
                    if self.update_library(|store| store.set_title(&game.id, &title)) {
                        self.title_edit = title;
                    }
                }
                if ui
                    .button(tr(
                        self.language,
                        "Rétablir le nom détecté",
                        "Restore detected name",
                    ))
                    .clicked()
                    && self.update_library(|store| store.reset_title(&game.id))
                {
                    self.title_edit_id = None;
                }
            });
            ui.small(tr(
                self.language,
                "Ce nom reste conservé après les scans. Les fichiers ne sont pas renommés.",
                "This name survives scans. Files are not renamed.",
            ));
        });
        ui.horizontal(|ui| {
            ui.label(tr(self.language, "Catégorie", "Category"));
            if self.category_edit_id.as_deref() != Some(&game.id) {
                self.category_edit = game.category.clone();
                self.category_edit_id = Some(game.id.clone());
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.category_edit).desired_width(120.0));
            if ui
                .button(tr(self.language, "Enregistrer", "Save"))
                .clicked()
                && !self.category_edit.trim().is_empty()
            {
                let category = self.category_edit.trim().to_owned();
                self.update_library(|store| store.set_category(&game.id, &category));
            }
        });
        if ui
            .button(tr(
                self.language,
                "Choisir une vignette locale…",
                "Choose a local cover…",
            ))
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "ico"])
                .pick_file()
            {
                if let Err(error) = read_cover(&path) {
                    self.status = format!(
                        "{} : {error}",
                        tr(self.language, "Vignette illisible", "Unreadable cover")
                    );
                    return;
                }
                if self.update_library(|store| store.set_cover(&game.id, Some(&path))) {
                    self.cover_textures.remove(&game.id);
                }
            }
        }
    }

    fn refresh_covers(&mut self) {
        self.cover_textures.clear();
        if self.cover_refresh.is_some() {
            return;
        }
        if self.store.is_none() {
            self.persist(|_| Ok(()));
            return;
        }
        self.cover_refresh = Some(crate::covers::Refresh::start(
            self.games
                .iter()
                .filter(|game| game.cover_path.is_none())
                .cloned()
                .collect(),
            sources::steam_cache_roots(),
        ));
        self.status = tr(
            self.language,
            "Recherche des jaquettes locales…",
            "Searching local covers…",
        )
        .into();
    }

    fn poll_covers(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.cover_refresh else {
            return;
        };
        let mut finished = false;
        for _ in 0..8 {
            match job.receiver.try_recv() {
                Ok(crate::covers::RefreshEvent::Found(id, path)) => {
                    let Some(store) = &self.store else {
                        job.errors += 1;
                        job.last_error = Some(
                            tr(
                                self.language,
                                "Base locale indisponible",
                                "Local database unavailable",
                            )
                            .into(),
                        );
                        continue;
                    };
                    match store.set_missing_cover(&id, &path) {
                        Ok(true) => {
                            job.saved += 1;
                            if let Some(game) = self.games.iter_mut().find(|game| game.id == id) {
                                game.cover_path = Some(path);
                            }
                            self.cover_textures.remove(&id);
                        }
                        Ok(false) => {}
                        Err(error) => {
                            job.errors += 1;
                            self.status = error.to_string();
                            job.last_error = Some(error.to_string());
                        }
                    }
                }
                Ok(crate::covers::RefreshEvent::Finished) => {
                    finished = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    job.errors += 1;
                    job.last_error = Some(
                        tr(
                            self.language,
                            "La recherche de jaquettes s’est arrêtée sans terminer",
                            "Cover search stopped before completing",
                        )
                        .into(),
                    );
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            self.status = format!(
                "{} {} · {} {}",
                job.saved,
                tr(self.language, "jaquette(s) ajoutée(s)", "cover(s) added"),
                job.errors,
                tr(self.language, "erreur(s)", "error(s)")
            );
            if let Some(error) = &job.last_error {
                self.status.push_str(&format!(" · {error}"));
            }
            self.cover_refresh = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn load_cover(&mut self, ctx: &egui::Context, game: &Game) -> Option<TextureHandle> {
        let path = game.cover_path.as_ref()?;
        if let Some((cached_path, texture)) = self.cover_textures.get(&game.id) {
            if cached_path == path {
                return texture.clone();
            }
        }
        let texture = read_cover(path)
            .ok()
            .map(|color| ctx.load_texture(game.id.clone(), color, egui::TextureOptions::LINEAR));
        if self.cover_textures.len() >= 128 {
            if let Some(id) = self.cover_textures.keys().next().cloned() {
                self.cover_textures.remove(&id);
            }
        }
        // Cache failures too: a missing drive or corrupt image must not cause I/O every frame.
        self.cover_textures
            .insert(game.id.clone(), (path.clone(), texture.clone()));
        texture
    }

    fn options_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_options;
        let mut close = false;
        let min_width = (ctx.screen_rect().width() - 48.0).clamp(280.0, 680.0);
        egui::Window::new(tr(self.language, "Options", "Settings"))
            .id(egui::Id::new("options-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .min_width(min_width)
            .default_width(760.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(tr(self.language, "Apparence et langue", "Appearance and language"));
                if ui.add_enabled(self.cover_refresh.is_none(), egui::Button::new(tr(self.language, "Recharger les vignettes locales", "Reload local covers"))).clicked() {
                    self.refresh_covers();
                }
                if self.cover_refresh.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tr(self.language, "Recherche locale des jaquettes manquantes…", "Searching locally for missing covers…"));
                        if ui.button(tr(self.language, "Annuler", "Cancel")).clicked() {
                            self.cover_refresh = None;
                            self.status = tr(self.language, "Recherche de jaquettes interrompue.", "Cover search cancelled.").into();
                        }
                    });
                }
                let previous_language = self.language;
                egui::ComboBox::from_label(tr(self.language, "Langue", "Language"))
                    .selected_text(match self.language { Language::French => "Français", Language::English => "English" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.language, Language::French, "Français");
                        ui.selectable_value(&mut self.language, Language::English, "English");
                    });
                if previous_language != self.language {
                    let key = self.language.key();
                    if !self.persist(|store| store.set_preference("language", key)) {
                        self.language = previous_language;
                    }
                }
                let previous_theme = self.theme;
                egui::ComboBox::from_label(tr(self.language, "Thème", "Theme"))
                    .height(280.0)
                    .selected_text(self.theme.label(self.language))
                    .show_ui(ui, |ui| {
                        for theme in [
                            Theme::Dark,
                            Theme::Light,
                            Theme::Breeze,
                            Theme::Horizon,
                            Theme::Classic,
                            Theme::Prism,
                        ] {
                            ui.selectable_value(&mut self.theme, theme, theme.label(self.language));
                        }
                    });
                if previous_theme != self.theme {
                    let key = self.theme.key();
                    if !self.persist(|store| store.set_preference("theme", key)) {
                        self.theme = previous_theme;
                    }
                    self.refresh_appearance(ctx);
                }
                ui.separator();
                ui.heading(tr(self.language, "Style CSS personnalisé", "Custom CSS style"));
                ui.label(tr(self.language, "Variables de couleurs, arrondis et espacement. Fichier local, rechargement manuel.", "Color, radius and spacing variables. Local file, manual reload."));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr(self.language, "Importer un fichier CSS…", "Import CSS file…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("CSS", &["css"]).pick_file() {
                            self.load_css(ctx, path);
                        }
                    }
                    if ui.add_enabled(self.css_path.is_some(), egui::Button::new(tr(self.language, "Recharger", "Reload"))).clicked() {
                        if let Some(path) = self.css_path.clone() { self.load_css(ctx, path); }
                    }
                    if ui.button(tr(self.language, "Enregistrer le modèle…", "Save template…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_file_name("rustyshelf.css").add_filter("CSS", &["css"]).save_file() {
                            match std::fs::write(&path, crate::style::EXAMPLE) {
                                Ok(()) => self.load_css(ctx, path),
                                Err(error) => self.css_error = Some(error.to_string()),
                            }
                        }
                    }
                });
                let mut css_enabled = self.css_enabled;
                if ui.checkbox(&mut css_enabled, tr(self.language, "Activer le style CSS", "Enable CSS style")).changed() {
                    self.set_css_enabled(ctx, css_enabled);
                }
                if let Some(path) = &self.css_path { ui.label(path.display().to_string()); }
                if let Some(error) = &self.css_error { ui.colored_label(Color32::from_rgb(225, 92, 85), error); }
                ui.small(tr(self.language, "Syntaxe : :root { --accent: #e5a270; --radius: 12px; } · CSS complet non pris en charge.", "Syntax: :root { --accent: #e5a270; --radius: 12px; } · Full CSS is not supported."));
                ui.separator();
                ui.heading(tr(self.language, "Émulation", "Emulation"));
                ui.label(tr(self.language, "Les associations se règlent dans chaque catégorie de ROMs ou d’images. Ajouter un émulateur ne l’associe à aucun jeu automatiquement.", "Assignments are configured inside each ROM or image category. Adding an emulator does not assign it automatically."));
                if ui.button(tr(self.language, "Ajouter un émulateur disponible…", "Add available emulator…")).clicked() { self.add_emulator(); }
                if self.emulators.is_empty() {
                    ui.label(tr(self.language, "Aucun émulateur détecté. Ajoute son exécutable ci-dessus.", "No emulator found. Add its executable above."));
                } else {
                    for emulator in &self.emulators { ui.label(format!("{} · {}", emulator.name, emulator.executable.display())); }
                }
                ui.separator();
                ui.heading(tr(self.language, "Scanner", "Scanner"));
                if ui.add_enabled(self.scan_control.is_none(), egui::Button::new(tr(self.language, "Scanner un dossier…", "Scan a folder…"))).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.begin_scan(vec![path], false);
                    }
                }
                ui.small(tr(self.language, "Le scan d’un dossier analyse seulement son contenu, sans parcourir les autres disques.", "Folder scans only examine that folder, without traversing other drives."));
                if ui.add_enabled(self.scan_control.is_none(), egui::Button::new(tr(self.language, "Actualiser les disques", "Refresh drives"))).clicked() {
                    self.volumes = scan::mounted_volumes();
                    self.selected_volumes = scan::volume_selection(&self.volumes, self.saved_scan_volumes.as_deref());
                }
                if ui.button(tr(self.language, "Gérer les éléments ignorés…", "Manage ignored items…")).clicked() { self.open_ignored_games(); }
                ui.label(tr(self.language, "Choisis les disques montés à analyser. Les jeux et images disque trouvés restent locaux.", "Choose mounted drives to scan. Games and disc images stay on this device."));
                ui.small(tr(self.language, "La sélection est mémorisée, y compris pour les disques temporairement débranchés.", "Selection is remembered, including temporarily disconnected drives."));
                let previous_selection = self.selected_volumes.clone();
                egui::ScrollArea::vertical()
                    .id_salt("scan-volumes")
                    .auto_shrink([false, true])
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (volume, selected) in self.volumes.iter().zip(&mut self.selected_volumes) {
                            ui.add(egui::Checkbox::new(
                                selected,
                                format!(
                                    "{}\n{}  ·  {}{}",
                                    volume.root.display(),
                                    volume.name,
                                    volume.filesystem,
                                    if volume.removable {
                                        tr(self.language, "  · amovible", "  · removable")
                                    } else {
                                        ""
                                    }
                                ),
                            )).on_hover_text(volume.root.display().to_string());
                        }
                    });
                if self.selected_volumes != previous_selection && !self.save_scan_volumes() {
                    self.selected_volumes = previous_selection;
                }
                ui.horizontal(|ui| {
                    if ui.button(tr(self.language, "＋ Exclure un dossier…", "＋ Exclude folder…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            if self.persist(|store| store.add_exclusion(&path)) && !self.exclusions.contains(&path) {
                                self.exclusions.push(path);
                            }
                        }
                    }
                    if ui
                        .button(if self.scan_control.is_some() {
                            tr(self.language, "Arrêter le scan", "Stop scan")
                        } else {
                            tr(self.language, "Démarrer le scan", "Start scan")
                        })
                        .clicked()
                    {
                        if self.scan_control.is_some() {
                            if let Some(control) = &self.scan_control {
                                control.cancel();
                            }
                        } else {
                            self.start_scan();
                        }
                    }
                    if ui.button(tr(self.language, "Fermer", "Close")).clicked() {
                        close = true;
                    }
                });
                if !self.exclusions.is_empty() {
                    ui.separator();
                    ui.label(tr(self.language, "Dossiers exclus", "Excluded folders"));
                    let mut remove = None;
                    for (index, path) in self.exclusions.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(path.display().to_string());
                            if ui.small_button(tr(self.language, "Retirer", "Remove")).clicked() {
                                remove = Some(index);
                            }
                        });
                    }
                    if let Some(index) = remove {
                        let path = self.exclusions[index].clone();
                        if self.persist(|store| store.remove_exclusion(&path)) {
                            self.exclusions.remove(index);
                        }
                    }
                }
                if self.scan_control.is_some() {
                    ui.label(format!("{} {} · {}", self.scan_visited, tr(self.language, "éléments parcourus", "items scanned"), self.scan_current));
                }
                ui.separator();
                ui.heading(tr(self.language, "Rapport du dernier scan", "Last scan report"));
                let state = match self.diagnostics.cancelled {
                    Some(true) => tr(self.language, "Interrompu", "Interrupted"),
                    Some(false) => tr(self.language, "Terminé", "Complete"),
                    None if self.scan_control.is_some() => tr(self.language, "En cours", "In progress"),
                    None => tr(self.language, "État final non enregistré", "Final state not recorded"),
                };
                ui.label(format!("{state} · {} {}", self.diagnostics.visited, tr(self.language, "éléments parcourus", "items scanned")));
                if !self.diagnostics.roots.is_empty() {
                    ui.collapsing(tr(self.language, "Disques et dossiers choisis", "Selected drives and folders"), |ui| {
                        for root in &self.diagnostics.roots { ui.label(root.display().to_string()); }
                    });
                }
                ui.label(format!("{} {} · {} {}", self.diagnostics.error_count, tr(self.language, "erreur(s)", "error(s)"), self.diagnostics.skipped_count, tr(self.language, "chemin(s) non analysé(s)", "skipped path(s)")));
                if self.diagnostics.filtered_count > 0 {
                    ui.label(format!("{} {}", self.diagnostics.filtered_count, tr(self.language,
                        "ancien(s) candidat(s) masqué(s) · restaurables dans les éléments ignorés",
                        "old candidate(s) hidden · recoverable from ignored items")));
                }
                if self.diagnostics.skipped_count > 0 {
                    ui.collapsing(tr(self.language, "Dossiers protégés ou indisponibles", "Protected or unavailable folders"), |ui| {
                        ui.label(tr(self.language, "Ces chemins n’ont pas pu être analysés. Les jeux déjà enregistrés sont conservés.", "These paths could not be scanned. Existing games are preserved."));
                        for message in &self.diagnostics.skipped { ui.label(message); }
                    });
                }
                if self.diagnostics.error_count > 0 {
                    ui.collapsing(tr(self.language, "Détails des erreurs", "Error details"), |ui| {
                        for error in &self.diagnostics.errors { ui.colored_label(Color32::from_rgb(220, 85, 75), error); }
                    });
                }
                if ui.button(tr(self.language, "Exporter le rapport…", "Export report…")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("rustyshelf-scan.txt").save_file() {
                        self.status = match std::fs::write(&path, self.diagnostics.report()) {
                            Ok(()) => format!("{} : {}", tr(self.language, "Rapport enregistré", "Report saved"), path.display()),
                            Err(error) => error.to_string(),
                        };
                    }
                }
                ui.separator();                ui.heading(tr(self.language, "Données et sauvegarde", "Data and backup"));
                ui.label(tr(self.language, "La sauvegarde conserve la bibliothèque et les réglages. L’export JSON permet de consulter le catalogue dans un autre outil.", "Backup includes your library and settings. JSON export lets you read your catalog in another tool."));
                if ui.add_enabled(self.scan_control.is_none() && self.store.is_some(), egui::Button::new(tr(self.language, "Sauvegarder la bibliothèque…", "Back up library…"))).clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("rustyshelf-backup.sqlite3").add_filter("SQLite", &["sqlite3"]).save_file() {
                        if let Some(store) = &self.store {
                            self.status = match store.backup_to(&path) {
                                Ok(()) => format!("{} : {}", tr(self.language, "Sauvegarde créée", "Backup created"), path.display()),
                                Err(error) => error,
                            };
                        }
                    }
                }
                if ui.add_enabled(self.scan_control.is_none(), egui::Button::new(tr(self.language, "Exporter le catalogue JSON…", "Export catalog JSON…"))).clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_file_name("rustyshelf-catalogue.json").add_filter("JSON", &["json"]).save_file() {
                        let document = serde_json::json!({
                            "format": "rustyshelf-catalogue", "version": 1,
                            "games": self.games, "console_overrides": self.console_overrides,
                            "console_emulators": self.console_emulators,
                        });
                        self.status = match serde_json::to_vec_pretty(&document).map_err(|error| error.to_string()).and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string())) {
                            Ok(()) => format!("{} : {}", tr(self.language, "Catalogue exporté", "Catalog exported"), path.display()),
                            Err(error) => error,
                        };
                    }
                }
            });
        if close || !open {
            self.show_options = false;
        }
    }
}

fn read_cover(path: &std::path::Path) -> Result<egui::ColorImage, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Vignette trop volumineuse (32 Mo maximum)".into());
    }
    let mut reader = image::ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| error.to_string())?
        .thumbnail(500, 300)
        .to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

fn launch_profile_command(profile: &LaunchProfile) -> std::io::Result<()> {
    let mut command = profile_process(profile);
    command.spawn().map(|_| ())
}

fn profile_process(profile: &LaunchProfile) -> Command {
    let mut command = Command::new(&profile.command);
    command.args(&profile.arguments);
    if let Some(directory) = &profile.working_directory {
        command.current_dir(directory);
    } else if let Some(parent) = profile
        .command
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        command.current_dir(parent);
    }
    command
}

fn launch_emulator(executable: &std::path::Path, image: &std::path::Path) -> std::io::Result<()> {
    let mut command = emulator_process(executable, image);
    command.spawn().map(|_| ())
}

fn emulator_process(executable: &std::path::Path, image: &std::path::Path) -> Command {
    let mut command = Command::new(executable);
    command.arg(image);
    if let Some(parent) = image.parent() {
        command.current_dir(parent);
    }
    command
}

fn card_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(37, 45, 57),
        Theme::Light => Color32::from_rgb(249, 250, 253),
        Theme::Breeze => Color32::from_rgb(229, 241, 251),
        Theme::Horizon => Color32::from_rgb(233, 241, 253),
        Theme::Classic => Color32::from_rgb(212, 208, 200),
        Theme::Prism => Color32::from_rgba_unmultiplied(35, 43, 56, 88),
    }
}

fn theme_accent(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(229, 162, 112),
        Theme::Light => Color32::from_rgb(163, 95, 64),
        Theme::Breeze => Color32::from_rgb(63, 134, 171),
        Theme::Horizon => Color32::from_rgb(49, 106, 197),
        Theme::Classic => Color32::from_rgb(0, 0, 128),
        Theme::Prism => Color32::from_rgb(174, 218, 255),
    }
}

fn placeholder_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(51, 47, 77),
        Theme::Light => Color32::from_rgb(220, 216, 239),
        Theme::Breeze => Color32::from_rgb(170, 204, 229),
        Theme::Horizon => Color32::from_rgb(190, 218, 250),
        Theme::Classic => Color32::from_rgb(192, 192, 192),
        Theme::Prism => Color32::from_rgba_unmultiplied(178, 205, 235, 24),
    }
}

fn theme_text(theme: Theme) -> Color32 {
    if matches!(
        theme,
        Theme::Light | Theme::Breeze | Theme::Horizon | Theme::Classic
    ) {
        Color32::from_rgb(35, 43, 55)
    } else {
        Color32::WHITE
    }
}

fn theme_muted_text(theme: Theme) -> Color32 {
    if matches!(
        theme,
        Theme::Light | Theme::Breeze | Theme::Horizon | Theme::Classic
    ) {
        Color32::from_rgb(90, 101, 116)
    } else {
        Color32::from_rgb(190, 196, 209)
    }
}

fn truncate_title(title: &str, max_chars: usize) -> String {
    let mut chars = title.chars();
    let result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!(
            "{}…",
            result
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        result
    }
}

fn merge_game_in_memory(games: &mut Vec<Game>, incoming: Game) {
    let target = games.iter_mut().find(|current| {
        current.id == incoming.id
            || current.installations.iter().any(|existing| {
                incoming
                    .installations
                    .iter()
                    .any(|new| same_path(&existing.path, &new.path))
            })
    });
    if let Some(current) = target {
        if incoming.confirmed && !current.confirmed {
            current.id = incoming.id.clone();
            current.title.clone_from(&incoming.title);
            current.category.clone_from(&incoming.category);
        }
        current.confirmed |= incoming.confirmed;
        current.favorite |= incoming.favorite;
        if current.cover_path.is_none() {
            current.cover_path = incoming.cover_path;
        }
        for installation in incoming.installations {
            if let Some(existing) = current
                .installations
                .iter_mut()
                .find(|item| same_path(&item.path, &installation.path))
            {
                if existing.launcher == Launcher::Unknown
                    && installation.launcher != Launcher::Unknown
                {
                    existing.launcher = installation.launcher;
                    existing.launch_profile = installation.launch_profile;
                } else if existing.launch_profile.is_none() {
                    existing.launch_profile = installation.launch_profile;
                }
                existing.available |= installation.available;
            } else {
                current.installations.push(installation);
            }
        }
    } else {
        games.push(incoming);
    }
}

fn deduplicate_library(mut games: Vec<Game>) -> Vec<Game> {
    games.sort_by_key(|game| (!game.confirmed, game.title.len()));
    let mut result = Vec::new();
    for game in games {
        merge_game_in_memory(&mut result, game);
    }
    result.sort_by_key(|game| game.title.to_lowercase());
    result
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .replace('\\', "/")
            .eq_ignore_ascii_case(&right.to_string_lossy().replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn launch_uri_handler(uri: &str) -> std::io::Result<()> {
    // Let the desktop resolve native/Flatpak/AppImage registrations. A launcher
    // may stay open for the entire game, so never wait for its lifetime here.
    open::that_detached(uri)
}

impl Drop for RustyShelfApp {
    fn drop(&mut self) {
        if let Some(control) = &self.scan_control {
            control.cancel();
        }
    }
}

impl eframe::App for RustyShelfApp {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.theme == Theme::Prism {
            [0.0; 4]
        } else {
            visuals.panel_fill.to_opaque().to_normalized_gamma_f32()
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let glass = self.theme == Theme::Prism;
        if self.native_glass != Some(glass) {
            if glass || self.native_glass == Some(true) {
                if let Err(error) = crate::glass::set_native(frame, glass) {
                    self.status = format!(
                        "{} : {error}",
                        tr(
                            self.language,
                            "Effet de flou indisponible sur ce bureau",
                            "Backdrop blur is unavailable on this desktop"
                        )
                    );
                }
            }
            self.native_glass = Some(glass);
        }
        self.handle_shortcuts(ctx);
        self.poll_scan(ctx);
        self.poll_covers(ctx);
        self.sidebar(ctx);
        self.content(ctx);
        if self.show_options {
            self.options_window(ctx);
        }
        if self.show_ignored {
            self.ignored_window(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prism_transparency_does_not_leak_into_other_themes() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.css_enabled = false;
        for theme in [Theme::Prism, Theme::Dark, Theme::Prism, Theme::Light] {
            app.theme = theme;
            app.refresh_appearance(&ctx);
            let visuals = &ctx.style().visuals;
            let alpha = eframe::App::clear_color(&app, visuals)[3];
            if theme == Theme::Prism {
                assert_eq!(alpha, 0.0, "The native backdrop must reach the content");
                assert!(visuals.panel_fill.a() < 128);
                assert!(visuals.window_fill.a() < 255);
                assert!(card_color(theme).a() < 128);
            } else {
                assert_eq!(
                    alpha, 1.0,
                    "Other themes must stay opaque after leaving Prisme"
                );
            }
        }
    }

    #[test]
    fn chosen_theme_survives_system_theme_changes() {
        for theme in [
            Theme::Dark,
            Theme::Light,
            Theme::Breeze,
            Theme::Horizon,
            Theme::Classic,
            Theme::Prism,
        ] {
            let ctx = egui::Context::default();
            apply_theme(&ctx, theme);
            let chosen_visuals = ctx.style().visuals.clone();
            for system_theme in [egui::Theme::Light, egui::Theme::Dark] {
                let _ = ctx.run(
                    egui::RawInput {
                        system_theme: Some(system_theme),
                        ..Default::default()
                    },
                    |ctx| {
                        assert_eq!(
                            ctx.style().visuals.panel_fill,
                            chosen_visuals.panel_fill,
                            "{theme:?}"
                        );
                        assert_eq!(
                            ctx.style().visuals.dark_mode,
                            chosen_visuals.dark_mode,
                            "{theme:?}"
                        );
                    },
                );
            }
        }
    }

    #[test]
    fn prominent_titles_use_text_color_instead_of_pressed_button_color() {
        for theme in [Theme::Dark, Theme::Light, Theme::Prism] {
            let ctx = egui::Context::default();
            let mut app = test_app();
            app.theme = theme;
            app.css_enabled = false;
            app.refresh_appearance(&ctx);
            let title = app.games[0].title.clone();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    app.sidebar(ctx);
                    app.content(ctx);
                },
            );
            let mut count = 0;
            for shape in &output.shapes {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    if text.galley.job.text == "RustyShelf" || text.galley.job.text == title {
                        count += 1;
                        for section in &text.galley.job.sections {
                            assert_eq!(
                                section.format.color,
                                theme_text(theme),
                                "{theme:?}: {}",
                                text.galley.job.text
                            );
                        }
                    }
                }
            }
            assert!(
                count >= 2,
                "Both navigation and game title must be rendered"
            );
        }
    }

    #[test]
    fn manual_3ds_addition_uses_an_emulator_profile_even_outside_rom_folders() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("Étoiles.3ds");
        let mut header = [0u8; 0x200];
        header[0x100..0x104].copy_from_slice(b"NCSD");
        std::fs::write(&path, header).unwrap();
        let mut app = test_app();
        app.store = Some(Store::open(temp.path().join("library.db")).unwrap());
        app.add_manual_path(path.clone());
        assert_eq!(app.games.len(), 1);
        assert_eq!(app.game_console(&app.games[0]), Some("Nintendo 3DS"));
        assert_eq!(app.games[0].installations[0].path, path);
        assert!(app.games[0].confirmed);
        assert!(app.games[0].installations[0].launch_profile.is_none());
    }

    #[test]
    fn manual_addition_accepts_extensionless_files_and_reports_disappearing_files() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("run-game");
        std::fs::write(&executable, []).unwrap();
        let mut app = test_app();
        app.store = Some(Store::open(temp.path().join("library.db")).unwrap());
        app.games.clear();
        app.add_manual_path(executable.clone());
        assert_eq!(app.games.len(), 1);
        assert_eq!(
            app.games[0].installations[0]
                .launch_profile
                .as_ref()
                .unwrap()
                .command,
            executable
        );
        let id = app.games[0].id.clone();
        app.store.as_ref().unwrap().ignore_game(&id).unwrap();
        app.add_manual_path(executable.clone());
        assert_eq!(app.games.len(), 1);
        assert_eq!(app.selected_game.as_ref(), Some(&id));
        std::fs::remove_file(&executable).unwrap();
        app.add_manual_path(executable);
        assert!(app.status.contains("introuvable"));
        assert_eq!(app.games.len(), 1);
    }

    #[test]
    fn keyboard_shortcuts_close_dialogs_and_focus_search() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let key_input = |key, modifiers| egui::RawInput {
            modifiers,
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        app.show_options = true;
        let _ = ctx.run(key_input(egui::Key::F, egui::Modifiers::CTRL), |ctx| {
            app.handle_shortcuts(ctx);
            assert!(!ctx.memory(|memory| memory.has_focus(egui::Id::new("library-search"))));
        });
        app.show_ignored = true;
        let _ = ctx.run(key_input(egui::Key::Escape, egui::Modifiers::NONE), |ctx| {
            app.handle_shortcuts(ctx)
        });
        assert!(!app.show_options);
        assert!(!app.show_ignored);
        let _ = ctx.run(key_input(egui::Key::F, egui::Modifiers::CTRL), |ctx| {
            app.handle_shortcuts(ctx);
            app.content(ctx);
            assert!(ctx.memory(|memory| memory.has_focus(egui::Id::new("library-search"))));
        });
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Text("Adventure".into())],
                ..Default::default()
            },
            |ctx| app.content(ctx),
        );
        assert_eq!(app.search, "Adventure");
    }

    #[test]
    fn scan_updates_keep_custom_titles_visible_before_completion() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app();
        let mut candidate = app.games[0].clone();
        candidate.confirmed = false;
        let store = Store::open(temp.path().join("library.db")).unwrap();
        store.save_game(&candidate).unwrap();
        store
            .set_title(&candidate.id, "Mon nom personnalisé")
            .unwrap();
        app.games = store.games().unwrap();
        app.store = Some(store);
        candidate.confirmed = true;
        let (sender, receiver) = mpsc::sync_channel(4);
        app.scan_receiver = Some(receiver);
        app.scan_control = Some(ScanControl::default());
        sender.send(ScanEvent::Candidate(candidate)).unwrap();
        app.poll_scan(&egui::Context::default());
        assert_eq!(app.games[0].title, "Mon nom personnalisé");
        assert!(app.games[0].confirmed);
        sender
            .send(ScanEvent::Finished {
                visited: 1,
                cancelled: true,
            })
            .unwrap();
        app.poll_scan(&egui::Context::default());
        assert_eq!(app.games[0].title, "Mon nom personnalisé");
    }

    #[test]
    fn failed_css_changes_preserve_the_saved_and_visible_style() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("library.db");
        let old_path = temp.path().join("old.css");
        let new_path = temp.path().join("new.css");
        std::fs::write(&old_path, ":root { --accent: #123456; }").unwrap();
        std::fs::write(&new_path, ":root { --accent: #abcdef; }").unwrap();
        let mut app = test_app();
        app.store = Some(Store::open(&database).unwrap());
        let ctx = egui::Context::default();
        app.load_css(&ctx, old_path.clone());
        assert!(app.css_error.is_none());
        assert_eq!(app.css_path.as_ref(), Some(&old_path));
        let accent = app.custom_style.as_ref().unwrap().accent;
        let connection = rusqlite::Connection::open(&database).unwrap();
        // The path write succeeds first; the enabled write then fails.
        connection.execute_batch("CREATE TRIGGER reject_css BEFORE INSERT ON preferences WHEN NEW.key='css_enabled' BEGIN SELECT RAISE(ABORT,'refus CSS'); END;").unwrap();
        app.load_css(&ctx, new_path);
        assert!(app.css_error.is_some());
        assert_eq!(app.css_path.as_ref(), Some(&old_path));
        assert_eq!(app.custom_style.as_ref().unwrap().accent, accent);
        assert_eq!(
            app.store.as_ref().unwrap().preference("css_path").unwrap(),
            Some(old_path.to_string_lossy().into_owned())
        );
        app.set_css_enabled(&ctx, false);
        assert!(app.css_enabled);
        assert!(app.css_error.is_some());
        connection
            .execute_batch("DROP TRIGGER reject_css;")
            .unwrap();
        app.set_css_enabled(&ctx, false);
        assert!(!app.css_enabled);
        // An invalid reload must not reactivate a disabled style.
        std::fs::write(&old_path, "broken CSS").unwrap();
        app.set_css_enabled(&ctx, true);
        assert!(!app.css_enabled);
        assert!(app.css_error.is_some());
        assert_eq!(
            app.store
                .as_ref()
                .unwrap()
                .preference("css_enabled")
                .unwrap()
                .as_deref(),
            Some("false")
        );
    }

    #[test]
    fn css_cannot_be_enabled_without_a_file() {
        let mut app = test_app();
        app.css_enabled = false;
        app.css_path = None;
        app.set_css_enabled(&egui::Context::default(), true);
        assert!(!app.css_enabled);
        assert!(app.css_error.as_deref().unwrap().contains("CSS"));
    }

    #[test]
    fn distinct_titles_are_not_merged_by_removing_spaces_and_punctuation() {
        let games: Vec<_> = ["A-B", "AB", "A B"]
            .into_iter()
            .enumerate()
            .map(|(index, title)| {
                Game::from_installation(
                    title,
                    "Steam",
                    Installation {
                        path: PathBuf::from(format!("/games/{index}")),
                        launcher: Launcher::Steam,
                        launch_profile: None,
                        available: true,
                    },
                )
            })
            .collect();
        let library = deduplicate_library(games);
        assert_eq!(library.len(), 3);
        assert!(library.iter().all(|game| game.installations.len() == 1));
    }

    #[test]
    fn same_named_images_are_not_merged_based_on_guessed_console() {
        let library = ["/roms/PS2/game.iso", "/roms/GameCube/game.iso"]
            .into_iter()
            .map(|path| {
                Game::from_installation(
                    "Game",
                    "ROM",
                    Installation {
                        path: path.into(),
                        launcher: Launcher::Unknown,
                        launch_profile: None,
                        available: true,
                    },
                )
            })
            .collect();
        assert_eq!(deduplicate_library(library).len(), 2);
    }

    fn test_app() -> RustyShelfApp {
        let games = (0..40)
            .map(|index| {
                Game::from_installation(
                    format!("Adventure {index}"),
                    "ROM · NES",
                    Installation {
                        path: PathBuf::from(format!("/roms/adventure-{index}.nes")),
                        launcher: Launcher::Manual,
                        launch_profile: None,
                        available: true,
                    },
                )
            })
            .collect();
        RustyShelfApp {
            selected_category: "ROMs".into(),
            search: String::new(),
            show_options: false,
            show_ignored: false,
            ignored_games: Vec::new(),
            ignored_search: String::new(),
            games,
            selected_game: None,
            review_selection: HashSet::new(),
            store: None,
            status: "Hors ligne".into(),
            volumes: vec![],
            selected_volumes: vec![],
            saved_scan_volumes: None,
            exclusions: vec![],
            scan_control: None,
            scan_receiver: None,
            scan_visited: 0,
            scan_current: String::new(),
            scan_roots: vec![],
            scan_exclusions: vec![],
            scan_seen_paths: vec![],
            diagnostics: ScanDiagnostics::default(),
            cover_textures: HashMap::new(),
            cover_refresh: None,
            category_edit: String::new(),
            category_edit_id: None,
            language: Language::French,
            title_edit: String::new(),
            title_edit_id: None,
            theme: Theme::Dark,
            native_glass: None,
            emulators: vec![],
            css_path: None,
            css_enabled: true,
            custom_style: Some(CustomStyle::parse(crate::style::EXAMPLE).unwrap()),
            css_error: None,
            filters: Filters::default(),
            console_overrides: HashMap::new(),
            console_emulators: HashMap::new(),
        }
    }

    #[test]
    fn interrupted_scan_reloads_saved_corrections_without_marking_games_missing() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let mut app = test_app();
        let game = app.games[0].clone();
        store.save_game(&game).unwrap();
        store.set_category(&game.id, "Mes aventures").unwrap();
        store.set_favorite(&game.id, true).unwrap();
        app.store = Some(store);
        app.scan_roots = vec!["/roms".into()];
        for abrupt in [false, true] {
            app.games = vec![game.clone()];
            let (sender, receiver) = mpsc::sync_channel(4);
            sender.send(ScanEvent::Candidate(game.clone())).unwrap();
            if !abrupt {
                sender
                    .send(ScanEvent::Finished {
                        visited: 20,
                        cancelled: true,
                    })
                    .unwrap();
            }
            drop(sender);
            app.scan_receiver = Some(receiver);
            app.scan_control = Some(ScanControl::default());
            app.poll_scan(&egui::Context::default());
            assert!(app.scan_receiver.is_none());
            assert!(app.scan_control.is_none());
            assert_eq!(app.games[0].category, "Mes aventures");
            assert!(app.games[0].favorite);
            assert!(app.games[0].installations[0].available);
            assert_eq!(app.diagnostics.error_count, usize::from(abrupt));
        }
    }

    #[test]
    fn ignored_items_can_be_restored_one_at_a_time_with_their_corrections() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app();
        app.games.truncate(2);
        app.games[0].favorite = true;
        app.games[0].category = "Mes aventures".into();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for game in &app.games {
            store.save_game(game).unwrap();
            store.ignore_game(&game.id).unwrap();
        }
        let restored = app.games[0].id.clone();
        let still_hidden = app.games[1].id.clone();
        app.store = Some(store);
        app.open_ignored_games();
        assert!(app.show_ignored);
        assert_eq!(app.ignored_games.len(), 2);
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| app.ignored_window(ctx));
        assert!(!output.shapes.is_empty());
        app.restore_ignored_item(&restored);
        assert_eq!(app.games.len(), 1);
        assert!(app.games[0].favorite);
        assert_eq!(app.games[0].category, "Mes aventures");
        assert_eq!(app.ignored_games.len(), 1);
        let store = app.store.as_ref().unwrap();
        assert!(store.is_ignored(&still_hidden).unwrap());
        assert_eq!(store.ignored_games().unwrap()[0].id, still_hidden);
        assert!(!store.is_ignored(&restored).unwrap());
    }

    #[test]
    fn selected_drives_are_persisted_without_losing_disconnected_choices() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.db");
        let mut app = test_app();
        app.store = Some(Store::open(&path).unwrap());
        app.volumes = vec![Volume {
            root: temp.path().join("a"),
            name: "A".into(),
            filesystem: "Test".into(),
            removable: false,
        }];
        app.selected_volumes = vec![false];
        app.saved_scan_volumes = Some(vec![temp.path().join("offline")]);
        assert!(app.save_scan_volumes());
        drop(app);
        let store = Store::open(&path).unwrap();
        let saved: Vec<PathBuf> =
            serde_json::from_str(&store.preference("selected_scan_volumes").unwrap().unwrap())
                .unwrap();
        assert_eq!(saved, [temp.path().join("offline")]);
    }

    #[test]
    fn review_selection_respects_filters_and_can_confirm_multiple_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app();
        app.games.truncate(2);
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for game in &mut app.games {
            game.confirmed = false;
            store.save_game(game).unwrap();
            app.review_selection.insert(game.id.clone());
        }
        app.store = Some(store);
        app.selected_category = "À confirmer".into();
        app.search = "Adventure 0".into();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.content(ctx));
        assert_eq!(app.review_selection.len(), 1);
        app.apply_review_selection(ReviewAction::Confirm);
        assert!(app.review_selection.is_empty());
        assert_eq!(app.games.iter().filter(|game| game.confirmed).count(), 1);
        app.search.clear();
        for game in &app.games {
            app.review_selection.insert(game.id.clone());
        }
        app.apply_review_selection(ReviewAction::Ignore);
        assert_eq!(app.games.len(), 1);
        assert!(app.games[0].confirmed);
    }

    #[test]
    fn cover_refresh_saves_missing_images_without_overwriting_new_manual_choices() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app();
        app.games.clear();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for title in ["Étoiles", "Manual", "Ignored"] {
            let folder = temp.path().join(title);
            std::fs::create_dir(&folder).unwrap();
            image::RgbaImage::new(12, 8)
                .save(folder.join("cover.png"))
                .unwrap();
            let game = Game::from_installation(
                title,
                "Ajout manuel",
                Installation {
                    path: folder,
                    launcher: Launcher::Manual,
                    launch_profile: None,
                    available: true,
                },
            );
            store.save_game(&game).unwrap();
            app.games.push(game);
        }
        let manual_id = app.games[1].id.clone();
        let ignored_id = app.games[2].id.clone();
        app.store = Some(store);
        app.refresh_covers();
        assert!(
            app.scan_control.is_none(),
            "Cover reload must not start a drive scan"
        );
        let chosen = temp.path().join("unplugged/chosen.png");
        assert!(app.update_library(|store| {
            store.set_cover(&manual_id, Some(&chosen))?;
            store.ignore_game(&ignored_id)
        }));
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.cover_refresh.is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_covers(&ctx);
            std::thread::yield_now();
        }
        let games = app.store.as_ref().unwrap().games().unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(
            games
                .iter()
                .find(|game| game.id == manual_id)
                .unwrap()
                .cover_path
                .as_ref(),
            Some(&chosen)
        );
        let automatic = games.iter().find(|game| game.title == "Étoiles").unwrap();
        assert_eq!(
            automatic.cover_path.as_ref(),
            Some(&temp.path().join("Étoiles/cover.png"))
        );
        assert!(app
            .status
            .starts_with("1 jaquette(s) ajoutée(s) · 0 erreur(s)"));
    }

    #[test]
    fn covers_cache_missing_files_and_reload_after_explicit_refresh() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cover.png");
        let mut app = test_app();
        let mut game = app.games[0].clone();
        game.cover_path = Some(path.clone());
        let ctx = egui::Context::default();
        assert!(app.load_cover(&ctx, &game).is_none());
        image::RgbaImage::new(12, 8).save(&path).unwrap();
        assert!(app.load_cover(&ctx, &game).is_none());
        app.cover_textures.clear();
        assert!(app.load_cover(&ctx, &game).is_some());
        game.cover_path = Some(temp.path().join("different.png"));
        assert!(app.load_cover(&ctx, &game).is_none());
        image::RgbaImage::new(4097, 1).save(&path).unwrap();
        assert!(read_cover(&path).is_err());
    }

    #[test]
    fn scan_hides_old_false_positives_but_respects_changes_made_during_scan() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("roms");
        std::fs::create_dir(&root).unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let mut app = test_app();
        app.games.clear();
        for name in [
            "Database",
            "Favorite",
            "Renamed",
            "Confirmed",
            "Console choice",
        ] {
            let path = root.join(format!("{name}.gen"));
            std::fs::write(&path, b"not a cartridge").unwrap();
            let game = Game::from_installation(
                name,
                "ROM · Mega Drive",
                Installation {
                    path,
                    launcher: Launcher::Unknown,
                    launch_profile: None,
                    available: true,
                },
            );
            store.save_game(&game).unwrap();
            app.games.push(game);
        }
        let ids: Vec<_> = app.games.iter().map(|game| game.id.clone()).collect();
        app.store = Some(store);
        app.begin_scan(vec![root], false);
        // The worker has a snapshot; the database and latest UI choices remain authoritative.
        let store = app.store.as_ref().unwrap();
        store.set_favorite(&ids[1], true).unwrap();
        store.set_title(&ids[2], "Mon jeu").unwrap();
        store.set_confirmed(&ids[3], true).unwrap();
        app.console_overrides
            .insert(ids[4].clone(), "Mega Drive".into());
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.scan_control.is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_scan(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(app.diagnostics.filtered_count, 1);
        assert_eq!(app.diagnostics.error_count, 0);
        assert_eq!(app.games.len(), 4);
        assert_eq!(app.status, "Scan terminé");
        let store = app.store.as_ref().unwrap();
        assert!(store.is_ignored(&ids[0]).unwrap());
        store.restore_ignored_game(&ids[0]).unwrap();
        assert_eq!(store.games().unwrap().len(), 5);
        assert!(temp.path().join("roms/Database.gen").is_file());
    }

    #[test]
    fn scan_hides_legacy_nested_pc_duplicates_after_confirming_launcher_installations() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("Games");
        let steamapps = root.join("Steam/steamapps");
        let install = steamapps.join("common/Été");
        let duplicate = install.join("Bin");
        let twin = install.join("Twin");
        let customized = install.join("Tools");
        let excluded = install.join("Excluded");
        let outside = temp.path().join("Other drive/Twin");
        let portable = steamapps.join("common/Été portable");
        for path in [
            &duplicate,
            &twin,
            &customized,
            &excluded,
            &outside,
            &portable,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        std::fs::write(
            steamapps.join("appmanifest_42.acf"),
            r#""AppState" { "appid" "42" "name" "Été" "installdir" "Été" }"#,
        )
        .unwrap();
        let rom = install.join("Homebrew.nes");
        std::fs::write(&rom, b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();
        let candidate = |title, path| {
            Game::from_installation(
                title,
                "À confirmer",
                Installation {
                    path,
                    launcher: Launcher::Unknown,
                    launch_profile: None,
                    available: true,
                },
            )
        };
        let old_root = candidate("setup", install.clone());
        let helper = candidate("Companion tool", duplicate);
        let mut multi = candidate("Twin", twin);
        multi.installations.push(Installation {
            path: outside,
            ..multi.installations[0].clone()
        });
        let protected = candidate("Keep this tool", customized);
        let excluded_game = candidate("Excluded tool", excluded.clone());
        let cartridge = candidate("Homebrew", rom);
        let neighbor = candidate("Portable game", portable);
        let store = Store::open(temp.path().join("library.db")).unwrap();
        for game in [
            &old_root,
            &helper,
            &multi,
            &protected,
            &excluded_game,
            &cartridge,
            &neighbor,
        ] {
            store.save_game(game).unwrap();
        }
        store.set_title(&protected.id, "À conserver").unwrap();
        let mut app = test_app();
        app.games = deduplicate_library(store.games().unwrap());
        app.exclusions = vec![excluded];
        app.store = Some(store);
        app.begin_scan(vec![root], false);
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.scan_control.is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_scan(&ctx);
            std::thread::yield_now();
        }
        let store = app.store.as_ref().unwrap();
        assert_eq!(app.diagnostics.filtered_count, 1);
        assert_eq!(app.diagnostics.error_count, 0);
        assert!(store.is_ignored(&helper.id).unwrap());
        for game in [
            &old_root,
            &multi,
            &protected,
            &excluded_game,
            &cartridge,
            &neighbor,
        ] {
            assert!(!store.is_ignored(&game.id).unwrap(), "{}", game.title);
        }
        let saved = store.games().unwrap();
        let upgraded = saved.iter().find(|game| game.id == old_root.id).unwrap();
        assert!(upgraded.confirmed);
        assert_eq!(upgraded.title, "Été");
        assert_eq!(upgraded.installations[0].launcher, Launcher::Steam);
        assert!(
            saved
                .iter()
                .find(|game| game.id == cartridge.id)
                .unwrap()
                .installations[0]
                .available
        );
    }

    #[test]
    fn last_scan_review_only_lists_new_saved_candidates_even_after_interruption() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        let make = |title: &str, launcher| {
            Game::from_installation(
                title,
                "ROM · NES",
                Installation {
                    path: temp.path().join(format!("{title}.nes")),
                    launcher,
                    launch_profile: None,
                    available: true,
                },
            )
        };
        let old = make("Ancien", Launcher::Unknown);
        let hidden = make("Ignoré", Launcher::Unknown);
        let new = make("Nouveau", Launcher::Unknown);
        let official = make("Reconnu", Launcher::Steam);
        for game in [&old, &hidden] {
            store.save_game(game).unwrap();
        }
        store.ignore_game(&hidden.id).unwrap();
        let mut app = test_app();
        app.selected_category = "Tous les jeux".into();
        app.games = store.games().unwrap();
        app.store = Some(store);
        let (sender, receiver) = mpsc::sync_channel(8);
        for game in [&old, &new, &new, &hidden, &official] {
            sender.send(ScanEvent::Candidate(game.clone())).unwrap();
        }
        sender
            .send(ScanEvent::Finished {
                visited: 20,
                cancelled: true,
            })
            .unwrap();
        app.scan_control = Some(ScanControl::default());
        app.scan_receiver = Some(receiver);
        app.poll_scan(&egui::Context::default());
        assert_eq!(app.selected_category, "Tous les jeux");
        assert_eq!(
            app.diagnostics.added_game_ids,
            [new.id.clone(), official.id.clone()].into()
        );

        let json = app
            .store
            .as_ref()
            .unwrap()
            .preference("last_scan_diagnostics")
            .unwrap()
            .unwrap();
        app.diagnostics = serde_json::from_str(&json).unwrap();
        app.search = "indésirable".into();
        app.filters.kind = GameKind::Pc;
        app.filters.sort = Sort::ReverseName;
        app.open_last_scan_candidates();
        assert!(app.search.is_empty());
        assert_eq!(app.filters.kind, GameKind::All);
        assert_eq!(app.filters.sort, Sort::ReverseName);
        assert_eq!(
            app.games
                .iter()
                .filter(|game| app.matches_category(game))
                .map(|game| &game.id)
                .collect::<Vec<_>>(),
            [&new.id]
        );
        app.review_selection
            .extend([new.id.clone(), old.id.clone()]);
        app.apply_review_selection(ReviewAction::Confirm);
        assert!(!app.games.iter().any(|game| app.matches_category(game)));
        assert!(
            !app.games
                .iter()
                .find(|game| game.id == old.id)
                .unwrap()
                .confirmed
        );
    }

    #[test]
    fn folder_scan_runs_to_completion_and_preserves_games_outside_its_scope() {
        let temp = tempfile::tempdir().unwrap();
        let chosen = temp.path().join("roms");
        std::fs::create_dir(&chosen).unwrap();
        std::fs::write(
            chosen.join("Étoiles.nes"),
            b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0",
        )
        .unwrap();
        let cover = chosen.join("Étoiles.png");
        image::RgbaImage::new(12, 8).save(&cover).unwrap();
        let mut app = test_app();
        let outside = app.games[0].clone();
        let store = Store::open(temp.path().join("library.db")).unwrap();
        store.save_game(&outside).unwrap();
        app.store = Some(store);
        app.begin_scan(vec![chosen.clone()], false);
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.scan_control.is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "Le scan ne se termine pas"
            );
            app.poll_scan(&ctx);
            std::thread::yield_now();
        }
        assert_eq!(app.scan_roots, [chosen]);
        assert_eq!(app.diagnostics.roots, app.scan_roots);
        assert_eq!(app.diagnostics.cancelled, Some(false));
        assert_eq!(app.diagnostics.error_count, 0);
        assert!(app
            .games
            .iter()
            .any(|game| game.title == "Étoiles" && !game.confirmed));
        let found = app
            .games
            .iter()
            .find(|game| game.title == "Étoiles")
            .unwrap();
        assert_eq!(found.cover_path.as_ref(), Some(&cover));
        assert_eq!(
            app.store
                .as_ref()
                .unwrap()
                .games()
                .unwrap()
                .iter()
                .find(|game| game.id == found.id)
                .unwrap()
                .cover_path
                .as_ref(),
            Some(&cover)
        );
        assert!(
            app.games
                .iter()
                .find(|game| game.id == outside.id)
                .unwrap()
                .installations[0]
                .available
        );
    }

    #[test]
    fn closing_app_cancels_the_worker_and_scan_requires_storage() {
        let mut app = test_app();
        app.start_scan();
        assert!(app.scan_control.is_none());
        assert!(app.status.contains("Base locale indisponible"));
        let control = ScanControl::default();
        app.scan_control = Some(control.clone());
        drop(app);
        assert!(control.is_cancelled());
    }

    #[test]
    fn failed_saves_keep_the_library_and_report_the_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.db");
        let mut app = test_app();
        let game = app.games[0].clone();
        assert!(!app.update_library(|store| store.set_favorite(&game.id, true)));
        assert!(!app.games[0].favorite);
        assert!(app.status.contains("Base locale indisponible"));
        let store = Store::open(&path).unwrap();
        store.save_game(&game).unwrap();
        app.store = Some(store);
        assert!(app.update_library(|store| store.set_favorite(&game.id, true)));
        assert!(app.games[0].favorite);
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_change BEFORE UPDATE ON games BEGIN SELECT RAISE(ABORT, 'écriture refusée'); END;").unwrap();
        assert!(!app.update_library(|store| store.set_favorite(&game.id, false)));
        assert!(app.games[0].favorite);
        assert!(app.status.contains("écriture refusée"));
        // A successful write followed by an unreadable catalogue also retains the last view.
        connection
            .execute_batch("ALTER TABLE installations RENAME TO damaged_installations;")
            .unwrap();
        assert!(!app.update_library(|store| store.set_preference("test", "saved")));
        assert_eq!(app.games.len(), 1);
        assert!(app.status.contains("rechargement impossible"));
    }

    #[test]
    fn unconfirmed_roms_only_appear_in_review_and_confirmed_items_can_be_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = test_app();
        let mut game = app.games[0].clone();
        game.confirmed = false;
        game.favorite = true;
        for category in [
            "Tous les jeux",
            "Favoris",
            "ROMs",
            "console:NES",
            "Ajout manuel",
        ] {
            app.selected_category = category.into();
            assert!(!app.matches_category(&game), "{category}");
        }
        app.selected_category = "À confirmer".into();
        assert!(app.matches_category(&game));
        game.confirmed = true;
        app.selected_category = "console:NES".into();
        assert!(app.matches_category(&game));
        let store = Store::open(temp.path().join("library.db")).unwrap();
        store.save_game(&game).unwrap();
        app.store = Some(store);
        assert!(app.update_library(|store| store.ignore_game(&game.id)));
        assert!(app.games.is_empty());
        assert!(app.update_library(|store| store.save_game(&game)));
        assert!(app.games.is_empty());
        assert!(app.update_library(|store| store.restore_ignored_games()));
        assert_eq!(app.games.len(), 1);
    }

    #[test]
    fn options_close_button_closes_and_window_can_reopen() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.show_options = true;
        let input = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        for _ in 0..3 {
            let _ = ctx.run(input(vec![]), |ctx| app.options_window(ctx));
        }
        let rect = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("options-window")))
            .unwrap();
        assert!(rect.width() >= 680.0);
        let pos = egui::pos2(rect.right() - 17.0, rect.top() + 17.0);
        for pressed in [true, false] {
            let _ = ctx.run(
                input(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| app.options_window(ctx),
            );
        }
        assert!(!app.show_options, "La croix doit fermer les options");
        app.show_options = true;
        let _ = ctx.run(input(vec![]), |ctx| app.options_window(ctx));
        assert!(app.show_options);
    }

    #[test]
    fn library_and_options_render_at_compact_and_wide_sizes() {
        let ctx = egui::Context::default();
        ctx.options_mut(|options| options.warn_on_id_clash = true);
        let mut app = test_app();
        app.refresh_appearance(&ctx);
        app.show_options = true;
        app.volumes = vec![Volume {
            root: "C:/".into(),
            name: "Système".into(),
            filesystem: "NTFS".into(),
            removable: false,
        }];
        app.selected_volumes = vec![true];
        app.diagnostics
            .skip(std::path::Path::new("C:/protected"), "Accès refusé");
        app.diagnostics.error("Erreur de test".into());
        app.games[0].category = "Plateforme".into();
        app.games[0].installations[0].launcher = Launcher::Steam;
        app.selected_category = "Steam".into();
        assert!(app.matches_category(&app.games[0]));
        app.selected_category = "ROMs".into();
        for (width, review) in [
            (780.0, false),
            (1280.0, false),
            (780.0, true),
            (1280.0, true),
        ] {
            if review {
                app.selected_category = "À confirmer".into();
                for game in &mut app.games {
                    game.confirmed = false;
                }
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    app.sidebar(ctx);
                    app.content(ctx);
                    app.options_window(ctx);
                },
            );
            assert!(!output.shapes.is_empty());
            fn check_ids(shape: &egui::epaint::Shape) {
                match shape {
                    egui::epaint::Shape::Text(text) => {
                        let text = text.galley.text();
                        assert!(
                            !(text.contains("use of") && text.contains(" ID ")),
                            "{text}"
                        );
                    }
                    egui::epaint::Shape::Vec(shapes) => {
                        for shape in shapes {
                            check_ids(shape);
                        }
                    }
                    _ => {}
                }
            }
            for shape in &output.shapes {
                check_ids(&shape.shape);
            }
        }
    }

    #[test]
    fn missing_emulator_returns_a_launch_error() {
        let temp = tempfile::tempdir().unwrap();
        assert!(launch_emulator(
            &temp.path().join("missing-emulator"),
            &temp.path().join("game.iso")
        )
        .is_err());
    }

    #[test]
    fn windows_classic_theme_preferences_round_trip() {
        for (theme, key, label) in [
            (Theme::Horizon, "horizon", "Horizon"),
            (Theme::Classic, "retro", "Rétro"),
        ] {
            assert_eq!(Theme::parse(Some(key.to_owned())), theme);
            assert_eq!(theme.key(), key);
            assert_eq!(theme.label(Language::French), label);
        }
    }

    #[test]
    fn emulator_launch_command_passes_image_path_without_running_it() {
        let emulator = PathBuf::from("fake-emulator");
        let image = PathBuf::from("C:/Games/Disc Image.iso");
        let command = emulator_process(&emulator, &image);
        assert_eq!(command.get_program(), emulator.as_os_str());
        assert_eq!(command.get_args().collect::<Vec<_>>(), [image.as_os_str()]);
    }

    #[test]
    fn launch_profile_preserves_arguments_without_running_it() {
        let profile = LaunchProfile {
            command: PathBuf::from("fake-runner"),
            arguments: vec!["--game".into(), "Save Slot 1".into()],
            working_directory: Some(PathBuf::from("C:/Games")),
            launcher_command: None,
        };
        let command = profile_process(&profile);
        assert_eq!(command.get_program(), profile.command.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--game", "Save Slot 1"]
        );
    }

    #[test]
    fn combines_games_with_the_same_install_path_and_keeps_known_title() {
        let path = PathBuf::from("C:/Games/Orange Juice");
        let known = Game::from_installation(
            "100% Orange Juice",
            "Steam",
            Installation {
                path: path.clone(),
                launcher: Launcher::Steam,
                launch_profile: None,
                available: true,
            },
        );
        let candidate = Game::from_installation(
            "100orange",
            "À confirmer",
            Installation {
                path,
                launcher: Launcher::Unknown,
                launch_profile: None,
                available: true,
            },
        );
        let library = deduplicate_library(vec![candidate, known]);
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].title, "100% Orange Juice");
        assert_eq!(library[0].installations.len(), 1);
    }
}
