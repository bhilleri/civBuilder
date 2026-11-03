# CivBuilder — Conception des cours d'eau et des lacs (v1, version 0.1.0 « River Update »)

Implémenté dans `src/hydro.rs` (simulation, logique pure), `src/basins.rs` (bassins versants, noms des cours d'eau et des lacs, logique pure), `src/waterways.rs` (formes à dessiner et axes des lits, 2D et 3D), `src/relief.rs` (lits taillés et plages dans le relief 3D), intégration dans `map.rs` (génération, rendements, déplacements), `render.rs` / `view3d.rs` (affichage), `labels.rs` (étiquettes sur la carte), `highlight.rs` (surbrillance au survol, carte du réseau), `tooltip.rs` / `options.rs` (infobulle et panneau d'options), `combat.rs`, `rules.rs`, `ui.rs`, `wiki.rs`.

## Choix de Benjamin (23-24/10/2026)

1. **Petites rivières sur les arêtes** entre les cases : on sait de quel côté sont les unités à pied.
2. **Grande rivière = case de fleuve** : « case aménageable par la ville avec des berges de chaque côté, mais essentiellement de l'eau ». Les villes devront pouvoir y construire des ponts, des moulins à eau…
3. **Petits lacs = élément de case** ; au-delà d'une certaine taille, **cases d'eau**, éventuellement en altitude.
4. La **fonte des neiges** viendra plus tard (avec les saisons).
5. Rendu 3D : cours d'eau **légèrement sous le niveau du sol**, **berges en relief**, **transitions de niveau amorties**.
6. Plages refaites, effet de cascade. Le **lissage du dénivelé des cours d'eau** sera revu avec la **refonte du dénivelé** (relief géologique) à venir.
7. Corrections (1er retour) : eau qui ne reposait pas dans son lit, jonction rivière/océan, petit lac dans un fleuve, marche entre deux tronçons, noir sous un lac.
8. Corrections (2e retour) : affichage 3D plus lent (cases blanches en attendant), niveau des fleuves et des rivières non lissé entre les cases, berges dégradées.
9. Corrections (3e retour) : fentes noires et fausse cascade aux changements de niveau, jonctions lac/fleuve et embouchures, teinte en dents de scie de l'autre côté du fleuve.
10. Corrections suivantes : hauteur, couleur et largeur aux jonctions fleuve/lac et fleuve/océan ; berges d'un fleuve adaptées aux cases voisines ; lacs voisins à des hauteurs différentes ; zones noires (gorges, remblai abrupt) le long des rivières ; baisse de FPS au changement de carte.
11. **Fleuve qui arrive dans un coin de lac** (ou de mer) : plutôt qu'une mince langue de terre entre le fleuve et la seconde case d'eau, le fleuve se jette dans les deux (choix « accepter que le fleuve soit connecté à deux lacs en entrée »).
12. **Virage serré juste avant l'embouchure** : la pointe de terre entre le tronçon amont et le lac est effacée (l'eau occupe l'intérieur du virage).
13. **Fleuve qui longe un lac** : il est connecté au lac sur toutes leurs arêtes communes (une seule nappe d'eau). Pas pour la mer : un fleuve côtier garde sa rive.
14. **Deux cartes hydrologiques** : bassins versants et débits des cours d'eau. **Noms des cours d'eau majeurs** inspirés des plus grands fleuves réels aux mêmes latitudes.
15. **Étiquettes** : grands noms seuls de loin, pas de chevauchement (décalage le long du cours), tailles selon l'importance ; **filtre** (noms des cours d'eau, des lacs et mers, des villes) ; en **zoom très fin, le nom de tous les cours d'eau** (rivières des arêtes comprises).
16. **Rivières au bord de la zone explorée** : eau coupée net au bord des cases explorées ; le niveau de l'eau ignore le relief (provisoire, plat) des cases inexplorées.
17. **Un seul nom par cours d'eau** (plus de noms différents sur un même fleuve ni de noms en cascade sur un delta) ; **nom conservé à travers un lac d'une ou deux cases** ; **grands lacs nommés** selon la latitude (repères en multijoueur) ; **surbrillance au survol** (cours d'eau en rouge, affluents en orange, cours récepteur en bleu) ; **cases qui desservent deux bassins hachurées**.
18. **Performances de l'exploration automatique** (36 unités : plus de 2 s en fin de tour) : recherche de cible réécrite.
19. **Surbrillance et infobulle réglables** : lacs intégrés à la surbrillance ; affluents récursifs en jaune ; mode au choix (survol du cours d'eau, survol de l'étiquette, aucun) ; rubriques de l'infobulle réglables pour chaque carte (voir `conception-options-carte.md`).
20. **Lacs et mers intérieures** : un fleuve presque entouré par un lac en fait partie (lac Nasser) ; un petit lac en amont d'un cours d'eau entre dans son réseau ; distinction **lac d'eau douce / lac salé / mer intérieure**, toutes nommées avec leurs affluents ; **tracés reliés aux confluents** (un segment par connexion) ; **chevrons animés** dans le sens du courant.
21. **Petits bras tracés** (une rivière qui se jette dans un bras d'une case n'était pas reliée), **aval récursif en violet**, **carte du réseau hydrographique** colorée selon le rang par rapport au déversoir (voir `conception-options-carte.md`).
22. **Rivière d'arête au bord de l'exploration non dessinée en 3D** (la case qui la porte était inexplorée : eau à une hauteur fixe, sous la berge taillée) : la case reprend les lits de ses voisines explorées.
23. **Bassin fermé nommé d'après son lac** : un bassin qui finit dans un lac salé ou une mer intérieure nommés porte leur nom (« Lac Eyre »), pas celui du fleuve qui s'y jette. Noms : Mar Chiquita, mer Morte et mer de Salton passées dans les lacs salés ; mers intérieures étendues à 50 noms (tropiques et hémisphère sud).
24. **Plus de grands fleuves qui finissent dans une case de mer isolée** : les petites étendues de mer enclavées deviennent des cuvettes que l'hydrologie remplit (voir « Mares de mer »).
25. **Variabilité des cours d'eau** (02/11/2026, pistes A à D choisies) : largeur des fleuves selon le débit, méandres des fleuves, rivières plus contrastées et légèrement bombées, teinte de l'eau selon le débit et le limon des terres traversées. Voir « Affichage › Formes et teintes des cours d'eau ».

## Chaîne de calcul (à la génération, après le climat)

Entrées par case : altitude continue (m), mer, ruissellement (mm/an, trop-plein du seau de sol du climat), pluie, évaporation potentielle (Hargreaves), indice d'aridité.

0. **Mares de mer** (`map.rs`, avant le relief et le climat) : une étendue de mer de moins de `MIN_INLAND_SEA_CELLS` (3) cases qui ne touche pas le bord de la carte devient une case de terre de niveau 1 à l'altitude `POND_DEPTH_M` (−60 m) : une cuvette. Le climat la traite comme de la terre ; l'hydrologie la remplit : lac (souvent agrandi aux cases basses voisines) qui déborde vers la vraie mer, ou cuvette sèche si l'eau manque. Graine 777 : 2 mares, devenues des lacs (dont le lac Victoria, 16 cases, qui reçoit le Fly et le Niger et se déverse dans l'Amazone).
1. **Altitude continue** : le bruit d'élévation est converti en mètres par interpolation entre les bornes des niveaux (300, 650, 1 075, 1 550, 2 100 m), pied des sommets 3 000 m, plus hauts pics 4 800 m. Les 6 niveaux de jeu en restent un découpage.
2. **Écoulement case à case** : Priority-Flood (Barnes et al., 2014) depuis la mer, avec une pente minimale de 1 mm par case dans les zones plates ou remplies ; chaque case s'écoule vers la voisine qui l'a atteinte (la plus basse).
3. **Cuvettes** : cases remplies voisines au même niveau (à 0,5 m près). Fond = case la plus basse ; exutoire = case hors cuvette la plus basse vers laquelle elle s'écoulait ; col = niveau de l'exutoire.
4. **Bilan d'un lac** : évaporation nette = 1,1 × ETP − pluie (≥ 0). Surface pleine = somme sur les cases de la cuvette de min(1, profondeur sous le col / 60 m) × surface d'une case. Si apports ≥ évaporation de la cuvette pleine, le lac **déborde** ; sinon **lac terminal** de surface apports / évaporation nette.
   - Surface ≥ ½ case : les N cases les plus basses deviennent du terrain **Lac**, au même niveau.
   - Sinon, surface ≥ 15 km² : **petit lac** dans la case du fond (jamais dans un fleuve).
