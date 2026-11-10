# Conception — Recherche et technologies

## Objectif
Donner un usage à la Recherche : un arbre de technologies progressif, où celui qui va vite prend un
avantage sans écraser les autres, où la géographie et les voisins comptent, et que l'on lit
facilement. Première phase : Néolithique → Moyen Âge, 4 ères d'environ 125 tours
(`conception-echelle-et-but.md`).

## Décisions de Benjamin (10/11/2026)
- **Arbre avec interdépendances** (prérequis entre technologies).
- **Technologies de base et spécialisations** :
  - les technologies de base sont celles que presque tout le monde prend ;
  - certaines ont des **spécialisations** qui donnent des **bonus supplémentaires applicables sur
    l'ère** : bateaux plus performants, armes améliorées, moulins améliorés… Seuls ceux que ça
    intéresse les prennent ;
  - **toutes les technologies n'ont pas forcément de spécialisation** ;
  - une spécialisation est **optimisée en appliquant la technologie de base** (par exemple, avoir
    construit des moulins rend moins chère la spécialisation des moulins).
- **Conditions de pré-acquisition** : une partie des points d'une technologie est donnée quand une
  condition est remplie. Exemple : une ville au bord de la mer fait avancer la recherche des
  premiers bateaux.
- **10 technologies par ère**, peu chères, chacune avec un petit avantage : une progression douce,
  un avantage à celui qui va vite, mais pas écrasant.
- **Taille de l'empire** : un empire plus grand reçoit une **légère pénalité** de recherche.
  S'agrandir reste intéressant, mais l'écart entre petits et grands royaumes est limité.
- **Transfert de technologie** :
  - une technologie peut être recherchée ou **acquise au contact** des nations qui l'ont déjà ;
  - quand une nation dispose d'une technologie, **ses voisins gagnent lentement des points** sur
    celle-ci ; **le gain augmente avec le retard technologique** et **se cumule** entre voisins ;
  - **le commerce accélère très fortement** l'acquisition, sans la rendre instantanée.
- **Le joueur oriente la recherche**, ce qui ne l'empêche pas d'avancer sur les autres technologies.
- **Coût calé sur le nombre de tours par ère.**
- **Interface lisible et intuitive** : elle met en avant les bonus débloqués et les liens entre technologies.

## Proposition (chiffres à régler)
### Structure
- Par ère : **10 technologies de base**, reliées par des prérequis (dans l'ère et avec l'ère
  précédente). Certaines ont **1 à 3 spécialisations**.
- Une spécialisation coûte environ **un tiers** d'une technologie de base ; elle demande sa base.
- Les technologies débloquent des aménagements, des bâtiments, des unités (et leur amélioration
  d'ère), et comptent dans les **jalons de passage d'ère** (par exemple 7 des 10 technologies de base,
  dont certaines obligatoires).
- **Agriculture**, première technologie du Néolithique, requise pour fonder la première ville
  (tâche 22) : pré-acquise en vivant près de céréales ou d'animaux sauvages (tâche 16).

### D'où viennent les points d'une technologie
Une technologie avance par quatre sources, qui se cumulent jusqu'à son coût :
1. **Recherche propre** : les points de Recherche du peuple vont à la technologie choisie.
2. **Conditions de pré-acquisition** : quand une condition est remplie (ville côtière pour les
   bateaux, mine pour la métallurgie, moulins construits pour leur spécialisation…), la technologie
   gagne des points, une fois ou chaque tour tant que la condition tient. Plafond proposé :
   **50 % du coût**.
3. **Diffusion par les voisins** : chaque voisin qui connaît la technologie en donne lentement,
   selon la proximité (dégressif avec la distance) et les flux (échanges, déplacements), d'autant
   plus que le retard est grand ; les voisins se cumulent.
4. **Commerce** : une route commerciale avec un peuple qui la connaît multiplie fortement la diffusion.

**Jamais instantané** : la diffusion et le commerce couvrent au plus **75 % du coût** ; le reste
vient de la recherche propre ou des conditions.

### Orientation par le joueur
- Le joueur choisit la technologie en cours ; **ses points de Recherche y vont**.
- Les autres technologies avancent quand même par les conditions, la diffusion et le commerce.
- Pas de recherche diffuse (décidé).

### Coût et rythme
- **Calé sur l'ère** : sur ≈ 125 tours, un peuple moyen qui ne prend que les bases termine ses
  10 technologies en ≈ 100 tours (une tous les ≈ 10 tours), ce qui laisse du temps pour des
  spécialisations.
- Coût d'une technologie de base = production de Recherche attendue d'un peuple moyen de l'ère
  × ≈ 10 tours ; à mesurer avec `bilan_ia` (`conception-ia.md`).
- **Pénalité de taille** : coût multiplié par (1 + 3 % par ville au-delà de la première), à régler.
  S'agrandir rapporte plus de Recherche qu'il n'en coûte, mais moins que proportionnellement.

### Interface
- **Colonnes par ère**, technologies de base en cartes, spécialisations en petites cartes accrochées.
- Chaque carte montre en icônes **ce qu'elle débloque** (aménagements, bâtiments, unités, bonus).
- **Survol** : les prérequis et ce qui en dépend s'éclairent, le reste s'estompe.
- **Barre de progression découpée par source** (recherche, conditions, voisins, commerce), avec
  l'explication (« +12 % : ville côtière ; +8 %/tour : les Kaldes la connaissent »).
- Conditions de pré-acquisition affichées sur la carte, cochées quand elles sont remplies.
- Liens vers le wiki (F1).

### IA
- L'IA choisit ses technologies par utilité, selon sa personnalité (un peuple marin prend les
  spécialisations navales) et ses conditions déjà remplies.

