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

32 tests Rust passent sous Windows, ainsi que le formatage et Clippy. Les tests utilisent des données temporaires et des commandes fictives ; aucun jeu réel n’est lancé.

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
| Émulation | Priorité au choix par jeu, puis par console, puis au défaut général. |
| Volumes amovibles | Actualisation des disques depuis les options, sans redémarrage. |
| Sauvegarde | Copie cohérente SQLite avec réglages ; export JSON du catalogue. Aucun fichier de jeu n’est copié. |

Les tests couvrent aussi la construction de l’interface egui en 780 et 1280 pixels, les fichiers CSS invalides ou trop volumineux, les filtres cumulés, le scan de ROMs Unicode, la séparation des consoles et les sauvegardes. Ce test de construction d’interface ne remplace pas une vérification visuelle sur le système cible.

Les archives ROM ZIP/7z, le choix automatique d’un cœur RetroArch, le téléchargement de jaquettes et le CSS de navigateur complet ne sont pas implémentés. Un émulateur nécessitant des arguments supplémentaires doit être fourni par un script de lancement déjà configuré. Les fichiers CSS, jaquettes et jeux restent des ressources locales référencées par leur chemin.
