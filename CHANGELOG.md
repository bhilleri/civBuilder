# Journal des versions

## 0.4.0 (en cours)

### Technique
- **Passage à Bevy 0.20.1** : champs de saisie avec `TextInput`, plus d'`Interaction` ni de `Button`, dépréciés : le module `pointer.rs` tient l'état du pointeur (`UiPointer`) à partir du picking de Bevy.
- Textes d'aide du menu alignés à gauche : la mise en page du texte a changé avec Bevy.
- Seuls les éléments cliquables, les panneaux et les champs réagissent au pointeur ; un élément recouvre maintenant ce qui est dessous (un nom de la carte sous un panneau n'est plus survolé).

## 0.3.0 « Planet Update » — 4 novembre 2026

Le monde devient une planète : une carte sans bord façonnée par la tectonique, des climats, des mers et des cours d'eau plus réalistes, des lieux nommés à toutes les échelles, une vue 3D plus vivante et des actions mises en scène. Les mondes changent, à graine égale.

### Planète et relief
- **Carte sans bord** : elle fait le tour de la planète d'est en ouest, sans raccord visible, en 2D comme en 3D, et toutes les règles en tiennent compte. Seuls les pôles restent des bords, bordés de mers navigables ; au moins une mer fait le tour du monde.
- **Plaques tectoniques** : une douzaine de plaques nommées, en mouvement (1 à 9 cm/an), avec leurs frontières (collision, subduction, arc d'îles, rift, dorsale, faille transformante) et leurs points chauds. Le relief en découle : chaînes, fosses, arcs et chapelets d'îles. Nouvelle carte « Plaques ».
- **Continents variés** : 4 à 6 grands blocs de tailles inégales et quelques microcontinents, indépendants des plaques, aux côtes découpées (golfes, péninsules, bras de mer). Un rift devient, selon sa vitesse d'écartement, une vallée à lacs, un bras de mer ou un jeune océan.
- **Érosion** : l'eau creuse le relief avant la pose des cours d'eau (vallées, cols, cuvettes comblées) ; beaucoup moins de lacs.
- **Altitude en mètres** : elle devient la référence (déplacements, combat, climat, infobulle) et le sol 3D la suit. 9 niveaux au lieu de 6, plus de plaines basses (~70 % des terres sous 1 000 m), chaque monde à sa façon.
- **Relief propre à chaque case** : plat, vallonné, accidenté, montagne ou pic, en plus du biome (le terrain « Montagnes » disparaît). Nouvelles cartes « Altitude » et « Relief ».
- **Fonds marins** calculés d'après les plaques : plateau continental, talus, abysses et fosses.

### Climat et océans
- **Courants marins** poussés par les vents simulés : tourbillons, courants rapides de bord ouest, courant circumpolaire, dérive chaude vers les hautes latitudes. Ils donnent la température de la mer, donc la douceur, la pluie ou les déserts des côtes. Nouvelle carte « Courants marins ».
- **Un monde moins sec** : orages tropicaux, mousson, dépressions d'hiver sur les mers chaudes, vent calmé à l'équateur. Déserts mieux placés et moins étendus (de 13-27 % à 11-19 % des terres selon la graine), plus de jungle.
- **Marées** simulées à la génération (eau peu profonde, Lune, Coriolis, frottement) : marnage, heure de la pleine mer et courant de marée, points sans marée, golfes qui résonnent. Nouvelle carte « Marées », avec lignes d'heure et sens de la marée en option. Leurs effets en jeu restent à décider.

### Cours d'eau
- **Tracés naturels** : méandres sur les fleuves, rivières d'un seul trait raccordées de coin en coin, largeur liée au débit et variable le long du cours.
- **Couleur de l'eau** selon le débit et les terres traversées : limoneuse en climat sec, claire en climat froid, mêlée aux confluences.
- **Rapides et chutes** : rapides composés dans le sens du courant, chutes d'eau qui épousent le relief.
- **Îles fluviales** sur les fleuves de plaine, bancs de sable ou îles boisées, seules ou par groupes, avec bras secondaires.
- **Raccords soignés** : embouchures évasées, affluents qui rejoignent le fleuve en biais et s'y fondent, berges continues d'une case à l'autre.

### Noms et géographie
- **Langues du monde** : huit langues en partie inventées, réparties selon les régions, remplacent les noms réels ; noms descriptifs, grammaire française et origine du nom dans l'infobulle.
- **Tout est nommé** : océans, mers, golfes, baies et détroits ; continents, îles, archipels, péninsules et isthmes ; chaînes, massifs, cols, plateaux et collines ; forêts, déserts, steppes et autres régions naturelles ; villages, camps et bancs de poissons.
- **Noms lisibles** : ils apparaissent à leur niveau de zoom, avec un quota par rang pour garder une densité régulière, et se courbent le long des chaînes et des golfes allongés.
- **Carte « Géographie »** : le découpage en zones nommées (terres, mers, relief ou régions), une couleur par zone, de plus en plus détaillé en zoomant.
- **Trouver les lieux** : recherche (Ctrl+F) avec filtres et vol de la caméra ; noms, légendes et étiquettes cliquables ; barre de lieu qui situe le centre de la vue (« Inetsie › Jungle de Shuchets › Bassin du Smurask »).

### Règles du jeu
- **Les montagnes coûtent du temps** : 4 à 8 PM selon l'altitude, avec une dette de PM reportée sur les tours suivants ; la cavalerie paie double, l'éclaireur les deux tiers. Cols, vallées et contournements deviennent les bons passages. Une pastille indique les tours d'immobilisation.
- **Terrain et pentes** : relief accidenté +2 PM ; montée de 400 à 900 m +1 PM, au-delà +2 PM ; falaises (1 000 m) et pics infranchissables.
- **Combat en hauteur** : être plus haut d'au moins 250 m avantage en défense de mêlée, au tir et en portée ; montagne +6 et relief accidenté +3 en défense.
- **Pêche** : eaux peu profondes 2 nourriture, haute mer 1. Eaux poissonneuses (+1) sur les remontées d'eaux froides, les monts sous-marins, autour des îles volcaniques et à la rencontre des courants, regroupées en bancs sur la nouvelle carte « Ressources ».

### Mise en scène et interface
- **Déplacements et combats animés** : les unités glissent de case en case ; élans, tirs, impacts et PV perdus en chiffres flottants ; récompenses affichées sur les villages et camps.
- **Fin de tour lisible** : les actions hostiles visibles se jouent l'une après l'autre, et le tour suivant attend la fin des animations (sauf en vitesse Instantanée).
- **Journal hiérarchisé** : cinq niveaux d'importance, flash selon la gravité, noms cliquables qui amènent la caméra sur place.
- **Menu Échap en pages** : Paramètres (vitesse des animations, mode debug, enregistrés d'une partie à l'autre) et Nouveau monde (graine à taper, copier, coller ou tirer au hasard).
- **[Comment se joue CivBuilder](docs/ANIMATIONS.md)** : un GIF par action, en 2D et en 3D, tourné par le jeu lui-même.

### Vue 3D
- **Relief modelé dans le sol** : crêtes et ravines continues, roche, neige et limite des arbres, montagnes reliées en chaînes, pics en aiguille.
- **Figurines** propres à chaque type d'unité, en petits groupes pour les militaires, posées sur le relief, en barque sur les fleuves, plus grandes et surmontées d'une icône de loin.
- **Villes et camps qui suivent le terrain**, sans terrasse.
- **Plus fluide** : relief de toute la carte construit en ≈ 7 s au lieu de ≈ 19 s, 60 images/s tenues pendant la construction, plus de saccades en zoomant en se déplaçant ou en changeant de carte.
- **Correction** : plus de lits de cours d'eau à la hauteur d'eau périmée (trous noirs, marches) en explorant.

### Outils de développement
- **Console de debug** (touche ², en mode debug) : observer une case, chercher des cases par critères, modifier le terrain (brut ou avec recalcul local `~`), refaire l'eau ou le climat, préparer une partie (`unite`, `ville`, `tour`, `chemin`), avec complétion par Tab.
- **Scripts de commandes** (`CIVBUILDER_SCRIPT`) pour rejouer un cas sans toucher au code ; `verifier` liste les incohérences d'une carte (aucune sur une carte générée).
- **Modes capture** (`CIVBUILDER_CAPTURE`) : vues PNG du rendu, de la mise en scène et de l'exploration, sans jouer.
- **Nouveaux bilans chiffrés** : `bilan_terres`, `bilan_relief`, `bilan_regions`, `bilan_courants`, `bilan_marees`, `bilan_aridite`, `bilan_montagnes`, `bilan_verifier`, `formes_des_terres`…
- **Compilation plus rapide** : `scripts/amorcer-target.ps1` réutilise les dépendances déjà compilées dans un nouveau worktree.

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
