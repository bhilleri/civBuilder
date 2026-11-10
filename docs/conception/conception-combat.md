# CivBuilder — Conception du combat (décisions)

_Dernière mise à jour : 10 novembre 2026 — armée et révisions après l'analyse critique (voir « Armée » et « Révision »). Les chiffres des sections réalisées décrivent le code actuel ; ils seront recalés (tâche 30)._

## Principes retenus
- **Une seule unité par case** (alliés compris).
- **À 0 PV, l'unité est dispersée** (détruite).
- **Pas de technologies pour l'instant** : toutes les unités sont disponibles d'emblée. Décidé le 10/11 : chaque type apparaîtra à une ère (voir « Révision »).
- **Cavalerie dès maintenant.**
- **Reportés à plus tard :** fortification (doit à terme protéger des charges de cavalerie), armes de siège.

## Caractéristiques d'une unité
- Points de vie (100), attaque mêlée, défense mêlée, attaque à distance, portée, défense à distance, PM, coût.
- **Traits** plutôt que colonnes : bonus anti-cavalerie, bonus contre certaines cibles, charge, zone de contrôle…
- Entretien et expérience : voir « Armée » et sa « Révision ».

## Résolution d'un combat
- Dégâts ≈ 30 × e^(écart de force ÷ 25) × aléa entre 0,8 et 1,2.
- Mêlée : le défenseur riposte. Tir : pas de riposte (force de tir contre défense à distance).
- Blessures : −1 de force par tranche de 10 PV perdus.
- **Terrain** : +3 en défense en forêt, +3 dans des collines, cumulables (forêt vallonnée : +6) ; mêlée comme tir.
- **En hauteur** : +2 en défense de mêlée si le défenseur est à un niveau d'altitude supérieur à l'attaquant.
- **Flanquement** : +2 en attaque de mêlée par allié de l'attaquant adjacent à la cible (pas pour un tir, ni à travers une falaise).
- Prévision du combat affichée au survol avant d'attaquer.
- **Une attaque par tour.** Elle consomme les PM restants, sauf l'archer monté après un tir (il garde ses PM moins 1 pour se replier).
- Le vainqueur d'un corps à corps avance sur la case libérée (pas après un tir).

## Unités (chiffres de travail)
| Unité | Coût | Att. mêlée | Déf. mêlée | Att. distance (portée) | Déf. distance | PM | Particularité |
|---|---|---|---|---|---|---|---|
| Milice | 15 | 8 | 8 | — | 6 | 2 | Zone de contrôle |
| Guerrier bouclier | 30 | 10 | 16 | — | 18 | 2 | Résiste aux flèches, zone de contrôle |
| Guerrier arme lourde | 35 | 20 | 10 | — | 8 | 2 | Attaquant de choc, zone de contrôle |
| Lancier (lance + bouclier) | 30 | 12 | 15 | — | 14 | 2 | +10 contre cavalerie, zone de contrôle |
| Piquier (lance à deux mains) | 35 | 12 | 13 | — | 6 | 2 | +20 contre cavalerie, zone de contrôle |
| Archer-épéiste | 35 | 12 | 11 | 12 (2) | 9 | 2 | Polyvalent |
| Archer expert | 40 | 5 | 6 | 20 (2) | 7 | 2 | Ne tire pas après s'être déplacé |
| Cavalerie légère | 45 | 16 | 9 | — | 8 | 4 | +10 contre archers et milice |
| Cavalerie lourde | 60 | 24 | 16 | — | 14 | 3 | Contrée par les lances |
| Archer monté | 50 | 8 | 8 | 10 (1) | 9 | 4 | Tir puis repli |

Hors combat : colon (40, 2 PM, ne peut pas attaquer), éclaireur (15, 3 PM, vision 3, attaque 4 / défense 5).
Les bonus anti-cavalerie s'appliquent à toute la cavalerie, archer monté compris.
Les unités capables de tirer attaquent toujours en tirant (y compris au contact).

## Charge de cavalerie
- Si au moins une case a été parcourue **dans le même tour** avant l'attaque : **+8 de force** (~+40 % de dégâts).
- Uniquement cavalerie légère et lourde ; pas de charge contre une cible en forêt ou dans des collines.
- Plus tard : une unité fortifiée annule la charge.

## Altitude (réalisé)
- (Ancien, remplacé par 9 niveaux déduits des mètres, tâche 7 c) 6 niveaux de terre : 1 littoral, 2 plaine basse, 3 plaine haute, 4 plateau, 5 haut plateau, 6 massif (où se dressent les sommets, infranchissables). L'eau est au niveau 0.
- **Pente** : écart de 1 niveau = coût normal ; montée de 2 niveaux = +1 PM ; écart de 3 niveaux ou plus = **falaise infranchissable** dans les deux sens. Pas de corps à corps ni de flanquement à travers une falaise.
- But : restrictions de déplacement (cols, plateaux à défendre) et préparation des **fleuves**, qui suivront la pente.
- Vision selon l'altitude : envisagée, reportée.

