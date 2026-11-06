# RustyShelf

RustyShelf est une bibliothèque de jeux de bureau écrite en Rust. Elle rassemble les jeux trouvés sur les volumes montés, présente leurs jaquettes enregistrées localement et permet de les lancer. La recherche et l’interface fonctionnent hors ligne.

## Utilisation

- **Scanner les disques** : choisir les volumes et les dossiers à exclure dans les paramètres. Le scan affiche sa progression, peut être annulé et conserve les jeux d’un disque débranché. Les exécutables détectés sont affichés dans « À confirmer » lorsqu’aucun lanceur ne permet de reconnaître leur titre.
- **Sources reconnues** : manifestes Steam, Epic Games, Heroic, profils Lutris et manifestes GOG `goggame-*.info`. Le parcours des disques reconnaît aussi Steam et GOG dans les dossiers personnalisés. GOG utilise la tâche principale de son manifeste, jamais le premier exécutable trouvé.
- **Scanner un dossier** : dans les options, choisir **Scanner un dossier…**. Seul son contenu est parcouru (y compris les manifestes Steam/GOG présents), sans recherche dans les catalogues des autres lanceurs. Les jeux hors de ce dossier restent inchangés.
- **Traiter les candidats par lot** : dans **À confirmer**, cocher les cartes ou utiliser **Sélectionner les résultats**, puis **Confirmer** ou **Ignorer**. Seuls les éléments sélectionnés et encore non confirmés sont traités ; changer de filtre retire de la sélection les résultats masqués. Les fichiers des jeux ne sont jamais supprimés. Les éléments ignorés peuvent être restaurés depuis les options.
- **Organiser** : rechercher, filtrer par source, modifier une catégorie, marquer un favori, ajouter un jeu et choisir une image locale.
- **Lancer** : utiliser le lien Steam, Heroic ou Lutris disponible, ou lancer directement le chemin enregistré. Sous Linux, Proton/Wine doit déjà être configuré dans le lanceur.
- **Images disque** : les fichiers ISO, CHD, CSO, WBFS, GCZ, CUE, PBP, CDI, NRG et MDF sont proposés dans **À confirmer**, puis dans **Images à classer** après confirmation. Choisir la catégorie de chaque image depuis sa fiche, puis ouvrir cette catégorie pour lui associer un émulateur. RustyShelf transmet le chemin de l’image à cet émulateur, qui doit être installé et compatible avec le format.
- **Personnaliser** : options de langue français/anglais et thèmes Nocturne, Aurore, Rétro, Horizon, Brise et Prisme. Le thème Prisme dessine des panneaux translucides dans l’application.
- **Style CSS** : importer un fichier local dans les options, activer/désactiver les variables puis recharger après modification. Un modèle est fourni dans le paquet et peut être enregistré depuis l’application. Voir [CUSTOM_STYLE.md](CUSTOM_STYLE.md) pour la syntaxe prise en charge.
- **ROMs et consoles** : la section ROMs se construit à partir de la bibliothèque. Les extensions de cartouches comme NES, SFC, GBA ou N64 permettent un classement direct. Les images disque restent dans **Images à classer** jusqu’au choix manuel de leur catégorie. Le nom du dossier et l’ancienne catégorie automatique ne déterminent plus leur console. Les choix manuels sont conservés après les scans.
- **Filtres communs** : recherche par titre/catégorie/console/source, type PC ou ROM, source, disponibilité et favoris. Le tri propose A → Z, Z → A ou les favoris en premier. Ces réglages s’appliquent à toutes les vues, y compris les consoles et catégories personnalisées. « Effacer » remet les filtres à zéro.
- **Émulateur par catégorie** : ouvrir une sous-catégorie console dans ROMs et choisir **Émulateur de cette catégorie** en haut de la galerie. Cette association s’applique uniquement aux fichiers de cette catégorie. L’ancien défaut global et les anciennes associations par jeu ne sont plus utilisés. Ajouter un exécutable dans les options le rend disponible dans la liste, sans l’associer automatiquement. Le lancement est désactivé si la catégorie ou son émulateur installé manque.
- **Rapport du scan** : les chemins protégés ou indisponibles sont comptés séparément des erreurs de lecture et de sauvegarde. Le dernier rapport est conservé et exportable depuis les options ; son détail est limité à 50 messages de chaque type, avec les compteurs complets.
- **Sauvegarder et exporter** : les options permettent une copie SQLite de la bibliothèque complète (favoris, réglages, exclusions et choix d’émulateur) et un export JSON du catalogue. Les jaquettes et fichiers de jeu restent à leur emplacement ; ils ne sont pas copiés dans la sauvegarde.

