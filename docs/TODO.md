# CivBuilder — prochaines tâches

Décidées avec Benjamin (fin octobre 2026), dans l'ordre conseillé.

## 1. Plaques océaniques polaires et passage maritime — fait et validé (30/10)
Voir `conception/conception-plaques.md`.

## 2. Mers fermées traitées comme les golfes — fait (30/10)
Voir `conception/conception-toponymie.md` (mers fermées : fermeture + taille).

## 3. Seuils sous-marins — fait (30/10)
Voir `conception/conception-toponymie.md`. Constat : sur 11 graines, aucune mer fermée (les grandes
poches sont des entonnoirs ouverts) ; à voir avec Benjamin s'il faut en provoquer à la génération.

## 4. Unités, villes, camps, villages enfouis dans le relief (3D)
Ils sont parfois « ensevelis » sous le relief. Hypothèse (à confirmer avec une capture) : leur
hauteur vient de l'altitude moyenne de la case, alors que le sol dessiné a des bosses, pentes et
sommets. Les poser sur la vraie surface 3D (lire le relief à leur position), les replacer quand le
relief de la case est recalculé, aplanir ou prendre le point haut sous une ville ; idem icônes et
barres de vie.

## 5. Courants marins
Version simplifiée : circulation de surface poussée par les vents simulés (tourbillons
subtropicaux et subpolaires, courants équatoriaux, renforcement à l'ouest des océans, courant
circumpolaire si le passage polaire existe). Effets :
- **température de la mer** calculée (remplace les règles fixes du climat : côtes ouest froides,
  côtes est chaudes…), donc température et **humidité** des côtes (déserts côtiers sur les
  courants froids, douceur et pluies sur les courants chauds, brouillards) ;
- **pêche** : rencontre d'un courant froid et d'un courant chaud ;
- plus tard : **navigation** (bonus/malus selon le courant), dérive de la banquise.
Après les plaques polaires (la forme des océans en dépend).

## Plus tard
- Banquise jouable (eau gelée, selon la température de la mer).
- Toponymie étapes 3 à 6 : continents, îles, archipels, péninsules, isthmes ; chaînes, massifs,
  cols ; régions naturelles ; barre de lieu (voir `conception/conception-toponymie.md`).
- Navigation et unités navales (barques, galères, caravelles), pêcheries.
- Police Inter Italic (les noms marins devraient être en italique).
