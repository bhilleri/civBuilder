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
  **Aménager dope fortement les rendements.** La ville construit elle-même ses aménagements.
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
  les cols, tâche 17).
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
  - matériaux selon le lieu : pierre, brique, bois ou torchis selon la géologie (tâche 13) et la forêt ;
  - toits selon le climat : forte pente sous la neige, plats en climat aride, larges débords sous les tropiques ;
  - plus tard, la culture (cours intérieures, damier ou rues tortueuses).
- **Règles des structures respectées** : bâtiments posés sur le relief sans remblai ni terrasse, pas de superposition.
- **Rendu** : une vingtaine de modèles par style, dessinés en instanciation. De loin, une tache
  de toits à la couleur des matériaux, avec seulement les bâtiments marquants en relief.

## Intendant guidable (décidé le 10/11/2026)
L'intendant est automatique, mais **guidable par le joueur** :
- **Forcer l'exploitation d'une case** (et son effectif).
- **Orienter les rendements** vers un type en choisissant le **niveau d'agressivité** : une
  priorité par ressource, à quelques crans (normale, prioritaire, maximale).

Fonctionnement proposé :
1. **Besoins du tour** : nourriture (consommation, soldats compris, + croissance voulue, plus
   tard réserve d'hiver), économie (entretien), production (ce qui est en construction).
2. **Placement des travailleurs par petits groupes** là où le gain marginal est le plus fort
   (rendements décroissants, paliers des bâtiments). Le poids de chaque ressource **grandit avec
   le manque** : tant que la ville ne se nourrit pas, la nourriture pèse lourd ; une fois couverte,
   son poids retombe et la production ou l'économie prennent le relais. L'eau décide quelles
   cases peuvent être irriguées ; la distance au centre pénalise un peu.
3. **Consignes du joueur** (priorités par ressource, cases forcées) appliquées d'abord.
4. **Chômage** : si la population active dépasse la capacité totale, le bonheur et l'attractivité baissent.
5. **Manque impossible à combler** : la ville le signale (« Lyon manque de nourriture : 2 habitants
   sans case fertile ») ; plus tard, le commerce pourra combler.
6. **Explication** : barres besoin / production / écart et raison de chaque choix (« 3 aux champs
   pour nourrir la ville, 1 en forêt pour le bateau en cours »).
- Avec les saisons, la répartition est recalculée chaque tour (rendements du mois, récolte) ; les stocks lissent l'année.

### Types d'aménagements
- Champ, pâturage, coupe de bois, mine, pêcherie, salines, irrigation, drainage, port fluvial, routes, digues.
- Ce qui est possible dépend de la case : relief et pente, fertilité (tâche 11), géologie (tâche 13), eau.
- Visible en 3D : champs autour de la ville, ville qui s'étend (`conception-rendu-3d.md`).
- Lisibilité à trois niveaux : la carte (aspect rural, bourg, urbain), le panneau de la ville
  (totaux, choix de l'intendant), le détail d'une case au survol (travailleurs, capacité, paliers).

## Culture et politique (pistes retenues comme base le 10/11/2026, à détailler dans une doc dédiée)
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

## Questions ouvertes
- Culture et politique : détailler les pistes dans une doc dédiée.
- Chiffres : consommation, seuils de croissance, conversion habitants → indice, rythme mensuel des rendements.