## Décisions complémentaires (Benjamin, 10/11/2026)
- **Les bonus des spécialisations se maintiennent aux ères suivantes**, mais l'évolution
  technologique les rend progressivement obsolètes ; une amélioration future peut les rendre
  vraiment obsolètes. Règle proposée pour l'équilibre (à confirmer, la phrase de Benjamin
  « une optimisation faite deux ères auparavant… » est restée inachevée) : **bonus entier pendant
  son ère, moitié à l'ère suivante, nul deux ères plus tard**, ou dès qu'une technologie la remplace
  (moulin amélioré remplacé par le moulin à vent, par exemple).
- **Pas de recherche diffuse** : les points de Recherche vont tous à la technologie choisie.
- **Diffusion par la proximité et le commerce** : dégressive avec la distance, et proportionnelle
  aux flux (volume des échanges, déplacements, migrations). Un voisin lointain sans échanges ne
  transmet rien.

## Liste proposée des 40 technologies (à valider)
D'après les grandes innovations de chaque époque (chronologies du Néolithique et de l'âge du
bronze, technologie de l'âge du fer et de l'Antiquité, technologie médiévale ; recherche du
10/11/2026). Domaines : agriculture et élevage, artisanat et matériaux, construction et eau,
transport et navigation, guerre, savoir et société, commerce. « Pré-acquisition » : condition qui
donne des points (plafond 50 %). Les spécialisations sont facultatives.

### Néolithique (≈ −8 500 → −3 500)
| Technologie | Prérequis | Débloque | Pré-acquisition | Spécialisation |
|---|---|---|---|---|
| Agriculture | — | fonder une ville, champs | céréales sauvages proches (tâche 16) | Sélection des semences (rendement des champs) |
| Domestication | — | pâturages, élevage | troupeaux sauvages proches | Transhumance (pâturages d'altitude en saison) |
| Pierre polie | — | coupe de bois, défrichement, lancier | forêt dans le territoire | — |
| Poterie | Agriculture | atelier de potier, variété de la nourriture | argile (géologie, tâche 14) | Tour de potier (production artisanale) |
| Tissage | Domestication | vêtements (Bien-être en climat froid) | lin ou troupeaux à laine | — |
| Construction en terre | Agriculture | maisons de torchis et brique crue, palissade | — | Palissade renforcée (défense) |
| Rites et mégalithes | — | lieu de culte (Culture) | pierre de taille proche | — |
| Pirogue | Pierre polie | embarcations sur fleuves et côtes, pêche | ville au bord d'un fleuve ou de la mer | Balancier (trajets côtiers plus longs) |
| Irrigation | Agriculture | aménagement d'irrigation | ville près d'un cours d'eau en climat sec | Bassins de retenue (stockage de l'eau) |
| Métallurgie du cuivre | Poterie | mine, outils de cuivre | gisement de cuivre accessible (tâche 15) | — |

