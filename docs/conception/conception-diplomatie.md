# Conception — Diplomatie, guerre et paix

## Objectif
Donner un sens aux relations entre les peuples (`conception-ia.md`) : des indicateurs qui
expliquent l'attitude de chacun, des guerres qui ont un motif et un but, des paix négociées, des
accords qui comptent pour l'économie et la recherche. Inspirations : les griefs de Civilization VI,
l'expansion agressive, le score de guerre et les traités de paix d'Europa Universalis IV.

## Décisions de Benjamin (10/11/2026)
### Quatre indicateurs
- **Relation** : l'état de la relation avec un pays ; **assez volatile**, selon les événements et
  les intentions des dirigeants.
- **Confiance** : à quel point un pays fait confiance à un autre, **sur la base des actes
  passés** : alliance honorée, paix commune, soutien dans une situation difficile ; ou au
  contraire pillage, brutalités…
- **Expansion agressive** : la perception, par un pays, de l'expansion jugée agressive d'un autre.
  Par exemple fonder une ville sur un territoire qu'il estime devoir lui revenir, ou une conquête
  rapide jugée injustifiée. **Dégrade la confiance et les relations.**
- **Griefs** (nom choisi le 10/11 ; « préjudice » au départ) : l'accumulation des actes hostiles subis, qui donne une
  **légitimité pour répliquer**, comme les griefs de Civilization VI. Elle permet des actions
  hostiles contre le pays fautif **sans prendre d'expansion agressive**.
- **Reprendre une ville qui nous appartenait** ne donne presque pas d'expansion agressive.

### Guerre
- **Score de guerre** calculé selon les pertes, l'occupation des villes, les pillages, l'usure de
  la guerre et l'idéologie du pays. Il sert à **négocier la paix** en faveur d'un camp.
- Il dépend du **but de guerre** ; atteindre le but donne un **bonus de score**.
- **Clauses de paix** : argent, ressources, villes échangées ; traités imposés (ne pas fonder de
  ville à tel endroit, ne pas s'allier avec tel pays…) ; **rentes en ressources** pendant un
  certain nombre de tours ; **revendications abandonnées**.
- **Casus belli** (liste ouverte) : invasion, reconquête, insulte, conflit frontalier (fondation
  d'une ville).

### Accords
- Ouverture des frontières à l'armée ; traité de libre-échange ; accord de coopération sur les
  ressources (favorise leurs échanges entre les deux pays) ; à l'inverse, fermeture des
  frontières au commerce ; accord de recherche ; …
- Militaires : garantie, traité de non-agression, alliance ; …
- **Limiter les alliances et les garanties**, pour qu'un joueur fort ne garantisse pas tout le monde.

## Proposition (à valider)
### Décisions complémentaires (Benjamin, 10/11/2026)
- Le « préjudice » s'appelle **Griefs**.
- **Coalitions** contre un pays à forte expansion agressive : **oui, mais moins punitives que dans
  Europa Universalis**.
- **Les barbares restent hors de la diplomatie.**
- **Pas de limite au nombre de guerres simultanées.** Le frein vient de lui-même : chaque guerre
  pèse sur le soutien à la guerre et le Bien-être, sur l'armée et sur le commerce.

### Coalitions (proposition)
- Un pays rejoint une coalition contre un autre quand l'expansion agressive qu'il perçoit dépasse
  un seuil élevé **et** qu'il est son voisin (ou voisin d'une de ses victimes).
- La coalition est **défensive** : ses membres se défendent ensemble si le pays visé attaque l'un
  d'eux. Elle ne déclare pas la guerre d'elle-même (différence avec Europa Universalis, où la
  coalition attaque).
- Elle se **dissout** quand l'expansion agressive retombe sous le seuil (elle décroît chaque année) ;
  un pays peut en sortir s'il s'allie avec le pays visé ou si sa relation remonte.
- Taille bornée : les membres doivent être proches (portée, comme les garanties) ; un pays
  lointain n'y entre pas.
- Le pays visé est **averti** avant qu'elle se forme (jauge sur l'écran Diplomatie : « les Kaldes
  et les Ourènes s'inquiètent de votre expansion »), pour pouvoir ralentir.

### Les indicateurs en chiffres
Tous se comptent **de pays à pays et dans un sens** : ce que A pense de B n'est pas ce que B pense
de A. Chacun s'affiche avec ses raisons au survol (liste des causes et de leur poids).

| Indicateur | Échelle | Vitesse | Ce qui le fait bouger | Effets |
|---|---|---|---|---|
| Relation | −100 à +100 | rapide | événements (cadeau, insulte, aide, refus), intentions annoncées ; tend vers une base fixée par la confiance, l'expansion agressive, le voisinage, la langue et les institutions | acceptation des propositions, attitude affichée |
| Confiance | 0 à 100 | lente | monte peu à peu avec les traités tenus et l'aide reçue ; chute d'un coup quand un traité est rompu, une trêve violée, un convoi pillé, des civils brutalisés | conditions des accords (une alliance demande une confiance élevée) |
| Expansion agressive | 0 et plus | décroît avec le temps (par an) | fondation sur une revendication, conquête sans casus belli ou au-delà du but, conquête rapide | relation et confiance en baisse ; au-delà d'un seuil, les voisins inquiets se rapprochent (coalitions, plus tard) |
| Griefs | 0 et plus | décroissent lentement | actes hostiles subis : pillage, insulte, espion pris, ville fondée sur sa revendication, frontière violée | ouvrent un casus belli ; une guerre justifiée par des griefs ne donne pas d'expansion agressive |

- **L'expansion agressive est vue par tous**, mais surtout par les voisins de la victime et ceux de
  la même aire de langue ; un pays lointain s'en soucie peu.
- **Reconquête** : reprendre une ville perdue ne coûte presque rien en expansion agressive, tant
  que la revendication est récente (elle s'éteint après un certain nombre de tours ou par traité).

### Revendications
Ce qu'un pays « estime devoir lui revenir » :
- les villes qu'il a perdues (revendication de reconquête, qui s'éteint avec le temps) ;
- le territoire de sa zone d'influence culturelle (`conception-villes.md`, territoire par la
  culture) ;
