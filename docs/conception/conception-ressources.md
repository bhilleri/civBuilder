# Conception — Ressources

## Objectif
Donner aux lieux de la carte une valeur propre : des ressources placées par la géologie et le
climat (tâche 15), qui rendent possibles certaines unités, rapportent un revenu et ont toujours un
usage civil. Fait suite à la discussion sur les unités par ère (`conception-recherche.md`).

## Décisions de Benjamin (10/11/2026)
- **Les unités dépendent des ressources dont dispose le peuple**, pas seulement de l'ère :
  fer, bronze, bois, montures (cheval, dromadaire, éléphant), etc.
- **Unités régionales** : chevaux, dromadaires et éléphants se trouvent dans certaines zones.
  **Posséder la ressource rapporte un revenu**, et les unités la **consomment pour
  s'approvisionner**, à la manière du matériel militaire (`conception-combat.md`, « Armée »).
- **Chaque ressource militaire a aussi un usage civil**, pour qu'elle garde sa valeur quand une
  autre la remplace : quand le fer remplace le bronze pour les armes, le bronze reste utile à
  l'économie. On évite ainsi la ressource devenue inutile, comme dans Civilization.
- **Des ressources sans usage militaire** (or, pierres précieuses…) ont un intérêt pour la
  Recherche, l'hygiène, la Production, etc.
- Points à voir : le bronze, qui est un alliage ; le bois, la forêt étant partout.

## Proposition (à valider)
### Fonctionnement commun
1. **Gisement ou zone** sur la carte, placé par la géologie (tâche 15) ou le climat et les biomes.
2. **Exploitation** par un aménagement choisi par le joueur (mine, carrière, haras, pâturage,
   coupe) dans le territoire d'une ville ; elle emploie des habitants, comme les bâtiments
   d'armement.
3. **Flux par tour** versé dans le **stock de la ville** qui exploite (voir « Marché entre les
   villes ») ; le pays en réquisitionne une part pour former les unités (équipement, montures) et
   les entretenir (remplacer les pertes, nourrir les bêtes).
4. **Revenu** : chaque ressource exploitée rapporte de l'Économie (vente, commerce) ; le
   surplus se vend aux voisins. L'import par le commerce donne accès aux unités sans posséder
   le gisement, au prix d'une dépendance (la route peut être coupée).
5. **Unité disponible** = technologie connue + ressource dans le stock ou en flux. Sans la
   ressource, l'unité ne peut être formée ; une unité qui en manque s'use (attrition,
   montures non remplacées).
6. **Gisements finis ou non** : pour l'instant, pas d'épuisement (même choix que la fertilité,
   `conception-volcans.md`).

### Ressources militaires (avec usage civil)
| Ressource | Où (génération) | Usage militaire | Usage civil (garde sa valeur) |
|---|---|---|---|
| Cuivre | subductions, roches volcaniques anciennes | armes du Néolithique final, base du bronze | outils, ustensiles, Économie |
| Étain | granites, alluvions ; rare | avec le cuivre, fait le bronze | commerce lointain (très recherché) |
| Bronze (produit) | atelier de bronzier : cuivre + étain | armes et armures de l'âge du bronze | après le fer : cloches, statues, monnaie, instruments → Culture et Économie |
| Fer | vieux bassins, fer des marais | armes et armures de l'âge du fer | outils agricoles (charrue lourde) → Nourriture, Production |
| Bois d'œuvre | vieilles forêts de grands arbres (chênes, cèdres, conifères), pas toute forêt | navires, engins de siège | charpente, grands bâtiments → Production |
| Chevaux | steppes et prairies tempérées | chars, cavalerie | trait, transport, poste (avec le collier d'épaule) |
| Dromadaires | déserts chauds | chameliers | caravanes : commerce à travers le désert |
| Éléphants | savanes et forêts tropicales | éléphants de guerre | travaux, prestige ; ivoire → Culture |
| Obsidienne | volcans (tâche 11) | armes tranchantes du Néolithique | commerce, outils fins |

- **Le bronze est un produit**, pas un gisement : il faut du cuivre et de l'étain. L'étain étant
  rare, l'âge du bronze crée naturellement du commerce lointain, comme dans l'histoire.
- **Le bois** : toute forêt donne du bois ordinaire par le rendement de la case (sans stock).
  Seules les vieilles forêts de grands arbres donnent du **bois d'œuvre**, utile aux navires et
  aux engins ; la ressource se raréfie là où l'on défriche (à voir avec la tâche 12).
- **Les montures** sont fixées à des zones (pâturages, haras) pour ne pas dépendre des
  déplacements des animaux sauvages (tâche 22). Les autres animaux domestiques restent dans la
  tâche 16 (variété de la nourriture seulement).

### Ressources sans usage militaire
| Ressource | Où | Intérêt |
|---|---|---|
| Or, argent | subductions, filons, or alluvial | Économie (monnaie, trésor), Culture (prestige) |
| Pierres précieuses, ambre | massifs anciens ; ambre des côtes froides | Bien-être (luxe), Culture, commerce |
| Sel | bassins fermés, marais salants, côtes arides | hygiène, conservation des aliments, commerce |
| Plantes médicinales | forêts, montagnes | hygiène (épidémies) |
| Encens et épices | climats chauds et secs ou tropicaux | Bien-être, commerce lointain |
| Teintures (pourpre, indigo) | côtes, climats chauds | Économie, Culture |
| Papyrus, roseaux | deltas et marais chauds | Recherche (écrits) |
| Pierre de taille | partout, mais belle pierre rare | Production des grands bâtiments (décision des volcans : pas une ressource commune) |
| Argile fine | plaines alluviales | Production (poterie), Culture |

## Révision du 12/11/2026 : familles, nourriture, stocks (étape 5)
Après l'avis de trois agents (pédologue, historien des techniques, game designer ; synthèse plus
bas), Benjamin a donné ces directions. Ce qui est marqué « proposé » reste à valider.

