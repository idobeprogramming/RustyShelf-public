# Revue du 5 novembre 2026

## Problèmes corrigés

- Presque tous les exécutables étaient considérés comme des candidats : la détection demande maintenant des fichiers de moteur de jeu. Les résultats incertains restent dans « À confirmer », avec un choix d’ignorance persistant et réversible.
- Les images ISO système pouvaient être proposées : la détection ISO exige désormais un dossier de jeux ou d’émulation.
- La découverte des lanceurs supprimait les installations supplémentaires d’un même titre : elles sont conservées pour la fusion en base.
- La recherche récursive Heroic pouvait n’importer qu’un jeu ou mélanger des champs de plusieurs jeux : chaque enregistrement est traité séparément, y compris sans exécutable si son lien Heroic suffit.
- Un profil d’exécutable inconnu pouvait remplacer le lien Steam et le titre reconnu : les métadonnées reconnues sont conservées.
- Deux émulateurs de même nom entraient en conflit : leur identité dépend maintenant du chemin. Les associations existantes sont conservées lors des migrations de jeux.
- Les raccourcis Linux partageaient le dossier d’installation des raccourcis et pouvaient fusionner des jeux différents : chacun conserve son chemin propre. La comparaison des chemins respecte la casse sous Linux.
- Les exclusions et volumes choisis sont appliqués aux installations découvertes par les lanceurs. Un scan interrompu ou un dossier illisible ne marque plus ses jeux comme supprimés ; les installations manuelles encore présentes restent disponibles.
- La grille imposait trois colonnes et chargeait les jaquettes hors écran : colonnes adaptatives, chargement des jaquettes visibles et cache de textures limité à 128 entrées.
- Les erreurs de lancement étaient silencieuses : elles sont affichées. « Ouvrir le dossier » d’une image disque ouvre maintenant son dossier.
- Le script Windows pouvait emballer un ancien exécutable après un échec de compilation : il s’arrête désormais sur cet échec.

## Vérification et limites

60 tests Rust passent sous Windows, ainsi que le formatage et Clippy. Les nouveaux tests de régression utilisent des données temporaires et des commandes fictives ; aucun jeu réel n’est lancé.

La détection autonome reste une heuristique : les moteurs inconnus et jeux sans fichiers reconnaissables nécessitent l’ajout manuel. Une application utilisant un moteur de jeu peut encore être proposée à confirmation. Les formats de configuration des lanceurs peuvent évoluer.

La compilation optimisée et le paquet Windows sont vérifiés localement. Cette revue n’a pas exécuté de scan complet des disques réels, de validation graphique, ni de compilation Linux. Le pipeline Linux est prévu pour Ubuntu ; la validation sur CachyOS Wayland et X11 reste à faire.

## Revue fonctionnelle RustyShelf

| Fonction | État et amélioration |
| --- | --- |
| Identité et données existantes | RustyShelf réutilise la base précédente ; les anciens choix de thème restent lisibles. |
| Présentation | Galerie à largeur adaptée, en-tête, compteurs, fiche latérale défilante, survol et bordure de sélection. Seules les lignes visibles sont dessinées. |
| Apparence | Six thèmes aux noms originaux et variables CSS locales pour couleurs, arrondis et espacement. Validation, modèle, activation et rechargement inclus. |
| ROMs | Section dynamique et sous-vues console, correction manuelle persistante, ajout manuel et scan. Les documents Markdown et modèles 3DS ordinaires sont écartés des ROMs ambiguës. |
| Jeux homonymes | Les versions sur consoles différentes gardent des identités distinctes après réouverture. |
| Recherche et filtres | Recherche titre/catégorie/console, type PC/ROM, source, disponibilité, favoris et trois tris dans toutes les vues. |
| Organisation | Favoris et catégories personnalisées disponibles dans la navigation. |
| Émulation | Association uniquement par catégorie, configurée dans la vue de cette catégorie. Aucun héritage global ou par jeu. |
| Volumes amovibles | Actualisation des disques depuis les options, sans redémarrage. |
| Sauvegarde | Copie cohérente SQLite avec réglages ; export JSON du catalogue. Aucun fichier de jeu n’est copié. |

Les tests couvrent aussi la construction de l’interface egui en 780 et 1280 pixels, les fichiers CSS invalides ou trop volumineux, les filtres cumulés, le scan de ROMs Unicode, la séparation des consoles et les sauvegardes. Ce test de construction d’interface ne remplace pas une vérification visuelle sur le système cible.

Les archives ROM ZIP/7z, le choix automatique d’un cœur RetroArch, le téléchargement de jaquettes et le CSS de navigateur complet ne sont pas implémentés. Un émulateur nécessitant des arguments supplémentaires doit être fourni par un script de lancement déjà configuré. Les fichiers CSS, jaquettes et jeux restent des ressources locales référencées par leur chemin.

## Correction du classement des images et des diagnostics

Les dossiers ne sont plus utilisés pour deviner la console d’une image disque. Les ISO, CHD et autres conteneurs sont regroupés dans « Images à classer ». Les anciens groupes contenant plusieurs images sont séparés pour permettre un classement individuel, avec conservation des favoris et des choix manuels. Les anciens libellés automatiques ne déterminent plus l’affichage ni la recherche. Les nouveaux identifiants d’image dépendent du fichier ; un rescan réutilise les identifiants existants pour préserver les corrections.

Les accès refusés et chemins devenus indisponibles sont signalés comme non analysés, séparément des erreurs de stockage ou de lecture. Le dernier rapport est enregistré en base et exportable. Les listes de détails sont bornées, et les compteurs restent exacts. Un test d’interface contrôle également l’absence de messages de collision d’identifiants egui dans les vues testées.