### Âge du bronze (≈ −3 500 → −1 200)
| Technologie | Prérequis | Débloque | Pré-acquisition | Spécialisation |
|---|---|---|---|---|
| Bronze | Métallurgie du cuivre | armes et outils de bronze, porteurs de bouclier | étain accessible ou importé | Fonte à la cire perdue (Culture) |
| Roue et chariot | Domestication | charrettes, routes de terre | — | Roue à rayons (char de guerre) |
| Araire | Agriculture, Domestication | labour à bœufs (rendement des champs) | — | — |
| Écriture | Poterie | Recherche, institutions de droit, jalon d'ère | ville d'indice 3 ou commerce actif | Calendrier (semailles et récoltes mieux placées) |
| Arc composite | Domestication | archers | — | Archers d'élite |
| Voile | Pirogue, Tissage | navires à voile, commerce côtier | ville côtière | Quille et bordé (navires plus grands) |
| Architecture monumentale | Construction en terre, Rites et mégalithes | temples, remparts de pierre | pierre de taille proche | Remparts de pierre (défense) |
| Poids et mesures | Écriture | marché, routes commerciales | commerce actif | Sceaux et contrats (Économie du commerce) |
| Conservation des aliments | Poterie | salaisons (variété, nourriture en campagne) | sel accessible | — |
| Canaux et digues | Irrigation | grands canaux, digues contre les crues | fleuve à crues dans le territoire | Qanat (galeries d'eau sous les montagnes, climat sec) |

### Âge du fer et Antiquité (≈ −1 200 → 500)
| Technologie | Prérequis | Débloque | Pré-acquisition | Spécialisation |
|---|---|---|---|---|
| Métallurgie du fer | Bronze | armes et outils de fer, bas fourneau | gisement de fer accessible | Acier trempé (armes améliorées) |
| Monnaie | Poids et mesures | impôt, trésor, institution d'économie | commerce actif | Change et crédit |
| Alphabet | Écriture | Recherche, Éducation | — | Bibliothèques (Recherche) |
| Mathématiques et arpentage | Alphabet | cadastre, aqueducs, territoire mieux tenu | — | Astronomie (navigation, calendrier précis) |
| Ingénierie | Architecture monumentale, Mathématiques et arpentage | arc et voûte, ponts de pierre, aqueducs | — | Béton hydraulique (ports, ponts) |
| Routes pavées | Roue et chariot, Ingénierie | routes rapides (capacité de gouverner) | — | Poste et relais (capacité de gouverner) |
| Galères | Voile | navires de guerre et de commerce | ville côtière | Éperon |
| Équitation | Domestication | cavalerie légère | chevaux dans le territoire (steppe, prairie) | Archers à cheval |
| Moulin à eau | Ingénierie | moulins | cours d'eau à débit régulier | Moulins améliorés |
| Médecine et hygiène | Alphabet | thermes, égouts (Bien-être) | ville d'indice 8 | — |

### Moyen Âge (≈ 500 → 1 400)
| Technologie | Prérequis | Débloque | Pré-acquisition | Spécialisation |
|---|---|---|---|---|
| Charrue lourde | Araire, Métallurgie du fer | culture des sols lourds et humides | sols argileux (géologie) | — |
| Assolement triennal | Charrue lourde | rendement des champs, moins de jachère | — | — |
| Collier d'épaule et ferrure | Équitation | chevaux de trait (transport, moins de bras saisonniers) | — | — |
| Étrier et selle | Équitation | cavalerie lourde | — | Caparaçon (cavalier lourd protégé) |
| Moulin à vent | Moulin à eau | moulins sans rivière | vent fort et régulier (vent simulé) | Moulin à pivot |
| Château | Ingénierie | châteaux, enceintes | relief élevé dans le territoire | Enceinte concentrique |
| Arbalète | Métallurgie du fer | arbalétriers | — | Arc long |
| Gouvernail et boussole | Galères, Mathématiques et arpentage | navigation hauturière | ville côtière | Cogue (gros navire de commerce) |
| Papier et écoles | Alphabet | Recherche, Éducation | — | Universités |
| Foires et lettres de change | Monnaie | foires, commerce lointain | routes commerciales | Ligues marchandes |

Remarques :
- Les dates sont indicatives : les innovations apparaissent à des époques différentes selon les régions.
- Les unités de chaque ère sont détaillées plus bas (« Unités par ère »).
- Les technologies qui supposent des gisements (cuivre, étain, fer, sel, argile) dépendent des
  tâches 14 et 15 ; elles restent accessibles par le commerce et la diffusion.

## Unités par ère (proposition à valider)
Benjamin (10/11/2026) : **davantage de diversité au sein de chaque ère**. Proposition fondée sur
une recherche des troupes de chaque époque (guerre préhistorique, armées de Sumer, d'Égypte et des
Hittites, armées grecques, hellénistiques et romaines, armées médiévales).

### Principes
- **Des rôles, pas seulement des niveaux** : éclaireur, tirailleur (fronde, javelines), archer,
  infanterie légère, infanterie lourde, anti-cavalerie, cavalerie légère, cavalerie lourde, chars,
  siège, navires. Chaque ère en couvre 7 à 10, avec des forces et des faiblesses croisées (les
  piquiers arrêtent la cavalerie mais souffrent sous les traits, etc.).
- **Unités régionales** liées à la géographie, comme la personnalité des peuples
  (`conception-ia.md`) : éléphants (forêt tropicale ou savane), chameliers (désert), archers à
  cheval (steppe), frondeurs (îles et côtes rocheuses), montagnards (massifs). Elles demandent un
  animal ou un milieu dans le territoire, ou un échange avec un peuple qui l'a.
- **Lignes d'amélioration** (déjà décidées : l'unité s'améliore d'une ère à l'autre et son effectif
  grandit, `conception-combat.md`) : lancier → phalange → hoplite → piquier ; frondeur →
  peltaste ; archer → archer à l'arc composite → arc long ou arbalétrier ; char → cavalerie.
  Une unité ancienne reste utilisable mais devient obsolète, comme les spécialisations.
