# Structures du monde : villes, villages, camps

Rendu des structures posées sur la carte. Maquettes et échanges : la page « Structures du monde ».
Code : `settlements.rs` (pose, logique pure), `settlement_mesh.rs` (maillage), `view3d.rs` (branchement), `render.rs` (symbole 2D).

## Décisions de Benjamin

- **Ville qui grandit** : pas de fortification à la fondation. Elle grandit sans se reconstruire : un stade ne fait qu'ajouter des bâtiments.
- **Muraille** : elle dépend de la présence d'une fortification dans la ville, jamais de la population. Elle s'agrandit avec la ville, sans coût, sans faubourgs.
- **Matériaux** : ils dépendent du lieu de la ville. La gestion des civilisations n'est pas encore choisie.
- **Stades** : seuils provisoires (1-2 habitants : hameau ; 3-5 : bourg, comme le territoire de rayon 2 ; 6 et plus : ville), à revoir plus tard.
- **Camps** : type et nom liés. Fumée plus épaisse avant la sortie d'un guerrier, à essayer et à retirer si elle ne se remarque pas.
- **Villages et camps** : placement au hasard conservé pour ce chantier.
- **Pose** : aucune superposition ; pas de remblai injustifié ; une rive visible et une petite marge de construction au bord de l'eau.
- **Chemins** : discrets, sans bordure, ils ne desservent pas chaque maison. Un axe passe devant le grand toit, relie les grandes zones et, avec une muraille, traverse la ville entre les portes. Pas de chemin périphérique. Leur présence dépend de la taille et de la fortification.
- **Marais** : pilotis au-dessus des mares, galerie autour de chaque maison, passerelles alignées vers la terre, mares réduites quand la ville grandit, zones humides aussi près d'une rivière ou d'une côte.
- **Portes** : un bâtiment à cheval sur le mur, comme une porte de ville fortifiée (pas de château) : passage en arc et herse pour la pierre et la terre, linteau et vantaux pour le bois, deux tours pour la porte principale. La porte d'eau est dans l'axe du ponton, qui part de sa face extérieure.

## Algorithme (`settlements.rs`)

