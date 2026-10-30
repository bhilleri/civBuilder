# CivBuilder — Plaques tectoniques et relief (version 0.3.0, en cours)

Implémenté dans `src/plates.rs` (logique pure : plaques, relief, passage maritime), `src/map.rs` (`WorldMap::plates`, `plate`, `plate_velocity`, `plate_boundary` ; relief tectonique dans `generate_in`), `src/erosion.rs`, `src/hydro.rs` (bord = déversoir), `src/render.rs` / `src/view3d.rs` (carte « Plaques » 2D/3D), `src/ui.rs` (légende, infobulle), `src/tooltip.rs`, `src/options.rs`, `src/wiki.rs`.

## Décisions de Benjamin (24-30/10/2026)
- Plutôt que des chaînes au hasard (ancienne « étape C »), des **plaques tectoniques**, version 2 : les plaques décident aussi des continents.
- Ordre : 1-2. planète sans bord (fait) ; 3. plaques et carte « Plaques » (fait) ; 3 bis. plaques calculées sur la sphère (fait, puis abandonné le 25/10 au soir) ; **4. continents et chaînes issus des plaques (fait)** ; 5. érosion et cols (fait) ; 6. plaques océaniques polaires et passage maritime (fait, 30/10).
- **25/10 au soir : retour au calcul sur la carte (cylindre)**. Constat de Benjamin : la sphère projetée crée une déformation massive aux pôles (une plaque ou un continent polaire s'étire sur toute la largeur de la carte, la « calotte en bande »). Raisonnement retenu : le monde du jeu est la carte elle-même (une unité met autant de tours à traverser la carte près du pôle qu'à l'équateur, une ville occupe une case partout) ; la sphère ne sert qu'à la latitude, c'est-à-dire au climat. Comme les Civilization.
- 25/10 : essai des **marges cachées** (20 lignes de plaques invisibles au-delà de chaque pôle, part continentale visée sur la partie visible). **Abandonné le 30/10** : Benjamin n'en était pas satisfait (des terres restaient collées aux bords).
- **30/10 : plaques océaniques polaires forcées** le long du bord nord et du bord sud, le reste des plaques réparti entre les deux. But : plus de masses de terre collées aux bords, des mers polaires **navigables**, **au moins un passage maritime qui fait le tour de la carte** (raboter le relief si une chaîne d'îles le ferme), de vraies frontières avec les plaques voisines (arcs d'îles, fosses, dorsales, chaînes côtières à haute latitude). Le bord reste un déversoir pour l'eau.
- **30/10 : continents moins compacts** (constat de Benjamin : souvent une seule grande masse aux formes compactes). Trois étapes décidées : côtes découpées (6a), blocs de croûte continentale dans les plaques (6b), rifts selon la vitesse d'écartement (6c). Benjamin : la piste « écarter les plaques continentales » seule donnerait une plaque par continent (pas de chaîne de collision intérieure) ; d'où les blocs de croûte indépendants des plaques.
- **Banquise** au nord, **jouable**, d'après la température de la mer : pour une future mise à jour. « La banquise, c'est juste de l'eau gelée » : elle ne règle pas la géologie des bords.
- Climat : hybride (latitude juste pour soleil, températures, vents ; transport à plat, 40 km entre voisines partout) — cohérent avec les plaques.

## Génération (`plates::generate(width, height, wrap, seed, cell_km)`)
- **Position** de chaque case : `map_point` (km ; x vers l'est, y vers le nord) ; deux voisines sont toujours à `cell_km` (40 km). `offset(a, b, world_km)` : décalage par le plus court côté du raccord ; `cylinder(p, world_km)` : point du cylindre où l'on lit les bruits.
- **Surfaces** : toutes égales (`cell_area` = 1).
- **Plaques polaires** (planète seulement, `POLAR_PLATES` = 1 à 2 par pôle) : les premières plaques. Elles se partagent le bord en secteurs de longitude égaux (départ tiré au hasard) ; chacune prend d'office les cases à moins de 10 à 18 lignes du bord (`POLAR_ROWS`, ≈ 350 à 620 km, soit au-delà de ≈ 63° à 74° de latitude ; largeur variable le long du bord, lue sur un bruit lent au bord ; au plus un quart de la hauteur de la carte). Toujours océaniques, sans rotation (une plaque qui fait le tour de la carte aurait une vitesse qui saute à l'opposé de son centre) ; dérive tirée au hasard comme les autres (3 à 9 cm/an), d'où la nature de leurs frontières.
- **Germes des autres plaques** (10 à 14) : tirés uniformément hors des bandes polaires, espacés (√(surface libre ÷ nombre) × 0,9, relâché au besoin).
- **Croissance** : Dijkstra multi-sources sur les cases libres ; coût d'un pas = 1 case × (1 + 2,5 × bruit) ÷ vitesse de croissance (0,6 à 1,5).
- **Frontières sinueuses** : plaque du point décalé par deux bruits lents (≈ 9 cases) — les bandes polaires ondulent aussi ; morceaux détachés rattachés ; plaques avalées retirées.
- **Croûte continentale** (`Plates::crust`, 30/10/2026, étape 6b ; avant : plaques entières continentales ou océaniques, tirées au hasard jusqu'à la part visée — les plaques continentales voisines se soudaient en une masse de 96 à 98 % des terres) :
  - **blocs** posés sans tenir compte des frontières : `CRUST_BLOCKS` = 4 à 6 grands blocs, poids tirés au carré (0,15 à 1)² puis plafonnés à `MAX_BLOCK_SHARE` = 45 % de la croûte (l'excédent va aux autres), plus `MICROCONTINENTS` = 3 à 6 microcontinents de 25 à 110 cases ; total visé `CONTINENTAL_SHARE` = 45 % des cases ;
  - **croissance** de chaque bloc (du plus grand au plus petit) : Dijkstra depuis un germe, coût d'un pas × (1 + 6 r²) (r : bruit 0 à 1, plus un surcoût de 1,5 aux hautes latitudes) × (1 + allongement × |sin| de l'angle avec l'axe du bloc) (allongement tiré de 0 à `ELONGATION` = 1,8, axe au hasard) ;
  - **espacement** : un bloc ne pousse pas à moins de `CRUST_GAP_KM` = 280 km des blocs déjà posés ; germe = la plus lointaine des blocs déjà posés parmi 80 cases tirées ; un grand bloc sur `JOINED_CHANCE` = 15 % (hors le premier) naît au contraire au plus près d'un autre et peut s'y souder (Afrique-Eurasie) ; ce qu'un bloc n'a pu prendre passe au suivant ;
  - **jamais sur les plaques polaires** ni à moins de `POLAR_MARGIN` = 2 cases ; germes à moins de ~60 % de la distance au pôle ; surcoût de croissance au-delà de `HIGH_LATITUDE` = 55 % de la distance au pôle (sinon un bloc lointain des autres longeait toute une plaque polaire) ;
  - **bords déformés** : croûte du point décalé par un bruit fin (`CRUST_WARP_CELLS` = 4 cases) ;
  - `crust_km` : distance de chaque case au bord de sa croûte.
- **Nature d'une plaque** : continentale si au moins la moitié de ses cases sont continentales (couleur de la carte « Plaques », recherche). Vitesse tirée entre des bornes mêlées selon sa part de continent (océanique 3-9 cm/an, continentale 1-4,5).
- **Mouvement** : dérive (`drift`, 3 à 9 cm/an océanique, 1 à 4,5 continentale) + légère rotation sur elle-même (`spin`, au plus 35 % de sa vitesse par 1 000 km, autour de `center`) ; `Plate::velocity_at`.
- **Frontières** : nature selon la **croûte des deux cases** de l'arête (pas la nature des plaques : une même frontière passe d'une subduction le long d'un continent à un arc d'îles en plein océan) ; plaque qui plonge en subduction : celle dont la case est océanique. Direction lissée sur 3,5 cases ; rapprochement = −(v_voisine − v_case) · n. ≥ 1,2 cm/an : collision / subduction / arc d'îles ; écartement ≥ 1,2 : rift / dorsale ; sinon faille transformante.
- **Distance aux frontières** (`boundary_km`), **frontière la plus proche** (`nearest_edge`), **points chauds** (3 à 6, ≥ 200 km des frontières, espacés de 400 km).

## Relief issu des plaques — `Plates::elevation(seed, cell_km)`
- Bruits lus sur le cylindre (km ÷ longueur d'onde) ; frontières, adoucissement (4 passes), points chauds : formules inchangées. Le socle d'une plaque océanique (−0,40 ± 0,18) reste sous la mer : seuls arcs d'îles et points chauds y font des îles.
- **Côtes découpées** (30/10/2026, tâche 6a du TODO ; constat de Benjamin : souvent une seule grande masse continentale, aux formes compactes). Avant : socle continental 0,30 + 0,30 × bruit, presque toute la plaque émergeait et la côte suivait sa frontière (formes rondes de la croissance des plaques). Maintenant :
  - socle = 0,55 × bruit des côtes (longueur d'onde 1 100 km, `COAST`) lu en un point **décalé par trois autres bruits** (1 200 km, décalage jusqu'à ~650 km, `WARP_KM` : « domain warping ») + 0,16 × bruit lent ;
  - **rivage par percentile** : le socle est abaissé pour que `EMERGED` = 78 % des cases continentales soient au-dessus de 0 (avant le relief des frontières), quelle que soit la graine (sans cela, l'émergé allait de 65 à 93 % selon la graine) ;
  - collines, vieux massifs et intérieur bombé ne s'ajoutent qu'au-dessus du rivage, progressivement sur 0,12 (sinon ils refermaient les mers peu profondes) ;
  - **plateau continental** : d'abord le long des marges passives (dorsale ou faille transformante entre plaques continentale et océanique, −0,22 × cloche(distance, 220 km)) ; depuis les blocs de croûte (6b), tout autour de la croûte continentale, compté dans le socle avant le percentile : −0,15 × cloche(`crust_km`, 200 km) (`SHELF`) ;
  - part continentale visée 38 → 45 % pour garder la part des terres.
- **Bord = déversoir** : `erosion::erode(…, outlet, …)` et l'hydrologie (`hydro.rs`, planète seulement) prennent les cases de terre des lignes du bord pour des exutoires : l'eau qui atteint le bord s'en va (sinon, graine 2 en version cylindre sans marge : « Lac Gelé » de 647 cases sur toute la largeur, à 1 230 m). Avec les plaques polaires, le bord est presque toujours en mer.

## Passage maritime autour de la planète
- Après l'érosion (`map.rs`), `plates::open_sea_passage(elevation, width)`.
- **Détection** (`has_sea_passage(sea, width)`) : parcours de chaque étendue de mer en comptant les tours franchis au raccord (−1 vers l'ouest, +1 vers l'est) ; une case retrouvée avec un autre compte ferme une boucle qui fait le tour.
- **Ouverture** s'il n'y en a pas : pour chaque pôle (zone de `PASSAGE_ROWS` = 26 lignes) et chaque ligne de départ en colonne 0, plus court chemin vers l'est sans repasser le raccord jusqu'à une case de la dernière colonne voisine du départ par le raccord ; coût d'une case : 0 en mer, 1 + 4 × altitude sur terre (moins de terres, et les plus basses). La boucle la moins chère des deux pôles est ouverte : ses terres passent à −0,03 (mer peu profonde). Un seul passage est garanti ; l'autre pôle peut rester fermé par des îles.
- **Bilan** (graines 777 / 2 / 5) : passage naturel aux deux pôles pour les trois graines (rien à raboter).

## Formes des terres (`plates::tests::formes_des_terres`, relief brut, graines 777 / 4242 / 2 / 5 / 11 / 2026)
- Indice de découpage : longueur de côte d'une masse ÷ celle d'un disque de même surface (1 pour un disque).
- Avant les côtes découpées : une seule masse de ≥ 300 cases sur les six graines, portant **96 à 98 % des terres** (Terre : Afrique-Eurasie ≈ 57 %) ; découpage de la plus grande 1,5 à 2,7.
- Après les côtes découpées (6a) : plaques continentales émergées à 72-84 % (après le relief des frontières) ; plus grande masse 89 à 97 % des terres (toujours dominante : plaques continentales voisines soudées) ; découpage 3,3 à 4,1 ; golfes, péninsules, bras de mer le long des rifts.
- Après les blocs de croûte (6b) : **3 à 5 masses de ≥ 300 cases** ; plus grande masse **37 à 49 % des terres** sur cinq graines, 71 % sur la graine 4242 (bloc soudé) ; tailles par ex. 3 777 / 1 959 / 1 830 / 441 (777), 3 039 / 2 941 / 1 848 (5) ; croûte continentale émergée à 81-86 % ; découpage 1,8 à 3,3 (le plus grand bloc a désormais une seule côte extérieure, sans les bras de mer entre plaques soudées qui gonflaient l'indice).

## Bilan (`map::tectonique::bilan_tectonique`, graines 777 / 2 / 5)
Sphère → cylindre → cylindre avec marges → **plaques polaires** :
- terres 44/44/49 → 44/50/49 → 41/42/41 → **38/37/42 %** des cases (45/44/51 % de la surface pondérée par cos(latitude) : les hautes latitudes sont en mer) ;
- montagnes 434/412/439 → 366/492/484 → 343/358/384 → **353/330/411** ; pics 29/26/34 → 31/35/29 → 35/31/27 → **40/30/32** ;
- lacs 274/147/127 → 70/145/120 → 87/197/17 → **9/1/248** (au-delà de 55° : 98/11/6 → 24/145/23 → 43/8/0 → **0/1/1**) : plus de lacs polaires ; la graine 5 a une grande cuvette intérieure ;
- terres au-delà de 70° : 140/282/198 cases (îles, bords des continents voisins, chaînes côtières) ; passages nord et sud : oui/oui pour les trois.
- **Blocs de croûte continentale** (30/10) : terres 39/38/38 % des cases (49/49/50 % de la surface) ; terres sur croûte continentale 97/97/98 % ; montagnes 295/289/367, **pics 15/11/28** (moins de collisions entre continents : moins de très hauts sommets) ; lacs 49/77/30 ; **terres au-delà de 70° : 48/124/31** ; passages nord et sud : oui/oui pour les trois. Courants (`bilan_courants`) : vitesse maximale 0,95-1,3 m/s, circumpolaire aux deux pôles, côtes ouest 45-65° +4,9 à +5,4 °C.
- **Côtes découpées** (30/10) : terres 39/36/32 % des cases (47/44/39 % de la surface) ; montagnes 333/342/290, pics 34/28/27 ; lacs 20/42/14 (la grande cuvette de la graine 5 a disparu) ; terres au-delà de 70° : 135/256/306 ; passages nord et sud : oui/oui pour les trois.

## Aires de langue
- `toponymy::languages` reçoit les points du cylindre (rayon 1) ; k-moyennes avec la moyenne des points.

## Carte « Plaques »
- Couleur par plaque (chaude : continentale, froide : océanique), mer assombrie ; frontières en traits épais ; flèches de vitesse ; points chauds. Légende, infobulle. Les plaques polaires sont des bandes le long des bords.

## Tests
- `plaques_couvrent_la_planete` (6 à 20 plaques ; chacune d'un seul tenant, raccord compris ; part de croûte continentale), `croute_continentale_en_blocs` (au moins 3 blocs de ≥ 25 cases, aucun au-delà de 35 % des cases, rien sur les plaques polaires, un continent qui chevauche une frontière, une plaque mi-continentale mi-océanique, nature des frontières selon la croûte), `plaques_oceaniques_aux_poles` (lignes du bord à ≥ 95 % océaniques, frontières de la plaque polaire, au plus 10 cases de terre sur une ligne du bord), `passage_maritime_autour_de_la_carte` (isthme raboté à son col le plus bas, mer déjà ouverte inchangée, mer qui ne fait pas le tour), `les_plaques_passent_le_raccord`, `mouvement_sur_la_carte`, `relief_issu_des_plaques`, `nature_des_frontieres`, `deterministe`. `map::tectonique::bilan_tectonique` (ignoré ; passages nord, sud, en tout ; terres au-delà de 70°). Érosion : `erode` prend un tableau d'exutoires (vide dans les tests).

## Limites et suite
- Moins de pics qu'avant les blocs (11 à 28 contre 27 à 34) : les collisions de continents sont plus rares. À revoir si Benjamin veut plus de hauts sommets.
- Une graine sur six environ garde une masse dominante (bloc soudé, `JOINED_CHANCE`).
- Suite : étape 6c (rifts en lacs, bras de mer ou jeunes océans selon la vitesse d'écartement).
- Bandes polaires larges (10 à 18 lignes, soit 20 à 30 % de la carte en mer polaire) : à réduire si Benjamin les trouve trop grandes (`POLAR_ROWS`).
- La part des terres dépend de la taille des plaques tirées (37 à 42 %).
- Le passage raboté fait une case de large ; le passage naturel peut être étroit par endroits (ondulation des frontières).
- `seas.rs` garde la règle « mer polaire » (tâche suivante du TODO : mers fermées).
- Banquise plus tard (eau gelée, jouable) ; courants marins (courant circumpolaire si le passage existe).
