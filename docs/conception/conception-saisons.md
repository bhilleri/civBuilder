# Conception — Saisons, calendrier et météo

## Objectif
Faire des saisons un élément du jeu, pas seulement un effet visuel : un calendrier mensuel, une
météo qui s'écarte des moyennes du climat, des cours d'eau qui suivent l'année (crues, étiages,
gel) et des effets sur les déplacements. Le rendu suivra (`conception-rendu-3d.md`, étape 9 g).

## Décisions de Benjamin (09/11/2026)
- **Cycle saisonnier séparé de la date historique** : les saisons tournent toujours au même
  rythme ; l'année affichée avance plus vite selon l'ère (ères à définir plus tard).
- **1 tour = 1 mois** (12 tours par an) par défaut, avec un **mode à 2 tours par mois**.
  Pourquoi : laisser le temps d'une vraie stratégie militaire (un hiver de trois mois est court :
  une campagne a le temps de se jouer et une saison prend de l'importance), et laisser une place
  à la météo (un ouragan qui dure 3 tours ne doit pas occuper un quart de l'année).
- **Météo = moyennes du climat + écarts générés.** Écarts du mois et anomalies durables
  (sécheresse, hiver rude) sur une région entière, plus rares, qui pèsent sur les récoltes.
  Méthode : un fond lent (écart de pression caché) et des systèmes qui se déplacent par-dessus,
  cyclones compris, et l'ensemble doit rester cohérent (voir plus bas). Contrainte : la météo ne
  doit pas peser lourd sur le processeur ni la carte graphique.
- **Cours d'eau** : crues et sécheresses visibles, et lit d'hiver / lit d'été des cours d'eau.
- **Déplacements** : oui (neige, boue au dégel, gués impraticables en crue, fleuves gelés franchissables).
- **Plus tard** :
  - navigation (à doser pour ne pas la rendre impossible) ;
  - combat (délicat avec le système actuel) ;
  - santé (avec des villes plus riches) ;
  - récolte saisonnière, stocks et greniers (idée à tester : jouable ou non) ;
  - prévisions imparfaites qui progressent avec le développement (le jeu en est encore aux unités et aux ponts).

## Ce qui existe
- `climate.rs` simule quatre saisons par case : `season_temp`, `season_rain`, `season_pet`
  (température, pluie, évaporation possible), saisons sèches, été sec. Les biomes en découlent.
- `hydro.rs` donne un débit annuel moyen par case (`discharge`) et par arête.
- `currents.rs`, `tides.rs` : courants et marées fixes toute l'année ; température de la mer calculée.
- Le jeu n'a ni date ni calendrier.

## Proposition (à valider)
### Calendrier
- Le tour porte un mois (12 tours par an, ou 24 en mode « 2 tours par mois », par demi-mois).
- Barre du haut : mois et **saison locale** de la case visée (les hémisphères sont inversés).
- Date historique : plus tard, avec les ères.

### Climat mensuel
- **Interpoler les quatre saisons simulées**, sans simuler douze mois : courbe périodique à deux
  harmoniques qui passe exactement par les quatre valeurs, pluie bornée à 0. Coût nul à la génération.
- Les biomes restent calculés sur le climat moyen : la météo ne change pas le terrain.

### Météo du mois : un fond lent et des systèmes qui se déplacent (décidé le 09/11/2026)
Benjamin a retenu une météo en deux couches, avec une exigence : **l'ensemble doit rester cohérent**.

**Couche 1, le fond lent** : un écart de pression caché.
- Champ continu sur une grille grossière (≈ 1 point pour 8 × 8 cases, environ 24 × 14 points), interpolé sur les cases.
- **Persistance** d'un mois au suivant : une part du mois précédent plus un tirage (graine et
  tour). Il fait les anomalies durables : sécheresse, hiver rude, blocage anticyclonique. Elles
  sont rares, s'étendent sur 500 à 1 500 km et durent 2 à 6 mois.
- Il décide où les systèmes naissent et passent, et module leur nombre et leur force.

