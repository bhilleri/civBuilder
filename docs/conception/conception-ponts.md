# CivBuilder — Conception des ponts (08/11/2026)

Aujourd'hui, le pont n'est qu'un décor : une ville au stade Ville en reçoit un, dessiné en 3D sur le cours d'eau le plus
proche de son centre (`settlements::find_bridge`, voir `conception-structures.md`), sans effet sur les règles. En pratique, il
franchit une rivière d'arête : un fleuve (16 à 29 de large, au milieu de la case voisine) n'est presque jamais à sa portée.

## Décisions de Benjamin (08/11/2026)
- **La ville construit ses ponts** : un pont est un ouvrage de la liste de production. Il n'est proposé que si la ville a un
  emplacement libre sur son territoire.
- **Partout sur son territoire**, sur une rivière comme sur un fleuve, plusieurs ponts par ville.
- **Le pont sert à tout le monde**, ennemis compris.
- **Déplacement** :
  - rivière : passer par le pont n'ajoute pas les +2 PM de la rivière ;
  - fleuve : entrer sur le pont coûte 1 PM ; l'unité peut s'y arrêter ou continuer (elle n'y est pas obligée).
- **Deux niveaux sur un fleuve** : le fleuve est navigable et le pont traversable. Une unité peut être sur le pont et une autre
  dans le fleuve, dans la même case sans être au même endroit. Un archer sur le pont peut tirer sur l'unité en dessous, avec le
  bonus de hauteur.
- **Combat** (valeurs harmonisées à partir des demandes de Benjamin) :

| Situation | Effet |
|---|---|
| **Unité sur le pont, attaquée en mêlée** | pas de flanquement, pas de charge |
| Infanterie ou lanciers sur le pont, en mêlée | +3 en défense (position défendable) |
| Lanciers sur le pont face à la cavalerie | +10 de plus (avec leur bonus habituel : +23 pour le lancier, +33 pour le piquier) |
| Archers sur le pont face à une unité à pied sans bouclier, en mêlée | +3 en défense (les assaillants n'arrivent qu'un par un) |
| Toute unité sur le pont, contre un tir | −3 en défense (exposée) |
| **Attaque depuis le pont** (sortie de pont), mêlée contre l'infanterie, les lanciers ou l'archer-épéiste | −5 |
| Cavalerie depuis le pont contre des lanciers | −10, pas de charge |
| Tir depuis le pont | −5 (difficile de tirer), sauf sur l'unité en dessous (bonus de hauteur) |
| Flanquement d'une attaque depuis le pont | seulement les alliés sur la terre ferme, à côté de la cible et à côté du pont |
| **Mêlée à travers une rivière par son pont** | pas de +3 « rivière à franchir » ; l'attaquant est en sortie de pont (−5, ou −10 et pas de charge) |

Familles : infanterie = milice, guerrier à bouclier, guerrier lourd ; lanciers = lancier, piquier ; archers = archer-épéiste,
archer expert ; cavalerie = cavalerie légère, cavalerie lourde, archer monté. Portent un bouclier : guerrier à bouclier,
lancier. Un fleuve et une rivière sans pont gardent les règles actuelles.

## Réponses de Benjamin (08/11/2026)
1. **Coût** : progressif selon le débit, de 20 (petite rivière) à 120 de production (grand fleuve).
2. **Emplacement** : le joueur le choisit. Une ville peut avoir plusieurs cours d'eau ; on peut construire un pont sur toutes
   les arêtes de rivière et toutes les cases de fleuve du territoire.
3. **Rivière** : une seule des deux cases sur le territoire suffit.
4. **Pas de décor qui contredit la réalité** : le pont décoratif des villes au stade Ville est retiré ; seuls les ponts
   construits (ou posés à la console) existent.
5. **Pont ou fleuve** : sur une case de fleuve avec pont, le joueur choisit de passer par le pont ou par l'eau (choix
   stratégique). Tant que les deux niveaux n'existent pas (étape 3), une unité qui entre dans la case par un bout du pont est
   sur le pont, sinon dans l'eau.
6. **Destruction ou prise** : plus tard.
7. **Ancrage sur les rives** : attention particulière à la façon dont le pont se pose sur la rive selon le relief, les arbres,
   les bâtiments.

## Étape 1 faite (08/11/2026)
- **Données** (`map::BridgeAt`, `WorldMap::bridge_site`, `set_bridge`, `bridge_between`, `fleuve_bridge`, `bridge_end`) : pont
  sur l'arête d'une rivière entre deux terres, ou en travers d'une case de fleuve (axe 0 à 2) entre deux terres fermes ; une seule
  écriture par pont ; un seul pont par case de fleuve (en poser un autre remplace le premier).
- **Déplacement** (`MoveCosts::step` de la carte) : rivière par son pont sans les +2 PM ; monter sur le pont d'un fleuve par un bout
  coûte `BRIDGE_MOVE_COST` (1 PM) sans arrêt (`StepCost::onto_bridge`). L'unité retient qu'elle est sur le pont
  (`UnitState::on_bridge`), d'après son dernier pas. En attendant les deux niveaux (étape 3), une unité qui entre dans la case
  par un bout du pont est sur le pont, sinon dans l'eau.
