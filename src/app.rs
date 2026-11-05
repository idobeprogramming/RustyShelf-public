use eframe::egui::{self, Color32, RichText, Vec2};

pub struct GameShelfApp {
    selected_category: String,
    search: String,
    show_options: bool,
}

impl GameShelfApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(17, 19, 26);
        visuals.window_fill = Color32::from_rgb(24, 27, 36);
        cc.egui_ctx.set_visuals(visuals);
        Self {
            selected_category: "Tous les jeux".into(),
            search: String::new(),
            show_options: false,
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
                    ("Récemment joués", "◷"),
                    ("Favoris", "♡"),
                    ("Installés", "⬇"),
                ] {
                    let selected = self.selected_category == name;
                    if ui
                        .selectable_label(selected, format!("{icon}   {name}"))
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
                    "Jeux indépendants",
                ] {
                    ui.label(format!("   ·   {name}"));
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if ui.button("⚙  Paramètres").clicked() {
                        self.show_options = true;
                    }
                    ui.label(
                        RichText::new("Hors ligne · Données locales")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                });
            });
    }

    fn content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.selected_category).size(25.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⌕  Scanner les disques").clicked() {
                        self.show_options = true;
                    }
                    ui.add_sized(
                        [220.0, 30.0],
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Rechercher un jeu…"),
                    );
                });
            });
            ui.add_space(22.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(26, 29, 40))
                .corner_radius(14.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.set_min_height(128.0);
                    ui.label(
                        RichText::new("VOTRE BIBLIOTHÈQUE, AU MÊME ENDROIT")
                            .size(11.0)
                            .color(Color32::from_rgb(168, 153, 255)),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("Retrouvez vos jeux").size(24.0).strong());
                    ui.label(
                        RichText::new("hors ligne, sur tous vos disques.")
                            .size(15.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Jeux récemment ajoutés").size(17.0).strong());
                ui.label(RichText::new("0 jeu").size(12.0).color(Color32::GRAY));
            });
            ui.add_space(12.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(21, 24, 32))
                .corner_radius(12.0)
                .inner_margin(28.0)
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 190.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("▧")
                                .size(34.0)
                                .color(Color32::from_rgb(118, 105, 209)),
                        );
                        ui.label(
                            RichText::new("Votre collection commence ici")
                                .size(18.0)
                                .strong(),
                        );
                        ui.label("Scannez vos disques pour retrouver automatiquement vos jeux.");
                        ui.add_space(10.0);
                        if ui.button("⌕   Scanner mes disques").clicked() {
                            self.show_options = true;
                        }
                    });
                });
        });
    }
}

impl eframe::App for GameShelfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sidebar(ctx);
        self.content(ctx);
        if self.show_options {
            egui::Window::new("Options de scan")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Le scanner parcourra les disques montés et accessibles.");
                    ui.label("Aucune image ne sera téléchargée : GameShelf reste hors ligne.");
                    if ui.button("Compris").clicked() {
                        self.show_options = false;
                    }
                });
        }
    }
}
