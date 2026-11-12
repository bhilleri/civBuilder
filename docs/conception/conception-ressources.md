# Conception — Ressources

## Objectif
Donner aux lieux de la carte une valeur propre : des ressources placées par la géologie et le
climat (tâche 15), qui rendent possibles certaines unités, rapportent un revenu et ont toujours un
usage civil. Fait suite à la discussion sur les unités par ère (`conception-recherche.md`).

## Décisions de Benjamin (10/11/2026)
- **Les unités dépendent des ressources dont dispose le peuple**, pas seulement de l'ère :
  fer, bronze, bois, montures (cheval, dromadaire, éléphant), etc.
- **Unités régionales** : chevaux, dromadaires et éléphants se trouvent dans certaines zones.
  **Posséder la ressource rapporte un revenu**, et les unités la **consomment pour
  s'approvisionner**, à la manière du matériel militaire (`conception-combat.md`, « Armée »).
- **Chaque ressource militaire a aussi un usage civil**, pour qu'elle garde sa valeur quand une
  autre la remplace : quand le fer remplace le bronze pour les armes, le bronze reste utile à
  l'économie. On évite ainsi la ressource devenue inutile, comme dans Civilization.
- **Des ressources sans usage militaire** (or, pierres précieuses…) ont un intérêt pour la
  Recherche, l'hygiène, la Production, etc.
- Points à voir : le bronze, qui est un alliage ; le bois, la forêt étant partout.

## Proposition (à valider)
### Fonctionnement commun
1. **Gisement ou zone** sur la carte, placé par la géologie (tâche 15) ou le climat et les biomes.
2. **Exploitation** par un aménagement choisi par le joueur (mine, carrière, haras, pâturage,
   coupe) dans le territoire d'une ville ; elle emploie des habitants, comme les bâtiments
   d'armement.
3. **Flux par tour** versé dans le **stock de la ville** qui exploite (voir « Marché entre les
   villes ») ; le pays en réquisitionne une part pour former les unités (équipement, montures) et
   les entretenir (remplacer les pertes, nourrir les bêtes).
4. **Revenu** : chaque ressource exploitée rapporte de l'Économie (vente, commerce) ; le
   surplus se vend aux voisins. L'import par le commerce donne accès aux unités sans posséder
   le gisement, au prix d'une dépendance (la route peut être coupée).
5. **Unité disponible** = technologie connue + ressource dans le stock ou en flux. Sans la
   ressource, l'unité ne peut être formée ; une unité qui en manque s'use (attrition,
   montures non remplacées).
6. **Gisements finis ou non** : pour l'instant, pas d'épuisement (même choix que la fertilité,
   `conception-volcans.md`).

### Ressources militaires (avec usage civil)
| Ressource | Où (génération) | Usage militaire | Usage civil (garde sa valeur) |
|---|---|---|---|
| Cuivre | subductions, roches volcaniques anciennes | armes du Néolithique final, base du bronze | outils, ustensiles, Économie |
| Étain | granites, alluvions ; rare | avec le cuivre, fait le bronze | commerce lointain (très recherché) |
| Bronze (produit) | atelier de bronzier : cuivre + étain | armes et armures de l'âge du bronze | après le fer : cloches, statues, monnaie, instruments → Culture et Économie |
| Fer | vieux bassins, fer des marais | armes et armures de l'âge du fer | outils agricoles (charrue lourde) → Nourriture, Production |
| Bois d'œuvre | vieilles forêts de grands arbres (chênes, cèdres, conifères), pas toute forêt | navires, engins de siège | charpente, grands bâtiments → Production |
| Chevaux | steppes et prairies tempérées | chars, cavalerie | trait, transport, poste (avec le collier d'épaule) |
| Dromadaires | déserts chauds | chameliers | caravanes : commerce à travers le désert |
| Éléphants | savanes et forêts tropicales | éléphants de guerre | travaux, prestige ; ivoire → Culture |
| Obsidienne | volcans (tâche 11) | armes tranchantes du Néolithique | commerce, outils fins |

- **Le bronze est un produit**, pas un gisement : il faut du cuivre et de l'étain. L'étain étant
  rare, l'âge du bronze crée naturellement du commerce lointain, comme dans l'histoire.
- **Le bois** : toute forêt donne du bois ordinaire par le rendement de la case (sans stock).
  Seules les vieilles forêts de grands arbres donnent du **bois d'œuvre**, utile aux navires et
  aux engins ; la ressource se raréfie là où l'on défriche (à voir avec la tâche 12).
- **Les montures** sont fixées à des zones (pâturages, haras) pour ne pas dépendre des
  déplacements des animaux sauvages (tâche 22). Les autres animaux domestiques restent dans la
  tâche 16 (variété de la nourriture seulement).

### Ressources sans usage militaire
| Ressource | Où | Intérêt |
|---|---|---|
| Or, argent | subductions, filons, or alluvial | Économie (monnaie, trésor), Culture (prestige) |
| Pierres précieuses, ambre | massifs anciens ; ambre des côtes froides | Bien-être (luxe), Culture, commerce |
| Sel | bassins fermés, marais salants, côtes arides | hygiène, conservation des aliments, commerce |
| Plantes médicinales | forêts, montagnes | hygiène (épidémies) |
| Encens et épices | climats chauds et secs ou tropicaux | Bien-être, commerce lointain |
| Teintures (pourpre, indigo) | côtes, climats chauds | Économie, Culture |
| Papyrus, roseaux | deltas et marais chauds | Recherche (écrits) |
| Pierre de taille | partout, mais belle pierre rare | Production des grands bâtiments (décision des volcans : pas une ressource commune) |
| Argile fine | plaines alluviales | Production (poterie), Culture |

## Révision du 12/11/2026 : familles, nourriture, stocks (étape 5)
Après l'avis de trois agents (pédologue, historien des techniques, game designer ; synthèse plus
bas), Benjamin a donné ces directions. Ce qui est marqué « proposé » reste à valider.

