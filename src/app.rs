use crate::{
    library::{Filters, GameKind, Sort},
    model::{Emulator, Game, Installation, LaunchProfile, Launcher},
    scan::{self, ScanControl, ScanEvent, Volume},
    sources,
    store::Store,
    style::CustomStyle,
};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
};

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
            visuals.panel_fill = Color32::from_rgba_unmultiplied(20, 29, 49, 235);
            visuals.window_fill = Color32::from_rgba_unmultiplied(49, 65, 94, 220);
            visuals.widgets.noninteractive.bg_fill =
                Color32::from_rgba_unmultiplied(66, 89, 125, 100);
            visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(90, 114, 153, 115);
            visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(130, 162, 213, 160);
            visuals.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(151, 180, 231, 190);
            visuals.window_corner_radius = egui::CornerRadius::same(18);
        }
    }
    visuals.selection.bg_fill = theme_accent(theme);
    visuals.selection.stroke.color = crate::style::text_on(theme_accent(theme));
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
    games: Vec<Game>,
    selected_game: Option<String>,
    store: Option<Store>,
    status: String,
    volumes: Vec<Volume>,
    selected_volumes: Vec<bool>,
    exclusions: Vec<PathBuf>,
    scan_control: Option<ScanControl>,
    scan_receiver: Option<Receiver<ScanEvent>>,
    scan_visited: usize,
    scan_current: String,
    scan_roots: Vec<PathBuf>,
    scan_exclusions: Vec<PathBuf>,
    scan_seen_paths: Vec<PathBuf>,
    errors: Vec<String>,
    cover_textures: HashMap<String, TextureHandle>,
    category_edit: String,
    category_edit_id: Option<String>,
    language: Language,
    theme: Theme,
    emulators: Vec<Emulator>,
    preferred_emulator: Option<String>,
    css_path: Option<PathBuf>,
    css_enabled: bool,
    custom_style: Option<CustomStyle>,
    css_error: Option<String>,
    filters: Filters,
    console_overrides: HashMap<String, String>,
    console_emulators: HashMap<String, String>,
}

