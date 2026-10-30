# CivBuilder — Plaques tectoniques et relief (version 0.3.0, en cours)

Implémenté dans `src/plates.rs` (logique pure : plaques et relief), `src/map.rs` (`WorldMap::plates`, `plate`, `plate_velocity`, `plate_boundary` ; relief tectonique dans `generate_in`), `src/erosion.rs`, `src/hydro.rs` (bord = déversoir), `src/render.rs` / `src/view3d.rs` (carte « Plaques » 2D/3D), `src/ui.rs` (légende, infobulle), `src/tooltip.rs`, `src/options.rs`, `src/wiki.rs`.

## Décisions de Benjamin (24-25/10/2026)
- Plutôt que des chaînes au hasard (ancienne « étape C »), des **plaques tectoniques**, version 2 : les plaques décident aussi des continents.
- Ordre : 1-2. planète sans bord (fait) ; 3. plaques et carte « Plaques » (fait) ; 3 bis. plaques calculées sur la sphère (fait, puis abandonné le 25/10 au soir) ; **4. continents et chaînes issus des plaques (fait)** ; 5. érosion et cols (fait).
- **25/10 au soir : retour au calcul sur la carte (cylindre)**. Constat de Benjamin : la sphère projetée crée une déformation massive aux pôles (une plaque ou un continent polaire s'étire sur toute la largeur de la carte, la « calotte en bande »). Raisonnement retenu : le monde du jeu est la carte elle-même (une unité met autant de tours à traverser la carte près du pôle qu'à l'équateur, une ville occupe une case partout) ; la sphère ne sert qu'à la latitude, c'est-à-dire au climat. Comme les Civilization.
- **25/10, idée de Benjamin : les plaques débordent de la carte** (prises en compte seulement pour la géologie) : des plaques non affichées au-dessus et en dessous. Retenu : marge simple de 20 lignes de chaque côté (pas de « repli au pôle » vers la longitude opposée, jugé déroutant à l'écran), part continentale visée sur la partie visible, bord de la carte = déversoir pour l'eau, plus de mer polaire forcée. « La banquise, c'est juste de l'eau gelée » : elle ne règle pas la géologie des bords.
- **Banquise** au nord, **jouable**, d'après la température de la mer : pour une future mise à jour.
- Climat : hybride (latitude juste pour soleil, températures, vents ; transport à plat, 40 km entre voisines partout) — cohérent avec les plaques.

## Génération (`plates::generate(width, height, wrap, seed, cell_km)`)
- **Marges cachées** : `generate_full` travaille sur `height + 2 × MARGIN_ROWS` lignes (`MARGIN_ROWS` = 20, pair pour garder le décalage des lignes) ; `crop` garde la partie visible (plaques tout entières cachées retirées de la liste et renumérotées ; germe d'une plaque coupée = sa case visible la plus proche de son centre ; frontières entre deux cases visibles ; `nearest_edge` = `u32::MAX` si la frontière la plus proche est cachée ; points chauds visibles) et range la version complète dans `Plates::full`, dont se sert `elevation` (calculée en entier puis rognée).
- **Nombre de plaques** : 10 à 14 × hauteur totale ÷ hauteur visible (≈ 14 à 19), pour en garder autant sur la partie visible.
- **Position** de chaque case : `map_point` (km ; x vers l'est, y vers le nord) ; deux voisines sont toujours à `cell_km` (40 km). `offset(a, b, world_km)` : décalage par le plus court côté du raccord ; `cylinder(p, world_km)` : point du cylindre où l'on lit les bruits.
- **Surfaces** : toutes égales (`cell_area` = 1).
- **Germes** : tirés uniformément, espacés (√(surface ÷ nombre) × 0,9, relâché au besoin).
- **Croissance** : Dijkstra multi-sources ; coût d'un pas = 1 case × (1 + 2,5 × bruit) ÷ vitesse de croissance (0,6 à 1,5).
- **Frontières sinueuses** : plaque du point décalé par deux bruits lents (≈ 9 cases) ; morceaux détachés rattachés ; plaques avalées retirées.
- **Nature** : continentales jusqu'à ≈ 40 % des cases **visibles** (sans dépasser 52 %) ; une plaque tout entière cachée est continentale avec une chance de 40 %.
- **Mouvement** : dérive (`drift`, 3 à 9 cm/an océanique, 1 à 4,5 continentale) + légère rotation sur elle-même (`spin`, au plus 35 % de sa vitesse par 1 000 km, autour de `center`) ; `Plate::velocity_at`.
- **Frontières** : direction lissée sur 3,5 cases ; rapprochement = −(v_voisine − v_case) · n. ≥ 1,2 cm/an : collision / subduction / arc d'îles ; écartement ≥ 1,2 : rift / dorsale ; sinon faille transformante.
- **Distance aux frontières** (`boundary_km`), **frontière la plus proche** (`nearest_edge`), **points chauds** (3 à 6, ≥ 200 km des frontières, espacés de 400 km).

## Relief issu des plaques — `Plates::elevation(seed, cell_km)`
- Calculé sur la carte et ses marges : frontières, intérieur bombé des continents, chapelets des points chauds continuent au-delà du bord ; un continent coupé par le bord paraît continuer hors champ.
- Bruits lus sur le cylindre (km ÷ longueur d'onde) ; socle, frontières, adoucissement (4 passes), points chauds : formules inchangées.
- **Bord = déversoir** : `erosion::erode(…, outlet, …)` et l'hydrologie (`hydro.rs`, planète seulement) prennent les cases de terre des lignes du bord pour des exutoires : l'eau qui atteint le bord s'en va (sinon, graine 2 en version cylindre sans marge : « Lac Gelé » de 647 cases sur toute la largeur, à 1 230 m). La mer polaire forcée (abaissement des 4 lignes du bord) est retirée.
- **Bilan** (graines 777 / 2 / 5), sphère → cylindre → cylindre avec marges : terres 44/44/49 → 44/50/49 → 41/42/41 % des cases ; montagnes 434/412/439 → 366/492/484 → 343/358/384 ; pics 29/26/34 → 31/35/29 → 35/31/27 ; lacs 274/147/127 → 70/145/120 → 87/197/17 (au-delà de 55° : 98/11/6 → 24/145/23 → 43/8/0).

## Aires de langue
- `toponymy::languages` reçoit les points du cylindre (rayon 1) ; k-moyennes avec la moyenne des points.

## Carte « Plaques »
- Couleur par plaque (chaude : continentale, froide : océanique), mer assombrie ; frontières en traits épais ; flèches de vitesse ; points chauds. Légende, infobulle. Les plaques et frontières coupées par le bord sont normales (elles continuent dans la marge).

## Tests
- `plaques_couvrent_la_planete` (6 à 20 plaques visibles ; connexité vérifiée sur la version complète avec marges ; part continentale visible), `les_plaques_passent_le_raccord`, `mouvement_sur_la_carte`, `relief_issu_des_plaques`, `nature_des_frontieres`, `deterministe`. `map::tectonique::bilan_tectonique` (ignoré). Érosion : `erode` prend un tableau d'exutoires (vide dans les tests).

## Limites et suite
- Les terres polaires ont une taille « normale » sur la carte : voulu ; à réduire si on en trouve trop.
- Banquise plus tard (eau gelée, jouable).
