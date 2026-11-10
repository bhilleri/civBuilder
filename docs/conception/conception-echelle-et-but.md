# Conception — Échelle de temps, progression et but du jeu

## Objectif
Donner au jeu sa colonne vertébrale avant d'enrichir ses systèmes : durée d'une partie, sens d'un
tour, déplacements, population, ères, adversaires et fin de partie. Fait suite à l'analyse
critique du 10/11/2026 : les chiffres actuels (croissance, PM, coûts) étaient réglés pour des
tours de plusieurs années, alors qu'un tour vaut désormais un mois de saison
(`conception-saisons.md`).

## Décisions de Benjamin (10/11/2026)
- **Période de la première phase : du Néolithique au Moyen Âge.** C'est déjà beaucoup de contenu ;
  la suite (Renaissance et au-delà) sera vue plus tard.
- **Durée d'une partie : environ 500 tours** (≈ 42 années de saisons, une partie longue de 4X).
- **Temps de jeu abstrait** : le tour est un mois de saison pour la météo, les récoltes et les
  saisons ; croissance, construction, formation et recherche sont réglées **au tour de jeu**, pour
  le rythme, sans prétendre à des durées réelles. L'année historique affichée avance plus vite
  selon l'ère.
- **Déplacements entre les deux** : environ **4 à 8 cases par tour** sur terrain facile, un compromis
  entre la tactique actuelle (2 à 4) et des marches réalistes (10 à 15).
- **Quatre ères, passées par jalons** : Néolithique, Âge du bronze, Âge du fer et Antiquité,
  Moyen Âge. On passe à l'ère suivante en réunissant des conditions, pas à date fixe ; chaque
  peuple avance à son rythme.
- **Population en paliers croissants** : chaque point d'indice demande plus d'habitants que le
  précédent. Colons, soldats, migrations et famine se comptent tous en habitants.
- **Le défi viendra des IA** : d'autres peuples joués par l'ordinateur. Elles ne sont pas encore
  prévues : **il faut les prévoir** (tâche dédiée).
- **Fin de partie** : pas encore décidée ; elle se définira avec les IA.

## Propositions (chiffres à régler)
### Année historique affichée
Le joueur voit le mois et une année historique. L'année avance, à chaque année de jeu (12 tours),
d'un nombre d'années qui dépend de **l'ère du joueur** :

| Ère | Années historiques par année de jeu | ≈ tours (si ≈ 125 par ère) | Durée couverte |
|---|---|---|---|
| Néolithique | ≈ 600 | 125 | ≈ 6 200 ans |
| Âge du bronze | ≈ 200 | 125 | ≈ 2 100 ans |
| Âge du fer et Antiquité | ≈ 100 | 125 | ≈ 1 000 ans |
| Moyen Âge | ≈ 50 | 125 | ≈ 500 ans |

Départ vers −8 500, arrivée vers 1 400. Comme les ères se passent par jalons, un peuple en avance
voit l'année avancer moins vite : l'année reste indicative, chaque peuple a sa propre date.
- **Mode 2 tours par mois** : mêmes durées en années de jeu, donc deux fois plus de tours ; les
  taux par tour (croissance, construction, recherche) sont divisés par deux.

### Déplacements
- PM actuels multipliés par environ 2 (milice 4, cavalerie 8), coût des cases inchangé (montagne,
  forêt, neige et boue ralentissent ; les routes accéléreront).
- Combat : une attaque par tour et par unité, comme aujourd'hui ; la zone de contrôle garde son
  rôle (elle compte davantage quand les unités vont loin).
- À vérifier en jeu : traversée d'un continent (≈ 50 à 100 cases) en 1 à 2 ans de jeu, une campagne
  qui tient dans une belle saison.

### Population : indice et habitants
Table de départ (à régler) :

| Indice | 1 | 2 | 3 | 5 | 8 | 10 | 15 | 20 |
|---|---|---|---|---|---|---|---|---|
| Habitants | 500 | 1 200 | 2 000 | 5 000 | 12 000 | 20 000 | 50 000 | 100 000 |

- L'indice sert aux règles de taille (territoire, bâtiments, affichage) ; les flux (naissances,
  famine, migrations, colons, soldats) se comptent en habitants.
- Un colon emporte un groupe d'habitants (par exemple 300 à 500), une unité militaire selon son
  type (`conception-combat.md`).
- La population active (40 à 60 %, `conception-villes.md`) se calcule sur les habitants.

### Ères et jalons (exemples)
| Passage | Conditions possibles (à choisir) |
|---|---|
| Néolithique → Bronze | agriculture et élevage, une ville d'indice 3, métallurgie du cuivre et de l'étain (gisements, tâche 15) |
| Bronze → Fer et Antiquité | écriture, une ville d'indice 8, première institution de droit ou de pouvoir, métallurgie du fer |
| Fer et Antiquité → Moyen Âge | plusieurs villes d'indice 10, monnaie et impôt, institutions sur trois axes |

- Chaque ère débloque des aménagements, des bâtiments, des unités et des institutions.
- **Recherche** : un arbre court par ère (environ 5 techniques), dans lequel la géographie oriente
  (pas de métallurgie sans gisement accessible, pas de navigation sans côte).
- Les jalons demandent à la fois de la recherche, de la culture (institutions) et du développement
  des villes : aucune voie unique.

### IA des peuples rivaux (à concevoir)
- Plusieurs peuples sur la même planète, partis d'endroits différents, qui suivent les mêmes règles
  que le joueur (villes, aménagements, recherche, ères, armée).
- Besoins : une IA de développement (où fonder, quoi aménager, quoi construire), une IA militaire
  (défense, campagnes saisonnières), plus tard la diplomatie et le commerce.
- Elle donnera leur sens à des décisions déjà prises : armes vendues à d'autres peuples,
  diplomatie, commerce, attractivité et migrations entre pays.
- En attendant, la faction hostile reste l'adversaire ; elle pourra grandir avec le temps.

### Fin de partie (à décider avec les IA)
Pistes :
- score de développement à la fin des 500 tours ou à la fin de l'ère médiévale ;
- jalons ou ambitions à atteindre (comme dans Old World) ;
- défaite par effondrement (plus de ville, instabilité totale) ;
- partie libre après la fin, le score servant de repère.

## Conséquences sur les autres docs
- `conception-saisons.md` : taux au tour de jeu ; mode 2 tours par mois avec taux divisés par deux.
- `conception-villes.md` : population en habitants, table indice ↔ habitants.
- `conception-combat.md` : PM environ doublés ; effectifs en habitants.
- Règles actuelles (`rules.rs`) à recaler : seuils de croissance, coûts des unités, soins, PM.

## Questions ouvertes
- Fin de partie (avec les IA).
- Chiffres : années par ère, table de population, PM, jalons exacts, nombre de techniques.
