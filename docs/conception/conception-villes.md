# Conception — Villes : ressources, indicateurs, répartition de la population

## Objectif
Définir les ressources d'une ville et la façon dont sa population se répartit sur ses cases :
selon les rendements les plus avantageux, le besoin d'autosuffisance alimentaire et financière,
et ce que la ville doit produire. Le système doit rester **compréhensible** par le joueur.

## Ce qui existe (`rules.rs`)
- Rendements par case : nourriture, production, or (`Yields`).
- Répartition fixe : chaque habitant prend la meilleure case selon un score figé (4 × nourriture
  + 3 × production + 2 × or, `choose_worked_tiles`), sans tenir compte des besoins de la ville.
- Consommation de 2 nourriture par habitant ; croissance au-delà d'un seuil ; la famine fait perdre un habitant.
- L'or est compté mais ne sert à rien (pas d'entretien).

## Décisions de Benjamin (09/11/2026)
### Ressources et indicateurs
| Famille | Élément | Rôle |
|---|---|---|
| Centre | **Population** | Nombre réel d'habitants, converti en un indice de taille affiché (et utilisé par les règles) |
| Principale | **Nourriture** | Produite par les cases et le commerce |
| Principale | **Production** | **Construire** (bâtiments, unités, aménagements). Vient surtout des cases |
| Principale | **Économie** | **Entretenir** les bâtiments ; acheter des ressources ; diplomatie ; acheter ou finir une unité ou un bâtiment. Vient des cases et du commerce |
| Principale | **Recherche** | Points de recherche, qui dépendent de l'éducation |
| Capacité | **Eau** | Vient des sources en eau de la ville, difficile à transporter ; il faut des aménagements pour irriguer les cases cultivées qui manquent d'eau |
| Secondaire | **Santé** | Le corps : eau potable, densité, climat et maladies, famine, bâtiments de soin, éducation. Baisse quand la population augmente. Agit sur la mortalité, l'attractivité et la productivité |
| Secondaire | **Bonheur** | La satisfaction : variété de la nourriture, impôts, guerre et sécurité, emploi, culture et croyances |
| Secondaire | **Éducation** | Une sorte de taux d'alphabétisation ; améliore la santé et la recherche |
| Calculé | **Attractivité** | Pas gérée par le joueur : calculée d'après la santé, le bonheur, l'emploi (économie) et la nourriture, avec son explication (« attirante : bonne santé, travail ; freinée par le manque d'eau »). Oriente les migrations entre villes et depuis l'extérieur |

- Les **principales** sont toujours visibles (bannière de la ville, barre du haut). Les
  **secondaires** sont dans le panneau de la ville et remontent en alerte quand elles posent problème.
- L'**Eau** est une capacité, non stockable : elle borne la population et les cultures irriguées.
  Elle vient de la simulation (débit des cours d'eau, lacs, pluie, nappes) et varie avec les
  saisons (étiage, `conception-saisons.md`).

### Enchaînements
- Eau → irrigation → Nourriture → croissance → Population.
- Eau potable + densité de Population → Santé → mortalité et productivité.
- Éducation → Santé et Recherche.
- Santé + Bonheur + Économie + Nourriture → Attractivité → migrations → Population.

## Aménagement des cases et bâtiments (décidé le 10/11/2026, proposition de Benjamin)
- **Rayon de départ** : une ville commence avec un rayon d'une case et une population à répartir.
- **La population se répartit sur plusieurs cases** selon les besoins, et non un habitant par case.
- **Capacité d'une case** : nombre maximal de personnes affectables, qui dépend des aménagements présents.
- **Sans aménagement**, la case est exploitée de façon sous-optimale (fermes mal organisées, chasse).
  **Aménager dope fortement les rendements.** (Révision : le joueur choisit les aménagements, voir plus bas.)
- **Bâtiments**, en plus des aménagements, aux fonctions variées : production d'armes,
  entraînement des soldats, production de ressources, stockage de l'eau, éducation, culture,
  défense… Un bâtiment construit sur une case **ne détruit pas l'aménagement rural** : il réduit
  le nombre maximal de personnes aux champs en créant ses propres emplois. Exemple : le moulin
  réduit le nombre de personnes aux champs, mais augmente la production de nourriture.
- **Seuils et paliers pour les bâtiments** : sous un effectif minimal, pas de bénéfice ; des
  paliers augmentent le bonus. L'intendant les pourvoit par blocs (le palier entier ou rien).
- **Rendements décroissants pour les aménagements ruraux** : chaque travailleur de plus rapporte un
  peu moins, jusqu'à la capacité.
- **Bâtiments = multiplicateurs avec des emplois**, sans chaînes de marchandises au début (le
  moulin donne +X % à la nourriture des champs, il ne transforme pas du blé en farine).
- **Plusieurs bâtiments par case**, certains sur plusieurs sous-cases. Au-delà d'un certain nombre
  de bâtiments, **la case passe de rurale à urbaine**. Entre les deux, son rendu évolue pour
  montrer la transformation ; deux cases urbaines voisines montrent l'**étalement urbain**.
- **Distance au centre** : une petite pénalité selon la distance, qui pousse à centraliser les
  bâtiments et fait apparaître la ceinture de cultures de von Thünen (maraîchage et lait près de
  la ville, céréales plus loin, bois et pâturages au bout).
- **Participation d'une ville à l'effort d'une autre** (décidé le 10/11/2026) : par **envoi de
  production** (ou de nourriture), avec une perte selon la distance et les routes. Pas de cases
  partagées entre villes.

### Population active et économie (décidé le 10/11/2026)
- **Population active variable** : environ 40 à 60 % de la population selon la santé et
  l'éducation ; la productivité par travailleur dépend aussi de la santé et de l'éducation.
- **Origine de l'Économie**, quatre sources : certaines **cases** (fleuve, côte, mines), un
  **impôt par habitant** (réglable, avec un effet sur le bonheur), la **vente des surplus** de
  nourriture et de production, le **commerce** (routes entre villes par les fleuves, les côtes et
  les cols, tâche 18).
- **Une seule file de construction** (bâtiments et aménagements), avec les suggestions de
  l'intendant (« drainer ce marais : +3 places aux champs »). Les soldats n'y passent pas : ils
  sont formés (voir « Armée » dans `conception-combat.md`).

### Sous-cases et quartiers (décidé le 10/11/2026)
Benjamin craignait qu'avec des sous-cases il faille beaucoup de bâtiments, difficiles à
reconnaître. Réponse retenue :
- **7 sous-cases par case** (un centre et six autour, ≈ 15 km chacune). Une case devient urbaine
  au-delà d'environ 4 sous-cases bâties (à régler).
- **Une sous-case porte un quartier** (plusieurs bâtiments), pas un seul bâtiment : artisanal,
  marché, religieux, militaire, résidentiel ; ou un aménagement rural (champs, pâturages, bois).
- **Seuls les bâtiments marquants ont leur propre modèle reconnaissable** : temple, caserne,
  port, muraille, moulin, grand marché. Les autres sont des maisons et des ateliers génériques.
- Le détail se lit au survol et dans le panneau de la ville.

**Forme d'un quartier** :
- **Le type dominant donne le plan** :
  - artisanal : longs ateliers bas, cours, cheminées ;
  - marché : place centrale, halle, étals, rues qui convergent ;
  - religieux : enceinte, grand édifice, jardins ;
  - militaire : bâtiments alignés, cour d'exercice, palissade ;
  - résidentiel : maisons serrées, ruelles.
- **Les autres bâtiments occupent une part des parcelles proportionnelle à leur nombre** : le
  quartier évolue à chaque construction, sans changement brutal.
- **Les bâtiments marquants sont placés logiquement** : centre du quartier, rive pour le port et le moulin.
- **Construction** : tracé de rues qui suit le relief (le long des courbes de niveau sur une
  pente) et se raccorde aux sous-cases voisines (étalement urbain continu), parcelles le long des
  rues, un bâtiment par parcelle tiré de la palette du type, avec un peu de variation.
  **Densité selon la population** : hameau, bourg, ville. Tirage déterministe (graine et ville).
- **Caractère régional** :
  - matériaux selon le lieu : pierre, brique, bois ou torchis selon la géologie (tâche 14) et la forêt ;
  - toits selon le climat : forte pente sous la neige, plats en climat aride, larges débords sous les tropiques ;
  - plus tard, la culture (cours intérieures, damier ou rues tortueuses).
- **Règles des structures respectées** : bâtiments posés sur le relief sans remblai ni terrasse, pas de superposition.
- **Rendu** : une vingtaine de modèles par style, dessinés en instanciation. De loin, une tache
  de toits à la couleur des matériaux, avec seulement les bâtiments marquants en relief.

### Population de l'aire, urbains et ruraux, faubourgs (11/11/2026)
**Décisions de Benjamin** : la population d'une ville est celle de **toute son aire d'influence**
(la ville et ses campagnes) ; une **statistique urbains / ruraux** en montre la répartition ; des
**faubourgs nommés** (espaces urbanisés détachés de la ville) peuvent apparaître.

**Proposition** :
- **Urbains et ruraux : pas de règle nouvelle** (Benjamin, 11/11/2026). La répartition se lit dans
  ce qui existe : la population est **affectée aux cases** de l'aire, et une case est **urbaine ou
  rurale** selon la règle déjà décidée (« la case passe de rurale à urbaine », sous-cases et
  quartiers). Population urbaine = habitants des cases urbaines ; rurale = le reste. Le panneau de la
  ville l'affiche (« 48 000 habitants · 12 % en ville »).
- **Les habitants vivent là où ils travaillent, pour l'instant** (Benjamin, 11/11/2026) : les
  habitants d'une case se répartissent comme ses emplois (travailleurs de la case et emplois de ses
  bâtiments ; un actif pour deux habitants) ; les sans-emploi vivent au centre. Le **taux
  d'urbanisation** en découle (part des habitants des cases urbaines) ; l'écran de la ville l'affiche
  sous ce nom, et une carte de l'onglet Population montre la répartition des habitants (l'autre, les
  emplois). Un logement distinct de l'emploi (quartiers d'habitation, trajets) pourra venir plus tard.
