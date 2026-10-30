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

## Effet de lac (24/10/2026, choix de Benjamin : version simplifiée)
- Constat : avec la seule évaporation du lac, la pluie sous le vent ne gagnait que 1 à 2 % (lac Huron, graine 777) ; sous les alizés (air qui descend), la vapeur ajoutée ne pleut pas.
- Ajout : **vitesse verticale supplémentaire** `LAKE_EFFECT_W` = 0,003 m/s (convection pleine : 0,004) sur le lac (moitié) et sur la **rive vers laquelle souffle le vent** du régime courant (produit scalaire direction lac → case · vent), décroissant jusqu'à `LAKE_EFFECT_KM` = 130 km (≈ 3 cases). Multipliée par le couvercle d'inversion : étouffée sous les anticyclones, comme la convection.
- Représente la brise de lac et l'air réchauffé par l'eau (effet de lac), faute de saisons et d'inertie thermique de l'eau.
- Graine 777, pluie de la 1ʳᵉ à la 5ᵉ case sous le vent (sans lac → avec lac et effet) : lac Nicaragua (14°, alizés) +28 % → +13 % (294 → 374 mm sur la rive) ; lac Titicaca +38 % → +7 % ; lac Malawi +18 % → +8 % ; lac Huron (37°) +5 % → +2 % ; lacs équatoriaux (Victoria, Tanganyika) +6 à +14 %.
- Observation : l'évaporation « bulk » des lacs tropicaux continentaux est forte (3 000 à 4 800 mm/an, contre 1 500 à 2 500 mm pour des lacs réels comme le Victoria ; ~2 700 pour le lac Nasser réel) : la température de l'eau est prise égale à celle de l'air continental. À recaler (température de l'eau plus fraîche) si les lacs paraissent trop évaporer.

## Circulation
- Pression : ceintures selon la latitude, effet thermique des continents, dépressions/anticyclones mobiles (~2 500 km).
- Vent de surface très dévié (72-84°) ; couche basse 1,7 × plus rapide ; vents d'altitude surtout d'ouest.
- Circulation de fond imposée par la latitude (descente vers 30°, montée à l'équateur et vers 60°).
- Cohérence de masse : l'air de la couche basse qui converge monte ; celui qui diverge est remplacé par de l'air qui descend.
- Fronts des latitudes moyennes (35-65°) et convection tropicale ; couvercle d'inversion sous les anticyclones.
- Déplacement saisonnier des ceintures : 0,45 × latitude du soleil.

## Océan
- Température de surface = température selon la latitude + anomalie des courants (froid côtes ouest 15-35° −4 °C, chaud côtes est +3 °C, dérive chaude côtes ouest 45-65° +5 °C) ; les côtes reprennent 70 % de l'anomalie voisine.
- Évaporation « bulk » (~2 300 mm/an sous les tropiques, ~800 vers 50°). Les lacs évaporent de la même façon (à partir de la deuxième passe).
- Pas d'affichage au survol des cases d'eau (décision de Benjamin).

## Deux couches d'air
- Basse : 0-2 km au-dessus d'un relief adouci (lissé sur ~40 km). Haute : centrée à 4 km.
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

## Limites connues / pistes
- Versants tropicaux exposés : jusqu'à ~19 000 mm sur une barrière de 3 800 m (records réels 11-13 000).
- Courants marins fixes, déduits de la forme des côtes.
- Le générateur de relief (bruit fractal) sera remplacé par une génération géologique.
- Les mers intérieures sont traitées comme l'océan (courants compris).