- **Version réduite d'abord** : le jeu a 12 types aujourd'hui (`units.rs`) ; on ajoute les autres
  par ère, en commençant par celles qui changent le plus la manière de jouer (frondeurs, chars,
  engins de siège, éléphants).

### Néolithique
| Unité | Rôle | Technologie | Existe (`UnitKind`) |
|---|---|---|---|
| Éclaireur (chasseurs) | reconnaissance | — | `Scout` |
| Guerriers à massue | infanterie légère | — | `Militia` |
| Frondeurs | tirailleurs, portée courte | — | nouvelle |
| Archers (arc simple) | tir | — | `ArcherSwordsman` (à renommer ?) |
| Lanciers | anti-cavalerie, défense | Pierre polie | `Spearman` |
| Guerriers à la hache de pierre polie | infanterie de choc | Pierre polie, spécialisation | nouvelle |
| Pirogues de guerre | razzia par fleuve et côte | Pirogue | nouvelle |

### Âge du bronze
| Unité | Rôle | Technologie | Existe |
|---|---|---|---|
| Porteurs de bouclier | infanterie de ligne | Bronze | `ShieldWarrior` |
| Phalange de lanciers (Sumer) | anti-cavalerie, mur de boucliers | Bronze | amélioration de `Spearman` |
| Guerriers à la hache ou au khépesh | infanterie de choc | Bronze, spécialisation | `HeavyWarrior` |
| Archers à l'arc composite | tir à longue portée | Arc composite | `ExpertArcher` |
| Frondeurs (balles de plomb) | tirailleurs | amélioration | nouvelle |
| Char lourd à roues pleines (ânes, Sumer) | choc, lent | Roue et chariot | nouvelle |
| Char léger à rayons (2 hommes, archer) | tir mobile, plaines | Roue à rayons | nouvelle |
| Béliers et échelles | siège | Architecture monumentale | nouvelle |
| Navires à voile et à rames | transport, combat côtier | Voile | nouvelle |

