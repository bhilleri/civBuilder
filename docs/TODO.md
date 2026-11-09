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

## [ ] 8. Rendu 3D (décidé le 09/11)
Voir `conception/conception-rendu-3d.md` (terrain continu avec grille par-dessus, style maquette
semi-réaliste, relief marqué et réglable, modèles CC0 retouchés et maison).
- [ ] a. Terrain continu en blocs : hauteurs, shader de terrain, données par case, grille par-dessus (prototype d'abord).
- [ ] b. Matières du sol, règles pente / altitude / neige, lisibilité.
- [ ] c. Eau animée : houle, écoulements des courants et des fleuves, profondeur, rivage.
- [ ] d. Végétation instanciée, clairières sous les unités et structures, vent.
- [ ] e. Unités glTF animées, animations par type et compétence, particules.
- [ ] f. Structures glTF, décalques au sol, villes qui grandissent.
- [ ] g. Saisons visuelles (après la tâche 9), nuages, brouillard de guerre stylisé.

## [ ] 9. Saisons, calendrier et météo (décidé le 09/11)
Voir `conception/conception-saisons.md` (1 tour = 1 mois, mode 2 tours par mois, cycle séparé de la
date historique, météo = moyennes + écarts, lit d'hiver et d'été). Avant le rendu (8 g).
- [ ] a. Calendrier : mois, saison locale, mode 2 tours par mois.
- [ ] b. Climat mensuel interpolé depuis les quatre saisons.
- [ ] c. Météo, fond lent : écart de pression caché, anomalies durables ; bilan de cohérence avec le climat.
- [ ] d. Météo, systèmes qui se déplacent (dépressions, anticyclones, tempêtes), même moteur que 7 g à terme.
- [ ] e. Neige au sol, gel des lacs et cours d'eau, boue au dégel.
- [ ] f. Débit mensuel : crues, étiages, lit mineur et majeur, oueds.
- [ ] g. Effets sur les déplacements.
- [ ] h. Cyclones (systèmes tropicaux du même moteur), carte du risque.
- Plus tard : navigation, combat, récolte et stocks, santé, prévisions.

## [ ] 10. Volcans (décidé le 09/11)
Placés d'après la tectonique déjà simulée (`plates.rs` : subductions, arcs d'îles, rifts,
dorsales, points chauds et leurs chapelets), avec une forme selon le type. Voir
`conception/conception-volcans.md` (passage selon le relief généré, sans règle propre).
- [ ] a. Placement et type selon le contexte tectonique, activité (actif, endormi, éteint) ; bilan `bilan_volcans`.
- [ ] b. Relief : édifices dans l'altitude de la génération (avant le climat et l'eau), lacs de cratère.
- [ ] c. Formes 3D selon le type (stratovolcan, bouclier, caldeira, dôme, champ de cônes, plateau de basalte, atoll), matières (basalte, cendres, coulées), fumerolles (après l'étape 8 a).
- [ ] d. Noms (toponymie), infobulle, wiki, carte thématique.
- [ ] e. Sols volcaniques fertiles (avec la tâche 11) ; ressources liées (obsidienne, soufre…) plus tard.
- [ ] f. Éruptions, plus tard (décidé le 10/11) : avec les risques naturels (20), après la météo (9) ; lien avec l'hiver volcanique.

## [ ] 11. Fertilité des sols (décidé le 09/11)
Pas de fertilité qui s'améliore à l'infini (comme dans Civilization). D'abord une recherche sur
les conditions qui font la fertilité d'un sol (roche mère, climat, relief, eau, âge…), puis les
sources : sols volcaniques, bassins sédimentaires, plaines alluviales, etc. Règles à décider avec Benjamin.

## Propositions retenues le 09/11
Ordre conseillé : 12 au plus tôt ; puis 13 → 11 → 14 (chacune prépare la suivante) ; les autres
avec le sujet voisin (18 avec la navigation, 20 avec la météo). Chaque tâche : conception à
proposer et à valider avec Benjamin avant de coder.

### [ ] 12. Sauvegarde et chargement
Enregistrer la graine, l'état de la partie et ce que le joueur a modifié ; le monde se recalcule
depuis la graine. À faire tôt : plus la simulation s'enrichit (météo, saisons), plus c'est difficile.

### [ ] 13. Géologie de surface (roche mère), avec une carte thématique
Calcaire, granite, grès, argile, basalte, lœss, alluvions, déduits des plaques, des bassins et
de l'érosion ; karst (grottes, gouffres, rivières qui disparaissent). Base de la fertilité (11),
des ressources (14) et des matériaux des structures selon le lieu.

### [ ] 14. Ressources placées par la géologie
Cuivre et or des subductions, étain des granites, fer des vieux bassins, sel des bassins fermés,
côtes arides et marais salants, charbon des anciens marais, or alluvial ; obsidienne et soufre
des volcans (10 e). Le joueur peut raisonner en géologue.

### [ ] 15. Plantes cultivables et animaux domesticables selon le climat
Blé, orge, riz, maïs, mil, pomme de terre ; chèvres, bovins, chevaux, lamas. Pour la variété,
**sans trop de conséquences sur le jeu** (décision de Benjamin).

### [ ] 16. Démographie et santé des villes
Naissances, famines (avec la météo), migrations, croisées avec d'autres indicateurs comme la
santé de la ville. Exigence : **un système compréhensible** par le joueur.

### [ ] 17. Transport : fleuves et routes
Le débit dit si un fleuve est navigable et à quelle saison ; les cols deviennent des passages
stratégiques ; le commerce dépend du coût du trajet (relief, coût des cases, rivières). Il faut
**construire les aménagements nécessaires** : port fluvial, routes, ponts (déjà là).

### [ ] 18. Littoral détaillé
Estran (marées), deltas, mangroves, récifs coralliens, côtes rocheuses ou sableuses, abris
naturels pour les ports (baies, fjords, rias). Complète les fonds marins et la navigation.

### [ ] 19. Glaciers et pergélisol
Selon le froid et l'altitude, avec la banquise ; les glaciers alimentent les fleuves en été (débit mensuel, 9 f).

### [ ] 20. Risques naturels, intégrés dès le départ
Séismes et tsunamis aux subductions, feux de forêt en saison sèche (météo), crues, cyclones :
chacun avec sa carte de risque calculée à la génération et l'infobulle. **Tout intégrer
directement** dans les systèmes concernés, plutôt que de l'ajouter après coup comme pour les poissons.

### [ ] 21. Faune sauvage et période nomade (à voir)
Idée de Benjamin : une période de début de partie sans vraie sédentarisation. Gibier selon le
biome, troupeaux qui migrent avec les saisons, animaux dangereux. Point à étudier : les
déplacements des animaux compliquent la présence des ressources.

### [ ] 22. Carte du joueur imparfaite
Lieux nommés et précisés à mesure qu'on les découvre ; le joueur peut renommer un lieu ou fonder un nom.

### [ ] 23. Comparaison automatique des captures
Comparer les PNG de `CIVBUILDER_CAPTURE` avant et après un changement (utile pour la refonte du rendu 3D, tâche 8).

### [ ] 24. Sons
Ambiance (vent, mer, pluie selon la météo) et sons des unités, des villes, des actions. Réactiver l'audio de Bevy.

### [ ] 25. Palettes lisibles pour les daltoniens
Mesurer l'impact visuel pour les joueurs non daltoniens : palettes communes si l'impact est
faible, sinon un mode pour les daltoniens.

### [ ] 26. Atlas géographique (idée de Benjamin, 09/11)
Atlas des territoires connus du joueur, avec images et icônes, qui va avec la carte imparfaite (22) :
- **Plan** : monde, continents et océans, régions naturelles, puis fiches de lieux (fleuves, lacs,
  mers, chaînes, cols, îles, villes), d'après la géographie déjà nommée (`lands.rs`,
  `mountains.rs`, `regions.rs`, `toponymy.rs`) et l'index de recherche (`search.rs`).
