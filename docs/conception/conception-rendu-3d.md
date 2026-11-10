# Conception — Rendu 3D : terrain continu, eau animée, unités et structures

## Objectif
Améliorer nettement la vue 3D (la 2D reste la référence du jeu) :
- eau animée qui suit les courants simulés (léger courant) ;
- relief propre, de bien meilleure qualité, avec un passage progressif d'une case à l'autre ;
- biomes et altitude faciles à distinguer, textures réalistes ;
- terrain soigné mais discret, qui laisse la place aux unités et aux structures ;
- structures visibles et reconnaissables, intégrées à leur case ;
- unités en modèles 3D reconnaissables et animés, animations de déplacement et d'attaque propres à
  chaque type et à ses compétences.

## État de départ (version 0.3.1, captures graine 777)
- Une case = un maillage (≈ 21 000 entités en 192 × 111), couleurs par sommet, `StandardMaterial`
  sans texture, aucun shader propre au jeu. Toute la carte se construit en 6,9 s ; un changement
  (brouillard, carte thématique) reconstruit des maillages.
- Centre de case plat et d'une seule couleur : les biomes forment une mosaïque d'hexagones, seul le
  raccord de hauteur adoucit le passage (`relief.rs`).
- Montagnes en dômes doux ; neige d'un blanc uniforme qui écrase les massifs ; l'altitude se lit mal de loin.
- Eau plate et immobile ; courants (`currents.rs`) et marées (`tides.rs`) ne servent qu'aux flèches.
- Figurines en formes simples (`figures.rs`), animées par déplacement et changement de taille
  (`feedback.rs`). Point fort à garder : `feedback.rs` sépare la logique des animations (`Cues`).

## Décisions de Benjamin (09/11/2026)
- **Terrain continu, grille dessinée par-dessus**, avec un léger aplat au centre de chaque case
  pour poser unités et villes. La grille fine s'affiche selon le zoom ou le mode.
- **Style « maquette » semi-réaliste** : textures réalistes, couleurs et contrastes maîtrisés,
  relief un peu exagéré ; un diorama lisible.
- **Relief marqué et réglable** : bien lisible par défaut, exagération verticale réglable dans les
  options de carte (à 40 km par case, l'échelle réelle donnerait des montagnes presque plates).
- **Modèles mélangés** : packs CC0 (Quaternius, KayKit…, licence vérifiée pack par pack) pour les
  squelettes, animations et bases, retouchés dans Blender aux couleurs du jeu ; modélisation
  maison pour ce qui manque.
- **Saisons** : ce sont un élément du jeu, pas seulement un effet visuel. Il faut d'abord définir
  leurs règles de jeu ; leur rendu (neige saisonnière, feuillage, savane verte ou sèche) suivra.
  Le shader de terrain doit seulement prévoir de recevoir la saison.

## Principe : terrain continu en blocs, données dans des textures
On cesse de faire une case = un maillage. Le terrain est découpé en **blocs** (par exemple 8 × 8
cases) dessinés par un **shader de terrain** (WGSL, `ExtendedMaterial<StandardMaterial, …>`) à
partir de quatre textures :

| Texture | Contenu | Rôle |
|---|---|---|
| Hauteurs (≈ 24 points par case, en mètres) | Relief fin | Géométrie, normales, ombrage |
| Mélange | Part de chaque matière du sol en chaque point | Passage progressif entre biomes |
| Données par case (1 point par case) | Biome, brouillard, territoire, surbrillance, carte thématique | Superpositions sans reconstruire de maillage |
| Écoulements | Vecteurs des courants marins et des fleuves | Eau animée |

- Les cases restent la logique du jeu ; à l'écran, la grille est dessinée par le shader.
- Changer de carte thématique, révéler le brouillard ou afficher une portée = mettre à jour une petite texture.
- Bord est-ouest : textures répétées horizontalement (règle de la planète, voir `conception-planete.md`).
- Une copie des hauteurs reste côté processeur pour poser les objets (`on_ground`) et la sélection à la souris.
- Génération déterministe : toutes ces textures se déduisent du monde, sans tirage nouveau.

## Techniques retenues par sujet
### Relief
- Altitudes réelles interpolées entre les centres des cases (aplat léger au centre, ≈ 25 %).
- Bruit à crêtes orienté par les chaînes et massifs connus (`mountains.rs`, `lands.rs`).
- Érosion fine à la résolution du rendu : vallées en arbre qui suivent les fleuves générés, lits creusés dans les hauteurs.
- Normales calculées dans le shader, ombres portées, occlusion ambiante pré-calculée une fois.
- Exagération verticale réglable.

### Transitions et matières du sol
- Poids des biomes selon la distance aux centres des cases, bord déformé par un bruit (contours irréguliers).
- Mélange selon la hauteur des textures (l'herbe s'infiltre entre les cailloux).
- Règles du shader : roche sur les fortes pentes ; neige selon l'altitude, la latitude et
  l'orientation (et plus tard la saison), jamais sur les falaises ; sable près du niveau de la mer ;
  sol humide le long des cours d'eau.
- 12 à 16 matières (couleur, normales, rugosité) dans un tableau de textures, sources CC0
  (ambientCG, Poly Haven, licence vérifiée).
- Deux échelles : teinte d'ensemble issue de la simulation (distingue les biomes de loin), texture
  de près qui se fond dans la teinte avec la distance. Mélange aléatoire des tuiles contre la
  répétition ; projection sur trois axes seulement sur les fortes pentes.

### Lisibilité (terrain discret)
- Terrain : saturation et contraste de luminosité resserrés. Unités et structures : couleurs plus
  vives, couleur de camp, liseré clair sur le contour.
- Clairière automatique : la végétation s'éclaircit sous une unité ou une structure.
- Courbes de niveau optionnelles (tous les 250 m), éclairage rasant de type cartographique, brume de distance légère.

### Eau
- Pas de simulation de fluide en temps réel. Carte des écoulements tirée des courants et des
  marées ; les normales de l'eau défilent dans le sens et à la vitesse du courant (deux phases
  décalées) ; le marnage varie sur un cycle ; un bruit léger casse la régularité.
