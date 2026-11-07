# CivBuilder — Érosion et graine du monde (version 0.3.0, en cours)

Implémenté dans `src/erosion.rs` (logique pure), `src/plates.rs` (`neighbor_graph`, `cell_area`), `src/map.rs` (branchement dans `generate_in`), `src/menu.rs` (graine), `src/wiki.rs`.

## Décisions de Benjamin (25/10/2026)
- Étape 5 (après les plaques et le relief qui en découle) : **érosion**, en attendant qu'elle réduise les **lacs** et les **cascades** (ruptures de pente le long des cours d'eau).
- Pouvoir **saisir la graine** de génération et regénérer avec la même graine.

## Érosion (`erosion::erode(elevation, neighbors, area, params)`)
Sur une planète, appliquée à l'altitude relative issue des plaques, avant le niveau de la mer, les altitudes en mètres, le climat et l'eau. La mer (sous 0) n'est pas touchée ; une case de terre reste de la terre.
- Voisines : grille qui boucle + lignes polaires (toutes leurs cases se touchent), distance = arc sur la sphère ÷ taille d'une case. Surface d'une case = cos(latitude).
- 10 passes ; à chaque passe :
  1. **Priority-Flood** depuis la mer : niveau de remplissage des cuvettes (jusqu'à leur seuil), case aval de chaque case, ordre de la mer vers l'amont ;
  2. **aire drainée** cumulée de l'amont vers l'aval (surfaces réelles) ;
  3. **creusement implicite** (loi « puissance du courant », exposant 0,5 de l'aire drainée, pente vers l'aval) sur la surface remplie : z ← (z + F z_aval) / (1 + F), F = 0,02 × A^0,5 / distance, de l'aval vers l'amont — jamais sous la case aval, stable même pour les grands fleuves ; le seuil d'une cuvette, traversé par toute son eau, s'use ;
  4. **comblement** : un fond de cuvette remonte de 8 % de sa profondeur sous le seuil (sédiments) ;
  5. **diffusion** des pentes (4 % vers la moyenne des voisines).
- Calage (graine 777) : lacs ≈ 270 cases (≈ 900 sans érosion, ≈ 30 avec un comblement de 25 %), ruptures de pente des fleuves (> 150 m vers l'aval) 23 sur 298 cases de fleuve (25 sur 195 sans érosion) ; graine 5 : 127 cases de lac. Les rivières de montagne gardent des ruptures (torrents).
- Durée : négligeable devant le climat.
- Tests : `une_cuvette_se_vide_ou_se_comble` (cuvette moins profonde, mer intacte), `les_grands_cours_d_eau_creusent_plus` (la vallée qui draine tout s'abaisse plus que les bords, profil toujours descendant).

## Vallées dans les chaînes, avant l'érosion (07/11/2026)
**Retour de Benjamin** (graine 3793283170, cordillère d'Omathurn) : beaucoup trop de cours d'eau, un rendu bizarre. Mesure : 92 % des cases de la chaîne touchent une rivière, en un « peigne » de rivières parallèles, une par rangée de cases, déjà présent sans le creusement après le climat. Une crête aux versants lisses ne rassemble pas l'eau ; relever le seuil de rivière ou bruiter l'écoulement ne suffit pas.

**Algorithme** (`erosion::with_valleys`, avant la première érosion) : dans les terres hautes (de 350 à 1 200 m environ, complet au-delà), un bruit à l'échelle de quelques cases (`VALLEY_ZOOM`) creuse des vallées et relève d'autant les crêtes voisines, jusqu'à ±35 % de l'altitude (`VALLEY_DEPTH`). L'érosion approfondit ensuite ces vallées.

**Essais** (graines 3793283170, 777, 8) : abaisser sans relever (35 %, 50 %, deux échelles) donne les mêmes confluences, mais perd 14 à 25 % d'altitude moyenne et jusqu'à 37 % des hautes terres ; relever les crêtes garde l'altitude ; ±50 % donne plus de falaises sans gain net.

**Bilan** (`bilan_incision`, 8 graines de référence, avant → après) : confluences pour 100 arêtes 5,1-5,8 → 7,3-8,1 (3 graines comparées) ; arêtes de rivière −14 % ; altitude moyenne gardée (écart de −1,3 à +1,2 %) ; cases à 1 500 m ou plus gardées (777 : 1 635 → 1 667 ; 8 : 2 687 → 2 728) ; embouchures : 9 sur 124 à 40 m ou plus, aucune à 500 m ; `verifier` à 0. **Prix à payer** : falaises 1 676 → 2 119 (+26 %) et chutes de fleuve 295 → 429 (+45 %), donc plus de passages infranchissables en montagne.

**Limites** : l'échelle des vallées est la même partout ; le bruit n'est pas orienté selon la pente. Les hautes terres restent assez arrosées pour rester touchées par une rivière à 50 % environ.

## Érosion par l'eau réelle, après le climat (07/11/2026)
**Décision de Benjamin** : l'érosion doit venir après le climat. La première passe voit seulement la surface drainée : un petit bassin très arrosé y compte pour un ruisseau, d'où des fleuves à 1 193 m au bord de la mer (Binvik, graine 4242 : 8 cases, 2 325 m³/s). Benjamin propose aussi de réutiliser la houle et le marnage.

**Algorithme** (`erosion::incise`, `WorldMap::incise_rivers`, une seule fois, à la génération) : une passe de climat, la houle, les marées et les embouchures donnent les débits et les flux, puis le creusement agit, puis tout le climat est refait sur le relief final (la pluie suit les vallées, et « regenerer climat » redonne la même carte). Même loi implicite que la première passe, sur 10 passes, F = 0,02 × (débit / 44 m³/s)^0,5 (44 m³/s : une case sous 1 000 mm/an), à partir de 100 m³/s. Les lacs et leur case de sortie ne bougent pas : l'abaisser les viderait. À l'embouchure, F est multiplié par √((houle + marée) / fleuve), entre 1 et 4, avec les flux de sédiments de `mouths.rs` : là où la mer emporte plus que le fleuve n'apporte, la vallée reste creusée ; le renfort s'éteint sur 3 cases (× 1, 0,6, 0,3). Classe d'altitude et relief suivent sur les cases abaissées et leurs voisines ; un pic reste un pic s'il en remplit encore les conditions.

**Bilan** (`bilan_incision`, graines 777, 4242, 8, 31337, sans → avec) : embouchures à 40 m ou plus 24 sur 65 → 4 sur 62 ; à 500 m ou plus 4 → 0 ; la plus haute 1 193 → 420 m ; altitude moyenne −6 à −8 % ; cases à 1 500 m ou plus 6 364 → 5 794 (−9 %) ; lacs 153 → 131 cases ; falaises 1 036 → 1 033 ; chutes de fleuve 197 → 150. Sur 8 graines : 9 embouchures à 40 m ou plus sur 119, 2 à 200 m ou plus, aucune à 500 m. Binvik : 1 193 → 15 m, et sa pluie 4 411 → 1 488 mm (le relief n'y soulève plus l'air de la même façon). Génération (graine 4242) : 25,5 → 35,7 s, à cause de la passe de climat en plus, mesuré une fois.

**Essais écartés** : réglage global de la première passe (× 3 ou 30 passes : 22 % de hautes terres en moins, 30 % de lacs en moins) ; second creusement seul à 0,05 (7 embouchures à 40 m ou plus sur 4 graines, mais lacs divisés par 2 et 27 % de falaises en plus).

**Limites** : les débits qui creusent viennent d'une seule passe de climat, sur le relief d'avant le creusement. Quelques embouchures restent perchées (la plus haute à 420 m, graine 8). Les réseaux par cases et par coins restent distincts.

## Graine du monde (menu Échap)
- Le menu affiche la graine du monde actuel dans un champ ; on tape des chiffres (Retour arrière pour effacer), puis Entrée ou « Regénérer avec cette graine ». « Nouveau monde (graine au hasard) » en tire une autre. Avertissement : la partie en cours est perdue.
- Mise en œuvre : le jeu se relance (nouveau processus, mêmes arguments et variables d'environnement, `CIVBUILDER_SEED` fixée ou retirée), la fenêtre actuelle se ferme. Repartir d'un processus neuf évite d'avoir à remettre à zéro tout l'état du jeu (carte, brouillard, relief 3D, étiquettes…).
- Même graine, même monde (génération déterministe) ; la graine apparaît aussi dans le journal au lancement (« Carte générée en … s (graine …) »).

## Limites, suite
- Les côtes au pied d'une cordillère de subduction restent souvent en falaise : les cours d'eau qui s'y jettent font des chutes (réalistes, mais nombreuses par endroits).
- Cols : créés par l'érosion, pas encore repérés ni nommés (passages de montagne).