### Familles de ressources (Benjamin ; en cours)
Une ressource a une famille principale et au plus une seconde (le bois d'œuvre est matériau et
stratégique).
- **Stratégiques** : armes, montures… **avec reconversion économique** quand l'armée n'en a plus
  besoin (le bronze après le fer : statues, monnaie).
- **Besoins de base** : à préciser (proposé : les textiles, lin et laine, à partir du Bronze).
- **Luxe** : produits de luxe → bien-être (+4 par sorte, attente qui grandit avec la ville, 4c).
- **Matériaux** (compatibles avec stratégique) : pierre de taille, argile, chaux, bois d'œuvre.
- **Santé, Culture, Savoir** : peut-être regroupés en une famille (nom proposé : « Développement » ;
  ressources consommées par les bâtiments et les institutions, l'infobulle dit sur quoi elles
  agissent : salubrité, culture, recherche).
- **Nourriture** : par type et par diversité (ci-dessous).

### Nourriture par type et par diversité (Benjamin)
- La nourriture est rangée **par type** ; **avoir les différents types donne un bonus de
  croissance** (nourriture équilibrée). La ville cherche d'abord à obtenir chaque type.
- **Diversité au sein d'un même type** (plusieurs sortes : blé et orge, bœuf et chèvre…) :
  **bien-être**. La ville cherche d'abord la première sorte de chaque type, puis la seconde.
- **L'intendant tient compte de la diversité.** Il priorise : 1) **nourrir tout le monde**
  (sinon la famine) ; 2) **une nourriture équilibrée** (croissance) ; 3) **la diversité des
  apports** (bien-être). Il peut passer par le **marché** pour équilibrer.
- **Richesse interne de la ville** : elle lui permet d'acheter et de vendre au marché.
- Proposé : trois types, **féculents** (céréales, tubercules ; champs, cueillette de graines),
  **protéines** (viande, laitages, poisson ; pâturages, chasse, pêche), **fruits et légumes**
  (cueillette, vergers, jardins) ; un type compte s'il couvre au moins 15 % du besoin ; croissance
  naturelle × (1 + 0,1 par type au-delà du premier) ; diversité : +2 de bien-être par sorte au-delà
  de la première de chaque type, au plus +8 (distinct du luxe).

### Stocks et conservation (Benjamin)
- Une ville a un **stock de nourriture**, utilisé **en cas de famine ou de siège**, disponible avec
  le **grenier** et d'autres bâtiments de stockage.
- La ville **cherche à remplir ses stocks** et **préfère stocker les nourritures qui se gardent
  longtemps**.
- La viande et le poisson se dégradent vite : **perte d'un pourcentage du stock à chaque tour**
  (sans calcul compliqué) ; **le sel limite la perte**. Même chose pour le **commerce** : une perte
  selon la distance, compensée par le sel.
- Proposé : perte par tour, grains 2 %, fruits et légumes 10 %, viande et poisson 20 % (5 % salés) ;
  1 de sel conserve 4 de viande ou de poisson ; le stock se remplit de l'excédent au-delà de ce que
  la croissance utilise (1,5 × le besoin ; l'objectif « Excédent » de l'intendant sert à remplir les
  stocks) ; capacité donnée par le grenier.