- ce qu'un traité de paix lui a reconnu.
Une ville fondée sur une revendication d'un autre crée des griefs chez lui et de l'expansion
agressive chez les autres. Les revendications se voient sur la carte diplomatique.

### Casus belli et buts de guerre
| Casus belli | Condition | But de guerre | Expansion agressive |
|---|---|---|---|
| Invasion (conquête) | aucune | prendre des villes | forte |
| Reconquête | revendication sur une ville perdue | reprendre ces villes | presque nulle |
| Insulte | griefs pour insulte, refus, espion pris | humilier : tribut, excuses, rente | faible ; prendre des villes coûte cher |
| Conflit frontalier | ville fondée sur une revendication | prendre, raser ou faire abandonner cette ville | faible |
| Représailles (proposé) | griefs pour pillage de convois ou de terres | rente, réparations | faible |
| Appel d'un allié (proposé) | alliance ou garantie | celui de l'allié | celle de l'allié |

### Score de guerre
De −100 à +100 pour chaque camp, calculé à chaque tour :
- **batailles** : pertes infligées et subies, en habitants ;
- **occupation** : villes occupées, pondérées par leur population (davantage pour la capitale) ;
- **pillages** : convois, aménagements, ressources prises ;
- **but de guerre** : bonus quand il est atteint et tenu (par exemple +25 après quelques tours) ;
- **usure de la guerre** : chaque pays accumule une lassitude (pertes, durée, famine, villes
  occupées) qui pousse à accepter la paix ;
- **idéologie** : les institutions changent la tolérance à l'usure (un peuple guerrier la supporte
  mieux).

### Traité de paix
- Chaque clause **coûte des points de score** : une ville selon sa population, une somme selon
  le trésor du vaincu, une rente selon sa durée.
- Les clauses **dans le but de guerre** coûtent moins cher et ne donnent pas d'expansion
  agressive ; au-delà du but, elles en donnent.
