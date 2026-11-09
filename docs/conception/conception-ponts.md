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

## Étapes proposées
1. **Le pont dans les règles** : pont sur une arête de rivière ou une case de fleuve, posé à la console (`pont col lig dir`,
   comme la muraille), déplacement et combat, dessin en 2D et en 3D entre les deux rives, infobulle. Tests des règles.
2. **Le pont à construire** : premier ouvrage de la production, emplacements du territoire, choix de l'emplacement.
3. **Deux niveaux sur un fleuve** : une unité sur le pont et une dans l'eau ; tir vers le bas. Lié à la navigation (TODO).
4. **Matériaux, formes, positions** : bois, pierre selon le lieu et l'époque, tête de pont fortifiée.