### Âge du fer et Antiquité
| Unité | Rôle | Technologie | Existe |
|---|---|---|---|
| Hoplites | infanterie lourde en phalange | Métallurgie du fer | amélioration de `ShieldWarrior` |
| Phalange à sarisse | piquiers | Métallurgie du fer, spécialisation | `Pikeman` |
| Légionnaires (javelot, épée courte) | infanterie lourde polyvalente, fortifie son camp | Ingénierie | nouvelle |
| Peltastes | tirailleurs à javelines | Métallurgie du fer | amélioration des frondeurs |
| Cavalerie légère | harcèlement, poursuite | Équitation | `LightCavalry` |
| Archers à cheval | tir mobile (steppe) | Archers à cheval (spécialisation) | `HorseArcher` |
| Cavalerie de choc (lance) | charge | Équitation + Métallurgie du fer | nouvelle |
| Cataphractes | cavalerie très lourde, lente | Acier trempé, chevaux de steppe | nouvelle, régionale |
| Éléphants de guerre | choc, effraie les chevaux, peut paniquer | Domestication + éléphants dans le territoire | nouvelle, régionale |
| Chameliers | cavalerie du désert, effraie les chevaux | Domestication + désert | nouvelle, régionale |
| Baliste et catapulte à torsion | siège, tir lourd | Mathématiques et arpentage, Ingénierie | nouvelle |
| Galères (trières) | combat naval, éperon | Galères | nouvelle |

### Moyen Âge
| Unité | Rôle | Technologie | Existe |
|---|---|---|---|
| Hommes d'armes | infanterie lourde (chevaliers à pied, mercenaires) | Château | amélioration de `HeavyWarrior` |
| Milice communale (vouge, hallebarde) | défense des villes | Foires et lettres de change | amélioration de `Militia` |
| Piquiers en carré | anti-cavalerie | amélioration | `Pikeman` |
| Arbalétriers à pavois | tir perçant, lent à recharger | Arbalète | nouvelle |
| Archers à l'arc long | tir rapide en masse | Arc long (spécialisation) | amélioration d'`ExpertArcher` |
| Sergents montés | cavalerie moyenne | Collier d'épaule et ferrure | amélioration de `LightCavalry` |
| Chevaliers | cavalerie lourde, charge à la lance couchée | Étrier et selle | `HeavyCavalry` |
| Archers à cheval des steppes | tir mobile | amélioration | `HorseArcher` |
| Trébuchet, mangonneau, tour de siège | siège | Château (et Ingénierie) | nouvelle |
| Cogues armées | transport et combat en haute mer | Gouvernail et boussole | nouvelle |

Bilan : 7 unités au Néolithique, 9 au Bronze, 12 au Fer, 10 au Moyen Âge, dont 3 régionales.
Toutes ne sont pas nouvelles : environ la moitié sont des améliorations des 12 types existants.

### Décisions de Benjamin (10/11/2026)
- **Engins de siège et navires de guerre : des unités à part.**
- **Unités régionales selon des ressources** présentes dans certaines zones (cheval, dromadaire,
  éléphant). Posséder la ressource rapporte un revenu ; les unités la consomment pour
  s'approvisionner, à la manière du matériel militaire.
- **Unités disponibles selon les ressources** dont dispose le peuple (fer, cuivre et étain,
  bois, montures…), et non toutes celles de l'ère. Détail dans `conception-ressources.md`.
- **Les archers restent exposés au corps à corps**, même face aux lanciers.
- **Le lancier est faible face aux soldats à l'épée**, pas face aux archers.
- **Deux façons d'armer l'infanterie** : arme à une main avec bouclier, ou arme à deux mains
  (nuance déjà présente : guerrier bouclier et guerrier arme lourde). Proposition de règles dans
  `conception-combat.md` (« Armes et rôles »).

## Questions ouvertes
- Confirmer la règle d'obsolescence des spécialisations.
- Valider la liste des 40 technologies, leurs prérequis et leurs spécialisations.
- Valider les unités par ère (les trois points ont été tranchés le 10/11).
- Chiffres (plafonds, pénalité de taille, coûts, diffusion selon la distance et les flux).