- Clauses : argent (somme ou rente), ressources (rente en tours), villes (cession, restitution),
  abandon de revendications, interdiction de fonder dans une zone, interdiction d'alliance avec un
  pays, **trêve** imposée (la rompre fait chuter la confiance de tous).
- L'IA accepte une paix si le score et son usure le justifient, et dit pourquoi.

### Accords
| Accord | Effet | Lien |
|---|---|---|
| Ouverture des frontières à l'armée | passage des unités | — |
| Libre-échange | pas de douane | marché (`conception-ressources.md`) |
| Coopération sur les ressources | les deux pays se servent l'un l'autre juste après leurs propres villes | ordre de service du marché |
| Fermeture des frontières au commerce | acte unilatéral ; coupe les échanges | griefs et relation en baisse |
| Accord de recherche | diffusion des technologies accrue | `conception-recherche.md` |
| Garantie | le garant entre en guerre si le garanti est attaqué | militaire |
| Non-agression | pas de guerre pendant la durée ; la rompre fait chuter la confiance | militaire |
| Alliance | défense mutuelle, appels à la guerre | militaire |

### Limiter les alliances et les garanties
Plusieurs freins combinés :
1. **Capacité diplomatique** : un nombre limité d'engagements militaires (garanties et
   alliances), qui grandit avec l'ère et certaines institutions (par exemple 1 au Néolithique, 2 au
   Bronze, 3 au Fer, 4 au Moyen Âge).
2. **Coût d'entretien** : chaque garantie ou alliance coûte de l'Économie ou de l'influence,
   davantage si le partenaire est grand ou lointain.
3. **Portée** : on ne garantit qu'un pays proche (distance en temps de trajet) avec qui l'on a
   un contact.
4. **Engagement réel** : un garant qui n'intervient pas perd la confiance de tous ; un petit pays
   peut refuser la garantie d'un voisin trop fort, qu'il craint.

### Interface
- Écran Diplomatie (charte commune, `conception-interface.md`) : un peuple par ligne, avec ses
  quatre indicateurs en jauges, leurs raisons au survol, les accords en cours et les
  revendications.
- Proposer un accord ou une paix sous forme de table : ce que l'on demande, ce que l'on offre, le
  coût en score, la réponse probable avant d'envoyer.
- Carte diplomatique (famille Peuples) : attitude, alliances, revendications, guerres en cours.

### Ordre
1. Contact, relation, confiance, paix ou guerre (première version de `conception-ia.md`).
2. Accords commerciaux et de recherche (avec le marché et la recherche).
3. Expansion agressive, griefs, revendications, casus belli, score de guerre, traités de paix.
4. Garanties, non-agression, alliances et leurs limites ; coalitions.

## Enrichissements (décidés par Benjamin le 10/11/2026, règles proposées)
Retenus parmi des propositions ; chacun avec une règle de départ à valider.

### Demandes des peuples au joueur (oui)
Un peuple demande de l'aide pendant une famine, un droit de passage, un soutien contre un
voisin, l'entrée dans une guerre. Accepter fait monter la confiance, refuser la fait baisser un
peu. Elles alimentent le rythme d'événements réguliers (`conception-echelle-et-but.md`). Au plus une
demande par peuple et par saison, avec une échéance.

### Échange de cartes (oui)
Donner ou échanger ce que l'on connaît du monde (brouillard, carte imparfaite, tâche 23) : monnaie
d'échange en début de partie. On peut donner une partie seulement (une région) ; la carte reçue
est datée (les souvenirs vieillissent, `conception-ia.md`).

### Ennemi commun (oui)
Subir les raids du même clan barbare rapproche deux peuples (relation en hausse). **Accord de
défense commune contre un clan** : on combat ce clan ensemble et l'on partage la vue autour de ses
camps, sans alliance complète ; l'accord s'éteint quand le clan disparaît.

### Affinités (oui)
Relation de base plus haute entre peuples d'une même famille de langues, aux institutions proches
ou partageant des lieux sacrés. Les blocs se forment d'eux-mêmes.

