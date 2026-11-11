# CivBuilder — prochaines tâches

Décidées avec Benjamin (depuis fin octobre 2026). Ordre de réalisation : voir « Feuille de route »
ci-dessous (proposée le 10/11 après les relectures).

## Feuille de route (validée le 10/11)
Chaque étape livre quelque chose de jouable ou de mesurable ; l'ordre suit le dévoilement par ère
(`conception/conception-echelle-et-but.md`). Les tâches de la planète (7, 8, 9, 10, 11) et de
l'interface (35, 36) avancent en parallèle, quand elles servent l'étape en cours.

1. ~~**Fondations techniques (45)**~~ (faites le 10-11/11, sauf la carte connue partagée, reportée) : RNG déterministe par graine, identifiants stables, peuple
   propriétaire des unités et des villes, mesure du temps par système (44), brouillard en bits,
   calcul de chemin en tableaux plats.
2. **Partie sauvegardable** : ~~sauvegarde et chargement (13)~~ (fait le 11/11) ; état de partie hors de l'ECS (reporté aux peuples rivaux) ; code de
   partie et écran de nouvelle partie minimal (42).
3. **Échelle recalée (30, première partie)** (population en habitants, paliers, PM et relief doublés : faits le 11/11) : PM et coûts du relief doublés, croissance, soins,
   population en habitants ; déplacements de groupe (39, première partie : traverser les alliés,
   sélection multiple et groupes au clavier faits le 11/11 ; relève à venir) ; écarts code et conception (33).
4. **Villes du Néolithique (28, 17)** : intendant, aménagements, Bien-être, hygiène, territoire par
   la Culture ; refonte de l'écran de la ville (35). Règles et chiffres validés le 11/11
   (`conception-villes.md`, « Étape 4 ») ; 4a (travailleurs sur les cases) faite le 11/11.
5. **Sol et ressources (14, 12, 15, 16)** : géologie, fertilité, gisements, plantes et animaux.
6. **Période nomade (22)** et **société de base (32)** : chefferie, conseil, dirigeant, Culture,
   influence, capacité de gouverner.
7. **Recherche et jalons (34, 30 seconde partie)** : arbre du Néolithique et du Bronze, passage d'ère.
8. **Armée réduite (29)** et barbares en clans (31, première partie).
9. **Peuples rivaux (31)** et **diplomatie, première étape (38)** : contact, relation, confiance,
   guerre et paix ; `bilan_ia`.
10. **Âge du bronze** : commerce entre villes, premier temps (18), religion organisée, diplomatie
    complète, assemblées, ambassades.
11. **Âge du fer et Antiquité** : espionnage, tributaires, ordres, épidémies (40), monuments (41),
    galères et ravitaillement (39).
