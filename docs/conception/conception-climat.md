# CivBuilder — Conception du climat (v4, petite planète à cases de 40 km)

Simulation lancée à la génération du monde (module `src/climate.rs`, logique pure et testée), en grandeurs physiques : °C, g/kg, mm, m/s.

## Échelle
- Historique des choix de Benjamin : d'abord une région (« option B », 4X de développement plutôt que de conquête), puis **une planète complète mais plus petite que la Terre**, en gardant 40 km entre deux centres de cases (taille d'une ville et de ses abords).
- Carte 192 × 111 (~21 300 cases) : ~7 700 km d'ouest en est, ~3 700 km d'un pôle à l'autre, rayon ~1 200 km (≈ 1/5 de la Terre). Repli prévu : 128 × 74 si les performances ne tiennent pas. `CIVBUILDER_MAP=LxH` pour essayer une autre taille.
- `Region { cell_km, center_lat, degrees_per_row, wraps }` : `Region::planet(cell_km, h)` (pôle à pôle, bouclage est-ouest) pour le jeu ; `Region::local(cell_km, lat)` (région d'une planète de taille terrestre) pour les tests.
- Le climat fait le tour complet (voisins et bruit des perturbations raccordés est-ouest, sur un cylindre). La carte de jeu ne boucle pas encore (bords océaniques) — à reprendre avec la future génération géologique prévue par Benjamin.
- Pas de temps adapté à la taille des cases (~45 min à 40 km) ; processus en durées réelles.
- Durées simulées : 30 jours de mise en place, puis 4 saisons × (6 jours de transition + 4 régimes de 7 jours).
- Performances : calcul en parallèle (rayon). 192 × 111 : 16,7 s sur 1 cœur, 8,4 s sur 2 cœurs → ~3 s attendues sur 8 cœurs.

## Ordre de la génération et passes climat ↔ eau (24/10/2026)
- Ordre : relief (niveau de la mer, **mares de mer** changées en cuvettes, niveaux, montagnes, collines) → climat → terrain selon le climat → côtes → cours d'eau et lacs (`hydro.rs`) → bassins et noms.
- Demande de Benjamin : refaire le climat après les lacs, **3 passes** (`map::CLIMATE_PASSES`). À partir de la deuxième, les cases de lac de la passe précédente sont données au climat comme `Surface::Lake(altitude du niveau du lac)` ; le terrain est remis à son état d'avant les lacs, puis recalculé, ainsi que les cours d'eau.
- `Surface::Lake` : **eau libre** pour l'évaporation et l'humidité (pas de seau de sol, évaporation « bulk », pas de ruissellement, faible écart jour-nuit), mais **compte comme de la terre** pour les courants marins, la continentalité, l'effet thermique et le relief vu par l'air.
- Graine 777 : passe 1 → 2 : forêt 3 136 → 3 217 cases, désert 1 557 → 1 473, cases de fleuve 264 → 257 ; passe 2 → 3 : quasi rien. La boucle converge dès la deuxième passe.
- Coût : ≈ 10 s par passe dans le conteneur de développement (version optimisée) : génération ≈ 3 × plus longue. Deux passes suffiraient si la durée gêne.

## Cycle de l'eau tropical (01/11/2026, demande de Benjamin : monde trop sec)
- Constat de Benjamin : beaucoup de désert, peu de jungle. Bilan `bilan_aridite` (map.rs, graines 777 / 2 / 4242) : désert 13 / 20 / 27 % des terres, jungle 2,9 / 3,7 / 1,4 % (Terre : ~14 % de déserts chauds, ~7 % de forêt tropicale humide). La pluie moyenne des terres n'est pourtant pas faible (1 250-1 300 mm, Terre ~800) : elle tombe au mauvais endroit. Désert surtout à 10-20° (38 à 65 % des terres de la bande), et même 100 à 380 cases de désert entre 0 et 10° ; forêt **tempérée** à 25-35° (ceinture désertique terrestre) ; latitudes moyennes détrempées (P/ETP 5 à 70 entre 40 et 60°).
- Cause : **l'air marin tropical est sec** (humidité relative de la couche basse 23-48 %, vapeur de la colonne 15-24 mm ; Terre : ~75-80 % et 40-60 mm), d'où une évaporation de la mer emballée (3 500 à 5 200 mm/an, Terre ~1 500-2 000). Bilan de la couche basse sur la mer à 0-10° (graine 4242) : évaporation 5 190 mm/an, vapeur soulevée 8 520 mm/an, dont **19 % seulement condensée** : l'air soulevé ne montait qu'au sommet de la couche haute (6 km), où l'air saturé garde ~3 g/kg, et l'air bas sec (~5 g/kg) y condensait peu ; 30 à 40 % du condensat seulement pleuvait (rendement selon l'humidité de l'air bas). Le reste partait en vapeur dans la couche haute, que les vents d'altitude emportaient vers 20-35° et les latitudes moyennes. Cercle vicieux : air bas sec → peu de condensation → vapeur exportée → air bas sec.
- Correction : **sommet des nuages selon la chaleur** (`DEEP_CLOUD_EXTRA_M` = 8 000 m) : 6 km au froid, jusqu'à 14 km pour une latitude à 27 °C (orages tropicaux, tropopause plus haute) ; et **rendement des précipitations** de (0,3 ; 0,7) à (0,6 ; 0,7).
- Résultat (avant → après), graines 777 / 2 / 4242 : désert 13,4 → 11,8 % / 20,4 → 17,5 % / 27,3 → 21,5 % ; jungle 2,9 → 4,9 % / 3,7 → 5,8 % / 1,4 → 3,3 %. Désert entre 0 et 10° : 223 → 46 (777), 379 → 46 (4242) ; la jungle couvre environ la moitié des basses terres équatoriales (sous 800 m). Pluie de la mer à 0-10° : 2 090 → 3 540 mm (4242). Les bandes 20-40° et les latitudes moyennes s'assèchent (terres à 20-30°, graine 777 : 1 650 → 1 070 mm). Graine 777 : ruissellement −10 %, cases de fleuve 225 → 174, plus grand fleuve 33 500 → 27 900 m³/s ; forêt tempérée 2 241 → 1 826 cases (au profit de la prairie, de la plaine et de la steppe).
- Essais sans effet notable sur les déserts (pour mémoire) : seuil et force de la convection, force du couvercle d'inversion, subsidence plus faible, brassage vertical, systèmes mobiles affaiblis sous les tropiques, déplacement saisonnier des ceintures (plus faible : plus de désert ; plus fort : −1 à −5 points de désert mais moins de jungle). Supprimer le retour vers les pôles des vents d'altitude double la pluie équatoriale mais rend 20-30° désertique (désert 27 → 42 % sur 4242).
- **Vent près de l'équateur** (01/11/2026, suite) : la convergence de la couche basse valait ±70 à ±100 mm/s en moyenne sur une bande de 10° près de l'équateur (circulation de fond : ±2,5 mm/s ; Terre : quelques mm/s). La déviation des vents s'annule à l'équateur géographique : l'équateur thermique décalé de 10° avec les saisons, l'air y descendait la pente de pression sans déviation, à pleine vitesse (la part qui descend la pente passe de 10 % du vent vers 11° à 100 % à l'équateur), et il était aspiré puis soulevé quatre à cinq fois trop vite. Dans les vrais tropiques, les écarts de pression restent faibles faute de force de Coriolis pour les tenir. Correction : part du vent qui descend la pente plafonnée à 0,3 × la pente (`EQUATORIAL_CROSS_FLOW_MAX`) ; la part qui longe les isobares ne change pas. Convergence moyenne sur la bande 0-10° : ±70-100 → ±30-40 mm/s (un vrai flux de quelques m/s à travers l'équateur, qui reste) ; plafond 0,15 : guère moins. Graine 4242, mer à 0-10° : humidité relative 24 → 34 %, évaporation 5 200 → 3 750 mm, pluie 3 540 → 2 100 mm (océans de la zone de convergence terrestre : 2 000-3 000). Saisons sèches à 0-10° : ~2,0 → ~1,5. Résultat (après le sommet des nuages → après le vent), graines 777 / 2 / 4242 : désert 11,8 → 11,2 % / 17,5 → 17,4 % / 21,5 → 19,7 % ; jungle 4,9 → 5,6 % / 5,8 → 6,5 % / 3,3 → 4,6 %. Graine 777 : cases de fleuve 174 → 177, ruissellement −7 %.
- Bilan des deux corrections (départ → fin), graines 777 / 2 / 4242 : désert 13,4 → 11,2 % / 20,4 → 17,4 % / 27,3 → 19,7 % ; jungle 2,9 → 5,6 % / 3,7 → 6,5 % / 1,4 → 4,6 %.
- **Mousson continentale** (01/11/2026, suite) : après le relief, la bande 10-20° restait sèche (désert 270 / 440 / 755 cases, graines 777 / 2 / 4242 ; pluie médiane des terres 690 / 450 / 360 mm pour ~1 500 mm d'ETP ; saison des pluies d'été seule, au rythme de 900 à 1 600 mm/an). Bilan de la couche basse sur la mer à 10-20° (graine 4242) : évaporation 4 035 mm/an, emportée surtout par le transport horizontal (−2 930) et le soulèvement (−1 360) ; la subsidence n'apporte que 280 mm d'air d'altitude. L'air marin est emporté par les alizés vers l'équateur et les côtes avant de se charger (humidité ~40 %). Essais sans effet sur les déserts : dépression thermique des continents deux fois plus forte (pluie concentrée là où il pleut déjà), subsidence trois fois plus faible (humidité de la mer inchangée), couche basse plus lente (×1 ou ×0,7 au lieu de ×1,7 : mer plus humide, 38 → 50-56 %, mais moins de vapeur livrée aux terres, désert en hausse). Ce qui marche : sur Terre, la dépression thermique d'été remonte bien plus loin sur les continents (Sahel vers 15°, Inde vers 20-25°, Australie du Nord vers 15°S) que sur la mer. **La pression y suit le soleil de 0,3 × sa latitude de plus** (`MONSOON_EXTRA_SHIFT`), au cœur d'un continent (part de terre à 800 km), entre 15 et 25° de latitude, en rampe de 10° de part et d'autre (`MONSOON_LATITUDES`). Partout (équateur compris) : jungle 6,3 → 3,9 / 8,0 → 6,7 / 8,6 → 4,4 % (l'équateur s'assèche en hiver, la ceinture de pluies partant loin dans l'autre hémisphère) ; jusqu'aux latitudes moyennes, les vents d'ouest des continents se déplaçaient (tests du continent d'essai à 35° et 48° en échec). Décaler aussi la circulation de fond (montée, subsidence) ne change presque rien : c'est la pression, donc le vent et sa convergence, qui compte. Résultat (avant → après) : désert 12,8 → 12,1 / 20,1 → 18,0 / 23,1 → 19,9 % ; désert à 10-20° 270 → 216 / 440 → 326 / 755 → 531 cases (au profit de la savane) ; jungle 6,3 → 6,2 / 8,0 → 7,7 / 8,6 → 9,2 %. Un peu plus de désert entre 0 et 10° (53 → 120 / 20 → 53 / 40 → 127 cases), à surveiller. Graine 777 : 160 cases de fleuve (165 avant).
- **Air marin sec et côtes est** (02/11/2026, suite, question de Benjamin : le séjour trop court de l'eau dans l'air fait-il les déserts ?). Bilan de la couche basse sur la mer (graine 4242, mm/an) : à 10-20°, évaporation +3 730, transport −2 410 (dont divergence des vents −165 seulement : l'air se charge le long de sa route et emporte sa vapeur), soulèvement −1 510, air chassé par la subsidence −830, air d'altitude descendu +246 ; eau de la couche 12,8 mm pour 40,8 mm à saturation ; **séjour de l'eau 1,1 à 1,9 jour** sur la mer (Terre : plus d'une semaine), 7 à 15 jours sur les terres de 10-30°. À l'équateur, le soulèvement retire 5 300 mm/an pour 2 070 mm de pluie : le reste part en altitude vers les latitudes moyennes. Essais sur la mer, tous sans gain ou en perte (désert 777 / 2 / 4242, départ 12,1 / 18,0 / 19,9 %) : air qui diverge remplacé en entier par l'air d'altitude, sans chasser l'air resté (mer +2 points d'humidité, désert 14,8 / 20,3 / 22,5 %) ; couche basse de 1 km (mer 51-60 %, évaporation 2 700-2 950 mm, mais 9 mm d'eau au lieu de 12,8 : pluie des terres −20 %, désert 12,3 / 15,1 / 23,1 %, jungle en baisse) ; moitié de l'eau des orages non tombée rendue à la couche basse (désert 14,0 / 20,1 / 20,7 %) ; convection dès 45 % d'humidité de la couche (inchangé). **La mer s'auto-règle** : un air marin plus humide fait moins évaporer la mer, et les terres ne reçoivent pas plus d'eau. Les déserts ne naissent pas loin de la mer : ~30 % des côtes tropicales sont désertiques. **Côtes est contre côtes ouest** (`bilan_aridite`) : les côtes ouest de 15-30° sont désertiques comme sur Terre (Namib, Atacama) ; les côtes est sont trop sèches (20-30° : 580-890 mm, P/ETP 0,45-0,80 ; 30-40° : 270-460 mm, P/ETP 0,26-0,47 ; Terre : Floride, Shanghai, Sydney 1 100-2 000 mm, humides). Par saison (simulations d'une seule période) : l'été, le vent vient de la mer et la mousson est forte à 20-30° ; l'hiver, le vent d'ouest sort du continent et il ne pleut presque pas (265-360 mm/an au rythme de l'hiver, Terre 600-1 000) ; à 30-40°, l'air marin d'été n'a que 51 % d'humidité. Essais sans gain : couvercle d'inversion selon la température de la mer (le couvercle est saturé ; même supprimé partout, côtes est +0 à 15 %, et +5 à 20 % avec une convection plus précoce) ; pression selon la température de la mer (désert en hausse) ; contraste thermique terre-mer (pression des continents selon l'écart saisonnier : dépression l'été, anticyclone l'hiver ; plus il est fort, plus la pluie d'été part vers l'intérieur, côtes est de 20-30° 5 570 → 2 850 mm/an au rythme de l'été).
- **Dépressions d'hiver des mers chaudes** (02/11/2026, choix de Benjamin) : l'hiver, l'air froid des continents rencontre la mer chaude des courants de bord ouest (Gulf Stream, Kuroshio) ; les dépressions y naissent et, en passant, brassent l'air marin avec celui de la côte. Intensité : mer plus chaude que sa latitude (3 °C pour le plein) avec une terre à l'ouest (d'où sort l'air froid ; ni les côtes ouest ni les mers douces devant l'Europe), étendue à 500 km côtes comprises, entre 30 et 45° (rampe de 6°), l'hiver seulement (`CYCLOGENESIS_*`). Elle ajoute les fronts (`STORM_W`) et le brassage du rail des dépressions (1,3 × 10⁶ m²/s). Une dépression fixe (pression abaissée de 7 hPa) ne marche pas : sur la côte, le vent part vers elle, donc vers la mer (désert 12,2 / 17,3 / 19,4 %) ; un critère sans « terre à l'ouest » prenait les côtes ouest et cassait les tests du continent d'essai à 48° ; un brassage triplé dessèche la côte (l'air sec du continent sort aussi). Résultat : désert 12,1 → 11,4 / 18,0 → 17,2 / 19,9 → 19,0 % ; jungle inchangée ; pluie au rythme de l'hiver des côtes est de 30-40° N 369 → 469 (777), 385 → 475 mm/an (4242). Ce n'est qu'un pis-aller du transport par les tourbillons : des dépressions qui se déplacent (régimes plus courts, systèmes décalés vers l'est) le remplaceraient.
- **Bilan d'eau de la planète** (02/11/2026, suite, question de Benjamin : l'eau séjourne-t-elle trop peu dans l'air ?). Moyennes par case (les cases du climat ont toutes la même surface ; une pondération par la latitude faisait croire à une fuite de 7 à 10 %). Graine 777, mm/an : évaporation 951 (Terre ~1 000), pluie 938 et rosée 7 ; le reste (~7 mm) s'accumule dans les sols, et le ruissellement sort du cycle. Transport, air chassé par la subsidence et changements de saison conservent l'eau à moins de 1 mm près : **pas de fuite**. Eau de l'air ~10 mm par case (13 mm en moyenne pondérée ; Terre ~25 mm), séjour ~4 jours (Terre 8-9). Mer : évaporation 1 290 / 1 205 / 1 086 mm (graines 777 / 2 / 4242), terres : pluie 1 150 / 1 156 / 1 245 mm (Terre ~800), évaporation 436 / 433 / 486 mm. Sorties de la couche basse : air soulevé jusqu'au sommet des nuages 1 321 mm/an (dont 829 condensés, 492 restés en vapeur dans la couche haute), qui vient à **87 % de la convergence des vents** (1 153), 165 des fronts, **2 de la convection** (seuil de 60 % d'humidité de la couche, rapportée à la saturation au sol, jamais atteint), 1 des lacs ; couche haute vers les nuages : saturation 389, soulèvement 189. **Il pleut là où les vents convergent, quelle que soit l'humidité de l'air** ; sur Terre, la pluie tropicale vient de la convection, qui ne part que dans un air humide (Bretherton et al., 2004 : pluie presque nulle sous ~60 % d'humidité de la colonne). Essais (désert 777 / 2 / 4242, départ 11,4 / 17,2 / 19,0 % ; jungle 6,2 / 7,7 / 9,2 %), tous abandonnés : pas de pluie profonde dans un air sec, seuils de 35 à 65 % (air marin 48-54 %, eau de l'air 13,1 → 13,8 mm, désert 22,1 / 26,1 / 31,8 %) ou de 15 à 40 % (sans effet) ; troisième couche (couche de mélange de 700 m sous une couche nuageuse jusqu'à 2 km, renouvelée 1 fois par jour : air marin inchangé, désert 13,1 / 18,8 / 21,1 % ; 0,4 fois par jour : air marin +4 points, désert 14,3 / 20,0 / 22,6 % ; avec la règle de pluie : désert 19,2 / 23,1 / 29,2 %, jungle 2,9-5,3 %) ; soulèvement par convergence divisé par deux (eau de l'air 14,0 mm, désert 14,2 / 21,0 / 23,1 %) ; couche basse saturée à la température de son milieu (inoffensif : désert 12,6 / 17,9 / 19,6 %, pluie des terres +5 %) ; convection de Betts-Miller (relaxation de la colonne vers 70 % d'humidité en 6 h, version simplifiée de Frierson 2007) : seule, sans effet (12,3 / 17,3 / 18,8 %) ; avec une convergence qui ne monte que de 1 km et ne pleut que si elle sature, désert 23,6 / 26,5 / 30,6 %, jungle 3,1-5,8 % ; avec en plus la convection qui monte l'humidité vers la couche haute, désert 17,8 / 21,9 / 30,1 %, jungle 2,2-4,2 %. **Conclusion** : quelle que soit la façon dont il pleut, l'air de la couche basse reste à ~45 % au-dessus de la mer et l'eau de l'air à 10-11 mm ; dès que la convergence ne fait plus pleuvoir d'elle-même, les terres plates perdent leur pluie (il ne reste que le relief et les fronts). Ce n'est pas la pluie qui fixe l'humidité de l'air marin, mais la **circulation de la couche basse** : l'air arrive sec des anticyclones (renouvelé par la subsidence) et n'a que deux jours environ pour se charger avant d'atteindre la ceinture de pluies ou les terres (vent de la couche basse 1,7 × celui du sol, transport −2 400 mm/an et air chassé −830 mm/an sur la mer à 10-20°, graine 4242). Pistes : temps de trajet de l'air marin, vitesse de la couche basse, renouvellement par la subsidence, diffusion numérique du transport sur des cases de 40 km ; puis une pluie déclenchée par l'humidité (convection) quand l'air marin sera assez humide.
- Limites restantes : air marin encore sec (humidité relative ~35 % à l'équateur, ~40 % sous les alizés, 36-52 % au large des côtes est) : c'est la limite commune des déserts côtiers et des côtes est ; un simple réglage ne suffit pas (voir ci-dessus), il faudra revoir la couche basse (couche humide mince sous l'inversion, moins d'évaporation, plus d'eau gardée). Côtes est de 20-40° encore trop sèches (P/ETP 0,3-0,8). La couche basse de 2 km, brassée et emportée vite, ne se charge pas comme la couche humide terrestre (~1 km sous l'inversion des alizés). Bande 10-20° encore assez sèche sur les très grands continents (graine 4242 : 33 % de désert). Relief deux fois trop haut (16 % des terres sous 300 m, Terre ~33 %) : corrigé le 01/11 (altitude en mètres d'abord, voir `conception-altitude.md`) ; jungle 469 → 525 / 507 → 624 / 346 → 653 cases (graines 777 / 2 / 4242), désert 943 → 1 076 / 1 364 → 1 577 / 1 483 → 1 746 (basses terres plus chaudes).

## Effet de lac (24/10/2026, choix de Benjamin : version simplifiée)
- Constat : avec la seule évaporation du lac, la pluie sous le vent ne gagnait que 1 à 2 % (lac Huron, graine 777) ; sous les alizés (air qui descend), la vapeur ajoutée ne pleut pas.
- Ajout : **vitesse verticale supplémentaire** `LAKE_EFFECT_W` = 0,003 m/s (convection pleine : 0,004) sur le lac (moitié) et sur la **rive vers laquelle souffle le vent** du régime courant (produit scalaire direction lac → case · vent), décroissant jusqu'à `LAKE_EFFECT_KM` = 130 km (≈ 3 cases). Multipliée par le couvercle d'inversion : étouffée sous les anticyclones, comme la convection.
- Représente la brise de lac et l'air réchauffé par l'eau (effet de lac), faute de saisons et d'inertie thermique de l'eau.
- Graine 777, pluie de la 1ʳᵉ à la 5ᵉ case sous le vent (sans lac → avec lac et effet) : lac Nicaragua (14°, alizés) +28 % → +13 % (294 → 374 mm sur la rive) ; lac Titicaca +38 % → +7 % ; lac Malawi +18 % → +8 % ; lac Huron (37°) +5 % → +2 % ; lacs équatoriaux (Victoria, Tanganyika) +6 à +14 %.
- Observation : l'évaporation « bulk » des lacs tropicaux continentaux est forte (3 000 à 4 800 mm/an, contre 1 500 à 2 500 mm pour des lacs réels comme le Victoria ; ~2 700 pour le lac Nasser réel) : la température de l'eau est prise égale à celle de l'air continental. À recaler (température de l'eau plus fraîche) si les lacs paraissent trop évaporer.

## Circulation
- Pression : ceintures selon la latitude, effet thermique des continents, dépressions/anticyclones mobiles (~2 500 km).
- Vent de surface très dévié (72-84° ; 87° au-delà de 50°, rampe depuis 40° : flux de la cellule de Ferrel, voir « Correctifs du 08/11 ») ; près de l'équateur, où la déviation s'annule, la part qui descend la pente de pression est plafonnée à 0,3 × la pente (`EQUATORIAL_CROSS_FLOW_MAX`, comme une déviation de ~72°, voir « Cycle de l'eau tropical ») ; couche basse 1,7 × plus rapide ; vents d'altitude surtout d'ouest.
- Circulation de fond imposée par la latitude (descente vers 30°, montée à l'équateur et vers 60°), renforcée l'été à l'ouest des continents (façades des anticyclones, voir « Correctifs du 08/11 »).
- Couche basse ralentie de 10 % au-dessus des terres, pas des lacs (frottement) : convergence sur les côtes au vent, divergence sur les côtes sous le vent.
- Cohérence de masse : l'air de la couche basse qui converge monte ; celui qui diverge est remplacé par de l'air qui descend.
- Fronts des latitudes moyennes (35-65°) et convection tropicale ; couvercle d'inversion sous les anticyclones. L'hiver, au-dessus des mers chaudes bordées d'un continent à l'ouest (30-45°), dépressions d'hiver : fronts et brassage renforcés (voir « Cycle de l'eau tropical »).
- Déplacement saisonnier des ceintures : 0,45 × latitude du soleil ; sur les continents, entre 5 et 35° de latitude, jusqu'à 0,3 × de plus (mousson, voir « Cycle de l'eau tropical »).

## Océan
- Température de surface = température selon la latitude + écart dû aux **courants marins** (module `currents.rs`, voir ci-dessous) ; les côtes reprennent 70 % de l'écart de la mer voisine. (Avant le 30/10/2026 : règles fixes, côtes ouest 15-35° −4 °C, côtes est +3 °C, côtes ouest 45-65° +5 °C.)
- Évaporation « bulk » (~2 300 mm/an sous les tropiques, ~800 vers 50°). Les lacs évaporent de la même façon (à partir de la deuxième passe).
- Pas d'affichage au survol des cases d'eau (décision de Benjamin).

## Courants marins (30/10/2026, choix de Benjamin : version simplifiée, carte des courants)
- **Modèle de Stommel** (1948) sur les cases de mer : `r ∇²ψ + β ∂ψ/∂x = rot τ / (ρ H)`, ψ fonction de courant de la couche mélangée (H = 1 000 m), τ tension du vent (ρ_air 1,2, Cd 1,3·10⁻³, vent normalisé × 8 m/s), β = 2Ω cos φ / R (Ω terrestre, R ≈ 1 200 km tiré de la carte : β ≈ 5 fois celui de la Terre). Frottement r choisi pour que la couche de bord ouest (r/β à 30°) fasse 1,5 case : tous les coefficients de la discrétisation restent positifs, surrelaxation (ω = 1,7 ; ω = 1,85 diverge, l'équation n'étant pas symétrique ; depuis les blocs de croûte continentale, certaines formes d'océans font diverger ω = 1,7 : le calcul reprend alors à ω = 1,4 puis 1 — Gauss-Seidel, qui converge toujours, les coefficients étant positifs).
- Discrétisation hexagonale : gradient (1/3d) Σ (f_j − f_i) e_j, laplacien (2/3d²) Σ (f_j − f_i).
- **Bords** : pôles (première et dernière lignes) et terres d'au moins `BLOCKING_MIN_CELLS` = 100 cases (lacs compris) ; les petites îles laissent passer l'eau (sinon l'eau serait immobile entre deux îles d'un archipel). Chaque ensemble de bords d'un seul tenant est une « île » : ψ = 0 sur les continents (pas de passage d'eau entre deux continents), sauf la région polaire.
- **Courant circumpolaire** : si la région polaire (bord du pôle et terres rattachées) est séparée des continents par une mer qui fait le tour de la planète de son côté, sa valeur de ψ est fixée par l'équilibre entre le vent moyen et le frottement r le long de l'anneau (cases où la solution « région polaire = 1 » vaut 0,2 à 0,8). Les passages de la tâche « plaques polaires » sont au-delà de ~60°, sous les vents d'est polaires : le courant y fait le tour vers l'ouest (sur Terre, le courant circumpolaire antarctique est plus près de l'équateur, sous les vents d'ouest).
- Vitesse de surface = vitesse moyenne de la couche × 50 (le courant est concentré près de la surface), plus une **dérive équatoriale** sous le vent (0,3 m/s pour un vent d'intensité 1, nulle à 12°).
- **Dérive chaude des hautes latitudes** (30/10/2026, demande de Benjamin : c'était l'un des buts des courants). Avec les seuls tourbillons, les côtes ouest des continents entre 45 et 65° ne gagnaient que +0,4 à +0,9 °C (l'ancienne règle donnait +5 °C) : le tourbillon subtropical remonte jusque vers 55°, l'eau qui longe ces côtes arrive du nord. Une mémoire thermique plus longue aux hautes latitudes (300 jours) aggravait le résultat. Ajout d'une **circulation de retournement simplifiée** : dérive de surface vers le pôle de 0,12 m/s, pleine entre 42 et 58°, nulle à 34° et 66° (l'eau plonge aux hautes latitudes ; écoulement non conservatif, sans effet sur ψ). Elle domine là où les tourbillons sont lents (côté est des océans) ; les courants froids rapides des côtes est des continents (Labrador) restent plus froids. Sans elle : +0,5 à +1,1 °C ; avec elle : +5,7 à +8,4 °C.
- **Température de la mer** : cible = température de la latitude − 6 °C × remontée d'eaux (transport d'Ekman vers le large, 0 à 1, à ≤ 2 cases d'une côte, 3-45° de latitude) ; transport amont en régime établi `u·∇T = (cible − T)/τ`, τ = 60 jours. Écart = température transportée − température de la latitude. La température de la latitude est une moyenne terrestre (air au niveau de la mer, continents compris) : aux hautes latitudes, la mer est plus chaude qu'elle, comme sur Terre (55° N : moyenne ≈ 2 °C, Atlantique nord-est 8-12 °C). Une latitude où la mer serait en moyenne plus froide (les tropiques, qui exportent leur chaleur) est ramenée à 0 : sinon les tropiques séchaient (graine 2 : désert 1 464 → 1 742 cases, jungle 212 → 161). Enfin, la **remontée d'eaux refroidit aussi sur place** (− 6 °C × intensité) : transportée seule, l'eau froide était aussitôt emportée (zones de remontée à 0 / +1 °C en moyenne ; un renouvellement de l'eau en 10 jours n'y changeait rien).
- (Première version, abandonnée : écart ramené à une moyenne nulle sur chaque latitude ; il annulait la dérive chaude.)
- **Passes** : à la première, vents théoriques (table par latitude calée sur les vents simulés sur la mer : alizés −0,95 vers 15°, vents d'ouest +0,65 vers 45°, vents d'est polaires −0,65 vers 75°) ; ensuite, vent simulé par le climat de la passe précédente. Vents d'ouest simulés sur la mer : 30-40° +0,3, 40-50° +0,65, 50-60° +0,3 (maximum vers 45°, comme sur Terre).
- **Pêche** : rencontre de courants (`marine::FRONT`) quand, à ≤ 2 cases, l'eau est à la fois ≥ 1,5 °C au-dessus et ≥ 1,5 °C au-dessous de la température de sa latitude, sur le plateau ou à ≤ 3 cases d'une côte.
- **Carte des courants marins** : mer teintée selon sa température (de −2 °C bleu nuit à 28 °C rose terne : une mer chaude couleur sable se confondait avec la terre), terres et lacs gris, flèches en quinconce (comme les vents) rouges sur un courant chaud, bleu clair sur un courant froid (écart ≥ 1,5 °C), blanches sinon.
- Bilan (`bilan_courants`, graines 777 / 2 / 5) : vitesse médiane 0,19-0,20 m/s, 99 % sous 0,55-0,57, maximum 1,3-1,6 m/s ; écart de température 1 % à −5 / −6 °C, 99 % à +14 / +15 °C (côtes norvégiennes) ; entre 15 et 35° : côtes ouest des continents −1,1 à −1,5 °C, côtes est +0,3 à +2,2 °C ; entre 45 et 65° : côtes ouest des continents +5,9 à +8,4 °C (Europe), côtes est +1,9 à +4,2 °C (Labrador) ; zones de remontée d'eaux −1,6 à −3,4 °C ; courant circumpolaire aux deux pôles sur les trois graines ; 170 à 310 cases de rencontre de courants.
- Effet sur les biomes (ancienne règle → courants, graines 777 / 2) : désert 1 364 → 1 315 / 1 464 → 1 582 (déserts côtiers), jungle 260 → 269 / 212 → 190, forêt tempérée 1 485 → 1 540 / 1 217 → 1 182, taïga 1 245 → 1 196 / 1 263 → 1 211, toundra 1 242 → 1 305 / 1 275 → 1 327.
- Les tests du climat sur le continent d'essai (région de 12° de latitude, mer étroite) reçoivent une mer « terrestre » explicite (`earth_sea` : côte ouest −3 °C vers 15-40°, +4 °C au-delà de 45° ; côte est +2 / −1 °C) : sans elle, la côte ouest à 48° n'aurait que ~280 mm de pluie.
- Tests (planète d'essai de 192 × 111 cases, un océan entre deux continents) : tourbillon subtropical avec courant de bord ouest, eau plus chaude à l'ouest qu'à l'est de l'océan vers 20-35°, dérive chaude vers 50-60° (côte est de l'océan plus chaude que la côte ouest), courant circumpolaire par un passage dans le sens du vent moyen (et pas sans passage).
- Limites : courants fixes toute l'année (pas de mousson océanique, pas d'El Niño) ; circulation de retournement réduite à une dérive vers le pôle (pas de plongée ni de courants profonds) ; pas de remontée équatoriale (langue froide à l'est des océans équatoriaux) ; pas de passage d'eau entre deux continents (détroits entre grandes terres fermés au courant) ; les mers fermées suivent le même modèle.
- Suite : navigation (bonus ou malus selon le courant), dérive de la banquise, saisons.

## Deux couches d'air
- Basse : 0-2 km au-dessus d'un relief adouci (lissé sur ~40 km). Haute : atmosphère libre, 2-6 km, soulevée seulement là où le relief dépasse sa base (2 000 m), d'autant ; au-dessus de 2 000 m, elle recouvre donc toute la couche basse (les deux couches se chevauchent). Avant le 08/11, elle suivait le sol dès 1 000 m et condensait sur les massifs sans y être soulevée.
- Clausius-Clapeyron (Bolton). Air soulevé en profondeur → condense, rendement 30-70 % selon l'humidité.
- Pluie de relief sur les pentes au vent ; ailleurs, brouillard (bruine).
- Neige : chute plus lente → débordement au-delà des crêtes.
- Au changement de saison, on garde l'humidité relative.

## Sols, évaporation, Tmin/Tmax
- Seau de 250 mm, β = min(1, W / 75 %) ; mise en place accélérée des sols.
- ETP de Hargreaves bornée par la formule « bulk » ; rosée à Tmin ; ruissellement du trop-plein (alimente les cours d'eau).

## Terrain
- Neige < −15 °C, toundra < −5 °C ; indice d'aridité P/ETP : désert < 0,2 (T ≥ 0 °C), plaine < 0,5, prairie < 0,75, forêt ≥ 0,75.

## Vérifications (tests)
- Continent de test de 1 600 km (mode région) : vents d'ouest à 47°, alizés à 15° ; profils de pluie ; ombre pluviométrique.
- Planète 96 × 56 : raccord est-ouest, vents d'ouest à ±45°, alizés à ±15°.
- Clausius-Clapeyron, ETP réalistes, courants marins.
- Suite de tests complète : ≈ 6 min avec les 3 passes (≈ 2 min avant).

## À reprendre avec les saisons en jeu
- **Inertie thermique de l'océan et des grands lacs** (l'effet de lac réel vient surtout d'une eau plus chaude que l'air en saison froide) : remplacerait le réglage simplifié.
- **Température de la mer au fil de l'année**.
- Cyclones tropicaux éventuels sous forme d'événements de jeu.

## Plafond des pluies (07/11/2026)
**Décision de Benjamin** : plafonner les pluies extrêmes (jusqu'à 35 666 mm/an), reporter l'humidité non tombée aux cases suivantes, ajouter du bruit pour que le plafond ne se voie pas. **Algorithme** : à chaque pas, la pluie d'un nuage est limitée à un rythme par case (`RAIN_CAP_MM_YEAR` 12 000 mm/an × `RAIN_CAP_PEAK` 1,5, ±25 % de bruit sur 320 km) ; le reste reste dans le nuage, que le vent d'altitude transporte. **Bilan** (`bilan_plafond_pluie`, 8 graines) : pluie maximale de 10 179 à 15 188 mm (auparavant jusqu'à 35 666), et 8 à 21 % de cases de désert en moins sur les 5 graines comparées (777 : 960 → 838 ; 4242 : 1 439 → 1 218 ; 31337 : 738 → 586 ; 8 : 1 476 → 1 362 ; 2 : 1 346 → 1 236). Une marge de 2,0 laissait 16 500 à 18 300 mm. **Limite** : le plafond porte sur le rythme, pas sur le total annuel : il dépasse parfois 12 000 mm, et il touche aussi de fortes pluies ordinaires (−8 % sur le continent équatorial du test, 2 139 → 1 968 mm).

## Comparatif avec la Terre (08/11/2026)
Question de Benjamin : le monde est-il trop humide ? Bilan `bilan_pluie_comparatif` (8 graines, médianes par bande de latitude
et par situation : côte ouest ou est à 2 cases au plus de la mer, intérieur à 8 cases ou plus, hautes terres à 1 500 m ou plus).
Pas de tableau fiable de la Terre par bande : comparaison avec des stations (Manaus 2 307 mm, Kisangani 1 804, Lima 13,
Oulan-Bator 270-278, Miami 1 487, Seattle 999, Vancouver 1 199, Bergen ≈ 2 250) et des ordres de grandeur.
- **Juste** : tropiques humides (0-10° : médiane 2 174 mm), côtes ouest à 30-40° (393 mm), Arctique (169 mm).
- **Trop arrosé** : intérieurs à 30-70° (40-50° : 719 mm contre 250-400 dans les steppes ; 60-70° : 949 contre 300-500),
  intérieur des tropiques secs (20-30° : 557 mm contre moins de 100), côtes ouest désertiques à 10-30° (456-670 mm contre
  moins de 50), hautes terres (2 à 4 fois les plaines).
- **Trop sec** : côtes est subtropicales (20-40° : 343-788 mm contre 1 000-1 500).
- Moyenne des terres ≈ 1 220 mm (Terre ≈ 850, GPCC). Une part de l'écart peut venir de la planète elle-même : rayon ≈ 1/5
  de la Terre, petits continents, peu de cases loin de la mer.
- Correctifs possibles : voir `docs/TODO.md`, point 8.

## Correctifs du 08/11/2026 (TODO 8a, 8b, 8c)
Trois agents ont cherché chacun un angle (continentalité, relief et tropiques secs, façades côtières), avec le même bilan ;
deux analystes indépendants (sens physique et code ; mesures et combinaisons) ont jugé leurs propositions. Retenu : les trois,
deux d'entre elles corrigées.
- **Couche haute au-dessus des massifs** (8b, `UPPER_BASE`). Le comptage de l'eau condensée par origine (`Climate::sources`,
  gardé pour les tests) montrait que sur les hautes terres de 30-40° (graine 777), 1 485 mm/an sur 1 618 venaient de la couche
  haute saturée, contre 103 mm d'air bas poussé contre la pente : l'air d'altitude venu d'une plaine, posé 1 à 3 km plus haut et
  6 à 20 °C plus froid, condensait sans avoir été soulevé, versants sous le vent compris. Elle n'est plus soulevée que par un
  relief plus haut que sa base. Le test `relief_et_ombre_pluviometrique` admet le maximum sur la crête (la couche haute y condense).
  Variantes écartées : couche haute toujours horizontale (ombre sous le vent trop faible), lissée sur 400 km (sans gain).
- **Façades des anticyclones** (8c, `FACADE_*`, Rodwell et Hoskins 2001). Sur les côtes ouest de 20-30°, la pluie tombait
  surtout l'été : la ceinture de subsidence, la même sur tout le parallèle, montait avec le soleil vers 40°. L'été, de 15 à 40°,
  la descente de fond reçoit 2 × 3 mm/s × max(0, E − O) en plus, E et O parts de terre à l'est et à l'ouest sur 1 000 km le long
  du parallèle (5 lignes) : ~5 mm/s sur une côte ouest, comme au large de la Californie ou du Pérou. Bornée (E − O ≤ 1).
  Corrigé par l'analyse : pas de montée en plus à l'est (elle est déjà celle de la dépression thermique des continents ; l'ajouter
  comptait deux fois la mousson et mouillait les continents subtropicaux).
- **Frottement des terres** (8c, `LAND_WIND` = 0,9) : la couche basse va 0,9 fois moins vite sur le sol (case et voisines ; les
  lacs, lisses, n'en ont pas). 0,8 asséchait les côtes ouest équatoriales.
- **Flux de Ferrel** (8a, `MIDLATITUDE_DEFLECTION` = 87°, `MIDLATITUDE_RAMP` 40-50°). À 60-70°, presque toute la terre est
  côtière et recevait autant que la mer (≈ 950 mm, surtout l'été) ; la pluie y est limitée par la vapeur apportée des mers de
  30-45° par la couche basse (0,85 m/s vers le pôle, Terre 0,4-0,5), pas par le soulèvement (fronts ou montée de fond réduits :
  sans effet). 87° divise ce flux par deux. Corrigé par l'analyse : la rampe proposée (30-40°) réduisait aussi la divergence du
  flanc polaire des anticyclones et faisait disparaître les déserts de 30-40° (22 → 8 % de la bande avec les autres
  corrections) ; à 40-50°, elle commence au début de la cellule de Ferrel. Une rampe à 45-55° (8 graines) donne 40-50° 744 mm
  au lieu de 798, mais 50-60° 1 026 au lieu de 933 et 60-70° 910 au lieu de 896 : écartée.
- **Essais sans gain** : saisons plus marquées sur les continents (60-70° plus arrosé), inertie thermique de la mer (côtes
  basses de 40-60° +90 mm), décalage saisonnier des ceintures réduit (60-70° 869 → 1 182 mm), fronts étouffés sous le
  couvercle (déserts de 30-40° en hausse), dépression thermique peu profonde (côtes est −1/3), façade multiplicative (E − O
  trop faible sur de petits continents), cellule de Hadley d'hiver plus forte.
- **Résultat** (8 graines, avant → après) :

| Indicateur | Repère Terre | Avant | Après |
|---|---|---|---|
| Moyenne des terres | ≈ 850 mm | 1 228 | 1 205 |
| Ruissellement | 290-460 mm | 659 | 639 |
| Déserts (cases de terre) | — | 13,7 % | 12,5 % (bande 30-40° : 22 → 13 %) |
| Équateur 0-10° | 1 800-2 300 mm | 2 174 | 2 162 |
| Intérieur 20-30° | < 250 mm | 557 | 505 |
| Côte ouest 20-30° | < 200 mm | 456 | 373 |
| Côte est / côte ouest 20-30° et 30-40° | > 1,5 | 1,73 et 0,87 | 2,34 et 0,86 |
| Intérieur / côte ouest 40-50° et 50-60° | < 0,7 | 0,98 et 1,08 | 0,71 et 1,06 |
| 60-70° | 300-500 mm | 949 | 896 |
| Hautes terres / basses 30-40° et 40-50° | ≈ 1,3-2 | 3,56 et 2,61 | 1,82 et 1,56 |

- **Effets de bord** : 40-50° plus arrosé (médiane 603 → 798 mm ; côtes ouest 730 → 1 150, intérieur 719 → 816, basses terres
  552 → 759) : l'eau qui ne tombe plus sur les montagnes et la vapeur que garde le flux de Ferrel s'y arrêtent. Biomes de 40-50° :
  forêt 32 → 46 %, la steppe sort des cinq premiers. 30-40° : forêt 23 → 28 %, méditerranéen 12 → 17 %. Le test
  `climats_selon_la_latitude` passe (rapport tempéré / subtropical 1,31) ; il ne peut pas trancher seul : son continent d'essai
  est à l'échelle de la Terre (3,3 lignes par degré, 5 fois plus que la planète).
- **Reste** : côtes est de 30-40° aussi sèches que les côtes ouest, intérieur des tropiques secs (pluie d'été de la mousson),
  60-70° encore deux fois la Terre, intérieurs de 40-50° trop arrosés, moyenne des terres. Tous renvoient à l'air marin trop sec
  (7e) et aux dépressions qui se déplacent (7g). Les eaux froides des courants sont 2 à 3 fois trop faibles (côtes ouest de
  20-30° : −2,4 °C au large, Terre −4 à −8 °C).

## Air marin (08/11/2026, TODO 7e) : chantier en pause
Décision de Benjamin : rendre l'air marin humide en corrigeant la circulation et les échanges de la couche basse (étape 1), puis
rendre leur pluie aux terres par une pluie déclenchée par l'humidité (étape 2). Méthode : trois agents par étape, deux
analystes, sur un **banc d'essai** (`banc_air_marin` : planète presque océanique avec deux continents rectangulaires à 300 m,
≈ 4 s ; bilan de la vapeur de la couche basse en 12 termes, `Climate::low_budget`). Résultat : l'air marin est devenu humide,
mais les terres tropicales ont perdu leur pluie, et rien de ce qui a été essayé ne la leur rend. `develop` garde le climat de
c8e3f50 ; les essais restent sur des branches locales.
- **Étape 1, réussie sur la mer** (branche `air-marin`, 6141f4a). Défauts trouvés : la cellule de Hadley était comptée deux fois
  (convergence de la pente des ceintures + circulation de fond imposée) ; sur cette petite planète (rayon ≈ 1/5 de la Terre),
  les vitesses verticales étaient 3 à 5 fois celles de la Terre et la couche basse se renouvelait en ~2 jours (Terre 8-9) ; la
  descente retirait deux fois l'air bas qui diverge et le remplaçait en partie par de l'air sec. Corrections : circulation de
  fond fermée (`hadley_return`, vent méridien par continuité, ∝ la taille de la planète : 0,3 m/s ici, 1,6 sur Terre), descente
  cohérente en masse, `LOW_LAYER_WIND` 1,2, couche haute sans faux retour vers le pôle, air soulevé lentement avec la vapeur du
  haut de la couche (`ASCENT_TOP_HUMIDITY`), air entraîné à l'humidité de la base de la couche haute (`ENTRAINED_HUMIDITY` 2),
  part du vent qui descend la pente des ceintures gardée près du sol mais retirée du transport. Banc : humidité de la mer à
  0-30° 43-49 → 73-74 %, évaporation 0-30° 2 078-3 442 → 1 144-1 704 mm. Vrai monde (3 graines) : équateur 2 080 → 1 463 mm,
  déserts 12,1 → 17,8 % (bande 10-20° : 48 %) ; gains à garder : côte ouest 20-30° 244 mm, 60-70° 516, hautes / basses 1,53.
- **Étape 2, sans solution**. Convection de Betts-Miller (C1) : presque sans effet. Convection selon l'instabilité (C2) :
  négatif, sans apport de vapeur la convection ne peut rien. Descente bornée sous les tropiques (C3, Sobel et al. 2001) :
  remplit presque tous les critères, mais **crée 252 mm d'eau par an (27 % de la pluie de la planète)** ; une version qui
  conserve l'eau revient à l'étape 1. Défaut trouvé : le transport case par case envoie l'air en parts en cos², d'où une
  divergence artificielle en bandes (w ≈ Δ²u/120) ; le transport aux faces (branche `analyse-etape2`, 8227ba9, derrière des
  réglages `A_*` à retirer) fait passer le désert de 0-10° de 17 à 10 %, mais 30-40°, arrosé par ces fausses pluies,
  s'assèche. Meilleure combinaison qui conserve l'eau (faces + convection C1, 8 graines) : équateur 1 659 mm, déserts 19,4 %,
  côte ouest 20-30° 190, 60-70° 410, hautes / basses 1,51.
- **Pas de fuite d'eau** : compté par case (toutes les cases ont la même masse d'air dans le modèle), pluie et évaporation
  de la planète s'équilibrent à 1,5 % près (banc : −7 mm/an à l'étape 1, −12 à `develop` ; l'écart sort par les bords
  polaires au transport et au changement de saison). Pondéré par cos φ, on trouve −57 et −131 mm : ce n'est pas un défaut,
  le modèle ne conserve pas l'eau ainsi. Le banc affiche les deux (ligne `EAU`).
- **Mousson** (banc `banc_mousson` : plein été boréal seul). Cause trouvée : la circulation de fond de l'étape 1 reste calée sur
  l'équateur thermique de la mer (~10° N l'été), alors que la dépression des continents remonte vers 17° N ; entre les deux,
  la couche basse diverge et l'air sec d'altitude descend : intérieur de 5-10° N 1 158 → 435 mm/an au rythme de l'été, 10-15°
  N 699 → 80. Trois essais (branche `air-mousson`, 367b436), vrai monde sur 3 graines :

| | Étape 1 | Fond calé sur la mousson | + air aspiré par la dépression (frottement, 0,3) | + montée profonde seulement dans un air humide |
|---|---|---|---|---|
| Équateur 0-10° | 1 463 mm | 1 360 | 1 428 | 1 324 |
| Désert 10-20° / 20-30° / 30-40° | 48 / 28 / 9 % | 41 / 42 / — | 43 / 8 / 26 | 45 / 9 / 27 |
| Intérieur 20-30° | 382 mm | 222 | 1 079 | 1 067 |
| Déserts, toutes terres | 17,8 % | 17,2 % | 18,2 % | 19,4 % |

  Le désert ne fait que se déplacer : la pluie tombe là où est la dépression thermique, alors que sur Terre elle reste sèche
  (air sec venu du pôle, l'harmattan ; descente au-dessus du Sahara induite par la mousson d'Asie, Rodwell et Hoskins 1996) et
  que la pluie tombe sur son bord côté équateur. L'air qu'elle aspire vient de la mer et converge à plus de 85 % d'humidité :
  une règle sur l'humidité n'y change rien.
- **Pour reprendre** : il faut une autre façon de calculer la circulation tropicale des continents (dépression thermique sèche
  coiffée par la descente, pluie sur son flanc équatorial, mousson qui traverse l'équateur), pas un réglage de plus. Partir
  de l'étape 1 et du transport aux faces, avec les deux bancs (`banc_air_marin`, `banc_mousson`) et le bilan
  `bilan_pluie_comparatif` ; ne pas réessayer une borne qui crée de l'eau, ni l'air aspiré par la dépression seul.

## Limites connues / pistes
- Courants marins fixes toute l'année (calculés une fois, sur le vent annuel moyen).
- Le générateur de relief (bruit fractal) sera remplacé par une génération géologique.
- Les mers intérieures sont traitées comme l'océan (courants compris, même modèle).
