# Conception — Interface : accès aux lieux, mise en scène des actions, journal

## Objectif
Les étiquettes et légendes deviennent des portes d'accès aux zones de la carte, et un index de recherche permet de retrouver tout lieu connu.

## Lieux (`MapTarget`, module `search.rs`)
- Ville (entité), cours d'eau, lac / mer intérieure, bassin versant, plaque tectonique.
- Cases d'un lieu = sa partie **explorée** (`ShownFog`, donc tout en mode debug) ; une ville = son territoire.

## Interactions
- **Survol** d'un `TargetButton` → lieu mis en avant :
  - zone (bassin, plaque, territoire de ville) : pourtour doré qui pulse (gizmos 2D épais / 3D posés sur le relief), arêtes en retrait vers l'intérieur, mis en cache tant que le lieu et le brouillard ne changent pas ;
  - cours d'eau / lac (et fleuve ou lac qui nomme un bassin) : surbrillance existante du réseau via `PointedFeature`, prioritaire sur le survol de la carte et sur l'option de surbrillance.
- **Clic** → `FocusTarget` : vol de caméra (lissage exponentiel ~7/s, zoom logarithmique) qui cadre le lieu (≈ 60 % de la vue en 2D, échelle 0,6 à 8 ; distance 250 à 2 800 en 3D), centre pris sur la copie la plus proche (planète). Interrompu par toute action caméra de l'utilisateur. Le lieu reste en avant 3 s.
- Points d'accès : lignes de la légende des bassins (survol + clic), noms des cours d'eau et lacs sur la carte (clic ; leur survol garde l'option de surbrillance, comptée aussi en mode « survol de la carte »), bannières des villes (survol = territoire, clic = caméra + sélection), résultats de recherche.

## Recherche (Ctrl+F ou bouton « Rechercher »)
Inspirée des bonnes pratiques d'autocomplétion (Baymard, Smart Interface Design Patterns) et de la recherche de Factorio :
- panneau en haut au centre ; champ avec loupe, texte d'aide quand il est vide, bouton × pour effacer ;
- **puces de filtre** (Tout, Villes, Cours d'eau, Lacs et mers, Bassins versants, Plaques) avec le **nombre de résultats** de chaque catégorie pour la requête en cours (catégorie vide grisée) ;
- au plus 8 résultats, mis à jour à la frappe ; pastille de couleur de catégorie, nom avec le **passage trouvé en gras** doré, détail à droite (type, destination et débit d'un cours d'eau, étendue…) ; « … et N autres » invite à préciser ;
- correspondance sans accents ni majuscules (`fold` un caractère pour un caractère, tirets et apostrophes = espaces) ; classement : début du nom < début d'un mot < ailleurs / mots dans le désordre ; puis ordre des catégories, puis importance (population, débit, étendue) ;
- requête vide : suggestions « Lieux connus » ; aucun résultat : message + conseil ;
- clavier : ↑ ↓ (la ligne choisie est prévisualisée sur la carte), Entrée (y aller), Tab / Maj+Tab (filtre), Échap (fermer) ; survol souris = ligne choisie ; clic hors interface ferme.
- Seuls les lieux connus sont indexés (index refait à l'ouverture, à l'exploration, au mode debug, aux villes).

## Saisie de texte
- Widget Bevy 0.20 `TextInput` avec `EditableText` (curseur, sélection, Ctrl+A/C/X/V), feature Bevy `system_clipboard` pour le presse-papiers du système.
- État du pointeur : composant `UiPointer` (`pointer.rs`, depuis Bevy 0.20, où `Interaction` est déprécié). Picking limité aux éléments marqués (`UiPickingSettings::require_markers`, `UiPickingCamera` sur les caméras) : tout élément décoratif laisse passer le pointeur, sans `Pickable::IGNORE` à poser. Survol direct (`DirectlyHovered`) : un bouton masque son panneau. Un clic hors d'un champ lui retire le focus (`PointerFocusPlugin`) : un champ prend le focus au clic grâce à `TabIndex`, la recherche et la graine le reprennent tant qu'elles sont ouvertes.
- Pendant qu'un champ a le focus (`InputFocus`), `ButtonInput<KeyCode>` est vidé en PreUpdate (sauf Échap) : les raccourcis du jeu ne se déclenchent pas. Les touches propres aux champs se lisent avec `ButtonInput<Key>`.
- Graine du monde (menu Échap) : champ chiffres seulement (filtre), 10 caractères max, graine actuelle sélectionnée à l'ouverture ; boutons Copier / Coller ; Entrée regénère.

## Pistes
- Lieux supplémentaires : mers et océans nommés, chaînes de montagnes, cols, villages et camps découverts.
- Historique des recherches récentes, épingles (comme Factorio).
- Légende des plaques interactive comme celle des bassins.

# Mise en scène des actions et journal hiérarchisé

## Demande de Benjamin
Les unités se téléportaient : le joueur doit voir le déplacement, voir qu'une unité en attaque une
autre, lire sur la carte les PV perdus, la perte d'une unité et la récompense d'un village tribal.
Le journal en haut à droite doit mettre davantage en évidence l'arrivée d'une information, selon son
importance, sans être envahissant.

## Recommandations retenues (recherche UX)
- Chiffres flottants **sobres** : texte clair cerné d'une ombre portée, lisible sur tout terrain ;
  taille selon la force du coup ; mise en avant réservée aux événements forts (dispersion).
- Pas de texte qui recouvre l'action : le mot « Dispersé ! » monte plus haut que les dégâts.
- Animation **courte**, et qui raccourcit vraiment le tour : longs trajets accélérés, actions hostiles
  hors de vue non montrées (un réglage de vitesse qui ne change que l'image ne sert à rien).
- Notifications non modales, hiérarchisées : couleur, durée du flash et pulsations selon la gravité ;
  une entrée douce (glissement) plutôt qu'une apparition brutale ; ce qui est grave reste marqué.

## Fonctionnement (`feedback.rs`)
- Les règles restent **instantanées** (positions, PV, dispersion) ; elles posent au passage des
  indications (`Cues`) : marche, combat, récompense. Un système les répartit dans le temps dans
  la même image et pose sur chaque unité une `Motion` (segments datés : marche, élan, secousse,
  effacement), lue par la 2D (`Transform`) et la 3D (figurine posée sur le relief).
- Ordre : actions du joueur ensemble ; puis faction hostile, une action visible après l'autre
  (+0,1 s), sans attendre la fin des effacements.
- **Passage de tour** (demande de Benjamin : enchaîner les fins de tour accumulait les animations,
  jusqu'à une dizaine de tours de retard) : le joueur ne donne pas d'ordre avant que son tour ait
  réellement commencé. `end_turn` pose l'indication `Cue::TurnEnd` ; la mise en scène retient
  alors le joueur (`TurnReplay`) jusqu'à la fin de la dernière animation de l'image, effacements
  compris. Pendant ce temps : bouton de fin de tour grisé (« Le tour se joue… »), Entrée, clic
  droit, touches d'unité et choix de production sans effet (ignorés, pas mis en attente) ; caméra,
  sélection et Tab libres. Les déplacements ordonnés pendant le tour ne retiennent rien ; en
  Instantané, la fin des animations est immédiate.
- Unité dispersée : retirée du jeu (composants `Unit`, `HexPos`, `Hostile` ôtés) mais gardée pour
  s'effacer (`Retired`), puis supprimée.
- `HeldHp` : les barres de PV gardent la valeur d'avant le coup jusqu'à l'impact.
- Chiffres flottants : nœuds d'interface placés à la projection de leur case (2D et 3D), apparition
  avec léger grossissement, montée de 34 px, effacement sur 1,7 s. Impacts et traits : gizmos.
- Chiffres (vitesse normale) : 0,2 s par case (1,3 s au plus par trajet), élan 0,36 s (coup aux 40 %), trait 0,28 s,
  secousse 0,3 s, effacement 0,6 s. Les PV affichés sont ceux réellement perdus (pas plus que ce
  qui restait).
- Vérification : `CIVBUILDER_CAPTURE_FEEDBACK=1` avec `CIVBUILDER_CAPTURE=<dossier>`.

## GIF des actions (`film.rs`, page `docs/ANIMATIONS.md`)
Demande de Benjamin : des GIF des combats et des déplacements pour montrer comment se joue le jeu,
en 2D et en 3D, **avec l'interface**, générés à la demande et rassemblés dans une page du dépôt.
- Une scène par action (déplacement, trajet de plusieurs tours, échange, corps à corps, charge et
  victoire, tir, attaque perdue, attaque hostile en fin de tour, village : commerce, habitants,
  embuscade ; camp vide) sur une zone de terre plate loin du départ (une ville y est fondée pour
  l'issue « habitants »). Les ordres passent par les vraies commandes : une souris pilotée
  (`input::Pilot` : case survolée et clics imposés) que le jeu dessine (flèche) et une légende en
  haut (« Clic gauche : sélectionner », « Entrée : fin du tour »). Aperçu du chemin, prévision de
  combat, journal et bouton de fin de tour sont donc filmés tels qu'en jeu.
- Temps fixe : `TimeUpdateStrategy::ManualDuration` (1/20 s par image), chaque image enregistrée en
  PNG ; GIF réguliers quelle que soit la vitesse du rendu (et du logiciel de rendu sans carte
  graphique). ffmpeg assemble (palette par scène), images effacées ensuite.
- Issues fixées : PV de départ (milice hostile à 30 PV contre la cavalerie lourde, milice du joueur
  à 10 PV contre un guerrier au bouclier, éclaireur à 12 PV) ; village : `NextVillageEvent`.
- Limites : la position des milices de l'embuscade reste tirée au hasard ; pas de scène pour
  l'exploration automatique (un déplacement comme un autre) ni pour la barque.

## Journal
- `Importance` : routine (gris, sans flash), information (bleu, 0,9 s), succès (doré, 1,3 s),
  alerte (orange, 1,8 s, 2 pulsations, contour), critique (rouge, 2,6 s, 3 pulsations, contour,
  texte plus grand). Alertes et critiques gardent une teinte de fond ; les anciens messages
  s'estompent (sauf alertes). Six messages gardés.

### Renvois vers la carte
Demande de Benjamin : dans le journal, les noms de lieux et d'unités renvoient vers le lieu ou
l'unité, comme les lieux de la recherche ; une unité morte ou qui n'est plus affichée renvoie vers
sa dernière position connue. Le bleu d'abord essayé attirait trop l'attention : les renvois gardent
la couleur du texte avec un soulignement léger (40 %), doré et franc au survol.
- Un message est une suite de morceaux (`LogText` : texte simple ou `LogLink`, la cible `MapTarget`
  et sa case au moment du message). `LogText::from(String)` garde les messages simples.
- Affichage : une ligne = un `Text` vide et un `TextSpan` par morceau ; le picking de Bevy repère le
  passage sous la souris (`HoverMap`). Survol : le lieu est mis en avant (`PointedTarget::link`) ;
  clic : `FocusTarget` (même vol de caméra que la recherche), sélection de la ville ou de l'unité du
  joueur.
- Cibles nouvelles : `MapTarget::Unit` (pourtour de sa case) et `MapTarget::Spot` (une case seule).
- Dernière position connue (`LastSeen`, pour les unités nommées au journal) : la case du message,
  puis la case de l'unité tant que le joueur la voit (toujours pour les siennes, seulement à vue pour
  les hostiles, selon le brouillard affiché). Unité dispersée ou hostile hors de vue → `Spot` de
  cette case ; ville, village ou camp disparu → `Spot` de sa case.
- Messages concernés : production, croissance, famine, fondation (qui donne maintenant le nom de la
  ville), combats (les deux unités), camps et villages, retours de commande sur une unité ou une
  ville. Pas de renvoi vers les milices surgies d'un village hostile (le village y mène).

## Limites et suite
- Le brouillard se lève dès la fin du tour, avant l'arrivée de l'unité.
- La caméra ne va pas chercher une attaque hostile hors de l'écran (le journal la signale).
- Un passage de tour chargé (beaucoup d'actions hostiles visibles) fait attendre le joueur ; piste :
  une touche pour passer la mise en scène.
- Pistes : vitesse séparée pour les actions hostiles (ou « combat rapide »), son,
  journal qui n'affiche un message hostile qu'au moment où l'action se joue.

# Menu Échap en pages et paramètres

## Demande de Benjamin
Déplacements un peu plus lents, vitesse d'animation réglable, depuis un nouveau bouton du menu
donnant accès à la configuration du jeu. Le menu actuel n'est pas mal fait, mais à force d'y
ajouter des options il va se charger (la regénération du monde y prend déjà beaucoup de place).

## Recommandations retenues (recherche UX)
- Paramètres accessibles **en pleine partie** depuis le menu pause, sans quitter la partie.
- Menu pause court : reprendre, paramètres, (re)commencer, quitter ; le reste dans des
  **sous-pages** ou rubriques regroupées logiquement, les réglages courants d'abord.
- Réglage appliqué **aussitôt** et enregistré (pas de bouton « Appliquer » pour un choix aussi
  simple), avec une ligne qui dit ce que fait la valeur choisie.
- Dans les 4X, les joueurs veulent accélérer, voire supprimer, les animations **sans perdre le
  résultat** : en Instantané, les unités sont aussitôt à leur place mais les chiffres flottants
  (PV perdus, récompenses) restent affichés. Un « rapide » ×2 est le minimum attendu.
- Échap = retour : sur une sous-page il ramène à la page principale, sur celle-ci il ferme.

## Organisation
- Page principale : Reprendre (Échap), Paramètres, Nouveau monde…, Quitter (rouge, à part).
- **Paramètres** : rubrique « Animations sur la carte » (sélecteur à quatre cases, la case choisie
  en bleu, ligne d'aide), rubrique « Développement » (mode debug, déplacé ici). Retour (Échap).
- **Nouveau monde** : avertissement « La partie en cours sera perdue » en tête, graine
  (champ, Copier, Coller), Regénérer avec cette graine, Graine au hasard, Retour (Échap).
  La graine est sélectionnée à l'ouverture de cette page (et non plus du menu).

## Vitesse des animations (`settings.rs`, `feedback.rs`)
- Rythme multipliant toutes les durées de la mise en scène : Lente 1,5, Normale 1, Rapide 0,5,
  Instantanée 0. Les textes flottants gardent leur durée (temps de lecture) ; les cercles
  d'impact aussi, seul leur instant suit le rythme.
- Vitesse normale ralentie à la demande de Benjamin : 0,2 s par case (0,15 avant), 1,3 s au
  plus par trajet (1 s avant).
- Fichier `parametres.txt` (`clé = valeur`, `#` commentaire, valeur inconnue ignorée) dans
  `%APPDATA%\CivBuilder` ; il suit un nouveau monde (qui relance le jeu). Le mode capture ne le
  lit ni ne l'écrit, pour que les vues restent les mêmes chez tous.
- Vérification : le mode capture ajoute les trois pages du menu à ses vues.

## Suite
- D'autres paramètres trouveront leur rubrique dans cette page (son, interface, caméra) ; les
  options de carte (touche O) restent à part, car on les change en regardant la carte.

# Mode joueur pilotable (07/11/2026)

**Demande de Benjamin** : faire jouer des agents en mode normal, sans debug, pour trouver des problèmes. Un script ne convient
pas : il est lu une fois au lancement, en mode debug, donc l'agent ne peut pas réagir à ce qu'il voit.

**Décisions** : `CIVBUILDER_PILOTE=<fichier>` (`pilote.rs`, `scripts/pilote.sh`) suit le fichier pendant la partie, et le mode
debug reste coupé. Les ordres passent par les vraies commandes : souris pilotée sur la carte (`input::Pilot`), messages de
clavier et de souris injectés avant la lecture des entrées pour le reste. La vraie souris n'est pas déplacée : plusieurs parties
tournent en même temps. Les réponses ne donnent que ce que le joueur peut savoir (cases explorées, ennemis en vue, infobulle), et
les commandes qui modifient ou révèlent la carte sont refusées. `CIVBUILDER_FENETRE=1920x1080` règle la taille de la fenêtre.

**Limites** : un clic sur la carte désigne une case sans passer par le calcul écran → case. `interface` liste aussi ce qui se trouve
sous un panneau et devient long avec les options ouvertes. `etat` répète le journal récent.

**Première campagne** (graine 777 et 7 autres) : une partie de 63 tours a tenu sans plantage ; voir `CHANGELOG.md`.

# Console de debug et scripts de commandes (01/11/2026)

## Demande de Benjamin
Améliorer le mode debug pour qu'il serve aux agents IA autant qu'aux humains : changer les
caractéristiques d'une case pour obtenir vite des cas testables, sans surcharger l'interface
(terminal affichable, menu replié par défaut). Attention au rendu des cases voisines, aux cours
d'eau et au climat. Le terminal doit proposer une **auto-complétion** et une **aide intégrée** ;
le tout est documenté dans le wiki, **seulement en mode debug**.

## Décisions
- **Un seul langage de commandes**, en texte, pour la console du jeu, les scripts
  (`CIVBUILDER_SCRIPT`) et les tests. Un agent écrit un fichier de commandes, lance le jeu, lit
  les réponses et les captures, sans retoucher `capture.rs` pour chaque cas.
- **Console repliée** : touche ² (à gauche du 1 en AZERTY, `Backquote`), seulement en mode
  debug ; ² ou Échap referme. Rappel discret dans le bandeau du mode debug.
- **Modifications brutes par défaut** : seul le champ demandé change. Les niveaux « local »
  (biome, température d'altitude et neige recalculés sur la case et ses voisines) et
  « régénération » (climat et eau relancés) viendront ensuite (voir Suite).
- Cases en **coordonnées de la carte** (colonne, ligne), affichées en tête de l'infobulle en
  mode debug.

## Fonctionnement
- `debug_cmd.rs` (pur, testé) : `SPECS` décrit chaque commande (arguments, nature, effet,
  rubrique) ; l'aide (`aide`, `aide <commande>`), la complétion (Tab), la ligne d'aide sous la
  saisie et la rubrique du wiki en sont tirées, elles restent donc à jour. Les valeurs se tapent
  en un mot sans accent (`foret-temperee`, `vallonne`, `courants-marins`) ; un début de nom
  suffit s'il est le seul (`foret`), un nom entier l'emporte (`savane` et non `savane-arboree`).
- Réponses : première ligne `OK …` ou `ERREUR …`, puis lignes `clé=valeur`. `info` donne aussi
  le coût d'un pas vers chaque voisine (`x` : infranchissable) et les falaises.
- `Session` garde le pinceau, 20 copies de la carte pour `annuler` et le journal des commandes
  réussies (`journal <fichier>` l'écrit en script, graine et taille en tête).
- `console.rs` : panneau, touches (la saisie coupe les raccourcis du jeu, comme la recherche),
  lecteur de script (une commande par image, `attendre n`, capture qui attend la fin de la
  construction du relief 3D, `ReliefPending`, console cachée sur l'image), réponses écrites
  dans le terminal et dans `<script>.reponses.txt`.
- **Rendu des voisines** : une modification émet `MapEdited` avec les cases à moins de deux cases
  des cases modifiées. En 3D, leur relief est refait (bosses et arêtes entre sommets partagées),
  leur décor recréé (`spawn_decor`, limite des arbres recalculée), leurs formes d'eau refaites ;
  en 2D, les motifs de relief sont redessinés (`spawn_relief_marks`) ; couleurs, pentes et eau
  suivent le brouillard affiché, marqué changé deux images de suite.

## Vérification
Scripts sur la graine 777 : forêt de montagne voisine d'un pic trouvée, passée en jungle au
pinceau (décor refait), pic ajouté (aiguille), montagne à 2 500 m et lac posés dans une plaine
(falaise, eau, 2D et 3D), trois annulations qui rendent la carte d'origine, journal écrit.
60 images par seconde console ouverte.

## Étape 2 : modifications locales et vérification (01/11/2026)
- **`~` devant une modification** : recalcul de ce qui en découle directement, sur la case et
  ses voisines (`settle`) :
  - altitude → température (gradient `LAPSE` du climat, -6,5 °C par 1 000 m), puis biome ; et
    **relief recalculé comme à la génération** (décision de Benjamin, 01/11 : une montagne est
    d'abord une dénivelée entre cases, pas une altitude). La génération note chaque case de terre :
    45 % rang de la plus grande dénivelée avec une voisine, 30 % rang d'un bruit propre, 35 %
    altitude (rapportée à 4 800 m) ; les 5 % mieux notées sont montagnes (au moins 1 500 m, sinon
    accidentées), puis 8 % accidentées, 15 % collines ; un pic est un sommet local de montagne
    (aucune voisine plus haute de 200 m) d'au moins 2 400 m, les plus hauts d'abord jusqu'à 1 % des
    terres, jamais côte à côte. Comme le classement est relatif à toute la carte, la carte garde
    la règle (`ReliefRule` : dénivelées et bruits triés, rangs de chaque case à la génération,
    notes des dernières cases de chaque relief, altitude du plus bas pic) ; `natural_relief` note
    une case avec les altitudes actuelles (rangs de la génération repris tant que la dénivelée
    n'a pas changé : ex aequo départagés comme alors ; les lacs, venus après, comptent comme la
    terre qu'ils couvrent). Test : sur les graines 777 et 2, 0 case de relief différent de la
    génération. Après une modification locale, le relief des cases modifiées et de leurs voisines
    est recalculé (`reshape`), puis les pics (un pic qui n'est plus un sommet assez haut redevient
    montagne ; une montagne qui l'est devient pic, les plus hautes d'abord, jamais côte à côte) ;
  - température, pluie → biome (`map::biome`, avec les saisons : une température ou une pluie
    modifiée décale ou met à l'échelle celles des quatre saisons, même en brut) ;
  - relief montagne ou pic → altitude relevée au minimum (1 500 ou 2 400 m, température et biome
    suivent) ; un pic ramène ses voisins pics à des montagnes ; au pinceau, un seul pic au centre
    et des montagnes autour (sinon chaque case rétrograderait la précédente) ;
  - terrain : gardé tel quel (c'est une demande explicite) ; la réponse dit quel biome le climat
    donnerait.
  - Le biome n'est changé que s'il n'est plus cohérent : la génération fait varier l'aridité de
    ±25 % d'une case à l'autre (sols, exposition), un biome est donc cohérent s'il est celui d'une
    aridité entre 0,7 et 1,3 fois la vraie (`biome_fits`). Un marais demande un sol humide qui ne
    gèle pas, bas et plat.
- **`verifier [col lig [rayon]]`** : pics côte à côte, montagne ou pic trop bas, relief sur un
  fleuve, biome que le climat ne donne pas, rivière qui remonte, cuvette sans lac, lac à deux
  niveaux. Résumé par type, puis les 20 plus proches du centre de la vue.
  - **Rivière qui remonte** : comparée au **niveau rempli** (inondation depuis la mer, file de
    priorité : chaque case relevée au plus bas passage vers la mer). Dans une cuvette, l'eau monte
    jusqu'au seuil ; comparer les altitudes brutes signalait 3 à 4 faux cas par carte.
  - **Cuvette sans lac** : cases sous le niveau rempli, d'un seul tenant, traversées par plus de
    1 m³/s, plus profondes que 20 m, sans lac ni petit lac : l'eau y monterait (barrage posé sur un
    fleuve). En dessous de 20 m, la génération laisse un grand fleuve traverser une cuvette de
    désert (graine 777 : 8 m sous le seuil, 1 600 m³/s).
  - **Bilan** (`bilan_verifier`, 128 × 74) : 0 incohérence sur les graines 777, 2, 5 et 4242 ;
    test : 0 sur 96 × 56 (777), et un barrage de 1 500 m sur un fleuve est signalé.
- Vérifié en script (graine 777) : plaine relevée d'un bloc à 2 600 m au pinceau 2 → dessus
  plat (forêt), bords accidentés ou montagnes, quatre pics sur le rebord ; une case seule à
  3 500 m → pic, voisines vallonnées ou accidentées ; `verifier` : 0.
- Vérifié en script (graine 777, avant le recalcul du relief) : plaine relevée à 2 800 m au pinceau (température 20 → 6 °C,
  jungle et savane → forêt tempérée et prairie), pic au centre, montagnes autour ; à -8 °C,
  toundra et neige en 3D ; `verifier` signale le fleuve coupé par le massif.

## Étape 3 : refaire la simulation, préparer une partie (01/11/2026)
- **Climat et eau extraits de la génération** : les étapes 4 à 8 (passes du climat avec les
  courants marins, biomes, eaux peu profondes, cours d'eau et lacs, marais, noms, eaux
  poissonneuses) sont une méthode, `simulate_climate_and_water`, appelée par la génération et par
  `regenerer climat`. Elle repart de l'état d'avant le climat, gardé dans la carte
  (`ClimateBase` : classes d'altitude et relief des terres, variation locale de l'aridité de
  chaque case) ; les modifications de la console y sont reportées (altitude, relief imposé,
  terre devenue mer). Vérification : empreinte de deux cartes générées (terrain, relief, niveau,
  débit, pluie, surface, fleuves, noms, eaux poissonneuses) identique avant et après
  (`empreinte_generation`).
- `regenerer eau` : lacs et marais redeviennent la terre que donne leur climat, les reliefs
  aplanis par un fleuve reviennent, puis hydrologie, marais, noms (0,1 s). `regenerer climat` :
  tout (11 s sur 192 × 111 en release). Sur une carte intacte, les deux redonnent la carte à
  l'identique (test) ; un barrage de 1 500 m sur un fleuve, puis `regenerer eau` : 0 incohérence
  (lac ou détour). Après, étiquettes des cours d'eau et index de la recherche refaits
  (`NamesChanged`).
- Biome attendu : avec la variation locale de l'aridité gardée (`aridity_jitter`), la
  vérification est exacte (0 sur une carte générée) ; l'intervalle 0,7 à 1,3 ne sert plus qu'aux
  cartes sans génération.
- `fleuve c l c l … [debit=]` : cases voisines liées par leurs arêtes, aval, débit (500 m³/s par
  défaut), cases plates. Brut : ni bassin ni nom.
- Partie : `unite <type> c l [hostile]` (refusée sur une case infranchissable ou occupée),
  `ville c l` (règles de fondation), `supprimer c l`, `tour [n]` (fin de tour demandée, puis
  attente du tour suivant et de la fin de ses animations, n fois ; le script attend aussi),
  `chemin c l c l [unité]` (trajet le plus rapide avec les coûts de l'unité, sur la carte
  entière : `find_path_turns`, `path_turns`).
- Vérifié en script (graine 777) : fleuve tracé de 5 cases vers la mer, barrage puis
  `regenerer eau` (lac derrière, cours d'eau détournés, carte du réseau refaite), `regenerer
  climat` (0 case changée sur une carte intacte), cavalerie lourde, milice hostile, ville fondée,
  chemin de 13 pas (arrivée au tour 6), deux tours joués (le guerrier hostile a bougé).

## Commandes d'unité en famille (01/11/2026)
- Demande de Benjamin : pouvoir retirer des PV à une unité ; et faire commencer ces commandes
  par l'objet dont on parle (« unite soin »), l'auto-complétion les rend simples.
- Famille `unite` : `creer <type> c l [hostile]`, `supprimer c l`, `pv c l n` (0 à 100 ; 0 :
  dispersée, retirée de la carte, sans l'animation d'un combat), `pm c l n` (borné au maximum de
  l'unité), `dette c l n` (PM dus ; l'immobilisation suit au tour suivant), `soigner c l` (PV, PM,
  dette). Réponse : l'état obtenu (`lancier joueur en 89 45 pv=35/100 pm=0/2 dette=3`) ; `info`
  ajoute l'unité de la case.
- Langage : une commande peut avoir un nom en deux mots (`SPECS` : « unite pv ») ; `lookup` prend
  le nom le plus long, `family` donne les sous-commandes : Tab sur `unite ` les propose, « aide
  unite » les détaille, « unite 3 4 » répond quelles sous-commandes existent.
- Vérifié en script (graine 777) : lancier blessé (barre de PV) et endetté (pastille « 1 »),
  milice hostile dispersée à 0 PV, soin, suppression, tour joué.

## Limites
- `regenerer climat` gèle le jeu une dizaine de secondes (calcul sur l'image).
- `regenerer` remplace les biomes et le climat modifiés à la main (ils viennent de la
  simulation) ; un lac posé à la main disparaît si l'eau n'en fait pas un.
- Brut : une forêt reste une forêt à 30 °C, un lac posé n'a pas d'exutoire, les cours d'eau ne
  suivent pas l'altitude ; le coût des pas, lui, est recalculé.
- Local : le nombre de pics n'est pas plafonné (1 % des terres à la génération) : le rebord d'un
  plateau relevé au-dessus du plus bas pic de la carte peut en porter plusieurs. Décision de
  Benjamin (01/11) : on garde ainsi, pas de plafond en mode local.
- Local : l'évaporation potentielle ne suit pas la température (l'aridité ne change qu'avec la
  pluie) ; le climat des voisines ne change pas (pas de pluie orographique, pas d'ombre
  pluviométrique) ; les cours d'eau ne suivent jamais (étape 3 : `regenerer`).
- Les rochers éboulés au pied des falaises ne sont pas refaits (une falaise créée n'en a pas).
- Les noms (fleuves, lacs, mers) et les bassins versants ne changent pas.

## Suite
1. ~~Cohérence~~ : fait (étape 2, ci-dessus).
2. ~~Simulation~~ : fait (étape 3, ci-dessus ; `riviere tracer` est devenu `fleuve`).

# Charte graphique, nouveaux écrans et nouvelles cartes (10/11/2026)

## Demande de Benjamin
- Les interfaces doivent être **belles, intuitives, fluides** et suivre une **charte graphique
  commune**. Certaines sont aujourd'hui trop compliquées, en particulier **celle des villes**.
- De **nouveaux écrans bien travaillés** : économie, commerce, démographie, technologie,
  organisation politique, etc.
- De **nouvelles cartes** : institutions politiques, avancée technologique, commerce, carte des
  ressources reprise, carte géologique, etc.
- Un **travail d'équilibrage** à la fin (tâche dédiée).

## Proposition (à valider)
### Charte graphique commune
- **Un module de style unique** (couleurs, polices, tailles, marges, coins, ombres, icônes) que
  tous les écrans utilisent : plus de couleurs ni de tailles écrites en dur dans chaque écran.
- **Composants partagés** : panneau, onglets, bouton, jauge, barre découpée par source (comme la
  barre de recherche, `conception-recherche.md`), liste triable, carte d'information (survol),
  icône de ressource avec sa valeur et sa tendance.
- **Une couleur et une icône par grandeur**, les mêmes partout (Nourriture, Production, Économie,
  Recherche, Culture, Bien-être, Eau, chaque ressource) ; lisibles par les daltoniens (forme en
  plus de la couleur).
- **Fluidité** : ouvertures et changements d'onglet animés brièvement (vitesse réglée par les
  paramètres existants), aucun écran qui fige le jeu ; raccourcis clavier pour chaque écran.

### Principes d'ergonomie
- **Trois niveaux de lecture** : un résumé d'un coup d'œil (quelques chiffres et alertes), le
  détail sur demande (onglet ou survol), l'explication du calcul au survol (« d'où vient ce +3 »).
- **Le problème d'abord** : chaque écran commence par ce qui demande une décision (besoin non
  couvert, famine, ville privée par une ponction).
- **Agir depuis là où l'on lit** : le panneau qui montre un manque propose l'action (construire,
  acheter, régler l'intendant).
- **Relié à la carte** : cliquer une ville, une route ou une ressource dans un écran centre la
  carte dessus (comme les renvois du journal).

### Écran de la ville (refonte, d'abord)
Remplace l'actuel, jugé trop compliqué :
- en tête : nom, population (habitants et indice), Bien-être, et les **alertes** de la ville ;
- quatre onglets : **Vue d'ensemble** (production des grandeurs, file de construction),
  **Population** (répartition, intendant), **Marché** (besoins, stock, prix, flux),
  **Bâtiments et quartiers** ;
- l'intendant se règle par priorités simples ; les cases forcées sur la carte.
Avant de coder : une maquette (image) à valider par Benjamin.

#### Maquette interactive (11/11/2026, 18 versions avec Benjamin)
**Maquette retenue (11/11/2026)** : `docs/conception/maquettes/ecran-ville.html` (version 18), à ouvrir
dans un navigateur. Elle sert de référence pour coder l'écran de la ville ; les retouches viendront en
jouant. Chiffres d'exemple ; les systèmes absents du jeu (eau prélevée, commerce, bâtiments, recherche,
ordres de la société) y sont montrés tels qu'ils sont prévus.
Demandes de Benjamin : comprendre **d'où viennent** les grandeurs (cases, bâtiments, commerce),
mettre en évidence les **limites** (eau, nourriture) quand on s'en approche, savoir où part
l'Économie et comment on la gagne, des écrans nationaux, et **ne pas surcharger** la vue.
Résultat (dernière version) :
- **Disposition** : un panneau à gauche (synthèse et fiches), à droite une **carte de la ville**
  (zoom à la molette ou par boutons, déplacement en glissant ; à partir de ×2, la nature des cases)
  et, dessous, la **liste des bâtiments**. Pas de numéros de case pour le joueur : une case se
  nomme par sa nature et sa direction depuis le centre (« Champs du nord-est », « lointain » en
  troisième couronne). Pas d'effectifs sur la carte (ils sont dans l'onglet Population).
- **Onglets** : Rendements, Commerce, Population (à venir).
- **Rendements** : quatre cartes (Nourriture, Production, Économie, Eau) et une ligne de pastilles
  pour les grandeurs sans production directe (Recherche, Culture, Militaire, Bien-être ; Religion
  et Influence plus tard). Sans grandeur choisie : la **vue d'ensemble**, un tableau des grandeurs
  par origine (centre, cases, bâtiments, commerce, total, état coloré) et des mini-barres
  nourriture / production / Économie sur chaque case. Une grandeur choisie (recliquer la carte ou
  Échap : retour à l'ensemble) donne sa synthèse **« D'où vient… »** : sources et usages (recettes
  et dépenses pour l'Économie, net versé au trésor ou pris), avec une **jauge à seuils** pour la
  nourriture (famine sous 1 × le besoin, croissance pleine à 1,5 ×) et l'eau (alerte à 80 %,
  blocage à 100 %) ; l'eau détaille ses **prélèvements par source** en m³/s (fleuve, ruisseau,
  nappes, citerne : prélevé sur prélevable). Les bâtiments y sont **regroupés en une ligne** sans
  être nommés (leur détail est à droite) ; le commerce en une ligne qui mène à l'onglet Commerce.
  La carte colore les cases selon la grandeur et trace les **flux commerciaux** chiffrés vers les
  villes partenaires (vert : entre, rouge : sort).
- **Bâtiments** (liste de droite) : **filtrée par la grandeur choisie**, triée par apport ; « tout
  voir » pour les autres. Survol ou clic d'un bâtiment : sa case (or) et les cases qu'il améliore
  (violet), avec le gain de la grandeur choisie seulement. Survol ou clic d'une case : ses
  bâtiments et ceux qui l'améliorent. Le survol est un aperçu ; le clic garde la sélection, que
  retirent un second clic, un clic dans le vide, « × » ou Échap.
- **Commerce** : carte régionale des **axes** (épaisseur selon le volume, style selon la voie :
  route, fleuve, col ; solde et durée sur chaque axe ; partenaire étranger marqué), tableau des
  axes (voie, durée, marchandises envoyées et reçues, solde, risque), solde en Économie, transport
  utilisé (chevaux et mulets du pays), dépendance (« 33 % du blé vient de Lyon ») ; fiche d'un axe
  (capacité, marchandises, Économie de l'axe, risque).
- **Écrans nationaux** : tableau des villes (habitants, couverture de la nourriture, eau,
  production, Économie, alertes), recettes et dépenses du trésor par source, tableau des bâtiments
  du pays ; une carte du commerce du pays.
- **Construire un bâtiment** (maquette v1) : catalogue filtré par grandeur (bâtiments verrouillés
  visibles avec ce qui manque, agrandissements comme des bâtiments, aménagements dans le même
  catalogue) ; la carte colore les **cases possibles selon le gain** et marque le conseil de
  l'intendant ; survol d'une case : son gain et les cases améliorées ; fiche : coût, durée (seule
  et après la file), emplois, entretien, gain par grandeur, **origine des travailleurs** (sans
  sans-emploi, l'intendant prend les moins utiles, avec la perte), bilan pour la ville, gain à
  pleine capacité (paliers) ; « Mettre en file » (file unique, tours restants par élément).
- **Reporté (Benjamin, 11/11/2026)** : utiliser **la vraie carte du jeu** (en 3D) au lieu de la
  carte schématique de l'écran, ou proposer les deux. Repoussé parce que la carte elle-même va
  beaucoup changer (sous-cases, quartiers, aménagements visibles).
- **Revue par trois agents (designer d'interface, game designer de 4X, joueur qui découvre) puis
  trois autres (interaction, jeux de gestion, visualisation de données), décisions de Benjamin
  (11/11/2026)** — maquette v19 :
  - **Monnaie : le denier**, par défaut ; en option de partie, un nom tiré de la langue de chaque
    civilisation. Pas de taux de change ni de valeurs différentes selon les pays. « Économie »
    reste le nom du domaine (marché, commerce, trésor), plus celui de la monnaie.
  - **Bandeau d'état** en tête : croissance et tendance (« ▲ +37/tour · palier 3 dans 15 tours »),
    deux alertes au plus (« +n »), cliquables vers leur cause, « Aucun problème » sinon, élément en
    construction.
  - **Panneau de gauche en pile** avec fil d'Ariane (« Ensemble › Production › Forêt du nord-ouest
    × ») ; Échap remonte d'un cran. **La vue d'ensemble est la racine de la pile** (Benjamin : elle
    évite de tout parcourir) : une ligne par grandeur avec valeur, tendance sur 10 tours, état et
    principales origines. Gauche = explication, droite = sélection (carte et listes) : la fiche d'un
    bâtiment s'ouvre à gauche.
  - **Survol = infobulle près du curseur ; clic = fiche** (le panneau ne saute plus au survol).
  - **Intendant** : réglé dans la synthèse de la grandeur, pas sur les cartes (un badge sur la carte
    seulement hors du réglage normal ; l'onglet Population récapitule). **Nourriture : un objectif
    plutôt qu'une priorité** (Benjamin : ressource limitante) : nourrir seulement (1 × le besoin),
    croître (1,25 ×), croissance maximale (1,5 ×), excédent à vendre (2 ×) ; l'intendant vise
    d'abord cet objectif puis répartit le reste. **Autres grandeurs : priorité normale, prioritaire,
    maximale ; plusieurs au même niveau = même poids.** Effet de chaque cran au survol. Eau : pas de
    réglage, la synthèse dit pourquoi et propose de construire.
  - **Tendances** : mini-courbe sur chaque carte (« 10 derniers tours · prévision », trait plein
    puis pointillé, repère aujourd'hui) ; **grand graphique en tête de la synthèse** : passé et
    prévision au rythme actuel sur un même axe, verticale « aujourd'hui », bande d'incertitude,
    seuils nommés (besoin, croissance pleine, alerte et limite d'eau, équilibre), croisement prévu
    annoté (« limite au tour +18 »), événements numérotés, période 20 / 40 tours / toute la partie ;
    axe vertical non étiré (depuis zéro). Plusieurs grandeurs : graphiques empilés, pas superposés.
  - **Eau** : discrète quand elle abonde (pastille « abondante »), carte et jauge dès 60 % de la
    limite, selon la disponibilité de la ville.
  - **Construction en mode placement** : ① bâtiment (catalogue, conseils motivés en tête, tri par
    gain pour 10 de production) › ② case (carte des gains, ★ conseil présélectionné, cases
    impossibles grisées avec leur raison au survol, survol : fantôme et cases améliorées) › ③
    « Mettre en file · <case> » (pas de doublon) ; le bâtiment apparaît en pointillé sur sa case et
    reste déplaçable tant que le chantier n'a pas commencé. Bandeau « Où construire… ? Échap
    annule ». Effet sur l'état de la ville (« eau 83 % → 79 % ») plutôt qu'un chiffre brut.
  - **Couleurs** : couleurs d'état (vert-bleu ✓, orange !, rouge ✕, toujours avec une icône)
    réservées aux états ; couleurs de grandeur seulement pour l'identité ; une seule couleur de
    sélection (blanc). **Icônes de bâtiments** au lieu des codes à lettres ; pas d'abréviations ;
    une unité fixe par grandeur ; texte d'au moins 11 px ; carte d'au moins 40 % de l'écran.
  - **Clavier** : 1 à 4 pour les grandeurs, B pour construire, Échap pour remonter.
  - **Maquette plein écran (grand écran, 1920 × 1080)**, après les retours de Benjamin : onglets
    Rendements, Population, Commerce sur toute la largeur, au-dessus de la zone qu'ils commandent
    (ils ne dépendent pas de la grandeur choisie) ; vue d'ensemble complète des huit grandeurs à
    l'ouverture ; grandeurs secondaires cliquables comme les autres ; carte de la ville sur environ
    la moitié de l'écran. **Construction** : chaque élément du catalogue dit en une ligne ce qu'il
    fait ; une fenêtre « Tout comparer » liste bâtiments et aménagements possibles avec leur effet à
    la meilleure case de la ville, leur coût et un bouton Construire ; bâtiments, aménagements de
    production et aménagements de bonus forment trois groupes distincts. **Case choisie** : la
    colonne de droite montre son activité, ses bonus, ses bâtiments, les bâtiments voisins qui
    l'améliorent et ce qu'on peut y construire. **Chantiers en file** : en pointillé blanc sur leur
    case ; au survol, avancement, délai et effet ; au clic, leur fiche (avancer, changer de case,
    retirer).
  - **Compléments (11/11/2026)** : légende graphique sous la carte, qui porte aussi les filtres des
    couches (activités, bonus, bâtiments, chantiers : clic pour masquer) ; zoom à la molette et aux
    boutons, déplacement en glissant ; carte de l'eau (fleuve tracé, prélevé / prélevable par source)
    et cartes des grandeurs secondaires (apport de chaque case ; bien-être par quartier et par
    campagne). **Onglet Population** : taux d'urbanisation, actifs, sans-emploi, mouvement du dernier
    tour (naissances, décès, migrations), évolution de la population avec les limites de la ville
    (nourriture, eau, emplois) sur une échelle logarithmique, cultures et religions (part, relation
    avec la ville, évolution), ordres de la société (à venir, tâche 32) ; deux cartes au choix :
    emplois, répartition des habitants. **Onglet Histoire** : frise des moments marquants (population,
    constructions, catastrophes, commerce, guerres, batailles, pillages des barbares, culture), avec
    filtres et lieux sur la carte.
  - **Troisième revue par agents (11/11/2026), décisions de Benjamin** :
    - **Programmes de ville** pour les aménagements de bonus (égouts, fontaines, rues pavées…) : un
      seul ordre « dans tous les quartiers éligibles », que l'intendant échelonne ; l'ordre quartier
      par quartier reste possible.
    - **Tout afficher, même dans un bourg** : un onglet ou une partie sans contenu le dit (« aucun axe
      de commerce pour l'instant ») plutôt que de disparaître.
    - **Ordres de la société** : gardés dans la maquette, pas codés avant la tâche 32.
    - Retenus aussi : un bloc « Ce qui freine la croissance » dans l'en-tête (diagnostic unique et
      classé, pas de palier promis au-delà d'une limite) ; des remèdes chiffrés dans les alertes,
      qui ouvrent le placement déjà rempli ; le goulot actuel affiché dans le catalogue ; les
      commandes de la carte au-dessus d'elle dans tous les onglets, avec « Carte simple » ;
      l'infobulle selon le mode de la carte ; le survol d'une ligne de liste qui éclaire sa case ;
      la confirmation du placement sur la carte (double-clic, Entrée) ; une couleur de relation hors
      de la palette des grandeurs ; des indicateurs inertes d'un style distinct ; l'intendant
      expliqué (emplois non pourvus, prochain bloc) ; l'Histoire triée et filtrée par lieu ; un seul
      mot par notion (« aménagement »).
    - **Appliqué dans la maquette (version 17)** : bloc « Ce qui freine la croissance » sous le nom
      de la ville (Paris : l'eau, limite 2 914 habitants dans ~19 tours, palier 3 hors de portée ;
      Lugdun : la population baisse) avec ses remèdes, qui ouvrent le placement au conseil de
      l'intendant ; même diagnostic en tête du catalogue, remèdes parmi les conseils ; programme de
      ville dans le placement des égouts, fontaines, rues pavées et jardins (un ordre, coût total,
      gain une fois fini) ; double-clic ou Entrée pour mettre en file ; légende et couches au-dessus
      de la carte, bouton « Carte simple » ; infobulle selon la carte affichée ; survol d'une ligne
      de liste qui éclaire sa case (bâtiment : sa case ; catalogue : la case conseillée) ; chantiers
      en pointillé orangé (le blanc reste la sélection) ; « Pourquoi » des emplois non pourvus et
      « Forcer l'effectif… » avec aperçu puis confirmation (bâtiment et case) ; Histoire triée par
      tour et filtrée par lieu (clic sur une case cerclée), repères des graphiques liés à l'Histoire ;
      période du graphique 10 / 20 tours / toute la partie ; « Ouvrir un axe… » qui propose des
      partenaires ; listes vides qui le disent ; indicateurs inertes sans fond.
  - **Plus tard** : une vue nationale détaillée pour gérer les villes et leurs problèmes (« ville
    suivante en alerte », file proposée par l'intendant) ; une vraie simulation pour les prévisions.
  - Implémentation dans Bevy : un seul état de sélection (pile), des nœuds d'interface persistants
    mis à jour par détection de changement, une solution pour dessiner la carte hexagonale de l'écran.
- **Questions ouvertes** : Règles derrière les chiffres d'exemple (eau par habitant et part prélevable d'un cours
  d'eau, recherche selon l'éducation, défense, calcul du Bien-être, liste et effets des bâtiments).

#### Développement de l'écran de la ville (découpage validé le 11/11/2026)
Sept lots, chacun livré seul : **1** fondations (écran plein écran, charte, en-tête, onglets, carte
schématique, deniers) ; **2** rendements et leur origine (graphique passé et prévision : 40 derniers
tours mémorisés par ville) ; **3** construction = livraison 4b (file unique, catalogue, placement,
conseil, « Tout comparer ») ; **4** intendant à l'écran (objectif de nourriture, priorités, forcer
avec aperçu) ; **5** onglet Population (limites, cartes emplois et habitants) ; **6** Histoire
(journal par ville sauvegardé) ; **7** « Ce qui freine la croissance ». Après, avec leur système :
eau prélevée, bâtiments, aménagements de bonus et programmes de ville (4c), culture (4d), commerce.
Choix retenus avec le découpage : l'écran remplace le panneau de la ville (un résumé reste sur la
carte) ; l'objectif de nourriture remplace la priorité de la nourriture (lot 4) ; la carte de la ville
est dessinée avec l'interface Bevy, un hexagone par case.

**Lot 1 fait le 11/11/2026** (`charte.rs`, `ecran_ville.rs`) :
- ouverture par **C** ou le bouton « Écran de la ville » du panneau de production ; Échap ferme
  d'abord la fiche de case, puis l'écran, sans ouvrir le menu ; les commandes de la carte du monde
  sont suspendues tant qu'il est ouvert ;
- **charte** : couleurs de la maquette, une couleur et une icône par grandeur ; les icônes (épi,
  marteau, pièce, goutte, fiole, plume, bouclier, visage, deux personnes) et les hexagones sont
  dessinés par le code au démarrage (aucun fichier d'image : la police ne contient pas d'icônes) ;
- **carte de la ville** : un hexagone (image teintée) par case du territoire, placé autour du centre
  en tenant compte du tour du monde ; la case sous le pointeur est calculée en coordonnées axiales
  (les rectangles des hexagones se chevauchent) à partir du pointeur du picking, ce qui marche aussi
  avec le pilote ; couleur de la grandeur principale (pâle si personne n'y travaille : ce que la case
  donnerait), remplissage dans l'onglet Population ; rond vert de l'aménagement ; zoom (1 à 4, molette
  autour du pointeur), déplacement en glissant, recentrage ;
- contenu de l'onglet Rendements : trois cartes (nourriture comparée au besoin, production et
  construction en cours, deniers), les grandeurs pas encore dans le jeu (eau, recherche, culture,
  défense, bien-être, marquées « — »), tableau « centre, cases, bâtiments, commerce, total » ;
  Population : habitants, palier suivant, actifs, au travail, sans emploi ; Commerce, Histoire,
  cultures, ordres de la société : un panneau qui dit qu'ils ne sont pas encore dans le jeu ;
- le panneau de production reste sur la carte jusqu'au lot 3 (file de construction).
**Lot 2 fait le 11/11/2026** (`graphique.rs`, `CityState.historique`) :
- chaque ville garde ses 40 derniers tours (nourriture, production, deniers, habitants), sauvegardés
  (champ facultatif : les sauvegardes du format 3 se chargent) ;
- détail d'une grandeur (clic sur sa carte ou sa ligne, touches 1 à 3) : graphique dessiné par le code
  dans une image (le passé, aujourd'hui = le rendement actuel, la prévision sur 20 tours au rythme de
  la tendance des 10 derniers tours, bande d'incertitude qui s'élargit, seuils en tirets), libellés
  posés par l'interface ; sources (centre, cases ; bâtiments et commerce « pas encore dans le jeu ») et
  usages (nourriture : besoin des habitants, réserve pour croître jusqu'à +50 %, excédent perdu ou
  manque ; production : construction en cours ou perdue ; deniers : trésor) ;
- fil d'Ariane « Vue d'ensemble › Nourriture › case » ; Échap remonte d'un cran ;
- carte de la ville aux couleurs de la grandeur (de faible à fort, valeur par case) ; liste des cases
  triée par la grandeur ; fiche de case : gain de 100 travailleurs de plus (ou « case pleine ») ;
- pas encore : réglages de l'intendant dans le détail (lot 4), repères d'événements sur les
  graphiques (lot 6, avec l'histoire).
**Lot 3 fait le 11/11/2026** (règles dans `rules.rs`, écran dans `ecran_ville.rs`) : file de
construction et projets de ville (voir `conception-villes.md`, 4b) ; bouton « Construire (B) » : le
catalogue remplace la liste de droite (aménagements avec leur case conseillée et leur gain, projets,
unités ; les ponts restent choisis sur la carte du monde) ; placement : carte des gains à pleine
capacité, cases impossibles en sombre, anneau doré du conseil, case choisie en blanc ; fiche du
placement en trois étapes ; mise en file par bouton, double-clic ou Entrée ; file dans la colonne de
gauche (avancer, retirer, projet). Écart avec la maquette : pas de fenêtre « Tout comparer » (avec
trois aménagements, le catalogue donne déjà la meilleure case et le gain de chacun) ; elle viendra
avec les bâtiments. Le panneau de production de la carte du monde met lui aussi en file.
**Lot 4 fait le 11/11/2026** (`intendant::ObjectifNourriture`) : l'objectif de nourriture remplace
la priorité de la nourriture dans le modèle (poids de la nourriture : 4 sous le besoin, 2 jusqu'à
l'objectif, 0,3 au-delà ; défaut : croissance maximale = comportement d'avant) ; les réglages
s'affichent sous le détail de la grandeur, une ligne par choix avec son effet calculé en refaisant la
répartition (aperçu exact, pas d'estimation) et la croissance qui en résulte ; le choix actuel est
encadré de blanc. Forcer l'effectif : ±100 depuis l'effectif actuel, aperçu de l'effet sur toute la
ville, confirmer ou annuler. Pas encore : le « pourquoi » des emplois non pourvus (il viendra avec
les bâtiments).
**Lot 5 fait le 11/11/2026** : onglet Population ; `graphique.rs` gagne l'échelle logarithmique
(bornée autour des valeurs utiles, graduations 1-2-5) et la prévision donnée tour par tour ; la
prévision de la population simule 40 tours avec les rendements actuels (la nourriture borne la
croissance ; les emplois ne la bornent pas, ils font des sans-emploi) ; limites : nourriture × 1 000,
capacité des cases ÷ part active ; carte des habitants : violet de la maquette (`RAMPE`), échelle
logarithmique entre la case la moins et la plus peuplée. Pas encore : naissances, décès et migrations
distingués (le jeu ne connaît que le solde), cultures et ordres de la société (affichés « pas encore
dans le jeu »).
Limite connue : en mode pilote, l'infobulle de la carte ne reste affichée que quelques images après
l'ordre (cause non élucidée) ; elle n'apparaît donc pas sur les captures.

### Écrans nationaux (un panneau par thème, même gabarit)
| Écran | Contenu |
|---|---|
| Économie | trésor, recettes et dépenses par source, impôt, entretien |
| Commerce | flux entre villes et avec l'étranger, prix, douanes, budget de transport, convois attaqués |
| Démographie | habitants par ville, croissance, migrations, famines, hygiène et épidémies |
| Technologie | arbre en colonnes par ère, barre découpée par source (`conception-recherche.md`) |
| Organisation politique | institutions, capacité de gouverner, stabilité (tâche 32) |
| Armée | effectifs, équipement, réquisitions, montures (`conception-combat.md`) |
| Diplomatie | peuples connus, attitude et raisons (`conception-diplomatie.md`) |

### Nouvelles cartes
Il y a déjà 13 cartes (`MapLayer`). Pour ne pas s'y perdre, deux familles dans le sélecteur :
- **Nature** : altitude, relief, climat, bassins, débits, réseau, plaques, courants, marées,
  houle, géographie, **géologie** (nouvelle : roches, bassins sédimentaires, volcans, tâches 11 et
  14), **ressources** (reprise : tous les gisements et zones, pas seulement les bancs de poissons).
- **Peuples** : **institutions politiques**, **avancée technologique** (ère et nombre de
  technologies par peuple connu), **commerce** (flux, épaisseur selon le volume, convois
  attaqués), territoires et culture, démographie.
Chaque carte a sa légende au même format, et respecte le brouillard (rien sur un peuple inconnu).

### Ordre proposé
1. Charte et composants partagés.
2. Refonte de l'écran de la ville (maquette d'abord).
3. Les écrans et cartes au fur et à mesure des systèmes (ressources, commerce, recherche,
   institutions), chacun livré avec son écran.

# Nouvelle partie, graines et prise en main (10/11/2026)

## Décisions de Benjamin
- **Écran de nouvelle partie** : monde (taille, graine, réglages de génération), peuples (nombre
  d'IA, difficulté), règles (mode 2 tours par mois, hégémonie, durée, barbares), aperçu du monde.
- **Taille de la carte** : à voir avec la génération (comment elle s'adapte à d'autres tailles).
- **Graines** : tirage au hasard, **copier-coller**, accès aux **graines déjà utilisées**,
  **favoris** et **notes** sur les graines.
- **Réglages de génération** (part des terres, climat plus chaud ou plus froid, relief plus ou
  moins marqué) : oui, **associés à la sauvegarde des graines**, pour que le joueur voie les
  réglages qui ont produit une carte.
- **Variables d'environnement** : l'idéal serait de ne plus en dépendre ; choix laissé à Claude.
- **Prise en main** : premiers tours guidés, conseillers, notifications hiérarchisées, systèmes
  dévoilés progressivement, wiki ouvert sur le sujet courant : validé.
- **Performance** : moins de 2 secondes par tour en fin de partie : validé.

## Proposition
### Code de partie
- Graine et réglages tiennent dans un **code de partie** court, lisible et copiable, par exemple
  `777-M-T35-C+1-R2` (graine, taille, part des terres, climat, relief) ; le coller dans l'écran
  de nouvelle partie remplit tous les champs. Deux joueurs qui partagent un code ont le même monde.
- Le code est rappelé dans le menu Échap et dans le nom des captures.

### Bibliothèque des graines
- Chaque monde généré s'enregistre automatiquement : code, date, réglages, **miniature de la
  carte**, nombre de parties jouées.
- Le joueur peut le marquer en **favori**, lui écrire des **notes** (« beau continent, départ
  difficile »), le renommer, l'effacer.
- Fichier dans le dossier des données du joueur (hors du dépôt), au même format que les réglages.

### Taille de la carte
- Quelques tailles proposées (petite, moyenne, grande), la moyenne étant l'actuelle 192 × 111 ;
  le nombre d'IA conseillé suit la taille. Les tailles exactes dépendent de ce que la génération
  supporte (bilans `bilan_tectonique`, `bilan_terres` à refaire sur chaque taille) : à mesurer.

### Variables d'environnement
Proposition de Claude : elles deviennent une simple porte d'entrée vers les mêmes réglages.
- Le jeu lit d'abord les **arguments de la ligne de commande** (`--code 777-M-T35`, `--capture
  <dossier>`, `--script <fichier>`…), puis, à défaut, les variables d'environnement actuelles, qui
  restent pour ne pas casser les scripts (captures, pilote, GIF).
- Le joueur, lui, n'en a jamais besoin : tout passe par l'écran de nouvelle partie.

### Prise en main (validée)
- **Premiers tours guidés** pendant la période nomade (« trouve une terre fertile près d'un fleuve »,
  « découvre l'agriculture »), qu'on peut désactiver.
- **Conseillers** par domaine (économie, armée, diplomatie, société) : seulement le problème le
  plus urgent de chacun.
- **Notifications hiérarchisées** (décision, important, information), réglables, appuyées sur le
  journal existant.
- **Systèmes dévoilés progressivement** : diplomatie, espionnage, religions, commerce lointain
  apparaissent quand ils deviennent utiles (premier contact, Écriture…).
- **Wiki (F1)** ouvert sur le sujet survolé ou l'écran courant.

### Écran de nouvelle partie : décisions après la maquette (11/11/2026)
Maquette v2 analysée par deux agents (designer UX, game designer de 4X) ; décisions de Benjamin :
- **Aperçu du monde masqué par défaut** : il gâcherait l'exploration (départ nomade, carte
  imparfaite) ; un bouton le révèle (« gâche la découverte »). Le monde calculé pour l'aperçu sert
  au lancement ; génération en tâche de fond, avec progression et annulation ; aperçu marqué périmé
  quand un réglage change.
- **Réglages pas encore disponibles affichés grisés, avec « à venir »** (terres, climat, relief,
  IA, difficulté) : l'écran montre la cible, pour ne pas changer d'interface à chaque étape.
- **Taille Grande (256 × 148) signalée « expérimentale »** tant que la génération n'y est pas vérifiée.
- **Rythme : 500 tours dans les deux modes** (`conception-saisons.md`, révision 3).
- **Pas de préréglages** (ils ne faisaient varier que la taille). À la place, **des
  configurations** : rétablir la configuration par défaut ; enregistrer la configuration en
  cours avec un **nom** et un **commentaire** ; retrouver les configurations enregistrées et celles
  des **parties déjà jouées**, avec des **favorites**. Au chargement d'une configuration, on
  demande s'il faut **garder la même graine** (le même monde) ou en **tirer une nouvelle**.
  « Mes configurations » remplace « Mes mondes ».
- **Enregistrer la configuration aussi pendant la partie** (Benjamin, 11/11/2026) : c'est en jouant
  qu'on voit si le monde est réussi. Menu Échap → « Enregistrer la configuration de ce monde… »,
  la même fenêtre (nom, commentaire, favorite), avec la graine et les réglages de la partie en
  cours ; la configuration rejoint « Mes configurations ».
Corrections retenues : valeur chiffrée sous chaque choix (« Comme la Terre · 29 % ») et code au
format de la doc ; ligne « Victoire : meilleur score d'héritage au tour 500 » ; pas de réglage
« continuer après 500 » (partie libre déjà décidée) ; bascule « Premiers tours guidés » ; une ligne
sur le peuple (« il naîtra de son lieu de départ ») ; titres de section plus visibles, aide plus
contrastée (≥ 4,5:1) ; bascules avec Oui/Non ; navigation au clavier et à la manette ; erreur
claire pour un code collé illisible ; actions sur les mondes (favori, note, effacer avec
confirmation) et lien vers leurs sauvegardes.

### Performance (validée)
- Objectif : **moins de 2 secondes par tour en fin de partie** (12 peuples, centaines de villes),
  idéalement moins d'une seconde.
- `bilan_ia` mesure le temps par système et par tour (tours 100, 250, 500) et alerte si un système
  dépasse sa part.
- Méthodes : calculer rarement ce qui change lentement (trajets entre villes, valeurs, cultures),
  étaler les décisions des IA, tableaux de bits pour le brouillard, calcul parallèle sur une vue
  figée puis application dans un ordre fixe (déterminisme).
- Taille des sauvegardes à surveiller.
