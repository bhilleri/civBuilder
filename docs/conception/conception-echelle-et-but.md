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
- **Le défi viendra des IA** : d'autres peuples joués par l'ordinateur (conçues depuis :
  `conception-ia.md`, tâche 31).
- **Fin de partie** : décidée le 10/11 (voir « Fin de partie et victoire »).

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
- **Mode 2 tours par mois** (révision du 11/11/2026, remplace la règle précédente) : **500 tours
  dans les deux modes**, mêmes taux par tour ; seul le calendrier avance deux fois moins vite
  (≈ 21 années de saisons). Voir `conception-saisons.md`, révision 3.

### Déplacements
- PM actuels multipliés par environ 2 (milice 4, cavalerie 8), coûts du relief doublés aussi, voir `conception-combat.md` (montagne,
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
- Un colon emporte un groupe d'habitants (500, de quoi fonder une ville d'indice 1), une unité militaire selon l'ère et son
  type (`conception-combat.md`).
- La population active (40 à 60 %, `conception-villes.md`) se calcule sur les habitants.

### Ères et jalons (exemples)
| Passage | Conditions possibles (à choisir) |
|---|---|
| Néolithique → Bronze | agriculture et élevage, une ville d'indice 3, métallurgie du cuivre (gisements, tâche 15) |
| Bronze → Fer et Antiquité | bronze, écriture, une ville d'indice 8, première loi de droit ou de pouvoir |
| Fer et Antiquité → Moyen Âge | une ville d'indice 10, métallurgie du fer et monnaie, lois sur trois axes |

- Chaque ère débloque des aménagements, des bâtiments, des unités et des institutions.
- **Recherche** : 10 technologies de base par ère, avec des spécialisations, des conditions de
  pré-acquisition et un transfert entre voisins (décidé le 10/11, `conception-recherche.md`).
- Les jalons demandent à la fois de la recherche, de la culture (institutions) et du développement
  des villes : aucune voie unique.