### Analyses des agents (12/11/2026, à valider)
- **Fertilité (pédologue)** : le sol dépend de la roche **et** du climat (un basalte tropical très
  arrosé donne un sol lessivé) ; les bassins sédimentaires agissent par leurs roches (marnes et
  limons fertiles, craie à nu et sables pauvres) ; la fertilité touche surtout les **champs**, les
  pâturages d'un tiers de l'écart (l'herbe suit la pluie ; ils occupent les sols qu'on ne labourait
  pas), ni la cueillette ni le bois. Sept classes : alluviaux × 1,4 ; volcaniques jeunes × 1,3 ;
  noirs et lœss × 1,3 ; bruns × 1,0 ; lourds × 0,85 (× 1,15 avec la Charrue lourde) ; pauvres acides
  × 0,7 ; lessivés tropicaux × 0,65. Seulement sur la nourriture que le champ tire du terrain.
- **Chronologie (historien)** : Néolithique : silex, obsidienne, ocre, argile, chaux, sel, jade,
  lin, peaux et fourrures, coquillages, plantes médicinales (cuivre natif et or en parure à la fin) ;
  Bronze : cuivre, étain, or, argent, plomb, bitume, natron, sel gemme, lapis-lazuli, vigne,
  olivier, chevaux, ambre, pourpre, cèdre ; Fer et Antiquité : fer, marbre, encens, poivre, soie,
  dromadaires, éléphants, verre ; Moyen Âge : laine, épices, sucre, alun, fourrures, poisson salé,
  papier. Placement : subductions (cuivre, or, jade, obsidienne, mercure), collisions (marbre,
  lapis), granites (étain), bassins (fer, bitume, sel, silex de la craie), déserts (natron), marais
  (fer des marais).
- **Jeu (game designer)** : classer par effet ; test du « plus » (son absence se voit et a un
  remède, et elle change où l'on s'installe ou crée un choix ; effet en une ligne) ; 20 à 25
  ressources au plus ; découverte en trois couches : indices visibles (roche de surface,
  affleurements révélés par l'exploration), savoir (une technologie montre les zones favorables),
  prospection (automatique dans le territoire, l'éclaireur au-dehors ; l'or d'un fleuve signale un
  filon en amont). Les gisements sont **à découvrir** (Benjamin).

## Marché entre les villes (décisions de Benjamin, 10/11/2026)
**Réalisation en deux temps (révision du 10/11, relecture, validé)** : d'abord les surplus envoyés
aux villes reliées selon la capacité de transport, avec un **dépôt national de l'armée** alimenté
par la réquisition (les armes vont au stock de la ville qui les produit) ; ensuite les prix par
l'offre et la demande, quand l'IA saura commercer. La conception ci-dessous reste le but.

- **Chaque ville exprime un besoin pour elle-même**, selon ses bâtiments et sa population ; elle
  **vend son surplus** aux villes reliées, **en priorité à celles du joueur**.
- **Choix des villes partenaires** selon :
  - l'appartenance au même empire (bonus) ;
  - la proximité, en temps de trajet par charrette ou par bateau ;
  - les relations entre pays : accord d'échange, ou frontières fermées ;
  - la rentabilité (gain rapporté au trajet).
- **Prix selon l'offre et la demande** du bien dans la ville et les villes proches.
- **Pas de caravanes à construire** : les routes commerciales se font seules. Des **bâtiments
  favorisent les échanges**. Le pays **alloue un budget de chevaux ou de dromadaires au
  transport** : arbitrage entre l'armée et l'économie.

### Proposition (à valider)
**Ce qui remplace le stock national** : chaque ville a son propre stock et son marché. Le pays
garde une **réquisition** pour l'armée et le transport (part des montures, du fer, du bois qu'il
achète aux villes à leur prix, en Économie du trésor).

**Ordre de servir un bien produit par une ville** :
1. son propre besoin (bâtiments, population) ;
2. la réquisition du pays (armée, transport), si le joueur l'a réglée ;
3. les villes du même empire ;
4. les villes étrangères avec qui l'échange est permis, la plus rentable d'abord.

