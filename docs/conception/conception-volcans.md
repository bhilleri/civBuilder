# Conception — Volcans

## Objectif
Ajouter des volcans placés d'après la tectonique déjà simulée, avec une forme propre à chaque type
(dôme, caldeira, stratovolcan, bouclier…), visibles en 3D, et cohérents avec le climat, l'eau,
la météo et les règles de jeu.

## Décisions de Benjamin (09/11/2026)
- **Placement par la tectonique** : on a déjà ce qu'il faut (`plates.rs` : subductions, arcs
  d'îles, rifts, dorsales, points chauds et leurs chapelets).
- **Formes selon le type de volcan**, et un rendu 3D qui les montre.
- **Sols fertiles, mais pas une fertilité qui s'améliore à l'infini** comme dans Civilization.
  La fertilité des sols n'est pas encore traitée : elle fera l'objet d'une tâche à part (sols
  volcaniques, bassins sédimentaires…), précédée d'une recherche sur les conditions qui font la
  fertilité d'un sol. Les volcans en seront l'une des sources.
- **Ressources** : plus tard, mais liées aux volcans (obsidienne, soufre…). Pas forcément la
  pierre, trop commune, même si l'idée est bonne.
- **Passage** : il dépend du relief que le volcan génère. Pas de règle propre : un volcan passe
  par les règles de déplacement existantes (altitude, montagnes par coût, pics), selon sa hauteur
  et ses pentes.

## Ce qui existe
- `plates.rs` : nature des frontières (`Boundary` : `Collision`, `Subduction`, `IslandArc`,
  `Rift`, `Ridge`), distance à la frontière (`boundary_km`), vitesse de chaque case, 3 à 6
  points chauds (`hotspots`) loin des frontières, plutôt sous un océan, avec leurs chapelets d'îles.
- `map.rs` : monts sous-marins et îles volcaniques isolées (pêche).

## Proposition (à valider)
### Type selon le contexte tectonique
| Contexte | Volcans | Forme |
|---|---|---|
| Subduction (océan sous un continent) | Chaîne de stratovolcans à 150-300 km de la fosse, parallèle à la côte ; caldeiras rares | Cône aux flancs concaves, petit cratère, ravines, neige au sommet |
| Arc d'îles | Îles-stratovolcans, caldeiras en partie noyées (comme Santorin) | Cône sortant de la mer, anneau ouvert sur la mer |
| Point chaud océanique | Bouclier actif, puis îles plus vieilles et érodées le long du chapelet, puis atolls (sous les tropiques) et monts sous-marins | Dôme très large aux pentes douces (2 à 4 cases), caldeira peu profonde, coulées sombres |
| Point chaud continental | Grande caldeira ; plateaux de basalte s'il est ancien | Rebord en anneau, fond plat souvent occupé par un lac ; plateaux en gradins |
| Rift continental | Grands volcans isolés (comme le Kilimandjaro), champs de petits cônes, cratères emplis d'eau | Cônes, petits lacs ronds |
| Dorsale | Émergée seulement avec un point chaud dessus (comme l'Islande) | Boucliers, fissures, plateaux |
| Collision | Presque pas de volcans | — |

- **Activité** : actif près des frontières qui bougent et au-dessus du point chaud, endormi,
  puis éteint et érodé le long du chapelet (sens donné par le mouvement de la plaque).
- **Échelle** : la base d'un stratovolcan fait 30 à 40 km, soit environ une case. Un bouclier en
  couvre plusieurs. Les dômes de lave et les petits cônes sont du décor dans une case.
- **Déterminisme** : tirages par la graine, comme le reste de la génération.

### Cohérence avec le reste du monde
- **Les volcans entrent dans l'altitude de la génération**, avant le climat et l'eau : pluie de
  relief, neige, lacs de cratère par les bassins (`basins.rs`).
- **Les mondes changent à graine égale** : comparer `empreinte_generation` avant et après.
- **Passage** : par les règles de déplacement existantes (voir les décisions).
- **En 3D**, les volcans sont des formes de relief dans la carte des hauteurs du terrain continu
  (`conception-rendu-3d.md`, étape 9 a). Matières propres : basalte noir, cendres grises, coulées
  récentes, flancs anciens couverts de végétation. Fumerolles sur les volcans actifs.
- **Éruptions** (plus tard, après la tâche des saisons) : elles pourront nourrir le fond lent de
  la météo (refroidissement durable, hiver volcanique, `conception-saisons.md`). C'est le même
  moteur, et l'ensemble reste cohérent.

### Noms et interface
- Toponymie (`toponymy.rs`) : mont, piton, caldeira, champ volcanique, selon le type et la langue de la région.
- Infobulle : type, activité. Wiki : une page sur les volcans. Carte thématique (plaques) : volcans par type.

## Bilans chiffrés à ajouter
- `bilan_volcans` : nombre par type et par contexte, part des volcans actifs, hauteur et
  emprise, sur les graines 777, 2 et 5.
- Ordres de grandeur réels pour comparer : environ 1 350 volcans potentiellement actifs sur Terre,
  dont la grande majorité le long des subductions (ceinture de feu du Pacifique).

## Éruptions (décidé le 10/11/2026)
D'abord des volcans géologiques (forme, sols, noms). Les éruptions viendront **plus tard, après
la météo**, avec les autres risques naturels (tâche 21) : coulées, cendres sur les récoltes, hiver volcanique.

## Fertilité (décidé le 10/11/2026)
Pour l'instant, un **bonus fixe selon le type de sol** (sols volcaniques, bassins sédimentaires,
limons déposés par les fleuves…), fondé sur une recherche (tâche 12). Pas d'épuisement ni de
régénération pour le moment.

## Questions ouvertes
- Effets de jeu de la fertilité : attendre la tâche « Fertilité des sols ».
- Ressources liées aux volcans : à décider avec les ressources.