- **Principes des jalons (décidés par Benjamin le 11/11/2026)**, pour ne pas bloquer une petite
  nation (Athènes, Venise, les cités phéniciennes) :
  1. **conditions au choix** : chaque passage propose environ 5 conditions, il suffit d'en remplir
     3 (taille, population totale, technologies, lois, commerce ou monument) ;
  2. **la taille compte pour une ville** (« une ville d'indice 10 »), jamais pour plusieurs ;
  3. **rien qui croisse mécaniquement avec l'étendue** (nombre de villes, territoire) : on
     récompense le développement, pas l'expansion.
  4. **plancher technique** (décidé le 11/11) : il faut toujours au moins **5 technologies de
     base de l'ère sur 10**, plus **2 autres conditions au choix** parmi les 4 restantes (taille
     d'une ville, population totale, lois, culture ou commerce). L'ère reste cohérente avec les
     techniques, sans voie unique. La recherche est bornée à l'ère en cours plus la suivante : un
     peuple n'a jamais plus d'une ère d'écart entre ses techniques et son ère.

### IA des peuples rivaux (conçues : `conception-ia.md`)
- Plusieurs peuples sur la même planète, partis d'endroits différents, qui suivent les mêmes règles
  que le joueur (villes, aménagements, recherche, ères, armée).
- Besoins : une IA de développement (où fonder, quoi aménager, quoi construire), une IA militaire
  (défense, campagnes saisonnières), plus tard la diplomatie et le commerce.
- Elle donnera leur sens à des décisions déjà prises : armes vendues à d'autres peuples,
  diplomatie, commerce, attractivité et migrations entre pays.
- En attendant, la faction hostile reste l'adversaire ; elle pourra grandir avec le temps.

### Fin de partie et victoire (décidé le 10/11/2026)
**Décisions de Benjamin** :
- **Victoire : le meilleur score d'héritage** à la fin de la partie.
- **Ambitions par ère : gardées.** Elles ajoutent des objectifs intéressants et **augmentent le
  score**.
- **Fin au bout de 500 tours**, pour le moment.
- **Hégémonie** : victoire anticipée, mais à **80 %** (et non 50 %) de la population mondiale sous
  son autorité, directe ou par ses vassaux et tributaires.
- **Les piliers du score ne sont pas pondérés** selon les valeurs du peuple : mêmes règles pour tous.

**Score d'héritage (proposition à chiffrer)**, visible pendant toute la partie :
| Pilier | Ce qui compte |
|---|---|
| Peuplement | habitants, villes, Bien-être moyen |
| Savoir | technologies, découvertes sociétales, ère atteinte |
| Culture | monuments, lieux sacrés, Culture produite au total |
| Rayonnement | habitants du monde de sa culture ou d'une culture qui en descend (mixte, fille), religion diffusée |
| Prospérité | commerce, Économie, routes |
| Puissance | territoire, vassaux et tributaires, guerres gagnées |
| Stabilité | durée sans révolte, légitimité |
- Le **rayonnement** prolonge les cultures qui évoluent (`conception-societe.md`) : un peuple
  disparu laisse un héritage dans les cultures mixtes et les cultures filles.
- **Défaites** : effondrement (plus de ville) ; absorption (toutes ses villes à d'autres), son
  héritage culturel restant compté.
- **Ambitions (proposition)** : à chaque ère, quelques ambitions tirées selon la situation du
  peuple (géographie, valeurs, voisins), par exemple « fonder une cité de 20 000 habitants »,
  « relier cinq villes par le commerce », « faire de sa religion celle de trois peuples », « tenir
  un col contre toute invasion pendant une génération ». Le joueur en garde quelques-unes (par
  exemple trois) ; chacune réussie donne des points d'héritage et un peu de légitimité. Une ambition
  non réalisée à la fin de l'ère est perdue, sans pénalité. Les IA ont les leurs.
- **Partie libre après la fin**, le score servant de repère.
- L'écran de nouvelle partie permet de désactiver l'hégémonie (proposition).

### Rythme : des événements réguliers (décidé le 10/11/2026)
Pour éviter les tours vides (12 tours par an, villes en partie automatisées), chaque tour ou
presque apporte une information ou une décision : récolte, crue, hiver, migration, rumeur,
découverte, contact avec un peuple. Le journal hiérarchisé existant les porte.

### Révision 2 : cohérence (décisions de Benjamin, 10/11/2026)
- **Période nomade : l'agriculture est requise.** Le peuple commence nomade ; il ne peut fonder sa
  première ville qu'après avoir découvert l'agriculture (première technique du Néolithique), en
  errant et en observant plantes et animaux. Comment se représente le peuple nomade (une ou
  plusieurs unités) reste à préciser (tâche 22).
- **Effectifs selon l'ère** : les unités grandissent en s'améliorant d'une ère à l'autre
  (`conception-combat.md`) ; les colons et les petites bandes du Néolithique restent compatibles
  avec des villes de quelques centaines d'habitants.
- **Mode 2 tours par mois** : 500 tours et mêmes taux dans les deux modes (révision du 11/11, `conception-saisons.md`).
- **Ordre** : la tâche 30 recale d'abord les chiffres (croissance, PM, coûts du relief, soins) ; les
  jalons et l'arbre de recherche viennent après les villes (28), la culture et les institutions (32)
  et les gisements (15).

### Dévoilement des systèmes par ère (décidé le 10/11/2026)
Le joueur découvre la complexité petit à petit, et le développement suit le même ordre (une
première version jouable plus tôt) :
| Ère | Systèmes qui apparaissent |
|---|---|
| Néolithique | période nomade, villes, recherche, Culture (influence et territoire), barbares, saisons, dirigeant, chefferie et conseil des anciens, contact simple avec les peuples (paix ou guerre) |
| Âge du bronze | diplomatie complète (accords, griefs, traités), commerce entre villes, religion organisée, lois de droit, **cités-États avec assemblée**, royauté, ambassades |
| Âge du fer et Antiquité | espionnage, tributaires, ordres au complet, monnaie, république marchande, empire, théocratie, galères |
| Moyen Âge | féodalité et vassaux, chancelleries et parlements, haute mer, universités |
Les assemblées existent dès les cités-États (comme à Rome ou à Athènes) ; le Moyen Âge apporte
les parlements et les chancelleries.

## Conséquences sur les autres docs
- `conception-saisons.md` : taux au tour de jeu ; mode 2 tours par mois sur 500 tours, mêmes taux.
- `conception-villes.md` : population en habitants, table indice ↔ habitants.
- `conception-combat.md` : PM environ doublés ; effectifs en habitants.
- Règles actuelles (`rules.rs`) à recaler : seuils de croissance, coûts des unités, soins, PM.

## Questions ouvertes
- Chiffres : années par ère, table de population, PM, jalons exacts.
