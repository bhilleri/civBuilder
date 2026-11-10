# CivBuilder — prochaines tâches

Décidées avec Benjamin (depuis fin octobre 2026). Ordre conseillé pour les tâches 9 à 32 : voir « Propositions retenues le 09/11 ».

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
Ordre conseillé (revu le 10/11 après l'analyse critique) : 13 (sauvegarde) au plus tôt, puis 30
(échelle et but) ; ensuite 14 → 12 → 15 (chacune prépare la suivante) ; les autres
avec le sujet voisin (19 avec la navigation, 21 avec la météo). Chaque tâche : conception à
proposer et à valider avec Benjamin avant de coder.

### [ ] 13. Sauvegarde et chargement
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
ère, améliorées en changeant d'ère (effectif qui grandit). Mode 2 tours par mois : tout réglé en mois.
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

### [ ] 34. Recherche et technologies (décidé le 10/11)
Voir `conception/conception-recherche.md`. Arbre avec prérequis, 10 technologies de base par ère
(peu chères, petit avantage chacune), spécialisations facultatives (bonus sur l'ère, optimisées en
appliquant la base), conditions de pré-acquisition, légère pénalité de taille d'empire, transfert
de technologie (diffusion lente et cumulée par les voisins, selon le retard ; commerce qui accélère
fortement, sans instantané), recherche orientée par le joueur, coût calé sur ≈ 125 tours par ère,
interface lisible. Fait partie de la tâche 30 (arbre et jalons) ; liste des 40 technologies à écrire.

### [ ] 33. Mettre le code en accord avec la conception (relevé le 10/11)
Écarts relevés par la relecture de cohérence, à faire avec les tâches concernées :
- « Or » → « Économie » (`Yields.gold`, `state.gold`, `CAMP_CAPTURE_GOLD`, `VillageEvent::Trade`,
  README, wiki) ; attention au mot « or » du métal (tâche 15). Trésor national.
- Colon : 2 habitants demandés, 1 retiré (`rules.rs`) → habitants selon la table de population.
- Soldats encore produits dans la file (`Production::Unit`, milice par défaut) → formés à part (29).
- Éclaireur `Civilian` (`units.rs`) → unité militaire peu coûteuse.
- `territory_radius` (villages qui rejoignent une ville, soins, ponts) → nouvelle règle de territoire.
- Village qui rejoint une ville : « +1 habitant » → en habitants.
- Stades des villes (`conception-structures.md`, `settlements.rs`) → quartiers sur sous-cases et étalement urbain.

### [ ] 35. Charte graphique et refonte des écrans (décidé le 10/11)
Voir `conception/conception-interface.md` (« Charte graphique, nouveaux écrans et nouvelles
cartes »). Interfaces belles, intuitives, fluides, avec une charte commune (module de style,
composants partagés, une couleur et une icône par grandeur). D'abord la **refonte de l'écran de
la ville**, trop compliqué aujourd'hui (maquette à valider). Puis les écrans nationaux (économie,
commerce, démographie, technologie, organisation politique, armée, diplomatie), chacun livré avec
son système.

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
