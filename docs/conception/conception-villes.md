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
  **Changé le 11/11/2026** (maquette v19, lot 4) : la nourriture n'a plus de priorité mais un **objectif**
  (1 ; 1,25 ; 1,5 ; 2 fois le besoin) que l'intendant vise d'abord ; la production et les deniers gardent
  leur priorité ; grandeurs au même niveau = même poids.
- **Chômage** : les actifs au-delà de la capacité totale sont signalés (et pèseront sur le Bien-être, 4c).

| Case | Capacité | Rendement maximal | Coût |
|---|---|---|---|
| Sans aménagement (cueillette, chasse) | 300 (500 avant le 13/11) | rendements actuels de la case (exposant 0,7) | — |
| Champ | 3 000 (1 500) | nourriture × 7,5 (× 2) pour 3 000 ; la production de la case reste | 20 production |
| Champ de légumineuses | 3 000 | 80 % d'un champ (60 % avant) | 20 production |
| Pâturage | 1 200 (1 000) | nourriture × 3 (× 1,5) pour 1 200, production +1 pour 800 | 15 production |
| Maraîchage (ex-jardin) | 600 (300) | nourriture × 1,8 pour 600 (× 0,75) | 10 production |
| Coupe de bois | 600 (1 000) | production × 2,5 pour 600 (× 2) ; la cueillette reste | 15 production |

