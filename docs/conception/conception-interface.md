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
- Widget Bevy 0.19 `EditableText` (curseur, sélection, Ctrl+A/C/X/V), feature Bevy `system_clipboard` pour le presse-papiers du système.
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

## Limites
- Brut : une forêt reste une forêt à 30 °C, un lac posé n'a pas d'exutoire, les cours d'eau ne
  suivent pas l'altitude ; le coût des pas, lui, est recalculé.
- Local : le nombre de pics n'est pas plafonné (1 % des terres à la génération) : le rebord d'un
  plateau relevé au-dessus du plus bas pic de la carte peut en porter plusieurs.
- Local : l'évaporation potentielle ne suit pas la température (l'aridité ne change qu'avec la
  pluie) ; le climat des voisines ne change pas (pas de pluie orographique, pas d'ombre
  pluviométrique) ; les cours d'eau ne suivent jamais (étape 3 : `regenerer`).
- Les rochers éboulés au pied des falaises ne sont pas refaits (une falaise créée n'en a pas).
- Les noms (fleuves, lacs, mers) et les bassins versants ne changent pas.

## Suite
1. ~~Cohérence~~ : fait (étape 2, ci-dessus).
2. **Simulation** : `regenerer eau|climat` (étapes 4 à 8 de la génération extraites en
   fonction, mêmes résultats qu'à la génération), `riviere tracer`, unités, villes, fin de tour,
   `chemin a b` (trajet et coût).
