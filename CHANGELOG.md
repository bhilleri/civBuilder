# Journal des versions

## 0.3.0 (en cours)

### Déplacements et combats visibles, journal hiérarchisé
- **Les unités ne se téléportent plus** : elles glissent case par case le long de leur trajet (0,2 s par case, un long trajet dure au plus 1,3 s, à la vitesse normale), en 2D comme en 3D (figurines posées sur le relief pendant la marche ; anneau de sélection et barre de PV suivent).
- **Combats mis en scène** : au corps à corps, l'attaquant s'élance vers sa cible et revient ; un tireur recule légèrement et un trait vole jusqu'à la cible. Au moment du coup, la cible tressaille, un cercle d'impact s'élargit et les **PV perdus s'affichent en chiffres flottants** au-dessus des deux unités (taille selon la force du coup). Les barres de PV gardent leur valeur jusqu'à l'impact.
- **Unité dispersée** : elle s'efface sur place, avec « Dispersé ! » (ennemi) ou « Unité perdue » (la vôtre) et un cercle sombre.
- **Villages et camps** : la récompense s'affiche sur le site à l'arrivée de l'unité (« +25 or », « +1 habitant à Paris », « Embuscade ! 2 milices »).
- **Ordre de la fin du tour** : vos unités bougent ensemble, puis chaque action hostile visible se joue l'une après l'autre ; une action hostile hors de vue n'est pas montrée et ne fait pas attendre. Les règles restent instantanées : la mise en scène suit, sans jamais bloquer les commandes.
- **Journal (en haut à droite) hiérarchisé** : chaque message a une importance (routine, information, succès, alerte, critique) et un liseré de sa couleur. Un nouveau message glisse depuis la droite et s'éclaire d'un flash qui s'éteint, plus long et en pulsations pour les alertes (2) et les messages critiques (3, avec un contour). Les alertes et messages critiques gardent une teinte de fond ; les anciens messages s'estompent. Critique : unité perdue, village hostile ; alerte : combat, famine, production bloquée ; succès : ville fondée, ennemi dispersé, récompense.
- **Menu Échap réorganisé en pages** : la page principale ne garde que Reprendre, Paramètres, Nouveau monde… et Quitter. **Paramètres** : vitesse des animations (Lente, Normale, Rapide, Instantanée ; un sélecteur et une ligne qui dit ce que fait le choix) et mode debug. **Nouveau monde** : graine, copier / coller, regénérer ou graine au hasard, avec l'avertissement que la partie sera perdue. Échap sur une sous-page revient à la page principale.
- **Paramètres enregistrés** aussitôt dans `%APPDATA%\CivBuilder\parametres.txt` (`~/.config/civbuilder/` ailleurs, ou le fichier donné par `CIVBUILDER_SETTINGS`) : ils suivent un nouveau monde et le prochain lancement. Le mode capture ne les lit ni ne les écrit.
- **Mode capture « mise en scène »** (`CIVBUILDER_CAPTURE_FEEDBACK=1` avec `CIVBUILDER_CAPTURE`) : un corps à corps, un tir, une attaque hostile mortelle et la visite d'un village, photographiés à sept instants de leur animation, en 2D puis en 3D.