**Couche 2, les systèmes qui se déplacent** : dépressions et anticyclones.
- Environ 50 à 150 en même temps sur la planète, chacun avec une position, une intensité, un rayon et une vitesse.
- 4 pas par jour simulé (≈ 120 par mois). Chaque système naît, se déplace et meurt :
  - **naissance** : là où le climat les fait naître (contraste terre-mer et mers chaudes de
    l'hiver, déjà repérés par `CYCLOGENESIS_*` dans `climate.rs`, front polaire), selon le fond lent ;
  - **déplacement** : avec le vent moyen du mois ; vers l'est aux latitudes moyennes, ce qui forme le rail des dépressions ;
  - **évolution** : il se creuse ou se comble, puis disparaît.
- Leurs effets (pression, température, pluie, vent, tempêtes) sont **cumulés sur le mois** sur la
  grille grossière, puis interpolés sur les cases.
- **Les cyclones sont des systèmes de cette couche**, avec leurs propres règles de naissance et
  d'intensité (voir « Cyclones ») : un seul moteur.

**Règles de cohérence.**
- **Mêmes moyennes que le climat** : sur de nombreuses années fictives, la moyenne de la météo
  doit redonner le climat mensuel de chaque case (température, pluie). Les biomes, calculés sur ce
  climat, restent donc justes. Les écarts sont calibrés pour avoir une moyenne nulle.
- **Une seule source pour chaque grandeur** : vent du mois, température de la mer et pression
  viennent du climat ; la météo n'en invente pas d'autres.
- **Températures et pluie découlent de la pression**, pas de tirages indépendants : pression
  haute = sec, été chaud ou hiver froid ; pression basse = humide et doux.
- **L'eau suit** : la pluie du mois alimente la neige au sol et le débit mensuel, donc les crues et les étiages.
- **Un seul moteur pour le jeu et pour la génération** : le même pourra servir à la tâche 7 g du
  climat (« dépressions qui se déplacent »), au prix d'années simulées à la génération.
- **Déterminisme** : même graine, même tour, même météo.

**Coût.**
- Fond lent : moins d'une milliseconde par tour.
- Systèmes : environ 100 systèmes × 120 pas × quelques centaines de points, quelques millisecondes par tour (`rayon` si besoin).
- Rien sur la carte graphique, hors rendu. Le vrai coût est en conception et en réglages.

**Bilans chiffrés à ajouter.**
- Écart entre la moyenne de la météo sur 50 ans et le climat, par case et par mois (doit être faible).
- Fréquence des sécheresses et des hivers rudes par région.
- Écarts types mensuels comparés aux ordres de grandeur réels.
- Densité des trajectoires de dépressions (rail des dépressions) et des cyclones.

### Neige et gel
- **Stock de neige par case** : la précipitation tombe en neige sous 0 °C et s'accumule ; elle
  fond selon les degrés au-dessus de 0 (méthode des degrés-jours).
  Elle donne la couverture de neige (déplacements, rendu) et la crue de printemps des fleuves de montagne.
- **Gel des lacs et des cours d'eau** : quand le mois est assez froid (seuil à régler, par exemple
  moyenne sous −5 °C), un fleuve gelé est franchissable.
- **Boue au dégel** : sol gelé qui dégèle avec un sol humide.

### Débit mensuel, crues et étiages
- Écoulement du mois par case : pluie moins évaporation, plus la fonte, routé vers l'aval dans
  l'ordre déjà connu de `hydro.rs`, avec un retard selon la taille du bassin.
  - Régimes : pluvial, à fonte de neige, tropical (saison des pluies).
  - Une passe par tour, quelques millisecondes.
