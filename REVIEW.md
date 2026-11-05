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

23 tests Rust passent sous Windows, ainsi que le formatage et Clippy. Les tests utilisent des données temporaires et des commandes fictives ; aucun jeu réel n’est lancé.

La détection autonome reste une heuristique : les moteurs inconnus et jeux sans fichiers reconnaissables nécessitent l’ajout manuel. Une application utilisant un moteur de jeu peut encore être proposée à confirmation. Les formats de configuration des lanceurs peuvent évoluer.

La compilation optimisée et le paquet Windows sont vérifiés localement. Cette revue n’a pas exécuté de scan complet des disques réels, de validation graphique, ni de compilation Linux. Le pipeline Linux est prévu pour Ubuntu ; la validation sur CachyOS Wayland et X11 reste à faire.
