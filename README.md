# CivBuilder

Un petit jeu 4X (eXplore, eXpand, eXploit, eXterminate) au tour par tour, écrit en Rust avec [Bevy](https://bevy.org) 0.19.

## Installation de Rust (Windows)

1. Installer les **Build Tools de Visual Studio** (composant « Développement Desktop en C++ ») :
   https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/
2. Installer Rust avec **rustup** : https://rustup.rs (télécharger `rustup-init.exe`, choix par défaut).
3. Ouvrir un nouveau terminal et vérifier : `cargo --version` (Rust 1.95 minimum).

## Lancer le jeu

```powershell
cd D:\projet\testIA\CivBuilder
cargo run
```

La première compilation prend plusieurs minutes (Bevy est volumineux) ; les suivantes sont rapides.
Pour une version optimisée : `cargo run --release`.
Pour rejouer une carte précise, fixer sa graine (affichée dans la console au lancement) : `$env:CIVBUILDER_SEED=12345; cargo run` (PowerShell).

> Les icônes sont chargées depuis le dossier `assets/`. Avec `cargo run`, Bevy le trouve automatiquement ;
> si vous lancez directement l'exécutable (`target\release\civbuilder.exe`), copiez `assets/` à côté de lui.

Tests unitaires de la logique (hexagones, carte, pathfinding, règles) : `cargo test`.

## Commandes

| Action | Contrôle |
|---|---|
| Déplacer la caméra | ZQSD (ou WASD) / flèches, clic milieu + glisser |
| Zoom | Molette |
| Sélectionner unité / ville | Clic gauche (cliquer à nouveau pour parcourir une pile) ; les cases accessibles ce tour-ci sont surlignées |
| Déplacer l'unité | Clic droit (le chemin est prévisualisé au survol, avec le numéro du tour d'arrivée à chaque étape ; l'ordre se poursuit sur plusieurs tours) |
| Attaquer / tirer | Clic droit sur un ennemi visible (cercle rouge = attaquable ce tour-ci ; la prévision du combat s'affiche au survol). Les archers tirent s'ils sont à portée, en s'approchant si besoin (sauf l'archer expert) |
| Échanger deux unités | Clic droit sur une de vos unités adjacentes |
| Unité suivante en attente | Tab |
| Passer / fortifier | Espace |
| Exploration automatique (unités militaires, éclaireur) | E (à nouveau pour arrêter) |
| Fonder une ville (colon) | B |
| Choisir la production (ville) | Panneau de gauche (clic) ou touches 1 à 9 |
| Fin du tour | Entrée ou bouton en bas à droite |
| Désélectionner | Clic gauche sur une case vide |
| Menu du jeu : reprendre, mode debug (carte révélée), quitter | Échap |
| Basculer vue 2D / vue 3D | Bouton au-dessus de « Fin du tour » ou touche V |
| Choisir la carte : normale, relief (topographique) ou climat | Bouton « Carte » (menu) ou touche T |
| Wiki du jeu (cases, unités, mécaniques, villes, villages, barbares) | Bouton « Wiki » ou touche F1 ; Échap pour fermer |
| Vue 3D : caméra | ZQSD / flèches, molette, clic milieu + glisser ; rotation A / R |

## Règles actuelles

- **Carte** : toute une planète, plus petite que la Terre, sur 192 × 111 hexagones de 40 km (une ville et ses abords) : environ 7 700 km d'ouest en est, 3 700 km d'un pôle à l'autre, l'équateur au milieu. Relief par bruit fractal, puis climat simulé (qui fait le tour complet de la planète) ; nouvelle carte à chaque partie. La carte de jeu ne boucle pas encore d'est en ouest (bords océaniques). Taille réglable pour les essais avec `CIVBUILDER_MAP` (par exemple `128x74`).
- **Climat** : simulé à la génération (module `climate.rs`), en grandeurs physiques (°C, mm, g/kg) à l'échelle des cases (pas de temps adapté à leur taille), calculée en parallèle sur tous les cœurs du processeur (bibliothèque rayon). Pression (ceintures selon la latitude, effet thermique des continents, dépressions et anticyclones mobiles) → vents fortement déviés par la rotation de la planète ; circulation de fond (descente vers 30°, montée à l'équateur et vers 60°) ; l'air qui converge monte et celui qui diverge est remplacé par de l'air qui descend (cohérence de masse). **Deux couches d'air** : la couche basse suit un relief adouci et porte la vapeur, la couche haute est plus froide ; il pleut quand l'air humide monte et sature (convergence, fronts, orages tropicaux, relief), avec un rendement de précipitation et une part des gouttes qui s'évapore en tombant. Couvercle d'inversion sous les anticyclones, brouillard qui bruine à peine, neige portée au-delà des crêtes. Capacité de l'air selon Clausius-Clapeyron (~7 %/°C). **Sols** : réserve de 250 mm, évaporation potentielle de Hargreaves bornée par l'humidité de l'air, ruissellement du trop-plein (qui alimente les cours d'eau), rosée. Courants marins. Quatre moments de l'année moyennés. Terrain selon la température et l'indice d'aridité (seuils du PNUE). Le survol d'une case affiche climat, température, pluie annuelle et latitude.
- **Carte du climat** : précipitations annuelles en mm (de l'ocre « presque rien » au bleu « 2 000 mm et plus ») et flèches des vents dominants, en 2D comme en 3D, avec une légende.
- **Cours d'eau et lacs** (module `hydro.rs`) : le ruissellement descend le relief de case en case jusqu'à la mer (Priority-Flood : les cuvettes sont remplies jusqu'à leur col), avec des débits réels en m³/s. **Lacs** : une cuvette évapore par sa surface (eau libre moins la pluie) ; si les apports dépassent ce que la cuvette pleine évapore, le lac déborde et l'eau repart vers l'aval, sinon c'est un lac sans issue à l'équilibre (mer Morte, lac Tchad). Une cuvette peu profonde ne noie qu'une partie de la case. Lac d'au moins une demi-case = cases d'eau (terrain « Lac », éventuellement en altitude) ; plus petit = petit lac dans une case de terre. **Pertes** en climat sec par infiltration dans le lit (proportionnelle à la largeur du lit) : les oueds se perdent, les grands fleuves traversent le désert. **Fleuve** (2 000 m³/s et plus) : occupe toute la case, bande d'eau entre deux berges. **Rivières** (100 m³/s et plus, bassin d'au moins 6 cases) : sur les arêtes entre les cases, l'eau allant de coin en coin. Effets : traverser une rivière +2 PM ; entrer dans un fleuve 3 PM (l'unité s'y arrête) ; mêlée à travers une rivière ou depuis un fleuve : +3 en défense pour la cible ; eau douce (rivière, fleuve, lac voisins, petit lac) : +1 nourriture ; fleuve : 2 nourriture, 1 or ; pas de ville dans le lit d'un fleuve. Les trajets des unités cherchent le chemin le plus rapide en tours (une traversée d'un coup peut battre un détour moins coûteux). Débits et rivières au survol.
- **Terrains** : rendements nourriture / production / or et coût de déplacement (forêt = 2 PM ; montagnes et eau infranchissables).
- **Carte du relief** : vue topographique (en 2D comme en 3D) où chaque case prend la couleur de son niveau d'altitude (vert au littoral, jaune, orange, brun vers le massif, blanc pour les sommets, deux bleus pour l'eau), avec une légende ; en 3D, arbres et bosses des collines sont masqués pour mieux lire le relief.
- **Collines** : relief interne à la case, qui s'ajoute au terrain (prairie, plaine, désert, toundra, neige ou forêt vallonnée) : +1 production, +1 PM (forêt vallonnée : 3 PM), +3 en défense (cumulable avec la forêt), pas de charge. Elles ne comptent pas dans la hauteur (c'est le rôle de l'altitude). Plus fréquentes au pied des changements de niveau (contreforts). En 2D : motif de bosses sur la couleur du terrain ; en 3D : bosses de la couleur du terrain, avec des arbres dessus en forêt.
- **Altitude** : 6 niveaux de terre, du littoral (1) au massif (6, où se dressent les sommets). Le niveau s'affiche au survol. Pente douce (1 niveau) : coût normal ; montée de 2 niveaux : +1 PM ; écart de 3 niveaux ou plus : falaise infranchissable dans les deux sens, y compris pour le corps à corps et le flanquement. Affichage : même niveau = séparation discrète ; descente praticable = ombre en dégradé côté bas (rampe douce en 3D), plus marquée pour 2 niveaux ; falaise = arête sombre à dents de hachure en 2D ; en 3D, paroi de roche striée et irrégulière, liseré clair au sommet et rochers éboulés au pied. En 3D, le sol est un relief continu : le centre des cases est plat et leurs bords se raccordent en pente douce ; arbres, collines, tracés et cercles suivent le sol ; villes et camps sont posés sur une terrasse. Les montagnes sont des massifs en facettes (sommet principal, sommets secondaires, neige), différents sur chaque case et reliés en chaînes par des crêtes. La mer (côte comme large) est au même niveau, plate ; la terre du littoral et de la plaine basse descend jusqu'à elle en plage de sable (débarquement possible), les terres plus hautes tombent dans la mer en falaise rocheuse. En 2D, la teinte s'éclaircit aussi avec l'altitude. Le froid augmente avec l'altitude (toundra, neige).
- **Brouillard de guerre** : inexploré (noir), exploré hors de vue (assombri), visible.
- **Déplacements** : les trajets ne s'appuient que sur le terrain exploré ; une case inconnue est supposée franchissable mais coûte 2 PM (le terrain connu est donc préféré). En avançant, l'unité découvre le terrain et recalcule son trajet. L'ordre est annulé si la destination s'avère inaccessible, et suspendu si le trajet devient plus de 50 % plus coûteux que l'estimation initiale.
- **Villes** : distance minimale de 4 cases. Elles exploitent leur case centrale + une case par habitant (choix automatique des meilleures). Chaque habitant consomme 2 nourriture. Territoire de rayon 1, puis 2 à partir de 3 habitants.
- **Exploration automatique** : l'unité vise la case connue qui maximise « cases inconnues révélées ÷ (coût d'accès + 2) », en ne marchant que sur du terrain connu ; la cible est réévaluée à chaque pas. Un village découvert vaut 25 cases à découvrir : les explorateurs font un détour pour le visiter. Ils évitent les abords des ennemis et des camps connus (à 2 cases ou moins), y compris ceux aperçus en cours de route. Les explorateurs coopèrent : les cases que l'un va révéler sont réservées et ne comptent pas pour les autres. Elle s'arrête d'elle-même quand plus rien d'accessible n'est à découvrir. Un ordre manuel ou Espace l'interrompt.
- **Villages** (~1 case terrestre sur 45, à 3 cases au moins du départ) : la première visite d'une unité déclenche un événement — les habitants rejoignent la ville la plus proche (+1 habitant, 35 %), échanges commerciaux (+15 à 35 or, 45 %), ou village hostile (1 à 2 guerriers hostiles surgissent, 20 %). Sans ville, le ralliement est remplacé par le commerce.
- **Faction hostile** (unifiée, hostile envers tous) : des camps (à 10 cases au moins du départ) recrutent une milice tous les 6 tours (2 au plus par camp) ; elles errent à 3 cases au plus de leur base. Elles ne sont visibles qu'à portée de vue et ne donnent pas de vision au joueur.
- **Unités** : colon, éclaireur et 10 unités militaires (milice, guerrier bouclier, guerrier arme lourde, lancier, piquier, archer-épéiste, archer expert, cavalerie légère, cavalerie lourde, archer monté) — statistiques détaillées dans le panneau de production. Un colon coûte 1 habitant (ville de 2+ habitants).
- **Une unité par case** : vos unités se contournent ; une unité produite apparaît sur la ville ou une case voisine libre (sinon la production attend).
- **Combat au corps à corps** : 100 PV ; dégâts ≈ 30 × e^(écart de force ÷ 25) × aléa (0,8 à 1,2) ; le défenseur riposte ; −1 de force par tranche de 10 PV perdus. Bonus : lancier +10 / piquier +20 contre la cavalerie, cavalerie légère +10 contre archers et milice. **Charge** : +8 pour la cavalerie (légère ou lourde) qui s'est déplacée ce tour-ci, sauf contre une cible en forêt ou dans des collines. L'attaque consomme les PM restants ; le vainqueur avance sur la case libérée ; à 0 PV, l'unité est dispersée.
- **Tir à distance** (archer-épéiste, archer expert, archer monté) : pas de riposte ; force de tir contre défense à distance. Hauteur : le niveau d'altitude (les collines n'y comptent pas). Archer plus bas que sa cible, ou cible en forêt : portée −1 (jamais moins de 1) ; archer plus haut : +20 % de force de tir. L'archer expert ne tire pas après s'être déplacé ; l'archer monté peut se replier après son tir (il garde ses PM moins 1). Une attaque par tour.
- **Terrain** : +3 en défense en forêt et +3 dans des collines, cumulables (mêlée comme tir). **En hauteur** : +2 en défense de mêlée si le défenseur est à un niveau d'altitude supérieur à l'attaquant.
- **Flanquement** : +2 en attaque de mêlée par allié adjacent à la cible.
- **Zone de contrôle** (infanterie de mêlée : milice, guerriers, lanciers, piquiers) : une unité ennemie qui entre sur une case adjacente s'arrête pour le tour ; elle peut encore attaquer. Le trajet d'approche d'une attaque ne traverse pas de zone de contrôle.
- **Soins** : +10 PV par tour sans bouger ni combattre (+20 en territoire ami).
- **Hostiles** : ils attaquent vos unités à 2 cases au plus (près de leur base), seulement si le combat leur est favorable ; blessés, ils restent sur place pour se soigner. Un camp vide est dispersé en y entrant (+25 or).