impl RustyShelfApp {
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
        let mut preferred_emulator = store
            .as_ref()
            .and_then(|s| s.preference("preferred_emulator").ok())
            .flatten()
            .filter(|id| !id.is_empty());
        if preferred_emulator.is_none() && emulators.len() == 1 {
            preferred_emulator = emulators.first().map(|emulator| emulator.id.clone());
            if let (Some(store), Some(id)) = (&store, &preferred_emulator) {
                let _ = store.set_preference("preferred_emulator", id);
            }
        }
        let exclusions = store
            .as_ref()
            .and_then(|s| s.exclusions().ok())
            .unwrap_or_default();
        if let Some(error) = store_error {
            status = format!("Base locale indisponible : {error}");
        }
        let volumes = scan::mounted_volumes();
        let console_overrides = load_mapping(store.as_ref(), "console_overrides");
        let console_emulators = load_mapping(store.as_ref(), "console_emulators");
        let selected_volumes = vec![true; volumes.len()];
        Self {
            selected_category: "Tous les jeux".into(),
            search: String::new(),
            show_options: false,
            selected_game: games
                .iter()
                .find(|game| game.confirmed)
                .or_else(|| games.first())
                .map(|game| game.id.clone()),
            games,
            store,
            status,
            volumes,
            selected_volumes,
            exclusions,
            scan_control: None,
            scan_receiver: None,
            scan_visited: 0,
            scan_current: String::new(),
            scan_roots: Vec::new(),
            scan_exclusions: Vec::new(),
            scan_seen_paths: Vec::new(),
            errors: Vec::new(),
            cover_textures: HashMap::new(),
            category_edit: String::new(),
            category_edit_id: None,
            language,
            theme,
            emulators,
            preferred_emulator,
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

    fn game_console<'a>(&'a self, game: &Game) -> Option<&'a str> {
        self.console_overrides
            .get(&game.id)
            .map(String::as_str)
            .or_else(|| crate::rom::game_console(game))
    }

    fn default_emulator(&self, game: &Game) -> Option<&String> {
        self.game_console(game)
            .and_then(|console| self.console_emulators.get(console))
            .or(self.preferred_emulator.as_ref())
    }

    fn save_mapping(&mut self, key: &str, mapping: HashMap<String, String>) {
        if let Some(store) = &self.store {
            match serde_json::to_string(&mapping) {
                Ok(json) => {
                    if let Err(error) = store.set_preference(key, &json) {
                        self.status = error.to_string();
                    }
                }
                Err(error) => self.status = error.to_string(),
            }
        }
    }

    fn refresh_appearance(&self, ctx: &egui::Context) {
        apply_theme(ctx, self.theme);
        if let Some(style) = self.active_style() {
            style.apply(ctx);
        }
    }

    fn load_css(&mut self, ctx: &egui::Context) {
        let Some(path) = self.css_path.as_deref() else {
            return;
        };
        match CustomStyle::load(path) {
            Ok(style) => {
                self.custom_style = Some(style);
                self.css_enabled = true;
                self.css_error = None;
                if let Some(store) = &self.store {
                    if let Err(error) = store
                        .set_preference("css_path", &path.to_string_lossy())
                        .and_then(|_| store.set_preference("css_enabled", "true"))
                    {
                        self.css_error = Some(error.to_string());
                    }
                }
                self.refresh_appearance(ctx);
            }
            Err(error) => self.css_error = Some(error),
        }
    }

    fn start_scan(&mut self) {
        if let Some(control) = &self.scan_control {
            control.cancel();
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
        let exclusions = self.exclusions.clone();
        self.scan_roots = roots.clone();
        self.scan_exclusions = exclusions.clone();
        self.scan_seen_paths.clear();
        let control = ScanControl::default();
        let worker_control = control.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut scan_exclusions = exclusions;
                for mut game in sources::discover(&roots) {
                    if worker_control.is_cancelled() {
                        break;
                    }
                    game.installations.retain(|installation| {
                        roots
                            .iter()
                            .any(|root| scan::path_is_within(&installation.path, root))
                            && !scan_exclusions
                                .iter()
                                .any(|excluded| scan::path_is_within(&installation.path, excluded))
                    });
                    if game.installations.is_empty() {
                        continue;
                    }
                    scan_exclusions.extend(
                        game.installations
                            .iter()
                            .map(|installation| installation.path.clone()),
                    );
                    if sender.send(ScanEvent::Candidate(game)).is_err() {
                        return;
                    }
                }
                scan::scan_roots(&roots, &scan_exclusions, &worker_control, |event| {
                    let _ = sender.send(event);
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
        self.scan_visited = 0;
        self.errors.clear();
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
        let events: Vec<_> = receiver.try_iter().take(8).collect();
        for event in events {
            match event {
                ScanEvent::Candidate(game) => {
                    self.scan_seen_paths.extend(
                        game.installations
                            .iter()
                            .map(|installation| installation.path.clone()),
                    );
                    if let Some(store) = &self.store {
                        if store.is_ignored(&game.id).unwrap_or(false) {
                            continue;
                        }
                        if let Err(error) = store.save_game(&game) {
                            self.errors.push(error.to_string());
                        }
                    }
                    merge_game_in_memory(&mut self.games, game);
                }
                ScanEvent::Progress(progress) => {
                    self.scan_visited = progress.visited;
                    self.scan_current = progress.current.to_string_lossy().to_string();
                }
                ScanEvent::Error(path, message) => {
                    // An unreadable subtree is not proof that a game was removed.
                    if !path.as_os_str().is_empty() {
                        self.scan_exclusions.push(path.clone());
                    }
                    if self.errors.len() < 30 {
                        self.errors.push(format!("{} : {message}", path.display()));
                    }
                }
                ScanEvent::Finished { visited, cancelled } => {
                    self.scan_visited = visited;
                    if !cancelled {
                        if let Some(store) = &self.store {
                            let _ = store.mark_unseen_under_roots_unavailable(
                                &self.scan_roots,
                                &self.scan_exclusions,
                                &self.scan_seen_paths,
                            );
                            self.games = store
                                .games()
                                .map(deduplicate_library)
                                .unwrap_or_else(|_| self.games.clone());
                        }
                    }
                    self.scan_control = None;
                    self.scan_receiver = None;
                    self.status = if cancelled {
                        format!(
                            "{} {visited} {}",
                            tr(self.language, "Scan arrêté ·", "Scan stopped ·"),
                            tr(self.language, "éléments parcourus", "items scanned")
                        )
                    } else {
                        format!(
                            "{} {} {} {visited} {}",
                            tr(self.language, "Scan terminé ·", "Scan complete ·"),
                            self.games.len(),
                            tr(self.language, "jeux ·", "games ·"),
                            tr(self.language, "éléments parcourus", "items scanned")
                        )
                    };
                }
            }
        }
        if self.scan_control.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn add_manual_game(&mut self) {
        let Some(executable) = rfd::FileDialog::new()
            .add_filter("Exécutable", &["exe", "sh", "x86_64"])
            .add_filter(
                "ROM / image disque",
                &[
                    "nes", "sfc", "smc", "gb", "gbc", "gba", "nds", "3ds", "n64", "z64", "iso",
                    "chd", "cso", "wbfs", "rvz", "gcm", "cue", "pbp", "gen", "sms", "gg",
                ],
            )
            .pick_file()
        else {
            return;
        };
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
        let game = Game::from_installation(
            title,
            "Ajout manuel",
            Installation {
                path: install_path,
                launcher: Launcher::Manual,
                launch_profile: if is_rom { None } else { Some(profile) },
                available: true,
            },
        );
        if let Some(store) = &self.store {
            if store.save_game(&game).is_ok() {
                self.games = store.games().map(deduplicate_library).unwrap_or_default();
                self.selected_game = Some(game.id);
                self.status = tr(
                    self.language,
                    "Jeu ajouté à la bibliothèque.",
                    "Game added to your library.",
                )
                .into();
            }
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
                    if self.preferred_emulator.is_none() {
                        self.preferred_emulator = Some(emulator.id.clone());
                        let _ = store.set_preference("preferred_emulator", &emulator.id);
                    }
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
        for game in &self.games {
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
                    ui.label(RichText::new("RustyShelf").size(21.0).strong());
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
                                self.games.iter().filter(|game| game.favorite).count(),
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
                                    self.selected_category == key,
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
            .unwrap_or(if self.theme == Theme::Classic { 0 } else { 12 });
        let gap = self
            .active_style()
            .and_then(|style| style.gap)
            .unwrap_or(12.0);
        let mut filtered: Vec<Game> = self
            .games
            .iter()
            .filter(|game| {
                self.matches_category(game)
                    && self
                        .filters
                        .matches(game, &self.search, self.game_console(game))
            })
            .cloned()
            .collect();
        filtered.sort_by(|left, right| {
            let name = left.title.to_lowercase().cmp(&right.title.to_lowercase());
            match self.filters.sort {
                Sort::Name => name,
                Sort::ReverseName => name.reverse(),
                Sort::Favorites => right.favorite.cmp(&left.favorite).then(name),
            }
        });
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
                other => other.strip_prefix("console:").unwrap_or(other),
            };
            ui.label(RichText::new(title).size(28.0).strong().color(text));
            ui.label(
                RichText::new(format!(
                    "{} {} · {}",
                    filtered.len(),
                    tr(self.language, "résultat(s)", "result(s)"),
                    tr(
                        self.language,
                        "Votre collection, sur votre ordinateur.",
                        "Your collection, on your computer."
                    )
                ))
                .size(12.0)
                .color(muted),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .desired_width(210.0)
                        .hint_text(tr(
                            self.language,
                            "Rechercher un jeu ou une console…",
                            "Search a game or console…",
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
            ui.add_space(8.0);
            if self.scan_control.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "{} {}",
                        self.scan_visited,
                        tr(self.language, "éléments parcourus", "items scanned")
                    ));
                });
            }
            ui.separator();
            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(55.0);
                    ui.label(RichText::new("▦").size(42.0).color(accent));
                    ui.label(
                        RichText::new(tr(
                            self.language,
                            "Une place pour chaque aventure",
                            "A place for every adventure",
                        ))
                        .size(20.0)
                        .color(text),
                    );
                    ui.label(tr(
                        self.language,
                        "Aucun jeu dans cette vue. Ajuste les filtres ou ajoute un jeu.",
                        "No games in this view. Adjust filters or add a game.",
                    ));
                    if ui
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
                    if self.games.iter().any(|game| !game.confirmed)
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
                                    let painter = ui.painter();
                                    painter.rect_filled(rect, radius, card);
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
                                    if let Some(texture) = texture {
                                        painter.image(
                                            texture.id(),
                                            cover,
                                            egui::Rect::from_min_max(
                                                egui::Pos2::ZERO,
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        painter.rect_filled(
                                            cover,
                                            radius.min(8),
                                            placeholder_color(self.theme),
                                        );
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
        ui.label(RichText::new(&game.title).size(21.0).strong());
        let category = if game.category == "À confirmer" {
            tr(self.language, "À confirmer", "To review")
        } else {
            &game.category
        };
        ui.label(RichText::new(category).color(Color32::GRAY));
        ui.add_space(10.0);
        if !game.confirmed
            && ui
                .button(tr(
                    self.language,
                    "Ce n’est pas un jeu · Ignorer",
                    "Not a game · Ignore",
                ))
                .clicked()
        {
            if let Some(store) = &self.store {
                match store.ignore_game(&game.id) {
                    Ok(()) => {
                        self.games.retain(|item| item.id != game.id);
                        self.selected_game = None;
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            return;
        }
        if !game.confirmed
            && ui
                .button(tr(
                    self.language,
                    "✓ Confirmer ce jeu",
                    "✓ Confirm this game",
                ))
                .clicked()
        {
            if let Some(store) = &self.store {
                let _ = store.set_confirmed(&game.id, true);
            }
            if let Some(item) = self.games.iter_mut().find(|item| item.id == game.id) {
                item.confirmed = true;
            }
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
            if let Some(store) = &self.store {
                let _ = store.set_favorite(&game.id, !game.favorite);
            }
            if let Some(item) = self.games.iter_mut().find(|item| item.id == game.id) {
                item.favorite = !game.favorite;
            }
        }
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
                if let Some(store) = &self.store {
                    let _ = store.set_category(&game.id, self.category_edit.trim());
                    self.games = store.games().map(deduplicate_library).unwrap_or_default();
                }
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
                if let Some(store) = &self.store {
                    let _ = store.set_cover(&game.id, Some(&path));
                    self.games = store.games().map(deduplicate_library).unwrap_or_default();
                }
                self.cover_textures.remove(&game.id);
            }
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(tr(self.language, "INSTALLATIONS", "INSTALLATIONS"))
                .size(11.0)
                .color(Color32::GRAY),
        );
        let mut launch_profile: Option<LaunchProfile> = None;
        let mut launch_uri: Option<String> = None;
        let mut launch_image: Option<(PathBuf, PathBuf)> = None;
        let is_disc_game = self.game_console(&game).is_some();
        if is_disc_game {
            let current_console = self.console_overrides.get(&game.id).cloned();
            let mut chosen_console = current_console.clone();
            egui::ComboBox::from_id_salt(("rom-console", &game.id))
                .selected_text(self.game_console(&game).unwrap_or("Console à préciser"))
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
                if let Some(console) = chosen_console {
                    self.console_overrides.insert(game.id.clone(), console);
                } else {
                    self.console_overrides.remove(&game.id);
                }
                self.save_mapping("console_overrides", self.console_overrides.clone());
            }
            let current_override = self
                .store
                .as_ref()
                .and_then(|store| store.game_emulator(&game.id).ok())
                .flatten();
            let default_name = self
                .default_emulator(&game)
                .and_then(|id| self.emulators.iter().find(|emulator| &emulator.id == id))
                .map(|emulator| emulator.name.as_str())
                .unwrap_or(tr(
                    self.language,
                    "Aucun émulateur par défaut",
                    "No default emulator",
                ));
            let mut chosen = current_override.clone();
            egui::ComboBox::from_id_salt(("game-emulator", &game.id))
                .selected_text(
                    current_override
                        .as_ref()
                        .and_then(|id| {
                            self.emulators
                                .iter()
                                .find(|emulator| &emulator.id == id)
                                .map(|emulator| emulator.name.clone())
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "{}: {default_name}",
                                tr(self.language, "Par défaut", "Default")
                            )
                        }),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut chosen,
                        None,
                        format!(
                            "{}: {default_name}",
                            tr(self.language, "Par défaut", "Default")
                        ),
                    );
                    for emulator in &self.emulators {
                        ui.selectable_value(&mut chosen, Some(emulator.id.clone()), &emulator.name);
                    }
                });
            if chosen != current_override {
                if let Some(store) = &self.store {
                    let _ = store.set_game_emulator(&game.id, chosen.as_deref());
                }
            }
        }
        for installation in &game.installations {
            ui.separator();
            ui.label(launcher_label(&installation.launcher, self.language));
            ui.label(installation.path.display().to_string());
            ui.label(if installation.available {
                tr(self.language, "Disponible", "Available")
            } else {
                tr(self.language, "Disque indisponible", "Drive unavailable")
            });
            if crate::rom::console(&installation.path).is_some() && installation.available {
                if ui
                    .button(tr(
                        self.language,
                        "▶  Lancer avec l’émulateur",
                        "▶  Launch in emulator",
                    ))
                    .clicked()
                {
                    let emulator_id = self
                        .store
                        .as_ref()
                        .and_then(|store| store.game_emulator(&game.id).ok())
                        .flatten()
                        .or_else(|| self.default_emulator(&game).cloned());
                    if let Some(emulator) = emulator_id
                        .and_then(|id| self.emulators.iter().find(|emulator| emulator.id == id))
                    {
                        launch_image =
                            Some((emulator.executable.clone(), installation.path.clone()));
                    } else {
                        self.status = tr(
                            self.language,
                            "Choisis un émulateur dans les paramètres.",
                            "Choose an emulator in Settings.",
                        )
                        .into();
                    }
                }
            } else if (installation.launch_profile.is_some()
                || installation.launcher == Launcher::Steam)
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
            if ui
                .button(tr(self.language, "Ouvrir le dossier", "Open folder"))
                .clicked()
            {
                let folder = if installation.path.is_file() {
                    installation.path.parent().unwrap_or(&installation.path)
                } else {
                    &installation.path
                };
                if let Err(error) = open::that(folder) {
                    self.status = error.to_string();
                }
            }
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
    }

    fn load_cover(&mut self, ctx: &egui::Context, game: &Game) -> Option<TextureHandle> {
        if let Some(texture) = self.cover_textures.get(&game.id) {
            return Some(texture.clone());
        }
        let path = game.cover_path.as_ref()?;
        let image = image::open(path).ok()?.thumbnail(500, 300).to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        if self.cover_textures.len() >= 128 {
            if let Some(id) = self.cover_textures.keys().next().cloned() {
                self.cover_textures.remove(&id);
            }
        }
        let texture = ctx.load_texture(game.id.clone(), color, egui::TextureOptions::LINEAR);
        self.cover_textures.insert(game.id.clone(), texture.clone());
        Some(texture)
    }

    fn options_window(&mut self, ctx: &egui::Context) {
        let mut close = false;
        egui::Window::new(tr(self.language, "Options", "Settings"))
            .collapsible(false)
            .resizable(true)
            .default_width(610.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(tr(self.language, "Apparence et langue", "Appearance and language"));
                let previous_language = self.language;
                egui::ComboBox::from_label(tr(self.language, "Langue", "Language"))
                    .selected_text(match self.language { Language::French => "Français", Language::English => "English" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.language, Language::French, "Français");
                        ui.selectable_value(&mut self.language, Language::English, "English");
                    });
                if previous_language != self.language {
                    if let Some(store) = &self.store { let _ = store.set_preference("language", self.language.key()); }
                }
                let previous_theme = self.theme;
                egui::ComboBox::from_label(tr(self.language, "Thème", "Theme"))
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
                    self.refresh_appearance(ctx);
                    if let Some(store) = &self.store { let _ = store.set_preference("theme", self.theme.key()); }
                }
                ui.separator();
                ui.heading(tr(self.language, "Style CSS personnalisé", "Custom CSS style"));
                ui.label(tr(self.language, "Variables de couleurs, arrondis et espacement. Fichier local, rechargement manuel.", "Color, radius and spacing variables. Local file, manual reload."));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr(self.language, "Importer un fichier CSS…", "Import CSS file…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("CSS", &["css"]).pick_file() {
                            self.css_path = Some(path);
                            self.load_css(ctx);
                        }
                    }
                    if ui.add_enabled(self.css_path.is_some(), egui::Button::new(tr(self.language, "Recharger", "Reload"))).clicked() { self.load_css(ctx); }
                    if ui.button(tr(self.language, "Enregistrer le modèle…", "Save template…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_file_name("rustyshelf.css").add_filter("CSS", &["css"]).save_file() {
                            match std::fs::write(&path, crate::style::EXAMPLE) {
                                Ok(()) => { self.css_path = Some(path); self.load_css(ctx); }
                                Err(error) => self.css_error = Some(error.to_string()),
                            }
                        }
                    }
                });
                if ui.checkbox(&mut self.css_enabled, tr(self.language, "Activer le style CSS", "Enable CSS style")).changed() {
                    if self.css_enabled && self.custom_style.is_none() { self.load_css(ctx); }
                    self.refresh_appearance(ctx);
                    if let Some(store) = &self.store { let _ = store.set_preference("css_enabled", if self.css_enabled { "true" } else { "false" }); }
                }
                if let Some(path) = &self.css_path { ui.label(path.display().to_string()); }
                if let Some(error) = &self.css_error { ui.colored_label(Color32::from_rgb(225, 92, 85), error); }
                ui.small(tr(self.language, "Syntaxe : :root { --accent: #e5a270; --radius: 12px; } · CSS complet non pris en charge.", "Syntax: :root { --accent: #e5a270; --radius: 12px; } · Full CSS is not supported."));
                ui.separator();
                ui.heading(tr(self.language, "Émulation", "Emulation"));
                let previous_emulator = self.preferred_emulator.clone();
                egui::ComboBox::from_label(tr(self.language, "Émulateur préféré par défaut", "Default preferred emulator"))
                    .selected_text(self.preferred_emulator.as_ref().and_then(|id| self.emulators.iter().find(|emu| &emu.id == id).map(|emu| emu.name.clone())).unwrap_or_else(|| tr(self.language, "À choisir", "Choose one").into()))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.preferred_emulator, None, tr(self.language, "Aucun", "None"));
                        for emulator in &self.emulators { ui.selectable_value(&mut self.preferred_emulator, Some(emulator.id.clone()), &emulator.name); }
                    });
                if previous_emulator != self.preferred_emulator {
                    if let Some(store) = &self.store { let _ = store.set_preference("preferred_emulator", self.preferred_emulator.as_deref().unwrap_or("")); }
                }
                if ui.button(tr(self.language, "＋ Ajouter un émulateur…", "＋ Add emulator…")).clicked() { self.add_emulator(); }
                let consoles: std::collections::BTreeSet<String> = self.games.iter().filter_map(|game| self.game_console(game).map(str::to_owned)).collect();
                let previous_console_emulators = self.console_emulators.clone();
                for console in consoles {
                    let mut chosen = self.console_emulators.get(&console).cloned();
                    egui::ComboBox::from_id_salt(("console-emulator", &console))
                        .selected_text(format!("{} : {}", console, chosen.as_ref().and_then(|id| self.emulators.iter().find(|emulator| &emulator.id == id)).map(|emulator| emulator.name.as_str()).unwrap_or(tr(self.language, "Défaut général", "Global default"))))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut chosen, None, tr(self.language, "Défaut général", "Global default"));
                            for emulator in &self.emulators { ui.selectable_value(&mut chosen, Some(emulator.id.clone()), &emulator.name); }
                        });
                    if let Some(id) = chosen { self.console_emulators.insert(console, id); } else { self.console_emulators.remove(&console); }
                }
                if previous_console_emulators != self.console_emulators { self.save_mapping("console_emulators", self.console_emulators.clone()); }
                if self.emulators.is_empty() {
                    ui.label(tr(self.language, "Aucun émulateur détecté. Ajoute son exécutable ci-dessus.", "No emulator found. Add its executable above."));
                } else {
                    for emulator in &self.emulators { ui.label(format!("{} · {}", emulator.name, emulator.executable.display())); }
                }
                ui.separator();
                ui.heading(tr(self.language, "Scanner", "Scanner"));
                if ui.add_enabled(self.scan_control.is_none(), egui::Button::new(tr(self.language, "Actualiser les disques", "Refresh drives"))).clicked() {
                    let selected: Vec<PathBuf> = self.volumes.iter().zip(&self.selected_volumes).filter(|(_, selected)| **selected).map(|(volume, _)| volume.root.clone()).collect();
                    self.volumes = scan::mounted_volumes();
                    self.selected_volumes = self.volumes.iter().map(|volume| selected.contains(&volume.root)).collect();
                }
                if ui.button(tr(self.language, "Réafficher les candidats ignorés", "Restore ignored candidates")).clicked() {
                    if let Some(store) = &self.store {
                        match store.restore_ignored_games().and_then(|_| store.games()) {
                            Ok(games) => self.games = deduplicate_library(games),
                            Err(error) => self.status = error.to_string(),
                        }
                    }
                }
                ui.label(tr(self.language, "Choisis les disques montés à analyser. Les jeux et images disque trouvés restent locaux.", "Choose mounted drives to scan. Games and disc images stay on this device."));
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (index, volume) in self.volumes.iter().enumerate() {
                            let selected = self.selected_volumes.get_mut(index).unwrap();
                            ui.checkbox(
                                selected,
                                format!(
                                    "{}  ·  {}  ·  {}{}",
                                    volume.root.display(),
                                    volume.name,
                                    volume.filesystem,
                                    if volume.removable {
                                        tr(self.language, "  · amovible", "  · removable")
                                    } else {
                                        ""
                                    }
                                ),
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button(tr(self.language, "＋ Exclure un dossier…", "＋ Exclude folder…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            if let Some(store) = &self.store {
                                let _ = store.add_exclusion(&path);
                            }
                            if !self.exclusions.contains(&path) {
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
                        let path = self.exclusions.remove(index);
                        if let Some(store) = &self.store {
                            let _ = store.remove_exclusion(&path);
                        }
                    }
                }
                if self.scan_control.is_some() {
                    ui.label(format!("{} {} · {}", self.scan_visited, tr(self.language, "éléments parcourus", "items scanned"), self.scan_current));
                }
                if !self.errors.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            self.errors.len(),
                            tr(self.language, "avertissement(s)", "warning(s)")
                        ))
                            .color(Color32::YELLOW),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for error in &self.errors {
                                ui.label(error);
                            }
                        });
                }
                ui.separator();
                ui.heading(tr(self.language, "Données et sauvegarde", "Data and backup"));
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
        if close {
            self.show_options = false;
        }
    }
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
        Theme::Prism => Color32::from_rgba_unmultiplied(66, 85, 119, 120),
    }
}

