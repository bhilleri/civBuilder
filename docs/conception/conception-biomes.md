# CivBuilder — Conception des biomes (version 0.2.0, en cours)

Implémenté dans `src/climate.rs` (valeurs par saison), `src/map.rs` (`Terrain`, `biome()`, marais), `src/combat.rs` (couvert boisé), `src/view3d.rs` (décor 3D), `src/wiki.rs`.

## Choix de Benjamin (24/10/2026)
- Affiner les biomes avec les saisons ; **8 biomes** : jungle, savane, **savane arborée** (« savane avec arbres »), broussailles (climat méditerranéen), forêt tempérée, taïga, steppe, marais.
- Rendements proposés par Claude (Benjamin n'a pas fixé d'équilibre) : à revoir en jeu.
- **Broussailles seulement entre 0,35 et 0,75 d'aridité en été sec** ; au-delà, **forêt tempérée** (pas de biome « forêt méditerranéenne » à part).
- Nom **« Broussailles »** plutôt que « Maquis » (mot jugé peu courant ; « garrigue » écarté pour la même raison). Le climat « à été sec » reste affiché dans l'infobulle. Accord : « Broussailles vallonnées ».

## Saisons dans le climat
- `Climate::season_temp`, `season_rain`, `season_pet` : `[f32; 4]` par case, pour les 4 moments de `Period::YEAR` (équinoxe, été boréal, équinoxe, hiver boréal) ; pluie et ETP au rythme annuel (mm/an).
- `coldest_warmest(i)`, `dry_seasons(i)` (pluie < `DRY_SEASON_RATIO` = 0,3 × ETP de la saison), `dry_summer(i)` (saison la plus chaude sèche et pluie < `SUMMER_DRY_SHARE` = 0,4 × celle de la saison la plus froide).
- Constat (graine 777) : écarts entre saisons plus faibles que sur Terre (vers 41° : saisons de −6 à +7 °C), et pluies tropicales très concentrées sur une saison. D'où des seuils adaptés : toundra toujours par la moyenne annuelle, jungle avec au plus 1 saison sèche ou très arrosée.

## Règles (`map::biome`, dans l'ordre)
1. Moyenne < −15 °C : neige ; < −5 °C : toundra.
2. Aridité < 0,2 et moyenne ≥ 0 °C : désert.
3. Aridité < 0,5 : saison froide < 0 °C → steppe ; tropical (saison froide ≥ 18 °C) et aridité ≥ 0,35 → savane ; été sec et aridité ≥ 0,35 → broussailles ; sinon plaine.
4. Saison froide < −3 °C : taïga si aridité ≥ 0,6, sinon steppe.
5. Tropical : aridité < 0,75 → savane ; au plus 1 saison sèche ou aridité ≥ 2 → jungle ; sinon savane arborée.
6. Été sec et aridité < 0,75 → broussailles.
7. Aridité < 0,75 → prairie ; sinon forêt tempérée.
- **Marais** (après l'hydrologie, à chaque passe) : terre hors montagne, neige, toundra, désert ; sans collines ; pas un fleuve ; niveau ≤ 2 ; aucune voisine de terre plus basse ; au bord d'un fleuve ou d'un lac (ou petit lac) ; aridité ≥ 1 et moyenne > 0 °C.
- Petite variation locale de l'aridité (bruit ±25 %) partout.
- **Seuil des arbres selon le climat** : forêt tempérée, jungle et savane arborée à partir de 0,75 ; taïga dès 0,6 (écart discuté avec Benjamin, conservé pour l'instant : l'indice d'aridité tient déjà compte de la faible évaporation du froid) ; broussailles (arbres épars) de 0,35 à 0,75 en été sec.

## Répartition (terres, 192 × 111, avant la règle « été sec humide → forêt »)
| Terrain | Graine 777 | Graine 2 |
|---|---|---|
| Désert | 16,8 % | 20,1 % |
| Forêt tempérée | 16,1 % | 14,0 % |
| Toundra | 14,8 % | 14,2 % |
| Taïga | 13,0 % | 13,5 % |
| Plaine | 10,4 % | 10,1 % |
| Broussailles | 4,7 % | 2,7 % |
| Steppe | 4,5 % | 2,8 % |
| Prairie | 4,3 % | 3,9 % |
| Jungle | 2,4 % | 2,8 % |
| Savane arborée | 2,4 % | 2,5 % |
| Savane | 2,2 % | 2,5 % |
| Neige | 1,7 % | 5,4 % |
| Marais | 1,1 % | 0,8 % |
| Montagnes | 5,4 % | 4,6 % |

## Jeu
| Terrain | Nourriture / production / or | Déplacement | Particularités |
|---|---|---|---|
| Jungle | 1 / 1 / 0 | 2 PM | couvert boisé |
| Taïga | 1 / 1 / 0 | 2 PM | couvert boisé |
| Forêt tempérée | 1 / 2 / 0 | 2 PM | couvert boisé |
| Savane | 2 / 0 / 0 | 1 PM | départ possible |
| Savane arborée | 1 / 1 / 0 | 1 PM | |
| Broussailles | 1 / 1 / 1 | 1 PM | départ possible |
| Steppe | 1 / 1 / 0 | 1 PM | départ possible |
| Marais | 1 / 0 / 0 | 3 PM | pas de charge |
- **Couvert boisé** (`Terrain::is_wooded`) : +défense, pas de charge, tir gêné (portée −1).
- Départ du joueur : prairie, plaine, savane, steppe ou broussailles.
- Infobulle : climat « à été sec » ou « à N saisons sèches ».

## Rendu
- 2D : couleur propre à chaque terrain (marais bleu-vert clair, taïga vert sombre bleuté pour bien les distinguer).
- 3D : forêt tempérée = feuillus (tronc + houppier rond) ; jungle = grands feuillus sombres, deux couronnes ; taïga = conifères étroits, deux couronnes ; savane arborée = 3 acacias (cime plate), savane = 1 acacia ; broussailles = buissons et un olivier sur trois ; marais = flaques et touffes de roseaux. Végétation masquée sur les cartes thématiques ; pas d'arbres dans le lit d'un fleuve.

## À faire ensuite
- Décider du seuil de la taïga (0,6 ou 0,75 comme les autres forêts).
- Équilibrer les rendements en jeu ; ressources propres (chevaux en steppe, bois précieux en jungle, vigne et olivier en broussailles, fourrures en taïga).
- Marais : maladies, assèchement par les villes.
- Glyphes 2D pour distinguer les biomes boisés et le marais sans la couleur seule.
- Recaler le climat (écarts saisonniers plus marqués) : les seuils pourraient alors se rapprocher de ceux de Köppen.