### Continents moins compacts
- **Plusieurs continents** au lieu d'une masse unique : la croûte continentale forme 4 à 6 grands blocs de tailles très inégales (aucun au-delà de 45 %) et 3 à 6 microcontinents, allongés et sinueux, séparés par l'océan (au moins ~280 km). Chaque grand bloc (hors le premier) a 15 % de chances de naître soudé à un autre (comme l'Afrique et l'Eurasie). Les blocs naissent loin des pôles et poussent moins volontiers aux hautes latitudes.
- **Les continents ne suivent plus les plaques** : un continent peut chevaucher deux plaques (chaîne de collision ou rift en plein continent), une plaque porter un continent et un océan. La nature d'une frontière dépend de la croûte de part et d'autre (subduction le long d'un continent, arc d'îles plus loin en plein océan). Une plaque est dite continentale si elle porte surtout du continent ; sa vitesse dépend de sa part de continent. Infobulle de la carte des plaques : croûte de la case.
- Plateau continental tout autour de la croûte continentale.
- **Rifts selon la vitesse d'écartement** : lent (moins de 3 cm/an), une vallée perchée sur un bombement, au fond découpé en bassins (lacs selon le climat, comme l'Afrique de l'Est) ; moyen, un **bras de mer étroit** (comme la mer Rouge) ; rapide (5,5 cm/an et plus), le continent s'est déchiré : un **jeune océan** s'ouvre entre ses deux morceaux, autour d'une dorsale, d'autant plus large que l'écartement est rapide (comme l'Atlantique naissant).
- Courants marins : le calcul reprend avec une surrelaxation plus douce s'il diverge (certaines formes d'océans le faisaient diverger).
- **Côtes découpées** : la mer entre dans les continents (golfes, péninsules, bras de mer, mers peu profondes à l'intérieur des terres). Le rivage vient d'un bruit déformé et une part fixe des plaques continentales émerge (≈ 78 %, le reste en mers peu profondes), quelle que soit la graine ; les marges sans subduction descendent en plateau continental. Les mondes changent, à graine égale.
- Bilan `formes_des_terres` (masses de terre, part de la plus grande, découpage des côtes, rifts sous la mer, dorsales des jeunes océans ; `FORMES_ASCII` : carte en caractères, croûte comprise) ; `bilan_tectonique` compte les lacs près des rifts.

### Courants marins
- **Courants de surface calculés** à partir des vents simulés (modèle de Stommel) : tourbillons subtropicaux et subpolaires, courants équatoriaux, **courants rapides et étroits le long des côtes ouest des océans** (jusqu'à 1,5 m/s), retours lents et froids le long des côtes ouest des continents, **courant circumpolaire** quand la mer fait le tour d'un pôle. Les grands continents arrêtent l'eau, pas les petites îles.
- **Dérive chaude des hautes latitudes** (circulation de retournement simplifiée : l'eau de surface dérive vers le pôle entre ~40 et 60°) : les côtes ouest des continents entre 45 et 65° sont douces (+6 à +8 °C, comme l'Europe), leurs côtes est plus froides (comme le Labrador).
- **Température de la mer calculée** (transportée par les courants, refroidie sur place par les remontées d'eaux) : elle remplace les règles fixes (côtes ouest froides, côtes est chaudes…). Les côtes en prennent une bonne part : déserts côtiers et brouillards sur les courants froids, douceur et pluie sur les courants chauds.
- **Pêche** : nouvelle origine d'eaux poissonneuses, la **rencontre d'un courant chaud et d'un courant froid** près des côtes (+1 nourriture ; bancs de morues, de harengs…).
- Nouvelle **carte des courants marins** (touche T) : mer colorée selon sa température, flèches des courants (rouges : chauds, bleu clair : froids), avec légende ; en 2D et en 3D.
- La console donne la vitesse maximale des courants et la présence des courants circumpolaires.

### Vue 3D : unités et sites sur le relief
- Les unités, villes, camps et villages ne sont plus « ensevelis » dans le relief : chacun est posé sur le vrai sol 3D, et replacé quand le sol de sa case est refait.
- **Figurines 3D** des unités, faites de formes simples, chacune avec sa silhouette : le colon en chapeau de paille tire une charrette à bras chargée de sacs, l'éclaireur en vert va avec son chien, la milice un habit de cuir et une massue, le guerrier lourd une armure sombre, un plumet et un grand bouclier aux couleurs du camp, le piquier une très longue pique, les archers un arc haut (et un carquois), le cavalier lourd un cheval caparaçonné. Les militaires sont un **petit groupe** (trois fantassins, deux cavaliers), chacun posé sur le sol sous ses pieds ; colon et éclaireur sont seuls. Les figurines **grandissent quand la caméra s'éloigne** (jusqu'à 3 fois) : elles restent visibles en dézoomant. Un fin anneau au sol, aux couleurs du camp (bleu, rouge), incliné comme la pente, entoure le groupe.
- **Sur un fleuve, l'unité est dans une barque en bois**, à la surface de l'eau, dans le sens du courant. De loin, l'icône du type flotte au-dessus, sur une pastille aux couleurs du camp, de taille à peu près constante à l'écran. La vue 2D garde ses icônes.
- Les villes et camps **suivent le terrain** (plus de terrasse) : remparts, tours, maisons, huttes, tentes et pieux posés chacun sur le sol à sa place, avec des fondations côté pente ; sols pavés et de terre battue drapés sur le relief. Sous un site, le relief est adouci, pas aplani.

### Performances
- **Vue 3D plus fluide pendant la construction du relief** : 60 images/s tenues (45 à 52 auparavant), et relief de toute la carte révélée construit en ≈ 7 s au lieu de ≈ 19 s (graine 777, mesuré avec la trace Chrome de Bevy). Les surfaces d'eau sont refaites sur tous les cœurs et comptées dans le temps alloué par image ; les paquets de cases sont plus gros (2 par cœur) ; l'ordre de traitement (cases les plus proches de la vue d'abord) est gardé d'une image à l'autre ; la hauteur d'un tronçon de cours d'eau n'est plus calculée inutilement.
- Tables du jeu passées au hachage rapide de Bevy (foldhash) au lieu de celui de la bibliothèque standard.
- **Cartes du réseau hydrographique et des plaques en 3D** : plus de baisse d'images par seconde en passant à ces cartes en plein défilement (50 à 56 images/s auparavant pendant ≈ 2 s, 60 désormais). Les tracés posés sur le relief sont gardés d'une image à l'autre et recalculés seulement quand l'exploration ou la forme du sol changent (plaques : 7,4 → 1 ms par image ; réseau : 6,2 → 3,2 ms, les chevrons animés restant calculés à chaque image).
- Le relief en construction prend moins de temps par image quand les images deviennent trop longues (moins de 60 par seconde), puis revient peu à peu à son budget.

### Outils de développement
- **Mode capture** : `CIVBUILDER_CAPTURE=<dossier>` met en scène villes et unités (relief accidenté, montagne), passe en 3D et enregistre plusieurs vues en PNG (dont la carte des courants marins), puis ferme le jeu : le rendu se vérifie sans jouer.
- **Mode capture « exploration »** (`CIVBUILDER_CAPTURE_EXPLORE=1` en plus) : carte non révélée, un éclaireur longe un fleuve case par case, des vues sont prises le long de son trajet, puis la console signale les cases dont les lits creusés ne correspondent plus au relief (lits périmés, plus grand écart de hauteur d'eau).
- Bilan `bilan_courants` (vitesses, écarts de température de la mer, côtes ouest et est, rencontres de courants ; `CURRENTS_DUMP` pour exporter les courants).

### Corrections
- Vue 3D : en explorant autour des cours d'eau, des cases gardaient un lit creusé avec une hauteur d'eau périmée (trou noir dans le sol, marche ou écume, eau sous la berge). Une case refaite plus tôt dans la même image qu'une voisine dont le relief venait d'être recalculé n'était plus remise en file ; elle l'est désormais (graine 4242 : 7 cases fausses, jusqu'à 25 d'écart d'eau → 0).
- Vue 3D lointaine : l'icône d'une unité était traversée par les casques et les armes de ses figurines (agrandies, plus hautes que l'icône), et en zoomant on voyait la figurine par-dessus ; les lignes de frontière la barraient aussi. L'icône est désormais avancée vers la caméra le long du rayon qui la vise (même place et même taille à l'écran) : elle passe devant ses figurines, le relief voisin et les lignes 3D. De près, seule la figurine reste, comme avant.

### Altitude et relief
- L'altitude en mètres devient la référence : déplacements, combat, climat et infobulle (« 2 950 m »).
- **Relief** de chaque case, en plus du terrain : plat, vallonné, accidenté, montagne, pic (dénivelée avec les voisines, bruit propre, altitude).
- **Montagnes franchissables** (3 PM, 0/1/0, +6 en défense, pas de ville) ; seuls les **pics** (≈ 0,5 % des terres, jamais côte à côte) sont infranchissables. Le terrain « Montagnes » disparaît : une montagne porte le biome de son climat (toundra, neige, forêt…).
- Relief accidenté : +2 PM, +1 production, −1 nourriture, +3 en défense. Coûts plafonnés à 3 PM.
- Pentes en mètres : montée de 400 à 900 m +1 PM, au-delà +2 PM ; falaise (≈ 1 000 m d'écart) toujours infranchissable.
- Combat : être plus haut d'au moins 250 m donne l'avantage (défense de mêlée, tir, portée).
- Nouvelles cartes **Altitude** (teintes en mètres) et **Relief** (plat → pic), avec légendes ; symboles 2D par relief.
- **Relief 3D modelé dans le sol** (fini les montagnes posées comme des objets) : crêtes et ravines continues d'une case à l'autre, ampleur croissante des collines aux montagnes, sommet propre aux pics ; roche sur les pentes raides et en hauteur, neige au-dessus d'une limite qui dépend du froid ; arbres sur les versants jusqu'à la limite des arbres ; le relief s'efface près des cours d'eau, qui coulent dans des vallées.
- **Sol 3D à l'altitude réelle** en mètres (fini les 6 tranches) : pentes douces continues, plateaux et bassins visibles ; lacs à la cote de leur surface.

### Planète sans bord
- La carte **fait le tour de la planète d'est en ouest** : on peut aller toujours vers l'est et revenir par l'ouest ; seuls les pôles restent des bords.
- Génération sans couture (relief lu sur un cylindre) et sans océan forcé à l'est et à l'ouest : des continents peuvent chevaucher le raccord ; cours d'eau, lacs, bassins et noms le traversent.
- Règles : déplacements, plus court chemin, zone de contrôle, portée de tir, vision, exploration automatique, villages et camps, IA hostile tiennent compte du raccord (distances par le plus court côté).
- Affichage : défilement sans fin en 2D comme en 3D, sans raccord visible (relief, lits, tracés, frontières, étiquettes).
- Le relief 3D se prépare aussi pendant qu'on joue en 2D (autour de la caméra) : en passant en 3D, les cases explorées sont déjà prêtes.

### Plaques tectoniques
- La planète est découpée en 10 à 14 **plaques** nommées (plaque pacifique, de Nazca, eurasienne…), continentales ou océaniques, **calculées sur la sphère** (distances et surfaces réelles, calottes polaires possibles) puis projetées sur la carte.
- Chaque plaque **tourne autour de son pôle d'Euler** (1 à 9 cm/an) : sa vitesse change d'un endroit à l'autre.
- **Frontières** classées selon le mouvement relatif : collision de continents, subduction, arc d'îles, rift continental, dorsale, faille transformante ; **points chauds** loin des frontières.
- Nouvelle **carte « Plaques »** (2D et 3D) : couleur par plaque (chaude : continentale, froide : océanique), frontières colorées, flèches des mouvements, points chauds ; légende ; infobulle « plaque de Nazca (océanique), 5 cm/an vers le nord-est ; frontière : subduction ».
- **Le relief découle des plaques** : continents sur les plaques continentales, grandes chaînes et hauts plateaux des collisions, cordillères côtières et fosses des subductions, arcs d'îles, rifts, dorsales, chapelets d'îles volcaniques des points chauds (les mondes changent, à graine égale).

- **Plaques calculées sur la carte, plus sur la sphère** : près des pôles, la projection étirait les plaques et les continents en bandes sur toute la largeur de la carte. Le monde du jeu est la carte elle-même (elle boucle d'est en ouest, une case a la même taille partout) : les plaques, leurs frontières, le relief, les points chauds et les aires de langue sont désormais calculés en distances sur la carte. Chaque plaque dérive dans une direction en tournant un peu sur elle-même. Les mondes changent, à graine égale.
- **Plaques océaniques polaires** : une ou deux plaques océaniques occupent chaque pôle, en bande de 10 à 18 lignes le long du bord. Plus de continent collé aux pôles : des **mers polaires navigables**, et de vraies frontières avec les plaques voisines (arcs d'îles, fosses, dorsales, chaînes côtières à haute latitude). Les marges cachées au-delà des pôles (essai précédent) sont retirées. Le bord de la carte reste un **déversoir** : l'eau qui l'atteint s'en va (érosion et cours d'eau). Les mondes changent, à graine égale.
- **Passage maritime autour de la planète** : au moins une mer fait le tour complet de la carte d'est en ouest ; si une terre ou une chaîne d'îles le ferme, le passage le moins coûteux près d'un pôle est raboté en mer peu profonde.
- Part des terres : environ 40 % des cases (37 à 42 % selon la graine).

### Érosion
- Avant d'y faire couler les cours d'eau, l'eau **creuse le relief** (10 passes) : vallées le long des grands cours d'eau, profils réguliers, seuils des cuvettes usés et cuvettes en partie comblées, cols aux points bas des crêtes ; les montagnes, peu drainées, gardent leur hauteur.
- Beaucoup moins de lacs (graine 777 : ≈ 270 cases au lieu de ≈ 900) et moins de ruptures de pente sur les fleuves (≈ 7 % des cases de fleuve au lieu de 13 %).

### Graine du monde
- Le menu (Échap) affiche la **graine** du monde ; on peut en taper une et **regénérer** le monde avec (même graine, même monde), ou créer un **nouveau monde** au hasard. Le jeu repart de zéro avec les mêmes réglages.

### Trouver les lieux
- **Recherche** (bouton « Rechercher » ou Ctrl+F) : villes, cours d'eau, lacs et mers intérieures, bassins versants, plaques. Résultats au fil de la frappe, sans tenir compte des accents ni des majuscules, classés (début du nom, début d'un mot, ailleurs), passage trouvé en gras ; **filtres** en puces avec le nombre de résultats de chaque catégorie ; clavier : ↑ ↓, Entrée, Tab (filtre suivant), Échap. Seuls les lieux explorés sont listés (tous en mode debug).
- Le lieu choisi **ressort sur la carte** (pourtour doré qui pulse pour une zone, surbrillance du réseau pour un cours d'eau ou un lac) et la **caméra le cadre** (vol court, zoom adapté, en 2D comme en 3D, à travers le raccord).
- **Étiquettes cliquables** : nom d'un cours d'eau ou d'un lac, bannière d'une ville (la ville est aussi sélectionnée).
- **Légende des bassins versants interactive** : survol d'un bassin pour le faire ressortir, clic pour y aller.
- Pendant la saisie dans un champ, les raccourcis du jeu sont coupés (les touches vont au champ).

### Graine du monde
- Champ de la graine éditable (curseur, sélection) : la graine actuelle est sélectionnée à l'ouverture du menu ; **Ctrl+C / Ctrl+V** (presse-papiers du système) et boutons **Copier** / **Coller**.

### Noms des lieux
- **Huit langues en partie inventées** (nordique, latine, slave, turque, berbère, bantoue, nahua, polynésienne) : des mots qui n'existent pas, avec les sons d'une vraie famille de langues et parfois une vraie racine choisie d'après le lieu (« Kaldvik » : *froid* en nordique). Chaque région du monde parle sa langue, selon son climat.
- Fini les noms réels (Amazone affluent du Congo…) : cours d'eau, lacs, mers intérieures, bassins et plaques sont nommés dans la langue de leur région, sans doublon. Noms descriptifs tirés de la nature des lieux (Lac Gelé, Grand Lac Salé, Rivière Noire), affluents jumeaux (Vaska Blanche), grammaire française (bassin de la Vaska, du Taldor, de l'Ilmar).
- **Villages tribaux et camps barbares nommés** (Repaire des Loups, Antre des Scorpions, Camp de Kaldvik…) : nom affiché sous l'icône de près, cliquable, dans le journal et dans la recherche (filtre « Villages et camps »).
- L'infobulle donne l'origine d'un nom (« nom nordique, « froid » »).

### Fonds marins et pêche
- Le **fond de la mer est calculé** d'après les plaques : plateau continental (moins de 200 m) le long de la plupart des côtes, plus ou moins large, absent devant les subductions ; talus ; abysses (plus de 3 000 m) ; **fosses** (plus de 6 000 m) le long des subductions et des arcs d'îles. La profondeur est dans l'infobulle ; la carte de l'altitude montre eaux peu profondes, haute mer et abysses.
- Les « côtes » deviennent les **eaux peu profondes** (le plateau continental, plus seulement la première case au bord des terres : 2 nourriture), l'« océan » la **haute mer** (1 nourriture).
- **Eaux poissonneuses** (+1 nourriture) : remontées d'eaux froides (où le vent pousse l'eau de surface vers le large, surtout sur les côtes ouest sous les alizés), monts sous-marins en pleine mer, eaux des îles volcaniques isolées.
- Elles forment des **bancs de poissons nommés** (Bancs des Sardines, Bancs des Thons, Banc de Kaldvik…), montrés sur la nouvelle **carte des ressources** : la zone de chaque banc voilée de turquoise et bordée, une icône de poisson discrète par banc (pas une par case), le contour doré au survol de l'icône ou du banc, l'origine de la richesse dans l'infobulle ; cliquables et dans la recherche (filtre « Bancs de poissons »). La carte normale reste lisible : pas de teinte supplémentaire.

### Mers et océans
- La mer est **partagée en étendues nommées**, de la plus vaste à la plus précise : océans (découpés là où la mer se resserre ; le nom d'un grand océan est répété en plusieurs endroits), mers (fermées ou presque, ou parties abritées d'un océan le long d'une côte, de taille modeste), golfes et baies (jusqu'à la ligne entre les deux caps de leur entrée), détroits (passages entre deux terres, précieux à tenir). Un golfe peut être dans une mer, elle-même dans un océan. Noms d'après la langue et la nature des côtes (Mer des Glaces, Golfe Ardent, Baie des Phoques, Passe des Vents) ; un golfe à l'embouchure d'un fleuve peut en porter le nom (Golfe de la Vaska).
- **Noms à leur échelle** : océans de très loin, mers de loin, golfes en s'approchant, baies et détroits de près ; fondu à l'apparition ; capitales espacées pour océans et mers ; un nom n'apparaît que si l'étendue est assez large à l'écran et en partie explorée.
- **Golfes, baies et mers bordières revus, à l'échelle** : un cercle venu du large balaie la mer à plusieurs tailles (de 1 à 12 cases de rayon) ; ce qu'il n'atteint pas est à l'abri derrière un goulet. Une échancrure d'une même terre, plus profonde que large à l'entrée, devient un golfe (sans limite de taille : il peut y en avoir d'immenses, comme le golfe du Mexique) ou une baie (3 à 20 cases), jusqu'à la ligne entre ses deux caps ; un abri entre plusieurs terres (une côte et des îles) devient une mer bordière, prolongée sur le plateau continental. Fini les mers en anneau ou en ruban le long des côtes : contours réguliers, trous bouchés, pas de doigts ; rien ne s'appuie sur le bord polaire de la carte. Une baie peut être dans un golfe, un golfe dans une mer. Le nom d'un océan se pose au large de ses golfes et mers.
- **Mers fermées revues** (comme les golfes) : une mer fermée est une poche à l'abri derrière une entrée étroite (au plus un tiers de sa largeur, 150 cases au moins), limitée à la ligne entre les caps de son entrée, contours lissés (pointes, doigts, trous), avec ses golfes et baies dedans. Avant, elle venait du découpage par distance à la côte : limite en dents de scie, une partie de la mer rattachée à l'océan. Ce découpage ne sert plus qu'à séparer les océans ; la règle « mer polaire » disparaît (le bord n'est plus une côte).
- **Seuils sous-marins** : un passage peu profond (plateau continental) ferme à demi l'entrée d'une mer, comme Gibraltar pour la Méditerranée ou les détroits danois pour la Baltique ; la mer fermée commence à ce seuil (le passage le plus étroit près de son entrée) et non plus à la ligne des caps.
- **Limites des océans corrigées** : un océan ne file plus en frange d'une case le long des côtes jusqu'à l'océan voisin (graine 22893809 : l'Océan Boréal descendait jusqu'à l'Océan Austral, à l'est et à l'ouest des deux océans du milieu ; même défaut sur 777, 4242…). Le découpage est un vrai partage des eaux : chaque case rejoint l'étendue de sa voisine la plus au large, et les cases à égale distance de la côte sont réparties par propagation depuis les étendues déjà formées (et non plus prises ligne par ligne, du nord au sud). Une mer bordière laisse au moins 15 % de son océan au large, où poser le nom de l'océan.
- **Océans polaires d'un seul tenant** : au-delà du 60e parallèle (la limite de l'océan Austral dans les atlas), la mer de chaque pôle forme un seul océan, qui n'est plus coupé en plusieurs par une ligne droite là où un continent s'avance vers le pôle (graine 2 : trois océans au sud et deux au nord auparavant). Les autres océans s'arrêtent à ce parallèle. L'océan polaire ne redescend plus vers les tropiques par un bassin trop petit pour être un océan (graine 3309035769 : l'Océan Boréal descendait jusqu'à la Mer des Moussons) : un tel bassin, s'il a la taille d'une mer et descend à plus de 10° sous le parallèle, reste à part (une mer, ou un océan s'il est assez grand) ; une frange le long du parallèle ou une petite échancrure rejoint l'océan polaire.
- Correction : une mer bordière pouvait se trouver dans une mer à part (pas dans un océan), et leurs deux noms se superposaient (graine 985384862 : Mer de Timanfan, 180 cases, dans la Mer Sombre, 252 cases). Une mer bordière n'est plus cherchée que dans un océan ; une mer peut toujours avoir ses golfes et ses baies.
- Correction : sur une planète aux pôles couverts de terre, l'océan ne touche aucun bord de la carte et était pris pour une mer intérieure géante (rangée parmi les lacs, bassins des fleuves comptés comme fermés) ; une mer intérieure fait désormais au plus 800 cases.
- Cliquables comme les autres noms (pourtour doré au survol, la caméra y va), dans la recherche (filtre « Mers et océans », qui reprend les mers intérieures) et dans l'infobulle (« Côte — Baie des Phoques, Mer de Vodava ») ; option d'affichage « Mers et océans ».

## 0.2.0 « Biome Update » — 24 octobre 2026

Les saisons du climat façonnent maintenant les paysages  : 7 nouveaux terrains, de la jungle à la taïga.

### Biomes
- Le climat garde ses 4 saisons (température, pluie, évaporation possible de chaque saison).
- 7 nouveaux terrains : **jungle**, **savane**, **savane arborée**, **broussailles** (climat méditerranéen, été sec ; au-delà d'un indice d'aridité de 0,75, forêt tempérée), **taïga**, **steppe**, **marais** ; la forêt devient **forêt tempérée**.
- Règles : saison la plus froide (taïga sous −3 °C, tropical au-dessus de 18 °C), nombre de saisons sèches, été sec ; marais sur les terres basses et plates au bord d'un fleuve ou d'un lac, en climat humide.
- Rendements : jungle et taïga 1/1/0 (2 PM, couvert boisé), savane 2/0/0, savane arborée et steppe 1/1/0, broussailles 1/1/1, marais 1/0/0 (3 PM, pas de charge).
- Couvert boisé (forêt, jungle, taïga) : +défense, pas de charge, tir gêné.
- Rendu : couleurs propres en 2D ; en 3D, feuillus, grands arbres de jungle, conifères, acacias, buissons, roseaux et flaques.
- Infobulle : climat « à été sec » ou « à N saisons sèches ».

## 0.1.0 « River Update » — 24 octobre 2026

Première version figée : monde généré (relief, climat, cours d'eau), exploration, villes, unités et combats.

### Monde
- Planète 192 × 111 cases de 40 km, pôle à pôle ; relief en 6 niveaux, montagnes et collines.
- Climat simulé à la génération (pression, vents, deux couches d'air, pluie, sols, courants marins, 4 saisons moyennées) ; terrain selon la température et l'aridité.
- **Trois passes climat ↔ eau** : à partir de la deuxième, les lacs évaporent et humidifient l'air ; **effet de lac** (air soulevé sur la rive sous le vent).
- Petites étendues de mer enclavées changées en cuvettes (lac qui déborde ou cuvette sèche).

### Cours d'eau et lacs
- Écoulement réel (m³/s) : lacs qui débordent ou sans issue, pertes de lit en climat sec, fleuves (cases d'eau entre deux berges) et rivières sur les arêtes.
- Lacs d'eau douce, lacs salés et mers intérieures ; fleuve absorbé par un lac qui l'entoure.
- Bassins versants (cases partagées hachurées, bassin fermé nommé d'après son lac) ; noms réels selon la latitude pour les fleuves, rivières, lacs et mers intérieures.
- Effets en jeu : traversée de rivière +2 PM, fleuve 3 PM, bonus de défense, eau douce +1 nourriture.
- Rendu 2D et 3D : lits taillés, berges, embouchures, cascades, plages, coupure nette au bord de la zone explorée.

### Cartes et interface
- Cartes normale, relief, climat, bassins versants, débits et réseau hydrographique (rang par rapport au déversoir, sens du courant animé).
- Surbrillance d'un cours d'eau ou d'un lac : affluents (orange puis jaune), récepteur (bleu), suite jusqu'à la mer (violet), chevrons du courant.
- Étiquettes des cours d'eau, lacs et villes selon le zoom, sans chevauchement.
- Panneau « Options de carte » (touche O) : étiquettes, mode de surbrillance, rubriques de l'infobulle pour chaque carte.
- Wiki en jeu (F1).

### Jeu
- Exploration automatique optimisée (36 éclaireurs ≈ 150 ms par tour), villes, production, unités, combats, camps hostiles, villages.