fn theme_accent(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(229, 162, 112),
        Theme::Light => Color32::from_rgb(226, 221, 249),
        Theme::Breeze => Color32::from_rgb(190, 220, 244),
        Theme::Horizon => Color32::from_rgb(49, 106, 197),
        Theme::Classic => Color32::from_rgb(0, 0, 128),
        Theme::Prism => Color32::from_rgba_unmultiplied(112, 145, 199, 150),
    }
}

fn placeholder_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(51, 47, 77),
        Theme::Light => Color32::from_rgb(220, 216, 239),
        Theme::Breeze => Color32::from_rgb(170, 204, 229),
        Theme::Horizon => Color32::from_rgb(190, 218, 250),
        Theme::Classic => Color32::from_rgb(192, 192, 192),
        Theme::Prism => Color32::from_rgba_unmultiplied(121, 151, 198, 140),
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
            || (normalized_title(&current.title) == normalized_title(&incoming.title)
                && crate::rom::game_console(current) == crate::rom::game_console(&incoming))
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

fn normalized_title(title: &str) -> String {
    title
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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
    if uri.starts_with("lutris:") {
        Command::new("lutris").arg(uri).spawn().map(|_| ())
    } else if uri.starts_with("heroic://") {
        #[cfg(target_os = "linux")]
        {
            Command::new("heroic")
                .arg(uri)
                .arg("--no-gui")
                .spawn()
                .map(|_| ())
        }
        #[cfg(not(target_os = "linux"))]
        {
            open::that(uri)
        }
    } else {
        open::that(uri)
    }
}

