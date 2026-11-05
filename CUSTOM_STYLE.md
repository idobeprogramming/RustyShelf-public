# Personnaliser RustyShelf

Dans **Options → Style CSS personnalisé**, enregistrer le modèle ou importer un fichier `.css`, puis activer le style. Les options mémorisent le chemin. Après avoir modifié le fichier dans un éditeur, cliquer sur **Recharger**. Désactiver le style rétablit le thème choisi.

RustyShelf utilise une interface native egui. Le format accepte un bloc de variables CSS, et non une feuille CSS de navigateur complète. Les sélecteurs, polices, images, animations, imports et liens distants ne sont pas pris en charge. Aucune ressource réseau n’est chargée.

```css
:root {
  --background: #15191f;
  --surface: #202630;
  --card: #252e39;
  --accent: #e5a270;
  --text: #f5f0e9;
  --muted: #aeb8c5;
  --radius: 12px;
  --gap: 10px;
}
```

| Variable | Effet |
| --- | --- |
| `--background` | Fond principal et champs de saisie |
| `--surface` | Fenêtres, fiche de jeu et contrôles |
| `--card` | Cartes de la bibliothèque |
| `--accent` | Sélection, survol, emblème et bordures |
| `--text` | Texte principal |
| `--muted` | Libellés secondaires |
| `--radius` | Arrondis, de 0 à 24 px |
| `--gap` | Espacement, de 4 à 24 px |

Les couleurs acceptent `#RGB`, `#RRGGBB` et `#RRGGBBAA`. Les commentaires `/* … */` sont permis. Les variables omises conservent les valeurs du thème. Une variable inconnue ou une valeur invalide affiche une erreur et conserve le dernier style chargé pendant la session. Au redémarrage, un fichier illisible ou invalide laisse le thème intégré actif. La taille maximale du fichier est de 64 Ko.
