# GameShelf

GameShelf est une bibliothèque de jeux de bureau écrite en Rust. Elle rassemble les jeux trouvés sur les volumes montés, présente leurs jaquettes enregistrées localement et permet de les lancer. La recherche et l’interface fonctionnent hors ligne.

## Utilisation

- **Scanner les disques** : choisir les volumes et les dossiers à exclure dans les paramètres. Le scan affiche sa progression, peut être annulé et conserve les jeux d’un disque débranché.
- **Sources reconnues** : manifestes Steam, Epic Games, Heroic et Lutris, ainsi que les dossiers de jeux GOG. Les exécutables inconnus sont marqués « À confirmer ».
- **Organiser** : rechercher, filtrer par source, modifier une catégorie, marquer un favori, ajouter un jeu et choisir une image locale.
- **Lancer** : utiliser le lien Steam, Heroic ou Lutris disponible, ou lancer directement le chemin enregistré. Sous Linux, Proton/Wine doit déjà être configuré dans le lanceur.

Les noms et manifestes des lanceurs varient selon leur version. Les jeux autonomes non détectés peuvent être ajoutés avec le bouton **＋ Jeu**. GameShelf ne télécharge pas de jaquettes et ne contacte aucun service distant.

## Windows

Installer Rust stable et les outils de compilation C++ de Visual Studio, puis :

```powershell
cargo run
```

Pour créer une archive de distribution, exécuter `.uild-windows.ps1`.

## CachyOS et autres distributions Linux

Installer Rust avec `rustup`, puis les bibliothèques de développement nécessaires à Wayland/X11 :

```bash
sudo pacman -S --needed base-devel pkgconf gtk3 libxkbcommon wayland libx11 libxrandr libxcursor libxi libxinerama dbus mesa
cargo run
```

Pour créer l’archive Linux : `bash build-linux.sh`. Extraire `dist/GameShelf-linux-x86_64.tar.gz`, puis lancer `./gameshelf`. Le fichier `gameshelf.desktop` peut être copié vers `~/.local/share/applications/` après avoir installé l’exécutable dans un dossier présent dans le `PATH`.

Les bibliothèques graphiques fournies par la distribution restent nécessaires. La compilation Windows est vérifiée dans CI ; pour la prise en charge de CachyOS, compiler et vérifier l’interface sur une machine CachyOS avec session Wayland ou X11.

## Données

La bibliothèque SQLite est enregistrée dans le dossier de données local de l’utilisateur sous `GameShelf/`. Les chemins d’installation et de jaquette sont stockés localement. Aucun scan ne monte de disque ou ne modifie les fichiers de jeu.

## Vérifications

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Voir [TASKS.md](TASKS.md) pour le découpage en tâches de Luna.