### Soutien à la guerre (à fusionner avec l'usure)
Benjamin : une notion de soutien à la guerre, peut-être à mêler à l'usure, qui dégraderait le
Bien-être. Proposition : chaque pays en guerre a une jauge de **soutien à la guerre** :
- elle part d'un niveau fixé par le motif : élevé pour une défense ou une reconquête, moyen pour
  un conflit frontalier ou des représailles, bas pour une invasion sans griefs ; l'idéologie
  (institutions) le déplace ;
- **l'usure la fait baisser** : pertes, durée, famine, villes occupées, convois pillés ; les
  victoires et le but atteint la font remonter ;
- sous un seuil, le **Bien-être baisse** dans tout le pays, et la paix devient plus facile à
  accepter (pour l'IA comme pour l'opinion) ;
- l'usure du score de guerre devient cette jauge : une seule notion au lieu de deux.

### Effort de guerre et impôt de guerre (Benjamin, 10/11/2026)
- **Décision** : en guerre, la population peut soutenir l'effort et **accepter d'être privée de
  certaines ressources** ; cette tolérance **diminue avec l'usure**. Un **impôt de guerre** peut
  être demandé contre de l'**influence politique** (produite par la Culture, tâche 32).
- **Tolérance aux privations (proposition)** : la ponction forcée (`conception-ressources.md`)
  coûte moins de Bien-être tant que le soutien à la guerre est haut. Par exemple, avec un soutien
  de 80 %, une ville privée de sel ou de luxe ne perd que 20 % du Bien-être habituel ; à mesure
  que le soutien baisse, la pénalité revient entière. Cela ne vaut que pour une guerre en cours.
