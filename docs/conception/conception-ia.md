# Conception — IA des peuples rivaux et faction hostile

## Objectif
Donner au jeu son défi : d'autres peuples joués par l'ordinateur, qui suivent les mêmes règles que
le joueur (`conception-echelle-et-but.md`). La fin de partie se décidera avec eux.

## Décisions de Benjamin (10/11/2026)
- **Nombre de peuples réglable.** Environ 8 joueurs au total, voire 12. Le bon nombre dépendra de
  la place pour s'étendre sur les cartes générées (à mesurer).
- **L'IA ne voit pas à travers le brouillard.**
- **Idéalement, pas de triche** ; à voir si c'est trop compliqué (avis d'un agent demandé, voir plus bas).
- **Départ nomade** pour tous les peuples (avec la période nomade, tâche 22).
- **Personnalité née de la géographie et de la langue** du lieu de départ.
- **Diplomatie simple** dans un premier temps, mais avec une **interface intuitive**.
- **Faction hostile** : Benjamin aimerait **un clan par camp barbare**, mais craint pour les
  performances. Autre piste : faire explorer les barbares pour trouver des cibles.

## Principe : mêmes règles que le joueur
- L'IA fonde des villes, aménage, construit, recherche, passe les ères par les jalons, forme des
  soldats, sans règle à part.
- **L'intendant des villes sert aux deux** (`conception-villes.md`) : celui d'une ville d'IA est le
  même que celui du joueur, réglé par l'IA au lieu du joueur. Le choix des aménagements, qui revient
  au joueur, est pour l'IA une décision de niveau opérationnel.

## Trois niveaux de décision (proposition)
1. **Stratégique**, une fois par saison ou par an : objectifs (s'étendre, développer, se défendre,
   préparer une guerre, commercer), choisis selon la personnalité et la situation (place libre,
   voisins menaçants, famine, retard).
2. **Opérationnel**, chaque tour : où fonder (eau, fertilité, relief, côte), quoi aménager et
   construire, quelle recherche, où envoyer l'armée, avec des **campagnes saisonnières** (partir au
   printemps, rentrer avant l'hiver, éviter la récolte).
3. **Tactique** : déplacements et combats. Base : l'IA hostile (`hostile.rs`) et l'exploration
   automatique (`explore.rs`).

**Méthode : IA par utilité.** Chaque action possible reçoit un score selon les besoins et la
personnalité ; l'IA prend la meilleure et peut dire pourquoi dans le journal (« Les Kaldes
fortifient leur frontière : armée voisine menaçante »).

## Peuples nés de la géographie
- Chaque peuple naît dans l'aire d'une des huit langues de la toponymie et en porte les noms.
- Personnalité tirée du lieu et de la graine : marin (côte découpée), montagnard (massif), cavalier
  (steppe), bâtisseur (plaine fertile) ; plus des traits (expansionniste, prudent, belliqueux, marchand).
- Des sociétés qui se développent différemment, en accord avec les institutions sans arbre unique.

## Diplomatie (première version)
Système complet décidé le 10/11/2026 (relation, confiance, expansion agressive, griefs, casus
belli, score de guerre, traités, accords, limites des alliances) : `conception-diplomatie.md`.

- Contact (on se découvre), paix ou guerre, frontières.
- **Interface intuitive** : attitude de chaque peuple et ses raisons visibles, intentions annoncées,
  propositions simples (paix, guerre, ouverture des frontières).
- Plus tard : commerce (routes, surplus, armes), tributs, alliances, vassalité, assimilation,
  migrations entre pays.

## Outil d'équilibrage et déterminisme
- **Parties sans joueur** : 500 tours avec seulement des IA, sans écran, et un bilan chiffré
  (population, ères atteintes, famines, guerres). Elles servent à régler les règles des villes, des
  saisons et de l'armée, et à mesurer la force des IA.
- Décisions tirées de la graine : une partie se rejoue à l'identique.

## Points d'attention
- **Durée des tours** : un budget par IA et par tour (par exemple moins de 200 ms) ; décisions
  stratégiques rares. Pour mémoire, l'exploration de 36 éclaireurs prend ≈ 150 ms.
- **Moteur de tours** à préparer pour plusieurs peuples (aujourd'hui un joueur et une faction hostile).
- **Ordre** : IA de développement pacifique (après les villes, tâche 28), puis IA militaire, puis diplomatie.

## Difficulté sans triche (avis d'un agent, validé par Benjamin le 10/11/2026)
**Constat** : c'est réaliste pour un développeur seul. Le rythme est lent (500 tours), il ne faut
pas une IA brillante mais cohérente, et la logique tactique existe déjà (`hostile_turn` n'attaque
que si `combat::preview` est favorable).

**Ce que font les autres jeux** : Civilization donne surtout des bonus à l'IA (vécus comme de la
triche au plus haut niveau) ; Freeciv définit la difficulté par une liste de handicaps nommés
(pas de vision à travers le brouillard, carte connue seulement…), mais son plus haut niveau voit
toute la carte ; Paradox affiche ses modificateurs. Les joueurs acceptent un bonus économique
**affiché** ; ils rejettent l'information cachée (Soren Johnson, « Game AI & Our Cheatin' Hearts »).

**Leviers d'habileté, sans triche** (un réglage par levier, combinés par niveau) :
1. bruit dans les choix (tirage parmi les meilleures options, plus large en facile) ;
2. fréquence de réévaluation de la stratégie (tous les 12 tours en facile, tous les 3 en difficile) ;
3. horizon (distance de recherche des emplacements, anticipation économique) ;
4. coordination militaire (unités isolées en facile ; groupes, flanquement, belle saison en difficile) ;
5. qualité des emplacements de ville (nourriture seule en facile ; eau, gisements, défense, côte en difficile) ;
6. gestion de l'économie et réglages de l'intendant : **le levier le plus rentable** pour un 4X de développement ;
7. agressivité et prudence (seuils de risque au combat, en partie tirés de la personnalité) ;
8. ouvertures préparées pour les 30 premiers tours.

