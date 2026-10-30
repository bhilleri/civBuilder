# Conception — Toponymie : zones nommées, affichage, attribution des noms

Objectif : nommer des zones reconnaissables pour que les joueurs puissent en parler (« des troupes arrivent par la Forêt Noire », « fonde une ville sur le détroit de Vaskar pour le contrôler »), sans rendre la carte illisible.

## État (25/10/2026)
- **Décidé** : langues « en partie inventées » (mots inventés dans le style d'une vraie famille + quelques vraies racines choisies d'après le lieu) ; génériques et descriptions en français ; recherche insensible aux accents (les noms gardent leurs accents).
- **Étape 1 faite** (module `toponymy.rs`) : 8 langues (nordique, latine, slave, turque, berbère, bantoue, nahua, polynésienne), aires de langue, générateur, grammaire ; cours d'eau, lacs, mers intérieures, bassins et plaques renommés ; **villages tribaux et camps barbares nommés** (ajout demandé), étiquettes cliquables sous leur icône, catégorie de recherche « Villages et camps » ; origine du nom dans l'infobulle.
  - Aires : 8 graines au hasard sur les terres, k-moyennes sur les terres (12 passes) → aires de terre comparables ; frontières déformées par un bruit sur la sphère ; chaque langue va à une aire selon le climat (température, aridité, part de mer pour le polynésien), meilleure répartition d'ensemble (8! permutations).
  - Mot : préfixe rare + racine du sens voulu (1 fois sur 2 quand un sens est demandé) ou 1–2 syllabes (attaque / voyelle / coda) + terminaison typique (55 %) ; raccords sans double voyelle ni triple consonne ; 4 à 11 lettres.
  - Lacs : ~30 % descriptifs selon les rives (Lac Gelé, Lac des Cimes, Lac des Bois, Grand Lac, Grand Lac Salé…) ; mers intérieures : « Mer d'X » ou Mer Gelée / Mer des Sables / Mer Amère. Rivières (pas les fleuves) : ~10 % descriptives selon la source (Rivière Blanche, Noire, Rouge…). Affluent fleuve ≥ 50 % du débit du fleuve récepteur : jumeau (« Vaska Blanche », « Petit Taldor »).
  - Camps : 65 % « Camp / Repaire / Antre / Fort » + bête du pays selon le climat (des Loups, des Hyènes, du Serpent…), sinon « Camp de X » ; villages : nom propre.
  - Unicité globale (insensible à la casse), déterminisme par la graine.
