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
- [ ] b. Vent près de l'équateur (convergence 15 à 20 fois trop forte, air marin encore sec) ; bande 10-20° des grands continents.
- [ ] c. Relief deux fois trop haut (16 % des terres sous 300 m, Terre ~33 %) : hauts plateaux équatoriaux en forêt tempérée au lieu de jungle. Baisser les altitudes des niveaux (par ex. 150 / 350 / 600 / 1 100 / 1 800 m) changerait aussi le nombre de montagnes (`MOUNTAIN_MIN_M`).

## Plus tard
- [x] Console de debug, étape 2 (01/11) : modifications locales (`~`) et `verifier`.
- [x] Console de debug, étape 3 (01/11) : `regenerer eau|climat`, `fleuve`, unités, villes, tours,
  `chemin` (voir `conception/conception-interface.md`).
- [ ] Banquise jouable (eau gelée, selon la température de la mer).
- [ ] Toponymie étapes 3 à 6 : continents, îles, archipels, péninsules, isthmes ; chaînes, massifs,
  cols ; régions naturelles ; barre de lieu (voir `conception/conception-toponymie.md`).
- [ ] Navigation et unités navales (barques, galères, caravelles), pêcheries.
- [ ] Police Inter Italic (les noms marins devraient être en italique).
