# CivBuilder — Planète sans bord est-ouest (version 0.3.0, en cours)

Implémenté dans `src/hex.rs` (`Wrap`), `src/map.rs`, `src/hydro.rs`, `src/basins.rs`, les règles (`rules.rs`, `pathfinding.rs`, `explore.rs`, `hostile.rs`, `sites.rs`, `game.rs`, `input.rs`), le rendu (`render.rs`, `view3d.rs`, `relief.rs`, `highlight.rs`, `labels.rs`) et `src/seam.rs` (défilement sans fin).

## Décision de Benjamin (24/10/2026)
- Avant les plaques tectoniques (version 2 retenue : les plaques décident aussi des continents), **faire la jonction est-ouest**, pour qu'une plaque ou une chaîne de montagnes passe le raccord.
- Ordre : 1. données, génération, règles ; 2. affichage 2D/3D sans fin ; 3. plaques et carte « Plaques » ; 4. continents et chaînes ; 5. érosion et cols.
- Les pôles restent des bords.

## Modèle
- `hex::Wrap { width }` (0 : pas de tour) : `hex()` ramène une case dans les colonnes 0 à width − 1 ; `nearest(from, to)` donne la copie de `to` la plus proche de `from` (directions, tracés) ; `distance` par le plus court côté ; `neighbors`, `neighbor`, `direction`, `range` ramenés dans la carte ; `offset(h)` = décalage monde (± un tour) d'une copie ; `world_width()` = un tour (192 × 55,4 ≈ 10 642 unités).
- Décaler de `width` colonnes en coordonnées « offset odd-r » = décaler q de `width` : le raccord est une symétrie exacte de la grille, quelle que soit la largeur.
- `WorldMap::wrap` (défini à la génération d'après `Region::wraps`, la planète) ; `index()` ramène la case, donc `get`, `contains`, etc. acceptent une case au-delà du raccord. Méthodes `canonical`, `neighbors`, `distance`, `range`. `MoveCosts::wrap()` pour la recherche de chemin (A*, Dijkstra) et la carte connue.
- **Règle d'or** : les ensembles et tables (brouillard, occupation, territoire, ZdC…) ne contiennent que des cases ramenées dans la carte ; toute case calculée géométriquement (`hex + direction`) est ramenée avant d'y être cherchée.

## Génération
- Bruit (relief, humidité, rugosité) lu sur un cylindre : angle = colonne / largeur × 2π, rayon tel qu'une colonne ait la même échelle qu'à plat.
- Plus d'atténuation vers l'est et l'ouest (seulement vers les pôles) : des terres touchent le raccord. Les mondes changent donc (même graine).
- Hydrologie : grille qui boucle (`HydroInput::wraps`), coins des rivières d'arête rangés avec des cases ramenées ; le bord est-ouest n'est plus un exutoire. Bassins (`BasinInput::wrap`) : bord = pôles seulement ; centre d'un lac calculé du même côté du raccord ; réemploi des noms par distance au plus court.
- Climat : bouclait déjà.

## Règles
- Déplacements, A* (heuristique au plus court), cases atteignables, carte connue, ZdC, position d'attaque (mêlée à 1, tir à portée), vision et brouillard, exploration automatique (tableaux indexés en ramenant les cases), villages/camps (distances), ville la plus proche, IA hostile (proie, errance, approche), distance minimale entre villes.

## Affichage
- **Caméras fantômes** (`seam.rs`) : pour chaque caméra (2D et 3D), deux copies décalées d'un tour (ouest, est), sous la caméra principale (ordre plus petit). Elles ne s'allument que si la vue déborde de leur côté (étendue : demi-largeur de fenêtre × zoom en 2D ; distance × 3 + 400 en 3D). La première caméra dessinée repeint le fond, les suivantes dessinent par-dessus (`ClearColorConfig::None`).
- La caméra principale reste dans le tour : passé un bord, elle est ramenée d'un tour (même image). Les requêtes de caméra excluent les fantômes (`Without<GhostCamera>`).
- 3D : tout est rangé par case de la carte ; une voisine vue au-delà du raccord est lue dans la carte et sa géométrie (lits des cours d'eau) décalée d'un tour (`Channel::shifted`). `Relief::ground` ramène le point dans la carte. `BumpMap` (struct) ramène les cases. Bruit des crêtes lu sur un cylindre (`ridge_noise(p, wrap)`).
- **Copies du bord** (`SeamCopy`) : sol et eau des colonnes 0 et 191 recopiés de l'autre côté (même maillage, matériau suivi). Sans elles, vue fantôme et vue principale se touchaient pile sur le raccord et le lissage MSAA y laissait un liseré sombre en pointillés.
- Tracés (trajets, cours d'eau surlignés) : chaque point pris du côté le plus proche du précédent. Frontières, zone accessible, bords d'exploration : voisines ramenées. Étiquettes : copie la plus proche de la caméra. Survol 2D/3D : case ramenée.
- Vérifié en captures (graine 777) : 2D et 3D de part et d'autre du raccord, vue large et rapprochée, sans couture visible.

## Tests ajoutés
- `hex::tour_est_ouest`, `map::planete_sans_bord_est_ouest` (voisines, chemin de 3 cases par le raccord, rivière d'arête vue des deux côtés), `map::planete_generee_sans_couture` (écart d'altitude au raccord comparable à celui entre colonnes, terres au raccord), `hydro::l_eau_passe_le_raccord_est_ouest`, `sites::ville_la_plus_proche` (par l'autre côté).

## Limites
- 3D : la caméra principale dessine par-dessus le fantôme sans test de profondeur commun : un relief très haut juste au raccord pourrait masquer à tort un peu du fantôme (non observé).
- Dézoom 2D maximal (×8) : la vue peut dépasser un tour ; deux fantômes suffisent (moins de deux tours visibles).
