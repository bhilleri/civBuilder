# CivBuilder — Conception de l'altitude et du relief (version 0.3.0, en cours)

Implémenté dans `src/map.rs` (`Relief`, génération, déplacements, rendements, `surface_elevation`), `src/combat.rs` / `src/battle.rs` / `src/rules.rs` (hauteur en mètres, falaises), `src/render.rs` / `src/view3d.rs` / `src/ui.rs` (cartes Altitude et Relief, symboles, légendes, hauteur 3D), `src/relief.rs` (relief 3D modelé dans le sol), `src/wiki.rs`.

## Constat de Benjamin (24/10/2026)
- Trop de montagnes : quelques pics, d'accord, mais une vaste zone infranchissable est étrange.
- Une altitude plus fine aiderait à orienter les cours d'eau.
- Toute terre au-delà d'une certaine altitude était infranchissable, alors que ça dépend du relief.
- Puis (3D) : les montagnes s'intègrent mal, « une structure au lieu d'un vrai relief », posées sur une prairie ; un fleuve qui traverse une montagne se voit mal.

## Plan
1. **Altitude en mètres partout, relief par case, montagnes franchissables et pics, pentes en mètres** — fait.
2. Rendu 3D, choix de Benjamin « A, puis B, puis C » :
   - **A. Relief modelé dans le sol** (collines, montagnes, pics), couleurs selon pente et altitude, vallées des fleuves — fait (« Bravo, c'est très réussi »).
   - **B. Sol 3D à la hauteur réelle en mètres** (plus de tranches) — fait.
   - **C.** Chaînes de montagnes (bruit « en crêtes ») et **érosion** qui creuse les vallées (cours d'eau mieux orientés) — à faire.
3. Géologie (plaques, points chauds) — plus tard.

## Choix de Benjamin
- Le joueur voit des **mètres** ; si ce n'est pas assez clair, on passera à des classes d'altitude pour l'affichage (et alors pour le combat).
- **Montagne franchissable à 3 PM, seuls les pics infranchissables.** Revu le 30/10/2026 (voir « Franchir une montagne ») : « les montagnes étaient passables, mais au prix de beaucoup de temps ».
- 5 reliefs : plat, vallonné, accidenté, montagne, pic (la représentation pourra être revue).
- Ajout des cartes **Altitude** (m) et **Relief** (plat → pic).
- Plutôt que plus de niveaux de sol (7 auparavant : mer + 6 tranches), passer à l'altitude continue (étape B).

## Modèle
- `WorldMap::elevation` (m) : référence ; `levels` (niveaux 1 à 9) en sont **déduits** (`level_of_meters`, voir « Altitude en mètres d'abord ») et ne servent plus qu'aux bords dessinés (descentes), aux teintes de la carte 2D, à la limite des marais et aux cartes faites à la main. Les falaises se décident en mètres (`is_cliff`).
- `WorldMap::lake_level_m` : cote de la surface de chaque lac (min sur le lac de la surface hydrologique et de la plus basse rive, ≥ 0), identique pour toutes les cases d'un même lac.
- `WorldMap::surface_elevation(hex)` : 0 en mer, cote du lac pour un lac, sinon altitude du sol (≥ 0).
- `Relief` (enum, remplace l'ancien booléen « collines ») ; le terrain `Montagnes` disparaît : une montagne porte le biome de son climat (souvent toundra ou neige).
- Le climat reçoit l'altitude en mètres (`Surface::Land(m)`) au lieu de l'altitude moyenne du niveau.

## Altitude en mètres d'abord (01/11/2026, demande de Benjamin)
- Constat (tâche « monde trop sec ») : les terres étaient deux fois trop hautes (16 % sous 300 m et 53 % sous 1 000 m, Terre ≈ 33 % et 70 %) et **identiques pour toutes les graines** : la génération rangeait les terres par altitude relative, les découpait en 6 classes de parts fixes (16 / 36 / 56 / 74 / 87 %), puis donnait des mètres à chaque classe (300 / 650 / 1 075 / 1 550 / 2 100 m, 3 000 m au 96ᵉ centile, 4 800 m au sommet). Les mètres venaient des classes. Conséquence : les hauts plateaux équatoriaux (au-dessus de ~1 500 m) n'étaient pas tropicaux et portaient de la forêt tempérée au lieu de jungle.
- Décision de Benjamin : **l'altitude est en mètres, les niveaux s'en déduisent**, et non l'inverse ; **9 niveaux** au lieu de 6, plus serrés en basse altitude (bornes 100 / 250 / 500 / 750 / 1 000 / 1 300 / 1 700 / 2 300 m, `LEVEL_BOUNDS_M` ; le niveau à 1 000 m a été ajouté par Benjamin).
- **Mètres** (`TECTONIC_METERS`) : échelle fixe de l'altitude relative issue des plaques et de l'érosion (0 au rivage, ~1,5 aux plus hauts sommets) : 0,04 → 150 m, 0,14 → 350 m, 0,25 → 650 m, 0,38 → 1 200 m, 0,55 → 2 100 m, 0,95 → 3 000 m, 1,5 → 4 800 m (interpolée, prolongée au-delà). Calée sur la moyenne des centiles de l'altitude relative de 5 graines (777, 2, 4242, 5, 31 : 16 % des terres sous 0,038-0,050, la moitié sous 0,18-0,23, 87 % sous 0,44-0,62, sommets 1,46-1,62) pour une répartition terrestre en moyenne. **Chaque monde a la sienne** : graine 4242, plus plate, 35 % sous 300 m et 77 % sous 1 000 m ; graine 2, 31 % et 64 %. Les cartes régionales (sans plaques, tests) gardent un placement par rang (`REGION_METERS`, même répartition moyenne).
- **Falaise en mètres** (`is_cliff`, `CLIFF_M` = 1 000 m) : dénivelée d'au moins 1 000 m, la case basse sous 1 500 m (`MOUNTAIN_MIN_M`). Avec des niveaux serrés en bas, « 3 niveaux d'écart » aurait fait une falaise de 100 à 500 m ; et une règle de dénivelée seule aurait couvert les massifs de falaises (deux montagnes à 2 200 et 4 000 m), là où c'est le coût de la montagne qui doit freiner (les anciennes classes du haut, très larges, l'évitaient). Côte : la mer compte à 0 m (dessin 3D, `cliff_with_sea`).
- **Marais** : jusqu'au niveau 3 (sous 500 m ; auparavant niveau 2, sous ~650 m).
- Bilans (graines 777 / 2 / 4242, avant → après) : terres sous 300 m 16 → 31 / 31 / 35 % ; sous 1 000 m 53 → 68 / 64 / 77 % ; sous 2 000 m 85 → 84 / 85 / 91 %. Niveaux 1 à 9 (777) : 1 007 / 1 151 / 1 715 / 1 030 / 769 / 673 / 479 / 580 / 994 cases. Falaises : 0,3 / 0,3 / 0,8 % → 0,9 / 0,9 / 1,0 % des arêtes entre terres ; côtes à plus de 1 000 m 135 / 116 / 158 → 50 / 46 / 35. Montagnes 301 / 272 / 295 → 338 / 319 / 321, pics 15 / 11 / 18 → 17 / 13 / 22. Marais 49 / 18 / 39 → 60 / 16 / 28. Biomes : jungle 469 / 507 / 346 → 525 / 624 / 653 cases ; désert 943 / 1 364 / 1 483 → 1 076 / 1 577 / 1 746 (basses terres plus chaudes, plus d'évaporation) ; forêt tempérée 1 853 / 1 517 / 1 884 → 1 724 / 1 333 / 1 387. `bilan_montagnes` (777 / 2 / 5) : 375 / 327 / 361 montagnes, à 4 / 5 / 6 / 7 / 8 PM : 33 / 50 / 95 / 108 / 89 ; 87 / 104 / 35 / 27 / 74 ; 73 / 75 / 71 / 57 / 85 ; traversées par au moins 2 montagnes 13/41, 5/84, 7/60. `bilan_verifier` : 0 incohérence.
- Test des falaises sur les cartes régionales de 64 × 40 (bruit sans plaques, plus heurté) : jusqu'à 6 % des arêtes ; seuil du test porté de 5 à 8 % (95 % du continent de départ reste accessible).
- Suite possible : recaler l'échelle sur d'autres graines si un monde paraît trop haut ou trop plat ; plaques et érosion pourraient produire directement des mètres.

## Génération du relief
- Note d'escarpement par case de terre = 0,45 × rang de la dénivelée maximale avec les voisines + 0,3 × rang d'un bruit propre + 0,35 × altitude / 4 800 m.
- Par note décroissante : 5 % montagnes (au moins 1 500 m, sinon accidenté), 8 % accidenté, 15 % collines, le reste plat.
- Pics : montagnes d'au moins 2 400 m, sommets locaux (aucune voisine plus haute de plus de 200 m), du plus haut au plus bas, jamais côte à côte, au plus 1 % des terres. Graine 777 : 54 pics (0,6 %), 375 montagnes (4,3 %), 696 accidentés (8 %), 1 298 vallonnés (15 %) ; graine 2 : 40 pics, 294 montagnes.
- Fleuves et lacs : cases plates. Relief de base gardé entre les passes climat ↔ eau.
- Accessibilité depuis le départ : 99 % des terres (graines 777 et 2).

## Règles
| Relief | Déplacement | Rendements | Défense | Autre |
|---|---|---|---|---|
| Plat | terrain | terrain | — | |
| Vallonné | +1 PM | +1 production | +3 | pas de charge |
| Accidenté | +2 PM | +1 production, −1 nourriture | +3 | pas de charge |
| Montagne | 4 à 8 PM selon l'altitude | 0 / 1 / 0 | +6 | pas de charge, pas de ville, pas de bonus d'eau douce |
| Pic | infranchissable | rien | — | |
- Coûts collines / accidenté + terrain plafonnés à 3 PM (`RELIEF_MOVE_CAP`, sauf terrain déjà plus cher).

## Franchir une montagne (30/10/2026)
- **Constat de Benjamin** : les déplacements en montagne ne sont pas assez pénalisés, les trajets les plus rapides sont souvent des lignes droites.
- **Cause** : on entre sur une case dès qu'il reste 1 PM, et ce qui dépassait était perdu. L'infanterie a 2 PM : forêt (2), marais (3), accidenté (3), montagne (3), et même montagne + forte montée (5) coûtaient tous un tour par case ; la montagne n'était que deux fois plus lente que la plaine, et la cavalerie (4 PM) y faisait plaine + montagne dans le même tour. Monter le coût de la montagne n'aurait rien changé.
- **Choix de Benjamin** : les pistes 1 à 3 (dette de PM, coût selon l'altitude, coût selon l'unité) ; rivières et fleuves gardent leur règle (le coût est celui du franchissement ou de la barque) ; indication visuelle sur les unités immobilisées.
- **Dette de PM** (`StepCost`, `UnitState::move_debt`, `map::turn_moves`) : un pas a une part terrain (case d'arrivée, relief, montée) et une part eau (traversée de rivière, entrée dans un fleuve). Avec `left` PM, il reste `left − total` (au moins 0) et la dette augmente de `terrain − left` (au moins 0). Au début du tour, la dette est payée d'abord, jusqu'à tous les PM ; sans PM, l'unité est **immobilisée** (`immobilized`) — ni déplacement ni attaque. Effet de bord voulu : forêt et marais se distinguent enfin pour l'infanterie (une forêt prise avec 1 PM laisse 1 PM dû).
- **Coût selon l'altitude** (`mountain_move_cost`) : 4 PM de 1 500 à 2 000 m, +1 PM par tranche de 500 m, au plus 8 PM. Les cols, points bas des crêtes, et les vallées creusées par l'érosion deviennent les passages, sans avoir à les repérer.
- **Coût selon l'unité** (`UnitKind::mountain_cost`, appliqué par `rules::ForUnit`) : cavalerie ×2 (les chevaux passent les cols, menés à la bride, lentement : Hannibal, les Mongols dans l'Altaï ; Benjamin garde le double pour toute la cavalerie, plutôt que de la limiter aux cols ou de ne pénaliser que la cavalerie lourde), éclaireur ×2/3 arrondi au-dessus, les autres ×1.
- Exemples (montagne à 4 / 6 / 8 PM, entrée avec tous ses PM) : infanterie (2 PM) immobilisée 1 / 2 / 3 tours ; éclaireur (3 PM, coûts 3 / 4 / 6) 0 / 0 (1 PM dû) / 1 tour ; cavalerie légère (4 PM, coûts 8 / 12 / 16) 1 / 2 / 3 tours ; cavalerie lourde (3 PM) 1 (puis 1 PM) / 3 / 4 tours.
- **Recherche du plus rapide** (`find_path_turns`) : une case de passage est notée par le tour et les PM du prochain départ (la dette y est soldée, c'est tout l'état utile) ; la destination, par le tour d'arrivée. `path_turns`, `reachable`, l'approche d'une attaque, l'exploration automatique et les barbares suivent la même règle (les barbares immobilisés ne jouent pas).
- **Bilan** (`bilan_montagnes`, cartes 192 × 111) — montagnes à 4 / 5 / 6 / 7 / 8 PM : graine 777 : 24 / 54 / 101 / 109 / 90 (378) ; graine 2 : 86 / 81 / 44 / 24 / 59 (294) ; graine 5 : 44 / 82 / 92 / 67 / 85 (370). Traversées de massif par une milice (d'une case à 4 cases d'une montagne à la case opposée) : le trajet le plus rapide passe par au moins 2 montagnes dans 14 cas sur 39 (777), 8 sur 69 (2), 8 sur 52 (5) ; les autres contournent ou passent par un col.
- **Vérifié en captures** (graine 777) : pastille sur l'unité immobilisée en 3D (vue `unites_montagne`) et en 2D (nouvelle vue `unites_montagne_2d` du mode capture).
- **Affichage** : pastille orange en haut à droite du jeton (2D et 3D, `labels::StuckBadge`), avec le nombre de tours avant de repartir (`UnitState::stuck_turns`), affiché dès que la dette prend au moins un tour entier ; panneau de l'unité : « Immobilisé(e) par le terrain : repart dans N tour(s) », « Terrain difficile : immobilisé(e) N-1 tour(s), repart dans N tour(s) » (tour de l'entrée en montagne) ou « N PM dus au prochain tour ». C'est un compte à rebours : un premier calcul comptait les tours immobilisés (le tour en cours seulement s'il l'était), si bien que le nombre restait figé entre le tour de l'entrée en montagne et le suivant (dette de 4 PM à 2 PM/tour : 2, 2, 1 au lieu de 3, 2, 1).
- **Limites** : la recherche de cible de l'exploration automatique garde des coûts communs à toutes les unités (la cavalerie n'y voit pas le double de la montagne, seulement en marchant) ; une case inconnue coûte toujours 2 PM, une montagne cachée par le brouillard n'est donc découverte qu'en approchant.
- Pentes en mètres (`climb_cost`) : montée < 400 m gratuite, < 900 m +1 PM, au-delà +2 PM ; descente gratuite.
- Falaise : dénivelée d'au moins 1 000 m, la case basse sous 1 500 m (`is_cliff`) : infranchissable, pas de mêlée ni de flanquement à travers.
- Combat : `Fighter { altitude (m), relief }` ; en hauteur si ≥ 250 m (`HIGHER_GROUND_M`) : +2 défense de mêlée, +20 % de tir ; tireur plus bas d'autant : portée −1.

## Affichage 2D et cartes
- Carte Altitude : teintes continues 0 → 4 800 m (vert, vert clair, jaune, orange, brun, gris, blanc), 24 teintes en 2D.
- Carte Relief : une couleur par relief (plat crème, vallonné jaune, accidenté ocre, montagne brun, pic blanc).
- 2D : bosses (collines), bosses serrées plus sombres (accidenté), deux sommets triangulaires (montagne), grand sommet à calotte de neige (pic).
- Infobulle : « Montagne (toundra) (2 950 m) », « Forêt tempérée accidentée (1 200 m) ».

## Rendu 3D du relief (étape A)
- Plus d'objets posés (anciens massifs en facettes `mountain_mesh` et bosses en demi-sphères, supprimés) : le relief est une **déformation du sol** (`relief::lift_at`), ajoutée au profil de chaque case.
- `Bumps { amp, summit, needle, snowline }` par case explorée (`view3d::bumps_for`) : vallonné 5, accidenté 9 d'ampleur ; montagne 19 + sommet 8 en **dôme** (cloche nulle à l'apothème) ; pic 16 + sommet 34 en **aiguille** (pente qui se raidit jusqu'à la pointe, `needle`).
- **Montagnes plus visibles (01/11/2026, demande de Benjamin)** : avec la dette de PM, une montagne coûte cher ; elle se lisait mal en 3D, surtout en forêt (léger voile gris, moins visible qu'un relief accidenté). Chaque montagne a désormais son sommet ; pour éviter le damier de cônes d'un premier essai, deux cases à sommet voisines sont reliées par une **arête** (cloche autour du segment entre leurs centres) haute de `SADDLE` = 0,45 × le moins haut des deux sommets : un massif forme une chaîne de sommets séparés par des cols. Le sommet retenu en un point est le plus haut des dômes, aiguilles et arêtes (pas leur somme). Essai à 0,7 : les arêtes s'enneigeaient toutes dans le froid et noyaient les pics vus de loin.
- **Pics toujours distincts** : par la forme (aiguille plus haute de 26 que le sommet d'une montagne) et l'absence d'arbres, pas par la neige — Benjamin a écarté l'idée de réserver la neige aux pics : elle suit la température de la case et la hauteur (une montagne froide est enneigée).
- Ampleur interpolée entre les centres des cases (portée 1,6 × la taille d'une case) : continue d'une case à l'autre, deux montagnes voisines forment une seule crête ; multipliée par un **bruit en crêtes** (Perlin, 3 octaves, période ≈ 70 unités) continu sur toute la carte : déformation = ampleur × (0,2 + 0,8 × crête) + sommet.
- **Vallées** : la déformation s'efface sur 10 unités au bord des cours d'eau (`valley_fade`) ; la hauteur de l'eau (`surface`) l'ignore, les lits restent cohérents. Les fleuves traversent les massifs dans des vallées visibles.
- Couleurs : la roche gagne avec la hauteur de déformation (de 6 à 17, jusqu'à 85 % ; avant : de 5 à 19, jusqu'à 65 %, trop peu pour se voir sous le vert sombre d'une forêt) et sur les pentes raides ; **ombre des ravines** : sur un relief marqué (fondu de 4 à 16 de déformation), la couleur est multipliée par 1 + 1,2 × (crête − 0,45) (`RAVINE_SHADE`), ravines plus sombres, crêtes plus claires ; comparé avec et sans : aucun artefact, modelé un peu plus lisible sur les massifs nus, surtout de loin ; **neige** au-dessus de la limite des neiges = (température + 12) × 2, entre 10 et 40 (0 sur un terrain de neige) : collines jamais enneigées hors terrain de neige, crêtes enneigées dans le froid, seuls les hauts des pics en climat tempéré.
- **Arbres** sur les versants jusqu'à la limite des arbres (déformation < 60 % de la limite des neiges, et < 14) ; plus d'arbres au sommet des pics. Seconde couronne pour jungle et taïga, filtrée de même. **Étage montagnard** sur une case de montagne : un emplacement sur deux, pas de seconde couronne, petits conifères à la place des feuillus de la forêt tempérée ; le haut de la case reste en roche nue. Limite : dans un massif froid, le sommet ajouté fait passer bien des emplacements au-dessus de la limite des arbres ; ces massifs sont plus nus qu'avant (voulu : c'est ce qui les distingue). Rochers épars sur le relief accidenté.
- Maillage fin (11 anneaux) dès qu'une case ou une voisine a du relief ; normales calculées avec la pente de la déformation.
- Les cases inexplorées n'ont pas de relief (rien n'est révélé) ; il apparaît à l'exploration.

## Sol 3D à l'altitude réelle (étape B)
- `view3d::meters_to_height(m) = SEA_LEVEL (8) + LAND_BASE (6) + m × 0,0175` : la terre la plus basse reste à 6 unités au-dessus de la mer (côtes lisibles), 4 800 m ≈ +84 unités. Exagération verticale voulue (une case fait 40 km).
- `tile_height` : mer au niveau de la mer ; lac = cote de sa surface − `LAKE_DEPTH` ; terre = son altitude en mètres. Les cases inexplorées sont dessinées à 150 m (rien n'est révélé).
- Les profils de case (pentes douces entre voisines, rives, falaises) s'appuient désormais sur ces hauteurs continues : plateaux, bassins et vallées larges se lisent sans marches.
- La flèche de vent de la carte climat se place au-dessus de 4 800 m.
- Les falaises dessinées suivent la règle en mètres (`LevelEdge::cliff`) ; les descentes, les niveaux.

## Unités, villes, camps et villages sur le relief (30/10/2026)
- **Constat** (TODO n° 4) : ils étaient parfois « ensevelis ». Cause confirmée : leur hauteur venait de `tile_height` (altitude moyenne de la case), alors que le sol dessiné est `Relief::ground` = profil de la case + bosses (jusqu'à 5, 9, 17, et 16 + 24 pour un pic) − lits des cours d'eau ; jeton haut de 6 posé à +3. Villes et camps : terrasse (parois) mais bosses toujours appliquées sur leur case ; villages posés sur le terrain bosselé.
- **Décisions de Benjamin** : pas de terrassement qui supprime le relief ; les sites suivent le terrain ; unités en **figurines 3D faites de formes simples** (option A, plutôt que des modèles glTF), la 2D garde ses icônes.
- **Sol** : plus de terrasse (`link` n'a plus de paroi autour des villes et camps). Sous une ville, un camp ou un village, le relief interne est **adouci** (`SITE_RELIEF` = 0,5 de l'ampleur et du sommet, `explored_bumps`), pas aplani ; le sol alentour est refait quand un site apparaît ou disparaît.
- **Placement** (`sync_mirrors`) : chaque représentation est replacée quand le sol de sa case est reconstruit (`Relief::generation`, compteur par case, incrémenté à chaque reconstruction) ou quand elle change de case. Emprise = centre + 6 points à un rayon donné (`footprint` : points bas et haut du sol).
  - Éléments d'un site (`Grounded`) posés chacun à sa place : bâti (`Rest::Stand` : remparts, tours, maisons, donjon, huttes, pieux, mât) de la base au point bas (fondations) au sommet à sa hauteur au-dessus du point haut (pièce étirée) ; toits et pointes (`Rest::Top`) sur le point haut ; tentes, feu, pierres (`Rest::Low`) sur le point bas. Le donjon est au centre, la garnison sur son toit.
  - Sols drapés (`Patch3d`) : pavés de la ville (hexagone, 0,9 case) et terre battue du camp (disque de 25), maillage en anneaux posé sur le relief (+0,35), refait au replacement ; remplacent le socle pavé et le disque plats.
  - Unités : figurine debout, pieds au point haut du sol sous elle (rayon 3), jamais enfoncée ; **anneau** au sol aux couleurs du camp (tore, rayon 5,5, épaisseur 0,6) incliné selon la pente (normale par différences finies). Le premier socle (disque plein de rayon 6) était trop lourd : retiré à la demande de Benjamin.
  - **Barque** sur un fleuve (`map.is_fleuve`, lits connus) : coque et deux pointes en bois (≈ 6,5 × 19), orientée selon le tronçon de lit le plus proche, coque à la surface de l'eau (`relief::water_surface`), la figurine sur le pont (+1,4) ; pas d'anneau. Demande de Benjamin (« un petit navire en bois pour les transporter ») : affichage seulement, les règles de déplacement ne changent pas.
- **Figurines** (`figures.rs`) : ≈ 12 de haut à pied ; cavaliers sur un cheval (pavé, pattes, encolure, tête). Retour de Benjamin (« les colons se confondent avec les guerriers et les éclaireurs ») : une silhouette par type, lisible de loin. Colon : habit de lin, chapeau de paille à large bord, charrette à bras (caisse, deux roues, brancards) chargée de sacs et d'un ballot ; éclaireur : habit et capuche verts, bâton, **un chien** à ses côtés (la première version, une longue cape conique, faisait bizarre selon Benjamin) ; milice : cuir, massue ; porteur de bouclier : bouclier rond aux couleurs du camp (umbo de métal), épée ; guerrier lourd : armure sombre, casque à plumet aux couleurs du camp, grand bouclier rectangulaire ; lancier : casque, bouclier rond, lance ; piquier : pique de 24, un peu inclinée ; archer-épéiste : arc haut (trois branches), épée ; archer d'élite : capuche verte, arc, carquois ; cavalier léger : cheval clair, lance inclinée ; cavalier lourd : armure sombre, plumet, cheval caparaçonné aux couleurs du camp, lance couchée ; archer à cheval : arc et carquois. Les militaires portent la tunique de leur camp.
- **Visibilité au dézoom** (choix de Benjamin : groupes + agrandissement) : les militaires sont un groupe (`figures::group` : trois fantassins en triangle, deux cavaliers côte à côte ; colon et éclaireur seuls), chaque figurine (`Member`) posée au point haut du sol sous ses pieds (rayon 3) ; resserrée (× 0,45) et tournée avec la barque sur un fleuve. Les figurines, l'anneau (rayon 8,5 autour du groupe) et la barque grandissent avec la distance de la caméra (`scale_units` : × distance / 450, de 1 à 3 ; × 2 à la distance par défaut de 900 ; un premier réglage, / 700 et 2,4 au plus, laissait les groupes trop petits en captures), et le groupe s'écarte d'autant (pas de chevauchement).
- **Icône au loin** (`FarIcon`) : au-delà d'une distance de caméra de 1 100, l'icône du type flotte au-dessus de la figurine, tournée vers la caméra (de près, la figurine suffit), sur une pastille aux couleurs du camp (non éclairée), agrandie avec la distance (taille à peu près constante à l'écran ; 0,9 × à l'apparition : sans chevauchement entre cases voisines). Placée simplement au-dessus du groupe, elle était traversée par les figurines agrandies (≈ 35 de haut à 3 ×, icône entre 6 et 30) et barrée par les lignes 3D (fort `depth_bias` des gizmos) : elle est donc **avancée vers l'œil de la caméra d'un quart de la distance** le long du rayon qui la vise (rayon pris par le plus court autour de la planète, pour les unités vues par une caméra fantôme), et réduite d'autant (`FAR_ICON_PULL` = 0,25) : même place et même taille à l'écran, mais devant tout ce qui l'entoure. Captures `unites_seuil_icones` (distance 1 150, juste après l'apparition) et `ville_loin` ajoutées pour le vérifier.
- **Vérifié en captures** (mode `CIVBUILDER_CAPTURE`, graine 777) : ville sur un relief accidenté au bord d'une falaise, douze types d'unités sur les pentes d'une montagne (aucune enfoncée), camp en forêt, village, vue lointaine. Premières icônes lointaines trop petites (blanches sans fond), puis trop grandes : réglées.
- **Limites** : un rempart long sur une forte pente est étiré de son point bas à son point haut (dessus droit) ; pas d'animation ; les barres de vie n'existent pas en 3D.

## Performances de la construction du relief 3D (30/10/2026)
- **Mesure** (trace Chrome de Bevy, `cargo build --release --features bevy/trace_chrome`, mode capture graine 777, carte révélée, Ryzen 7 7800X3D / RTX 5070) : `update_relief_3d` prenait ≈ 14 ms par image (au plus 26) pendant ≈ 19 s, pour un budget de 6 ms → 45 à 52 images/s. Détail : file triée à chaque image 2,3 → 0,1 ms, préparation 1,4 ms, construction parallèle 5,3 ms (24 ms de calcul : ≈ 4,6 cœurs occupés sur 16, paquets de 12 cases), **surfaces d'eau 5 à 11 ms hors budget, sur un seul cœur** (dont ≈ 2,4 ms de `Relief::channel` calculé pour chaque triangle alors qu'il ne sert qu'en secours, ≈ 3 ms de `nearest_water` par sommet). Au repos, ≈ 7 ms de travail par image (bridé à 60 images/s par la synchronisation verticale).
- **Changements** (choix de Benjamin : pistes 1, 2, 4, 5 et 6 de l'analyse) :
  1. Surfaces d'eau construites **en parallèle** et comptées dans le budget, qui couvre désormais tout le système : un paquet n'est lancé que si sa durée et les surfaces d'eau de toutes les cases traitées (moyennes glissantes des images précédentes, `ReliefQueue::chunk_ms` / `water_ms`) tiennent dans le budget ; au moins un paquet par image.
  2. `Relief::channel` (hauteurs d'un tronçon) calculé seulement quand un point de l'eau est hors de portée des lits.
  4. Tables du jeu (`HashMap` / `HashSet`) passées au hachage rapide de Bevy (`bevy::platform::collections`, foldhash) dans tout le code : clés produites par le jeu, la résistance de SipHash aux attaques est inutile ; ordre de parcours fixe d'un lancement à l'autre.
  5. Paquets de **2 cases par cœur** (`RELIEF_CHUNK_PER_THREAD`, 32 sur 16 fils) au lieu de 12 : cases simples et fines mieux réparties.
  6. **Ordre de traitement gardé d'une image à l'autre** (`ReliefQueue::order`, tas trié par distance au centre de la vue) : refait seulement quand la vue s'est déplacée de plus de 3 cases (`RELIEF_REFOCUS`) ou après un changement de carte ; les cases remises en file y sont ajoutées une à une, une case déjà traitée est sautée en sortant.
- **Résultat** (même scénario) : relief construit en ≈ 7 s au lieu de ≈ 19 s, **60 images/s tenues pendant la construction** ; `update_relief_3d` ≈ 5 ms par image en moyenne, au plus ≈ 8 ms. Captures identiques au pixel près (hors infobulle de survol), 0 case aux lits périmés.
- **Changement de carte en plein défilement** (constat de Benjamin : baisse d'images par seconde sur les cartes du réseau hydrographique et des plaques en 3D). Mesure (scénario temporaire : vue qui défile vers la gauche, changement de carte en route) : pendant ≈ 2 s, 50 à 56 images/s, pire image 25 ms ; s'additionnent le relief recoloré (≈ 5,6 ms, toutes les cases explorées changent de couleur), l'envoi des maillages refaits (≈ 2,4 ms) et `draw_plates_3d` (7,4 ms) ou `draw_network_map` (6,2 ms), qui posaient à chaque image tous les tracés de la carte explorée sur le relief (`Relief::on_ground`). En profil de développement (`cargo run`), ces deux systèmes prennent 7 à 9,5 ms.
- **Corrections** (choix de Benjamin : les deux) :
  1. **Tracés posés sur le sol gardés d'une image à l'autre** (`PlatesCache`, `highlight::NetworkCache`) : recalculés quand l'exploration change ou quand la forme du sol change (`Relief::shape`, version incrémentée quand un profil de case est recalculé, quand le relief interne change ou quand les lits d'une case changent ; pas pour un simple changement de couleurs). Les chevrons animés du réseau restent posés exactement sur le sol à chaque image. Plaques 7,4 → 1,0 ms, réseau 6,2 → 3,2 ms.
  2. **Budget du relief adaptatif** : une image de plus de 17,5 ms (`RELIEF_SLOW_FRAME_MS`) retire 15 % du budget (jusqu'à 75 %, `RELIEF_SLOWDOWN_MAX`), qui revient de 2 % par image ensuite.
- **Résultat** : 60 images/s tenues au changement de carte en défilement (une seule image à ≈ 21 ms, celle qui calcule les tracés). Tracés vérifiés en captures (cours d'eau et chevrons sur l'eau, frontières et flèches sur le sol).
- **Non retenu pour l'instant** : piste 3 (chercher les étendues d'eau proches une fois par maillage plutôt qu'à chaque sommet) ; tableaux indexés par case à la place des tables les plus consultées.
- **Non mesuré** : vue 2D, fin de tour, coût du rendu sur la carte graphique.

## Saccades au zoom et au déplacement (04/11/2026)
- **Constat de Benjamin** : saccades en zoomant et dézoomant en se déplaçant sur la carte.
- **Mesure** : nouvelle commande de console `travelling col lig distance [images]` (vue déplacée en douceur, zoom en échelle logarithmique, souris tenue au milieu de la vue : la case survolée change en chemin), qui donne les durées d'image (moyenne, pire, à-coups au-delà de 33, 50 et 100 ms). Scénario : graine 777, carte révélée, profil de développement (`cargo run`), six longs déplacements rapides en 3D (de 2 800 à 260 de distance) puis trois en 2D. **Avant** les travaux sur les cours d'eau (commit 5bd1a12) : pire image 22 ms, aucun à-coup. **Après** (rivières en courbes de six tronçons, affluents, panaches, rapides, îles) : pire 43 ms, 18 à-coups au-delà de 33 ms en 3D (2D inchangée, fluide). Un paquet de relief (32 cases, au moins un par image) prenait 10 à 17 ms : préparation **en série** de 5 à 8 ms (hauteurs d'eau des lits des cases du paquet et de leurs voisines, `Relief::channel` : 4 à 6 fois plus de lits par case qu'avant), construction parallèle de 3 à 7 ms ; surfaces d'eau 5 à 11 ms.
- **Corrections** :
  1. Hauteurs d'eau des lits calculées **en parallèle** avant la préparation (préparation 5-8 → ≈ 1,4 ms) ; pas de calcul pour un lit d'île (pas d'eau).
  2. **Grille de lits** (`relief::ChannelGrid`, mailles de 8 unités) : chaque point du sol ou de l'eau ne regarde que les lits à sa portée, au lieu de tous ceux de la case et de ses voisines ; même résultat (test `grille_de_lits_comme_la_liste_entiere`).
  3. Surfaces d'eau : hauteur de chaque point calculée une fois par triangle (un sommet sert au découpage des chutes et au maillage) ; mers et lacs d'alentour cherchés **une fois par triangle** (piste 3 de l'analyse du 30/10, non retenue alors), la distance au plus proche à chaque sommet. Surfaces d'eau ≈ 2 à 3 ms.
- **Résultat** (même scénario) : un seul à-coup (36 ms) au lieu de 18, pire image 36 ms au lieu de 43 ; paquet de relief ≈ 8 ms. Rendu identique en captures. Reste : la construction d'un paquet (32 cases) est indivisible et coûte encore plus que le budget de 6 ms quand beaucoup de cases à cours d'eau arrivent d'un coup.

## Vitesse d'affichage des cases selon la carte (06/11/2026)
- **Mesure** (demande de Benjamin) : CSV image par image (instrumentation temporaire, non gardée), nouveau processus à chaque essai pour tout recalculer, carte révélée, 192×111, graine 777 sauf mention contraire, profil de développement, écran à 59 Hz, Ryzen 7 7800X3D et RTX 5070, avec et sans défilement à la vitesse des touches.
- **Avant** : toute la carte en ≈ 11 s (≈ 9,8 s en défilant) à 60 images/s, sauf plaques (16,2 s ; 17,0 s en défilant, 19-20 ms par image) et réseau (13,7 s ; 15,4 s, 18-20 ms par image). Un changement de carte à chaud (couleurs seules) prend ≈ 3 s, le réseau 4,4 s (6,6 s en défilant). Carte graphique peu sollicitée (utilisation ≤ 10 % sur 95 % des relevés) : le processeur limite.
- **Causes** :
  1. Pendant la construction, `Relief::shape` change à presque chaque image : les tracés gardés (`PlatesCache`, `NetworkCache`, voir plus haut) étaient donc reposés sur le sol à chaque image. L'image dépassait 17,5 ms et le budget adaptatif descendait au minimum (un paquet, 32 cases par image).
  2. Une fois la carte construite, le réseau coûtait encore 5,3 ms par image : les chevrons animés en prenaient 5,26 (sans eux, 0,06 ms), à cause de trois `Relief::on_ground` pour chacun des ≈ 2 420 chevrons de la carte, y compris hors de l'écran.
- **Corrections** :
  1. **Recalcul limité pendant la construction** (`view3d::ground_lines_stale`) : au plus une fois par 0,5 s (`GROUND_LINES_REFRESH_S`) tant que le relief se construit ; aussitôt sans tracés ou dès que tout est fini. Pendant la construction, des tracés peuvent donc rester décalés du sol jusqu'à 0,5 s.
  2. **Chevrons du réseau limités au champ de vision** (`highlight::in_view`) : champs des caméras 3D allumées, caméras du raccord est-ouest comprises, avec une marge (`CHEVRON_VIEW_MARGIN`) ; ceux qui sont à l'écran restent posés exactement sur le sol à chaque image. **Le plan lointain ne doit pas compter** (`intersects_sphere(…, false)`, comme Bevy pour les maillages) : il reste à 1 000 unités alors que la caméra peut être à 2 800, et le compter retirait les chevrons de la vue lointaine.
- **Résultat** (même protocole, 2 à 3 essais) :

  | Carte | Construction | Construction en défilant | Durée d'image | Réseau, carte construite |
  |---|---|---|---|---|
  | Plaques | 16,2 → 10,8 s | 17,0 → 9,8 s | 19-20 → 16,7 ms | — |
  | Réseau | 13,7 → 10,8 s | 15,4 → 9,7 s | 18-20 → 16,7 ms | 5,3 → 0,3 ms (2800 : 0,5 ms ; en défilant 0,15 à 1,7 ms) |

  Audit temporaire des chevrons contre leur projection à l'écran, sur 1 196 images (distances 900 et 2800, avec et sans défilement) : 0 chevron visible écarté. 16 à 528 chevrons à l'écran sur ≈ 2 420.
- **Non traité** :
  - Budget : le débit vaut ≈ 42 cases × images par seconde (sans synchronisation verticale : 6,7 s au lieu de 11). Un paquet coûte ≈ 4,1 ms, donc 78 % des images n'en traitent qu'un alors qu'il reste de la marge.
  - 32 % de traitements en plus du nombre de cases (rivières voisines refaites).
  - Flèches du climat et des courants, recalculées à chaque image (fil principal à 8 et 6 ms au lieu de 4 quand la carte est construite).
  - `draw_tide_lines_3d` : même invalidation que les tracés corrigés, non mesurée avec ses options activées.
  - À-coup de 60 à 100 ms vers x ≈ 8 260 en défilant à la distance 2800, côté extraction ou rendu, cause non identifiée.
  - Version distribuée (release) non mesurée.

## Limites et suite
- Le bruit d'élévation donne de larges hauts plateaux : le massif le plus haut reste une grande zone de montagnes (désormais franchissable). L'étape C (chaînes + érosion) doit le découper en crêtes et vallées.
- Les pics ressemblent encore à des cônes arrondis enneigés ; collines discrètes sous les forêts denses.
- Cols : non encore identifiés comme tels (points bas des crêtes), à faire avec les chaînes.