L’application était fermée lors de l’inspection des fenêtres : les messages exacts de la session signalée n’ont pas été observés. Les corrections ci-dessus ont été validées sur des cas reproductibles et des données temporaires ; un message persistant pourra être analysé grâce au rapport exporté.

## Revue de fiabilité complémentaire

Corrections vérifiées le 5 novembre 2026 :

- **Vignettes personnalisées** : un résultat de découverte ne remplace plus une vignette déjà enregistrée, même lorsque son disque est débranché. Le remplacement explicite reste possible.
- **Échecs d’écriture** : les favoris, confirmations, catégories, vignettes, exclusions et associations de consoles/émulateurs ne sont plus modifiés silencieusement si leur enregistrement échoue. Langue et thème reviennent au choix précédent en cas d’échec. Un rechargement impossible conserve la dernière bibliothèque affichée.
- **Scan interrompu** : un canal de scan fermé sans événement de fin ne laisse plus l’interface en analyse permanente. L’annulation recharge les résultats enregistrés et leurs corrections sans marquer les jeux non parcourus comme absents. Fermer l’application transmet aussi une demande d’arrêt.
- **Sources multiples** : les dossiers déjà reconnus servent uniquement à éviter le scan générique des exécutables. Ils ne masquent plus les résultats d’un autre lanceur ; seules les exclusions utilisateur et les volumes sélectionnés filtrent ces résultats.
- **Disques Linux** : `/run/media` reste accessible au scanner, tandis que les autres sous-dossiers d’exécution de `/run` restent exclus. La règle est testée sous Windows avec des chemins Linux ; la validation sur CachyOS reste nécessaire.
- **Raccourcis Linux** : seules les propriétés de `[Desktop Entry]` déterminent le titre, la catégorie et la commande. Une section d’action secondaire ne peut plus fournir une commande manquante.

Les tests simulent notamment un refus d’écriture SQLite, un catalogue devenu illisible après une écriture, une interruption brutale du scan et deux lanceurs partageant le même dossier. Le formatage, les 44 tests et Clippy passent. Le disque de développement X: produit encore un avertissement de cache de compilation sans liens physiques ; Rust utilise la copie de fichiers à la place.

Limites encore présentes dans les adaptateurs : la lecture Lutris couvre un sous-ensemble de YAML et certains formats de lanceurs ne sont pas reconnus. Les lectures système en cours ne peuvent pas être interrompues immédiatement. Cette revue ne constitue pas une validation graphique Windows/CachyOS ni un essai sur tous les disques réels.


## Scanner : corrections et revue finale

- Les extensions ambiguës `.gen`, `.md`, `.cso`, `.chd` et `.pbp` demandent une signature de contenu ; MDF demande un fichier MDS associé dans une collection de ROMs. Les formats SMD/CUE/CDI/NRG et ISO demandent un contexte ROMs/console. Les tests couvrent des shaders, bases de données et fichiers générés sans supprimer de fichiers réels.
- Les anciens candidats non confirmés apparaissent exclusivement dans « À confirmer ». L’action Ignorer fonctionne aussi pour les jeux confirmés et reste réversible. Les favoris et sous-catégories n’exposent plus les candidats par accident.
- Steam et GOG sont reconnus pendant le parcours général, y compris dans des dossiers personnalisés. Les erreurs de manifeste sont remontées, et les chemins relatifs sortant de la bibliothèque sont rejetés.
- GOG lit le titre et la tâche principale du manifeste, conserve les arguments entre guillemets et le répertoire de travail. Il ne choisit plus un exécutable auxiliaire. Sur Linux, un exécutable Windows seul ne crée pas de profil direct : les profils Heroic/Lutris configurés restent nécessaires.
- La découverte des lanceurs vérifie l’annulation entre les fichiers et les phases, avec progression. Les lectures de manifestes sont bornées à 4 Mo. Les racines imbriquées ne sont parcourues qu’une fois.
- La reconnaissance d’un titre officiel réutilise une installation déjà enregistrée et conserve favoris/catégories. Sous Windows, une différence de casse ou de séparateur ne crée plus une seconde installation. Sous Linux, la comparaison conserve la distinction de casse.
- Les configurations Lutris incomplètes ne deviennent plus des jeux partageant tous le dossier de configuration.
- Nouveau scan limité à un dossier, testé de bout en bout avec le canal de progression et une base temporaire. Il conserve les jeux situés hors de son périmètre.
- Options : croix testée par clic simulé, réouverture vérifiée, largeur minimale adaptée à la fenêtre et lignes de disques sur toute la largeur.

Validation : 60 tests passent, formatage et Clippy passent. Aucun jeu réel n’a été lancé et aucune nouvelle dépendance n’a été ajoutée. Les tests couvrent les profils GOG, les manifestes Steam compacts/invalides, les lectures trop volumineuses, l’annulation, les faux positifs et la conservation des corrections. La signature d’un conteneur n’est pas une validation intégrale de son contenu ; les résultats incertains nécessitent toujours une confirmation. Les catalogues réels complets et CachyOS restent à vérifier sur les machines cibles.

Références de format utilisées pendant le développement (aucun accès réseau dans l’application) : [tâches GOG documentées par Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To%3A-DOSBOX-and-SCUMMVM), [signatures de conteneurs utilisées par PPSSPP](https://github.com/hrydgard/ppsspp/blob/master/Core/FileSystems/BlockDevices.cpp).

Les raccourcis Linux utilisent aussi la lecture bornée : un fichier illisible ou trop volumineux ne peut plus être traité comme un exécutable ordinaire. Un test couvre ce cas même en présence de fichiers de moteur de jeu.
