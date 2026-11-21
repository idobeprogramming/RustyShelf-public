# Étapes RustyShelf pour Luna

Une tâche à la fois : comprendre, modifier, vérifier les critères, puis créer un commit dédié.

1. [x] Initialiser Cargo et Git.
2. [x] Écrire les consignes et découper les tâches.
3. [x] Fenêtre sombre et navigation de la bibliothèque.
4. [x] Modèles jeu, installation, catégorie et lancement.
5. [x] Base SQLite locale et tables versionnées par création.
6. [x] Détection des volumes et exclusions configurables.
7. [x] Scan récursif annulable avec progression et erreurs.
8. [x] Bibliothèques et manifestes Steam.
9. [x] Manifestes Epic Windows.
10. [x] Métadonnées locales Heroic.
11. [x] Configurations Lutris.
12. [x] Dossiers de jeux GOG.
13. [x] Candidats indépendants et ajout manuel.
14. [x] Fusion persistante sans perte de favoris et catégories personnalisées.
15. [x] État de scan raccordé à l'interface.
16. [x] Recherche, tri alphabétique, catégories et favoris.
17. [x] Jaquettes locales et remplacement manuel.
18. [x] Lancement direct et liens Steam sous Windows/Linux.
19. [x] Délégation aux lanceurs Linux Heroic et Lutris.
20. [x] Options, exclusions et états de disponibilité.
21. [x] Scripts de paquet Windows/Linux, fichier `.desktop`, guide CachyOS et CI multiplateforme.
22. [x] Limiter l'utilisation mémoire du scan, traiter les erreurs par accès refusé et conserver les candidats à confirmer.
23. [x] Ajouter les options de langue française/anglaise et les thèmes Nocturne, Aurore, Rétro, Horizon, Brise et Prisme.
24. [x] Détecter les images disque et associer un émulateur uniquement à leur catégorie.
25. [x] Construire les paquets Windows/Linux et les joindre aux exécutions du pipeline CI.
26. [ ] Vérifier graphiquement sur une vraie session CachyOS Wayland et X11.
27. [x] Renommer l’application RustyShelf et conserver l’accès à la bibliothèque précédente.
28. [x] Donner aux thèmes des noms originaux et prendre en charge les anciens réglages.
29. [x] Refaire la galerie, la fiche latérale et le défilement par lignes visibles.
30. [x] Importer un style CSS local limité, avec modèle, erreurs et rechargement.
31. [x] Ajouter les vues ROMs/consoles, les filtres communs et le tri.
32. [x] Corriger la console d’un jeu et choisir un émulateur par console.
33. [x] Sauvegarder la base SQLite et exporter le catalogue JSON.
34. [x] Retirer le défaut global d’émulateur et les classements arbitraires d’images par dossier.
35. [x] Séparer les anciens groupes d’images et préserver leurs corrections.
36. [x] Distinguer chemins non analysés et erreurs réelles, conserver et exporter le rapport.

37. [x] Ajouter la croix des options et élargir la liste des disques.
38. [x] Filtrer les extensions ROM ambiguës et isoler tous les candidats à confirmer.
39. [x] Détecter les manifestes Steam/GOG dans les dossiers personnalisés.
40. [x] Annuler la découverte des lanceurs, borner les lectures et rapporter les erreurs.
41. [x] Ajouter le scan ciblé d’un dossier et préserver les autres installations.
42. [x] Revoir les identités Windows et les profils Lutris incomplets.

