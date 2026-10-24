# Journal des versions

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