### Familles de ressources (Benjamin ; en cours)
Une ressource a une famille principale et au plus une seconde (le bois d'œuvre est matériau et
stratégique).
- **Stratégiques** : armes, montures… **avec reconversion économique** quand l'armée n'en a plus
  besoin (le bronze après le fer : statues, monnaie).
- **Besoins de base** (décisions de Benjamin, 12/11/2026) : deux **besoins**, le sel et les
  vêtements ; les ressources qui couvrent un besoin portent son nom.
  - **Sel** : un **bonus** (la norme est de ne pas en avoir) : couvert, +5 de salubrité (proposé) ;
    il conserve aussi la viande et le poisson.
  - **Vêtements** : **couverts, rien ; en manque, un malus de salubrité** (proposé : jusqu'à −10, au
    prorata ; validé le 13/11/2026), parce que la salubrité a déjà beaucoup de sources de gain. Couverts par **Fibres et
    peaux** (lin, laine, coton ou peaux selon le climat ; famille principale Besoins de base), et par
    la **soie** et les **fourrures** (Luxe, seconde famille Besoins de base). Les teintures restent du
    luxe seul. Pas de bonus de diversité des vêtements : la diversité passe par le luxe.
  - **Décidé le 13/11/2026** : base de la salubrité 50, seuil des maladies 40 (même marge
    qu'avant, plafond d'une ville très bien fournie à 87) ; pas de bonus à forte salubrité pour
    l'instant (plus tard, moindre prise des épidémies).
- **Luxe** : produits de luxe → bien-être (+4 par sorte, attente qui grandit avec la ville, 4c).
- **Matériaux** (compatibles avec stratégique) : pierre de taille, argile, chaux, bois d'œuvre.
- **Santé, Culture, Savoir** : peut-être regroupés en une famille (nom proposé : « Développement » ;
  ressources consommées par les bâtiments et les institutions, l'infobulle dit sur quoi elles
  agissent : salubrité, culture, recherche).
- **Nourriture** : par type et par diversité (ci-dessous).

### Outils, armes et montures (décisions de Benjamin, 12/11/2026)
Après l'avis d'un géographe, d'un économiste et d'un historien, puis d'un game designer et d'un
joueur (page de synthèse publiée : familles, ressources, bonus) :
- **Huit familles** : Nourriture, Besoins de base, Luxe, Métaux précieux, Matériaux, **Outils et
  armes**, **Montures et bêtes de bât**, Développement (santé, culture, savoir).