- **Étape 2 faite** (module `seas.rs`), revue deux fois après les retours de Benjamin (1 : une mer ouverte doit être une partie de l'océan ; pas de découpage arbitraire des grands océans ; golfes jusqu'à la ligne des caps ; hiérarchie golfe › mer › océan. 2 : pas de mer en anneau ou avec un creux ; question d'échelle — golfe du Mexique ≈ 1 100 cases de 40 km, donc pas de limite haute aux golfes ; baies 3–20 cases ; utiliser le plateau) :
  - Distance à la côte ; découpage « par persistance » des cases les plus au large aux plus côtières : deux étendues restent séparées si le passage qui les relie est moins large que 65 % de la moins large (les étendues de moins de 40 cases fusionnent toujours).
    - **Partage des eaux (30/10)** : chaque case rejoint l'étendue de sa voisine la plus au large (la pente la plus raide de la distance à la côte), à égalité celle au sommet le plus haut ; les cases d'un même niveau sont prises par propagation (parcours en largeur) depuis les cases déjà rangées, puis les autres (nouveaux sommets). Avant : à niveau égal, ordre des lignes et étendue au sommet le plus haut parmi les voisines → la frange côtière (distance 1-2) était captée de proche en proche par l'étendue du nord et un océan descendait en ruban d'une case le long des côtes jusqu'à l'océan voisin (graine 22893809 : Océan Boréal jusqu'à l'Austral de part et d'autre des deux océans du milieu ; aussi 777, 4242). Après : graine 22893809 Boréal 6 169, Austral 4 461, deux océans de 1 393 et 1 149 cases ; 777 : 5 océans (4 avant) ; 4242 : 3 ; 2 : 6 (5 avant).
    - Une mer bordière n'est gardée que si son océan garde au moins 15 % (1 − 0,85) de ses cases au large : sinon un océan étroit (graine 22893809, 1 149 cases) était couvert à 100 % par deux mers bordières et son nom n'avait pas de place.
    - **Océans polaires au 60e parallèle (30/10, décision de Benjamin, « règle B »)** : la bande polaire (où la distance à la côte n'est bornée que par les terres, le bord n'étant pas une côte) était coupée en plusieurs océans là où un continent la resserrait à moins de 65 % de sa largeur, par une limite droite en pleine mer (graine 2 : trois océans au sud, deux au nord). Désormais, au-delà de 60° (comme l'océan Austral des atlas), le plus grand morceau d'un seul tenant de chaque pôle est l'océan polaire, sans découpage ; il ne prend pas part au partage des eaux et s'arrête au parallèle. Seulement sur une carte qui va jusqu'au pôle (pas sur une carte régionale). Les étendues trop petites pour être des océans rejoignent leur voisine (celle qui partage le plus de pourtour), avec une exception pour l'océan polaire (30/10, retour de Benjamin sur la graine 3309035769, où l'Océan Boréal descendait jusqu'à la Mer des Moussons, près de l'équateur) : il n'absorbe une étendue d'au moins 150 cases (`CLOSED_SEA_MIN_CELLS`) que si elle reste à moins de 10° sous son parallèle (`POLAR_FRINGE_DEG` : une frange) ; sinon l'étendue reste à part, mer (ou océan si elle en a la taille). Les plus petites (échancrures) le rejoignent toujours et peuvent y devenir golfe ou baie. `compute` reçoit pour cela, par case, son éloignement en degrés au-delà du 60e parallèle.
      - Chiffres (après l'exception) : 3309035769 Boréal 3 345 cases (lignes 0 à 20), le bassin qui descendait devient la Mer Sombre (1 040 cases) ; l'océan polaire ne dépasse plus 10° sous son parallèle, sauf petites échancrures (graine 2 : 157 cases jusqu'à ~33° N). Les mers de bassin (étendues à part, sans océan parent) sont plus nombreuses : 4242 en a 8, de 500 à 820 cases, entre les continents du nord et du sud.
      - Pas de mer bordière dans une mer à part (30/10, graine 985384862 : Mer de Timanfan, 180 cases, dans la Mer Sombre, 252 cases ; les deux noms se superposaient) : comme pour les mers fermées, une mer bordière n'est cherchée que dans un océan. Invariant testé (`monde_genere`, graines 777 et 985384862) : une mer a pour parent un océan ou rien.
      - Règle écartée (« A ») : aucun passage de la bande polaire ne sépare deux étendues. Bandes polaires entières, mais un grand océan qui s'ouvre largement sur la bande fusionne avec elle (graine 777 : un océan de ~6 000 cases du centre au pôle sud, sans Océan Austral) : un océan très horizontal avec de grosses excroissances vers le sud ou le nord. La règle B accepte de petites variations le long du parallèle et évite ces formes.
      - Chiffres (règle B) : 22893809 Boréal 5 031, Austral 4 281, trois océans de 1 127 à 1 580 cases ; 777 : 5 océans (nord 3 840, Austral 3 631) ; 4242 : Boréal 7 506, Austral 6 170 ; 2 : 4 océans (6 avant) ; 5 : 3 ; 31 : 6.
      - À voir : le nom « Océan Boréal / Austral » reste tiré au sort parmi les noms possibles (graine 777 : océan polaire nord « Igrisza »).
  - Classement : étendue ≥ max(700 cases, 8 % de la mer) = océan ; plus petite : fondue dans sa voisine (**30/10, décision de Benjamin** : le découpage ne sert plus qu'à séparer les océans ; plus de mer fermée par distance à la côte ni de « mer polaire », le bord n'étant plus une côte). Une étendue isolée (sans voisine) garde son bassin. Pas de partage artificiel des grands océans : leur nom est répété (une fois par 2 500 cases, 4 au plus, à ≥ 35 cases d'écart, sur des cases bien au large) et se pose au large de leurs golfes et mers bordières (ces cases comptent comme bord pour le choix de la case du nom).
  - **Golfes, baies, mers bordières à l'échelle (refonte du 25/10)** — définitions retenues : golfe = grande échancrure de la mer dans une terre, entrée plus étroite que l'intérieur, de toute taille (Mexique, Persique, Gascogne ; on accepte aussi les grandes échancrures ouvertes appelées golfes, Guinée, Alaska) ; baie = petite échancrure (test du demi-cercle du droit de la mer) ; mer bordière = partie d'un océan à l'abri entre plusieurs terres (côte et îles, arc d'îles), souvent sur le plateau.
    - Algorithme : « ouverture morphologique » à 8 échelles (rayons 2, 3, 4, 5, 6, 8, 10, 13). À l'échelle r, centres = cases à ≥ r de la côte ; le large = le plus grand groupe de centres de chaque océan / mer ; cases balayées = à ≤ r − 1 d'un centre du large. Poche = cases non balayées d'un seul tenant ayant un cœur à ≥ 2 cases de la mer balayée (une frange d'une case le long d'une côte découpée ne compte pas) + 2 cases de frange.
    - Terres riveraines : une terre ≥ 70 % du rivage (ou une seule terre notable) → golfe/baie, sinon les terres ≥ 15 % du rivage (≥ 3 cases) → mer. Prolongement jusqu'à la ligne entre les caps (case ayant une de ces terres des deux côtés à ≤ max(2r + 2, 3r) cases).
    - **Mers fermées (30/10, cas « mer Baltique »)** : poche très fermée (entrée ≤ 0,33 × largeur intérieure, largeur = 2 × éloignement maximal à la côte) d'au moins 150 cases, quelles que soient ses terres (choix de Benjamin : fermeture + taille ; plus petite ou plus ouverte : golfe). Première passe : seules les mers fermées sont retenues (plus grandes échelles d'abord, contours lissés comme les golfes). Seconde passe : chaque mer fermée devient une étendue à part (`region_of`, identifiant ≥ `CLOSED_BASE`) balayée par son propre cercle ; golfes et baies y sont cherchés, pas de mer bordière dedans. Rangées dans `coastal_of` (parent : l'océan). Avant : limite en dents de scie et partie de la mer atteinte par le chenal rattachée à l'océan (« Mer des Glaces »). Sur 11 graines (777, 4242, 2, 5, 31, 50, 1, 3, 9, 12, 20), aucune poche ne remplit les deux conditions : les grandes échancrures sont ouvertes (ex. graine 777, 905 cases, entrée 45 pour une largeur de 28 : golfe). Une mer fermée plus grande que le seuil des océans (≥ 700 cases) reste un océan à part (comme avant).
    - **Seuils sous-marins (30/10)** : la fermeture se mesure au seuil, la plus étroite des tranches de la poche parallèles à l'entrée (cases à même éloignement de l'entrée, `depth`) parmi la première moitié (≤ profondeur max / 2) ; une case du plateau (< 200 m) y compte pour `SILL_WEIGHT` = 0,5 (demi-fermeture). Mer fermée si seuil ≤ 0,33 × largeur intérieure ; elle commence au seuil (cases en deçà rendues au large), 150 cases au moins après coupe. Test `seuil_sous_marin` (chenal de 6 cases : golfe ; avec un haut-fond de 3 rangées en travers : mer fermée à partir du haut-fond). **Bilan** (mêmes 11 graines) : toujours aucune mer fermée ; les grandes poches sont des entonnoirs qui se resserrent vers le fond (ex. graine 5 : tranches 27 → 4 cases), sans passage étroit à l'entrée (seuils de 9 à 40 cases pour une largeur intérieure de 12 à 28). Le relief issu des plaques ne crée presque jamais de mer à entrée étroite : piste, si Benjamin en veut, du côté de la génération (continents qui se rapprochent, isthmes).
    - Tests : entrée = cases qui donnent sur la mer extérieure ; profondeur = éloignement max à l'entrée. Golfe : surface ≥ 0,15 × entrée², profondeur ≥ 3 et ≥ entrée / 3 ; baie : 0,3, profondeur ≥ 2. Golfe ≥ 21 cases sans limite haute ; baie 3–20 cases (40 au plus, les plus fermées d'abord) ; mer bordière 50–1 200 cases, terre sur ≥ 50 % du pourtour, plus petite qu'un océan. Aucune poche qui touche les 2 rangées polaires (le bord de carte n'est pas une côte). Une poche qui fait > 85 % de son étendue (océan isolé ou mer fermée) est cette étendue.
    - Choix : les plus grandes échelles d'abord ; un golfe déjà pris à > 50 % = le même golfe vu de plus près ; une baie dans un golfe seulement si elle en fait ≤ 40 %. Mer bordière prolongée sur le plateau voisin (≤ 3 cases, seulement en épaisseur : case touchant ≥ 2 cases de la mer).
    - Contours : cases entourées (4 voisines sur 6) ajoutées, trous d'eau enfermés bouchés, pointes et doigts d'une case retirés, plus grand morceau gardé.
    - Hiérarchie : baie › golfe › mer (fermée ou bordière) › océan (`bay_of`, `inlet_of`, `coastal_of`, `basin_of` ; `chain()` donne détroit, baie, golfe, mer, océan). Parent : l'étendue qui contient plus de la moitié des cases.
    - Résultat (graines 777 / 4242 / 31 / 2 / 5 / 50) : 4–6 golfes (jusqu'à ~600 cases), 1–6 baies, 1–2 mers bordières par carte, plus les mers fermées ; plus de rubans le long des côtes ni de triangles contre le pôle. Moins de zones qu'avant : seules les parties remarquables sont nommées.
  - Détroits : case dont un axe a de la terre des deux côtés à ≤ 4 cases d'eau en tout, terres distinctes (≥ 4 cases chacune) ; au plus 30 ; parent : golfe ou mer bordière qui en contient la plupart, sinon l'océan.
  - Noms (`toponymy::sea_name`) : langue et caractère des côtes ; océans polaires « Océan Boréal / Austral », le plus grand « Grand Océan » ; ~40 % descriptifs (Mer des Glaces, Mer Ardente, Golfe Gelé, Baie des Phoques, Passe des Vents…) ; golfe / baie à l'embouchure d'un fleuve : « Golfe de la Vaska » une fois sur deux.
  - Affichage : capitales espacées (océans 22 px, mers 17 px), golfes 14 px, baies et détroits 12,5 px ; plages de zoom en pixels par case (océans ≤ 12, mers 4–22, golfes 9–55, baies et détroits 15–120) avec fondu ; nom sur la case explorée la plus éloignée du bord de l'étendue (au moins 4 cases explorées) ; caché si l'étendue est moins large que ~70 % du texte à l'écran ou s'il chevauche un nom plus important ; ombre portée. L'italique n'est pas rendu : la police Inter embarquée n'a pas de face italique (à ajouter : Inter Italic, même licence).
  - Cliquables (pourtour, caméra), recherche « Mers et océans » (avec les mers intérieures ; le filtre des lacs devient « Lacs »), infobulle « Côte — Golfe de Belino, Mer de Vliamaprok, Océan Kosk », options d'étiquettes « Mers et océans » et « Villages et camps ».
  - Bug corrigé au passage : sur une planète aux pôles de terre, l'océan ne touchait aucun bord et devenait une « mer intérieure » géante (lac) ; mer intérieure ≤ 800 cases.
  - Pistes : mers de plateau autonomes (grands plateaux larges, type mer du Nord / mer Jaune) — écartées pour l'instant car les plateaux autour des îles forment des anneaux ; à revoir si Benjamin veut plus de mers.

## 1. Zones à nommer (détection automatique, à la génération, module pur)

### Eau (fait, voir État)
Océans, mers (fermées, bordières), golfes, baies, détroits ; lacs et mers intérieures. Option future : fosses et dorsales (frontières de plaques sous la mer, vue lointaine).

### Terre
| Zone | Règle | Nombre | Intérêt |
|---|---|---|---|
| Continent | Composante de terre > ~2 % des terres ; découpée aux isthmes en sous-continents si très grande | 3–6 | repère global |
| Île / grande île / îlot | Composante plus petite ; îlot (1–3 cases) nommé seulement de près | 10–30 | colonisation |
| Archipel | ≥ 3 îles à ≤ 3 cases les unes des autres (regroupement) ; ses îles sont des sous-zones | 5–10 | |
| Péninsule | Partie de terre reliée par un col étroit du champ de distance à la mer, entourée d'eau sur l'essentiel de son pourtour | 5–15 | défense |
| Isthme | Passage de terre de 1–3 cases entre deux masses | 2–6 | **stratégique** |
| Chaîne de montagnes → massifs | Amas de cases montagne/pic (+ accidenté haut) ; nom posé le long de la crête ; chaîne longue découpée en massifs (~15–20 cases) | 6–12 (+15–30) | obstacle |
| Col | Passage le moins coûteux à travers une chaîne (plus court chemin) | 15–40 | **stratégique** |
| Plateau / hauts plateaux | Terres hautes (> ~1 500 m) peu pentues | 3–8 | |
| Collines | Amas de cases vallonnées | 5–15 | défense |
| Régions naturelles | Amas d'un même groupe de biome (lissé, petits trous tolérés) : forêt, jungle, taïga, désert, steppe/plaines, savane, marais, toundra ; seuils par type (désert ≥ 30, forêt ≥ 20, marais ≥ 6) ; > ~150 cases → sous-régions | 30–60 | « la forêt » |
| Villages tribaux, camps barbares | fait | ~40 | repères, menaces |

Leçons de l'étape 2 à appliquer : préférer des zones « remarquables » (abritées, resserrées, caractéristiques) plutôt que tout découper ; zones emboîtées (sous-zone d'une zone plus vaste) ; pas de frontières arbitraires en ligne droite ; raisonner à plusieurs échelles (ouverture morphologique : la même idée doit servir pour péninsules et isthmes, côté terre) ; jamais de forme en anneau ou en ruban.

### Hiérarchie (sous-zones)
- Océan › mer › golfe › baie ; détroit.
- Continent › péninsule ; île › archipel.
- Chaîne › massif › col.
- Région naturelle › sous-région (« Forêt Noire du Nord »).
Une case appartient à plusieurs familles (continent + forêt + bassin) : chacune a son niveau d'affichage.

## 2. Affichage

Recommandations de cartographie (Axis Maps, *Making Maps*, conventions d'Imhof) :
- Hiérarchie visuelle par importance : plus grand / plus gras / capitales / plus foncé = plus important ; écarts de taille ≥ 15 % sinon indiscernables ; minimum ~11 px à l'écran.
- **Eau en italique bleu** ; **zones étendues en capitales espacées**, l'espacement suivant l'étendue ; nom **courbé selon la forme** (chaîne, golfe allongé) plutôt que penché ; noms des cours d'eau le long du tracé, répétés sur les longs cours ; jamais à l'envers ; nom d'une très grande zone répété (fait pour les océans).
- Deux polices au plus : une sans-serif (Inter, villes et interface) + éventuellement une serif (noms de la nature, style atlas).
- Halo / ombre plutôt que pastille opaque pour les noms de zones (les pastilles restent pour villes et cours d'eau).
- Densité régulière sur l'écran ; chaque niveau a son quota.
- Jeux (CK3) : trois niveaux de zoom — vue lointaine « carte papier » avec grands noms, vue intermédiaire, vue proche avec noms locaux.

### Niveaux (zoom mesuré en pixels par case)
| Niveau | Zoom | Noms | Style |
|---|---|---|---|
| N0 | planète (case < ~6 px) | océans, continents | capitales très espacées, 22–28 px, translucides |
| N1 | régional (6–14 px) | mers, grandes régions naturelles, chaînes, grands archipels et îles, fleuves principaux, mers intérieures | capitales espacées 15–18 px |
| N2 | local (14–30 px) | golfes, détroits, péninsules, isthmes, sous-régions, massifs, plateaux, lacs, rivières, îles moyennes, villes | minuscules 13–15 px |
| N3 | proche (> 30 px) | baies, cols, îlots, collines, marais, affluents, villages et camps (dès 16 px) | minuscules 11–12 px |

Règles :
- Un nom n'apparaît que si sa zone tient à l'écran avec son texte ; fondu aux bords de sa plage de zoom (fait pour les étendues marines).
- Priorité puis collisions, quota par niveau.
- Taille fixée par le niveau, pas par la taille de la zone ; l'espacement des lettres s'adapte à l'étendue.
- Nom placé sur la partie explorée.
- Couleurs : eau bleu clair ; relief brun/sépia ; végétation vert sombre ; désert sable ; humain blanc.
- **Barre de lieu** (en haut) : la chaîne des zones sous le centre de la vue (« Ardanie › Forêt Noire › bassin de la Vaska ») ; l'infobulle donne la même chaîne pour la case survolée (fait pour la mer).
- Recherche : nouvelles catégories (terres et îles, relief, régions naturelles, passages : isthmes, cols).

## 3. Attribution des noms
- **Langues en partie inventées** (voir État) ; une langue par aire géographique ; plus tard, les civilisations reprendront ces aires.
- **Composition française** : générique + nom propre (« Mer de Vaskar », « Golfe d'Ilmar », « Monts Taldor ») pour ~60 % ; **descriptif** (« Forêt Noire », « Mer Gelée », « Désert Rouge », « Plaines Venteuses », « Monts Blancs », « Marais Fiévreux », « Détroit des Brumes ») pour ~40 %, **choisi d'après les vraies données** : couleur, climat, position, forme, faune.
- **Cohérence par racine partagée** : fleuve Vaska → bassin de la Vaska, golfe de la Vaska à son embouchure (fait) ; chaîne Taldor → col de Taldor ; archipel Kerrane → îles au nom de la même langue.
- **Sous-zones** : « X du Nord / du Sud », « X occidentale / orientale », « Haute-X / Basse-X ».
- **Grammaire** : genre (« la Vaska », « le Taldor » selon la terminaison), élision (« d'Ilmar », « de l'Ilmar » ; h aspiré : « du Hjardal »), accord des adjectifs.

## 4. Étapes
1. ~~Socle : langues, générateur, grammaire ; cours d'eau, lacs, bassins, plaques, villages, camps~~ (fait).
2. ~~Eau : océans, mers, golfes et baies, détroits ; niveaux de zoom, fondu, style des noms marins~~ (fait, revu deux fois ; refonte à l'échelle le 25/10).
3. Terre : continents, îles, archipels, péninsules, isthmes.
4. Relief : chaînes et massifs, cols, plateaux, collines.
5. Régions naturelles et sous-régions.
6. Barre de lieu, infobulle (chaîne complète des zones), nouvelles catégories de recherche.