12. **Moyen Âge** : féodalité, vassaux, haute mer, prix du marché (18, second temps).
13. **Finitions** : prise en main (43), cartes thématiques (36), atlas (27), sons (25), équilibrage (37).



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
- [ ] e. **En pause (08/11)**, voir `conception/conception-climat.md`, « Air marin (08/11/2026) » : l'air marin a été rendu humide (branche locale `air-marin`), mais les terres tropicales perdent leur pluie (déserts 12,5 → 17-19 %) et ni la convection, ni le transport aux faces, ni la mousson recalée ne la leur rendent ; pas de fuite d'eau. Reprendre avec une autre circulation tropicale des continents (dépression thermique sèche, pluie sur son flanc équatorial). Ancien constat : Air marin encore sec (~45 %, Terre ~75-80 %), eau de l'air ~10 mm par case (Terre ~25), séjour ~4 jours (Terre 8-9) : limite commune des déserts côtiers et des côtes est. Essayés sans gain (02/11, voir `conception/conception-climat.md`, « Bilan d'eau de la planète ») : règle de pluie selon l'humidité, troisième couche, convection de Betts-Miller, convergence limitée. Il n'y a pas de fuite ; 87 % de la pluie vient de la convergence des vents, la convection est inactive. À étudier : la circulation de la couche basse (temps de trajet de l'air marin, vitesse 1,7 × le vent du sol, renouvellement par la subsidence, diffusion numérique), puis une pluie déclenchée par l'humidité.
- [x] f. Côtes est (02/11) : dépressions d'hiver au-dessus des mers chaudes (fronts et brassage). Couvercle d'inversion et contraste terre-mer essayés sans gain. Encore trop sèches (P/ETP 0,3-0,8).
- [ ] g. Dépressions qui se déplacent (régimes plus courts, systèmes décalés vers l'est) : transport par les tourbillons, à la place du brassage (après e).
- [x] c. Relief trop haut (01/11) : altitude en mètres d'abord (échelle fixe depuis les plaques), 9 niveaux déduits des mètres, falaise en mètres (voir `conception/conception-altitude.md`).

## [ ] 8. Pluie plus proche de la Terre (décidé le 08/11)
Voir `conception/conception-climat.md` (« Comparatif avec la Terre »), bilan `bilan_pluie_comparatif`. But de Benjamin : une
distribution du climat cohérente avec le monde généré (ses continents ne sont pas ceux de la Terre), pas les chiffres exacts.
Correctifs possibles, à essayer et comparer :
- [x] a. (08/11, en partie : flux de Ferrel plus faible au-delà de 40°, 60-70° 949 → 896 mm ; intérieurs de 40-50° encore trop arrosés) **Intérieurs et hautes latitudes trop arrosés** (× 2 à 3 à 40-70°) : assèchement continental (recyclage de l'humidité
  au-dessus des terres, pluie plus faible quand l'air vient de loin dans les terres, évaporation des sols), pluie du froid.
- [x] b. (08/11 : couche haute au-dessus des massifs, rapport 1,6-1,8) **Pluie de relief forte** (hautes terres : 2 à 4 fois les plaines) : moins d'eau perdue par l'air qui monte une pente,
  ou un effet qui sature avec la hauteur.
- [ ] c. (08/11, en partie : façades des anticyclones et frottement des terres ; côtes ouest de 20-30° 456 → 373 mm, côtes est de 30-40° encore sèches) **Côtes est subtropicales trop sèches** (÷ 2 à 4) et **côtes ouest pas assez sèches** : air marin, courants chauds et
  froids, mousson (lié à 7e et 7g).
- [ ] d. (08/11 : 557 → 505 mm ; pluie d'été de la mousson, attend 7e) **Intérieur des tropiques secs trop arrosé** (≈ 550 mm à 20-30° contre moins de 100 au Sahara) : subsidence, couvercle
  d'inversion.
- [ ] e. Moyenne des terres ≈ 1 220 mm (Terre ≈ 850) : devrait suivre des corrections a à d, sans baisse uniforme (les déserts
  reviendraient : voir 7).

## Décisions de Benjamin après la première campagne de tests (07/11)
- [x] **Rivières** (07/11) : plus de seuil de bassin de 6 cases. Un seuil de débit seul, et une largeur qui part de presque rien à la source (T3-01).
- [x] **Pluies extrêmes plafonnées** (07/11), avec du bruit sur le plafond et l'humidité non tombée reportée aux cases suivantes : 8 à 21 % de cases de désert en moins (T0-03).
- [x] **Raccord 3D** (07/11) : unités, bâtiments et rivières visibles des deux côtés (T5-01, T3-02). La paroi sombre en vue tournée (case du bord qui domine sa voisine de l'autre côté) est corrigée (08/11) : la caméra 3D est gardée dans le tour par son œil.
- [x] **Forêts tempérées à moins de 10 °C en été** (08/11) : limite des arbres à 10 °C en été, prairie d'altitude au-dessus (voir `conception/conception-biomes.md`). Reste : les étés trop frais de la taïga (89 % de ses cases sous 10 °C).
- [ ] **Sites inaccessibles à pied** : pas par défaut. Éventuellement une option au lancement de la partie pour les autoriser (T0-08, T4-11).
- [ ] **Or** (plus tard) : acheter des unités et des bâtiments, entretenir le tout, éventuellement un coût structurel de l'État. Décidé le 10/11 : devient « Économie » (tâche 28).
- [ ] **Frein à l'expansion et choix de développement** (plus tard) : à concevoir avec l'or. Décidé le 10/11 : la capacité de gouverner (tâche 28, `conception/conception-villes.md`).
- [x] **Limite d'habitat selon le climat** (T0-09), **relief moins adouci sous les sites** (T4-18), **fleuves limoneux plus bleus en 3D** (T5-22) : faits (07/11).
- [x] **Rias perchées** (T0-04, 07/11) : seconde érosion après le climat, par le débit réel, renforcée à l'embouchure par la houle et la marée (voir `conception/conception-erosion.md`). Aucune embouchure à 500 m ou plus sur 8 graines.

## [ ] 9. Rendu 3D (décidé le 09/11)
Voir `conception/conception-rendu-3d.md` (terrain continu avec grille par-dessus, style maquette
semi-réaliste, relief marqué et réglable, modèles CC0 retouchés et maison).
- [ ] a. Terrain continu en blocs : hauteurs, shader de terrain, données par case, grille par-dessus (prototype d'abord).
- [ ] b. Matières du sol, règles pente / altitude / neige, lisibilité.
- [ ] c. Eau animée : houle, écoulements des courants et des fleuves, profondeur, rivage.
- [ ] d. Végétation instanciée, clairières sous les unités et structures, vent.
- [ ] e. Unités glTF animées, animations par type et compétence, particules.
- [ ] f. Structures glTF, décalques au sol, villes qui grandissent.
- [ ] g. Saisons visuelles (après la tâche 10), nuages, brouillard de guerre stylisé.

## [ ] 10. Saisons, calendrier et météo (décidé le 09/11)
Voir `conception/conception-saisons.md` (1 tour = 1 mois, mode 2 tours par mois, cycle séparé de la
date historique, météo = moyennes + écarts, lit d'hiver et d'été). Avant le rendu (9 g).
- [ ] a. Calendrier : mois, saison locale, mode 2 tours par mois.
- [ ] b. Climat mensuel interpolé depuis les quatre saisons.
- [ ] c. Météo, fond lent : écart de pression caché, anomalies durables ; bilan de cohérence avec le climat.
- [ ] d. Météo, systèmes qui se déplacent (dépressions, anticyclones, tempêtes), même moteur que 7 g à terme.
- [ ] e. Neige au sol, gel des lacs et cours d'eau, boue au dégel.
- [ ] f. Débit mensuel : crues, étiages, lit mineur et majeur, oueds.
- [ ] g. Effets sur les déplacements.
- [ ] h. Cyclones (systèmes tropicaux du même moteur), carte du risque.
- Plus tard : navigation, combat, récolte et stocks, santé, prévisions.

## [ ] 11. Volcans (décidé le 09/11)
Placés d'après la tectonique déjà simulée (`plates.rs` : subductions, arcs d'îles, rifts,
dorsales, points chauds et leurs chapelets), avec une forme selon le type. Voir
`conception/conception-volcans.md` (passage selon le relief généré, sans règle propre).
- [ ] a. Placement et type selon le contexte tectonique, activité (actif, endormi, éteint) ; bilan `bilan_volcans`.
- [ ] b. Relief : édifices dans l'altitude de la génération (avant le climat et l'eau), lacs de cratère.
- [ ] c. Formes 3D selon le type (stratovolcan, bouclier, caldeira, dôme, champ de cônes, plateau de basalte, atoll), matières (basalte, cendres, coulées), fumerolles (après l'étape 9 a).
- [ ] d. Noms (toponymie), infobulle, wiki, carte thématique.
- [ ] e. Sols volcaniques fertiles (avec la tâche 12) ; ressources liées (obsidienne, soufre…) plus tard.
- [ ] f. Éruptions, plus tard (décidé le 10/11) : avec les risques naturels (21), après la météo (10) ; lien avec l'hiver volcanique.

## [ ] 12. Fertilité des sols (décidé le 09/11)
Pas de fertilité qui s'améliore à l'infini (comme dans Civilization). D'abord une recherche sur
les conditions qui font la fertilité d'un sol (roche mère, climat, relief, eau, âge…), puis les
sources : sols volcaniques, bassins sédimentaires, plaines alluviales, etc. Décidé le 10/11 : pour
l'instant un **bonus fixe selon le type de sol**, fondé sur la recherche ; pas d'épuisement ni de régénération.

## Propositions retenues le 09/11
Ordre : voir la « Feuille de route » en tête (19 avec la navigation, 21 avec la météo). Chaque
tâche : conception à proposer et à valider avec Benjamin avant de coder.

### [x] 13. Sauvegarde et chargement — fait le 11/11 (`sauvegarde.rs`)
Fait : sauvegarde texte (RON) versionnée, empreinte du monde, chargement par relance du jeu
(`CIVBUILDER_CHARGER`), menu et console, partie rechargée identique. Reste : journal non gardé ;
retouches de la carte à la console non gardées (sauf les ponts) ; `Partie` hors de l'ECS (pour
`bilan_ia` sans écran) quand les peuples rivaux arriveront ; plusieurs emplacements de sauvegarde
(liste, noms) avec la refonte des écrans (35).
Enregistrer la graine, l'état de la partie et ce que le joueur a modifié ; le monde se recalcule
depuis la graine. À faire tôt : plus la simulation s'enrichit (météo, saisons), plus c'est difficile.
Analyse critique (10/11) : séparer un `MondeGénéré` immuable d'un `ÉtatDuMonde` sauvegardé (ponts,
aujourd'hui dans `WorldMap.bridges`, aménagements, sous-cases, météo, neige, débit) ; versionner la
sauvegarde par la version de la génération et `empreinte_generation` ; tirer le combat et l'IA
hostile de la graine (`rand::rng()` dans `battle.rs` et `hostile.rs`).

### [ ] 14. Géologie de surface (roche mère), avec une carte thématique
Calcaire, granite, grès, argile, basalte, lœss, alluvions, déduits des plaques, des bassins et
de l'érosion ; karst (grottes, gouffres, rivières qui disparaissent). Base de la fertilité (12),
des ressources (15) et des matériaux des structures selon le lieu.

### [ ] 15. Ressources placées par la géologie
Cuivre et or des subductions, étain des granites, fer des vieux bassins, sel des bassins fermés,
côtes arides et marais salants, charbon des anciens marais, or alluvial ; obsidienne et soufre
des volcans (11 e). Le joueur peut raisonner en géologue.
Décidé le 10/11 (`conception/conception-ressources.md`) : les unités dépendent des ressources
(fer, cuivre et étain pour le bronze, bois d'œuvre, chevaux, dromadaires, éléphants) ; une
ressource exploitée rapporte un revenu et approvisionne les troupes comme le matériel militaire ;
chaque ressource militaire garde un usage civil ; or, pierres précieuses, sel… servent la
Recherche, la Santé, la Production.

### [ ] 16. Plantes cultivables et animaux domesticables selon le climat
Blé, orge, riz, maïs, mil, pomme de terre ; chèvres, bovins, chevaux, lamas. Pour la variété,
**sans trop de conséquences sur le jeu** (décision de Benjamin). Décidé le 10/11 : seul effet, la
**variété de la nourriture**, qui améliore le Bien-être.

### [ ] 17. Démographie et Bien-être des villes
Naissances, famines (avec la météo), migrations, croisées avec d'autres indicateurs comme le
Bien-être de la ville (Santé et Bonheur fusionnés, tâche 28). Exigence : **un système compréhensible** par le joueur.

### [ ] 18. Transport : fleuves et routes
Le débit dit si un fleuve est navigable et à quelle saison ; les cols deviennent des passages
stratégiques ; le commerce dépend du coût du trajet (relief, coût des cases, rivières). Il faut
**construire les aménagements nécessaires** : port fluvial, routes, ponts (déjà là).
Décidé le 10/11 (`conception/conception-ressources.md`, « Marché entre les villes ») : chaque ville
couvre son besoin puis vend son surplus aux villes reliées, celles du joueur d'abord ; partenaires
choisis selon l'empire, la durée du trajet, les relations et la rentabilité ; prix par l'offre et la
demande ; pas de caravanes, des bâtiments qui favorisent les échanges ; budget national de chevaux
et de dromadaires pour le transport (arbitrage avec l'armée).

### [ ] 19. Littoral détaillé
Estran (marées), deltas, mangroves, récifs coralliens, côtes rocheuses ou sableuses, abris
naturels pour les ports (baies, fjords, rias). Complète les fonds marins et la navigation.

### [ ] 20. Glaciers et pergélisol
Selon le froid et l'altitude, avec la banquise ; les glaciers alimentent les fleuves en été (débit mensuel, 10 f).

### [ ] 21. Risques naturels, intégrés dès le départ
Séismes et tsunamis aux subductions, feux de forêt en saison sèche (météo), crues, cyclones :
chacun avec sa carte de risque calculée à la génération et l'infobulle. **Tout intégrer
directement** dans les systèmes concernés, plutôt que de l'ajouter après coup comme pour les poissons.

### [ ] 22. Faune sauvage et période nomade
Décidé le 10/11 : tous les peuples partent nomades ; **l'agriculture est requise pour fonder la
première ville** (`conception/conception-echelle-et-but.md`). À préciser : représentation du peuple
nomade (une ou plusieurs unités), chasse et cueillette. Idée de Benjamin : une période de début de partie sans vraie sédentarisation. Gibier selon le
biome, troupeaux qui migrent avec les saisons, animaux dangereux. Point à étudier : les
déplacements des animaux compliquent la présence des ressources.

### [ ] 23. Carte du joueur imparfaite
Lieux nommés et précisés à mesure qu'on les découvre ; le joueur peut renommer un lieu ou fonder un nom.

### [ ] 24. Comparaison automatique des captures
Comparer les PNG de `CIVBUILDER_CAPTURE` avant et après un changement (utile pour la refonte du rendu 3D, tâche 9).

### [ ] 25. Sons
Ambiance (vent, mer, pluie selon la météo) et sons des unités, des villes, des actions. Réactiver l'audio de Bevy.

### [ ] 26. Palettes lisibles pour les daltoniens
Mesurer l'impact visuel pour les joueurs non daltoniens : palettes communes si l'impact est
faible, sinon un mode pour les daltoniens.

### [ ] 27. Atlas géographique (idée de Benjamin, 09/11)
Atlas des territoires connus du joueur, avec images et icônes, qui va avec la carte imparfaite (23) :
- **Plan** : monde, continents et océans, régions naturelles, puis fiches de lieux (fleuves, lacs,
  mers, chaînes, cols, îles, villes), d'après la géographie déjà nommée (`lands.rs`,
  `mountains.rs`, `regions.rs`, `toponymy.rs`) et l'index de recherche (`search.rs`).
- **Fiche d'un lieu** : carte de situation, vignette 3D rendue par le jeu (comme les captures),
  chiffres réels (débit, longueur, altitude, surface), climat (diagramme des températures et des
  pluies par mois), origine du nom.
- **Records du joueur** : plus grand fleuve connu, plus haut sommet, plus grand lac…
- **Relations** : fleuve qui alimente une ville en eau, axe commercial entre deux villes (après
  18), col qui relie deux régions, risques connus (21).
- **Histoire** : découvert au tour N par telle unité, ville fondée, événements (crues, éruptions).
- Seulement ce que le joueur connaît ; liens vers la carte (vol de la caméra, comme la recherche).
  Les informations secondaires vont dans l'atlas plutôt que sur la carte.

### [ ] 28. Villes : ressources, indicateurs et répartition de la population
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
Révision du 10/11 après l'analyse critique : Santé et Bonheur fusionnés en Bien-être, Éducation
modificateur de la Recherche, Eau affichée quand elle limite, 2 effets directs au plus par grandeur ;
**le joueur choisit les aménagements**, l'intendant exploite ; « Or » → « Économie », frein =
capacité de gouverner ; territoire par la culture et l'achat en Économie ; nourriture lissée ;
sous-cases = un centre et six secteurs.

### [ ] 29. Armée : armes, équipement, formation, expérience
Voir `conception/conception-combat.md` (« Armée »). Soldats formés (pas produits) dans la ville
ou les casernes avec une formation progressive (accélérée par les casernes, recrutement possible
avant la fin avec des malus), armes comme ressource stratégique (production seule ; vendues,
capturées), taux d'équipement par type, attrition, niveaux d'expérience, effectif selon le type,
entretien en nourriture, économie et matériel, soldats comptés dans la population de leur ville
(pertes = population perdue). Après la tâche 28. Révision du 10/11 : **version réduite d'abord**
(habitants, armes en stock, entretien, formation fixe par type) ; armes produites en continu par
les bâtiments d'armement ; colon dans la file (coûte des habitants), éclaireur = militaire peu coûteux.

### [ ] 30. Échelle de temps, progression et but (décidé le 10/11)
Voir `conception/conception-echelle-et-but.md`. Néolithique → Moyen Âge pour la première phase,
≈ 500 tours, temps de jeu abstrait (taux au tour, année historique selon l'ère), 4 à 8 cases par
tour, 4 ères passées par jalons, population en paliers croissants (habitants ↔ indice).
Ordre (révision du 10/11) : d'abord recaler les règles actuelles (croissance, coûts, soins, PM
doublés et coûts du relief doublés, rayons de l'IA hostile et de l'exploration), avant 28 et 29 ;
ensuite l'arbre de recherche (34) et les jalons, après 28, 32 et 15. Unités réparties par
ère, améliorées en changeant d'ère (effectif qui grandit). Mode 2 tours par mois : 500 tours et mêmes taux (décidé le 11/11).
Fin de partie à décider avec les IA (31) ; cas d'un peuple qui atteint le Moyen Âge avant le tour 500.
Fin de partie décidée le 10/11 : victoire au meilleur score d'héritage (piliers non pondérés,
dont le rayonnement culturel ; ambitions par ère qui l'augmentent), fin au bout de 500 tours, hégémonie anticipée à 80 % de la
population mondiale, partie libre ensuite.

### [ ] 31. IA des peuples rivaux et faction hostile (décidé le 10/11)
Voir `conception/conception-ia.md`. Nombre de peuples réglable (≈ 8, voire 12), pas de vision à
travers le brouillard, départ nomade, personnalité née de la géographie et de la langue, diplomatie
simple avec une interface intuitive ; idéalement sans triche (niveaux par leviers d'habileté, bonus
économique affiché seulement en Expert, validé) ; un clan par camp barbare
(clan → bande → unité, validé). Premier pas : `AiView` à brouillard (tableaux de bits) et
générateur initialisé par la graine, d'abord pour la faction hostile. Mesure : `bilan_ia`.

### [ ] 32. Culture, institutions et capacité de gouverner (décidé le 10/11)
Voir `conception/conception-villes.md` (« Culture et politique », « Révision 2 »). Culture =
ressource de départ, deux effets directs (influence et territoire) ; institutions sur plusieurs axes
débloquées par des conditions et adoptées avec l'influence ; capacité de gouverner = frein principal
à l'expansion (instabilité au-delà). Territoire par la culture et l'achat en Économie. Avant le
territoire (28) et les jalons (30).
Doc dédiée : `conception/conception-societe.md` (10/11). Trois couches : valeurs (dérive selon le
vécu, léger coup de pouce des institutions), institutions et organisation politique (régime,
administration, capacité de gouverner), religion (système complet, proche de la culture). La
culture fait partie de la démographie : groupes par culture et religion dans chaque ville, avec
leur relation à l'autorité, assimilation, conversion, diasporas, villes à dominante étrangère.
Statuts d'autonomie par degrés jusqu'à la ville fantoche ; lois et découvertes sociétales payées en
Culture. Ordres (guerriers, prêtres, marchands, artisans, paysans) dont la richesse produite
renforce le pouvoir ; pas de bonus propre à un pays ; nom du pays local au départ, qui peut
évoluer au fil des âges dans la continuité (ou être gardé), cultures qui évoluent, cultures mixtes
dans les villes partagées (plus proches de celle du dirigeant), ordres qui se transforment ;
dirigeants personnages qui vieillissent sur une horloge propre et se succèdent ; gouvernement et assemblée nommés selon le
régime, bonus partagés entre dirigeant, gouvernement et assemblée selon la centralisation.

### [ ] 33. Mettre le code en accord avec la conception (relevé le 10/11)
Écarts relevés par la relecture de cohérence, à faire avec les tâches concernées :
- ~~« Or » → « Économie »~~ (fait le 11/11 : `Yields.economie`, `CAMP_BUTIN_ECONOMIE`,
  `VillageEvent::Trade { economie }`, interface, journal, wiki, README, pilote `economie=` ;
  trésor par peuple). Le mot « or » reste libre pour le métal (tâche 15).
- ~~Colon : 2 habitants demandés, 1 retiré~~ (fait le 11/11 : 1 000 habitants, depuis 2 000).
- Soldats encore produits dans la file (`Production::Unit`, milice par défaut) → formés à part (29).
- ~~Éclaireur `Civilian` → unité militaire peu coûteuse~~ (fait le 11/11 : famille
  `Reconnaissance`, militaire, sans zone de contrôle).
- `territory_radius` (villages qui rejoignent une ville, soins, ponts) → nouvelle règle de territoire.
- ~~Village qui rejoint une ville : « +1 habitant » → en habitants~~ (fait le 11/11 : +1 000).
- Restent, avec leurs tâches : soldats formés à part (29), règle de territoire (28, 32), quartiers (28).
- Stades des villes (`conception-structures.md`, `settlements.rs`) → quartiers sur sous-cases et étalement urbain.

### [ ] 34. Recherche et technologies (décidé le 10/11)
Voir `conception/conception-recherche.md`. Arbre avec prérequis, 10 technologies de base par ère
(peu chères, petit avantage chacune), spécialisations facultatives (bonus sur l'ère, optimisées en
appliquant la base), conditions de pré-acquisition, légère pénalité de taille d'empire, transfert
de technologie (diffusion lente et cumulée par les voisins, selon le retard ; commerce qui accélère
fortement, sans instantané), recherche orientée par le joueur, coût calé sur ≈ 125 tours par ère,
interface lisible. Fait partie de la tâche 30 (arbre et jalons) ; liste des 40 technologies à écrire.

### [ ] 35. Charte graphique et refonte des écrans (décidé le 10/11)
Voir `conception/conception-interface.md` (« Charte graphique, nouveaux écrans et nouvelles
cartes »). Interfaces belles, intuitives, fluides, avec une charte commune (module de style,
composants partagés, une couleur et une icône par grandeur). D'abord la **refonte de l'écran de
la ville**, trop compliqué aujourd'hui (maquette à valider). Puis les écrans nationaux (économie,
commerce, démographie, technologie, organisation politique, armée, diplomatie), chacun livré avec
son système. Maquette interactive de l'écran de la ville et de la construction faite le 11/11
(`conception-interface.md`, « Maquette interactive ») ; la vraie carte 3D dans l'écran est reportée.
**Maquette retenue le 11/11 (version 18)** : `docs/conception/maquettes/ecran-ville.html`, point de
départ du développement de l'écran de la ville.
Découpage en 7 lots (`conception-interface.md`, « Développement de l'écran de la ville ») ; **lot 1 fait
le 11/11** (écran plein écran, charte, carte des cases, deniers). Suite : lot 2 (rendements, historique).
Maquette v19 après deux revues par agents (11/11) : pile avec vue d'ensemble à la racine, objectif de
nourriture et priorités, graphique passé et prévision, mode placement. **Monnaie : le denier** (option :
nom tiré de la langue) : renommer « Économie » en deniers dans le code, l'interface et le wiki.

### [ ] 36. Nouvelles cartes thématiques (décidé le 10/11)
Cartes en deux familles (Nature, Peuples). Nouvelles : géologie, institutions politiques,
avancée technologique, commerce ; carte des ressources reprise (tous les gisements). Chacune avec
son système (tâches 14, 15, 18, 32, 34).

### [ ] 37. Équilibrage final (décidé le 10/11)
Une fois les systèmes en place : régler les chiffres (croissance, coûts, recherche, commerce,
combat, IA) avec les parties sans joueur (`bilan_ia`, `conception-ia.md`).

### [ ] 38. Diplomatie, guerre et paix (décidé le 10/11)
Voir `conception/conception-diplomatie.md`. Quatre indicateurs de pays à pays : relation
(volatile), confiance (actes passés), expansion agressive, griefs (légitiment une riposte sans
expansion agressive) ; reconquête presque sans expansion agressive. Casus belli
(invasion, reconquête, insulte, conflit frontalier…), buts de guerre, score de guerre (pertes,
occupation, pillages, usure, idéologie, bonus du but atteint), traités de paix (argent, ressources,
villes, rentes, interdictions, revendications abandonnées). Accords commerciaux, de recherche et
militaires (garantie, non-agression, alliance), limités pour qu'un pays fort ne garantisse pas tout
le monde. Coalitions défensives, moins punitives qu'Europa Universalis ; barbares hors de la
diplomatie. Enrichissements retenus : demandes des peuples, échange de cartes, défense commune
contre un clan, affinités, soutien à la guerre (fusionné avec l'usure, Bien-être), frontières par
traité, cadeaux, mariages et otages, États tributaires puis vassaux (guerre d'indépendance),
réfugiés, réputation globale, messagers (nouvelles retardées ou perdues), histoire commune (avec
l'atlas, 27) ; médiation à voir. Pas de limite au nombre de guerres simultanées. Écran de la
guerre (score et sa valeur visée, pertes par cause, forces estimées, soutien), batailles sur
plusieurs tours nommées d'après la ville ou la zone, historique des guerres, niveau
d'information sur chaque pays (commerce, frontières, espionnage, découverte, proximité ; baisse
avec le temps). Effort de guerre : privations mieux tolérées tant que le soutien tient ;
impôt de guerre payé en influence.
Espionnage et ambassadeurs : intentions, influence, actes hostiles, vol de technologie (des
points, jamais une technologie entière) ; ambassadeur qui informe, diffuse un peu les techniques
et influence sans hostilité ; réseau d'espionnage, missions avec chance et risque, contre-espionnage (bâtiments, politique,
investissement). Niveau d'information qui tend vers une valeur d'équilibre (apports continus,
perte proportionnelle), traduit en classes de connaissance. Artisans emmenés sans bonus
multiples ; pas de secret des guildes ; désinformation à voir. Avec les IA (31), par étapes.

### [ ] 39. Campagnes militaires (décidé le 10/11)
Voir `conception/conception-combat.md` (« Campagnes »). Une unité par case, sans empilement ;
déplacements de groupe : traverser les alliés, ordre de relève, sélection multiple, cases
d'arrivée réparties, chemins coopératifs avec réservation dans le temps, allure en ordre ou au plus
vite ; groupes de soldats associés à un chef, rappelés par raccourci clavier (Ctrl + chiffre). Chefs d'armée et amiraux : personnages en nombre limité, compétence selon bâtiments,
expérience militaire et investissement, sur la case d'une unité mais fragiles, expérience et bonus
de combat. Ravitaillement (lignes, fourrage, attrition, hivernage, camps), mer (cabotage puis haute
mer, tempêtes, transport, blocus, pirates). Le déplacement de groupe peut venir tôt (il sert déjà).
Fait le 11/11 (`groupes.rs`) : traverser les alliés, Maj + clic, cases d'arrivée réparties autour
de la cible, Ctrl + chiffre / chiffre / double appui. Reste : ordre de relève, rectangle de
sélection, réservation dans le temps, allures, ordres de groupe sur plusieurs tours, chefs.

### [ ] 40. Épidémies (décidé le 10/11)
Voir `conception/conception-monde-vivant.md`. Maladies du milieu et épidémies propagées par le
commerce et les armées ; les habitants et les personnages tombent malades et en meurent selon leur
santé ; immunité, quarantaine, Médecine et hygiène. Pas d'effets de l'homme sur l'environnement
pour le moment.

### [ ] 41. Monuments et merveilles (décidé le 10/11)
Voir `conception/conception-monde-vivant.md`. Monuments nationaux aux buts variés (bibliothèques,
phares, temples…), certains demandant des technologies facultatives ; le premier de chaque type
devient une merveille mondiale (bonus en plus) ; ruines qui gardent une valeur ; score d'héritage.

### [ ] 42. Nouvelle partie et bibliothèque des graines (décidé le 10/11) — écran fait le 11/11
Fait (`accueil.rs`, `config_partie.rs`) : écran de nouvelle partie au lancement, code du monde,
taille, barbares, carte cachée et aperçu en tâche de fond, configurations (enregistrer, favorites,
parties jouées, charger même monde ou nouvelle graine), menu Échap (enregistrer la configuration,
nouvelle partie). Reste : terres, climat, relief dans le générateur ; les réglages des IA, du
rythme, de l'hégémonie et des premiers tours guidés avec leurs tâches.
Voir `conception/conception-interface.md` (« Nouvelle partie, graines et prise en main »). Écran
de nouvelle partie (monde, peuples, règles, aperçu) ; code de partie copiable (graine et
réglages) ; graines déjà utilisées, favoris, notes, miniatures ; réglages de génération (terres,
climat, relief) ; tailles de carte à mesurer avec la génération ; arguments de ligne de commande
devant les variables d'environnement (gardées pour les scripts).
Décidé le 11/11 après la maquette : aperçu masqué par défaut, réglages à venir grisés, Grande
expérimentale ; configurations enregistrées (nom, commentaire, favorites, parties jouées),
rétablir par défaut, au chargement : même graine ou nouvelle. Enregistrer la configuration aussi en cours de
partie (menu Échap).

### [ ] 43. Prise en main (décidé le 10/11)
Premiers tours guidés, conseillers par domaine, notifications hiérarchisées et réglables,
systèmes dévoilés progressivement, wiki ouvert sur le sujet courant.

### [ ] 44. Budget de performance (décidé le 10/11)
Moins de 2 secondes par tour en fin de partie (12 peuples) ; temps par système dans `bilan_ia`,
alerte au-delà ; taille des sauvegardes surveillée.

### [ ] 45. Fondations techniques (relecture technique du 10/11)
Avant la sauvegarde, à faible risque : ~~générateur aléatoire tiré de la graine à la place de
`rand::rng()`~~ (fait le 10/11 : `hasard.rs`, par graine, flux, peuple et numéro de tirage ;
combats, villages et camps, faction hostile) ; ~~identifiants stables~~ (fait le 10/11 pour l'ordre de jeu :
`game::Ident` ; restent à passer sur `Ident` le journal et `Territory`, avec l'état de partie) ; ~~propriétaire sur
chaque unité et ville, trésor par peuple~~ (fait le 10/11 : `peuple.rs` ; le marqueur `Hostile` reste
pour les requêtes, à remplacer par le peuple avec les peuples rivaux) ; ~~brouillard en tableaux de bits~~
(fait le 10/11 : `cases.rs` ; un brouillard par peuple viendra avec `AiView`) ; ~~calcul de
chemin en tableaux plats~~ (fait le 10/11 : `cases::Grille`, et A* pour le chemin en tours, 8,5 → 2,3 ms
par long trajet) ; `Known` construit une fois par tour (`explore.rs`) : reporté, à mesurer avec
`bilan_ia` (une carte connue par éclaireur, qui dépend de ses cases réservées) ; ~~mesure du temps par
système (44)~~ (fait le 10/11 : `chrono_tour.rs`, console `chrono` ; à étendre à `bilan_ia`). Ensuite : état de partie (`Partie`, `jouer_tour`) hors de l'ECS, séparation du monde
généré et du monde modifié, puis la sauvegarde (13).

## Plus tard
- [x] Console de debug, étape 2 (01/11) : modifications locales (`~`) et `verifier`.
- [x] Console de debug, étape 3 (01/11) : `regenerer eau|climat`, `fleuve`, unités, villes, tours,
  `chemin` (voir `conception/conception-interface.md`).
- [x] Villes et villages par stades, muraille, matériaux du lieu, adaptation à la case (`conception/conception-structures.md`).
- [x] Camps : trois types selon la case, silhouette hérissée, nom lié au type, fumée d'alerte.
- [x] Marais : pilotis au-dessus des mares, galeries, passerelles, drainage.
- [x] Structures : ponts et tête de pont d'une ville fortifiée (décor pour l'instant).
- [ ] **Ponts** (décidé le 08/11, voir `conception/conception-ponts.md`) : étapes 1 (règles, console, dessin 2D et 3D,
  infobulle), 2 (construits par les villes, 09/11), 3 (deux niveaux sur un fleuve, 09/11) et 4 (matériaux et formes, 09/11)
  faites. Restent : l'IA hostile ne choisit pas entre le pont et l'eau ; la pierre partout plus tard (technologies, époques) ;
  un pont de cordes (qui laisse passer les charrettes) ; destruction et prise des ponts.
- [ ] **Arbres dans les rivières** (remarqué par Benjamin, 08/11) : des arbres du décor 3D sont posés dans le lit des rivières.
- [ ] Règle de la fortification (production d'un bâtiment, effet en combat) : la muraille ne se pose encore qu'à la console.
- [x] Banquise jouable (06/11) : calcul, affichage, marche, pas de ville ni de structure, pêche conservée (voir `conception/conception-fonds-marins.md`).
- [x] Toponymie étape 3 (01/11) : continents, îles, îlots, archipels, péninsules, isthmes (`lands.rs`).
- [x] Toponymie étape 4 (01/11) : chaînes, massifs, cols, plateaux, collines (`mountains.rs`).
- [x] Toponymie étape 5 (01/11) : régions naturelles et sous-régions (`regions.rs`).
- [x] Toponymie étape 6 (01/11) : barre de lieu (voir `conception/conception-toponymie.md`).
- [ ] Navigation et unités navales (barques, galères, caravelles), pêcheries.
- [ ] Police Inter Italic (les noms marins devraient être en italique).
- [ ] À évaluer (09/11) : aménagement du territoire (défricher, irriguer, terrasses, drainer, digues, canaux) avec ses conséquences (érosion des sols, salinisation). Risque : rendre le jeu trop complexe.
