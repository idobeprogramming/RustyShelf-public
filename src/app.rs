use crate::{
    model::{Game, Installation, LaunchProfile, Launcher},
    scan::{self, ScanControl, ScanEvent, Volume},
    sources,
    store::Store,
};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver},
};

pub struct GameShelfApp {
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
}

impl GameShelfApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(17, 19, 26);
        visuals.window_fill = Color32::from_rgb(24, 27, 36);
        cc.egui_ctx.set_visuals(visuals);
        let store = Store::open_default().ok();
        let mut status = String::from("Hors ligne · Données locales");
        let games = store
            .as_ref()
            .and_then(|s| s.games().ok())
            .unwrap_or_default();
        let exclusions = store
            .as_ref()
            .and_then(|s| s.exclusions().ok())
            .unwrap_or_default();
        if store.is_none() {
            status = "Base locale indisponible".into();
        }
        let volumes = scan::mounted_volumes();
        let selected_volumes = vec![true; volumes.len()];
        Self {
            selected_category: "Tous les jeux".into(),
            search: String::new(),
            show_options: false,
            selected_game: games.first().map(|game| game.id.clone()),
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
            self.status = "Sélectionnez au moins un disque à scanner.".into();
            return;
        }
        let exclusions = self.exclusions.clone();
        self.scan_roots = roots.clone();
        self.scan_exclusions = exclusions.clone();
        self.scan_seen_paths.clear();
        let control = ScanControl::default();
        let worker_control = control.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for game in sources::discover(&roots) {
                if worker_control.is_cancelled() {
                    break;
                }
                if sender.send(ScanEvent::Candidate(game)).is_err() {
                    return;
                }
            }
            scan::scan_roots(&roots, &exclusions, &worker_control, |event| {
                let _ = sender.send(event);
            });
        });
        self.scan_control = Some(control);
        self.scan_receiver = Some(receiver);
        self.scan_visited = 0;
        self.errors.clear();
        self.status = "Analyse des disques en cours…".into();
        self.show_options = false;
    }

    fn poll_scan(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.scan_receiver else {
            return;
        };
        let events: Vec<_> = receiver.try_iter().collect();
        for event in events {
            match event {
                ScanEvent::Candidate(game) => {
                    self.scan_seen_paths.extend(
                        game.installations
                            .iter()
                            .map(|installation| installation.path.clone()),
                    );
                    if let Some(store) = &self.store {
                        if let Err(error) = store.save_game(&game) {
                            self.errors.push(error.to_string());
                        }
                        self.games = store.games().unwrap_or_else(|_| self.games.clone());
                    }
                }
                ScanEvent::Progress(progress) => {
                    self.scan_visited = progress.visited;
                    self.scan_current = progress.current.to_string_lossy().to_string();
                }
                ScanEvent::Error(path, message) => {
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
                            self.games = store.games().unwrap_or_else(|_| self.games.clone());
                        }
                    }
                    self.scan_control = None;
                    self.scan_receiver = None;
                    self.status = if cancelled {
                        format!("Scan arrêté · {visited} éléments parcourus")
                    } else {
                        format!(
                            "Scan terminé · {} jeux · {visited} éléments parcourus",
                            self.games.len()
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
            .pick_file()
        else {
            return;
        };
        let title = executable
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Nouveau jeu".into());
        let install_path = executable.parent().unwrap_or(&executable).to_path_buf();
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
                launch_profile: Some(profile),
                available: true,
            },
        );
        if let Some(store) = &self.store {
            if store.save_game(&game).is_ok() {
                self.games = store.games().unwrap_or_default();
                self.selected_game = Some(game.id);
                self.status = "Jeu ajouté à la bibliothèque.".into();
            }
        }
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("▣")
                            .size(26.0)
                            .color(Color32::from_rgb(135, 117, 255)),
                    );
                    ui.label(RichText::new("GAMESHELF").size(18.0).strong());
                });
                ui.add_space(25.0);
                ui.label(
                    RichText::new("BIBLIOTHÈQUE")
                        .size(11.0)
                        .color(Color32::GRAY),
                );
                ui.add_space(8.0);
                for (name, icon) in [
                    ("Tous les jeux", "▦"),
                    ("Favoris", "♡"),
                    ("Installés", "⬇"),
                    ("À confirmer", "?"),
                ] {
                    if ui
                        .selectable_label(
                            self.selected_category == name,
                            format!("{icon}   {name}"),
                        )
                        .clicked()
                    {
                        self.selected_category = name.into();
                    }
                }
                ui.add_space(18.0);
                ui.label(RichText::new("SOURCES").size(11.0).color(Color32::GRAY));
                ui.add_space(8.0);
                for name in [
                    "Steam",
                    "Epic Games",
                    "Heroic",
                    "Lutris",
                    "GOG",
                    "Ajout manuel",
                ] {
                    if ui
                        .selectable_label(self.selected_category == name, format!("   ·   {name}"))
                        .clicked()
                    {
                        self.selected_category = name.into();
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if ui.button("⚙  Paramètres").clicked() {
                        self.show_options = true;
                    }
                    ui.label(RichText::new(&self.status).size(10.0).color(Color32::GRAY));
                });
            });
    }

    fn content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.selected_category).size(25.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = if self.scan_control.is_some() { "■  Arrêter le scan" } else { "⌕  Scanner les disques" };
                    if ui.button(button).clicked() { if self.scan_control.is_some() { if let Some(control) = &self.scan_control { control.cancel(); } } else { self.show_options = true; } }
                    if ui.button("＋ Jeu").clicked() { self.add_manual_game(); }
                    ui.add_sized([220.0, 30.0], egui::TextEdit::singleline(&mut self.search).hint_text("Rechercher un jeu…"));
                });
            });
            ui.add_space(12.0);
            if self.scan_control.is_some() {
                ui.horizontal(|ui| { ui.spinner(); ui.label(format!("{} éléments parcourus · {}", self.scan_visited, self.scan_current)); });
            }
            ui.separator();
            let mut filtered: Vec<Game> = self.games.iter().filter(|game| self.matches_category(game) && game.title.to_lowercase().contains(&self.search.to_lowercase())).cloned().collect();
            filtered.sort_by_key(|game| game.title.to_lowercase());
            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(95.0);
                    ui.label(RichText::new("▧").size(40.0).color(Color32::from_rgb(118, 105, 209)));
                    ui.label(RichText::new("Votre collection commence ici").size(20.0).strong());
                    ui.label("Scannez les disques pour retrouver vos jeux, ou ajoutez-en un manuellement.");
                    ui.add_space(10.0);
                    if ui.button("⌕   Scanner mes disques").clicked() { self.show_options = true; }
                    if ui.button("＋   Ajouter un jeu").clicked() { self.add_manual_game(); }
                });
                return;
            }
            ui.columns(2, |columns| {
                let (list, details) = columns.split_at_mut(1);
                egui::ScrollArea::vertical().show(&mut list[0], |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for game in &filtered {
                            egui::Frame::new().fill(Color32::from_rgb(24, 27, 36)).corner_radius(10.0).inner_margin(8.0).show(ui, |ui| {
                                ui.set_width(170.0);
                                let texture = self.load_cover(ctx, game);
                                if let Some(texture) = texture { ui.add(egui::Image::new((texture.id(), Vec2::new(154.0, 96.0))).corner_radius(6.0)); }
                                else { egui::Frame::new().fill(Color32::from_rgb(51, 47, 77)).corner_radius(6.0).show(ui, |ui| { ui.set_min_size(Vec2::new(154.0, 96.0)); ui.centered_and_justified(|ui| { ui.label(RichText::new(game.title.chars().take(2).collect::<String>().to_uppercase()).size(26.0).strong()); }); }); }
                                let star = if game.favorite { "★ " } else { "" };
                                if ui.selectable_label(self.selected_game.as_deref() == Some(&game.id), format!("{star}{}", game.title)).clicked() { self.selected_game = Some(game.id.clone()); }
                                ui.label(RichText::new(game.category.clone()).size(11.0).color(Color32::GRAY));
                            });
                        }
                    });
                });
                egui::Frame::new().fill(Color32::from_rgb(21, 24, 32)).corner_radius(10.0).inner_margin(16.0).show(&mut details[0], |ui| self.details_panel(ui, &filtered));
            });
        });
    }

    fn matches_category(&self, game: &Game) -> bool {
        match self.selected_category.as_str() {
            "Tous les jeux" => true,
            "Favoris" => game.favorite,
            "Installés" => game
                .installations
                .iter()
                .any(|installation| installation.available),
            "À confirmer" => !game.confirmed,
            category => game.category.eq_ignore_ascii_case(category),
        }
    }

    fn details_panel(&mut self, ui: &mut egui::Ui, games: &[Game]) {
        let Some(id) = self.selected_game.clone() else {
            ui.label("Sélectionnez un jeu pour afficher ses détails.");
            return;
        };
        let Some(game) = games.iter().find(|game| game.id == id).cloned() else {
            ui.label("Sélectionnez un jeu pour afficher ses détails.");
            return;
        };
        ui.label(RichText::new(&game.title).size(21.0).strong());
        ui.label(RichText::new(&game.category).color(Color32::GRAY));
        ui.add_space(10.0);
        if ui
            .button(if game.favorite {
                "★  Retirer des favoris"
            } else {
                "☆  Ajouter aux favoris"
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
            ui.label("Catégorie");
            if self.category_edit_id.as_deref() != Some(&game.id) {
                self.category_edit = game.category.clone();
                self.category_edit_id = Some(game.id.clone());
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.category_edit).desired_width(120.0));
            if ui.button("Enregistrer").clicked() && !self.category_edit.trim().is_empty() {
                if let Some(store) = &self.store {
                    let _ = store.set_category(&game.id, self.category_edit.trim());
                    self.games = store.games().unwrap_or_default();
                }
            }
        });
        if ui.button("Choisir une vignette locale…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "ico"])
                .pick_file()
            {
                if let Some(store) = &self.store {
                    let _ = store.set_cover(&game.id, Some(&path));
                    self.games = store.games().unwrap_or_default();
                }
                self.cover_textures.remove(&game.id);
            }
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("INSTALLATIONS")
                .size(11.0)
                .color(Color32::GRAY),
        );
        let mut launch_path: Option<PathBuf> = None;
        let mut launch_uri: Option<String> = None;
        for installation in &game.installations {
            ui.separator();
            ui.label(installation.launcher.label());
            ui.label(installation.path.display().to_string());
            ui.label(if installation.available {
                "Disponible"
            } else {
                "Disque indisponible"
            });
            if (installation.launch_profile.is_some() || installation.launcher == Launcher::Steam)
                && installation.available
                && ui.button("▶  Jouer").clicked()
            {
                if let Some(profile) = &installation.launch_profile {
                    if let Some(uri) = &profile.launcher_command {
                        launch_uri = Some(uri.clone());
                    } else {
                        launch_path = Some(profile.command.clone());
                    }
                }
            }
            if ui.button("Ouvrir le dossier").clicked() {
                let _ = open::that(&installation.path);
            }
        }
        if let Some(uri) = launch_uri.filter(|uri| !uri.is_empty()) {
            launch_uri_handler(&uri);
        }
        if let Some(path) = launch_path {
            launch_executable(&path);
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
        let texture = ctx.load_texture(game.id.clone(), color, egui::TextureOptions::LINEAR);
        self.cover_textures.insert(game.id.clone(), texture.clone());
        Some(texture)
    }

    fn options_window(&mut self, ctx: &egui::Context) {
        let mut close = false;
        egui::Window::new("Options de scan")
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(
                    "Choisissez les disques montés à analyser. Tout le traitement reste local.",
                );
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
                                        "  · amovible"
                                    } else {
                                        ""
                                    }
                                ),
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("＋ Exclure un dossier…").clicked() {
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
                            "Arrêter le scan"
                        } else {
                            "Démarrer le scan"
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
                    if ui.button("Fermer").clicked() {
                        close = true;
                    }
                });
                if !self.exclusions.is_empty() {
                    ui.separator();
                    ui.label("Dossiers exclus");
                    let mut remove = None;
                    for (index, path) in self.exclusions.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(path.display().to_string());
                            if ui.small_button("Retirer").clicked() {
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
                    ui.label(format!(
                        "{} éléments parcourus · {}",
                        self.scan_visited, self.scan_current
                    ));
                }
                if !self.errors.is_empty() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} avertissement(s)", self.errors.len()))
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
            });
        if close {
            self.show_options = false;
        }
    }
}

fn launch_executable(path: &std::path::Path) {
    let mut command = Command::new(path);
    if let Some(parent) = path.parent() {
        command.current_dir(parent);
    }
    let _ = command.spawn();
}

fn launch_uri_handler(uri: &str) {
    if uri.starts_with("lutris:") {
        let _ = Command::new("lutris").arg(uri).spawn();
    } else if uri.starts_with("heroic://") {
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("heroic").arg(uri).arg("--no-gui").spawn();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = open::that(uri);
        }
    } else {
        let _ = open::that(uri);
    }
}

impl eframe::App for GameShelfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan(ctx);
        self.sidebar(ctx);
        self.content(ctx);
        if self.show_options {
            self.options_window(ctx);
        }
    }
}