La bibliothèque principale affiche les jeux reconnus et confirmés. Les exécutables autonomes doivent présenter des fichiers de jeu (Unity, Unreal, Godot, GameMaker, Ren’Py ou RPG Maker) pour être proposés dans **À confirmer**. Les raccourcis Linux doivent porter la catégorie `Game`. Les anciens résultats incertains restent dans cette section. Le bouton **Ce n’est pas un jeu · Ignorer** masque durablement un élément, même déjà confirmé, sans supprimer ses fichiers ; les options permettent de réafficher les candidats ignorés.

Pour éviter les images d’installation système, les fichiers ISO doivent se trouver dans un dossier ROMs, ISO, émulation ou une console reconnue. Ils restent à confirmer avant de rejoindre la bibliothèque principale. Les ROMs détectées restent uniquement dans « À confirmer » avant de rejoindre les vues ROMs et consoles. Les anciens candidats suivent aussi cette règle. Les archives ZIP/7z et les fichiers BIN génériques ne sont pas importés automatiquement. Les fichiers `.md` et `.gen` demandent une signature SEGA. Les CSO, CHD et PBP sont contrôlés par leur signature. Les MDF demandent un dossier ROMs et un fichier MDS associé. Les SMD, CUE, CDI et NRG demandent aussi un dossier ROMs/console. Ces contrôles réduisent les faux positifs sans certifier qu’un fichier est un jeu complet. L’ajout manuel reste disponible pour les formats inhabituels.

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

Heroic et Lutris sont ouverts via leurs associations de liens enregistrées dans le bureau. Une installation Flatpak ou AppImage doit avoir cette association configurée. RustyShelf ne demande plus que leur exécutable soit directement accessible dans le PATH, et n’attend pas la fermeture du lanceur pour rendre la main.

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

Les options disposent d’une croix de fermeture. Leur liste de disques occupe toute la largeur et affiche le chemin séparément des détails.

Le choix des disques est mémorisé après redémarrage, même lorsqu’un disque est débranché. Un nouveau disque doit être coché après actualisation. Les choix reposent sur le chemin de montage : un changement de lettre Windows ou de point de montage Linux demande une nouvelle sélection.

**Options → Gérer les éléments ignorés…** permet de rechercher un élément, consulter son chemin et le restaurer individuellement avec ses favoris et ses catégories. **Tout restaurer** réaffiche tous les éléments ignorés, indépendamment de la recherche. Le rapport de scan conserve aussi les disques/dossiers choisis, le nombre d’éléments parcourus et l’état terminé/interrompu.

La découverte vérifie les en-têtes NES et Nintendo 64 ; les trois ordres d’octets N64 sont acceptés quelle que soit l’extension N64 utilisée. Ces contrôles ne certifient pas l’intégrité du jeu complet. Les formats inhabituels restent ajoutables manuellement. Les fichiers déjà acceptés ne sont pas supprimés par ces nouvelles règles.

Le scan évite de parcourir deux fois des racines imbriquées. L’annulation est vérifiée entre les fichiers, y compris pendant la découverte des lanceurs ; une lecture système en cours doit toutefois se terminer. Les manifestes lus sont limités à 4 Mo et leurs erreurs sont intégrées au rapport. Les profils Lutris sans exécutable local résolu sont signalés comme non analysés.

Les dossiers de jeux nommés `Windows`, `dev`, `sys`, `proc` ou `temp` restent analysés : leur nom seul ne suffit plus à les exclure. Le vrai dossier système Windows reste protégé. Sous Linux, un scan général écarte les arbres système, mais un dossier explicitement choisi dans `/tmp` ou `/var/lib` peut être analysé. Les exclusions ajoutées dans les options restent prioritaires. Les jaquettes de la galerie conservent leurs proportions, en portrait comme en paysage.

La recherche accepte plusieurs mots dans n’importe quel ordre et ignore les accents latins courants : `etoile steam` retrouve par exemple un jeu « Étoile » de Steam. Une recherche vide évite de reconstruire les textes de recherche.

La recherche couvre aussi les chemins d’installation : `D:` retrouve les jeux référencés sur ce disque, et `archives ete` peut retrouver un dossier `Archives/Été`, même déconnecté. Lorsque tu sélectionnes une source, le filtre de disponibilité considère uniquement ses installations : une copie Epic présente ne rend pas disponible une copie Steam absente.

Les vignettes sont limitées à 32 Mo et 4096 × 4096 pixels à la lecture. Les échecs sont mis en cache pour éviter de relire constamment un fichier absent ou endommagé. Après reconnexion d’un disque ou modification d’une image, utiliser **Options → Recharger les vignettes locales**.