- **Impôt de guerre (proposition)** : le joueur le lève pour une durée (par exemple une année de
  jeu) ; il paie une somme d'influence au départ, puis l'impôt rapporte un supplément d'Économie
  chaque tour (par exemple +25 % de l'impôt par habitant). Plus le soutien est haut, moins il coûte
  d'influence ; il fait baisser un peu le soutien chaque tour. Il prend fin avec la guerre.
- L'écran de la guerre montre la tolérance actuelle et l'impôt en cours.

### Frontières fixées par traité (oui)
Un accord de délimitation trace une ligne et éteint les revendications de part et d'autre.
Possible en paix ou comme clause d'un traité de paix ; fonder au-delà de la ligne est alors une
violation (griefs, confiance en baisse).

### Cadeaux entre souverains (oui)
Comme dans les lettres d'Amarna : un cadeau (Économie, ressources, objets de prestige) fait monter
la relation tout de suite ; un **échange régulier** fait monter la confiance. Un cadeau trop
modeste au regard du rang du donateur peut vexer (plus tard).

### Mariages dynastiques et otages (oui)
- Mariage : scelle une alliance ou une paix, confiance en hausse ; au Moyen Âge, il peut ouvrir une
  **revendication d'héritage** (casus belli de succession).
- Otages : garantissent un traité ; le rompre coûte très cher en confiance et en réputation, et
  l'otage est perdu.

### États tributaires puis vassaux (oui)
- **Antiquité : État tributaire.** Un peuple paie une rente (Économie ou ressources) contre
  protection ; il garde sa diplomatie, sauf la guerre contre son protecteur.
- **Moyen Âge : vassal.** Il garde ses villes mais suit la diplomatie de son suzerain, lui doit une
  rente et des troupes en guerre ; le suzerain le défend.
- **Désir d'indépendance** : grandit quand le vassal ou le tributaire se renforce face à son
  suzerain, quand la relation baisse ou quand le suzerain est en difficulté (guerre, famine).
- **Guerre d'indépendance** : au-delà d'un seuil, il peut se soulever et **chercher des soutiens**
  parmi les autres vassaux du même suzerain ou des pays étrangers (appel à la guerre).
- Devenir tributaire ou vassal : clause d'un traité de paix, ou choix d'un petit peuple menacé.

### Réfugiés (oui)
Une famine ou une guerre fait fuir des habitants vers les voisins (attractivité, migrations,
`conception-villes.md`). Les accueillir : confiance en hausse chez le pays d'origine, population
en plus, nourriture en moins ; les refuser : frontière fermée, relation en baisse.

### Médiation (à voir comment l'implémenter)
Piste simple : un pays tiers, en bons termes avec les deux camps et à la réputation élevée, propose
une paix ; elle reprend le score de guerre mais les belligérants l'acceptent plus facilement ; sa
réputation monte. À revoir quand la guerre et la paix seront en place.

### Réputation globale (oui)
En plus de la confiance de pays à pays, une **réputation de fiabilité vue par tous** : traités
tenus, alliances honorées, otages rendus la font monter ; un traité rompu la fait baisser partout,
même chez ceux qui n'étaient pas concernés (moins fort que pour la victime).

### Délai et perte des nouvelles (oui)
Au Néolithique et à l'âge du bronze, une nouvelle (déclaration de guerre, traité, demande, réponse)
voyage avec un **messager** le long d'un chemin, à la vitesse d'une unité ; **elle peut ne jamais
arriver** (messager perdu en montagne, tué par des barbares, pris par un ennemi).
- Délai et risque selon la distance, le relief, la saison et le danger du chemin.
- Les technologies réduisent l'un et l'autre : Écriture (messages fiables), Routes pavées et sa
  spécialisation Poste et relais, Gouvernail et boussole par mer.
- **Décision de Benjamin (10/11/2026)** : le délai et la perte des nouvelles concernent surtout
  les **peuples lointains, mal informés** de ce qui se passe ailleurs. Entre voisins, la nouvelle
  arrive (une déclaration de guerre est connue de la victime) ; pas de règle d'attaque surprise
  liée au messager.
- Conséquence : un peuple lointain juge sur ce qu'il sait. Une conquête, un traité rompu ou une
  guerre qu'il ignore ne change ni son expansion agressive perçue, ni la réputation qu'il prête,
  tant que la nouvelle ne lui est pas parvenue ; elle peut ne jamais lui parvenir.
- L'écran Diplomatie montre les messages en route et ceux perdus.

### Histoire commune (oui, avec l'atlas)
Une chronologie des relations avec chaque peuple : guerres, traités, services rendus, cadeaux,
trahisons. Elle explique la confiance, se consulte dans l'écran Diplomatie et rejoint l'**atlas**
(tâche 27) : chaque peuple connu y a sa page (langue, territoire connu, histoire commune).

## Écran de la guerre, batailles et historique (Benjamin, 10/11/2026)
### Demande
- Un **écran de la guerre** qui montre : le score de guerre, son évolution récente et ses raisons,
  **la valeur vers laquelle il tend** ; les pertes de chaque camp en unités, en hommes et en
  matériel ; les soldats restants (**connus pour son camp, estimés pour l'autre** selon le niveau
  d'information) et le rapport de force ; les causes des pertes (attrition, combats…) ; l'usure.
- Un **historique des guerres** de la partie, avec leurs chiffres : pertes, batailles notables…
- **Une bataille peut durer plusieurs tours** et opposer plusieurs unités. Elle prend le **nom de
  la ville la plus proche**, ou à défaut de la **zone géographique**.
- Un **niveau d'information** sur chaque pays, qui dépend du commerce, de l'ouverture des
  frontières, de l'espionnage, de la découverte du territoire et de la proximité, et qui
  **baisse naturellement avec le temps**.

### Proposition : batailles
- **Regroupement** : les combats à moins de 3 cases les uns des autres, entre les mêmes camps et
  sans plus d'un tour d'interruption, forment une même bataille. Elle se termine après deux tours
  sans combat dans la zone ; le vainqueur est le camp qui tient le terrain ou a le moins perdu.
- **Siège** : une ville encerclée donne un « siège de … », avec sa durée et son issue.
- **Nom** : « bataille de <ville> » si une ville est à 3 cases au plus ; sinon le nom de la zone
  géographique la plus précise (col, vallée, fleuve, plaine, région naturelle : `geography.rs`,
  `toponymy.rs`) ; en mer, le détroit, le golfe ou la mer. Une deuxième bataille au même endroit
  devient « deuxième bataille de … ».
- **Bataille notable** : pertes élevées au regard des armées engagées, ou fort effet sur le score
  de guerre (bataille décisive). Elle s'annonce dans le journal et reste dans l'historique.
- **Fiche d'une bataille** : dates, lieu, camps et unités engagées, pertes par camp et par cause,
  durée, vainqueur, effet sur le score.

### Proposition : écran de la guerre
Un écran par guerre (charte commune, `conception-interface.md`) :
| Bloc | Contenu |
|---|---|
| En-tête | belligérants et alliés, motif, but de guerre et sa progression, durée, saison à venir (l'hiver qui approche) |
| Score de guerre | valeur, courbe des derniers tours, **raisons** du changement (bataille, ville prise, pillage, but tenu), **valeur visée** : celle vers laquelle il tend si rien ne change (villes occupées, but tenu) |
| Forces | soldats restants : exacts pour son camp, **en fourchette** pour l'autre selon le niveau d'information ; **rapport de force** |
| Pertes | par camp, en unités, en hommes et en matériel (armes, montures, navires) |
| Causes des pertes | combats, attrition (faim, froid, maladie), sièges, pillages de convois, désertion, captures |
| Soutien à la guerre | jauge (qui reprend l'usure), ses causes, l'effet sur le Bien-être |
| Coût | Économie dépensée, ressources consommées, production et commerce perdus |
| Front | mini-carte : villes occupées, batailles, armées connues |
| Batailles | en cours et passées, avec un lien vers leur fiche et la carte |
| Paix | ce que l'adversaire accepterait aujourd'hui (estimation, fiable selon le niveau d'information) |

### Proposition : historique des guerres
- Une page par guerre terminée : nom (par exemple « guerre de reconquête de Kaldra » ou
  « guerre kaldo-ourène »), années historiques, participants, motif, but, issue, clauses du
  traité, pertes totales, batailles notables, villes qui ont changé de main, courbe du score.
- Les guerres de tous les peuples **connus** y figurent, avec ce que l'on en sait (niveau
  d'information, nouvelles parvenues).
- Elle alimente l'**histoire commune** de chaque peuple et l'**atlas** (tâche 27) ; elle est
  gardée dans la sauvegarde (tâche 13).

### Proposition : niveau d'information
Un niveau par pays connu, de 0 à 100, qui fixe la précision de ce qu'on sait de lui : effectifs
(fourchette de ±50 % à exact), villes et leur taille, technologies, accords, ce qu'il accepterait.
| Source | Effet |
|---|---|
| Commerce | proportionnel aux flux entre les deux pays (les marchands rapportent) |
| Ouverture des frontières | fort |
| Espionnage | fort, ciblé (plus tard : espions, risque d'être pris → griefs) |
| Découverte du territoire | cases vues et villes visitées |
| Proximité | voisins mieux connus |
| **Ajouts proposés** | ambassade ou émissaire permanent ; alliés qui partagent ce qu'ils savent ; échange de cartes ; langue de la même famille (affinités) ; combats et prisonniers (on apprend sur l'armée qu'on affronte) ; réfugiés et migrants ; nouvelles par les messagers |
- **Baisse naturelle avec le temps** : sans source active, le niveau décroît chaque tour ; ce qu'on
  sait vieillit (date du dernier aperçu affichée).
- Le niveau s'affiche dans l'écran Diplomatie et au survol des estimations (« environ 2 000 à
  3 000 soldats, information moyenne »).
- Même règle pour l'IA : elle estime le joueur avec la même précision (`AiView`, `conception-ia.md`).

## Questions ouvertes
- Médiation : comment l'implémenter.
- Chiffres : échelles, décroissances, seuils de confiance par accord, coût des clauses, capacité
  diplomatique par ère.
- Seuil et portée des coalitions.
- Le joueur et l'IA suivent les mêmes règles ; la faction barbare reste hors de la diplomatie (décidé).
