# CivBuilder — consignes pour Claude

Jeu 4X au tour par tour en Rust (édition 2024) avec Bevy 0.20, sur une planète en cases hexagonales
de 40 km (192 × 111 par défaut). L'objectif est un 4X de **développement d'une civilisation**
plutôt que de conquête. Développé par Benjamin, seul, pour l'instant en solo (pas d'IA adverse).

## Langue et échanges
- **Répondre en français.** Code, commentaires, doc, wiki, CHANGELOG, messages de commit : en français.
- Benjamin décide des choix de jeu. Pour une question de conception, proposer (avec recherche si
  utile) et attendre sa décision avant de coder. Quand il demande « juste une analyse », ne rien modifier.
- En fin de tâche : un résumé court de ce qui a changé et une **proposition de message de commit**.

## Commandes
- **Compiler vite** :
  - Nouveau worktree : avant la première compilation, lancer `powershell -ExecutionPolicy Bypass -File scripts/amorcer-target.ps1`
    (copie le `target` d'un worktree voisin : ≈ 2 min au lieu de ≈ 6 min pour le premier build).
  - Pendant le développement : `cargo check --no-default-features` (≈ 4 s après une modification, contre ≈ 20 s pour
    un build) ; ne compiler que pour lancer le jeu ou des tests.
  - Toujours passer `--no-default-features`, y compris à `cargo run` : l'alterner avec les options par défaut
    recompile 15 crates (≈ 2 min).
  - Ne pas définir `RUSTC_BOOTSTRAP` dans un `target` à garder : le changer recompile la plupart des dépendances.
- Lancer le jeu : `cargo run --no-default-features` (graine fixe : `CIVBUILDER_SEED=777` ; autre taille : `CIVBUILDER_MAP=128x74`).
- **Captures d'écran** (vérification visuelle sans jouer) : `CIVBUILDER_SEED=777 CIVBUILDER_CAPTURE=<dossier> cargo run --no-default-features`
  → met en scène (deux villes dont une sur un relief accidenté, les douze types d'unités près d'une
  montagne, trois unités en barque sur un fleuve, carte révélée), passe en 3D, enregistre ~8 vues en PNG dans le dossier (ville, unités,
  vue lointaine, pic voisin d'une montagne, camp, village, carte des courants marins) puis se ferme (≈ 30 s après la génération). Lire les images pour
  vérifier un rendu ; ajouter des vues dans `src/capture.rs` si besoin. Le dossier de captures
  va dans le scratchpad, pas dans le dépôt. Avec `CIVBUILDER_CAPTURE_FEEDBACK=1` en plus : animations
  des combats et d'une visite de village (`feedback.rs`), vues à huit instants (la dernière, bouton de fin de tour rendu), en 2D puis en 3D.
- **GIF des actions** (`docs/ANIMATIONS.md`, liée depuis le README) : `scripts/animations.sh` (sans écran : sous
  `xvfb-run`, ffmpeg requis) rejoue chaque scène de `src/film.rs` avec une souris pilotée, en 2D puis en 3D, et
  remplace les GIF de `docs/animations` (`SCENES=tir,camp` pour quelques scènes, ≈ 10 min pour toutes en rendu logiciel).
- Tests : `cargo test --no-default-features` (suite complète ≈ 5-6 min : la génération des mondes
  est longue ; pendant le développement, lancer les tests du module touché, par ex.
  `cargo test --no-default-features seas::`).
- Avertissements : `cargo clippy --no-default-features --all-targets -- -D warnings` doit être propre.
- **Ne pas lancer `cargo fmt`** : le code n'est pas formaté par rustfmt (lignes longues voulues).
- `--no-default-features` retire seulement Wayland (Linux) ; sous Windows le jeu est le même, mais Cargo compile à part les crates concernées.
- Bilans chiffrés (tests ignorés, à lancer à la main avec `-- --ignored --nocapture`) :
  `bilan_tectonique` (terres, montagnes, pics, lacs sur les graines 777 / 2 / 5),
  `bilan_fond_marin`, `vue_des_mers` (`SEAS_SEED`, `SEAS_DUMP` : vue des étendues marines),
  `formes_des_terres` (`FORMES_ASCII` : carte en caractères), `bilan_noms`, `bilan_terres` (continents, îles, archipels, péninsules, isthmes), `bilan_relief` (chaînes, massifs, cols, plateaux, collines), `bilan_regions` (régions naturelles et sous-régions), `bilan_eau`, `bilan_courants` (`CURRENTS_DUMP` : courants exportés), `bilan_marees` (marnage en pleine mer, sur le plateau et aux côtes, courants de marée, plus forts marnages ; `SEEDS=777,2`), `bilan_montagnes` (montagnes par coût, traversées de massif), `bilan_verifier` (incohérences de cartes générées, doit être 0), `bilan_aridite` (pluie, ETP, humidité de l'air, biomes et altitude par bande de latitude, côtes est et ouest par saison, bilan d'eau de la planète ; `SEEDS=777,2`), `empreinte_generation` (empreinte
  de deux cartes : à comparer avant et après un remaniement de la génération), `generation_planete`.
- Mode debug (carte révélée) : menu Échap → Paramètres, bouton « Mode debug ». Wiki en jeu : F1.
- **Scripts de commandes** (préparer un cas à tester sans toucher au code) :
  `CIVBUILDER_SEED=777 CIVBUILDER_SCRIPT=<scratchpad>/cas.txt cargo run --release --no-default-features` exécute les commandes
  du fichier (console de debug, `debug_cmd.rs`), écrit les réponses dans `<scratchpad>/cas.reponses.txt`
  et les captures à côté du script. Commencer par `trouver …` (coordonnées colonne ligne), puis
  `aller`, `vue 3d`, `camera 300`, `carte <nom>`, `geographie terres|mers|relief|regions`, `capture nom`, modifications brutes (`terrain`, `relief`,
  `altitude`, `temperature`, `pluie`, `pinceau`, `annuler` ; `lignes-maree oui|non`, `fleches-maree oui|non` (carte des marées) ; `travelling col lig distance` pour mesurer la fluidité (durées d'image en se déplaçant avec zoom) ; `~` devant pour un recalcul local :
  température, biome, pics), `verifier` (incohérences ; 0 sur une carte générée), `regenerer eau|climat`
  (l'eau ou toute la simulation refaite comme à la génération), `fleuve`, `unite creer|pv|pm|dette|soigner|supprimer`, `ville`, `tour n`,
  `chemin`, et finir par `quitter`. `aide` et
  `aide <commande>` donnent la liste et les valeurs possibles. Ajouter une commande : `SPECS` et
  `parse` dans `debug_cmd.rs` (l'aide, la complétion et le wiki suivent) ; une commande qui agit
  sur un objet du jeu commence par son nom (`unite pv`, famille complétée par Tab).

## Conventions du code
- Modules de **logique pure** testés (génération : `plates.rs`, `erosion.rs`, `climate.rs`,
  `hydro.rs`, `basins.rs`, `seas.rs`, `toponymy.rs`…) séparés du rendu (`render.rs` 2D,
  `view3d.rs` / `relief.rs` 3D, `labels.rs`, `ui.rs`).
- **Génération déterministe** : même graine, même monde. Tout tirage passe par un générateur
  initialisé par la graine.
- **La carte boucle d'est en ouest**, les pôles sont ses bords. Règle d'or : toute case calculée
  géométriquement (`hex + direction`) est ramenée dans la carte (`canonical`) avant d'être
  cherchée dans une table (brouillard, occupation, territoire…). Voir `docs/conception/conception-planete.md`.
- Unités réelles partout où c'est possible (m, km, °C, mm/an, m³/s, cm/an).
- Commentaires et noms de tests en français ; une ligne de doc par fonction publique.

## Principes de jeu et d'interface (demandés par Benjamin)
- **Ne pas surcharger la carte** : informations secondaires dans l'infobulle, les cartes
  thématiques ou au survol ; les noms n'apparaissent qu'à leur niveau de zoom.
- **Ne pas masquer la console** dans les versions distribuées : ceux qui jouent sont aussi des
  développeurs et y lisent ce qui se passe.
- Montrer ce qui a un effet pour le joueur (ex. profondeur de la mer : seulement ce qui compte pour
  la pêche et la navigation).

## À chaque fonctionnalité livrée
1. Tests et clippy propres.
2. Vérification à l'écran quand c'est visuel (lancer le jeu, graines 777, 4242, 2…).
3. Mettre à jour : `CHANGELOG.md` (section de la version en cours), `README.md` (description
   technique), le wiki en jeu (`src/wiki.rs`) et la doc de conception concernée dans
   `docs/conception/` (décisions de Benjamin, algorithme, chiffres des bilans, limites, suite).
4. Proposer le message de commit.

## Où trouver quoi
- `docs/conception/` : une doc par grand sujet (climat, hydrologie, biomes, altitude, plaques,
  érosion, planète, fonds marins, toponymie, interface, options de carte, combat). À lire avant
  de toucher au sujet.
- `docs/TODO.md` : les prochaines tâches décidées avec Benjamin.
