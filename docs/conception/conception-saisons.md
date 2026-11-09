# Conception — Saisons, calendrier et météo

## Objectif
Faire des saisons un élément du jeu, pas seulement un effet visuel : un calendrier mensuel, une
météo qui s'écarte des moyennes du climat, des cours d'eau qui suivent l'année (crues, étiages,
gel) et des effets sur les déplacements. Le rendu suivra (`conception-rendu-3d.md`, étape 8 g).

## Décisions de Benjamin (09/11/2026)
- **Cycle saisonnier séparé de la date historique** : les saisons tournent toujours au même
  rythme ; l'année affichée avance plus vite selon l'ère (ères à définir plus tard).
- **1 tour = 1 mois** (12 tours par an) par défaut, avec un **mode à 2 tours par mois**.
  Pourquoi : laisser le temps d'une vraie stratégie militaire (un hiver de trois mois est court :
  une campagne a le temps de se jouer et une saison prend de l'importance), et laisser une place
  à la météo (un ouragan qui dure 3 tours ne doit pas occuper un quart de l'année).
- **Météo = moyennes du climat + écarts générés.** Écarts du mois et anomalies durables
  (sécheresse, hiver rude) sur une région entière, plus rares, qui pèsent sur les récoltes.
  Méthode à choisir (systèmes de pression ou écarts probabilistes, voir plus bas) ; contrainte :
  la météo ne doit pas peser lourd sur le processeur ni la carte graphique.
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

### Météo du mois : une pression cachée qui commande le reste
À un mois par tour, une dépression (quelques jours) dure moins qu'un tour : suivre chaque
dépression et chaque anticyclone n'apporterait rien au jeu. Je propose un compromis entre les deux
méthodes :
- **Un écart de pression caché**, champ continu sur la planète, calculé sur une grille grossière
  (≈ 1 point pour 8 × 8 cases, environ 24 × 14 points) puis interpolé.
  - Il évolue d'un mois au suivant avec de la **persistance** : une part du mois précédent plus un
    tirage. Un anticyclone de blocage peut ainsi durer plusieurs mois et faire une sécheresse.
  - Il est poussé par le vent moyen du mois : les perturbations vont d'ouest en est aux latitudes moyennes.
  - Tirage déterministe : graine et numéro du tour.
- **Températures et pluie en découlent**, avec des coefficients selon la saison et la latitude :
  - pression haute = temps sec, été chaud ou hiver froid ;
  - pression basse = humide et doux ;
  - plus un petit bruit propre à la case.
  Les écarts restent ainsi cohérents entre eux, au lieu de tirages indépendants.
- **Anomalies durables** : rares, de 500 à 1 500 km, sur 2 à 6 mois, ajoutées au champ de pression (sécheresse, hiver rude).
- **Coût** : quelques centaines de points par tour, plus une interpolation sur 21 000 cases.
  Moins d'une milliseconde, une fois par tour. Rien sur la carte graphique, hors rendu.
- Bilans chiffrés à ajouter : fréquence des sécheresses et des hivers rudes par région, écarts types mensuels comparés aux ordres de grandeur réels.

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
3. Météo du mois : écart de pression, températures, pluie, anomalies durables ; bilans.
4. Neige au sol, gel, boue.
5. Débit mensuel, crues et étiages, lit mineur et majeur, oueds.
6. Effets sur les déplacements.
7. Cyclones et tempêtes.
8. Rendu (étape 8 g du rendu 3D) et cartes thématiques (écart du mois, neige, débit).

## Questions ouvertes
- Méthode de la météo : l'écart de pression caché (proposé) convient-il, ou faut-il de vrais
  systèmes mobiles dans le mode à 2 tours par mois ?
- Mois de départ d'une partie ; seuils et chiffres des effets sur les déplacements.
