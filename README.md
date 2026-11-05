# RustyShelf

RustyShelf est une bibliothèque de jeux de bureau écrite en Rust. Elle rassemble les jeux trouvés sur les volumes montés, présente leurs jaquettes enregistrées localement et permet de les lancer. La recherche et l’interface fonctionnent hors ligne.

## Utilisation

- **Scanner les disques** : choisir les volumes et les dossiers à exclure dans les paramètres. Le scan affiche sa progression, peut être annulé et conserve les jeux d’un disque débranché. Les exécutables détectés sont affichés dans « À confirmer » lorsqu’aucun lanceur ne permet de reconnaître leur titre.
- **Sources reconnues** : manifestes Steam, Epic Games, Heroic et Lutris, ainsi que les dossiers de jeux GOG.
- **Organiser** : rechercher, filtrer par source, modifier une catégorie, marquer un favori, ajouter un jeu et choisir une image locale.
- **Lancer** : utiliser le lien Steam, Heroic ou Lutris disponible, ou lancer directement le chemin enregistré. Sous Linux, Proton/Wine doit déjà être configuré dans le lanceur.
- **Images disque** : les fichiers ISO, CHD, CSO, WBFS, GCZ, CUE, PBP, CDI, NRG et MDF apparaissent dans la file « À confirmer ». Choisir un émulateur par défaut dans les options ou un émulateur propre à chaque jeu, puis le lancer depuis sa fiche. RustyShelf transmet le chemin de l’image à l’émulateur ; celui-ci doit être installé et compatible avec le format.
- **Personnaliser** : options de langue français/anglais et thèmes Nocturne, Aurore, Rétro, Horizon, Brise et Prisme. Le thème Prisme dessine des panneaux translucides dans l’application.
- **Style CSS** : importer un fichier local dans les options, activer/désactiver les variables puis recharger après modification. Un modèle est fourni dans le paquet et peut être enregistré depuis l’application. Voir [CUSTOM_STYLE.md](CUSTOM_STYLE.md) pour la syntaxe prise en charge.
- **ROMs et consoles** : la section ROMs se construit à partir de la bibliothèque. NES, Super Nintendo, Game Boy, GBA, DS, Nintendo 64, Mega Drive, GameCube, Wii, PSP et plusieurs formats disque sont reconnus. Les disques ambigus sont classés à partir de leurs dossiers (`PS2`, `PS1`, `GameCube`, etc.) ou restent dans « Console à préciser ». La fiche permet de corriger la console ; ce choix est conservé après les scans.
- **Filtres communs** : recherche par titre/catégorie/console, type PC ou ROM, source, disponibilité et favoris. Le tri propose A → Z, Z → A ou les favoris en premier. Ces réglages s’appliquent à toutes les vues, y compris les consoles et catégories personnalisées. « Effacer » remet les filtres à zéro.
- **Émulateur par console** : les options permettent un défaut pour chacune des consoles présentes. Le choix d’un jeu est prioritaire, puis le défaut de sa console, puis le défaut général. L’émulateur doit accepter le chemin de ROM comme argument et être déjà configuré.
- **Sauvegarder et exporter** : les options permettent une copie SQLite de la bibliothèque complète (favoris, réglages, exclusions et choix d’émulateur) et un export JSON du catalogue. Les jaquettes et fichiers de jeu restent à leur emplacement ; ils ne sont pas copiés dans la sauvegarde.

La bibliothèque principale affiche les jeux reconnus et confirmés. Les exécutables autonomes doivent présenter des fichiers de jeu (Unity, Unreal, Godot, GameMaker, Ren’Py ou RPG Maker) pour être proposés dans **À confirmer**. Les raccourcis Linux doivent porter la catégorie `Game`. Les anciens résultats incertains restent dans cette section. Le bouton **Ce n’est pas un jeu · Ignorer** masque durablement un candidat ; les options permettent de réafficher les candidats ignorés.

Pour éviter les images d’installation système, les fichiers ISO doivent se trouver dans un dossier de jeux/ROMs ou une console reconnue. Ils restent à confirmer avant de rejoindre la bibliothèque principale. Les ROMs apparaissent immédiatement dans leur vue console avec leur état à confirmer. Les archives ZIP/7z et les fichiers BIN génériques ne sont pas importés automatiquement. Les fichiers `.md` de Mega Drive demandent une signature SEGA afin de ne pas confondre les documents Markdown avec des jeux.

Les noms et manifestes des lanceurs varient selon leur version. Les jeux autonomes sans signature reconnue peuvent être ajoutés avec le bouton **＋ Jeu**. RustyShelf ne télécharge pas de jaquettes et ne contacte aucun service distant.

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

Pour créer l’archive Linux : `bash build-linux.sh`. Extraire `dist/RustyShelf-linux-x86_64.tar.gz`, puis lancer `./rustyshelf`. Le fichier `rustyshelf.desktop` peut être copié vers `~/.local/share/applications/` après avoir installé l’exécutable dans un dossier présent dans le `PATH`.

Les bibliothèques graphiques fournies par la distribution restent nécessaires. Le pipeline CI vérifie formatage, tests et Clippy, puis produit des archives Windows et Linux à chaque changement poussé ou proposition de fusion. Elles sont téléchargeables comme artefacts de l’exécution CI. Pour valider CachyOS graphiquement, compiler et ouvrir l’application sur une machine CachyOS en session Wayland ou X11.

## Données

La bibliothèque SQLite est enregistrée dans le dossier de données local de l’utilisateur sous `RustyShelf/`. Si une bibliothèque GameShelf existe déjà, RustyShelf la réutilise à son emplacement initial avec ses favoris et paramètres. Les chemins d’installation et de jaquette sont stockés localement. Aucun scan ne monte de disque ou ne modifie les fichiers de jeu.

Pour restaurer une sauvegarde, fermer RustyShelf et conserver une copie du fichier actif, puis remplacer sa base SQLite par la sauvegarde. Au prochain démarrage, les chemins doivent toujours correspondre aux jeux et émulateurs installés. Le JSON est un export de consultation, sans réimport automatique.

## Vérifications

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Voir [TASKS.md](TASKS.md) pour le découpage en tâches de Luna.
