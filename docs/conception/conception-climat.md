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
- Température de surface = température selon la latitude + écart dû aux **courants marins** (module `currents.rs`, voir ci-dessous) ; les côtes reprennent 70 % de l'écart de la mer voisine. (Avant le 30/10/2026 : règles fixes, côtes ouest 15-35° −4 °C, côtes est +3 °C, côtes ouest 45-65° +5 °C.)
- Évaporation « bulk » (~2 300 mm/an sous les tropiques, ~800 vers 50°). Les lacs évaporent de la même façon (à partir de la deuxième passe).
- Pas d'affichage au survol des cases d'eau (décision de Benjamin).

## Courants marins (30/10/2026, choix de Benjamin : version simplifiée, carte des courants)
- **Modèle de Stommel** (1948) sur les cases de mer : `r ∇²ψ + β ∂ψ/∂x = rot τ / (ρ H)`, ψ fonction de courant de la couche mélangée (H = 1 000 m), τ tension du vent (ρ_air 1,2, Cd 1,3·10⁻³, vent normalisé × 8 m/s), β = 2Ω cos φ / R (Ω terrestre, R ≈ 1 200 km tiré de la carte : β ≈ 5 fois celui de la Terre). Frottement r choisi pour que la couche de bord ouest (r/β à 30°) fasse 1,5 case : tous les coefficients de la discrétisation restent positifs, surrelaxation stable (ω = 1,7 ; ω = 1,85 diverge, l'équation n'étant pas symétrique).
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
- Courants marins fixes toute l'année (calculés une fois, sur le vent annuel moyen).
- Le générateur de relief (bruit fractal) sera remplacé par une génération géologique.
- Les mers intérieures sont traitées comme l'océan (courants compris, même modèle).