## Collines (réalisé)
- **Deux échelles de relief** : l'altitude est le relief *entre* les cases (à quel niveau est la case, qui domine qui) ; les collines sont le relief *interne* à la case (sol accidenté).
- Les collines s'ajoutent au terrain de base : prairie, plaine, désert, toundra, neige ou forêt vallonnée.
- Effets : +1 production, +1 PM (forêt vallonnée : 3 PM), +3 en défense (cumulable avec la forêt), pas de charge de cavalerie.
- Elles ne comptent pas dans la hauteur pour le tir ni pour la défense en hauteur (rôle de l'altitude).
- Génération : environ 16 % des terres, plus fréquentes au pied des changements de niveau (contreforts).

## Ligne de vue des archers (réalisé)
- Hauteur = niveau d'altitude (les collines n'y comptent pas).
- Archer **plus bas** que la cible, ou cible **en forêt** : portée −1.
- Archer **plus haut** que la cible : +20 % de puissance de tir.
- Seules comptent les cases de l'archer et de la cible (pas les cases intermédiaires).
- **Portée minimale 1** : un tir sur une case adjacente reste toujours possible.

## Zone de contrôle (réalisé)
- Exercée par l'**infanterie de mêlée uniquement** : milice, guerrier bouclier, guerrier arme lourde, lancier, piquier (pas les archers ni la cavalerie).
- Une unité qui entre sur une case adjacente à un ennemi qui l'exerce s'arrête : plus de déplacement ce tour-ci, mais l'attaque reste possible s'il lui reste des PM. Un ordre de déplacement reprend au tour suivant.
- Le trajet d'approche d'une attaque ne traverse pas de zone de contrôle (il peut s'y terminer).

## « Une unité par case » (réalisé)
- Unité produite dans une ville occupée : apparaît sur une case voisine libre, sinon la production attend (coût remboursé).
- Échange de place entre deux unités alliées adjacentes (clic droit), sauf à travers une falaise.
- Les unités alliées sont des obstacles pour les trajets et l'exploration ; l'occupation réelle est vérifiée à chaque pas.
- Clic droit sur un ennemi visible : approche éventuelle puis attaque ou tir (si possible ce tour-ci ; l'archer expert ne se déplace pas pour tirer).

## Règles ajoutées en cours de route
- **Soins** : +10 PV par tour sans bouger ni combattre, +20 en territoire ami.
- **IA hostile** : attaque une unité du joueur à 2 cases au plus (près de sa base), seulement si le combat lui est favorable (pas de mort certaine, et au moins autant de dégâts infligés que subis, ou une chance de disperser la cible) ; tient compte du terrain et du flanquement par les autres hostiles. Blessée (< 60 PV) sans cible favorable, elle reste sur place pour se soigner.
- **Camps** : un camp vide est dispersé quand une unité y entre (+25 or) ; les hostiles n'occupent jamais la case du camp.
- **Exploration automatique prudente** : les explorateurs évitent la zone dangereuse (à 2 cases ou moins d'un hostile visible ou d'un camp connu), y compris les ennemis aperçus en cours de route ; une cible dangereuse n'est choisie que s'il ne reste rien d'autre. Mesure sur des parties simulées de 80 tours : 25 à 47 attaques subies (contre plus de 50 sans cette règle).

## Armée : armes, formation, expérience (décidé le 10/11/2026, à venir)
Décisions de Benjamin, avec le système des villes (`conception-villes.md`) :
- **On ne produit pas des soldats, on les forme.** Ils ne passent pas par la file de construction de la ville.
- **Les armes, ressource stratégique** :
  - produites dans les bâtiments de production d'armes, contre de la **production seule** ;
  - vendues à d'autres joueurs, ou données aux soldats ;
  - **capturées** en vainquant une unité adverse.
- **Équipement** (comme dans Hearts of Iron) : chaque unité a un taux d'équipement. Le besoin
  dépend du type : très faible pour la milice, beaucoup plus fort pour la cavalerie lourde.
  **Un équipement incomplet donne un malus.**
- **Attrition** : perte de matériel ; les pertes au combat détruisent aussi du matériel.
- **Recrutement** dans la ville ou dans les casernes. **Formation progressive** : chaque unité
  gagne de l'expérience au fil des tours, à un rythme qui dépend de son type. Les aménagements
  comme la caserne accélèrent la formation et réduisent le temps nécessaire. Une unité peut être
  recrutée **sans être totalement formée**, avec d'importants malus de combat.
- **Niveaux d'expérience** (comme dans Hearts of Iron) : sans formation, une pénalité de combat ;
  bien formée, l'unité commence avec un petit bonus et peut l'augmenter en gagnant de l'expérience.
- **Effectif selon le type** : chaque unité représente un nombre d'habitants propre à son type
  (par exemple 500 à 1 000 pour l'infanterie, moins pour la cavalerie lourde) ; les pertes au
  combat retirent des habitants en proportion des PV perdus.
- **Entretien** : nourriture (les soldats mangent), économie (une solde selon le type) et usure du
  matériel (l'attrition consomme des armes, remplacées depuis le stock).
- **Les soldats restent des habitants** : vivants, ils comptent dans la population de leur ville
  d'origine, consomment de la nourriture mais ne produisent pas ; **leurs pertes coûtent de la
  population** à cette ville. Lever des soldats retire des travailleurs : une campagne pendant la
  récolte coûte cher (lien avec les saisons, `conception-saisons.md`).

À décider : chiffres (rythme de formation par type, effet des casernes, besoins d'équipement et
malus par type, effectifs, solde).

### Révision après l'analyse critique (décisions de Benjamin, 10/11/2026)
- **Armes produites en continu** par les bâtiments d'armement, chaque tour, selon leurs
  travailleurs (hors file de construction).
- **Unités civiles** (colon) produites dans la file de la ville, au prix d'habitants. **L'éclaireur
  est une unité militaire de faible coût**, recrutée comme les soldats.
- **Première version réduite** : habitants, armes en stock, entretien (nourriture et Économie),
  durée de formation fixe par type. L'expérience, l'attrition, l'équipement partiel et le
  recrutement avant la fin de la formation viendront ensuite, quand les IA sauront s'en servir.
- « Or » devient « Économie » (solde, butins).

### Révision 2 : cohérence (décisions de Benjamin, 10/11/2026)
- **Unités par ère, avec amélioration** : chaque type apparaît à une ère (par exemple milice,
  éclaireur, lancier au Néolithique ; archers et porteurs de bouclier au Bronze ; cavalerie au Fer ;
  cavalerie lourde au Moyen Âge ; répartition à régler). **En changeant d'ère, une unité existante
  s'améliore et son effectif grandit** : petites bandes au Néolithique, armées plus grandes ensuite.
  Les effectifs (500 à 1 000 habitants) écrits plus haut sont remplacés par des effectifs selon
  l'ère, à chiffrer avec la table de population (`conception-echelle-et-but.md`).
- **PM doublés et coûts du relief doublés aussi** (montagnes, forêts, fleuves) : le relief reste
  une barrière, la dette de déplacement garde son rôle. Les rayons de l'IA hostile (`HUNT_RANGE`,
  `WANDER_RADIUS`) et la zone dangereuse de l'exploration sont à recaler.
- **Armes** : les bâtiments d'armement sont des **emplois de la ville** ; l'intendant y affecte des
  travailleurs, qui ne produisent alors pas de Production pour la file ; le joueur règle l'effectif voulu.
- **Campagnes saisonnières** : voir le besoin de bras saisonnier (`conception-villes.md`).

## Armes et rôles (décisions de Benjamin du 10/11/2026, règles proposées)
Décisions : les archers restent exposés au corps à corps, même face aux lanciers ; le lancier est
faible face aux soldats à l'épée ; l'infanterie s'arme d'une **arme à une main avec bouclier** ou
d'une **arme à deux mains**. Les unités par ère sont dans `conception-recherche.md`, les ressources
qui les rendent possibles dans `conception-ressources.md`.

**Deux prises pour l'infanterie** :
| Prise | Exemples | Forces | Faiblesses |
|---|---|---|---|
| Une main + bouclier | porteur de bouclier, légionnaire, homme d'armes | défense, résiste aux traits | attaque moyenne |
| Deux mains | hache, khépesh à deux mains, épée longue, hallebarde | attaque forte, brise les boucliers | défense faible, exposé aux traits |

**Cycle des rôles** (bonus de travail à régler) :
- **Épée contre lance** : une fois la pointe passée, la lance est gênante. Épéistes (une ou deux
  mains) : +8 contre lanciers et piquiers. La pique à deux mains, sans bouclier, y est plus
  exposée que la lance avec bouclier.
- **Lance contre cavalerie** : bonus actuels (+10 lancier, +20 piquier), charge annulée de face.
- **Cavalerie contre tireurs et épéistes isolés en terrain ouvert** : charge, poursuite.
- **Tireurs contre infanterie lente** à distance, surtout les deux mains (sans bouclier).
- **Tireurs faibles au corps à corps contre toute infanterie**, lanciers compris : défense de
  mêlée basse (déjà le cas : 6 à 11). Proposition : au contact, un tireur attaqué en mêlée se
  défend à −25 % ; l'archer-épéiste échappe à ce malus (son épée).

## Découpage du développement
- **Étape 1 (fait) :** une unité par case, statistiques, PV, combat au corps à corps avec prévision, charge, dispersion des unités, hostiles qui attaquent.
- **Étape 2 (fait) :** tir à distance et ligne de vue, zone de contrôle, flanquement, bonus de terrain.
- **Altitude (fait) :** 6 niveaux, pentes et falaises, défense en hauteur.
- **Collines (fait) :** relief interne à la case, combinable avec tout terrain dont la forêt.
- **Plus tard :** fleuves, fortification, armes de siège, technologies.