- **Outils** : une ressource porte le tag « outils » avec un **niveau** : 1 pierre (silex,
  obsidienne, pierre dure), 2 cuivre, 3 bronze (cuivre + étain ; l'étain seul n'a aucun effet), 4 fer.
  Rendements des cases aménagées **+10 / +15 / +20 / +25 %** (validé le 13/11/2026) ; seul le meilleur niveau couvert
  compte (pas d'addition) ; les outils s'usent (0,1 par tour pour 1 000 travailleurs : la demande),
  bonus au prorata de la couverture ; pas d'atelier d'outils au début (la ressource sert d'outil).
- **Bonus propres** (Benjamin) : une ressource peut garder un bonus à elle, qui s'ajoute et continue
  de s'appliquer après son remplacement par un meilleur niveau. Proposé : silex (faucilles,
  tribulum : +5 % de nourriture des champs), obsidienne (luxe), pierre dure (mégalithes, lieu de
  culte ; meules), cuivre (vaisselle, petite monnaie : +5 % de deniers), bronze (cloches, statues :
  +1 culture), fer (sols lourds × 1,15 avec la Charrue lourde).
- **Armes** (étape 8) : une seule ressource « Armes » (conception du combat), avec le **niveau** de
  son matériau ; le même métal sert aux champs et à l'armée (arbitrage).
- **Montures** (Benjamin) : **elles ne font pas partie du matériel militaire** ; elles sont gérées à
  part et **doivent alimenter le front** (les bêtes perdues sont remplacées depuis les villes).
  Bêtes de bât : coût du trajet par la route −50 % (proposé).
- **Bois d'œuvre** : exigé seulement pour les grands navires (voile, galères, cogues) ; pirogues et
  barques au bois ordinaire.
- Le bronze à l'arsenic est retiré : sans étain, le cuivre reste au niveau 2.

### Revue par six rôles et décisions (Benjamin, 13/11/2026)
**Ressources validées par Benjamin le 13/11/2026** (familles, liste, bonus, outils +10 à +25 %,
affichage par effet avec un onglet « Marché », salubrité base 50 et seuil 40, vêtements −10) ; les
chiffres encore « proposés » se calent par les bilans au fil des livraisons.
Revue de la page de synthèse par un game designer systèmes, un joueur, un historien, un économiste,
un designer d'interface et un développeur. Décisions de Benjamin :
- **Commerce** : les villes commercent entre elles, et le jeu se jouera contre d'autres peuples.
  Idée de Benjamin : des **cités-États** (comme dans Civilization), états stables, plus ou moins
  pacifiques, qui ne fondent pas de nouvelles villes : des partenaires de commerce.
- **Matériaux** : la carte est grande, réunir tous les matériaux ne sera pas facile ; **augmenter un
  peu les rendements des outils, sans exagérer** : tout l'enjeu est l'équilibre (proposé : outils
  +10 / +15 / +20 / +25 %, à vérifier par un bilan).
- **Recette du bronze** (« comme dans Minecraft ») : proposé 3 cuivre + 1 étain.
- **Culture tirée des ressources plafonnée à +3 par ville** ; le marbre +1 par bâtiment, au plus +3 ;
  la cire perdue ne donne plus de culture.
- **Bonus de nourriture des champs : un seul réservoir additif, plafonné à +40 %** ; les faucilles de
  silex sont supprimées (doublon avec son niveau d'outil).
- **Légumineuses** : une sorte de Protéines (lentilles, pois, fèves), au rendement réduit (reprise de
  la décision « pas de légumineuses pour le moment »).
- **Rangs de luxe** : commun +2, fin +4, rare +6, même plafond de +16 (la soie et les épices valent
  plus que l'ocre locale).
- **Bien-être utile à la limite** : part active de 40 à 60 % selon le bien-être (prévu en 4a) ; le
  bien-être a alors deux effets (croissance naturelle, part active).
- Corrections consensuelles (à appliquer) : outils couverts par tranches (le niveau inférieur couvre
  le reste), usure 0,05 par tour pour 1 000 travailleurs ; métaux précieux sans prix plancher avant
  la Monnaie (l'argent pesé facilite un peu le commerce), frappe bornée à rendement décroissant
  après ; « +5 % de deniers » du cuivre seulement après la Monnaie ; au plus deux effets par
  ressource (l'huile perd la salubrité, l'encens la culture ; le collier d'épaule devient une
  technologie : les chevaux **accélèrent** sa recherche (pré-acquisition de
  `conception-recherche.md` : des points jusqu'à 50 % du coût, pas une condition), et son **effet**
  (labour au collier) demande des chevaux ; une technologie se connaît sans la ressource, c'est son
  usage qui la demande) ; parures fusionnées avec les pierres précieuses ; « Papyrus et
  parchemin » (savoir) ; bitume **ou poix** pour le calfatage ; ânes au Néolithique final,
  dromadaires au Fer ; soie et vigne au Néolithique final dans leur foyer ; bœufs : « traction :
  araire » ; sel des côtes tempérées (saumure chauffée) et sel gemme ; montures qui demandent des
  pâturages ; demande chiffrée (sel et vêtements 0,1 pour 1 000 habitants ; luxe 0,1 × palier par
  sorte, stock qui s'use) ; un poids par ressource quand il s'écarte de sa famille ; rareté comptée
  une fois.
- **Pas de ressource bloquante (Benjamin)** : exiger une ressource pour un bâtiment ou une unité
  serait trop bloquant pour les joueurs qui n'y ont pas accès. Une ressource rend une chose
  **meilleure ou moins chère**, jamais possible ou impossible. Proposé : temple en pierre locale,
  et en marbre s'il y en a (+ culture) ; mégalithe en pierre locale, moins cher avec la pierre dure ;
  grands navires au bois ordinaire (+50 % de production), au coût normal avec le bois d'œuvre ;
  unités montées formées sans montures, avec le malus d'équipement incomplet déjà décidé.
- **Ordre des livraisons (développeur)** : roches et sols → nourriture par type → diversité →
  greniers → gisements → besoins de base et luxe → outils → matériaux, montures et marché.
  `Rendements` reste l'agrégat, avec un panier par sorte à côté ; une sorte par case, choisie à
  l'aménagement ; prévision simplifiée (sans usure ni marché).

### Décisions pour les livraisons 5a à 5c (Benjamin, 13/11/2026)
- **5a Géologie** : huit roches (granite et gneiss, roches volcaniques d'arc, basalte, calcaire,
  grès, argile et marnes, alluvions, lœss), déduites des plaques, du relief et de l'eau ; le
  **karst** seulement affiché (pas de changement de l'hydrologie ; à revoir avec la refonte
  graphique) ; la roche visible dans l'**infobulle** d'une case explorée, rien sur la carte
  principale (« sinon trop d'information ») ; une **couche en plus** : terrain, relief et biomes
  inchangés, mondes et sauvegardes identiques.
- **5b Sols** : les sept classes et leurs multiplicateurs (alluviaux 1,4 ; volcaniques jeunes 1,3 ;
  noirs et lœss 1,3 ; bruns 1,0 ; lourds 0,85 ; pauvres acides 0,7 ; lessivés tropicaux 0,65) ;
  seulement sur la nourriture que le champ tire du terrain ; pâturages un tiers de l'écart ; ni
  cueillette, ni bois, ni eau douce. Le sol dans l'infobulle et la fiche de case, et une **carte
  de la fertilité des sols** (ou la future carte des ressources). Répartition cible (10 % alluviaux,
  15 % noirs et lœss, 40 % bruns) **selon la géographie de chaque monde** ; vérifier qu'aucune
  vallée alluviale ne devient une métropole dès le Néolithique.
- **5c Nourriture par type** : **une seule sorte présentée par type et par case, la plus adaptée et
  la plus rentable** (Benjamin) ; surveiller que les sortes se répartissent bien dans le monde ;
  cases non aménagées : leur type selon le biome ; **jardin d'appoint** (fruits et légumes, petite
  capacité ; peut-être un peu de bien-être : promenade) ; légumineuses à 60 % d'un champ de
  céréales ; seuils et effets déjà décidés ; **les pourcentages des types se comptent sur la
  consommation, pas sur la production** (Benjamin).

### Fait le 13/11/2026 : roches et sols (5a, 5b)
- Module `geologie.rs` (logique pure, testée) : `roche(Lieu)`, `sol(Roche, Lieu)`, `karstique` ;
  `WorldMap::roche`, `sol`, `karstique`, calculés à la demande et gardés en cache (vidé avec le cache
  de turbidité) : rien n'est sauvegardé, la même graine donne la même géologie.
- Règles des roches, dans l'ordre : alluvions (fleuves, deltas, marais, plaines basses d'une grande
  rivière) ; arc volcanique au-dessus d'une subduction ou d'un arc d'îles (< 350 km, côté qui reste
  en surface) ; basalte (rift < 150 km, point chaud < 250 km, petite terre de moins de 60 cases sur
  croûte océanique) ; collision (granite au cœur < 350 km en montagne ou au-dessus de 1 500 m, grès
  au piémont < 700 km) ; lœss (steppes et prairies fraîches, 250 à 700 mm) ; granite (montagnes, et
  socles loin des frontières au-dessus de 300 m) ; bassins : grès sous 400 mm, puis calcaire ou
  argile selon un bruit lent et la température. Karst : calcaire et au moins 800 mm.
- Constat : la croûte « océanique » de la génération porte 40 à 57 % des terres ; elle ne sert donc
  pas de signe de basalte (sauf pour les petites îles), et la règle des dorsales a été retirée.
- Fertilité : `intendant::rendement_max` multiplie la nourriture du terrain des champs (pas l'eau
  douce), un tiers de l'écart pour les pâturages.
- Bilan `bilan_geologie` (777 / 2) : granite 10,5 / 11,3 %, arcs 8,7 / 6,2, basalte 5,2 / 4,2,
  calcaire 15,3 / 16,4, grès 25,7 / 30,5, argile 18,3 / 18,0, alluvions 11,2 / 11,1, lœss 5,1 /
  2,4 ; karst 6,6 / 6,9 ; sols : alluvial 11,2 / 11,1, volcanique jeune 13,9 / 10,4, noir 7,9 / 5,0,
  brun 42,7 / 46,8, lourd 6,9 / 3,5, pauvre et acide 16,6 / 22,7, lessivé tropical 0,7 / 0,5 (peu de
  jungles hors des fleuves et des volcans). Limite d'un territoire de 19 cases en champs (milliers
  d'habitants) : médiane 75 → 75 / 75 → 73, 95e centile 104 → 112 / 107 → 109, maximum 125 → 137 /
  130 → 136.
- Pas encore : la Charrue lourde (sols lourds × 1,15) et le seigle et l'avoine (sans malus des sols
  pauvres), avec la recherche ; le karst ne change pas l'hydrologie (refonte graphique).

### Fait le 13/11/2026 : nourriture par type (5c)
- `intendant::TypeNourriture` (céréales et féculents, protéines, fruits et légumes) ; `Parts` d'une
  case (`parts_de_la_case`) : champ → céréales ; légumineuses et pâturage → protéines ; jardin →
  fruits et légumes ; sans aménagement, selon le biome (forêts 0 / 50 / 50, méditerranéen 20 / 30 /
  50, prairies, steppes, savanes 40 / 60 / 0, marais 30 / 70 / 0, eau et le reste : protéines) ;
  centre de la ville 50 / 25 / 25. Nouveaux aménagements : **champ de légumineuses** (60 % d'un
  champ, 1 500 travailleurs, 20 de production) et **jardin** (fruits et légumes, nourriture × 0,75 de la case (d'appoint ;
  × 1,5 au premier essai : plus fort qu'un champ par travailleur), 300 travailleurs, 10 de production ; le bien-être de la promenade n'est pas fait).
- `types_comptes` : un type compte s'il couvre 15 % de ce que la ville mange (le besoin, ou toute
  la nourriture en famine) ; `CityState::types_nourriture` garde le nombre du dernier tour (pas de
  boucle avec l'intendant). `besoin_nourriture` est divisé par `facteur_equilibre` (1 / 1,1 / 1,2) :
  la limite monte ; la famine est multipliée par `facteur_famine` (1 / 0,75 / 0,5) ; zone
  d'équilibre de 3 % sans croissance naturelle. L'intendant (`repartir`) ajoute un poids de 3 à la
  nourriture des types sous le seuil, une fois la ville nourrie.
- Effet : le centre seul (2 de nourriture, trois types) nourrit 2 400 habitants au lieu de 2 000.
- Pas encore : les sortes (diversité, 5d) ; le choix de la sorte d'un champ (la plus adaptée, par
  case) ; la répartition des sortes dans le monde à surveiller.

### Fait le 13/11/2026 : grenier (5e, décisions G1 à G4)
- `Production::Grenier` (30 de production, un par ville, `CityState::grenier`) ; stock par sorte
  (`CityState::stock`), capacité `GRENIER_TOURS` = 5 tours de besoin.
- Remplissage (`excedent_pour_grenier`) : l'excédent que la croissance n'utilise pas, c'est-à-dire
  la zone d'équilibre (jusqu'à 3 % au-dessus du besoin, où la croissance est nulle) et ce qui dépasse
  1,5 fois le besoin (la croissance est alors pleine) ; les sortes qui se gardent le mieux d'abord,
  chacune au plus ce que la ville en a produit.
- Pertes (`Sorte::perte`) : grains, légumineuses, châtaignes, dattes 2 % ; tubercules, fruits,
  légumes 10 % ; viande et poisson (bétail, gibier compris) 20 % ; le sel (5 %) viendra avec lui.
- Usage (`tire_du_grenier`) : en famine, le manque, d'abord ce qui se garde le moins ;
  `nourriture_avec_grenier` sert à la croissance, au bien-être (pas de terme « famine ») et à la
  prévision. Le siège n'existe pas encore : il s'appuiera sur le même stock.
- Bilan (`bilan_grenier`, 8 villes de 19 cases sur 777 et 2, projet Peupler) : stock à 4,9 tours de
  besoin dès le tour 25 (une jeune ville a plus de 1,5 fois son besoin), jusqu'à ce que la ville
  touche sa limite ; il retombe alors vers 1,5 tour (3 % d'excédent contre 2 % de perte du grain).
- Pas encore : séchage et fumage (climat froid ou aride), fête quand le grenier est plein et varié,
  stocks dans le commerce.

### Fait le 13/11/2026 : sols du désert et du froid
- Question de Benjamin : « les déserts ont un facteur de 1 ? ». Ils n'en avaient pas l'usage (aucun champ possible), mais la
  carte et l'infobulle montraient « sol brun : champs × 1 ». Nouvelles classes **sol aride** (désert) et **sol gelé**
  (toundra, neige) : « pas de culture » (`Sol::cultivable`, `effet_texte`) ; leur multiplicateur reste 1 pour les pâturages
  de la toundra (rien ne change dans les règles).
- Le désert irrigué devient deux biomes (voir `conception-biomes.md`) : **rives vertes** (sol alluvial, champs et pâturages :
  les Égyptiens élevaient bœufs, moutons, chèvres et porcs dans le Delta, sur les berges et les chaumes) et **oasis** (dattes,
  chameaux, capacité `CAPACITE_OASIS` = 0,5).

### Fait le 13/11/2026 : diversité des repas (5d)
- `intendant::Sorte` (17 sortes, chacune d'un type) ; `sortes_de_la_case` : une sorte par type et
  par case, la plus adaptée (champ : tubercules en jungle et savane arborée, riz au-dessus de 20 °C
  et 1 000 mm, millet et sorgho en savane, steppe ou sec et chaud, blé en méditerranéen ou tempéré
  de 8 à 20 °C et 400 à 1 000 mm, sinon orge ; pâturage : rennes, lamas et chameaux en taïga,
  toundra, désert ou au-dessus de 2 500 m, porcs en forêt, moutons et chèvres en steppe,
  méditerranéen ou relief, sinon bœufs ; jardin : dattes au chaud et sec, sinon légumes ; sans
  aménagement : gibier, baies, fruits tropicaux, châtaignes, orge ou millet sauvages, riz des marais,
  poisson) ; `sortes_du_centre`.
- `sortes_en_plus` : une sorte compte à partir de 5 % des repas ; +2 de bien-être par sorte en plus
  de la première de chaque type, au plus +8 (`CityState::sortes_en_plus`, terme « diversité des
  repas » du bien-être). L'intendant ajoute un poids de 1 à une sorte qui ne compte pas, une fois
  l'équilibre fait (`repartir_avec`).
- Jardin ramené à 0,75 × la nourriture de la case (1,5 au premier essai : plus de nourriture par
  travailleur qu'un champ).
- Bilan (`bilan_geologie`, 777 / 2) : céréales des champs possibles : blé 19,0 / 16,8 %, orge 29,0 /
  34,6, millet et sorgho 8,4 / 6,3, riz 0,9 / 0,9, tubercules 0,4 / 0,0 ; bétail des pâturages :
  bœufs 19,8 / 16,9, moutons et chèvres 17,5 / 15,0, porcs 39,9 / 35,2, rennes, lamas et chameaux
  11,0 / 20,3. Le riz et les tubercules sont rares : ces mondes ont peu de terres tropicales
  cultivables.
- Pas encore : les sortes de l'âge du bronze (seigle et avoine, fruits du verger, vigne et olivier),
  le maïs (foyer d'origine sur un seul continent), les foyers et leur diffusion (commerce).

### Nourriture par type et par diversité (Benjamin)
- La nourriture est rangée **par type** ; **avoir les différents types donne un bonus de
  croissance** (nourriture équilibrée). La ville cherche d'abord à obtenir chaque type.
- **Diversité au sein d'un même type** (plusieurs sortes : blé et orge, bœuf et chèvre…) :
  **bien-être**. La ville cherche d'abord la première sorte de chaque type, puis la seconde.
- **L'intendant tient compte de la diversité.** Il priorise : 1) **nourrir tout le monde**
  (sinon la famine) ; 2) **une nourriture équilibrée** (croissance) ; 3) **la diversité des
  apports** (bien-être). Il peut passer par le **marché** pour équilibrer.
- **Richesse interne de la ville** : elle lui permet d'acheter et de vendre au marché.
- Proposé : trois types, **féculents** (céréales, tubercules ; champs, cueillette de graines),
  **protéines** (viande, laitages, poisson ; pâturages, chasse, pêche), **fruits et légumes**
  (cueillette, vergers, jardins) ; un type compte s'il couvre au moins 15 % du besoin ; croissance
  naturelle × (1 + 0,1 par type au-delà du premier) ; diversité : +2 de bien-être par sorte au-delà
  de la première de chaque type, au plus +8 (distinct du luxe).

### Types de nourriture : avis de cinq agents et décisions (Benjamin, 12/11/2026)
Avis demandés à un historien de l'alimentation, un géographe du peuplement, un nutritionniste, un
spécialiste du gameplay et un joueur. Accord : trois types, les féculents comme base (60 à 80 %
des calories), un **bonus jamais une punition** (une ville de taïga sans fruits n'a pas de malus,
seulement pas le bonus), un type principal par aménagement (jardin tôt pour les fruits et légumes,
pêche et chasse pour les protéines), les sortes laissées à l'intendant.
- **Trois types (décision)** : **Céréales et féculents**, **Protéines** (viande, laitages, poisson),
  **Fruits et légumes** (les oléagineux ne forment pas un type : « on remplace oléagineux par
  fruit »). **Pas de légumineuses pour le moment.**
- **Seuils (décision)** : un type compte s'il couvre **15 % du besoin** (le besoin, pas la
  production : un excédent ne fait jamais perdre un type) ; une sorte compte si elle couvre **5 %
  du besoin**.
- **Seigle et avoine (décision du 12/11/2026)** : disponibles à l'âge du bronze, ils **ignorent le
  malus des sols pauvres et acides** (classe × 0,7 de la fertilité, 5b) et le seigle supporte mieux
  le froid que l'orge : les terres du nord (granite, bord de la taïga, landes) deviennent bonnes à
  cultiver (« la conquête des terres froides »), comme la Charrue lourde pour les sols lourds.
- **Pertes du stock selon la sorte, pas le type (décision)** (les grains se gardent des années, la
  viande fraîche quelques jours) ; idée de l'historien : séchage et fumage (climat froid ou aride)
  réduisent aussi la perte, sans sel.
- **Tout doit se voir (décision)** : dans l'onglet de la nourriture, dans la croissance et dans le
  bien-être.
- **Pas de spécialités régionales nommées** (décision : caler l'économie sur des spécialités locales
  serait trop compliqué).
- **Fête** quand le grenier est plein et varié : attention à l'effet boule de neige ; un délai
  entre deux fêtes (Benjamin ; chiffres à proposer).
- **Effet de l'équilibre (décision du 12/11/2026)**, après l'avis d'un historien démographe et
  d'un géographe du peuplement (la population pré-industrielle était bornée par ce que la terre
  nourrissait, pas par le rythme des naissances ; la diversité relevait la capacité de charge :
  bétail sur les terres non labourables, fumure, pêche, jardins ; et amortissait les mauvaises
  années), puis d'un game designer et d'un joueur (« c'est la limite qu'on regarde ») :
  - **la limite de la ville** × 1,10 avec deux types, × 1,20 avec trois (1 point de nourriture
    nourrit 1 100 ou 1 200 habitants ; le seuil de 15 % se calcule sur le besoin avant ce bonus) ;
  - **famines adoucies** : pertes × 0,75 avec deux types, × 0,5 avec trois, affichées en
    permanence (« en cas de disette : pertes × 0,75 ») et rappelées dans le journal ;
  - **pas de bonus de rythme** tiré de l'équilibre ; **un effet par cause** : les types relèvent
    la limite, les sortes donnent du bien-être ;
  - **taux par tour, quelle que soit l'ère** (Benjamin : « ne pas trop coller à la réalité au
    détriment du jeu » ; un taux par an avec une durée de tour qui varie, ou par saison, donnerait
    des choses bizarres) ; au besoin un coefficient par ère réglé en jouant ;
  - **pas d'oscillations à la limite** : une zone d'équilibre (croissance nulle entre 100 et
    environ 103 % du besoin), l'état affiché « à la limite du territoire » ;
  - affichage séparé de la **limite**, du **rythme** et de la **disette** ; trois jauges avec le
    seuil de 15 % ; l'aménagement en choix annonce son gain de limite ;
  - à surveiller : fertilité (× 1,4) et équilibre (× 1,2) se multiplient (× 1,68) : bilan sur les
    graines 777 et 2 pour éviter des métropoles alluviales dès le Néolithique.
- **Croissance (Benjamin)** : elle reste intéressante, avec d'autres facteurs que l'équilibre.
  Facteurs actuels : surplus, bien-être, « Peupler la ville », salubrité. Pistes proposées : front
  pionnier (une jeune ville loin de sa limite grandit plus vite : contre l'effet boule de neige),
  attractivité et migrations (déjà prévues), techniques (bouillies de sevrage avec la Poterie),
  paix et sécurité. **Décision de Benjamin (12/11/2026)** : des **technologies** débloquent des
  bonus de croissance liés à la **baisse de la mortalité infantile**, et peut-être des
  **bâtiments** (l'hôpital). Proposé, selon l'ère : Poterie (bouillies de sevrage, Néolithique) ;
  sanctuaire de guérison (type Asclépiéion, Antiquité, avec la technologie Médecine et hygiène) ;
  hôtel-Dieu ou hospice (Moyen Âge) ; chaque bonus affiché dans la croissance, cause par cause.
- Pas de carences (le nutritionniste proposait protéines manquantes × 0,8 sur la croissance,
  scorbut −10 de salubrité) : écartées au nom du « bonus jamais une punition » ; le scorbut pourra
  revenir pour une ville assiégée qui vit sur ses greniers.

### Stocks et conservation (Benjamin)
- Une ville a un **stock de nourriture**, utilisé **en cas de famine ou de siège**, disponible avec
  le **grenier** et d'autres bâtiments de stockage.
- La ville **cherche à remplir ses stocks** et **préfère stocker les nourritures qui se gardent
  longtemps**.
- La viande et le poisson se dégradent vite : **perte d'un pourcentage du stock à chaque tour**
  (sans calcul compliqué) ; **le sel limite la perte**. Même chose pour le **commerce** : une perte
  selon la distance, compensée par le sel.
- Proposé : perte par tour, grains 2 %, fruits et légumes 10 %, viande et poisson 20 % (5 % salés) ;
  1 de sel conserve 4 de viande ou de poisson ; le stock se remplit de l'excédent au-delà de ce que
  la croissance utilise (1,5 × le besoin ; l'objectif « Excédent » de l'intendant sert à remplir les
  stocks) ; capacité donnée par le grenier.

### Analyses des agents (12/11/2026, à valider)
- **Fertilité (pédologue)** : le sol dépend de la roche **et** du climat (un basalte tropical très
  arrosé donne un sol lessivé) ; les bassins sédimentaires agissent par leurs roches (marnes et
  limons fertiles, craie à nu et sables pauvres) ; la fertilité touche surtout les **champs**, les
  pâturages d'un tiers de l'écart (l'herbe suit la pluie ; ils occupent les sols qu'on ne labourait
  pas), ni la cueillette ni le bois. Sept classes : alluviaux × 1,4 ; volcaniques jeunes × 1,3 ;
  noirs et lœss × 1,3 ; bruns × 1,0 ; lourds × 0,85 (× 1,15 avec la Charrue lourde) ; pauvres acides
  × 0,7 ; lessivés tropicaux × 0,65. Seulement sur la nourriture que le champ tire du terrain.
- **Chronologie (historien)** : Néolithique : silex, obsidienne, ocre, argile, chaux, sel, jade,
  lin, peaux et fourrures, coquillages, plantes médicinales (cuivre natif et or en parure à la fin) ;
  Bronze : cuivre, étain, or, argent, plomb, bitume, natron, sel gemme, lapis-lazuli, vigne,
  olivier, chevaux, ambre, pourpre, cèdre ; Fer et Antiquité : fer, marbre, encens, poivre, soie,
  dromadaires, éléphants, verre ; Moyen Âge : laine, épices, sucre, alun, fourrures, poisson salé,
  papier. Placement : subductions (cuivre, or, jade, obsidienne, mercure), collisions (marbre,
  lapis), granites (étain), bassins (fer, bitume, sel, silex de la craie), déserts (natron), marais
  (fer des marais).
- **Jeu (game designer)** : classer par effet ; test du « plus » (son absence se voit et a un
  remède, et elle change où l'on s'installe ou crée un choix ; effet en une ligne) ; 20 à 25
  ressources au plus ; découverte en trois couches : indices visibles (roche de surface,
  affleurements révélés par l'exploration), savoir (une technologie montre les zones favorables),
  prospection (automatique dans le territoire, l'éclaireur au-dehors ; l'or d'un fleuve signale un
  filon en amont). Les gisements sont **à découvrir** (Benjamin).

## Marché entre les villes (décisions de Benjamin, 10/11/2026)
**Réalisation en deux temps (révision du 10/11, relecture, validé)** : d'abord les surplus envoyés
aux villes reliées selon la capacité de transport, avec un **dépôt national de l'armée** alimenté
par la réquisition (les armes vont au stock de la ville qui les produit) ; ensuite les prix par
l'offre et la demande, quand l'IA saura commercer. La conception ci-dessous reste le but.

- **Chaque ville exprime un besoin pour elle-même**, selon ses bâtiments et sa population ; elle
  **vend son surplus** aux villes reliées, **en priorité à celles du joueur**.
- **Choix des villes partenaires** selon :
  - l'appartenance au même empire (bonus) ;
  - la proximité, en temps de trajet par charrette ou par bateau ;
  - les relations entre pays : accord d'échange, ou frontières fermées ;
  - la rentabilité (gain rapporté au trajet).
- **Prix selon l'offre et la demande** du bien dans la ville et les villes proches.
- **Pas de caravanes à construire** : les routes commerciales se font seules. Des **bâtiments
  favorisent les échanges**. Le pays **alloue un budget de chevaux ou de dromadaires au
  transport** : arbitrage entre l'armée et l'économie.

### Proposition (à valider)
**Ce qui remplace le stock national** : chaque ville a son propre stock et son marché. Le pays
garde une **réquisition** pour l'armée et le transport (part des montures, du fer, du bois qu'il
achète aux villes à leur prix, en Économie du trésor).

**Ordre de servir un bien produit par une ville** :
1. son propre besoin (bâtiments, population) ;
2. la réquisition du pays (armée, transport), si le joueur l'a réglée ;
3. les villes du même empire ;
4. les villes étrangères avec qui l'échange est permis, la plus rentable d'abord.

**Ponction forcée (décision de Benjamin, 10/11/2026)** : le joueur peut ponctionner des
ressources **au détriment de sa population**, c'est-à-dire passer avant le besoin propre des
villes, **mais il doit être averti**. Proposition : le réglage de la réquisition affiche, avant de
valider, ce qui manquera et où (« Kaldra : forge à l'arrêt, sel manquant, Bien-être −2 ») ; une
alerte reste dans le journal tant que la ponction prive une ville.

**Besoins** :
- bâtiments : la forge consomme du fer, l'atelier de bronzier du cuivre et de l'étain, le chantier
  naval du bois d'œuvre, le relais des chevaux ;
- population : sel, puis biens de luxe (encens, teintures, pierres) d'autant plus que la ville
  grandit (paliers d'indice) ; ils comptent pour le Bien-être ;
- un besoin non couvert a un effet visible (forge à l'arrêt, Bien-être en baisse), affiché dans le
  panneau de la ville.

**Prix** : prix de base du bien × (demande ÷ offre) dans un rayon de quelques jours de trajet,
borné (de ×0,25 à ×4) et lissé d'un tour à l'autre pour éviter les à-coups.

**Choix du partenaire** : score = (prix d'achat sur place − prix chez le vendeur − coût du
trajet) ÷ durée du trajet ; × bonus du même empire (pas de douane) ; nul si les frontières sont
fermées ; réduit par une douane sans accord d'échange.

**Trajets** : chemin le plus rapide par terre (charrette, porteurs), fleuve ou côte, avec le coût
des cases, le relief, les saisons (boue, neige, crues, `conception-saisons.md`). Les durées entre
villes se calculent rarement (tous les quelques tours, ou quand une route ou un pont change) et se
gardent en cache : peu coûteux même avec des centaines de villes.

**Capacité de transport** : chaque lien a une capacité :
- à pied (porteurs) : faible, toujours disponible (Néolithique) ;
- montures et charrettes : selon le budget de chevaux et de dromadaires alloué au transport
  (dromadaires dans le désert, chevaux ailleurs, avec Roue et chariot et le collier d'épaule) ;
- bateaux : grande capacité sur fleuves navigables et côtes (Pirogue, Voile, Gouvernail).
Les montures allouées au transport ne servent pas à l'armée, et inversement.

**Bâtiments qui favorisent les échanges** : marché (plus de demande et d'offre visibles),
entrepôt (stock plus grand), relais et caravansérail (trajets plus rapides et sûrs), port
(fluvial puis maritime), routes et ponts (tâche 18), foires (Moyen Âge).

**Pillage des convois (décision de Benjamin, 10/11/2026)** : les convois sont **pillables par
les ennemis et les barbares**. Comme il n'y a pas de caravane à construire, le convoi n'est pas
une unité : c'est le flux qui suit sa route. Proposition :
- une unité ennemie ou barbare **sur la route ou à côté** intercepte une part du flux qui passe
  (proportionnelle à sa force), qu'elle prend comme butin ; les montures du transport peuvent
  être perdues ;
- une route menacée devient **moins rentable** : les villes choisissent un autre partenaire ou un
  autre chemin, et le commerce se détourne de lui-même ;
- **protection** : unités en escorte ou en garnison le long de la route, relais fortifiés,
  routes surveillées ; zone de contrôle des unités amies ;
- les barbares voient les routes comme des proies (`conception-ia.md`, cartes des proies) ;
- l'écran et la carte du commerce montrent les convois attaqués et les pertes.

**Revenus** : le vendeur gagne de l'Économie ; le pays prélève une douane sur le commerce
étranger. Les flux comptent pour la **diffusion des technologies** (`conception-recherche.md`) et
pour les relations entre pays (`conception-ia.md`).

**Interface** : carte thématique des flux (épaisseur selon le volume), panneau du marché dans la
ville (besoins, stock, prix, d'où vient et où va chaque bien), réglage national de la réquisition
et du budget de transport.

### Lien avec la recherche
Les ressources donnent les **conditions de pré-acquisition** (`conception-recherche.md`) :
cuivre accessible pour la métallurgie du cuivre, étain pour le bronze, chevaux pour
l'équitation, etc. Une technologie peut être connue sans la ressource ; c'est la formation des
unités qui la demande.

### Interface
- Ressources visibles dans l'infobulle de la case et sur une carte thématique (pas sur la carte
  principale, pour ne pas la surcharger).
- Panneau du peuple : stock, flux, consommation par l'armée, revenu, importations.
- Liste des unités : celles qui manquent de ressource sont grisées, avec la ressource à obtenir.

## Questions ouvertes
- Valider la liste des ressources et leurs usages civils.
- Chiffres : flux par exploitation, besoin par unité, revenu, prix du commerce.
- Commerce entre peuples : avec les IA (`conception-ia.md`).
- Chiffres : bornes des prix, douane, capacité par monture, rayon du marché.