- **Combat** (`combat.rs`, constantes `BRIDGE_*` ; `battle::engagement`) : valeurs du tableau ci-dessus. Pour la cavalerie
  contre des lanciers, le −10 remplace le −5. Un attaquant qui va se déplacer avant de frapper est supposé arriver par le pont.
- **Console** : `pont col lig e|ne|no|o|so|se [oui|non]`.
- **Dessin** : 2D, planche brune bordée de sombre (`render::sync_bridges_2d`) ; 3D (`view3d::sync_bridges_3d`), tablier de bois
  bombé sur piles, **perpendiculaire au courant** (demande de Benjamin) ; sur un fleuve, les deux rives sont dans la case du
  fleuve : le pont la traverse d'une berge à l'autre. Chaque bout est repoussé vers sa rive tant que le sol y est sous l'eau
  (`anchor_bridge`). Arbres et rochers retirés à moins de 4,5 du tablier. Le pont d'une rivière qui borde une ville est dessiné
  par la pose de la ville (porte, tête de pont fortifiée).
- **Infobulle** et wiki (« Cours d'eau et lacs », Ponts).
- **Limites** : le pont n'existe pas encore pour les villes (pas de production) ; pont en bois partout hors des villes.

## Étape 2 faite (09/11/2026)
- **Production** (`rules::Production`) : une ville produit une unité ou un pont (`Production::Bridge { at, cost }`, coût fixé au
  choix). Coût (`rules::bridge_cost`) : 20 + 100 × ln(débit / 100) / ln(100 000 / 100), de 20 (rivière de 100 m³/s) à 120
  (100 000 m³/s), arrondi à 5 ; débit de l'arête, ou sortant de la case de fleuve. Exemples : 1 500 m³/s, 60 ; 20 000 m³/s, 95.
- **Emplacements** (`rules::bridge_spots`, `bridge_build::city_spots`) : arêtes de rivière dont une case au moins est au
  territoire, cases de fleuve du territoire (chaque traversée possible), sans les ponts déjà construits ni ceux qu'une autre
  ville produit.
- **Choix** (`bridge_build.rs`) : bouton « Pont… » du panneau de production (caché sans emplacement), mode choix de la carte,
  emplacements surlignés en 2D (traits jaunes) et en 3D (traits posés sur le sol), celui sous le curseur en blanc
  (`spot_under` : arête de rivière la plus proche du curseur, près du bord de la case ; sur un fleuve, la traversée dont la rive
  est vers le curseur). Clic gauche : la ville produit ce pont ; clic droit, nouveau clic sur le bouton ou autre sélection :
  annulé. Le stock de production est gardé, comme quand on change d'unité.
- **Fin de production** (`game::end_turn`) : le pont est posé sur la carte, message au journal, la ville revient à la milice.
- **Mode joueur pilotable** : `clic col lig côté` vise le bord de la case de ce côté (point du curseur, `HoveredPoint`).
- **Vérifié** (graine 777, partie pilotée) : Paris, 5 emplacements ; pont sur la rivière de 1 542 m³/s, 60 de production, posé
  au tour 13.
- **Limites** : sur un fleuve, la traversée choisie dépend du côté visé, mais le dessin 3D est toujours perpendiculaire au
  courant ; le pont ne coûte pas d'entretien ; il reste debout si la ville perd son territoire.

## Étape 3 faite (09/11/2026) : deux niveaux sur un fleuve
Décisions de Benjamin (09/11) : on ne quitte le pont (et on n'y monte) que par ses deux bouts ; un clic sur le tablier envoie
l'unité sur le pont, ailleurs dans la case dans l'eau ; entre les deux niveaux, seulement des tirs (du pont vers l'eau avec le
bonus de hauteur, de l'eau vers le pont sans bonus), pas de mêlée ; une barque passe sous un pont tenu par l'ennemi.
- **Occupation** (`map::slot`) : une case de fleuve avec pont a deux places, l'eau (la case) et le pont (une clé à part dans
  les mêmes ensembles `HashSet<Hex>` : la case décalée hors de toute carte). `Occupancy` et `Blocked` (ennemis visibles)
  rangent chaque unité à sa place : un ennemi sur le pont ne bloque que le pont.
- **Ordre** (`UnitState::order_layer`) : le pont (`Some(true)`), l'eau (`Some(false)`) ou selon l'arrivée (`None`). Depuis le
  pont, le trajet passe d'abord par le bout le plus proche du but ; pour monter sur le pont depuis l'eau ou le côté, il passe
  d'abord par le bout le plus proche de l'unité. Descendre dans l'eau depuis un bout coûte l'entrée dans le fleuve (3 PM,
  arrêt). Passer du pont à l'eau de la même case : par une rive.
- **Clic** (`input::PointedLayer`, `pointed_layer`) : à moins de 6 du tablier, le pont ; ailleurs dans la case, l'eau. En 3D,
  c'est le tablier dessiné (`Bridges3d::deck`), en 2D la planche. Sur une case à deux niveaux, on ne vise que l'ennemi ou
  l'unité alliée du niveau pointé ; le niveau libre est une destination.
- **Combat** : même case, tirs seulement (`shot_below` : +20 %, sans le malus du tir depuis le pont) ; mêlée contre le pont,
  ou depuis le pont, seulement depuis ou vers ses deux bouts. Le vainqueur d'une mêlée avance à la place de l'ennemi (sur le
  pont s'il y était).
- **Dessin** : 3D, l'unité du pont au milieu du tablier, à sa hauteur, en file, sans anneau ni barque ; celle de l'eau dans sa
  barque, à 9 du milieu du pont, en amont ou en aval. 2D : la première sur la planche, la seconde à côté.
- **Mode joueur pilotable** : `CIVBUILDER_PILOTE_DEBUG=1` autorise toutes les commandes de la console (préparer une scène).
- **Vérifié** (graine 777, partie pilotée) : archer expert monté sur le pont depuis un bout (1 PM, peut continuer), au-dessus
  d'une milice hostile dans l'eau ; au tour suivant, tir vers le bas (24 « depuis le pont +4 » contre 6, −63 PV).
- **Limites** : l'IA hostile ne choisit pas son niveau (elle suit l'arrivée) ; une unité alliée dans l'eau bloque la case pour
  la recherche de chemin qui la traverse (pas pour s'y arrêter sur le pont) ; l'animation 2D d'un déplacement finit au centre
  de la case avant de reprendre sa place.

## Étape 4 faite (09/11/2026) : matériaux et formes
Décisions de Benjamin (09/11) : matériau selon le lieu pour l'instant, la pierre partout plus tard (technologies, époques) ;
trois formes, arches de pierre, tablier de bois sur pilotis, pont haut en gorge ; tous les ponts laissent passer des
charrettes (un pont de cordes devrait aussi le permettre : pas fait) ; des ponts assez larges pour être bien vus, même hors
d'échelle, sans cacher la case ; la pierre coûte 1,5 fois le bois ; une tour garde l'autre bout si la ville qui a construit le
pont est fortifiée.
- **Matériau** (`WorldMap::bridge_stone`) : bois si l'une des deux rives est d'une famille à muraille de bois (forêt, taïga,
  jungle, savane, marais, steppe : `Family::wooden_wall`), pierre sinon. Coût (`rules::bridge_cost`) × `BRIDGE_STONE_COST`
  (1,5) pour la pierre.
- **Largeur** (`settlements::BRIDGE_HALF_WIDTH`) : tablier de 4,4 de large (2,5 avant), sur une case de 55 ; en 2D, planche
  de 6,4 bordée de sombre, grise pour la pierre, brune pour le bois.
- **Formes** (`settlement_mesh`, `Baker::bridge`) : le lit est le sol le plus bas sous le tablier ;
  - pierre : tablier à parapets, bombé de 0,9, sur une maçonnerie dont le dessous est une voûte par travée (une arche par 9
    de portée environ), piles jusqu'au lit, culées sur les rives ;
  - bois : tablier à garde-corps (main courante sur poteaux), bombé de 0,4, sur des rangées de trois pieux sous une traverse
    tous les 3,5 ;
  - gorge (rives à plus de 6 au-dessus du lit) : une seule portée presque plate ; une grande arche de pierre, ou des
    contrefiches de bois depuis chaque rive jusqu'au tiers du tablier. Les bouts montent en haut d'une berge raide
    (`anchor_bridge` : on remonte tant que le sol s'élève de plus de 0,35 par unité, jusqu'à 14 ; on y pose le bout si la berge
    s'élève de plus de 3).
- **Tête de pont** : `CityState::bridges` retient les ponts construits par la ville ; si elle est fortifiée, une tour à la
  couleur du joueur, du matériau de sa muraille, garde le bout le plus éloigné d'elle.
- **Vérifié** en captures (graine 777) : bois sur une rivière en forêt (120-121 73) et sur un fleuve de montagne (133 75),
  pierre sur des fleuves de désert et de plaine (132 71, 136 68), gorge (132 71 avec les deux rives relevées à 1 900 m).
- **Limites** : la caméra plonge et laisse peu voir les arches et les pieux ; pas de pont de cordes ; un pont construit garde
  son matériau.

## Étapes proposées
1. **Le pont dans les règles** : pont sur une arête de rivière ou une case de fleuve, posé à la console (`pont col lig dir`,
   comme la muraille), déplacement et combat, dessin en 2D et en 3D entre les deux rives, infobulle. Tests des règles.
2. **Le pont à construire** : premier ouvrage de la production, emplacements du territoire, choix de l'emplacement.
3. **Deux niveaux sur un fleuve** : une unité sur le pont et une dans l'eau ; tir vers le bas. Lié à la navigation (TODO).
4. **Matériaux, formes, positions** : bois, pierre selon le lieu et l'époque, tête de pont fortifiée.
