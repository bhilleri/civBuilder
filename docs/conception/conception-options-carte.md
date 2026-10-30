# CivBuilder — Options de carte, infobulle et surbrillance (24/10/2026)

Modules : `src/options.rs` (panneau), `src/tooltip.rs` (rubriques de l'infobulle), `src/highlight.rs` (surbrillance et carte du réseau), `src/labels.rs` (étiquettes), `src/ui.rs` (texte de l'infobulle, légendes).

## Demandes de Benjamin

- Intégrer les **lacs** à la surbrillance.
- Surbrillance **sur option**, dans un menu qui règle le comportement de la carte (avec les étiquettes) : **au survol du cours d'eau** (comportement d'origine), **au survol de l'étiquette**, **aucune**.
- **Infobulle allégée et configurable pour chaque carte** : par défaut seulement l'utile au joueur (coût en PM, biome, climat, production), plus ce qui concerne la carte affichée.
- Affluents affichés **de proche en proche** (récursivement), **en jaune**.
- Tracés **reliés aux confluents** et **sens du courant animé**.
- **Aval récursif** (jusqu'où va l'eau) dans une autre couleur ; **petits bras** d'un fleuve tracés (une rivière qui se jette dans un bras d'une case n'était pas reliée).
- **Carte globale du réseau hydrographique** : coloration selon le rang de chaque cours d'eau ou lac par rapport à son déversoir, avec le courant.

## Panneau « Options de carte » (bouton ou touche O)

- **Étiquettes** : bascules « Cours d'eau », « Lacs et mers », « Villes » (`LabelFilter`) ; la touche **N** masque tout ou réaffiche tout.
- **Surbrillance des cours d'eau et des lacs** (`HighlightMode`) : « Au survol du cours d'eau » (défaut), « Au survol de l'étiquette », « Aucune ». En mode étiquette, les étiquettes reçoivent une `Interaction` (seulement dans ce mode).
- **Infobulle** : tableau rubriques × cartes (Normale, Relief, Climat, Bassins, Débits, Réseau) ; un clic sur une case affiche ou masque la rubrique sur cette carte. Réglages en mémoire pour la partie.
- Panneau opaque, au-dessus du reste de l'interface ; il bloque le survol de la carte.

## Rubriques de l'infobulle (`tooltip::Info`) et réglages par défaut

| Rubrique | Normale | Relief | Climat | Bassins | Débits | Réseau |
|---|---|---|---|---|---|---|
| Terrain | oui | oui | oui | oui | oui | oui |
| Altitude (niveau) | | oui | | | | |
| Climat | oui | oui | oui | oui | oui | oui |
| Température, précipitations, latitude | | | oui | | | |
| Rendements | oui | oui | oui | oui | oui | oui |
| Coût de déplacement | oui | oui | oui | oui | oui | oui |
| Cours d'eau (traversée, eau douce) | | | | oui | oui | oui |
| Lac ou mer intérieure (nom, nature, taille) | | | | oui | oui | oui |
| Débit de la case | | | | | oui | |
| Bassin versant | | | | oui | | |
| Affluents et embouchure | | | | oui | oui | oui |

Toujours affichés quand ils s'appliquent : village, camp, unité hostile, aperçu de combat, trajet.

## Surbrillance (`highlight.rs`)

- Élément nommé (`Feature`) : cours d'eau ou lac. Sous la souris : le lac de la case, sinon son fleuve, sinon la plus abondante de ses rivières nommées.
- **Récepteur** : un cours d'eau se jette dans un autre (`flows_into`) ou dans un lac (`into_lake`) ; un lac se déverse dans le cours d'eau qui en sort (`NamedLake::outflow`).
- **Réseau** (`network`) : élément survolé en **rouge**, récepteur en **bleu**, **suite du chemin de l'eau** au-delà du récepteur jusqu'au déversoir final en **violet** (`downstream`), affluents directs en **orange**, affluents des affluents de proche en proche en **jaune**. Un lac est tracé par son pourtour.
- Tracé d'un fleuve : son cours plus ses **petits bras** (`NamedRiver::arms` : tronçons de moins de 3 cases qui portent son nom, partant du milieu de son lit). Une rivière qui se jette dans un fleuve ou dans un petit lac qu'il traverse rejoint le centre de cette case.
- **Chevrons blancs animés** dans le sens du courant.
- Seules les parties explorées sont tracées ; l'infobulle ne cite que les éléments connus (« En rouge : X — se jette dans : Y (en bleu) — puis : Z → W (en violet) — affluents (en orange) : … — et 4 plus en amont (en jaune) »).

## Carte du réseau hydrographique (`MapLayer::Network`)

- Fond neutre (terres crème, lacs bleu pâle, mer bleu gris) ; tous les cours d'eau et lacs nommés connus tracés : fleuves en traits épais (3,5 px en 2D), rivières et pourtours de lacs en traits fins (2 px) ; en 3D, lignes posées sur le relief (gardées d'une image à l'autre, recalculées quand l'exploration ou la forme du sol changent ; voir `conception-altitude.md`, « Performances »).
- Couleur selon le **rang par rapport au déversoir final** (`NamedRiver::level`, `NamedLake::level`, calculés dans `basins.rs`) : 0 bleu nuit (se jette dans la mer, un lac salé ou une mer intérieure), 1 bleu, 2 turquoise, 3 vert, 4 et plus olive. Un cours qui se jette dans un autre a un rang de plus ; un lac a le rang du cours qui en sort ; le **principal cours qui entre dans un lac garde le rang du lac** (il le traverse), les autres ont un rang de plus.
- Chevrons du courant d'une teinte plus sombre que le trait.
- Le survol garde ses couleurs : le réseau survolé n'est pas dessiné par la carte, mais par la surbrillance par-dessus.
- Graine 777 : 132 cours de rang 0, 99 de rang 1, 37 de rang 2, 4 de rang 3.

## À faire ensuite

- Enregistrer les options sur disque pour les retrouver d'une partie à l'autre.
- Éventuellement une carte des températures.
- Carte du réseau : largeur du trait selon le débit (aujourd'hui deux largeurs) ; teinte de l'eau dessinée plus claire sur cette carte.
