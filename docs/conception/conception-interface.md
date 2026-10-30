# Conception — Accès aux lieux (recherche, légendes et étiquettes cliquables)

## Objectif
Les étiquettes et légendes deviennent des portes d'accès aux zones de la carte, et un index de recherche permet de retrouver tout lieu connu.

## Lieux (`MapTarget`, module `search.rs`)
- Ville (entité), cours d'eau, lac / mer intérieure, bassin versant, plaque tectonique.
- Cases d'un lieu = sa partie **explorée** (`ShownFog`, donc tout en mode debug) ; une ville = son territoire.

## Interactions
- **Survol** d'un `TargetButton` → lieu mis en avant :
  - zone (bassin, plaque, territoire de ville) : pourtour doré qui pulse (gizmos 2D épais / 3D posés sur le relief), arêtes en retrait vers l'intérieur, mis en cache tant que le lieu et le brouillard ne changent pas ;
  - cours d'eau / lac (et fleuve ou lac qui nomme un bassin) : surbrillance existante du réseau via `PointedFeature`, prioritaire sur le survol de la carte et sur l'option de surbrillance.
- **Clic** → `FocusTarget` : vol de caméra (lissage exponentiel ~7/s, zoom logarithmique) qui cadre le lieu (≈ 60 % de la vue en 2D, échelle 0,6 à 8 ; distance 250 à 2 800 en 3D), centre pris sur la copie la plus proche (planète). Interrompu par toute action caméra de l'utilisateur. Le lieu reste en avant 3 s.
- Points d'accès : lignes de la légende des bassins (survol + clic), noms des cours d'eau et lacs sur la carte (clic ; leur survol garde l'option de surbrillance, comptée aussi en mode « survol de la carte »), bannières des villes (survol = territoire, clic = caméra + sélection), résultats de recherche.

## Recherche (Ctrl+F ou bouton « Rechercher »)
Inspirée des bonnes pratiques d'autocomplétion (Baymard, Smart Interface Design Patterns) et de la recherche de Factorio :
- panneau en haut au centre ; champ avec loupe, texte d'aide quand il est vide, bouton × pour effacer ;
- **puces de filtre** (Tout, Villes, Cours d'eau, Lacs et mers, Bassins versants, Plaques) avec le **nombre de résultats** de chaque catégorie pour la requête en cours (catégorie vide grisée) ;
- au plus 8 résultats, mis à jour à la frappe ; pastille de couleur de catégorie, nom avec le **passage trouvé en gras** doré, détail à droite (type, destination et débit d'un cours d'eau, étendue…) ; « … et N autres » invite à préciser ;
- correspondance sans accents ni majuscules (`fold` un caractère pour un caractère, tirets et apostrophes = espaces) ; classement : début du nom < début d'un mot < ailleurs / mots dans le désordre ; puis ordre des catégories, puis importance (population, débit, étendue) ;
- requête vide : suggestions « Lieux connus » ; aucun résultat : message + conseil ;
- clavier : ↑ ↓ (la ligne choisie est prévisualisée sur la carte), Entrée (y aller), Tab / Maj+Tab (filtre), Échap (fermer) ; survol souris = ligne choisie ; clic hors interface ferme.
- Seuls les lieux connus sont indexés (index refait à l'ouverture, à l'exploration, au mode debug, aux villes).

## Saisie de texte
- Widget Bevy 0.19 `EditableText` (curseur, sélection, Ctrl+A/C/X/V), feature Bevy `system_clipboard` pour le presse-papiers du système.
- Pendant qu'un champ a le focus (`InputFocus`), `ButtonInput<KeyCode>` est vidé en PreUpdate (sauf Échap) : les raccourcis du jeu ne se déclenchent pas. Les touches propres aux champs se lisent avec `ButtonInput<Key>`.
- Graine du monde (menu Échap) : champ chiffres seulement (filtre), 10 caractères max, graine actuelle sélectionnée à l'ouverture ; boutons Copier / Coller ; Entrée regénère.

## Pistes
- Lieux supplémentaires : mers et océans nommés, chaînes de montagnes, cols, villages et camps découverts.
- Historique des recherches récentes, épingles (comme Factorio).
- Légende des plaques interactive comme celle des bassins.