impl eframe::App for RustyShelfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan(ctx);
        self.sidebar(ctx);
        self.content(ctx);
        if self.show_options {
            self.options_window(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_and_options_render_at_compact_and_wide_sizes() {
        let ctx = egui::Context::default();
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
        let mut app = RustyShelfApp {
            selected_category: "ROMs".into(),
            search: String::new(),
            show_options: false,
            games,
            selected_game: None,
            store: None,
            status: "Hors ligne".into(),
            volumes: vec![],
            selected_volumes: vec![],
            exclusions: vec![],
            scan_control: None,
            scan_receiver: None,
            scan_visited: 0,
            scan_current: String::new(),
            scan_roots: vec![],
            scan_exclusions: vec![],
            scan_seen_paths: vec![],
            errors: vec![],
            cover_textures: HashMap::new(),
            category_edit: String::new(),
            category_edit_id: None,
            language: Language::French,
            theme: Theme::Dark,
            emulators: vec![],
            preferred_emulator: None,
            css_path: None,
            css_enabled: true,
            custom_style: Some(CustomStyle::parse(crate::style::EXAMPLE).unwrap()),
            css_error: None,
            filters: Filters::default(),
            console_overrides: HashMap::new(),
            console_emulators: HashMap::new(),
        };
        app.refresh_appearance(&ctx);
        for width in [780.0, 1280.0] {
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