- Fleuves : écoulement le long de l'axe connu (`WaterAxis`), vitesse liée au débit, écume aux cascades et confluences.
- Houle : 4 à 6 ondes superposées ou deux cartes de normales qui défilent.
- Couleur selon la profondeur (fonds marins), écume et ressac au rivage (distance à la côte),
  transparence en eau peu profonde, reflets de l'environnement et effet de Fresnel.
- Lacs calmes, marais ternes, banquise aux pôles.

### Structures
- Modèles glTF (2 000 à 5 000 triangles), silhouettes reconnaissables du dessus.
- Décalques au sol (sol tassé, champs, chemins), champs autour de la ville selon les cases
  exploitées, ville qui grandit par quartiers posés sur le relief.
- Règles des structures : muraille liée à la fortification, matériaux selon le lieu, pas de superposition ni de remblai.

### Unités et animations
- Modèles glTF articulés, peu de squelettes (humain, cheval et cavalier, chien, barque),
  équipement accroché aux os, masque de couleur de camp. Groupes de figurines et icônes de loin conservés.
- Graphe d'animation de Bevy : repos, marche, course ou galop, attaque, tir, coup reçu, mort, victoire.
- Table par type d'unité, pilotée par les `Cues` : charge du cavalier (accélération, impact, recul
  de la cible), piques abaissées, volée de flèches en cloche, flèches qui butent sur les arbres en
  forêt (tir gêné).
- Au sol : orientation vers la cible, inclinaison sur la pente, poussière, neige ou boue selon le
  terrain, éclaboussures au gué (particules : `bevy_hanabi` si une version compatible existe).

### Autres
- Végétation instanciée qui ondule selon le vent simulé ; canopée continue au loin.
- Brouillard de guerre à bord doux, stylisé (nuages ou parchemin).
- Ombres de nuages, brume de vallée.
- Portée, trajet et frontières de territoire (dégradé intérieur) dessinés par le shader, sur le relief.
- Rendu final : tonemapping AgX, occlusion ambiante d'écran, léger halo sur l'eau, anticrénelage.
- Rechargement à chaud des shaders pendant le développement (`file_watcher`).

