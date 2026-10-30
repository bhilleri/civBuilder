# Conception — Interface : accès aux lieux, mise en scène des actions, journal

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

# Mise en scène des actions et journal hiérarchisé

## Demande de Benjamin
Les unités se téléportaient : le joueur doit voir le déplacement, voir qu'une unité en attaque une
autre, lire sur la carte les PV perdus, la perte d'une unité et la récompense d'un village tribal.
Le journal en haut à droite doit mettre davantage en évidence l'arrivée d'une information, selon son
importance, sans être envahissant.

## Recommandations retenues (recherche UX)
- Chiffres flottants **sobres** : texte clair cerné d'une ombre portée, lisible sur tout terrain ;
  taille selon la force du coup ; mise en avant réservée aux événements forts (dispersion).
- Pas de texte qui recouvre l'action : le mot « Dispersé ! » monte plus haut que les dégâts.
- Animation **courte**, et qui raccourcit vraiment le tour : longs trajets accélérés, actions hostiles
  hors de vue non montrées (un réglage de vitesse qui ne change que l'image ne sert à rien).
- Notifications non modales, hiérarchisées : couleur, durée du flash et pulsations selon la gravité ;
  une entrée douce (glissement) plutôt qu'une apparition brutale ; ce qui est grave reste marqué.

## Fonctionnement (`feedback.rs`)
- Les règles restent **instantanées** (positions, PV, dispersion) ; elles posent au passage des
  indications (`Cues`) : marche, combat, récompense. Un système les répartit dans le temps dans
  la même image et pose sur chaque unité une `Motion` (segments datés : marche, élan, secousse,
  effacement), lue par la 2D (`Transform`) et la 3D (figurine posée sur le relief).
- Ordre : actions du joueur ensemble ; puis faction hostile, une action visible après l'autre
  (+0,1 s), sans attendre la fin des effacements.
- Unité dispersée : retirée du jeu (composants `Unit`, `HexPos`, `Hostile` ôtés) mais gardée pour
  s'effacer (`Retired`), puis supprimée.
- `HeldHp` : les barres de PV gardent la valeur d'avant le coup jusqu'à l'impact.
- Chiffres flottants : nœuds d'interface placés à la projection de leur case (2D et 3D), apparition
  avec léger grossissement, montée de 34 px, effacement sur 1,7 s. Impacts et traits : gizmos.
- Chiffres : 0,15 s par case (1 s au plus par trajet), élan 0,36 s (coup aux 40 %), trait 0,28 s,
  secousse 0,3 s, effacement 0,6 s. Les PV affichés sont ceux réellement perdus (pas plus que ce
  qui restait).
- Vérification : `CIVBUILDER_CAPTURE_FEEDBACK=1` avec `CIVBUILDER_CAPTURE=<dossier>`.

## Journal
- `Importance` : routine (gris, sans flash), information (bleu, 0,9 s), succès (doré, 1,3 s),
  alerte (orange, 1,8 s, 2 pulsations, contour), critique (rouge, 2,6 s, 3 pulsations, contour,
  texte plus grand). Alertes et critiques gardent une teinte de fond ; les anciens messages
  s'estompent (sauf alertes). Six messages gardés.

## Limites et suite
- Le brouillard se lève dès la fin du tour, avant l'arrivée de l'unité.
- La caméra ne va pas chercher une attaque hostile hors de l'écran (le journal la signale).
- Pistes : réglage de vitesse des animations (et « combat rapide ») dans les options, son,
  journal qui n'affiche un message hostile qu'au moment où l'action se joue.
