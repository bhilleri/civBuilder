# CivBuilder — prochaines tâches

Décidées avec Benjamin (fin octobre 2026), dans l'ordre conseillé.

## [x] 1. Plaques océaniques polaires et passage maritime — fait et validé (30/10)
Voir `conception/conception-plaques.md`.

## [x] 2. Mers fermées traitées comme les golfes — fait (30/10)
Voir `conception/conception-toponymie.md` (mers fermées : fermeture + taille).

## [x] 3. Seuils sous-marins — fait (30/10)
Voir `conception/conception-toponymie.md`. Constat : sur 11 graines, aucune mer fermée (les grandes
poches sont des entonnoirs ouverts) ; à voir avec Benjamin s'il faut en provoquer à la génération.

## [x] 4. Unités, villes, camps, villages enfouis dans le relief (3D) — fait (30/10), vérifié en captures
Posés sur le vrai sol, sites qui suivent le terrain (pas de terrasse), figurines 3D des unités.
Voir `conception/conception-altitude.md`.

## [x] 5. Courants marins — fait (30/10)
Voir `conception/conception-climat.md` (« Courants marins »). Version simplifiée : circulation de surface poussée par les vents simulés (tourbillons
subtropicaux et subpolaires, courants équatoriaux, renforcement à l'ouest des océans, courant
circumpolaire si le passage polaire existe). Effets :
- **température de la mer** calculée (remplace les règles fixes du climat : côtes ouest froides,
  côtes est chaudes…), donc température et **humidité** des côtes (déserts côtiers sur les
  courants froids, douceur et pluies sur les courants chauds, brouillards) ;
- **pêche** : rencontre d'un courant froid et d'un courant chaud ;
- plus tard : **navigation** (bonus/malus selon le courant), dérive de la banquise.
Après les plaques polaires (la forme des océans en dépend).

## [x] 6. Continents moins compacts (décidé le 30/10) — fait (30/10)
Constat de Benjamin : la carte donne souvent une seule grande masse continentale, aux formes
compactes. Cause : une plaque continentale est presque entièrement émergée (la côte suit la
frontière de plaque, forme ronde de la croissance), et les plaques continentales voisines se
soudent par collision. Trois étapes, chacune vérifiée par bilan et captures :
- [x] a. **Côtes découpées** (fait, 30/10) : socle continental plus bas, bruit déformé (golfes, péninsules,
  mers peu profondes), plateau continental sur les marges.
- [x] b. **Blocs de croûte continentale dans les plaques** (fait, 30/10) (au lieu de plaques entièrement
  continentales) : tailles très inégales, microcontinents, un continent peut chevaucher plusieurs
  plaques (chaînes de collision intérieures) et une plaque porter continent et océan ; règle
  d'espacement entre blocs (pas de supercontinent systématique). Nature des frontières selon la
  croûte de chaque côté.
- [x] c. **Rifts selon la vitesse d'écartement** (fait, 30/10) : vallée et lacs (lent), bras de mer étroit
  (moyen), jeune océan entre deux côtes qui se répondent (rapide).