43. [x] Réduire et mesurer le coût de chargement des grandes bibliothèques.
44. [x] Borner le décodage des vignettes et mettre les échecs en cache.
45. [x] Rechercher plusieurs mots sans accents et réduire les allocations du tri.
46. [x] Confirmer ou ignorer les candidats sélectionnés par lot, avec transaction.
47. [x] Mémoriser les disques sélectionnés, y compris pendant leur déconnexion.
48. [x] Rechercher les éléments ignorés et les restaurer individuellement.
49. [x] Conserver le périmètre, le nombre d’éléments parcourus et l’issue du scan.
50. [x] Contrôler les en-têtes NES/Nintendo 64 pendant la découverte.
51. [x] Lire toutes les bibliothèques Steam dans les listes compactes et signaler les erreurs.
52. [x] Ne plus masquer les jeux dont les dossiers portent un nom système et permettre les scans Linux ciblés.
53. [x] Vérifier l’annulation entre les entrées exclues et conserver les proportions des jaquettes.
54. [x] Garder distincts les titres qui ne diffèrent que par espaces ou ponctuation.
55. [x] Appliquer le style CSS uniquement après sauvegarde atomique et conserver le style précédent en cas d’erreur.
56. [x] Transférer les corrections de console lors des migrations d’identifiants anciens.
57. [x] Conserver les arguments Epic et écarter les installations incomplètes.
58. [x] Lire les installations Heroic imbriquées et choisir leur lanceur sans deviner depuis le nom de l’utilisateur.
59. [x] Appliquer le filtre de disponibilité à la source sélectionnée.
60. [x] Rechercher aussi par disque et dossier d’installation.
61. [x] Conserver la console des ROMs `.md` après changement de catégorie et déconnexion du disque.
62. [x] Utiliser les associations système pour Heroic/Lutris et ne pas attendre la fermeture des lanceurs.
63. [x] Renommer un jeu sans écraser son nom détecté ni perdre ce choix après un scan.
64. [x] Distribuer un exécutable Windows graphique sans console supplémentaire.
65. [x] Ajouter Ctrl+F pour rechercher et Échap pour fermer les fenêtres secondaires.
66. [x] Restaurer une fiche ignorée lors de son ajout manuel explicite, dans une transaction.
67. [x] Préserver les métadonnées et profils des lanceurs lors d’un ajout manuel ; compléter seulement les profils absents.
68. [x] Accepter l’ajout des exécutables sans extension et signaler un fichier disparu.
69. [x] Réutiliser l’écriture de tous les chemins d’installations correspondants lors d’un scan Windows.
70. [x] Préserver le thème choisi face au thème système et corriger les titres illisibles.
71. [x] Détecter les cartouches 3DS par en-tête sans imposer le nom du dossier, y compris à l’ajout manuel.
72. [x] Aligner les volumes à scanner à gauche des options.
73. [x] Refaire Prisme avec transparence native, flou Windows, reflets et contrôles arrondis ; préserver l’opacité des autres thèmes.
74. [x] Découvrir les jaquettes locales Steam et les images associées aux jeux/ROMs pendant le scan, sans remplacer les choix enregistrés.
75. [x] Rechercher les jaquettes manquantes à la demande en arrière-plan, sans scan complet et avec annulation.
76. [x] Écarter les outils auxiliaires et limiter la reconnaissance des exécutables Unreal au dossier du jeu et à ses binaires.
77. [x] Revérifier les anciens candidats pendant le scan, masquer les faux positifs identifiés sans perdre les corrections, et afficher un bilan restaurable.
78. [x] Reconnaître les composants Office, Visual Studio, SDKs, pilotes et navigateur par leur dossier d’installation, sans exclure les jeux portant un nom similaire.
79. [x] Valider le contenu SMD, RVZ, GCZ et WBFS ; partager les contrôles entre découverte et nettoyage des anciens candidats.
80. [x] Lire les manifestes locaux avant les fichiers internes des jeux et éviter les candidats parasites, sans masquer les autres manifestes ni les dossiers voisins.
81. [x] Valider la structure AppState de Steam, l’identifiant et le dossier d’installation ; préserver les titres Unicode et les chaînes échappées.
82. [x] Filtrer les dossiers d’installation connus des outils de développement, de bureautique et du système qui encombrent encore la liste des candidats.

Critères : formatage, tests concernés, `cargo test`, Clippy sans avertissement. Chaque lot autonome produit un commit séparé. Le pipeline CI exécute ces contrôles et construit les deux paquets à chaque push ou pull request.
