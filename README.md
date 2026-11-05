# GameShelf

Bibliothèque de jeux hors ligne en Rust pour Windows et Linux (dont CachyOS).

GameShelf réunit les jeux installés dans une bibliothèque locale. Il lit les fichiers des lanceurs et les dossiers accessibles, garde ses données sur l'ordinateur et n'effectue aucun accès réseau.

## Développement

Installer Rust stable, puis lancer `cargo run`. Sous Linux, les dépendances de fenêtre Wayland/X11 de `eframe` doivent être présentes.

Voir `TASKS.md` pour les petites étapes du projet. Le dépôt suit les consignes de `AGENTS.md`.
