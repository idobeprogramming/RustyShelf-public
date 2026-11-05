# Étapes GameShelf pour Luna

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
23. [x] Ajouter les options de langue française/anglaise et les thèmes sombre, clair, Windows 98, XP, Vista et Liquid Glass.
24. [x] Détecter les images disque et choisir un émulateur par défaut ou par jeu.
25. [x] Construire les paquets Windows/Linux et les joindre aux exécutions du pipeline CI.
26. [ ] Vérifier graphiquement sur une vraie session CachyOS Wayland et X11.

Critères : formatage, tests concernés, `cargo test`, Clippy sans avertissement. Chaque lot autonome produit un commit séparé. Le pipeline CI exécute ces contrôles et construit les deux paquets à chaque push ou pull request.
