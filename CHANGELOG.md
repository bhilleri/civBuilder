# Journal des versions

## Prochaine version (en cours)

### Échelle de la partie (étape 3 de la feuille de route)
- **Population en habitants de toute l'aire** (ville et campagnes) : une ville naît avec 1 000 habitants ; le **palier** (1 à 30) résume sa taille, de 1 000 à 2 500 000 habitants (repères : 10 000 au palier 5, 50 000 au 10, 150 000 au 15, 400 000 au 20, 1 000 000 au 25), et règle le territoire, le stade de la ville et les cases exploitées. **1 point de nourriture nourrit 1 000 habitants** ; croissance naturelle de 1,5 % par tour au plus (atteinte avec 50 % de nourriture en plus du besoin), famine de 2 à 5 % par tour. Plus de stock de nourriture ni de seuil.
- **Colon** : il emporte 1 000 habitants, d'une ville qui en compte au moins 2 000. Un village qui rejoint une ville lui apporte 1 000 habitants.
- **Points de mouvement doublés** (milice 4, cavalerie légère 8), **coûts des terrains difficiles doublés** (forêt 4, marais 6, collines +2, relief accidenté +4, montagne 8 à 16, rivière +4, fleuve 6, banquise 4, montée +2 ou +4) ; plaine et pont inchangés (1 PM) : on va deux fois plus loin en plaine, le relief reste une barrière. L'archer monté garde ses PM moins 2 après un tir. La faction hostile chasse à 4 cases et erre à 6 cases de sa base.
- La console `ville population` règle le palier. Les sauvegardes passent au format 2 (les anciennes sont refusées).
- **« Or » devient « Économie »** partout (trésor, rendements des cases, butin des camps, échanges des villages, panneau de la ville, wiki) : le mot « or » reste pour le métal à venir. Le pilote écrit `economie=` au lieu de `or=`.
- **Traverser ses propres unités** : un trajet passe par la case d'une unité alliée, à condition d'en repartir dans le même tour vers une case libre (hors zone de contrôle) ; sinon il la contourne. Le trajet affiché au survol suit la même règle.
- **Groupes d'unités** (module `groupes.rs`) : Maj + clic ajoute une unité au groupe sélectionné (ou l'en retire) ; un clic droit envoie l'unité principale sur la case visée et les autres sur les cases libres les plus proches autour d'elle (chacune vers la plus proche d'elle-même). Ctrl + chiffre range le groupe, le chiffre le rappelle (sauf ville choisie), deux appuis rapides centrent la vue. Les autres membres du groupe ont un anneau plus pâle, en 2D et en 3D.
- **Travailleurs sur les cases** (étape 4a, module `intendant.rs`) : la moitié des habitants travaille ; chaque case a une capacité (500 sans aménagement, 1 500 pour un champ, 1 000 pour un pâturage ou une coupe de bois) et un rendement décroissant (maximum × (travailleurs / capacité)^0,7). L'intendant place les travailleurs par blocs de 100 là où le gain est le plus fort, la nourriture pesant lourd tant qu'elle manque ; au-delà des capacités, des habitants sont sans emploi. Les rendements deviennent fractionnaires (le trésor garde les fractions d'Économie). Le panneau de la ville montre les travailleurs et les sans-emploi ; le pilote, les cases et leurs effectifs.
- Console : `amenagement col lig champ|paturage|bois|aucun` (pose sans construire), `ville priorite col lig ressource cran`, `ville forcer col lig ccol clig effectif`. Sauvegardes au format 3 (aménagements, consignes, fractions du trésor).
- Pilote : `maj+clic col lig`, `ctrl+clic col lig` et `touche ctrl+1` (clic ou touche en tenant Maj ou Ctrl).
- **L'éclaireur devient une unité militaire** légère et peu coûteuse (famille « reconnaissance »), sans zone de contrôle.
- **Écran de la ville** (lot 1 de la refonte, maquette `docs/conception/maquettes/ecran-ville.html`) : touche C ou bouton « Écran de la ville » ; plein écran fermé par Échap. En-tête (palier, habitants, croissance, construction en cours), onglets **Rendements** (nourriture comparée au besoin, production, deniers, et d'où ils viennent : centre, cases), **Population** (habitants, actifs, sans-emploi), **Commerce** et **Histoire** (vides pour l'instant, et ils le disent) ; carte schématique des cases de la ville (grandeur principale ou remplissage, aménagements, infobulle, clic pour la fiche de la case, zoom, déplacement, légende au-dessus) ; liste des cases travaillées ou par remplissage.
- **Écran de la ville, lot 2** : clic sur une grandeur (ou touches 1 à 3) pour son détail, « D'où vient la nourriture ? » : graphique du passé (10, 20 ou 40 tours) et de la prévision au rythme actuel avec sa bande d'incertitude, seuils (besoin, croissance pleine, équilibre), sources et usages (réserve pour croître, excédent perdu, construction en cours, trésor) ; carte des cases aux couleurs de la grandeur avec sa valeur ; liste des cases triée ; fil d'Ariane (Échap remonte d'un cran) ; fiche de case avec le gain de 100 travailleurs de plus. Chaque ville garde l'historique de ses 40 derniers tours (sauvegardé ; les anciennes sauvegardes se chargent).
- **Charte graphique** commune (`charte.rs`) : couleurs, une couleur et une icône par grandeur (nourriture, production, deniers, eau, recherche, culture, défense, bien-être, habitants).
- **La monnaie s'appelle le denier** (choix de Benjamin) : « Économie » devient « deniers » partout (barre du haut, panneau et écran de la ville, journal, butin des camps, échanges des villages, wiki, console `ville priorite … deniers`, pilote `deniers=`).

### Nouvelle partie
- **Écran de nouvelle partie** au lancement du jeu (maquette v4 validée) :
  - **Code du monde** (`777-M-T29-C0-R0` : graine, taille, terres, climat, relief) avec Hasard, Copier, Coller (Entrée l'applique, un code illisible est signalé) ;
  - **taille** (Petite, Moyenne, Grande expérimentale) et **barbares** (rares, normaux, nombreux : un camp pour 320, 160 ou 80 cases de terre) ;
  - réglages encore **à venir**, affichés grisés : terres, climat, relief, peuples d'IA, difficulté, rythme, hégémonie, premiers tours guidés ;
  - **carte cachée** par défaut (elle gâcherait la découverte) ; « Révéler la carte » génère le monde en tâche de fond et en montre une miniature ;
  - **Mes configurations** : enregistrer la configuration (nom, commentaire, favorite), rétablir celle par défaut, retrouver les configurations enregistrées et celles des parties jouées (favorites, modifier, effacer) ; au chargement, choisir le même monde ou une nouvelle graine ;
  - Entrée lance la partie, Échap quitte.
- **Menu Échap** : « Enregistrer la configuration de ce monde… » (on voit en jouant si le monde est réussi) et « Nouvelle partie… », qui remplace « Nouveau monde… ».
- Les lancements avec une graine imposée (scripts, captures, pilote, tests) passent directement à la partie, comme avant.

### Sauvegarde
- **Sauvegarder et charger une partie** : menu Échap → « Sauvegarder la partie » et « Charger la dernière partie » ; console `sauver [fichier]` et `charger [fichier]`. Le monde se regénère depuis sa graine ; la partie (tour, trésor, hasard, unités, villes, camps et villages, cases explorées, territoire, ponts) est remise en place. Une partie rechargée continue exactement comme l'originale (vérifié sur 10 tours avec une ville, des barbares et de l'exploration). Une sauvegarde d'une version du jeu qui génère un autre monde est refusée avec un message clair. Fichiers texte (RON) dans `%APPDATA%\CivBuilder\sauvegardes`. Le journal n'est pas gardé.
- **Correctifs de déterminisme** : le hasard des villages n'est plus tiré à chaque déplacement mais à chaque visite ; les objets apparus dans une même image reçoivent leur identifiant dans l'ordre de leur case.

### Fondations techniques
- **Hasard tiré de la graine pendant la partie** (module `hasard.rs`) : les dégâts des combats, les visites des villages et des camps, et les décisions de la faction hostile ne dépendent plus du hasard du système. Même graine et mêmes ordres, mêmes résultats : c'est la base de la sauvegarde, de la reproduction des bugs et des parties d'IA comparables. Recharger avant une attaque ne changera pas son résultat. Chaque système a sa propre suite de tirages, par peuple.
- **Identifiants stables** (`game::Ident`) pour les unités, les villes, les camps et les villages, donnés dans l'ordre de création : les villes, les unités et les guerriers hostiles jouent dans l'ordre de ces identifiants, les camps et les villages dans l'ordre de leur case, et non plus dans l'ordre interne de Bevy, qui pourra changer au chargement d'une sauvegarde.
- **Peuple propriétaire** (module `peuple.rs`) : chaque unité et chaque ville appartient à un peuple (le joueur par défaut, la faction hostile pour les guerriers des camps), et le trésor se compte par peuple. Rien ne change en jeu ; c'est la base des peuples rivaux.
- **Durée du passage de tour mesurée** (module `chrono_tour.rs`) : chaque étape (villes et unités, faction hostile, villages et camps, cases exploitées, brouillard, connaissances) est chronométrée. La commande de console `chrono [tours]` donne la moyenne et le pire des derniers tours, en tout et par étape, face au budget de 2 s par tour (tolérance 4 s) ; un tour au-delà du budget est signalé dans le terminal. Aujourd'hui, en début de partie : moins d'une milliseconde.
- **Brouillard rangé en bits** (module `cases.rs`, `CaseSet`) : un bit par case, ligne par ligne (≈ 3 Ko pour une carte de 192 × 111), au lieu d'un ensemble haché. Les cases explorées se parcourent dans un ordre fixe. C'est la base d'un brouillard par peuple ; rien ne change en jeu.
- **Calcul de chemin plus rapide** : le chemin le plus rapide en tours (celui des unités) estime maintenant la distance restante (A*, avec une borne qui ne surestime jamais : même arrivée, vérifiée contre la recherche complète) et range ses tables dans des tableaux plats (`cases::Grille`). Un long trajet sur une carte de 192 × 111 passe de 8,5 à 2,3 ms (`bilan_chemins`).

### Outils de test
- **Mode joueur pilotable** : `CIVBUILDER_PILOTE=<fichier>` (`scripts/pilote.sh`) fait jouer une partie normale, sans mode debug, par des ordres ajoutés au fichier pendant la partie : clics sur la carte, touches, boutons de l'interface, captures. Les réponses ne donnent que ce que le joueur peut savoir. Plusieurs parties peuvent tourner en même temps, la vraie souris n'est pas déplacée.
- **Taille de la fenêtre au lancement** : `CIVBUILDER_FENETRE=1920x1080`.

### Ponts
- **Les ponts ont un effet en jeu**, et servent à tout le monde. Par un pont, une rivière se traverse sans surcoût. Sur un fleuve, on monte sur le pont par un de ses bouts pour 1 PM, et l'on peut s'y arrêter ou continuer.
- **Combat sur le pont** : pas de flanquement ni de charge contre une unité sur le pont ; l'infanterie et les lanciers le tiennent (+3, les lanciers encore +10 face à la cavalerie), les archers aussi face aux unités à pied sans bouclier (+3) ; contre les tirs, on y est exposé (−3). En sortie de pont : −5 en mêlée contre l'infanterie, les lanciers et l'archer-épéiste, −10 et pas de charge pour la cavalerie contre des lanciers, −5 pour tirer depuis le pont ; seuls les alliés déjà sur la rive flanquent ; plus de bonus de rivière à franchir.
- **Les villes construisent leurs ponts** : « Pont… » dans le panneau de production surligne les emplacements du territoire (rivière : une des deux cases suffit ; fleuve) ; un clic choisit, un clic droit annule. De 20 à 120 de production selon le débit.
- **Plus de pont décoratif** au stade Ville : seuls les ponts construits existent. La console en pose aussi (`pont col lig direction`).
- **Matériaux et formes** : pont de bois si une rive est boisée, de pierre sinon (1,5 fois plus cher). Arches de pierre, tablier de bois sur pilotis, et une seule portée haute au-dessus d'une gorge. Ponts plus larges, pour bien les voir. Une tour garde l'autre bout si la ville qui l'a construit est fortifiée.
- **Deux niveaux sur un fleuve** : une unité sur le pont, une autre dans l'eau, dans la même case. Un clic sur le tablier envoie l'unité sur le pont, ailleurs dans la case dans l'eau ; on ne monte sur le pont et on n'en descend que par ses deux bouts. Entre les deux niveaux, seulement des tirs : depuis le pont vers l'eau avec le bonus de hauteur. Une barque passe sous un pont tenu par l'ennemi.
- **Mode joueur pilotable** : `clic col lig côté` vise le bord de la case de ce côté (choix d'une arête de rivière) ; `CIVBUILDER_PILOTE_DEBUG=1` autorise toutes les commandes de la console, pour préparer une scène.
- **Dessin** : en 2D, une planche sur l'eau ; en 3D, un pont perpendiculaire au courant, posé sur une berge sèche, sans arbre à ses abords. Infobulle des cases.

### Climat et rivières
- **Pluies extrêmes plafonnées** : plus de 35 000 mm/an sur un versant, le maximum tombe vers 10 000 à 15 000 mm. L'eau qui ne tombe pas continue avec le vent : il y a 8 à 21 % de cases de désert en moins. Le monde généré change : pour une même graine, biomes, cours d'eau et noms ne sont plus les mêmes.
- **Rivières plus nombreuses, qui partent d'un filet** : une rivière se trace dès 100 m³/s, quelle que soit la taille de son bassin, et s'élargit depuis sa source. Plus de rivière qui naît d'un coup à pleine largeur.

- **Les cours d'eau creusent le relief après le climat** : les vallées suivent l'eau réelle, et les fleuves arrivent à la mer à peu près à son niveau au lieu de tomber d'une case à plus de 1 000 m. La houle et la marée renforcent le creusement des dernières cases. Les rias deviennent de vraies vallées noyées entre leurs reliefs. Altitude moyenne un peu plus basse (−6 à −8 %), lacs préservés.

- **Correctif : plus de rivière coupée net en pleine plaine** : une rivière qui arrivait dans une cuvette sèche s'y arrêtait, parfois à plus de 1 000 m³/s. Elle passe maintenant le col et continue.

- **Des vallées dans les chaînes de montagnes** : l'eau y converge en vrais réseaux d'affluents, au lieu d'un peigne de rivières parallèles sur des versants lisses. Altitude moyenne inchangée, mais un quart de falaises en plus. Le monde généré change.

- **Pluie plus proche de la Terre** (le monde généré change) :
  - **Montagnes moins détrempées** : l'air d'altitude passe au-dessus des massifs au lieu d'y condenser sans être soulevé. Les hautes terres recevaient 2,6 à 3,6 fois la pluie des plaines ; elles en reçoivent 1,6 à 1,8 fois, surtout sur le versant au vent.
  - **Façades côtières** : l'été, l'air descend davantage à l'ouest des continents, et le vent ralentit sur les terres. Les côtes ouest subtropicales sont plus sèches (20-30° : 373 mm au lieu de 456) et les côtes est relativement plus arrosées.
  - **Hautes latitudes moins arrosées** : le vent porte moins de vapeur vers les pôles au-delà de 40°. De 60 à 70°, 896 mm au lieu de 949.
  - Un peu moins de désert (12,5 % des terres au lieu de 13,7 %), la ceinture sèche de 30-40° est conservée ; les latitudes de 40-50° sont plus arrosées et plus boisées.

### Raccord est-ouest
- **Correctif en 3D** : les unités et les bâtiments des colonnes du bord ne disparaissent plus selon le côté d'où on les regarde, et les rivières restent continues. Un ennemi pouvait y être invisible.
- **Paroi sombre au raccord, en vue tournée** : près du bord est, vue tournée vers l'ouest (ou l'inverse), les parois d'une case haute du bord se dessinaient par-dessus le sol plus proche, de l'autre côté du raccord. La caméra 3D est maintenant ramenée dans le tour selon la position de son œil, et non plus selon le point qu'elle vise.

### Habitat de montagne et lisibilité
- **Plus de village ni de camp sans été** : aucun site là où la saison la plus chaude reste sous 5 °C. La limite suit le climat, pas l'altitude : un plateau chaud reste habité. Tous les villages et camps d'une graine changent donc de place.
- **Villages d'éleveurs** : sous 10 °C en été, un village prend des yourtes (sauf en toundra et sur neige).
- **Limite des arbres** : plus de forêt tempérée quand l'été reste sous 10 °C. Ces forêts des hauts plateaux tropicaux deviennent des prairies d'altitude (130 à 240 cases par graine) ; plus de yourtes au milieu des arbres.
- **Le relief reste plus visible sous les sites** en 3D : il est adouci de 25 % au lieu de 50 %.
- **Fleuves limoneux plus bleus en 3D** : ils ne ressemblent plus à des routes.

### Embouchures
- **Chaque embouchure de fleuve a maintenant un type** : delta digité, delta arqué, delta de marée, estuaire, lagune, ria ou fjord, selon ce que le fleuve apporte, ce que la houle et la marée reprennent, le fond devant elle, le relief et le froid. Il apparaît dans l'infobulle.
- **Estuaires et deltas digités dessinés** : l'estuaire s'ouvre en entonnoir vers la mer, le delta digité se divise en 2 à 4 bras à travers sa case. Plus d'île de fleuve isolée à l'embouchure.
- **Les deltas gagnent sur la mer** : une ou deux vraies cases de terre basse devant l'embouchure, que le fleuve traverse. Le monde généré change donc : pour une même graine, certaines côtes et certains noms ne sont plus les mêmes qu'avant.
- **Correctif** : les cours d'eau étaient invisibles en vue 2D depuis une version récente ; ils sont de retour.
- **Côtes plus naturelles en 3D** : le rivage ondule et ses angles s'arrondissent au lieu de suivre exactement le bord des cases, sans changer les cases ni les règles. Les bords des lacs et de la banquise en profitent aussi.
- **Rivières plus naturelles** : elles ne font plus la même vague régulière d'une case à l'autre, ni en parallèle de leurs voisines.
- **Rias, fjords et deltas arqués dessinés** : dans une ria ou un fjord, la mer remonte la vallée sur deux cases ; un delta arqué forme un lobe de terre sur la côte, avec une seule sortie.
- **Embouchures en vue 2D** : l'eau s'arrête à la côte et se fond dans la couleur de la mer, sur toutes les cartes. Plus de griffes sur la mer.
- **Fleuves plus naturels** : sources effilées au lieu de bouts ronds, bouches sans barres droites, écume de chute progressive au lieu d'éclats et de barres blanches, plus d'eau coupée à angle droit dans un virage avant l'embouchure.
- **Côtes et embouchures plus propres** : plus de triangles ni d'éclats de sable au bout des deltas, plus de dalles claires ni de coupure droite entre le fleuve et la mer, plus de fleuve côtier qui bleuit le long de la mer, plus d'écume ni de rapides dans une embouchure, plus de plage de sable contre la banquise ni sur la neige, plus de fentes noires dans les falaises, rapides sans bandes. Sur les cartes thématiques en 3D, l'embouchure se fond dans la couleur de la carte.

### Banquise
- **La banquise apparaît sur les cartes** (normale, climat, altitude, courants, marées, houle…, sauf les plaques et la géographie) et dans l'infobulle, près des pôles, au-delà de 72 à 80° selon l'endroit, avec un bord découpé. Elle arrête la houle.
- **On peut marcher sur la banquise** (2 PM par case, sauf au pied d'une côte escarpée), ce qui ouvre des passages entre des terres. On ne peut pas y fonder de ville, et aucun village ni camp n'y prend place. Elle garde ses rendements de mer : on y pêche toujours.

### Houle
- **Modèle de houle** (module `waves.rs`) : un bilan d'énergie simplifié à la génération (12 directions, 3 périodes), sous le vent de chaque régime du climat : la houle naît du vent, voyage de case en case, traverse les océans, s'use sur les plateaux et s'arrête à la banquise. Moins d'une seconde de plus à la génération (0,8 s en profil de développement sur 16 fils). Un modèle complet, type WaveWatch III, prenait plus de 2 minutes dans notre comparatif.
- **Carte de la houle** (menu des cartes, touche T, console `carte houle`), en 2D et en 3D : hauteur moyenne par classes et flèches vers où va la houle, plus longues pour une longue période. Infobulle d'une case de mer : hauteur, plus forte hauteur de l'année, période, origine, déferlement sur la côte.
- La houle servira au type d'embouchure et aux flèches de sable ; ses effets en jeu restent à décider. Le reste du monde ne change pas : une même graine donne les mêmes terres, climats et cours d'eau.

### Villes et villages
- **Une ville naît en hameau** : quelques maisons, un grand toit et un mât à la couleur du joueur, sans enceinte. Elle devient bourg à 3 habitants, puis ville à 6 (seuils provisoires), en ajoutant des bâtiments sans déplacer les autres.
- **Muraille** : elle n'existe que si la ville a une fortification, et s'agrandit avec elle. La règle de jeu reste à concevoir : pour l'instant, seule la console la pose (`ville fortifier`, `ville population`). Elle suit le terrain, longe l'eau, et s'ouvre par plusieurs portes : un bâtiment à cheval sur le mur, en arc avec herse pour la pierre et la terre, à linteau pour le bois, à deux tours pour la porte principale.
- **Matériaux du lieu** : dix familles (forêt, prairie, taïga, désert, froid, tropiques, broussailles, savane, steppe, marais) habillent villes et villages.
- **Adaptation à la case** : plus de remparts sur les rivières ni de bâtiments superposés. Rien ne se construit sur une rive ou juste derrière ; en pente, les maisons suivent le terrain et le bas des murs épouse le sol, sans remblai.
- **Ponton et barque** quand l'eau est proche ; avec une muraille, le ponton part de la porte d'eau.
- **Chemins discrets** : un axe passe devant le grand toit et relie les sorties et les grandes zones.
- **La garnison se tient sur la place**, et non plus sur un donjon. Les arbres restent autour de la ville, qui occupe une clairière.
- **Technique** : un maillage par site au lieu d'une entité par pièce (modules `settlements.rs` et `settlement_mesh.rs`).

### Camps hostiles
- **Trois types, selon la case** : le Fort (case plate et dégagée, palissade pleine et tour de guet), le Repaire (relief, adossé à des rochers) et le Campement (ailleurs, tentes et pieux inclinés). Le nom du camp annonce son type : les noms changent, à graine égale.
- **Une menace qui se lit à la forme** : des pointes partout, et un rouge réservé à l'étendard, à la tente du chef et à la tour de guet. Les autres tentes prennent les matériaux du lieu.
- **Fumée d'alerte** : elle s'épaissit dans les deux tours qui précèdent la sortie d'un guerrier (essai).
- **Les noms des villages et des camps passent sous le site** en 3D.

### Marais
- **Maisons sur pilotis au-dessus des mares**, chacune entourée d'une galerie, reliées à la terre par des passerelles alignées qui ne traversent aucun bâtiment.
- **Une ville draine son sol** : les mares rétrécissent quand elle grandit, sans que les maisons bougent.
- **Plus lisible** : chaque stade ajoute des maisons (3, 5 puis 7), les passerelles restent à angle droit, le grand toit se place derrière la place et un escalier descend vers la caméra.

### Ponts
- **Une ville sur un cours d'eau jette un pont au stade Ville**, en travers du courant et jusqu'à la terre ferme de l'autre rive, même chez le voisin quand la rivière suit le bord de la case. Il est en pierre sous une muraille de pierre, en bois sinon. Il prend la meilleure place, et le ponton s'en écarte. C'est pour l'instant du décor : il ne change ni les déplacements ni les combats.
- **Une ville fortifiée le garde** : une porte dans l'axe du pont, et une tour de tête de pont sur l'autre rive.

### Détails
- **La barque** se pose là où l'eau est assez large pour elle, le long du ponton ou un peu au-delà, et il n'y en a pas si l'eau manque. Ponton et barque suivent le rivage tel qu'il est dessiné, même arrondi.
- **Les pieux du Campement** sont plus épais et plus hauts, pour se lire de loin.

## 0.3.1 — 6 novembre 2026

Correctif de performances : la carte s'affiche bien plus vite en 3D, et les cartes les plus lourdes ne ralentissent plus le jeu.

### Performances de la vue 3D
- **Cases construites plus vite** : toute la carte en 6,9 s au lieu de 11 (8,8 s au lieu de 15,3 en 256×148), 60 images/s tenues sur un écran 60 Hz. Le temps de calcul du relief passe de 6 à 10 ms par image.
- **Cartes des plaques et du réseau hydrographique** : elles se construisent maintenant aussi vite que les autres (plaques 16,2 → 6,8 s, réseau 13,7 → 6,6 s), sans descendre sous 60 images/s. Leurs tracés ne sont plus reposés sur le sol qu'au plus toutes les 0,5 s pendant la construction ; ils peuvent donc rester décalés du sol pendant ce court moment.
- **Carte des marées, lignes et flèches activées** : même correction (jusqu'à 16 → 11 s, mesuré avant le passage à 10 ms).
- **Réseau hydrographique** : seuls les chevrons du courant visibles à l'écran sont calculés (5,3 → 0,3 ms par image une fois la carte construite).
- **Cartes du climat et des courants** : flèches gardées d'une image à l'autre (climat 10,7 → 7,3 s pour toute la carte ; 8,1 → 6,1 ms par image une fois construite).

Mesures sur toute la carte révélée, graine 777, carte 192×111 sauf mention contraire, sans défilement, en profil de développement (Ryzen 7 7800X3D, RTX 5070). Sur un écran plus rapide que 60 Hz, le jeu affiche moins d'images pendant la construction (simulé sans synchronisation verticale : environ 98 → 78 images/s).

### Technique
- **Passage à Bevy 0.20.1** : champs de saisie avec `TextInput`, plus d'`Interaction` ni de `Button`, dépréciés : le module `pointer.rs` tient l'état du pointeur (`UiPointer`) à partir du picking de Bevy.
- Textes d'aide du menu alignés à gauche : la mise en page du texte a changé avec Bevy.
- Seuls les éléments cliquables, les panneaux et les champs réagissent au pointeur ; un élément recouvre maintenant ce qui est dessous (un nom de la carte sous un panneau n'est plus survolé).
- **Worktrees amorcés automatiquement** : un hook de Claude Code copie le `target` du checkout principal au démarrage d'une session en worktree (premier build en ≈ 1 min 15 au lieu de ≈ 6 min).

## 0.3.0 « Planet Update » — 4 novembre 2026

Le monde devient une planète : une carte sans bord façonnée par la tectonique, des climats, des mers et des cours d'eau plus réalistes, des lieux nommés à toutes les échelles, une vue 3D plus vivante et des actions mises en scène. Les mondes changent, à graine égale.

### Planète et relief
- **Carte sans bord** : elle fait le tour de la planète d'est en ouest, sans raccord visible, en 2D comme en 3D, et toutes les règles en tiennent compte. Seuls les pôles restent des bords, bordés de mers navigables ; au moins une mer fait le tour du monde.
- **Plaques tectoniques** : une douzaine de plaques nommées, en mouvement (1 à 9 cm/an), avec leurs frontières (collision, subduction, arc d'îles, rift, dorsale, faille transformante) et leurs points chauds. Le relief en découle : chaînes, fosses, arcs et chapelets d'îles. Nouvelle carte « Plaques ».
- **Continents variés** : 4 à 6 grands blocs de tailles inégales et quelques microcontinents, indépendants des plaques, aux côtes découpées (golfes, péninsules, bras de mer). Un rift devient, selon sa vitesse d'écartement, une vallée à lacs, un bras de mer ou un jeune océan.
- **Érosion** : l'eau creuse le relief avant la pose des cours d'eau (vallées, cols, cuvettes comblées) ; beaucoup moins de lacs.
- **Altitude en mètres** : elle devient la référence (déplacements, combat, climat, infobulle) et le sol 3D la suit. 9 niveaux au lieu de 6, plus de plaines basses (~70 % des terres sous 1 000 m), chaque monde à sa façon.
- **Relief propre à chaque case** : plat, vallonné, accidenté, montagne ou pic, en plus du biome (le terrain « Montagnes » disparaît). Nouvelles cartes « Altitude » et « Relief ».
- **Fonds marins** calculés d'après les plaques : plateau continental, talus, abysses et fosses.

### Climat et océans
- **Courants marins** poussés par les vents simulés : tourbillons, courants rapides de bord ouest, courant circumpolaire, dérive chaude vers les hautes latitudes. Ils donnent la température de la mer, donc la douceur, la pluie ou les déserts des côtes. Nouvelle carte « Courants marins ».
- **Un monde moins sec** : orages tropicaux, mousson, dépressions d'hiver sur les mers chaudes, vent calmé à l'équateur. Déserts mieux placés et moins étendus (de 13-27 % à 11-19 % des terres selon la graine), plus de jungle.
- **Marées** simulées à la génération (eau peu profonde, Lune, Coriolis, frottement) : marnage, heure de la pleine mer et courant de marée, points sans marée, golfes qui résonnent. Nouvelle carte « Marées », avec lignes d'heure et sens de la marée en option. Leurs effets en jeu restent à décider.

### Cours d'eau
- **Tracés naturels** : méandres sur les fleuves, rivières d'un seul trait raccordées de coin en coin, largeur liée au débit et variable le long du cours.
- **Couleur de l'eau** selon le débit et les terres traversées : limoneuse en climat sec, claire en climat froid, mêlée aux confluences.
- **Rapides et chutes** : rapides composés dans le sens du courant, chutes d'eau qui épousent le relief.
- **Îles fluviales** sur les fleuves de plaine, bancs de sable ou îles boisées, seules ou par groupes, avec bras secondaires.
- **Raccords soignés** : embouchures évasées, affluents qui rejoignent le fleuve en biais et s'y fondent, berges continues d'une case à l'autre.

### Noms et géographie
- **Langues du monde** : huit langues en partie inventées, réparties selon les régions, remplacent les noms réels ; noms descriptifs, grammaire française et origine du nom dans l'infobulle.
- **Tout est nommé** : océans, mers, golfes, baies et détroits ; continents, îles, archipels, péninsules et isthmes ; chaînes, massifs, cols, plateaux et collines ; forêts, déserts, steppes et autres régions naturelles ; villages, camps et bancs de poissons.
- **Noms lisibles** : ils apparaissent à leur niveau de zoom, avec un quota par rang pour garder une densité régulière, et se courbent le long des chaînes et des golfes allongés.
- **Carte « Géographie »** : le découpage en zones nommées (terres, mers, relief ou régions), une couleur par zone, de plus en plus détaillé en zoomant.
- **Trouver les lieux** : recherche (Ctrl+F) avec filtres et vol de la caméra ; noms, légendes et étiquettes cliquables ; barre de lieu qui situe le centre de la vue (« Inetsie › Jungle de Shuchets › Bassin du Smurask »).

### Règles du jeu
- **Les montagnes coûtent du temps** : 4 à 8 PM selon l'altitude, avec une dette de PM reportée sur les tours suivants ; la cavalerie paie double, l'éclaireur les deux tiers. Cols, vallées et contournements deviennent les bons passages. Une pastille indique les tours d'immobilisation.
- **Terrain et pentes** : relief accidenté +2 PM ; montée de 400 à 900 m +1 PM, au-delà +2 PM ; falaises (1 000 m) et pics infranchissables.
- **Combat en hauteur** : être plus haut d'au moins 250 m avantage en défense de mêlée, au tir et en portée ; montagne +6 et relief accidenté +3 en défense.
- **Pêche** : eaux peu profondes 2 nourriture, haute mer 1. Eaux poissonneuses (+1) sur les remontées d'eaux froides, les monts sous-marins, autour des îles volcaniques et à la rencontre des courants, regroupées en bancs sur la nouvelle carte « Ressources ».

### Mise en scène et interface
- **Déplacements et combats animés** : les unités glissent de case en case ; élans, tirs, impacts et PV perdus en chiffres flottants ; récompenses affichées sur les villages et camps.
- **Fin de tour lisible** : les actions hostiles visibles se jouent l'une après l'autre, et le tour suivant attend la fin des animations (sauf en vitesse Instantanée).
- **Journal hiérarchisé** : cinq niveaux d'importance, flash selon la gravité, noms cliquables qui amènent la caméra sur place.
- **Menu Échap en pages** : Paramètres (vitesse des animations, mode debug, enregistrés d'une partie à l'autre) et Nouveau monde (graine à taper, copier, coller ou tirer au hasard).
- **[Comment se joue CivBuilder](docs/ANIMATIONS.md)** : un GIF par action, en 2D et en 3D, tourné par le jeu lui-même.

### Vue 3D
- **Relief modelé dans le sol** : crêtes et ravines continues, roche, neige et limite des arbres, montagnes reliées en chaînes, pics en aiguille.
- **Figurines** propres à chaque type d'unité, en petits groupes pour les militaires, posées sur le relief, en barque sur les fleuves, plus grandes et surmontées d'une icône de loin.
- **Villes et camps qui suivent le terrain**, sans terrasse.
- **Plus fluide** : relief de toute la carte construit en ≈ 7 s au lieu de ≈ 19 s, 60 images/s tenues pendant la construction, plus de saccades en zoomant en se déplaçant ou en changeant de carte.
- **Correction** : plus de lits de cours d'eau à la hauteur d'eau périmée (trous noirs, marches) en explorant.

### Outils de développement
- **Console de debug** (touche ², en mode debug) : observer une case, chercher des cases par critères, modifier le terrain (brut ou avec recalcul local `~`), refaire l'eau ou le climat, préparer une partie (`unite`, `ville`, `tour`, `chemin`), avec complétion par Tab.
- **Scripts de commandes** (`CIVBUILDER_SCRIPT`) pour rejouer un cas sans toucher au code ; `verifier` liste les incohérences d'une carte (aucune sur une carte générée).
- **Modes capture** (`CIVBUILDER_CAPTURE`) : vues PNG du rendu, de la mise en scène et de l'exploration, sans jouer.
- **Nouveaux bilans chiffrés** : `bilan_terres`, `bilan_relief`, `bilan_regions`, `bilan_courants`, `bilan_marees`, `bilan_aridite`, `bilan_montagnes`, `bilan_verifier`, `formes_des_terres`…
- **Compilation plus rapide** : `scripts/amorcer-target.ps1` réutilise les dépendances déjà compilées dans un nouveau worktree.

## 0.2.0 « Biome Update » — 24 octobre 2026

Les saisons du climat façonnent maintenant les paysages  : 7 nouveaux terrains, de la jungle à la taïga.

### Biomes
- Le climat garde ses 4 saisons (température, pluie, évaporation possible de chaque saison).
- 7 nouveaux terrains : **jungle**, **savane**, **savane arborée**, **broussailles** (climat méditerranéen, été sec ; au-delà d'un indice d'aridité de 0,75, forêt tempérée), **taïga**, **steppe**, **marais** ; la forêt devient **forêt tempérée**.
- Règles : saison la plus froide (taïga sous −3 °C, tropical au-dessus de 18 °C), nombre de saisons sèches, été sec ; marais sur les terres basses et plates au bord d'un fleuve ou d'un lac, en climat humide.
- Rendements : jungle et taïga 1/1/0 (2 PM, couvert boisé), savane 2/0/0, savane arborée et steppe 1/1/0, broussailles 1/1/1, marais 1/0/0 (3 PM, pas de charge).
- Couvert boisé (forêt, jungle, taïga) : +défense, pas de charge, tir gêné.
- Rendu : couleurs propres en 2D ; en 3D, feuillus, grands arbres de jungle, conifères, acacias, buissons, roseaux et flaques.
- Infobulle : climat « à été sec » ou « à N saisons sèches ».

## 0.1.0 « River Update » — 24 octobre 2026

Première version figée : monde généré (relief, climat, cours d'eau), exploration, villes, unités et combats.

### Monde
- Planète 192 × 111 cases de 40 km, pôle à pôle ; relief en 6 niveaux, montagnes et collines.
- Climat simulé à la génération (pression, vents, deux couches d'air, pluie, sols, courants marins, 4 saisons moyennées) ; terrain selon la température et l'aridité.
- **Trois passes climat ↔ eau** : à partir de la deuxième, les lacs évaporent et humidifient l'air ; **effet de lac** (air soulevé sur la rive sous le vent).
- Petites étendues de mer enclavées changées en cuvettes (lac qui déborde ou cuvette sèche).

### Cours d'eau et lacs
- Écoulement réel (m³/s) : lacs qui débordent ou sans issue, pertes de lit en climat sec, fleuves (cases d'eau entre deux berges) et rivières sur les arêtes.
- Lacs d'eau douce, lacs salés et mers intérieures ; fleuve absorbé par un lac qui l'entoure.
- Bassins versants (cases partagées hachurées, bassin fermé nommé d'après son lac) ; noms réels selon la latitude pour les fleuves, rivières, lacs et mers intérieures.
- Effets en jeu : traversée de rivière +2 PM, fleuve 3 PM, bonus de défense, eau douce +1 nourriture.
- Rendu 2D et 3D : lits taillés, berges, embouchures, cascades, plages, coupure nette au bord de la zone explorée.

### Cartes et interface
- Cartes normale, relief, climat, bassins versants, débits et réseau hydrographique (rang par rapport au déversoir, sens du courant animé).
- Surbrillance d'un cours d'eau ou d'un lac : affluents (orange puis jaune), récepteur (bleu), suite jusqu'à la mer (violet), chevrons du courant.
- Étiquettes des cours d'eau, lacs et villes selon le zoom, sans chevauchement.
- Panneau « Options de carte » (touche O) : étiquettes, mode de surbrillance, rubriques de l'infobulle pour chaque carte.
- Wiki en jeu (F1).

### Jeu
- Exploration automatique optimisée (36 éclaireurs ≈ 150 ms par tour), villes, production, unités, combats, camps hostiles, villages.