**Ponction forcée (décision de Benjamin, 10/11/2026)** : le joueur peut ponctionner des
ressources **au détriment de sa population**, c'est-à-dire passer avant le besoin propre des
villes, **mais il doit être averti**. Proposition : le réglage de la réquisition affiche, avant de
valider, ce qui manquera et où (« Kaldra : forge à l'arrêt, sel manquant, Bien-être −2 ») ; une
alerte reste dans le journal tant que la ponction prive une ville.

**Besoins** :
- bâtiments : la forge consomme du fer, l'atelier de bronzier du cuivre et de l'étain, le chantier
  naval du bois d'œuvre, le relais des chevaux ;
- population : sel, puis biens de luxe (encens, teintures, pierres) d'autant plus que la ville
  grandit (paliers d'indice) ; ils comptent pour le Bien-être ;
- un besoin non couvert a un effet visible (forge à l'arrêt, Bien-être en baisse), affiché dans le
  panneau de la ville.

**Prix** : prix de base du bien × (demande ÷ offre) dans un rayon de quelques jours de trajet,
borné (de ×0,25 à ×4) et lissé d'un tour à l'autre pour éviter les à-coups.

**Choix du partenaire** : score = (prix d'achat sur place − prix chez le vendeur − coût du
trajet) ÷ durée du trajet ; × bonus du même empire (pas de douane) ; nul si les frontières sont
fermées ; réduit par une douane sans accord d'échange.

**Trajets** : chemin le plus rapide par terre (charrette, porteurs), fleuve ou côte, avec le coût
des cases, le relief, les saisons (boue, neige, crues, `conception-saisons.md`). Les durées entre
villes se calculent rarement (tous les quelques tours, ou quand une route ou un pont change) et se
gardent en cache : peu coûteux même avec des centaines de villes.

**Capacité de transport** : chaque lien a une capacité :
- à pied (porteurs) : faible, toujours disponible (Néolithique) ;
- montures et charrettes : selon le budget de chevaux et de dromadaires alloué au transport
  (dromadaires dans le désert, chevaux ailleurs, avec Roue et chariot et le collier d'épaule) ;
- bateaux : grande capacité sur fleuves navigables et côtes (Pirogue, Voile, Gouvernail).
Les montures allouées au transport ne servent pas à l'armée, et inversement.

**Bâtiments qui favorisent les échanges** : marché (plus de demande et d'offre visibles),
entrepôt (stock plus grand), relais et caravansérail (trajets plus rapides et sûrs), port
(fluvial puis maritime), routes et ponts (tâche 18), foires (Moyen Âge).

**Pillage des convois (décision de Benjamin, 10/11/2026)** : les convois sont **pillables par
les ennemis et les barbares**. Comme il n'y a pas de caravane à construire, le convoi n'est pas
une unité : c'est le flux qui suit sa route. Proposition :
- une unité ennemie ou barbare **sur la route ou à côté** intercepte une part du flux qui passe
  (proportionnelle à sa force), qu'elle prend comme butin ; les montures du transport peuvent
  être perdues ;
- une route menacée devient **moins rentable** : les villes choisissent un autre partenaire ou un
  autre chemin, et le commerce se détourne de lui-même ;
- **protection** : unités en escorte ou en garnison le long de la route, relais fortifiés,
  routes surveillées ; zone de contrôle des unités amies ;
- les barbares voient les routes comme des proies (`conception-ia.md`, cartes des proies) ;
- l'écran et la carte du commerce montrent les convois attaqués et les pertes.

**Revenus** : le vendeur gagne de l'Économie ; le pays prélève une douane sur le commerce
étranger. Les flux comptent pour la **diffusion des technologies** (`conception-recherche.md`) et
pour les relations entre pays (`conception-ia.md`).

**Interface** : carte thématique des flux (épaisseur selon le volume), panneau du marché dans la
ville (besoins, stock, prix, d'où vient et où va chaque bien), réglage national de la réquisition
et du budget de transport.

### Lien avec la recherche
Les ressources donnent les **conditions de pré-acquisition** (`conception-recherche.md`) :
cuivre accessible pour la métallurgie du cuivre, étain pour le bronze, chevaux pour
l'équitation, etc. Une technologie peut être connue sans la ressource ; c'est la formation des
unités qui la demande.

### Interface
- Ressources visibles dans l'infobulle de la case et sur une carte thématique (pas sur la carte
  principale, pour ne pas la surcharger).
- Panneau du peuple : stock, flux, consommation par l'armée, revenu, importations.
- Liste des unités : celles qui manquent de ressource sont grisées, avec la ressource à obtenir.

## Questions ouvertes
- Valider la liste des ressources et leurs usages civils.
- Chiffres : flux par exploitation, besoin par unité, revenu, prix du commerce.
- Commerce entre peuples : avec les IA (`conception-ia.md`).
- Chiffres : bornes des prix, douane, capacité par monture, rayon du marché.