5. **Deux passes** : la première dit quels lacs débordent ; ensuite l'eau d'un lac qui déborde traverse la cuvette du fond au col.
6. **Débits** d'amont en aval, en m³/s réels. **Pertes de lit** en climat sec. Le débit peut **baisser vers l'aval**.
7. **Fleuve** : débit ≥ 2 000 m³/s ; il le reste vers l'aval tant qu'il garde la moitié du seuil. **Fleuve absorbé par un lac** (`FLEUVE_IN_LAKE` = 3).
8. **Rivières sur les arêtes** : graphe des coins ; rivière tracée si débit ≥ 100 m³/s **et** bassin ≥ 6 cases, entre deux cases de terre. Gardé : `RiverSegment`, `corner_outlet`, `corner_cells`.
9. **Contrôle** : ruissellement total = arrivée à la mer + évaporation des lacs + pertes de lit (tests).
10. **Bassins versants et noms** (`basins.rs`, voir plus bas).

Repères de débits : Charente ≈ 100 m³/s, Rhône 1 700, Congo 41 000.

## Bassins versants et noms (`basins.rs`)

- **Bassin d'une case** : en suivant la direction aval, la dernière case avant la mer ou le bord de la carte, ou le fond d'un lac sans issue. Bassin **fermé** : lac salé, ou mer intérieure.
- **Nom d'un bassin** (`Basins::basin_name`) : pour un bassin fermé, le lac salé ou la mer intérieure nommés où finit son eau (`Basin::lake`) ; sinon son fleuve principal (`Basin::river`). **Grand bassin** (`Basin::major`) : l'un ou l'autre.
- **Cohérence avec les rivières des arêtes** : plus de la moitié des coins vers un autre bassin → la case le rejoint ; exactement la moitié et l'un des deux est grand → hachurée (`basin_also`).
- **Tronçons de fleuve, depuis les embouchures**, en **traversant les lacs de 1 ou 2 cases** (`PASS_LAKE_CELLS`).
- **Petits bras** (moins de 3 cases) : nom et tracé du cours récepteur (`NamedRiver::arms`).
- **Tronçons de rivière** : même principe sur les arêtes. Importance 2.
- **Liens** : `flows_into`, `into_lake`, `outflow`. **Rang** (`level`) : 0 au déversoir final ; +1 par affluent ; un lac a le rang du cours qui en sort ; le principal cours qui entre dans un lac garde son rang.
- **Tracé et jonctions** (`CoursePoint` : centre, milieu d'arête ou coin) : traits reliés aux confluents.
- **Choix des noms** (`pick_name`) : `REAL_RIVERS`, `REAL_STREAMS` ; fenêtres de latitude ±8°, ±16°, ±30° ; réutilisation à 30 cases.
- **Étendues d'eau** : lac d'eau douce, lac salé, mer intérieure (au moins 3 cases, ne touche pas le bord). Tables `REAL_LAKES`, `REAL_SALT_LAKES`, `REAL_INLAND_SEAS` (50 noms).
- **Couleurs des bassins** : 12 vives pour les grands bassins, 4 grises pour les autres.

## Résultats (graine 777, 192 × 111, cases de 40 km)

Chiffres antérieurs aux mares de mer (à remesurer) : ruissellement 236 900 m³/s → mer 225 300, lacs 7 400, lits 4 200 ; 1 709 arêtes de rivière, 270 cases de fleuve, 44 lacs ; 914 bassins ; 272 cours d'eau nommés. Génération ≈ 8,4 s.

## Règles de jeu

- Traverser une rivière : **+2 PM**. Entrer dans un fleuve : **3 PM** (l'unité s'y arrête). Mêlée à travers une rivière ou depuis un fleuve : **+3 en défense**.
- **Eau douce** : +1 nourriture. Fleuve : 2 / 0 / 1 or ; pas de ville dans le lit. Lac : 2 / 0 / 1, infranchissable.
- Trajets : `find_path_turns` pour le joueur ; `find_path` pour l'IA et l'exploration.

## Exploration automatique : performances (`explore.rs`)

- `Known` en tableaux, une seule recherche, arrêt anticipé, `WorldMap::step_costs` : ≈ 150 ms par tour à 7 000 cases connues (36 éclaireurs).

## Affichage

- **Cartes** : normale, relief, climat, bassins versants, débits, réseau hydrographique.
- **Étiquettes**, **surbrillance**, **carte du réseau** : voir `conception-options-carte.md`.
- **Bord de la zone explorée** : `clip_to_explored` ; en 3D, une case inexplorée qui porte une rivière d'arête visible prend les lits de ses voisines explorées.
- **Formes et teintes des cours d'eau** (`waterways.rs`, géométrie partagée par la 2D et la 3D) :
  - **Largeur selon le débit** : rivière, demi-largeur 1 + 4 par décade de débit au-delà de 100 m³/s, au plus 7 (l'arête mesure 32) ; fleuve, d'un quart de case au seuil (2 000 m³/s) à 92 % de la case à 60 000 m³/s, en échelle logarithmique. Sur une arête entre deux cases de fleuve, la largeur est celle du plus petit des deux débits (la même vue des deux cases). Une grosse rivière arrive presque à la largeur d'un petit fleuve.
  - **Méandres des fleuves** : le point où le fleuve franchit une arête vers une autre case de fleuve glisse le long de l'arête (tirage selon la graine et l'arête, le même des deux côtés), dans la place que laisse sa largeur (marge de 1,5 avec les coins). Un fleuve qui traverse sa case (deux arêtes) suit une courbe de Hermite d'une arête à l'autre par un point du milieu un peu écarté du centre, en travers du cours (au plus 45 % de sa demi-largeur : l'eau couvre toujours le centre, où sont les unités) ; les courbes arrivent perpendiculaires aux arêtes (raccord lisse d'une case à l'autre). Ailleurs (confluent, source, embouchure, rive de lac) : un rond au milieu et un bras courbe vers chaque arête. La sinuosité baisse avec la pente (dénivelée entre les deux cases : 1 en plaine, 0,3 au-delà de 250 m). La surbrillance au survol passe par les mêmes points.
  - **Embouchure** : le fleuve s'évase de moitié (au plus toute l'arête) ; le long d'un lac qui n'est pas sur son cours, l'arête reste ouverte sur toute sa largeur (une seule nappe d'eau) ; les coins ne sont remplis qu'à côté d'une arête ouverte en entier (sinon un petit fleuve finissait en pointe de flèche).
  - **Rivières raccordées aux coins** (retour de Benjamin : « les connexions entre les rivières ne sont pas fluides ») : chaque arête est une courbe de Hermite d'un coin à l'autre, dans le sens du courant (`map::river_down_corner`, d'après les tronçons orientés de l'hydrologie). À chaque coin, sa direction est la moyenne de la sienne et de celle du tronçon voisin (aval, ou amont le plus abondant) : même tangente des deux côtés, plus d'angle de 60° ; la courbe s'écarte de l'arête d'environ 2 au plus, les bouts restent aux coins (on voit toujours de quel côté de la rivière est une unité). La largeur et la teinte partent de celles du tronçon amont principal : plus de marche de largeur d'une arête à l'autre. Un rond à chaque coin couvre les confluents. **Affluent qui rejoint un fleuve** (retour de Benjamin : le Laurom n'était pas relié à ses affluents ; la bande d'affluent n'allait qu'à 60 % du chemin vers le milieu de la case, assez quand le fleuve occupait toute la case, plus maintenant qu'il est étroit) : du coin, il continue dans sa direction et courbe jusqu'au point le plus proche de l'axe du fleuve ; en 3D, sa hauteur rejoint celle du fleuve une seule fois sur tout ce chemin (chaque petit tronçon porte sa part du chemin). (Premier essai écarté : un bombement tiré au hasard sur chaque arête accentuait les cassures.)
  - **Teinte** (par sommet, interpolée) : les grands débits sont plus sombres (jusqu'à −30 %) ; l'eau chargée tire vers un limon olive (au plus 65 %). **Turbidité** (`map::murk_of_cells`, calculée à la première demande) : chaque case a un limon propre selon son biome (désert 1, steppe 0,85, savane 0,8, plaine et maquis 0,6, jungle 0,5, prairie 0,45, forêt 0,25, taïga et toundra 0,1, neige 0), réduit jusqu'à 70 % entre 600 et 2 000 m (hautes terres rocheuses : eau claire) ; l'eau d'une case est celle de l'amont (affluents mêlés selon leur débit) tirée de 35 % vers le limon de la case : un cours d'eau prend la couleur des terres qu'il traverse en trois cases environ. Un lac garde 75 % du limon (une fois, à son exutoire). Premier essai écarté : un mélange pondéré par l'eau apportée laissait presque toute l'eau claire (les terres sèches donnent peu d'eau, les montagnes beaucoup). Graine 777 : fleuve de savane à l'embouchure 0,75, fleuve de jungle 0,51, fleuve de taïga 0,07. Console : `info` donne `limon=`.
  - Coût : un fleuve est découpé en 12 tronçons de lit par case (3 auparavant), une rivière d'arête en 6 ; FPS inchangés une fois le relief 3D construit.
- **3D** : hauteur de l'eau sur les axes, surface mélangée, lit taillé, maillage par paquets, cascades et plages. Surfaces d'eau refaites en parallèle et comptées dans le budget de temps par image (voir `conception-altitude.md`, « Performances de la construction du relief 3D »).

## À faire ensuite

- **Refonte du dénivelé** (relief géologique) : lissage le long des cours d'eau, unifier les deux écoulements.
- **Aménagements des villes** : ponts, moulins à eau.
- **Îles et bras** dans les grands fleuves de plaine, **deltas** à l'embouchure (piste E, mise de côté).
- Méandres : la courbe passe près du centre de chaque case ; des boucles plus amples qu'une case demanderaient de lisser le tracé sur plusieurs cases.
- **Saisons** : fonte des neiges, crues et étiages.
- **Retour au climat** : évaporation des lacs et mers intérieures.
- Navigation fluviale et lacustre.
- Remesurer les résultats de la graine 777 après les mares de mer.
- Vérifier à l'écran le mode « survol de l'étiquette » et les fentes noires au bord de l'exploration en partie réelle.