## Architecture

```
assets/icons/       Icônes des unités, villages et camps (PNG blanc sur fond transparent)
assets/fonts/       Police Inter (licence SIL OFL)
src/
├── main.rs         Assemblage de l'application Bevy
├── hex.rs          Coordonnées hexagonales axiales (pur, testé)
├── map.rs          Terrains, altitude et pentes, rendements, génération procédurale (pur, testé)
├── climate.rs      Simulation du climat : pression, vents, deux couches d'air, pluie, sols (pur, testé)
├── hydro.rs        Hydrologie : écoulement, lacs, débits et pertes, fleuves, rivières sur les arêtes (pur, testé)
├── waterways.rs    Formes des cours d'eau à dessiner, partagées par la 2D et la 3D (pur, testé)
├── pathfinding.rs  A* sur la grille (pur, testé)
├── explore.rs      Exploration automatique par frontière (pur, testé)
├── sites.rs        Placement des villages/camps et événements de village (pur, testé)
├── hostile.rs      ECS : villages, camps, IA hostile, occupation des cases
├── units.rs        Types d'unités et statistiques (pur, testé)
├── combat.rs       Combat : mêlée et tir, charge, terrain, flanquement, hauteur, prévision (pur, testé)
├── battle.rs       ECS : engagement (terrain, flanquement), combat, journal, dispersion
├── rules.rs        Déplacements, soins, villes, croissance, production (pur, testé)
├── game.rs         ECS : composants, ressources, fin de tour, brouillard
├── render.rs       Affichage des cases, unités, villes, frontières
├── input.rs        Caméra, sélection, ordres
├── ui.rs           Interface (barre du haut, panneaux, journal, boutons)
├── wiki.rs         Wiki du jeu : contenu tiré des données et des règles, fenêtre de lecture
├── labels.rs       Étiquettes ancrées sur la carte (bannières des villes, tours d'arrivée en 3D)
├── relief.rs       Relief continu de la vue 3D : profil des cases, raccords, maillage (pur, testé)
└── view3d.rs       Vue 3D optionnelle (relief, forêts, pions), synchronisée avec la vue 2D
```

Toute la logique de jeu vit dans les modules « purs » (sans Bevy), ce qui permet de la tester avec `cargo test` et, plus tard, de la réutiliser pour une IA ou une sauvegarde.

## Pistes pour la suite

- Adversaires IA et combat (guerriers, défense des villes)
- Bâtiments (grenier, atelier, marché) et arbre technologique (science)
- Aménagements des ouvriers (fermes, mines, routes) et ressources spéciales
- Sauvegarde/chargement (serde), choix de la graine et de la taille de carte
- Carte qui boucle horizontalement
- Cours d'eau : ponts et moulins à eau (aménagements des villes), navigation fluviale, fonte des neiges et crues avec les saisons, retour de l'évaporation des lacs dans le climat
