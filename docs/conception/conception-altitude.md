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
- **Montagne franchissable à 3 PM, seuls les pics infranchissables.**
- 5 reliefs : plat, vallonné, accidenté, montagne, pic (la représentation pourra être revue).
- Ajout des cartes **Altitude** (m) et **Relief** (plat → pic).
- Plutôt que plus de niveaux de sol (7 auparavant : mer + 6 tranches), passer à l'altitude continue (étape B).

## Modèle
- `WorldMap::elevation` (m) : référence ; `levels` (classes 1 à 6) ne servent plus qu'à la détection des falaises (écart ≥ 3 classes, ≈ 1 000 m) et aux bords dessinés (falaises, descentes), pour que règles et dessin concordent.
- `WorldMap::lake_level_m` : cote de la surface de chaque lac (min sur le lac de la surface hydrologique et de la plus basse rive, ≥ 0), identique pour toutes les cases d'un même lac.
- `WorldMap::surface_elevation(hex)` : 0 en mer, cote du lac pour un lac, sinon altitude du sol (≥ 0).
- `Relief` (enum, remplace l'ancien booléen « collines ») ; le terrain `Montagnes` disparaît : une montagne porte le biome de son climat (souvent toundra ou neige).
- Le climat reçoit l'altitude en mètres (`Surface::Land(m)`) au lieu de l'altitude moyenne du niveau.

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
| Montagne | 3 PM | 0 / 1 / 0 | +6 | pas de charge, pas de ville, pas de bonus d'eau douce |
| Pic | infranchissable | rien | — | |
- Coûts relief + terrain plafonnés à 3 PM (sauf terrain déjà plus cher).
- Pentes en mètres (`climb_cost`) : montée < 400 m gratuite, < 900 m +1 PM, au-delà +2 PM ; descente gratuite.
- Falaise : écart de 3 classes d'altitude (≈ 1 000 m) : infranchissable, pas de mêlée ni de flanquement à travers.
- Combat : `Fighter { altitude (m), relief }` ; en hauteur si ≥ 250 m (`HIGHER_GROUND_M`) : +2 défense de mêlée, +20 % de tir ; tireur plus bas d'autant : portée −1.

## Affichage 2D et cartes
- Carte Altitude : teintes continues 0 → 4 800 m (vert, vert clair, jaune, orange, brun, gris, blanc), 24 teintes en 2D.
- Carte Relief : une couleur par relief (plat crème, vallonné jaune, accidenté ocre, montagne brun, pic blanc).
- 2D : bosses (collines), bosses serrées plus sombres (accidenté), deux sommets triangulaires (montagne), grand sommet à calotte de neige (pic).
- Infobulle : « Montagne (toundra) (2 950 m) », « Forêt tempérée accidentée (1 200 m) ».

## Rendu 3D du relief (étape A)
- Plus d'objets posés (anciens massifs en facettes `mountain_mesh` et bosses en demi-sphères, supprimés) : le relief est une **déformation du sol** (`relief::lift_at`), ajoutée au profil de chaque case.
- `Bumps { amp, summit, snowline }` par case explorée (`view3d::bumps_for`) : vallonné 5, accidenté 9, montagne 17 d'ampleur ; pic 16 + sommet 24. Pas de sommet propre aux montagnes (un premier essai donnait un damier de cônes) : seuls les pics en ont un (cloche nulle au bord de la case).
- Ampleur interpolée entre les centres des cases (portée 1,6 × la taille d'une case) : continue d'une case à l'autre, deux montagnes voisines forment une seule crête ; multipliée par un **bruit en crêtes** (Perlin, 3 octaves, période ≈ 70 unités) continu sur toute la carte : déformation = ampleur × (0,2 + 0,8 × crête) + sommet.
- **Vallées** : la déformation s'efface sur 10 unités au bord des cours d'eau (`valley_fade`) ; la hauteur de l'eau (`surface`) l'ignore, les lits restent cohérents. Les fleuves traversent les massifs dans des vallées visibles.
- Couleurs : la roche gagne avec la hauteur de déformation (jusqu'à 65 %) et sur les pentes raides ; **neige** au-dessus de la limite des neiges = (température + 12) × 2, entre 10 et 40 (0 sur un terrain de neige) : collines jamais enneigées hors terrain de neige, crêtes enneigées dans le froid, seuls les hauts des pics en climat tempéré.
- **Arbres** sur les versants jusqu'à la limite des arbres (déformation < 60 % de la limite des neiges, et < 14) ; plus d'arbres au sommet des pics. Seconde couronne pour jungle et taïga, filtrée de même. Rochers épars sur le relief accidenté.
- Maillage fin (11 anneaux) dès qu'une case ou une voisine a du relief ; normales calculées avec la pente de la déformation.
- Les cases inexplorées n'ont pas de relief (rien n'est révélé) ; il apparaît à l'exploration.

## Sol 3D à l'altitude réelle (étape B)
- `view3d::meters_to_height(m) = SEA_LEVEL (8) + LAND_BASE (6) + m × 0,0175` : la terre la plus basse reste à 6 unités au-dessus de la mer (côtes lisibles), 4 800 m ≈ +84 unités. Exagération verticale voulue (une case fait 40 km).
- `tile_height` : mer au niveau de la mer ; lac = cote de sa surface − `LAKE_DEPTH` ; terre = son altitude en mètres. Les cases inexplorées sont dessinées à 150 m (rien n'est révélé).
- Les profils de case (pentes douces entre voisines, rives, falaises) s'appuient désormais sur ces hauteurs continues : plateaux, bassins et vallées larges se lisent sans marches.
- La flèche de vent de la carte climat se place au-dessus de 4 800 m.
- Les falaises et descentes dessinées gardent la détection par classes, comme les règles.

## Unités, villes, camps et villages sur le relief (30/10/2026)
- **Constat** (TODO n° 4) : ils étaient parfois « ensevelis ». Cause confirmée : leur hauteur venait de `tile_height` (altitude moyenne de la case), alors que le sol dessiné est `Relief::ground` = profil de la case + bosses (jusqu'à 5, 9, 17, et 16 + 24 pour un pic) − lits des cours d'eau ; jeton haut de 6 posé à +3. Villes et camps : terrasse (parois) mais bosses toujours appliquées sur leur case ; villages posés sur le terrain bosselé.
- **Décisions de Benjamin** : pas de terrassement qui supprime le relief ; les sites suivent le terrain ; unités en **figurines 3D faites de formes simples** (option A, plutôt que des modèles glTF), la 2D garde ses icônes.
- **Sol** : plus de terrasse (`link` n'a plus de paroi autour des villes et camps). Sous une ville, un camp ou un village, le relief interne est **adouci** (`SITE_RELIEF` = 0,5 de l'ampleur et du sommet, `explored_bumps`), pas aplani ; le sol alentour est refait quand un site apparaît ou disparaît.
- **Placement** (`sync_mirrors`) : chaque représentation est replacée quand le sol de sa case est reconstruit (`Relief::generation`, compteur par case, incrémenté à chaque reconstruction) ou quand elle change de case. Emprise = centre + 6 points à un rayon donné (`footprint` : points bas et haut du sol).
  - Éléments d'un site (`Grounded`) posés chacun à sa place : bâti (`Rest::Stand` : remparts, tours, maisons, donjon, huttes, pieux, mât) de la base au point bas (fondations) au sommet à sa hauteur au-dessus du point haut (pièce étirée) ; toits et pointes (`Rest::Top`) sur le point haut ; tentes, feu, pierres (`Rest::Low`) sur le point bas. Le donjon est au centre, la garnison sur son toit.
  - Sols drapés (`Patch3d`) : pavés de la ville (hexagone, 0,9 case) et terre battue du camp (disque de 25), maillage en anneaux posé sur le relief (+0,35), refait au replacement ; remplacent le socle pavé et le disque plats.
  - Unités : figurine debout, pieds au point haut du sol sous elle (rayon 3), jamais enfoncée ; **anneau** au sol aux couleurs du camp (tore, rayon 5,5, épaisseur 0,6) incliné selon la pente (normale par différences finies). Le premier socle (disque plein de rayon 6) était trop lourd : retiré à la demande de Benjamin.
  - **Barque** sur un fleuve (`map.is_fleuve`, lits connus) : coque et deux pointes en bois (≈ 6,5 × 19), orientée selon le tronçon de lit le plus proche, coque à la surface de l'eau (`relief::water_surface`), la figurine sur le pont (+1,4) ; pas d'anneau. Demande de Benjamin (« un petit navire en bois pour les transporter ») : affichage seulement, les règles de déplacement ne changent pas.
- **Figurines** (`figures.rs`) : ≈ 12 de haut à pied ; cavaliers sur un cheval (pavé, pattes, encolure, tête). Retour de Benjamin (« les colons se confondent avec les guerriers et les éclaireurs ») : une silhouette par type, lisible de loin. Colon : habit de lin, chapeau de paille à large bord, charrette à bras (caisse, deux roues, brancards) chargée de sacs et d'un ballot ; éclaireur : longue cape et capuche vertes, bâton ; milice : cuir, massue ; porteur de bouclier : bouclier rond aux couleurs du camp (umbo de métal), épée ; guerrier lourd : armure sombre, casque à plumet aux couleurs du camp, grand bouclier rectangulaire ; lancier : casque, bouclier rond, lance ; piquier : pique de 24, un peu inclinée ; archer-épéiste : arc haut (trois branches), épée ; archer d'élite : capuche verte, arc, carquois ; cavalier léger : cheval clair, lance inclinée ; cavalier lourd : armure sombre, plumet, cheval caparaçonné aux couleurs du camp, lance couchée ; archer à cheval : arc et carquois. Les militaires portent la tunique de leur camp.
- **Icône au loin** (`FarIcon`) : au-delà d'une distance de caméra de 1 100, l'icône du type flotte au-dessus de la figurine, tournée vers la caméra (de près, la figurine suffit), sur une pastille aux couleurs du camp (non éclairée), agrandie avec la distance (taille à peu près constante à l'écran ; 0,9 × à l'apparition : sans chevauchement entre cases voisines).
- **Vérifié en captures** (mode `CIVBUILDER_CAPTURE`, graine 777) : ville sur un relief accidenté au bord d'une falaise, douze types d'unités sur les pentes d'une montagne (aucune enfoncée), camp en forêt, village, vue lointaine. Premières icônes lointaines trop petites (blanches sans fond), puis trop grandes : réglées.
- **Limites** : un rempart long sur une forte pente est étiré de son point bas à son point haut (dessus droit) ; pas d'animation ; les barres de vie n'existent pas en 3D.

## Limites et suite
- Le bruit d'élévation donne de larges hauts plateaux : le massif le plus haut reste une grande zone de montagnes (désormais franchissable). L'étape C (chaînes + érosion) doit le découper en crêtes et vallées.
- Les pics ressemblent encore à des cônes arrondis enneigés ; collines discrètes sous les forêts denses.
- Cols : non encore identifiés comme tels (points bas des crêtes), à faire avec les chaînes.