**Rendements révisés le 13/11/2026 (décision de Benjamin, après Bowles 2011 et les densités de
Smil et Marlowe)** : les premiers champs rendaient moins par heure que la cueillette (Bowles : la
cueillette 50 à 73 % plus productive), mais la culture fait vivre 10 à 100 fois plus de monde par
km² (cueillette 0,1 à 1 hab./km², culture sur brûlis 20 à 30). Règle : **la cueillette rapporte plus
par travailleur, la ferme porte plus de monde**. Le minimum vital est de 0,002 de nourriture par
travailleur (1 pour 1 000 habitants, la moitié actifs). Une plaine sans aménagement : 1 pour 300
travailleurs, soit 1,67 × le minimum vital (≈ 0,4 hab./km² sur une case de 40 km) ; un champ :
7,5 pour 3 000, soit 1,25 × (≈ 4 hab./km², dix fois plus d'habitants par case). Pâturage, maraîchage
et coupe de bois : voir les courbes par grandeur, plus bas. Avant, la
cueillette d'une plaine tombait juste au minimum vital, un champ plein ne nourrissait pas ses
familles (2 pour 3 000 habitants) et un champ tout juste construit, avec les 500 travailleurs de
la case, rendait moins que la nature (0,93 contre 1). Suite décidée : les techniques de l'âge du
bronze (araire, traction animale, irrigation) relèveront le rendement par travailleur des champs
(tâche 34, recherche).

**Courbes par grandeur (13/11/2026, décisions de Benjamin après l'avis d'un game designer, d'un
ergonome et d'un joueur)** : avec une seule courbe par case, une coupe de bois construite sur une
forêt de 300 cueilleurs donnait +0,15 de production et faisait perdre 0,57 de nourriture (la
nourriture de la forêt étalée sur 1 000 places). Règles retenues (`intendant::Courbe`) :
- **chaque grandeur a sa courbe et sa capacité** ; la case a autant de places que la plus grande ;
- **deux exposants** : cueillette et chasse 0,7 (les ressources sauvages s'épuisent vite) ;
  aménagements 0,85 (on étend le travail : une parcelle, des arbres de plus). Avec 0,7, un champ
  de 300 travailleurs aurait donné 50 % de plus que la cueillette ; avec 0,85, 6 % (« une petite
  variation », Benjamin) ;
- ~~jamais moins que la nature (le maximum des deux courbes)~~ : remplacé le même jour par la
  subsistance (plus bas) : le maximum comptait les mêmes travailleurs deux fois, et la courbe n'était
  plus régulière (l'intendant sous-estimait un champ de 150 à 200 travailleurs) ;
- **coupe de bois** : 600 places (une coupe ne prend qu'une partie de la forêt), production × 2,5
  (5 sur une forêt), soit 0,0083 par bûcheron contre 0,0067 par cueilleur ; la nourriture de la
  forêt reste (plafond 300) ; avec 300 travailleurs : 2,8 de production au lieu de 2 ;
- **jardin renommé maraîchage** (« jardin » évoquait un parc) : 600 places, fruits et légumes
  × 1,8 (0,003 par maraîcher, moins que la cueillette, autant qu'elle avec 300 travailleurs) ;
  les sauvegardes d'avant (« Jardin ») se chargent ;
- **légumineuses à 80 % d'un champ** : à 60 %, leurs travailleurs ne nourrissaient pas leurs
  familles (1,5 habitant par travailleur, il en faut 2).
L'intendant remplit bien une coupe de bois (test `l_intendant_remplit_une_coupe_de_bois`) ; à 300
travailleurs, la courbe d'un aménagement reste proche de la nature
(`un_champ_avec_peu_de_travailleurs_reste_proche_de_la_nature`) et ne passe jamais dessous
(`un_amenagement_ne_rend_jamais_moins_que_la_nature`). Page de travail : courbes des
aménagements (habitants nourris et production selon les travailleurs). Limite : la part des types
de nourriture d'une case aménagée est celle de l'aménagement, même quand la nature donne plus (peu
de travailleurs). Bilan `bilan_croissance` (inchangé, la croissance reste bornée par le taux naturel).

**Subsistance (13/11/2026, proposition de Benjamin, avis d'un game designer, d'un ergonome et
d'un joueur)** : une case sans aménagement ne donne que la **cueillette**, le travail des
chasseurs-cueilleurs (pas de subsistance en plus). Une case aménagée donne son **travail plus la
subsistance** de ceux qui y vivent (petite chasse, cueillette d'appoint, bois de chauffe…),
indépendante de l'aménagement : **75 % de la cueillette du terrain, atteinte à 500 personnes**
(`TAUX_SUBSISTANCE`, `PLACES_SUBSISTANCE`, exposant 0,7) ; elle montera avec les technologies,
jusqu'à dépasser 100 %. À nombre égal, elle rapporte moins que la cueillette. Le **travail est
réduit d'autant** : l'aménagement plein garde le total décidé (champ de plaine : 6,75 + 0,75 =
7,5 ; légumineuses 6 ; maraîchage 1,8 ; pâturage 3 de nourriture et 2 de production ; coupe de
bois 5 de production). La subsistance garde la **différence entre plaine et prairie** (Benjamin).
Conséquences : la grandeur que l'aménagement ne produit pas vient de la subsistance (champ de
plaine : 0,75 de production au lieu de 1 ; coupe de bois : 0,75 de nourriture au lieu de 1 ;
exploitation : elle a maintenant la nourriture de la subsistance, gardée par Benjamin) ; un champ de 300 travailleurs
donne ≈ 1,47 de nourriture (le travail et la subsistance), contre 1 pour la cueillette. Types et
sortes : ceux du travail, plus ceux de la nature au prorata de la subsistance (un champ de prairie :
surtout des céréales, et du gibier). Tests : `la_subsistance_rapporte_moins_que_la_cueillette`,
`un_amenagement_plein_garde_son_total`, `la_prairie_reste_plus_riche_que_la_plaine`,
`l_intendant_remplit_une_coupe_de_bois`. Affichage (maquette « Rendement des cases ») : à faire,
avec la subsistance en cellules hachurées et la décomposition « travail + subsistance » dans la fiche.

Premier bilan `bilan_croissance` (graines 777 et 2, villes de plaine de 19 cases,
200 tours) : en cueillette, 9 000 → 5 400 places ; avec champs et pâturages, nourriture pleine × 1,8
à 3 (par exemple 76 → 221) et 10 000 à 38 000 places ; la croissance sur 200 tours change peu (elle
reste bornée par le taux naturel de 1,5 % par tour : ≈ 17 000 à 18 800 habitants), les villes sans
aménagement perdent ≈ 5 % par le chômage.

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
- Construits par la file unique de la ville (**fait le 11/11/2026**, lot 3 de l'écran de la ville) ; pendant le choix, la vue zoome sur la ville et montre
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
    ville ne refait plus sans fin la même unité ; **c'est bloquant** (Benjamin, 11/11/2026) : le joueur
    ne peut pas finir son tour tant qu'une de ses villes n'a ni construction ni projet (l'écran de la
    ville s'ouvre alors sur son catalogue) ;
  - à la place de construire, la ville peut se lancer dans un **projet de ville**, sans fin, qui
    reçoit toute sa production tant que la file est vide :
    - **Peupler la ville** : la production augmente la **croissance** (et non la nourriture ;
      Benjamin, après les tests du 11/11/2026 : ajouter de la nourriture ne servait à rien quand
      elle couvrait déjà 1,5 fois le besoin) ; bonus de croissance = 0,1 % par point de production
      rapporté au besoin de nourriture, au plus +1,5 % par tour (la croissance naturelle double au
      plus) ; seulement si la ville n'a pas faim (chiffres proposés, à régler en jouant) ;
    - **Grands jeux** : bien-être (avec la livraison 4c) ;
    - **Projet de recherche** : recherche du pays (tâche 34) ;
    - **Développement de l'art et de la culture** : culture, qui étend le territoire (4d) ;
    tant que leur système manque, ces trois projets sont montrés « pas encore dans le jeu » ;
  - le joueur est prévenu si une ville reste longtemps en projet (il l'a peut-être oubliée) :
    au 10e tour, puis tous les 10 tours (seuil proposé, à régler en jouant) ; la fin de tour du
    joueur étant bloquée, seule une fin de tour forcée (console, scènes de test) peut laisser une ville
    sans rien : sa production est alors perdue et le journal le signale.

### 4c. Bien-être et hygiène (0 à 100)
- (Proposition d'origine, remplacée ci-dessous.) **Bien-être** (60 au départ) : + nourriture
  couverte (jusqu'à +10 avec un surplus), − famine (−20), − chômage. Deux effets : croissance
  naturelle (× 0,5 à × 1,5) et productivité. **Hygiène** : + eau douce proche (+20 pour une
  rivière ou un lac voisin), − entassement, − famine. Affichée seulement sous 50.
- **Révision du 12/11/2026** (remarque de Benjamin : « l'eau peut aussi amener des maladies » ;
  avis de trois agents, game designer, historien du Néolithique, concepteur de systèmes ; version
  consolidée validée par Benjamin) :
  - Constats : l'entassement par case du territoire ne mordait jamais (230 habitants par case au
    tour 50, 1 900 au tour 220 : 1,4 hab/km², une campagne normale) et faisait de l'achat d'une
    case un remède absurde ; × (0,5 + bien-être / 100) cassait le calage de 4a (× 1,1 à 60) ; la
    famine comptait trois fois ; l'effet sur la productivité (±2,5 %) était invisible ; ni le
    bien-être ni l'hygiène n'avaient de levier pour le joueur ; « hygiène » est un mot du XIXe siècle.
  - **Bien-être** : visée = 60 − 15 en famine − 1 par tranche de 2 % de chômeurs parmi les actifs
    (au plus −20) + Grands jeux (1 point pour 5 de production, au plus +15) + **luxe** (décision
    de Benjamin : biens de luxe de la production locale ou du marché, pierres précieuses, soie… ;
    +4 par sorte consommée, au plus +16, et une attente qui grandit avec la ville : à partir du
    palier 5, une sorte de plus attendue tous les 5 paliers, −4 par sorte attendue qui manque ;
    vient avec les ressources et le marché). Le bien-être s'approche du visé d'un cinquième de
    l'écart par tour. **Un seul effet** : croissance naturelle × (1 + (bien-être − 60) / 100).
    Humeur en mots : mécontente sous 40, satisfaite, prospère au-dessus de 70. Le bonus de surplus
    de nourriture est retiré (la nourriture règle déjà la croissance ; la variété de la nourriture,
    tâche 16, prendra sa place) ; l'effet sur la productivité est reporté.
  - **Salubrité** (nom proposé par l'historien) : base 60 (**50 depuis le 13/11/2026**, seuil des
    maladies 40 au lieu de 50 : même marge, mais les bonus des ressources gardent de l'effet sous
    100 ; pas de bonus à forte salubrité pour l'instant, elle sert quand la population augmente ;
    plus tard, elle réduira la prise des épidémies) ; eau courante voisine +15 ; lac voisin
    +5 (eau qui se renouvelle mal) ; marais voisin −15 pondéré par l'été (paludisme : rien sous
    15 °C, tout au-dessus de 20 °C) ; −1 par pâturage du territoire (zoonoses), au plus −10 ;
    taille : −1 par millier d'habitants au-delà de 5 000, doublé sans eau courante, au plus −40 ;
    **puits** +10. Pas de malus de famine (un seul effet de la famine en plus de la perte
    d'habitants). Effet : sous 50, la croissance perd jusqu'à 1 point de % par tour à 0
    (mortalité ; message distinct de la famine). Montrée seulement sous 50.
  - **Puits** (décision de Benjamin) : construction de la ville, dans la file, sur le centre, un
    par ville, 15 de production ; +10 de salubrité ; il donnera aussi de la **capacité en eau**
    quand l'Eau sera dans le jeu. Les fontaines (âge du Bronze) et l'assèchement des grands
    marais sont anachroniques au Néolithique ; plus tard : dépotoirs à l'écart, drains de marais.
- **Fait le 12/11/2026** : `CityState.bien_etre`, `site`, `puits`, `insalubre` ; `rules::Croissance`
  (croissance par cause), `CityState::bien_etre_vise`, `termes_bien_etre`, `salubrite`,
  `termes_salubrite`, `rules::site_de_la_ville` (mis à jour avec les cases exploitées) ;
  `CityState::passer_le_tour(rendements, chômeurs)` ; `Production::Puits` ; la prévision simule
  le bien-être. Écran de la ville : puce du bien-être sous les cartes (salubrité si sous 50),
  panneau « Bien-être et salubrité » de l'onglet Population, puits dans le catalogue, gain des
  Grands jeux. Pas encore : le luxe (ressources), la part active selon le bien-être, les
  épidémies (tâche 40). Remarque de l'historien à garder : 1,5 % par tour n'est réaliste que si
  un tour dure plusieurs années.

### 4d. Territoire par la Culture
- Départ : la ville et ses 6 voisines.
- Culture : +1 par tour pour la ville, +2 avec un **sanctuaire** (premier bâtiment).
- Une case de plus quand le stock atteint 10 + 6 × (cases déjà gagnées) ; choisie automatiquement
  (la meilleure voisine, atouts d'abord). **Achat d'une case précise** en Économie : 20 + 10 × distance.
- Remplace `rules::territory_radius` ; la borne par la capacité de gouverner vient avec la tâche 32.
- **Fait le 12/11/2026** : stock `CityState.culture` (+1 par tour, dans `process_turn`), seuil
  `rules::seuil_culture`, case choisie par `rules::case_par_la_culture` (voisine à personne, la
  meilleure valeur aménagée au mieux, la plus proche du centre à égalité ; les atouts viendront avec
  les ressources) ; projet « Développement de l'art et de la culture » : 1 culture pour 5 de
  production (proposé, validé par Benjamin le 12/11/2026) ; achat `game::acheter_case` (20 + 10 × distance ; une
  case achetée relève aussi le seuil de culture de la suivante : Benjamin, 12/11/2026). Achat depuis l'écran de la ville
  (décision de Benjamin) : les voisines à acheter en pointillé sur la carte avec leur prix, leur fiche
  et son bouton « Acheter » (`ecran_ville::fiche_achat`). Sanctuaire **fait le 13/11/2026** :
  `Production::Sanctuaire` (40 de production, +1 de culture par tour ; mégalithe à 30 avec 2 de
  pierre dure, décision M1). Pas encore : la prévision du territoire
  (décision C2 : non). Les villes des anciennes sauvegardes gardent leur territoire.

## Questions ouvertes
- Chiffres : consommation, seuils de croissance, conversion habitants → indice, rythme mensuel des rendements.
