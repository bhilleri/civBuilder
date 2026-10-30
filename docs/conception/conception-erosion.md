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

## Graine du monde (menu Échap)
- Le menu affiche la graine du monde actuel dans un champ ; on tape des chiffres (Retour arrière pour effacer), puis Entrée ou « Regénérer avec cette graine ». « Nouveau monde (graine au hasard) » en tire une autre. Avertissement : la partie en cours est perdue.
- Mise en œuvre : le jeu se relance (nouveau processus, mêmes arguments et variables d'environnement, `CIVBUILDER_SEED` fixée ou retirée), la fenêtre actuelle se ferme. Repartir d'un processus neuf évite d'avoir à remettre à zéro tout l'état du jeu (carte, brouillard, relief 3D, étiquettes…).
- Même graine, même monde (génération déterministe) ; la graine apparaît aussi dans le journal au lancement (« Carte générée en … s (graine …) »).

## Limites, suite
- Les côtes au pied d'une cordillère de subduction restent souvent en falaise : les cours d'eau qui s'y jettent font des chutes (réalistes, mais nombreuses par endroits).
- Cols : créés par l'érosion, pas encore repérés ni nommés (passages de montagne).
