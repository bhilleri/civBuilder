# Période nomade

Le début de partie : le peuple est une bande de chasseurs-cueilleurs, qui erre jusqu'à découvrir
l'Agriculture, puis fonde sa première ville. Tâche 22 du TODO, étape 6 de la feuille de route.
Code : `nomade.rs` (logique pure), étape 1b de `game::end_turn`, fiche dans `ui.rs`.

## Ce qui était décidé
- 10/11 (`conception-echelle-et-but.md`, révision 2) : tous les peuples partent nomades ;
  l'**Agriculture est requise** pour fonder la première ville ; elle s'observe en errant (plantes et
  animaux), sans plafond, et la bande produit un peu de Recherche (`conception-recherche.md`).

## Décisions de Benjamin (14/11/2026)
- **Refus d'un écart fort entre peuples** : si la découverte dépendait beaucoup de la région, les
  premiers peuples à fonder auraient un énorme avantage sur les derniers.
- Retenu : **départs équitables + fenêtre étroite + rattrapage** (et non la sédentarité avant
  l'agriculture, qui aurait levé la règle du 10/11).
- Proposition acceptée telle quelle pour le reste : une **bande** (≈ 1 000 personnes) et son
  **éclaireur** ; **épuisement** des cases cueillies pour obliger à bouger ; troupeaux qui migrent
  et animaux dangereux **reportés** (saisons, barbares).

## Règles et chiffres
**Départ** : une bande (`UnitKind::Bande`, 1 000 membres comme un colon, 4 PM, vue 2, civile, ne se
produit pas) et un éclaireur ; plus de colon ni de milice. La ville fondée (B) reprend les membres
de la bande (`UnitState::habitants_emportes`). Fonder une ville, quelle qu'en soit la manière
(console, captures), rend l'Agriculture connue.

**Départ équitable** (`WorldMap::find_start`) : parmi les emplacements classés comme avant (terres,
rendements, centre de la carte), le premier qui a au moins 3 sortes domesticables à 5 cases, dont
une céréale et une bête (`nomade::depart_equitable`) ; sinon le meilleur.

**Sortes domesticables sauvages** (`sortes_sauvages`) : la céréale d'un champ et les légumineuses
(8 à 24 °C) là où un champ est possible, hors forêt et taïga (terres ouvertes ; tubercules dans la
jungle et la savane arborée) ; le bétail d'un pâturage (porcs en forêt, rennes dans la taïga, lamas
au-dessus de 2 500 m, chameaux au désert…).

**Nourriture** (`nourriture_bande`) : la moitié des membres travaille, à parts égales sur la case
et ses voisines (cueillette, exposant 0,7), × (1 − épuisement). Croissance et famine comme une
ville (1,5 % par tour avec 50 % de surplus ; 2 à 5 % de pertes), plancher 300.
**Épuisement** (`Nomadisme::epuiser`) : chaque tour, une case cueillie perd 40 % de ce qui lui
reste ; toutes repoussent de 10 % (les trois quarts en un an). Sur place : 1,0 → 0,40 → 0,62 →
0,73 → 0,80 → 0,83 d'épuisement ; une bande de 1 000 qui cueille 5,5 a faim au cinquième tour.

**Agriculture** (`Savoir::apport`, 100 points) :

| Source | Points |
|---|---|
| Chaque tour (Recherche de la bande) | 1,2 |
| Par sorte domesticable sur la case ou une voisine (4 au plus) | 0,4 |
| Chaque sorte vue la première fois (à 2 cases) | 5, une fois |
| Diffusion : par ville d'un autre peuple (hors faction hostile) à 8 cases ou moins | 1,5 (3 au plus) |

## Bilan (`bilan_nomade`, 14/11/2026)
Sur la planète du jeu (192 × 111). Bande qui erre vers ce qu'elle n'a pas vu (2 cases par tour au
plus) ou qui reste sur place ; départs quelconques (une case de terre sur 400, en errant) pour comparer :

| Graine | Départ | Sortes à 5 cases | Errant | Sur place | Départs quelconques (médiane, max) |
|---|---|---|---|---|---|
| 777 | 121 75 | 8 | 22 tours (1 386 membres) | 29 (466) | 20, 39 |
| 2 | 109 69 | 8 | 18 (1 306) | 25 (640) | 24, 60 |
| 5 | 108 56 | 8 | 19 (1 326) | 29 (582) | 22, 34 |
| 4242 | 71 50 | 8 | 22 (1 386) | 24 (656) | 22, 99 (jamais) |

Les peuples ne sont séparés que de quelques tours (18 à 22 en errant), alors qu'un départ quelconque
irait jusqu'à 40 ou 60 tours, ou ne trouverait jamais rien. Rester sur place retarde de 3 à 10 tours
et coûte 35 à 55 % de la bande. Réglage : 1,6 point de base donnait 16 à 19 tours, trop vite pour la
fenêtre de 20 à 30 tours.

**Vérifié en jeu** (pilote, graine 777) : fondation refusée avant l'Agriculture ; six tours sur place
→ cases épuisées à 85 %, famine dès le cinquième tour ; déplacée vers les prairies, la bande observe
blé, millet, légumineuses et bœufs ; Agriculture au tour 21 malgré l'attente ; ville fondée au tour 22
avec 1 213 habitants. Figurines 3D vérifiées (graine 2).

## Interface
- Fiche de la bande (sélection) : membres, cueillette et besoin, croissance ou famine, épuisement
  des cases autour, Agriculture (points, apport ici, tours restants), sortes observées, tout près,
  à observer au prochain tour.
- Infobulle d'une case de terre tant que l'Agriculture n'est pas connue : sortes à l'état sauvage,
  épuisement de la cueillette.
- Journal : sortes observées (avec les points), faim, découverte de l'Agriculture.
- Figurines 3D : trois personnes en peaux, bâton et ballot ; icône à trois silhouettes.
- Wiki : « Période nomade ». Console : `agriculture <points>`.

## Limites et suite
- Une seule bande ; la scinder (plusieurs groupes) plus tard.
- La diffusion ne joue qu'avec des peuples rivaux, qui n'existent pas encore (étape 9) ; leurs
  bandes suivront les mêmes règles (`Nomadisme` est par peuple).
- Pas de troupeaux qui migrent avec les saisons (tâche 10), pas d'animaux dangereux (étape 8).
- La Recherche de la bande ne va qu'à l'Agriculture ; l'arbre de recherche (étape 7) la reprendra.
- Pas de camp saisonnier ni de stock de nourriture de la bande.
