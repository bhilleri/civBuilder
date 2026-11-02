# Comment se joue CivBuilder : les actions en images

Chaque action du joueur ou de la faction hostile est animée sur la carte : les unités glissent
case par case, l'attaquant s'élance vers sa cible (ou tire un trait), la cible tressaille, les PV
perdus s'affichent en chiffres flottants, une unité dispersée s'efface, la récompense d'un village
apparaît sur place. Les règles, elles, sont résolues aussitôt : l'animation montre après coup ce
qui s'est passé (voir [la conception du combat](conception/conception-combat.md)).

Chaque action est montrée en **2D** puis en **3D**, avec l'interface du jeu. Le curseur dessiné
et la légende en haut de l'écran indiquent ce que fait le joueur.

**Commandes** : clic gauche sur une unité pour la sélectionner ; survoler une case montre le
chemin (le chiffre est le tour d'arrivée) ou la prévision du combat ; clic droit pour donner
l'ordre (se déplacer, attaquer, échanger de place) ; Entrée pour finir le tour.

> Ces GIF sont tournés par le jeu lui-même (graine 777) : voir [Refaire les GIF](#refaire-les-gif).

## Sommaire

- [Se déplacer](#se-déplacer)
- [Un trajet de plusieurs tours](#un-trajet-de-plusieurs-tours)
- [Échanger deux unités de place](#échanger-deux-unités-de-place)
- [Corps à corps](#corps-à-corps)
- [Charge et victoire](#charge-et-victoire)
- [Tir à distance](#tir-à-distance)
- [Une attaque perdue](#une-attaque-perdue)
- [Fin de tour : la faction hostile attaque](#fin-de-tour--la-faction-hostile-attaque)
- [Visiter un village](#visiter-un-village)
- [Prendre un camp vide](#prendre-un-camp-vide)

## Se déplacer

Clic gauche sur l'unité (son panneau s'ouvre en bas à gauche), survol de la destination : le
chemin s'affiche, chaque case avec son tour d'arrivée. Clic droit : l'unité part et glisse case
par case. Le coût d'une case dépend du terrain, du relief et de l'unité (la cavalerie légère a 4 PM).

| 2D | 3D |
|---|---|
| ![Déplacement en 2D](animations/deplacement_2d.gif) | ![Déplacement en 3D](animations/deplacement_3d.gif) |

## Un trajet de plusieurs tours

Une destination trop lointaine pour un seul tour est gardée en ordre : l'unité avance autant que
ses PM le permettent (le colon en a 2), puis reprend sa route à chaque fin de tour. Pendant le
passage du tour, le bouton de fin de tour est grisé jusqu'à la fin des animations.

| 2D | 3D |
|---|---|
| ![Trajet de plusieurs tours en 2D](animations/deplacement_plusieurs_tours_2d.gif) | ![Trajet de plusieurs tours en 3D](animations/deplacement_plusieurs_tours_3d.gif) |

## Échanger deux unités de place

Clic droit sur une de ses unités voisines : les deux unités échangent leurs cases.

| 2D | 3D |
|---|---|
| ![Échange en 2D](animations/echange_2d.gif) | ![Échange en 3D](animations/echange_3d.gif) |

## Corps à corps

Survoler un ennemi avec une unité sélectionnée affiche la **prévision du combat** dans
l'infobulle (dégâts attendus de part et d'autre, risque d'être dispersé). Clic droit : l'attaquant
s'élance, le coup porte pendant l'aller, la cible tressaille et les deux unités perdent des PV
(chiffres rouges ; les barres de PV baissent au moment du coup).

| 2D | 3D |
|---|---|
| ![Corps à corps en 2D](animations/corps_a_corps_2d.gif) | ![Corps à corps en 3D](animations/corps_a_corps_3d.gif) |

## Charge et victoire

Une cible plus loin que la case voisine : l'unité s'approche d'abord, puis attaque (la cavalerie
qui s'est déplacée charge). L'ennemi à 0 PV est **dispersé** : il s'efface, et le vainqueur
avance sur la case libérée.

| 2D | 3D |
|---|---|
| ![Charge en 2D](animations/charge_victoire_2d.gif) | ![Charge en 3D](animations/charge_victoire_3d.gif) |

## Tir à distance

Un archer tire jusqu'à 2 cases : le trait vole jusqu'à la cible (en cloche en 3D), le tireur
recule à peine et ne subit aucune riposte.

| 2D | 3D |
|---|---|
| ![Tir en 2D](animations/tir_2d.gif) | ![Tir en 3D](animations/tir_3d.gif) |

## Une attaque perdue

Une milice blessée attaque un guerrier au bouclier : la prévision annonce la défaite, et
l'attaquant est dispersé par la riposte (« Unité perdue »).

| 2D | 3D |
|---|---|
| ![Attaque perdue en 2D](animations/attaque_perdue_2d.gif) | ![Attaque perdue en 3D](animations/attaque_perdue_3d.gif) |

## Fin de tour : la faction hostile attaque

Entrée : fin du tour. Les actions du joueur se jouent d'abord, puis celles de la faction
hostile, l'une après l'autre (une action hors de vue n'est pas montrée). Ici un guerrier hostile
s'approche d'un éclaireur blessé et l'attaque.

| 2D | 3D |
|---|---|
| ![Attaque hostile en 2D](animations/attaque_hostile_2d.gif) | ![Attaque hostile en 3D](animations/attaque_hostile_3d.gif) |

## Visiter un village

Entrer dans un village (ou le traverser) a trois issues possibles, annoncées à l'arrivée de l'unité.

**Échanges commerciaux** : de l'or (45 % des cas).

| 2D | 3D |
|---|---|
| ![Village, commerce, en 2D](animations/village_commerce_2d.gif) | ![Village, commerce, en 3D](animations/village_commerce_3d.gif) |

**Les habitants rejoignent la ville la plus proche** : +1 habitant (35 %).

| 2D | 3D |
|---|---|
| ![Village, habitants, en 2D](animations/village_habitants_2d.gif) | ![Village, habitants, en 3D](animations/village_habitants_3d.gif) |

**Village hostile** : une embuscade, une ou deux milices hostiles surgissent (20 %).

| 2D | 3D |
|---|---|
| ![Village, embuscade, en 2D](animations/village_embuscade_2d.gif) | ![Village, embuscade, en 3D](animations/village_embuscade_3d.gif) |

## Prendre un camp vide

Un camp hostile laissé sans défenseur est dispersé par la première unité qui y entre : 25 or.

| 2D | 3D |
|---|---|
| ![Camp en 2D](animations/camp_2d.gif) | ![Camp en 3D](animations/camp_3d.gif) |

## Refaire les GIF

Les GIF sont tournés par le jeu (module `src/film.rs`) : il joue chaque scène en pilotant une
souris dessinée, avec les vraies commandes du joueur, fait avancer le temps de 1/20 s par image
(GIF réguliers même sur une machine lente), enregistre chaque image puis les assemble avec
**ffmpeg** (à installer). Les issues des combats sont fixées par les PV de départ des unités, celle
du village est imposée.

```bash
scripts/animations.sh                      # toutes les scènes, dans docs/animations (≈ 10 min)
SCENES=tir,camp scripts/animations.sh      # seulement ces scènes
```

Sous Windows (PowerShell), sans le script :

```powershell
$env:CIVBUILDER_SEED=777; $env:CIVBUILDER_CAPTURE="docs/animations"; $env:CIVBUILDER_CAPTURE_GIF=1; cargo run --release
```

`CIVBUILDER_CAPTURE_GIF=tir,camp` limite aux scènes nommées ; `CIVBUILDER_CAPTURE_KEEP=1` garde les
images PNG de chaque scène. Ajouter une scène : une entrée de `SCENES` dans `src/film.rs`
(unités, site, étapes : pointer une case, clic, fin de tour, attente), puis une section ici.