**Niveaux proposés** :
| Niveau | Contenu |
|---|---|
| Découverte | bruit fort, réévaluation rare, pas de coordination, IA pacifique |
| Normal | IA honnête et moyenne |
| Difficile | tous les leviers au maximum, **aucun bonus** |
| Expert | Difficile + un **bonus économique affiché** dans le menu (+15 à 25 % de production ou de croissance, ou colons en plus au départ) |

Jamais de bonus de vision, de combat ni d'unités gratuites.

**Respect du brouillard** :
- **`AiView`** : l'IA ne reçoit qu'une vue construite depuis son brouillard et sa mémoire, dans un
  module de logique pure. Elle ne lit jamais l'ECS directement ; aujourd'hui `hostile_turn`
  parcourt toutes les unités du joueur, une fuite à corriger.
- **Brouillard par peuple en tableaux de bits** (`Vec<u64>`) : ≈ 2,7 Ko par carte et par peuple,
  mémoire des derniers aperçus ≈ 100 à 200 Ko par peuple ; moins de 3 Mo pour 12 peuples. L'actuel
  `Fog` en `HashSet<Hex>` ne convient pas pour cela.
- **Chemins dans l'inconnu** : coût moyen supposé, recalcul quand la case se révèle.
- **Souvenirs qui s'oublient** : une armée vue il y a longtemps compte moins.
- **Déterminisme** : un générateur par peuple, initialisé par (graine, tour, peuple) ;
  `rand::rng()` est à retirer de `hostile.rs` et `battle.rs`.
- **Exploration** : `Known::new` est reconstruit pour chaque éclaireur (`explore.rs`) ; une fois
  par peuple et par tour suffirait (gain estimé de 5 à 10 fois, à mesurer).

**Mesure** :
- `bilan_ia` : 12 IA, 500 tours, sans écran, graines 777 / 2 / 5 ; population, villes, ère,
  recherche, territoire, pertes, ms par IA et par tour, aux tours 100, 250 et 500.
- Tournois niveau N contre N − 1 (classement Elo) ; aucun changement d'IA ne doit régresser.
- **Test de non-triche** : modifier une zone inconnue d'un peuple ne doit pas changer ses décisions.
- Console : `ia <peuple>` montre sa carte connue, ses souvenirs et son plan.

## Un clan par camp barbare (avis d'un agent, validé par Benjamin le 10/11/2026)
**Constat** : c'est tenable. Le coût vient des chemins et de la fréquence des décisions, pas du
nombre de clans. Aujourd'hui ≈ 40 à 50 camps (`LAND_PER_CAMP` = 160) de 2 guerriers au plus
(`CAMP_MAX_UNITS`), ≈ 100 guerriers, sans calcul de chemin : moins d'une milliseconde. Le vrai
danger serait 100 guerriers calculant chacun un long chemin (A* sur `HashMap`) à chaque tour.

**Architecture** :
1. **Clan → bande → unité** :
   - clan (stratégique) : décide tous les 4 à 6 tours ou sur événement, décisions étalées entre clans (≈ 8 clans par tour) ;
   - bande (un but : razzia, défense, éclairage, migration) : un champ de flux partagé vers sa cible, plus d'A* par unité ;
   - unité (tactique) : chaque tour, le code actuel.
2. **Personnalité par clan** (agressivité, rayon d'action, pillage ou tribut), tirée de la graine,
   du type de camp (`CampKind`), du relief et de la langue : c'est elle qui rend les clans distincts.
3. **Cartes partagées par la faction, une fois par tour** : menace (force ennemie aperçue) et proies
   (villes et unités connues, peu défendues) ; autour de chaque camp, un Dijkstra borné (rayon
   12 à 15 cases) mis en cache.
4. **Budget** (≈ 10 ms pour la faction) ; ce qui dépasse passe au tour suivant.
5. **Parallélisme** : chaque clan calcule ses ordres sur une vue figée, appliqués ensuite dans
   l'ordre des identifiants (déterminisme).
Estimation : moins de 10 ms par tour (à mesurer).

**Trouver des cibles sans tricher** :
- brouillard de faction alimenté par ce que voient les guerriers ;
- éclaireurs barbares de temps en temps (`explore.rs`), plafonnés (par exemple 6 sur la planète) ;
- **rumeurs comme règle affichée** : un clan apprend l'existence des villes à moins de 8 à 10 cases
  de son camp, plus loin quand elles grandissent (fumées, renommée) ; le wiki le dit ;
- mémoire des proies vues et des razzias réussies ou ratées ;
- **routes commerciales** comme proies : piller les convois rapporte un butin (décidé le 10/11,
  `conception-ressources.md`) ;
- événements rares liés aux saisons : coalition de clans (horde), migration après une famine ou un
  hiver dur ; les survivants d'un clan détruit rejoignent le clan voisin.

**Premier pas conseillé** : refaire `hostile_turn` sur une `AiView` à brouillard et un générateur
initialisé par la graine ; cette base servira ensuite aux IA des peuples.

## Questions ouvertes
- Nombre de peuples par taille de carte (à mesurer sur les cartes générées).
- Chiffres : bruit, fréquences, horizons par niveau ; bonus de l'Expert ; rayon des rumeurs.
- Fin de partie : décidée le 10/11 (`conception-echelle-et-but.md`) ; l'IA vise aussi le score d'héritage.