- **Lit mineur et lit majeur** :
  - **Étiage** : la largeur dessinée suit le débit du mois. En été sec, le lit rétrécit, des bancs
    de sable apparaissent, et les petits cours d'eau des régions arides peuvent être à sec (oueds).
  - **Crue** : au-delà du débit à pleins bords (≈ crue de 1 à 2 ans), l'eau déborde sur la plaine
    inondable (cases basses et plates le long du cours d'eau).
- **Effets** : gué praticable à l'étiage, impraticable en crue ; navigation sur les fleuves (plus tard).

### Cyclones (règles validées pour l'instant, 09/11/2026)
**Dans la réalité.**
- **Naissance** : mer d'au moins 26,5 °C sur ≈ 50 m, latitude d'au moins 5° (rotation de la
  Terre), peu de cisaillement du vent, air humide en altitude, perturbation de départ (onde d'est,
  amas orageux), fin d'été. Aucun dans l'Atlantique Sud ni l'est du Pacifique Sud (courants froids, cisaillement).
- **Intensification** : machine thermique, intensité maximale selon la température de la mer ;
  cyclone en 2 à 5 jours ; vie de 1 à 2 semaines.
- **Déplacement** : porté par les vents qui l'entourent. D'est en ouest dans les alizés
  (15-25 km/h) avec une dérive vers le pôle, recourbure au bord de l'anticyclone subtropical, puis
  vers l'est dans les vents d'ouest (30-50 km/h), où il devient une tempête ordinaire.
- **Affaiblissement** : sur terre, environ la moitié de sa force par jour, mais fortes pluies
  loin dans les terres ; sur mer froide ; par cisaillement.
- **Dégâts** : vent plus fort à droite de la trajectoire au nord (à gauche au sud), marée de
  tempête sur les plateaux peu profonds, crues ; sillage froid derrière lui.

**Dans le jeu.**
- **Naissance** : chance par case de mer et par mois selon l'excès de la mer au-dessus de
  26,5 °C, la latitude (5-25°), le mois (fin d'été local) et l'écart de pression du mois (pression
  basse, plus de cyclones). Cisaillement non simulé (une seule couche de vent) : ignoré au début.
  Nombre moyen par an réglé selon la surface de mer chaude.
- **Trajectoire** : pas de 6 h ; direction = vent du mois au point (recourbure naturelle), plus une
  dérive vers le pôle et un tirage de la graine et du tour ; intensité qui tend vers le maximum
  permis par la mer locale, baisse vite sur terre et sur mer froide ; fin quand il s'affaiblit ou
  entre dans les vents d'ouest. Coût : quelques cyclones de ≈ 60 pas par mois, négligeable.
- **Durée en tours** : à un mois par tour, la trajectoire tient dans un tour, jouée dans
  l'animation de fin de tour. En mode 2 tours par mois, un cyclone peut être à mi-parcours en fin
  de tour : le joueur voit la menace arriver.
- **Effets** : dégâts dans un rayon de 3 à 7 cases selon l'intensité, marée de tempête sur les
  côtes basses du plateau (`tides.rs`), pluie ajoutée au débit du mois (crues) ; navires plus tard.
- **Carte du risque cyclonique** : une cinquantaine d'années fictives simulées à la génération ;
  l'infobulle signale les côtes exposées.
- **Souhait de Benjamin, à revoir plus tard** : qu'un cyclone dure plusieurs tours. Pistes : le
  ralentir dans le jeu, ou découper le tour en épisodes quand un événement est en cours. Les règles
  pourront changer.

### Autres événements
- **Tempêtes d'hiver** des mers des latitudes moyennes : nombre du mois selon l'écart de pression (navigation, plus tard).

### Déplacements (chiffres à régler)
- Neige épaisse : PM en plus.
- Boue au dégel : PM en plus, pas de charge.
- Gué en crue : infranchissable.
- Fleuve ou lac gelé : franchissable à pied.

## Étapes
1. Calendrier (mois, saison locale, mode 2 tours par mois).
2. Climat mensuel interpolé.
3. Météo, fond lent : écart de pression, températures, pluie, anomalies durables ; bilan de cohérence avec le climat.
4. Météo, systèmes qui se déplacent : dépressions, anticyclones, tempêtes ; bilans (cohérence, rail des dépressions).
5. Neige au sol, gel, boue.
6. Débit mensuel, crues et étiages, lit mineur et majeur, oueds.
7. Effets sur les déplacements.
8. Cyclones (systèmes tropicaux du même moteur).
9. Rendu (étape 9 g du rendu 3D) et cartes thématiques (écart du mois, neige, débit, risque cyclonique).

## Mois de départ (décidé le 10/11/2026)
Une partie commence au **printemps de l'hémisphère nord** (mars ou avril, à régler) : la belle
saison au nord, l'automne au sud.

## Questions ouvertes
- Seuils et chiffres des effets sur les déplacements.
- Réutiliser le moteur des systèmes à la génération du climat (tâche 7 g) : à décider après l'étape 4, selon son coût.
