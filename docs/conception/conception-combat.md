# CivBuilder — Conception du combat (décisions)

_Dernière mise à jour : 23 octobre 2026 — étapes 1 et 2 réalisées, altitude ajoutée, collines devenues un relief interne à la case_

## Principes retenus
- **Une seule unité par case** (alliés compris).
- **À 0 PV, l'unité est dispersée** (détruite).
- **Pas de technologies pour l'instant** : toutes les unités sont disponibles d'emblée (à revoir plus tard).
- **Cavalerie dès maintenant.**
- **Reportés à plus tard :** fortification (doit à terme protéger des charges de cavalerie), armes de siège.

## Caractéristiques d'une unité
- Points de vie (100), attaque mêlée, défense mêlée, attaque à distance, portée, défense à distance, PM, coût.
- **Traits** plutôt que colonnes : bonus anti-cavalerie, bonus contre certaines cibles, charge, zone de contrôle…
- Pistes encore ouvertes : entretien en or, expérience.

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
- 6 niveaux de terre : 1 littoral, 2 plaine basse, 3 plaine haute, 4 plateau, 5 haut plateau, 6 massif (où se dressent les sommets, infranchissables). L'eau est au niveau 0.
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

## Découpage du développement
- **Étape 1 (fait) :** une unité par case, statistiques, PV, combat au corps à corps avec prévision, charge, dispersion des unités, hostiles qui attaquent.
- **Étape 2 (fait) :** tir à distance et ligne de vue, zone de contrôle, flanquement, bonus de terrain.
- **Altitude (fait) :** 6 niveaux, pentes et falaises, défense en hauteur.
- **Collines (fait) :** relief interne à la case, combinable avec tout terrain dont la forêt.
- **Plus tard :** fleuves, fortification, armes de siège, technologies.
