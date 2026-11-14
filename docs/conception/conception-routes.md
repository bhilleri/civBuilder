# Routes

Les chemins et les routes que construisent les villes : leur effet sur le déplacement des unités et
des porteurs des échanges, leur coût, leur tracé, leur construction. Les ponts ont leur propre doc
(`conception-ponts.md`) ; les échanges, la leur (`conception-ressources.md`).

## Ce qui était prévu
- La recherche (`conception-recherche.md`) : « Roue et chariot » donne des charrettes et des routes
  de terre ; « Routes pavées » donne des routes rapides.
- Les villes (`conception-villes.md`) comptent les routes parmi leurs aménagements, et la perte du
  commerce selon la distance et les routes.
- Au Néolithique, il existait déjà des **chemins aménagés** : le Sweet Track, chaussée de bois sur un
  marais anglais (vers 3800 av. J.-C.), des pistes damées et balisées. C'est le premier niveau.

## Décisions de Benjamin (14/11/2026)
- **Déplacement** : une case de route coûte 1 PM sur le plat, quel que soit le biome, et monte un peu
  avec le dénivelé (route sinueuse). En montagne, le gain reste important, mais la route n'annule pas
  la difficulté de la traversée.
- **En plaine, pas de gain pour les unités** (1 PM déjà, le minimum), un gain pour le commerce : les
  porteurs y vont deux fois plus vite.
- **Construction** : un projet de la ville, hors du territoire aussi. Le projet montre le coût total,
  mais la route se construit au fur et à mesure entre les deux bouts ; ce qui a déjà une route n'est
  pas reconstruit.
- **Tracé** : le plus rapide est proposé, mais le joueur peut le changer (modèle : le traceur de
  route d'Europa Universalis V).
- **Destination libre** : une ville ou n'importe quelle case.
- **Fleuve** : en construisant la route, on construit le pont (à son coût habituel).
- **Coût** selon le biome (marais, forêt, jungle plus difficiles que la prairie ou la plaine) et le
  relief, qui pèse fortement.
- **Chemin dès le départ**, sans recherche ; **améliorer** une route est aussi un projet de ville, qui
  coûte moins cher que la construire (le tracé est là) : 50 %.
- Les chiffres seront revus à la passe d'équilibrage, une fois les fonctionnalités en place.

## Règles et chiffres
**Déplacement** (`WorldMap::route_step`, appelé par `WorldMap::step` quand un tronçon relie les
deux cases) :

| Case d'arrivée | Sans route (unité) | Chemin (unité) | Porteur sans route | Porteur sur chemin |
|---|---|---|---|---|
| Plaine, prairie, steppe | 1 | 1 | 1 | 0,5 |
| Forêt, jungle, taïga | 4 | 1 | 2 | 0,5 |
| Marais | 6 | 1 | 3 | 0,5 |
| Collines | 3 à 6 | 2 | 1,5 à 3 | 1 |
| Relief accidenté | 5 à 6 | 3 | 2,5 à 3 | 1,5 |
| Montagne | 8 à 16 | 4 à 8 | 4 à 8 | 2 à 4 |
| Montée de 400 à 900 m / au-delà | +2 / +4 | +1 / +2 | +1 / +2 | +0,5 / +1 |
| Rivière | +2 (arrêt) | 0 (passerelle) | +1 | 0 |

Le coût de montagne propre à chaque unité s'applique toujours (la cavalerie le paie double).
Exemples : un col de trois cases à 2 500 m prend 9 tours à un fantassin, 4,5 par un chemin ;
six cases de forêt entre deux villes, 3 tours de porteur, 0,75 par un chemin (transport 15 % → 4 %).

**Coût de construction** (`routes::cout_case`, `cout_troncon`) : une case vaut 4 de production ×
biome (prairie, plaine, steppe, savane, désert, rives 1 ; broussailles, savane arborée, toundra 1,5 ;
forêt, taïga 2 ; jungle, marais, neige 3) × relief (plat 1, collines 1,5, relief accidenté 2,5,
montagne 4, 5 au-dessus de 2 500 m). Un tronçon coûte la moitié de chacune de ses deux cases, plus
le pont s'il entre dans un fleuve sans pont ; rien s'il a déjà ce niveau ; la moitié pour améliorer.
Exemples : 10 cases de plaine, 40 ; 6 cases de forêt en colline, 72 ; un col de 3 cases, 48 à 60.

**Tracé** (`routes::tracer`) : Dijkstra sur les cases, temps une fois construit d'abord (PM des
unités), puis coût de construction au dixième ; seulement par les cases explorées, ni eau, ni glace,
ni pic, ni falaise ; un fleuve se franchit tout droit, d'une rive à la rive opposée (le pont est dans
cet axe, ou y sera construit). Les points de passage découpent le tracé en morceaux ; un nouveau point
se range là où il allonge le moins le chemin à vol d'oiseau (`route_build::rang_du_point`).

**Chantier** (`routes::Chantier`, `CityState::chantiers`, commande `Production::Route { id, cost }`) :
la file ne le termine jamais d'elle-même ; à chaque tour, `game::end_turn` paie avec la production
investie les tronçons suivants dans l'ordre (`Chantier::avancer`), les pose sur la carte (et les
ponts), puis met à jour le coût restant ; fini, la commande sort de la file et le reste de la
production passe à la suivante. Retirer la commande de la file abandonne le chantier (ce qui est fait
reste).

**Carte** : les tronçons (`map::RouteAt`, arête entre deux cases voisines, rangée sous la case de plus
petit indice) et leur niveau ; ils vident le cache des coûts de pas ; enregistrés dans la sauvegarde.

## Interface
- « Chemin… » dans la construction de la ville (groupe Bâtiments) : l'écran se ferme, la carte passe
  en mode tracé (`route_build.rs`). Survol : tracé jusqu'à la case ; clic : arrivée fixée, puis points
  de passage ; clic droit : retirer le dernier point (ou sortir) ; Entrée : lancer. Panneau en haut :
  coût, ponts compris, tours de production, porteur sur ce tracé avant → après.
- Tracé en jaune (trois traits), ses tronçons déjà faits en gris ; chemins construits en ocre (deux
  traits fins en 2D, posés sur le relief en 3D), seulement entre des cases explorées.
- File : « Chemin », avec « jusqu'à … : n tronçons sur N faits, coût annoncé ».
- Console : `route col lig col lig` pose un chemin d'un coup (tracé le plus rapide).

## Vérifié (14/11/2026, graine 777)
Paris fondée, chemin de 7 tronçons par un point de passage (coût 71, porteur 4,6 → 1,6 tour) ; deux
tronçons faits au tour 4, chemin achevé au tour 10 ; unités à 2 PM par case de colline boisée sur le
chemin ; partie enregistrée puis rechargée, chemin redessiné. Tests : `routes::tests` (8),
`route_build::tests`.

## Limites et suite
- Un seul niveau (chemin) ; routes de terre (Roue et chariot : capacité des charrettes, plus que les
  4 unités par tour à pied) et pavées (gain pour les unités en plaine) avec la recherche ; le projet
  « Améliorer » viendra avec elles (coût déjà prévu : 50 %).
- Pas d'entretien (avec l'économie : une route non entretenue redeviendrait un sentier), pas de
  pillage, pas de territoire étranger (diplomatie).
- On ne peut pas déplacer un point de passage déjà posé (le retirer puis en poser un autre).
- Le tracé ne passe que par des cases explorées ; il ne tient pas compte des chantiers en cours des
  autres villes.
- Une route ne donne pas encore de capacité aux étapes des échanges, seulement de la vitesse.