- **Faubourg** : une **case urbaine détachée** de la ville (non contiguë à ses cases urbaines), là où
  la règle des cases urbaines la fait naître, souvent près d'un atout (gué ou pont, port, carrefour,
  mine, sanctuaire). Il est **nommé** dans la langue de la ville (toponymie : « Kaldra-sur-Vesse »,
  « Port-de-Kaldra », ou un nom propre), visible sur la carte à son niveau de zoom.
- **Un faubourg ne devient pas une ville à part entière** (Benjamin, 11/11/2026) : deux villes
  seraient trop proches l'une de l'autre (distance minimale entre villes).

## Intendant guidable (décidé le 10/11/2026)
L'intendant est automatique, mais **guidable par le joueur** :
- **Forcer l'exploitation d'une case** (et son effectif).
- **Orienter les rendements** vers un type en choisissant le **niveau d'agressivité** : une
  priorité par ressource, à quelques crans (normale, prioritaire, maximale).

Fonctionnement proposé :
1. **Besoins du tour** : nourriture (consommation, soldats compris, + croissance voulue, plus
   tard besoin de bras saisonnier), économie (entretien), production (ce qui est en construction).
2. **Placement des travailleurs par petits groupes** là où le gain marginal est le plus fort
   (rendements décroissants, paliers des bâtiments). Le poids de chaque ressource **grandit avec
   le manque** : tant que la ville ne se nourrit pas, la nourriture pèse lourd ; une fois couverte,
   son poids retombe et la production ou l'économie prennent le relais. L'eau décide quelles
   cases peuvent être irriguées ; la distance au centre pénalise un peu.
