# Journal des versions

## 0.3.0 (en cours)

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
- Les plaques ne façonnent pas encore le relief (étape suivante : continents et chaînes de montagnes).

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
