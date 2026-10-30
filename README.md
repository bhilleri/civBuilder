# CivBuilder

[![Tests](https://github.com/bhilleri/civBuilder/actions/workflows/ci.yml/badge.svg)](https://github.com/bhilleri/civBuilder/actions/workflows/ci.yml)
[![Dernière version](https://img.shields.io/github/v/release/bhilleri/civBuilder?label=version)](https://github.com/bhilleri/civBuilder/releases/latest)
[![Téléchargements](https://img.shields.io/github/downloads/bhilleri/civBuilder/total?label=t%C3%A9l%C3%A9chargements)](https://github.com/bhilleri/civBuilder/releases)

Un petit jeu 4X (eXplore, eXpand, eXploit, eXterminate) au tour par tour, écrit en Rust avec [Bevy](https://bevy.org) 0.19, sur une planète générée et simulée : relief, climat, cours d'eau et lacs réalistes.

Version actuelle : **0.2.0 « Biome Update »**. Nouveautés : voir le [journal des versions](CHANGELOG.md).

## Jouer

1. Télécharger `CivBuilder-<version>.zip` sur la [page de la dernière version](https://github.com/bhilleri/civBuilder/releases/latest).
2. Décompresser le zip, puis lancer `civbuilder.exe`. Rien d'autre à installer.

- **Configuration** : Windows 10 ou 11 (64 bits), carte graphique DirectX 12 ou Vulkan.
- **Alerte Windows** : l'exécutable n'est pas signé, Windows peut afficher « Windows a protégé votre ordinateur » : « Informations complémentaires » puis « Exécuter quand même ».
- **Génération du monde** : quelques dizaines de secondes au lancement (le climat est simulé plusieurs fois), selon le processeur.
- Le **terminal** reste ouvert à côté du jeu : il affiche la graine de la carte et les messages utiles en cas de problème.
- **Wiki en jeu** : touche F1.

## Aperçu

- **Planète simulée** : 192 × 111 cases de 40 km, du pôle Nord au pôle Sud ; climat calculé (vents, pluie, 4 saisons), 14 terrains de terre, dont jungle, savanes, broussailles, taïga, steppe et marais selon la température, l'aridité et les saisons.
- **Cours d'eau réels** : débits en m³/s, fleuves dans les cases, rivières sur les arêtes, lacs d'eau douce ou salés, mers intérieures, noms tirés de huit langues en partie inventées.
- **Cartes** : normale, altitude, relief, climat, bassins versants, débits, réseau hydrographique ; vues 2D et 3D.
- **Jeu** : exploration (automatique ou non), villes et production, unités, combats, camps hostiles et villages.

## Développement

### Installer Rust (Windows)

1. Installer les **Build Tools de Visual Studio** (composant « Développement Desktop en C++ ») :
   https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/
2. Installer Rust avec **rustup** : https://rustup.rs (télécharger `rustup-init.exe`, choix par défaut).
3. Ouvrir un nouveau terminal et vérifier : `cargo --version` (Rust 1.95 minimum).

### Lancer depuis les sources

```powershell
cd D:\projet\testIA\CivBuilder
cargo run
```

La première compilation prend plusieurs minutes (Bevy est volumineux) ; les suivantes sont rapides.
Pour une version optimisée : `cargo run --release`.

Pour rejouer une carte précise, fixer sa graine (affichée dans la console au lancement) : `$env:CIVBUILDER_SEED=12345; cargo run` (PowerShell).

**Mode capture** (vérifier le rendu sans jouer) : `$env:CIVBUILDER_CAPTURE="captures"; cargo run` met en scène deux villes (dont une sur un relief accidenté), les douze types d'unités près d'une montagne, révèle la carte, passe en 3D et enregistre plusieurs vues en PNG dans le dossier `captures` (ville, unités de près et de loin, camp, village), puis se ferme (module `capture.rs`). Avec en plus `CIVBUILDER_CAPTURE_EXPLORE=1`, la carte n'est pas révélée : un éclaireur longe un fleuve case par case (exploration progressive), des vues sont prises le long de son trajet et la console indique combien de cases gardent des lits creusés périmés (doit être 0).

> Les icônes sont chargées depuis le dossier `assets/`. Avec `cargo run`, Bevy le trouve automatiquement ;
> si vous lancez directement l'exécutable (`target\release\civbuilder.exe`), copiez `assets/` à côté de lui.

Tests unitaires de la logique (hexagones, carte, climat, hydrologie, pathfinding, règles) : `cargo test`.

### Livraison

- **Intégration continue** (`.github/workflows/ci.yml`) : à chaque push sur `main` et à chaque pull request, GitHub vérifie clippy (sans avertissement) et lance les tests.
- **Publier une version** (`.github/workflows/release.yml`) :
  1. Changer `version` dans `Cargo.toml` (par exemple `0.2.0`) et ajouter la section `## 0.2.0 …` en tête de `CHANGELOG.md`.
  2. Commiter, puis créer et pousser le tag : `git tag -a v0.2.0 -m "Version 0.2.0"` puis `git push origin main v0.2.0`.
  3. GitHub compile la version Windows, fabrique le zip et publie la version (page « Releases ») avec la section du journal. Le tag doit correspondre à la version de `Cargo.toml`, sinon la publication s'arrête.
- **Paquet en local** : `.\package.ps1` (PowerShell, à la racine) fabrique le même zip `CivBuilder-<version>.zip` (exécutable, dossier `assets`, README, journal). Si Windows refuse les scripts : `powershell -ExecutionPolicy Bypass -File package.ps1`.

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
| Menu du jeu : reprendre, mode debug (carte révélée), graine du monde (copier/coller avec Ctrl+C / Ctrl+V ou les boutons, regénérer), quitter | Échap |
| Rechercher un lieu (ville, cours d'eau, lac ou mer, bassin versant, plaque) avec filtres | Bouton « Rechercher » ou Ctrl+F ; ↑ ↓ choisir, Entrée y aller, Tab filtre suivant, Échap fermer |
| Aller à un lieu depuis la carte | Clic sur le nom d'un cours d'eau, d'un lac ou sur la bannière d'une ville ; légende des bassins : survol pour le faire ressortir, clic pour y aller |
| Basculer vue 2D / vue 3D | Bouton au-dessus de « Fin du tour » ou touche V |
| Choisir la carte : normale, altitude, relief, climat, bassins versants, débits, réseau hydrographique, plaques ou ressources | Bouton « Carte » (menu) ou touche T |
| Options de carte : étiquettes (cours d'eau, lacs, mers et océans, villes, villages et camps), surbrillance des cours d'eau, rubriques de l'infobulle par carte | Bouton « Options de carte » ou touche O ; touche N : toutes les étiquettes |
| Wiki du jeu (cases, unités, mécaniques, villes, villages, barbares) | Bouton « Wiki » ou touche F1 ; Échap pour fermer |
| Vue 3D : caméra | ZQSD / flèches, molette, clic milieu + glisser ; rotation A / R |

## Règles actuelles

- **Carte** : toute une planète, plus petite que la Terre, sur 192 × 111 hexagones de 40 km (une ville et ses abords) : environ 7 700 km d'ouest en est, 3 700 km d'un pôle à l'autre, l'équateur au milieu. Relief par bruit fractal, puis climat simulé (qui fait le tour complet de la planète) ; nouvelle carte à chaque partie. La carte **fait le tour de la planète d'est en ouest** : pas de bord à l'est ni à l'ouest (continents, fleuves, bassins et plus court chemin passent le raccord, bruit du relief lu sur un cylindre) ; seuls les pôles sont des bords. Les vues 2D et 3D défilent sans fin (caméras « fantômes » décalées d'un tour, module `seam.rs`). Taille réglable pour les essais avec `CIVBUILDER_MAP` (par exemple `128x74`).
- **Climat** : simulé à la génération (module `climate.rs`), en grandeurs physiques (°C, mm, g/kg) à l'échelle des cases (pas de temps adapté à leur taille), calculée en parallèle sur tous les cœurs du processeur (bibliothèque rayon). Pression (ceintures selon la latitude, effet thermique des continents, dépressions et anticyclones mobiles) → vents fortement déviés par la rotation de la planète ; circulation de fond (descente vers 30°, montée à l'équateur et vers 60°) ; l'air qui converge monte et celui qui diverge est remplacé par de l'air qui descend (cohérence de masse). **Deux couches d'air** : la couche basse suit un relief adouci et porte la vapeur, la couche haute est plus froide ; il pleut quand l'air humide monte et sature (convergence, fronts, orages tropicaux, relief), avec un rendement de précipitation et une part des gouttes qui s'évapore en tombant. Couvercle d'inversion sous les anticyclones, brouillard qui bruine à peine, neige portée au-delà des crêtes. Capacité de l'air selon Clausius-Clapeyron (~7 %/°C). **Sols** : réserve de 250 mm, évaporation potentielle de Hargreaves bornée par l'humidité de l'air, ruissellement du trop-plein (qui alimente les cours d'eau), rosée. Courants marins. Quatre moments de l'année, gardés saison par saison. **Biomes** selon la température, l'indice d'aridité (seuils du PNUE) et les saisons (saison la plus froide, saisons sèches, été sec) : neige, toundra, taïga, steppe, désert, plaine, prairie, forêt tempérée, broussailles (climat méditerranéen), savane, savane arborée, jungle ; marais sur les terres basses et plates au bord des fleuves et des lacs (voir le wiki, « Climat »). Le survol d'une case affiche climat, température, pluie annuelle et latitude.
- **Carte du climat** : précipitations annuelles en mm (de l'ocre « presque rien » au bleu « 2 000 mm et plus ») et flèches des vents dominants, en 2D comme en 3D, avec une légende.
- **Cours d'eau et lacs** (module `hydro.rs`) : le ruissellement descend le relief de case en case jusqu'à la mer (Priority-Flood : les cuvettes sont remplies jusqu'à leur col), avec des débits réels en m³/s. **Climat et eau en 3 passes** : à partir de la deuxième, le climat voit les lacs de la passe précédente (ils évaporent et humidifient l'air), puis le terrain et les cours d'eau sont recalculés. Les petites étendues de mer enclavées (moins de 3 cases) deviennent des cuvettes, que l'eau remplit en lac ou laisse sèches. **Lacs** : une cuvette évapore par sa surface (eau libre moins la pluie) ; si les apports dépassent ce que la cuvette pleine évapore, le lac déborde et l'eau repart vers l'aval, sinon c'est un lac sans issue à l'équilibre (mer Morte, lac Tchad). Une cuvette peu profonde ne noie qu'une partie de la case. Lac d'au moins une demi-case = cases d'eau (terrain « Lac », éventuellement en altitude) ; plus petit = petit lac dans une case de terre. **Pertes** en climat sec par infiltration dans le lit (proportionnelle à la largeur du lit) : les oueds se perdent, les grands fleuves traversent le désert. **Fleuve** (2 000 m³/s et plus) : occupe toute la case, bande d'eau entre deux berges. **Rivières** (100 m³/s et plus, bassin d'au moins 6 cases) : sur les arêtes entre les cases, l'eau allant de coin en coin. Effets : traverser une rivière +2 PM ; entrer dans un fleuve 3 PM (l'unité s'y arrête) ; mêlée à travers une rivière ou depuis un fleuve : +3 en défense pour la cible ; eau douce (rivière, fleuve, lac voisins, petit lac) : +1 nourriture ; fleuve : 2 nourriture, 1 or ; pas de ville dans le lit d'un fleuve. Les trajets des unités cherchent le chemin le plus rapide en tours (une traversée d'un coup peut battre un détour moins coûteux). Débits et rivières au survol.
- **Terrains** : rendements nourriture / production / or et coût de déplacement (forêt, jungle, taïga = 2 PM ; marais 3 PM ; pics et eau infranchissables).
- **Bassins versants et noms des fleuves** (module `basins.rs`) : chaque case appartient au bassin de l'endroit où aboutit son eau (embouchure, petite côte, lac sans issue). Les fleuves sont découpés en cours principal et affluents, chacun avec son nom ; les rivières des arêtes, découpées de même de coin en coin, aussi (noms donnés par `toponymy.rs`). Noms affichés sur la carte (sur la case découverte la plus proche du milieu du cours), et au survol : de loin seuls les cours principaux, en plus gros ; les longs affluents en s'approchant ; toutes les rivières en zoom fin ; un nom qui chevaucherait une bannière de ville ou un nom plus important est décalé le long du cours, ou masqué.
- **Un nom par cours d'eau** : tronçons construits depuis l'embouchure (un fleuve garde son nom même si son débit baisse vers l'aval), à travers les lacs d'une ou deux cases ; les petits bras prennent le nom du cours où ils se jettent. **Lacs et mers intérieures nommés**, selon leur nature : lac d'eau douce (il se déverse), lac salé (sans issue), mer intérieure (mer enclavée, sans contact avec le bord de la carte) ; une case de fleuve presque entourée par un lac en fait partie ; un petit lac traversé par un fleuve porte le nom du fleuve. **Survol d'un cours d'eau ou d'un lac** (ou de son étiquette, selon l'option) : tracé en rouge, affluents directs en orange, affluents plus lointains en jaune, cours d'eau ou lac récepteur en bleu, suite du chemin de l'eau jusqu'à la mer en violet, tracés reliés aux confluents, chevrons animés dans le sens du courant, et liste dans l'infobulle. **Infobulle** réglable : rubriques affichées choisies pour chaque carte (par défaut terrain, climat, rendements, coût de déplacement, plus les rubriques propres à la carte).
- **Carte des bassins versants** : une couleur par bassin (vives pour ceux d'un fleuve nommé, grises pour les petits), légende des grands bassins découverts, cases hachurées quand elles desservent deux bassins (la moitié de leurs coins envoie l'eau vers l'autre bassin par les rivières ; à plus de la moitié, la case rejoint cet autre bassin) ; **carte des débits** : débit qui traverse chaque case, de 10 à 50 000 m³/s (échelle logarithmique) ; **carte du réseau hydrographique** : tous les cours d'eau et lacs connus, colorés selon leur rang par rapport à leur déversoir (mer, puis affluents de plus en plus lointains), avec chevrons animés du courant. En 2D comme en 3D ; le survol donne le bassin et le débit.
- **Cartes de l'altitude et du relief** : altitude en mètres (teintes du vert au niveau de la mer au blanc vers 4 800 m) et relief des cases (plat, vallonné, accidenté, montagne, pic), en 2D comme en 3D, avec leurs légendes ; en 3D, arbres et bosses sont masqués pour mieux lire le relief.
- **Relief** (interne à la case, s'ajoute au terrain) : plat ; vallonné (collines : +1 production, +1 PM, +3 en défense) ; accidenté (+1 production, −1 nourriture, +2 PM, +3 en défense) ; montagne (franchissable, 3 PM, 0/1/0, +6 en défense, pas de ville) ; pic (sommet infranchissable, environ 0,5 % des terres, jamais deux côte à côte). Les coûts s'ajoutent au terrain sans dépasser 3 PM ; collines, relief accidenté et montagne empêchent la charge. Le relief vient de la dénivelée avec les cases voisines, d'un bruit propre et de l'altitude. En 2D : bosses (collines), bosses serrées (accidenté), sommets triangulaires (montagne), grand sommet enneigé (pic) ; en 3D, le relief est modelé dans le sol (bruit continu en crêtes, d'ampleur croissante des collines aux montagnes, sommet propre aux pics), raccordé d'une case à l'autre ; roche sur les pentes raides et en hauteur, neige au-dessus d'une limite qui dépend du froid, arbres sur les versants jusqu'à la limite des arbres ; le relief s'efface près des cours d'eau, qui coulent dans des vallées.
- **Noms des lieux** (module `toponymy.rs`) : huit langues en partie inventées (nordique, latine, slave, turque, berbère, bantoue, nahua, polynésienne) : syllabes, consonnes et terminaisons d'une vraie famille de langues, plus quelques vraies racines qui ont un sens (froid, eau, montagne, rouge, sable…), choisies d'après le lieu. Le monde est partagé en huit aires de terre comparables (graines au hasard puis k-moyennes sur les terres, frontières irrégulières), et chaque langue va à l'aire dont le climat lui ressemble (meilleure répartition d'ensemble). Noms en français pour les génériques (Lac, Mer de…), accordés (genre, élision : bassin de la Vaska, du Taldor, de l'Ilmar) ; environ 30 % des lacs et 10 % des rivières ont un nom descriptif tiré de leur nature (Lac Gelé, Grand Lac Salé, Rivière Noire) ; un affluent presque aussi abondant que son fleuve en devient le jumeau (Vaska Blanche). Plaques (« plaque de Vaskar »), villages tribaux et camps barbares (Repaire des Loups, Antre des Scorpions, Camp de Kaldvik) nommés de même. Un nom ne sert qu'une fois ; même graine, mêmes noms. L'infobulle donne l'origine d'un nom (« nom nordique, « froid » »).
- **Fonds marins et pêche** : profondeur de chaque case de mer d'après le socle des plaques, rangé parmi les mers et corrigé par la distance à la côte (plateau continental jusqu'à 200 m ≈ 14 % des mers, plus ou moins large selon un bruit doux ; talus jusqu'à 3 000 m ; abysses jusqu'à 6 000 m ; fosses de 6 000 à 9 000 m le long des subductions et des arcs d'îles, côté océanique). Terrains : **eaux peu profondes** (plateau ; 2 nourriture, 1 or) et **haute mer** (1 nourriture, 1 or). **Eaux poissonneuses** (+1 nourriture) : remontées d'eaux froides (transport d'Ekman du vent simulé dirigé vers le large, à moins de 2 cases d'une côte, entre 3 et 45° de latitude), monts sous-marins (fond de haute mer à moins de 2 000 m, au moins 900 m au-dessus du fond alentour, loin du plateau), eaux des îles volcaniques isolées (au plus 30 cases, sur une plaque océanique). Les eaux poissonneuses d'un seul tenant forment des **bancs de poissons** nommés (« Bancs des Sardines » dans une remontée d'eaux chaude, « Bancs des Morues » au froid, « Bancs des Thons » sur un mont sous-marin, sinon « Banc de Kaldvik ») : **carte des ressources** (zone de chaque banc voilée de turquoise et bordée, une icône de poisson discrète par banc au centre de sa partie explorée, nom en zoom plus fin ; contour doré au survol de l'icône ou du banc ; origine dans l'infobulle de cette carte), recherche (« Bancs de poissons »). Profondeur dans l'infobulle (« 610 m de fond », « fosse ») ; carte de l'altitude : eaux peu profondes, haute mer, abysses.
- **Mers et océans** (module `seas.rs`) : distance à la côte de chaque case de mer, puis découpage là où la mer se resserre (des cases les plus au large aux plus côtières ; deux étendues reliées par un passage moins large que 65 % de la moins large restent séparées, les petites rejoignent leur voisine) : ce découpage ne sépare que les **océans** (leur nom est répété en plusieurs endroits) ; puis, à plusieurs échelles (rayons 1 à 12 cases), les **poches à l'abri** : cases qu'un cercle venu du plus grand large de chaque étendue ne peut atteindre (ouverture morphologique ; une frange d'une case le long d'une côte ne compte pas), prolongées jusqu'à la ligne entre les caps de l'entrée et gardées si elles s'enfoncent (surface ≥ 0,15 × entrée², profondeur ≥ 3 cases et ≥ un tiers de l'entrée ; baies : 0,3 et 2) : très fermées (seuil ≤ un tiers de la largeur intérieure, deux fois l'éloignement maximal à la côte ; seuil = la plus étroite des tranches parallèles à l'entrée dans la première moitié de la poche, une case du plateau continental y comptant pour moitié : un **seuil sous-marin** ferme à demi) et d'au moins 150 cases = **mers fermées**, limitées à leur seuil, quelles que soient leurs terres, balayées à leur tour par leur propre cercle pour y trouver golfes et baies (seconde passe) ; sinon bordées à 70 % par une même terre = **golfes** (21 cases et plus, sans limite haute) ou **baies** (3 à 20 cases, au plus 40) ; bordées par plusieurs terres et assez fermées (50 à 1 200 cases, terre sur la moitié du pourtour) = **mers bordières**, prolongées sur le plateau continental voisin ; contours lissés, trous bouchés, doigts retirés ; aucune poche contre le bord polaire de la carte ; détroits = passages de 1 à 4 cases entre deux terres distinctes d'au moins 4 cases. Hiérarchie : baie › golfe › mer › océan. Noms d'après la langue et la nature des côtes (Mer des Glaces, Golfe Ardent, Baie des Phoques, Passe des Vents, Golfe de la Vaska à l'embouchure d'un fleuve). Affichage par niveau de zoom (océans de très loin, mers de loin, golfes, puis baies et détroits de près), avec fondu, capitales espacées pour océans et mers, seulement si l'étendue est assez large à l'écran ; cliquables (pourtour au survol, la caméra y va), dans la recherche (« Mers et océans ») et l'infobulle.
- **Plaques tectoniques** (module `plates.rs`) : une douzaine de plaques de forme irrégulière, calculées sur la carte elle-même (le monde du jeu : elle boucle d'est en ouest, les pôles sont ses bords, une case a la même taille partout ; positions en km, distances par le plus court côté du raccord, bruits lus sur le cylindre de la carte) ; une ou deux **plaques océaniques polaires** par pôle occupent une bande de 10 à 18 lignes le long du bord (largeur variable lue sur un bruit lent), les germes des autres plaques sont tirés hors de ces bandes ; continentales (≈ 38 % des cases visés, plaques choisies tant qu'elles rapprochent de cette part) ou océaniques, qui dérivent chacune dans une direction (1 à 9 cm/an) en tournant un peu autour de leur centre ; chaque frontière est classée d'après le mouvement relatif des deux plaques (direction de la frontière lissée sur quelques cases) : collision, subduction, arc d'îles, rift, dorsale, faille transformante ; quelques points chauds loin des frontières. Carte « Plaques » (2D et 3D) avec légende et infobulle. **Le relief en découle** (`Plates::elevation`) : socle haut sur les plaques continentales et bas sur les océaniques (côtes découpées par un bruit lu sur le cylindre de la carte, intérieur des continents légèrement bombé ; le bord de la carte est un déversoir pour l'érosion et les cours d'eau), grande chaîne et haut plateau des collisions, cordillère côtière et fosse des subductions, arcs d'îles, fossés de rift, dorsales, chapelets d'îles des points chauds ; après l'érosion, **au moins un passage maritime fait le tour de la planète** (`plates::has_sea_passage` : parcours des mers en comptant les tours franchis ; sinon `open_sea_passage` rabote en mer peu profonde la boucle la moins coûteuse à moins de 26 lignes d'un pôle) ; le relief des frontières est adouci (quelques passes) pour éviter les arêtes droites à mi-chemin de deux frontières. La mer est sous 0 ; altitudes en mètres par les rangs des terres, comme avant.
- **Érosion** (module `erosion.rs`) : sur une planète, le relief tectonique est creusé avant le climat et l'eau, en 10 passes : cuvettes remplies jusqu'à leur seuil (Priority-Flood), aire drainée cumulée de l'amont vers l'aval, creusement implicite selon la loi « puissance du courant » (aire drainée^0,5 × pente, jamais sous la case aval), comblement partiel des fonds de cuvettes, légère diffusion des pentes. Vallées, profils réguliers, beaucoup moins de lacs, cols.
- **Graine** : le menu (Échap) montre la graine du monde ; on peut en saisir une pour regénérer le même monde, ou tirer un nouveau monde (le jeu se relance avec la graine dans `CIVBUILDER_SEED`).
- **Altitude** : en mètres pour chaque case de terre (affichée au survol), utilisée par le climat, les cours d'eau, les déplacements et le combat. Pente : montée de moins de 400 m gratuite, jusqu'à 900 m +1 PM, au-delà +2 PM, descente gratuite ; écart d'environ 1 000 m ou plus entre deux cases voisines : falaise infranchissable dans les deux sens, y compris pour le corps à corps et le flanquement. En 3D, le sol est à sa hauteur réelle en mètres (altitude continue, lacs à la cote de leur surface) : descente praticable = ombre en dégradé côté bas ; falaise = arête sombre à dents de hachure en 2D, paroi de roche en 3D avec rochers éboulés au pied ; la terre basse descend en plage jusqu'à la mer, les terres plus hautes tombent en falaise. Le froid augmente avec l'altitude (toundra, neige).
- **Brouillard de guerre** : inexploré (noir), exploré hors de vue (assombri), visible.
- **Déplacements** : les trajets ne s'appuient que sur le terrain exploré ; une case inconnue est supposée franchissable mais coûte 2 PM (le terrain connu est donc préféré). En avançant, l'unité découvre le terrain et recalcule son trajet. L'ordre est annulé si la destination s'avère inaccessible, et suspendu si le trajet devient plus de 50 % plus coûteux que l'estimation initiale.
- **Villes** : distance minimale de 4 cases. Elles exploitent leur case centrale + une case par habitant (choix automatique des meilleures). Chaque habitant consomme 2 nourriture. Territoire de rayon 1, puis 2 à partir de 3 habitants.
- **Exploration automatique** : l'unité vise la case connue qui maximise « cases inconnues révélées ÷ (coût d'accès + 2) », en ne marchant que sur du terrain connu ; la cible est réévaluée à chaque pas. Un village découvert vaut 25 cases à découvrir : les explorateurs font un détour pour le visiter. Ils évitent les abords des ennemis et des camps connus (à 2 cases ou moins), y compris ceux aperçus en cours de route. Les explorateurs coopèrent : les cases que l'un va révéler sont réservées et ne comptent pas pour les autres. Elle s'arrête d'elle-même quand plus rien d'accessible n'est à découvrir. Un ordre manuel ou Espace l'interrompt.
- **Villages** (~1 case terrestre sur 45, à 3 cases au moins du départ) : la première visite d'une unité déclenche un événement — les habitants rejoignent la ville la plus proche (+1 habitant, 35 %), échanges commerciaux (+15 à 35 or, 45 %), ou village hostile (1 à 2 guerriers hostiles surgissent, 20 %). Sans ville, le ralliement est remplacé par le commerce.
- **Faction hostile** (unifiée, hostile envers tous) : des camps (à 10 cases au moins du départ) recrutent une milice tous les 6 tours (2 au plus par camp) ; elles errent à 3 cases au plus de leur base. Elles ne sont visibles qu'à portée de vue et ne donnent pas de vision au joueur.
- **Unités** : colon, éclaireur et 10 unités militaires (milice, guerrier bouclier, guerrier arme lourde, lancier, piquier, archer-épéiste, archer expert, cavalerie légère, cavalerie lourde, archer monté) — statistiques détaillées dans le panneau de production. Un colon coûte 1 habitant (ville de 2+ habitants).
- **Une unité par case** : vos unités se contournent ; une unité produite apparaît sur la ville ou une case voisine libre (sinon la production attend).
- **Combat au corps à corps** : 100 PV ; dégâts ≈ 30 × e^(écart de force ÷ 25) × aléa (0,8 à 1,2) ; le défenseur riposte ; −1 de force par tranche de 10 PV perdus. Bonus : lancier +10 / piquier +20 contre la cavalerie, cavalerie légère +10 contre archers et milice. **Charge** : +8 pour la cavalerie (légère ou lourde) qui s'est déplacée ce tour-ci, sauf contre une cible en forêt ou dans des collines. L'attaque consomme les PM restants ; le vainqueur avance sur la case libérée ; à 0 PV, l'unité est dispersée.
- **Tir à distance** (archer-épéiste, archer expert, archer monté) : pas de riposte ; force de tir contre défense à distance. Hauteur : l'altitude en mètres (écart d'au moins 250 m). Archer plus bas que sa cible, ou cible en couvert boisé : portée −1 (jamais moins de 1) ; archer plus haut : +20 % de force de tir. L'archer expert ne tire pas après s'être déplacé ; l'archer monté peut se replier après son tir (il garde ses PM moins 1). Une attaque par tour.
- **Terrain** : +3 en défense en couvert boisé (forêt, jungle, taïga), +3 en collines ou relief accidenté, +6 en montagne, cumulables (mêlée comme tir). **En hauteur** : +2 en défense de mêlée si le défenseur est au moins 250 m plus haut que l'attaquant.
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
├── basins.rs       Bassins versants, couleurs des bassins, découpage des cours d'eau nommés (pur, testé)
├── seas.rs         Étendues marines : océans, mers, golfes, baies, détroits (pur, testé)
├── toponymy.rs     Langues des régions et noms des lieux : cours d'eau, lacs, plaques, villages, camps (pur, testé)
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
├── search.rs       Accès aux lieux : recherche (index, filtres), légendes et étiquettes cliquables, cadrage de la caméra
├── relief.rs       Relief continu de la vue 3D : profil des cases, raccords, maillage (pur, testé)
├── figures.rs      Figurines 3D des unités, faites de formes simples (une par type)
├── capture.rs      Mode capture (CIVBUILDER_CAPTURE) : mise en scène, vues 3D enregistrées en PNG
└── view3d.rs       Vue 3D optionnelle (relief, forêts, figurines, villes et camps posés sur le sol), synchronisée avec la vue 2D
```

Toute la logique de jeu vit dans les modules « purs » (sans Bevy), ce qui permet de la tester avec `cargo test` et, plus tard, de la réutiliser pour une IA ou une sauvegarde.

## Pistes pour la suite

- Adversaires IA et combat (guerriers, défense des villes)
- Bâtiments (grenier, atelier, marché) et arbre technologique (science)
- Aménagements des ouvriers (fermes, mines, routes) et ressources spéciales
- Sauvegarde/chargement (serde), choix de la graine et de la taille de carte
- Carte qui boucle horizontalement
- Cours d'eau : ponts et moulins à eau (aménagements des villes), navigation fluviale, fonte des neiges et crues avec les saisons, retour de l'évaporation des lacs dans le climat