## Étapes
| Étape | Contenu | Risque |
|---|---|---|
| 1 | Terrain continu en blocs, hauteurs, shader de terrain, données par case, grille par-dessus (prototype sur une partie de la carte d'abord) | Élevé (refonte de `view3d.rs` / `relief.rs`) |
| 2 | Matières du sol, règles pente / altitude / neige, réglage de la lisibilité | Moyen |
| 3 | Eau : houle, écoulements, profondeur, rivage | Moyen |
| 4 | Végétation instanciée, clairières, vent | Faible |
| 5 | Unités glTF animées, animations par type et compétence, particules | Moyen (production des modèles) |
| 6 | Structures glTF, décalques, villes qui grandissent | Moyen |
| 7 | Saisons visuelles (après les règles de jeu des saisons), nuages, brouillard stylisé | Faible |

Chaque étape se vérifie par captures (`CIVBUILDER_CAPTURE`, graines 777, 4242, 2) avant et après,
et la fluidité par `travelling` (budget : 60 images/s tenues, comme en 0.3.1).

## Constats techniques de l'analyse critique (10/11/2026)
Vérifiés sur le code ; à traiter au prototype de l'étape 9 a.
- **Lits de rivière** (bloquant) : une case mesure ≈ 55 unités (`HEX_SIZE` = 32), 24 points par case
  donnent un point toutes les 2,3 unités, pour des rivières de 2 unités de large
  (`RIVER_MIN_HALF_WIDTH` = 1). Doubler la résolution dépasse la limite de 8 192 par texture.
  Le prototype tranche entre : lits dans une couche à part (segments par bloc lus par le shader,
  même fonction `relief::carve` côté processeur), ou seuls les fleuves gravés dans les hauteurs.
- **Textures par bloc** (8 × 8 cases avec une marge pour les normales), pas une seule grande :
  sinon chaque ville fondée ou modification renvoie 25 à 50 Mo.
- **Hauteurs indépendantes du brouillard** : aujourd'hui le relief et le niveau de l'eau dépendent
  des cases explorées (`unexplored_height`, `Relief::water_level`). Calculer sur le relief réel et
  masquer dans le shader, y compris pour le prépass, les ombres et `ground()`.
- **Raccord est-ouest** : placer chaque bloc et objet sur sa copie la plus proche de la caméra
  (une seule copie suffit à la distance maximale), plutôt que des caméras fantômes (`seam.rs`),
  qui posent problème avec l'occlusion ambiante, l'anticrénelage temporel et les ombres.
- **Déplacement dans le vertex shader** : prépass à adapter, boîtes englobantes à poser, jupes entre
  LOD ; objets et gizmos seulement sur le LOD 0 (`ground()` en bilinéaire).
- **Pas de texture de mélange** : les règles se calculent dans le shader depuis la texture par
  case ; matières en 1K compressées (BC7, KTX2), ≈ 60 à 90 Mo au lieu d'environ 1 Go.
- **Exagération verticale** : hauteurs stockées en mètres, exagération passée au shader, décalage
  des lits à part ; tout ce qui est posé (villes, ponts, labels) se replace quand elle change.
- **Modèles animés** : Bevy ne recible pas les animations ; un seul squelette humain de référence.
  Options de Cargo à ajouter en une fois (glTF, animation, KTX2, zstd, LUT de tonemapping pour AgX,
  `file_watcher`), car elles recompilent.
- **Crates** : compatibilité de `bevy_hanabi` et des contours avec Bevy 0.20 non garantie ; un
  système de particules maison suffit (comme `CampSmoke`).
- **Végétation** : une entité par élément aujourd'hui (`Decor3d`) ; fusionner par bloc au-delà
  d'environ 100 000 éléments.
- **Migration sans big bang** : étape 0 = une ressource ou un trait `Ground` (l'API utilisée hors de
  `view3d` est étroite : `ground`, `on_ground`, `hovered_ground_3d`), bascule entre l'ancien et le
  nouveau terrain par une option ; **comparaison des captures (tâche 24) avant** ; profil « qualité
  basse » pour le film et les GIF (rendu logiciel).
- **Prototype minimal (2 à 3 jours)** : cuire le relief actuel (graine 777) en textures sur
  4 × 4 blocs autour de la capitale et une zone à cheval sur le raccord ; grille partagée,
  déplacement en vertex shader, texture par case, grille dessinée ; mesurer images/s
  (`travelling`), mémoire, temps de cuisson, écart sous les unités, rivières à 24 et 48 points par case.

## Limites et questions ouvertes
- Résolution des hauteurs (≈ 24 points par case ≈ 4 600 × 2 700 en 192 × 111) à valider au prototype, avec les cartes plus grandes.
- Compatibilité avec Bevy 0.20 des crates envisagées (particules, contours) à vérifier avant de s'en servir.
- Règles de jeu des saisons : décidées (`conception-saisons.md`).
