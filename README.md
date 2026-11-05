# GameShelf

GameShelf est une bibliothèque de jeux de bureau écrite en Rust. Elle rassemble les jeux trouvés sur les volumes montés, présente leurs jaquettes enregistrées localement et permet de les lancer. La recherche et l’interface fonctionnent hors ligne.

## Utilisation

- **Scanner les disques** : choisir les volumes et les dossiers à exclure dans les paramètres. Le scan affiche sa progression, peut être annulé et conserve les jeux d’un disque débranché. Les exécutables détectés sont affichés dans « À confirmer » lorsqu’aucun lanceur ne permet de reconnaître leur titre.
- **Sources reconnues** : manifestes Steam, Epic Games, Heroic et Lutris, ainsi que les dossiers de jeux GOG.
- **Organiser** : rechercher, filtrer par source, modifier une catégorie, marquer un favori, ajouter un jeu et choisir une image locale.
- **Lancer** : utiliser le lien Steam, Heroic ou Lutris disponible, ou lancer directement le chemin enregistré. Sous Linux, Proton/Wine doit déjà être configuré dans le lanceur.
- **Images disque** : les fichiers ISO, CHD, CSO, WBFS, GCZ, CUE, PBP, CDI, NRG et MDF apparaissent dans la file « À confirmer ». Choisir un émulateur par défaut dans les options ou un émulateur propre à chaque jeu, puis le lancer depuis sa fiche. GameShelf transmet le chemin de l’image à l’émulateur ; celui-ci doit être installé et compatible avec le format.
- **Personnaliser** : options de langue français/anglais et thèmes sombre, clair, Windows 98, Windows XP, Windows Vista et Liquid Glass. Le thème Liquid Glass dessine des panneaux translucides dans l’application.

La bibliothèque principale affiche les jeux reconnus et confirmés. Les exécutables autonomes doivent présenter des fichiers de jeu (Unity, Unreal, Godot, GameMaker, Ren’Py ou RPG Maker) pour être proposés dans **À confirmer**. Les raccourcis Linux doivent porter la catégorie `Game`. Les anciens résultats incertains restent dans cette section. Le bouton **Ce n’est pas un jeu · Ignorer** masque durablement un candidat ; les options permettent de réafficher les candidats ignorés.

Pour éviter les images d’installation système, les fichiers ISO doivent se trouver dans un dossier nommé `Games`, `Jeux`, `ROMs`, `ISOs`, `Emulation`, `PS2`, `PSP`, `Wii` ou `GameCube`. Ils restent à confirmer avant de rejoindre la bibliothèque.

Les noms et manifestes des lanceurs varient selon leur version. Les jeux autonomes sans signature reconnue peuvent être ajoutés avec le bouton **＋ Jeu**. GameShelf ne télécharge pas de jaquettes et ne contacte aucun service distant.

## Windows

Installer Rust stable et les outils de compilation C++ de Visual Studio, puis :

```powershell
cargo run
```

Pour créer une archive de distribution, exécuter `.\build-windows.ps1`.

## CachyOS et autres distributions Linux

Installer Rust avec `rustup`, puis les bibliothèques de développement nécessaires à Wayland/X11 :

```bash
sudo pacman -S --needed base-devel pkgconf gtk3 libxkbcommon wayland libx11 libxrandr libxcursor libxi libxinerama dbus mesa
cargo run
```

Pour créer l’archive Linux : `bash build-linux.sh`. Extraire `dist/GameShelf-linux-x86_64.tar.gz`, puis lancer `./gameshelf`. Le fichier `gameshelf.desktop` peut être copié vers `~/.local/share/applications/` après avoir installé l’exécutable dans un dossier présent dans le `PATH`.

Les bibliothèques graphiques fournies par la distribution restent nécessaires. Le pipeline CI vérifie formatage, tests et Clippy, puis produit des archives Windows et Linux à chaque changement poussé ou proposition de fusion. Elles sont téléchargeables comme artefacts de l’exécution CI. Pour valider CachyOS graphiquement, compiler et ouvrir l’application sur une machine CachyOS en session Wayland ou X11.

## Données

La bibliothèque SQLite est enregistrée dans le dossier de données local de l’utilisateur sous `GameShelf/`. Les chemins d’installation et de jaquette sont stockés localement. Aucun scan ne monte de disque ou ne modifie les fichiers de jeu.

## Vérifications

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Voir [TASKS.md](TASKS.md) pour le découpage en tâches de Luna.