- **Fiche d'un lieu** : carte de situation, vignette 3D rendue par le jeu (comme les captures),
  chiffres réels (débit, longueur, altitude, surface), climat (diagramme des températures et des
  pluies par mois), origine du nom.
- **Records du joueur** : plus grand fleuve connu, plus haut sommet, plus grand lac…
- **Relations** : fleuve qui alimente une ville en eau, axe commercial entre deux villes (après
  17), col qui relie deux régions, risques connus (20).
- **Histoire** : découvert au tour N par telle unité, ville fondée, événements (crues, éruptions).
- Seulement ce que le joueur connaît ; liens vers la carte (vol de la caméra, comme la recherche).
  Les informations secondaires vont dans l'atlas plutôt que sur la carte.

### [ ] 27. Villes : ressources, indicateurs et répartition de la population
Voir `conception/conception-villes.md`. Décidé : Population (habitants → indice) ; principales
Nourriture, Production (construire), Économie (entretenir, acheter, diplomatie), Recherche ;
capacité Eau ; secondaires Santé (le corps), Bonheur (la satisfaction), Éducation (alphabétisation) ;
Attractivité calculée (migrations). Décidé le 10/11 : intendant guidable (cases forcées, priorités
par ressource), population répartie sur plusieurs cases selon leur capacité, aménagements et
bâtiments construits par la ville (seuils et paliers pour les bâtiments, rendements décroissants
pour le rural), passage de rural à urbain, pénalité de distance (von Thünen), une seule file de
construction, 7 sous-cases par case portant des quartiers (plan selon les bâtiments dominants),
population active variable, économie (cases, impôt, surplus, commerce), entraide par envoi de
production. Culture et politique : pistes retenues comme base, à détailler dans une doc dédiée.

### [ ] 28. Armée : armes, équipement, formation, expérience
Voir `conception/conception-combat.md` (« Armée »). Soldats formés (pas produits) dans la ville
ou les casernes avec une formation progressive (accélérée par les casernes, recrutement possible
avant la fin avec des malus), armes comme ressource stratégique (production seule ; vendues,
capturées), taux d'équipement par type, attrition, niveaux d'expérience, effectif selon le type,
entretien en nourriture, économie et matériel, soldats comptés dans la population de leur ville
(pertes = population perdue). Après la tâche 27.

## Plus tard
- [x] Console de debug, étape 2 (01/11) : modifications locales (`~`) et `verifier`.
- [x] Console de debug, étape 3 (01/11) : `regenerer eau|climat`, `fleuve`, unités, villes, tours,
  `chemin` (voir `conception/conception-interface.md`).
- [ ] Banquise jouable (eau gelée, selon la température de la mer).
- [x] Toponymie étape 3 (01/11) : continents, îles, îlots, archipels, péninsules, isthmes (`lands.rs`).
- [x] Toponymie étape 4 (01/11) : chaînes, massifs, cols, plateaux, collines (`mountains.rs`).
- [x] Toponymie étape 5 (01/11) : régions naturelles et sous-régions (`regions.rs`).
- [x] Toponymie étape 6 (01/11) : barre de lieu (voir `conception/conception-toponymie.md`).
- [ ] Navigation et unités navales (barques, galères, caravelles), pêcheries.
- [ ] Police Inter Italic (les noms marins devraient être en italique).
- [ ] À évaluer (09/11) : aménagement du territoire (défricher, irriguer, terrasses, drainer, digues, canaux) avec ses conséquences (érosion des sols, salinisation). Risque : rendre le jeu trop complexe.
