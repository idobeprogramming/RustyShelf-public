# Consignes RustyShelf

- Interface et messages visibles en français ; le code reste en anglais.
- Chaque fonctionnalité est petite, testée, puis commitée séparément.
- Toute détection ou lecture de jaquette doit rester locale et hors ligne.
- Ne jamais supprimer les corrections utilisateur lorsqu'un scan est relancé.
- Lancer `cargo fmt --check`, `cargo test` et `cargo clippy --all-targets -- -D warnings` avant un commit.
- Les tests ne lancent jamais un jeu réel et utilisent des répertoires temporaires.