Tout se calcule dans le repère de la case, à travers le trait `Site` (hauteur, distance à l'eau) : le module se teste sans rendu. La distance à la mer et aux lacs se mesure sur le rivage tel qu'il est dessiné (`relief::Shoreline`, avec la déformation de `coast_warp`), pas sur le bord exact des cases.

1. **Limite de la ville** : un rayon par angle (72 angles) et par stade. Elle s'arrête avant la rive et le bord de la case et, sur un relief, cherche le tracé qui monte et descend le moins (programmation dynamique sur quatre rayons). C'est le tracé de la muraille, réservé même sans fortification : la construire ne déplace rien.
2. **Place centrale** (rayon 5,5) libre : la garnison s'y tient.
3. **Grand toit** au bord de la place, côté amont sur une pente, à la plus grande taille possible. **Axes des portes** réservés dès le hameau. Puis mât, grenier et halle, à une place réservée même s'ils ne sont pas encore construits.
4. **Maisons** sur une spirale fixe pour la case, stade après stade. Un emplacement est refusé s'il touche une emprise (débord du toit compris), l'eau, sa rive ou une marge au-delà, s'il sort de la limite, ou si le dénivelé sous lui dépasse la limite de son type (habitat léger 2,7 ; maison 4 ; pilotis 6,5).
5. **Portes** : deux, trois puis quatre vers la terre avec une muraille, plus une vers le ponton et une dans l'axe du pont. Sans muraille, deux directions.
6. **Chemins** : depuis un point de la place devant le grand toit, vers les sorties puis vers le grenier et la halle, en ligne droite ou avec un coude, sans traverser de bâtiment.

Le tirage vient d'une empreinte de la case : il ne touche pas au générateur du monde et ne change aucune carte.

## Maillage (`settlement_mesh.rs`)

Un seul maillage par site, couleurs portées par les sommets (au lieu d'une entité par pièce : 33 par ville, 17 par village auparavant). Le bas des murs suit `Relief::ground` point par point ; le haut se place par rapport à la hauteur moyenne sous le bâtiment. Le sol lui-même ne change pas, conformément à la décision de ne pas faire de terrasses.

Le sol de terre battue et les chemins partagent un maillage transparent : le bord se fond dans le terrain.

La pose est gardée dans `SiteModel` : quand le sol alentour est refait, le maillage suit les hauteurs mais les bâtiments ne bougent pas. Elle se calcule à la première exploration de la case, quand le terrain est connu.

## Familles de matériaux

Parts des 8 238 cases de terre, graine 777 seulement.

| Famille | Biomes | Terres | Murs et toits |
|---|---|---|---|
| Forêt tempérée | Forêt tempérée | 22 % | Bois sombre, bardeaux |
| Boréale | Taïga | 21 % | Rondins sombres, toit raide |
| Prairie | Prairie, Plaine | 17 % | Torchis clair, chaume |
| Aride | Désert | 11 % | Pisé ocre, toit plat |
| Froide | Toundra, Neige | 10 % | Pierre basse et tourbe, tentes de peaux |
| Tropicale | Jungle, Savane arborée | 7 % | Bois, palmes, pilotis |
| Méditerranéenne | Broussailles | 4 % | Pierre claire, tuile |
| Savane | Savane | 3 % | Terre, chaume conique |
| Steppe | Steppe | 3 % | Feutre, yourtes |
| Marais | Marais | < 1 % | Roseaux, pilotis |

## Camps

Un camp prend son type à la génération, d'après sa case (`CampKind::of`) ; son nom commence par le mot correspondant, pour lire dans le nom ce qu'on voit sur la carte.

| Type | Case | Début du nom | Allure |
|---|---|---|---|
| Fort | plate et dégagée | Fort | palissade de pieux jointifs percée d'une entrée, tour de guet |
| Repaire | relief (vallonné ou plus) | Repaire, Antre | adossé à des rochers du côté de la pente, entrée sombre, arc de pieux du côté ouvert |
| Campement | plate et boisée, marais | Camp | tentes basses, arc de pieux inclinés vers l'extérieur, trophée |

- **Silhouette** : des pointes partout (pieux, tentes coniques, rochers), pour se distinguer d'un village même sans couleur.
- **Rouge réduit** : seuls l'étendard, la tente du chef et le toit de la tour de guet le portent. Les autres tentes prennent les matériaux du lieu.
- **Sol** : terre piétinée au bord irrégulier, qui se fond dans le terrain. Les arbres restent autour d'un campement ou d'un repaire ; un fort dégage toute la case.
- **Fumée** (`CAMP_WARNING_TURNS` = 2) : dans les deux derniers tours avant la sortie d'un guerrier, elle est plus dense, plus large et monte plus haut. Elle reste un essai : à retirer si le signal ne se remarque pas en jeu.
- **Nom** : en 3D, il passe sous le site, pour ne masquer ni le feu ni les bâtiments.

## Marais

Pour la famille Marais, la pose change (`marsh_ponds`, `marsh_network`) :

- **Mares** : réparties sur la case, et touchant l'eau vive si elle est proche. Elles remplacent les flaques du décor. **Drainage** : elles rétrécissent au stade Bourg, et au stade Ville celles du cœur disparaissent. Les maisons ne bougent pas : leurs emplacements dépendent des mares d'origine, et elles gardent leurs pilotis.
- **Maisons** : sur pilotis, d'abord au-dessus des mares, jamais sur l'eau vive (limite à 3 de la rivière ou de la mer). Une galerie de planches fait le tour de chacune. Plus petites qu'ailleurs, car la galerie prend de la place.
- **Passerelles** : un réseau parti d'un palier sur la place, au niveau des planchers. Maisons et passerelles suivent les mêmes axes ; une passerelle va en ligne droite ou en équerre, s'arrête à une galerie ou se raccorde à une autre, sans traverser de bâtiment ni en croiser une autre. Si un détour est nécessaire, c'est en U, à angle droit ; le biais n'est que le tout dernier recours (aucun cas sur les sites de test). Un seul escalier descend du palier à la terre : sa place est réservée avant les bâtiments, du côté de la caméra de préférence.
- **Emplacements** (`marsh_spots`) : des rangées à peine décalées autour des axes, du centre vers le bord, puis la spirale. La spirale gaspillait la place entre des galeries rectangulaires. Le palier remplace la place (rayon 3,8) et le grand toit passe derrière elle, vu de la caméra. Effectifs visés : 3, 5 puis 7 maisons.
- **Muraille** : elle passe au-dessus des mares et ne s'arrête qu'à l'eau vive. Les passerelles s'arrêtent à la face intérieure d'une porte et repartent de sa face extérieure.

## Ponts

- **Où** (`find_bridge`) : sur un cours d'eau proche, à l'endroit où l'on arrive le plus près du centre, jamais sur la mer ni un lac (`Site::river_near`, adossé aux lits de `relief.channels`). Largeur d'eau limitée à 18. Écarté du ponton : à 11 au moins de son départ et de 0,4 rad autour du centre.
- **Forme** : perpendiculaire au courant, d'une terre ferme à l'autre (3,2 de la rive). Tablier bombé sur piles, en pierre sous une muraille de pierre, en bois sinon.
- **Quand** : le pont n'apparaît qu'au stade Ville, mais sa culée et l'axe de sa porte sont réservés dès le hameau : rien ne bouge quand il arrive.
- **Autre rive** : les cours d'eau suivent souvent le bord de la case. Le pont arrive alors dans la case voisine, et la tête de pont y est posée. Un chemin s'y perd.
- **Fortifiée** : la porte est là où l'axe du pont coupe la muraille, et une tour à la couleur du joueur garde l'autre rive.
- **Décor** : aucun effet sur les déplacements ni les combats, à décider plus tard.

## Vérifier

Script de commandes : `ville c l`, `ville population c l n`, `ville fortifier c l oui|non`, `vue 3d`, `camera 300`, `capture`.

## Limites

- La fortification n'a pas encore de règle de jeu : seule la console la pose. `CityState` ne produit que des unités ; il faudra une production de bâtiments et l'effet en combat (`conception-combat.md`).
- Une tête de pont dans une case voisine ne dégage pas ses arbres et n'en occupe pas la case : un arbre ou une autre ville peut s'y trouver tout près.
- Une ville fortifiée reste sur une seule rive.
- Sur un relief accidenté, les familles à habitat léger peuvent n'avoir que quelques bâtiments au stade Ville.

## Suite

- Vue lointaine : un camp reste petit à grande distance ; vérifier en jouant s'il lui faut une icône, comme aux unités.
- Effet de jeu du pont (traversée sans coût de rivière ?), à décider. Barques d'un village sans ponton.
- Animation de croissance dans `feedback.rs`.