## [ ] 7. Monde trop sec (décidé le 01/11)
Voir `conception/conception-climat.md` (« Cycle de l'eau tropical »).
- [x] a. Cycle de l'eau tropical (01/11) : sommet des nuages selon la chaleur, rendement des pluies.
- [x] b. Vent près de l'équateur (01/11) : part du vent qui descend la pente de pression plafonnée (convergence divisée par 2 à 3).
- [x] d. Mousson continentale (01/11) : pression qui suit le soleil plus loin sur les continents tropicaux ; désert de la bande 10-20° −20 à −30 %.
- [ ] e. Air marin encore sec (~45 %, Terre ~75-80 %), eau de l'air ~10 mm par case (Terre ~25), séjour ~4 jours (Terre 8-9) : limite commune des déserts côtiers et des côtes est. Essayés sans gain (02/11, voir `conception/conception-climat.md`, « Bilan d'eau de la planète ») : règle de pluie selon l'humidité, troisième couche, convection de Betts-Miller, convergence limitée. Il n'y a pas de fuite ; 87 % de la pluie vient de la convergence des vents, la convection est inactive. À étudier : la circulation de la couche basse (temps de trajet de l'air marin, vitesse 1,7 × le vent du sol, renouvellement par la subsidence, diffusion numérique), puis une pluie déclenchée par l'humidité.
- [x] f. Côtes est (02/11) : dépressions d'hiver au-dessus des mers chaudes (fronts et brassage). Couvercle d'inversion et contraste terre-mer essayés sans gain. Encore trop sèches (P/ETP 0,3-0,8).
- [ ] g. Dépressions qui se déplacent (régimes plus courts, systèmes décalés vers l'est) : transport par les tourbillons, à la place du brassage (après e).
- [x] c. Relief trop haut (01/11) : altitude en mètres d'abord (échelle fixe depuis les plaques), 9 niveaux déduits des mètres, falaise en mètres (voir `conception/conception-altitude.md`).

## Décisions de Benjamin après la première campagne de tests (07/11)
- [x] **Rivières** (07/11) : plus de seuil de bassin de 6 cases. Un seuil de débit seul, et une largeur qui part de presque rien à la source (T3-01).
- [x] **Pluies extrêmes plafonnées** (07/11), avec du bruit sur le plafond et l'humidité non tombée reportée aux cases suivantes : 8 à 21 % de cases de désert en moins (T0-03).
- [x] **Raccord 3D** (07/11) : unités, bâtiments et rivières visibles des deux côtés (T5-01, T3-02). Reste une paroi sombre, en vue tournée surtout, quand une case du bord domine sa voisine de l'autre côté.
- [ ] **Forêts tempérées à moins de 10 °C en été** : 105 à 435 cases par graine, à revoir avec les biomes (villages en yourtes au milieu des arbres).
- [ ] **Sites inaccessibles à pied** : pas par défaut. Éventuellement une option au lancement de la partie pour les autoriser (T0-08, T4-11).
- [ ] **Or** (plus tard) : acheter des unités et des bâtiments, entretenir le tout, éventuellement un coût structurel de l'État.
- [ ] **Frein à l'expansion et choix de développement** (plus tard) : à concevoir avec l'or.
- [x] **Limite d'habitat selon le climat** (T0-09), **relief moins adouci sous les sites** (T4-18), **fleuves limoneux plus bleus en 3D** (T5-22) : faits (07/11).
- [x] **Rias perchées** (T0-04, 07/11) : seconde érosion après le climat, par le débit réel, renforcée à l'embouchure par la houle et la marée (voir `conception/conception-erosion.md`). Aucune embouchure à 500 m ou plus sur 8 graines.

## Plus tard
- [x] Console de debug, étape 2 (01/11) : modifications locales (`~`) et `verifier`.
- [x] Console de debug, étape 3 (01/11) : `regenerer eau|climat`, `fleuve`, unités, villes, tours,
  `chemin` (voir `conception/conception-interface.md`).
- [x] Villes et villages par stades, muraille, matériaux du lieu, adaptation à la case (`conception/conception-structures.md`).
- [x] Camps : trois types selon la case, silhouette hérissée, nom lié au type, fumée d'alerte.
- [x] Marais : pilotis au-dessus des mares, galeries, passerelles, drainage.
- [x] Structures : ponts et tête de pont d'une ville fortifiée (décor pour l'instant).
- [ ] Effet de jeu d'un pont (traversée d'une rivière) : à décider avec Benjamin.
- [ ] Règle de la fortification (production d'un bâtiment, effet en combat) : la muraille ne se pose encore qu'à la console.
- [x] Banquise jouable (06/11) : calcul, affichage, marche, pas de ville ni de structure, pêche conservée (voir `conception/conception-fonds-marins.md`).
- [x] Toponymie étape 3 (01/11) : continents, îles, îlots, archipels, péninsules, isthmes (`lands.rs`).
- [x] Toponymie étape 4 (01/11) : chaînes, massifs, cols, plateaux, collines (`mountains.rs`).
- [x] Toponymie étape 5 (01/11) : régions naturelles et sous-régions (`regions.rs`).
- [x] Toponymie étape 6 (01/11) : barre de lieu (voir `conception/conception-toponymie.md`).
- [ ] Navigation et unités navales (barques, galères, caravelles), pêcheries.
- [ ] Police Inter Italic (les noms marins devraient être en italique).