3. **Consignes du joueur** (priorités par ressource, cases forcées) appliquées d'abord.
4. **Chômage** : si la population active dépasse la capacité totale, le Bien-être et l'attractivité baissent.
5. **Manque impossible à combler** : la ville le signale (« Lyon manque de nourriture : 2 habitants
   sans case fertile ») ; plus tard, le commerce pourra combler.
6. **Explication** : barres besoin / production / écart et raison de chaque choix (« 3 aux champs
   pour nourrir la ville, 1 en forêt pour le bateau en cours »).
- La nourriture est lissée sur l'année (révision) ; l'intendant évite de déplacer les travailleurs d'un mois à l'autre, sauf besoin de bras saisonnier.

### Types d'aménagements
- Champ, pâturage, coupe de bois, mine, pêcherie, salines, irrigation, drainage, port fluvial, routes, digues.
- Ce qui est possible dépend de la case : relief et pente, fertilité (tâche 12), géologie (tâche 14), eau.
- Visible en 3D : champs autour de la ville, ville qui s'étend (`conception-rendu-3d.md`).
- Lisibilité à trois niveaux : la carte (aspect rural, bourg, urbain), le panneau de la ville
  (totaux, choix de l'intendant), le détail d'une case au survol (travailleurs, capacité, paliers).

## Culture et politique (pistes retenues comme base le 10/11/2026, détaillées dans `conception-societe.md`)
Benjamin imagine un **système politique que le joueur développe en parallèle de ses villes**, et
qu'il doit acquérir. Difficulté : les sociétés humaines se sont développées différemment, une
seule voie ne conviendrait pas. La culture pourrait être liée au savoir ou générer du bonheur.

Proposition :
- **Culture** : ressource secondaire produite par les lieux de culte, les monuments, les artisans
  et les fêtes. Elle donne du **bonheur** et des **points d'influence**, qui servent à adopter des
  institutions. Elle se distingue de la recherche : la recherche fait progresser les techniques,
  la culture les croyances, les arts et les institutions. Deux progressions parallèles.
- **Institutions plutôt qu'un arbre** : plusieurs axes indépendants, chacun avec des options :
  - pouvoir : chef, conseil des anciens, assemblée, royauté…
  - droit : coutume, loi écrite ;
  - économie : troc, monnaie, impôt ;
  - croyances ;
  - armée : milice, levée, armée de métier.
  Chaque option est **débloquée par des conditions** (taille de la population, éducation et
  écriture, nombre de villes, commerce, géographie) plutôt que par une suite imposée. Elle
  s'adopte avec de l'influence. Des sociétés différentes émergent ainsi selon le lieu et les choix.
- **Capacité de gouverner** (autorité ou cohésion) : nombre de villes et étendue que les
  institutions permettent de tenir ; au-delà, de l'instabilité. Elle donne une raison de
  développer la politique en même temps que l'expansion.

## Révision après l'analyse critique (décisions de Benjamin, 10/11/2026)
Ces décisions **remplacent** ce qui les contredit plus haut.
- **Indicateurs réduits au départ** :
  - **Santé et Bonheur fusionnés en « Bien-être »** ; on les séparera plus tard si le jeu le demande ;
  - **Éducation = simple modificateur de la Recherche**, pas une ressource ;
  - **Eau affichée seulement quand elle limite** ;
  - **règle : chaque grandeur agit directement sur 2 autres au plus**, avec des amortisseurs contre
    les spirales (plancher de population active, saturation de l'attractivité, aide d'urgence).
- **Aménagements : le joueur choisit quoi aménager**, l'intendant décide comment exploiter les cases
  aménagées. Pendant le choix, l'interface zoome sur la ville, met en évidence les gains et les
  pertes et les cases les plus profitables.
- **« Or » devient « Économie » partout** (code, butins des camps et villages, wiki, interface).
  **Frein principal à l'expansion : la capacité de gouverner** ; l'entretien en Économie n'est
  qu'un coût secondaire.
- **Territoire** (direction donnée par Benjamin, à préciser) : il grandit avec la **culture**,
  peut **s'acheter case par case en Économie**, et reste borné par la **capacité de gouverner**.
  Remplace la règle actuelle (rayon 1, puis 2 à partir de 3 habitants, `rules::territory_radius`).
- **Nourriture : rendements lissés.** Pas de vraie récolte : la nourriture du mois est la moyenne
  annuelle, à peine modulée par la saison et la météo (sécheresse, hiver rude). Pas de famine
  mécanique en hiver ; les greniers et la récolte saisonnière restent une idée pour plus tard.
- **Plantes et animaux (tâche 16)** : leur seul effet est la **variété de la nourriture**, qui
  améliore le Bien-être ; aucun effet sur les rendements de base.
- **Unités civiles** : le colon est produit dans la file de construction et coûte des habitants
  (il fonde une ville). L'éclaireur est une unité militaire de faible coût (voir `conception-combat.md`).

## Révision 2 : cohérence (décisions de Benjamin, 10/11/2026)
- **Culture, ressource de départ** : produite par les lieux de culte, les monuments, les artisans ;
  **deux effets directs** : l'influence (institutions, jalons) et le territoire. Le Bien-être n'en
  dépend qu'indirectement (par des bâtiments). Culture, institutions et capacité de gouverner ont
  leur tâche (32).
- **Économie = trésor national** : les villes versent leur Économie dans un trésor commun (comme
  `state.gold` aujourd'hui), qui paie l'entretien, la solde, les achats, la diplomatie et le territoire.
- **Besoin de bras saisonnier** : aux mois de semailles et de récolte (selon le climat local), les
  champs demandent plus de travailleurs ; lever des soldats à ce moment baisse les rendements de
  l'année. La nourriture reste lissée.
- **Bâtiments d'armement = emplois** comme les autres (voir `conception-combat.md`).
- **Tableau et enchaînements plus haut** : lire Santé et Bonheur comme « Bien-être », l'Éducation
  comme un modificateur de la Recherche (révision précédente). Grandeurs de départ : Population,
  Nourriture, Production, Économie (nationale), Recherche, Culture ; Eau (capacité) ; Bien-être ;
  Attractivité calculée.

## Constats techniques de l'analyse critique (10/11/2026)
- **Géométrie des sous-cases** : sept petits hexagones ne pavent pas l'hexagone parent. Prendre
  **un centre et six secteurs**, avec une fonction pure `subcell(p) -> 0..6` partagée entre Rust
  et WGSL ; écrire les invariants (rattachement à la case, voisinage entre cases pour l'étalement).
- **Quartiers** : leur construction (rues le long des courbes de niveau) est mise en cache par
  sous-case, déterministe par (graine, ville, sous-case) ; la répartition de la population coûte peu.
- **État modifiable** : aménagements et sous-cases sont de l'état du monde, à séparer du monde
  généré et à sauvegarder (tâche 13).

## Étape 4 de la feuille de route : règles et chiffres (validés par Benjamin le 11/11/2026)
Quatre livraisons : **4a** travailleurs sur les cases, **4b** aménagements choisis par le joueur,
**4c** Bien-être et hygiène, **4d** territoire par la Culture ; maquette de l'écran de la ville
(tâche 35) pendant la 4a, le panneau actuel étant seulement étendu en attendant. Chiffres de
départ, recalés par un bilan (graines 777 et 2) pour garder la croissance actuelle (1 000 → ≈ 2 400
habitants en 60 tours).

### 4a. Travailleurs sur les cases
- **Population active : 50 %** des habitants (plus tard 40 à 60 % selon le Bien-être).
- L'intendant place les travailleurs **par blocs de 100** là où le gain marginal est le plus fort ;
  le poids d'une ressource grandit avec son manque ; petite pénalité de distance au centre.
- **Capacité** de chaque case en travailleurs ; **rendements décroissants** :
  rendement = maximum × (travailleurs / capacité)^0,7 (la moitié de la capacité donne ≈ 62 %).
- Consignes du joueur : une priorité par ressource (normale, prioritaire, maximale) et des cases forcées.
- **Chômage** : les actifs au-delà de la capacité totale sont signalés (et pèseront sur le Bien-être, 4c).

| Case | Capacité | Rendement maximal | Coût |
|---|---|---|---|
| Sans aménagement (cueillette, chasse) | 500 | rendements actuels de la case | — |
| Champ | 1 500 | nourriture × 2 | 20 production |
| Pâturage | 1 000 | nourriture × 1,5, +1 production | 15 production |
| Coupe de bois | 1 000 | production × 2 | 15 production |

**Fait le 11/11/2026** (`intendant.rs`, `rules::cases_de_la_ville`, `rules::repartition_ville`).
Constats à la première partie (graine 777) :
- avec les rendements décroissants, **une petite ville se répartit sur plusieurs cases** (1 000
  habitants : 100 travailleurs sur 5 cases plutôt que 500 sur une) : elle tire 4,6 nourriture et
  6,5 production, contre ≈ 3 et 4 avant ; la production des débuts presque doublée est à
  surveiller (équilibrage, tâche 37) ;
- **croissance inchangée** tant que la nourriture ne la borne pas : 1 000 → 2 439 habitants en 60
  tours, comme avant ; 26 300 habitants au tour 220, avec 2 600 sans emploi sur 18 cases non
  aménagées (la nourriture couvre encore 1,4 fois le besoin) ;
- le dernier bloc d'une ville peut être plus petit que 100 ; les grandes villes placent des blocs
  plus gros (1/200 des actifs) pour garder un calcul léger.
Les aménagements se posent pour l'instant à la console (`amenagement`) ; leur construction et leur
choix à l'écran viennent en 4b.

### 4b. Aménagements choisis par le joueur
- **Une case peut se prêter à plusieurs aménagements : le joueur choisit lequel y faire**
  (Benjamin, 11/11/2026). Exemples : une prairie accepte un champ ou un pâturage ; une forêt, une
  coupe de bois, ou un champ ou un pâturage après défrichement (plus cher, la forêt est perdue pour
  le bois). Une case porte **un aménagement à la fois** ; en changer remplace l'ancien (nouveau
  coût). Plus tard, avec les sous-cases, une case pourra en mêler plusieurs.
- Conditions de départ : champ sur relief plat ou collines, avec assez de pluie ou une rivière
  voisine ; pâturage sur prairie, plaine, steppe, savane, broussailles, toundra, collines ; coupe
  de bois en forêt, jungle, taïga, savane arborée.
- Construits par la file unique de la ville ; pendant le choix, la vue zoome sur la ville et montre
  le gain de chaque case et de chaque aménagement possible.
- Irrigation, drainage, mines : plus tard, avec la recherche.
- **Deux sortes d'aménagements, en plus des bâtiments (Benjamin, 11/11/2026)** :
  - **aménagement de production** : l'activité de la case, **un par case** (il remplace l'ancien) :
    champ, pâturage, coupe de bois, ferme… et en ville des **quartiers de production** (quartier
    d'artisans pour les guildes et les ateliers privés, quartier marchand) ; il fixe le rendement
    de la case et sa capacité en travailleurs ;
  - **aménagement de bonus** : ne prend pas la place d'un bâtiment ni de l'activité, **plusieurs
    par case**, chacun une fois : égouts, irrigation (sur un champ), drainage, rues pavées,
    fontaines publiques, jardins publics… ; il donne un bonus à la case (hygiène, nourriture,
    deniers, bien-être) ;
  - **bâtiment** : s'ajoute sur une case et améliore aussi les cases voisines (moulin, atelier…).
  Les trois familles se distinguent partout (catalogue, carte, fiches) : bâtiment en carré violet,
  aménagement de production en rond vert (avec un motif sur la case : sillons du champ, points du
  pâturage), aménagement de bonus en losange turquoise. Pas de champs dans les quartiers.
- **File de construction et projets de ville (Benjamin, 11/11/2026)** :
  - une file unique par ville : unités, ponts, aménagements ; chaque élément garde la production
    déjà investie (perdue s'il est retiré) ; le surplus d'un élément fini passe au suivant ;
  - **une nouvelle ville commence avec une file vide** : le joueur doit indiquer quoi produire ; une
    ville ne refait plus sans fin la même unité ;
  - à la place de construire, la ville peut se lancer dans un **projet de ville**, sans fin, qui
    reçoit toute sa production tant que la file est vide :
    - **Peupler la ville** : la production s'ajoute au surplus de nourriture pour la croissance
      (2 production = 1 nourriture ; jamais pour le besoin) ;
    - **Grands jeux** : bien-être (avec la livraison 4c) ;
    - **Projet de recherche** : recherche du pays (tâche 34) ;
    - **Développement de l'art et de la culture** : culture, qui étend le territoire (4d) ;
    tant que leur système manque, ces trois projets sont montrés « pas encore dans le jeu » ;
  - le joueur est prévenu si une ville reste longtemps en projet (il l'a peut-être oubliée) :
    au 10e tour, puis tous les 10 tours (seuil proposé, à régler en jouant) ; une ville sans rien à
    produire (file vide, pas de projet) perd sa production et le journal le signale à chaque tour.

### 4c. Bien-être et hygiène (0 à 100)
- **Bien-être** (60 au départ) : + nourriture couverte (jusqu'à +10 avec un surplus), − famine
  (−20), − chômage. Deux effets : croissance naturelle (× 0,5 à × 1,5) et productivité.
- **Hygiène** : + eau douce proche (+20 pour une rivière ou un lac voisin), − entassement, −
  famine. Effet pour l'instant : la mortalité (croissance nette) ; les épidémies plus tard (tâche
  40). Affichée seulement sous 50.

### 4d. Territoire par la Culture
- Départ : la ville et ses 6 voisines.
- Culture : +1 par tour pour la ville, +2 avec un **sanctuaire** (premier bâtiment).
- Une case de plus quand le stock atteint 10 + 6 × (cases déjà gagnées) ; choisie automatiquement
  (la meilleure voisine, atouts d'abord). **Achat d'une case précise** en Économie : 20 + 10 × distance.
- Remplace `rules::territory_radius` ; la borne par la capacité de gouverner vient avec la tâche 32.

## Questions ouvertes
- Chiffres : consommation, seuils de croissance, conversion habitants → indice, rythme mensuel des rendements.
