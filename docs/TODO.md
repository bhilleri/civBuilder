# CivBuilder — prochaines tâches

Décidées avec Benjamin (fin octobre 2026), dans l'ordre conseillé.

## 1. Plaques océaniques polaires et passage maritime
Benjamin n'est pas satisfait du débordement des plaques au-delà des bords (marges cachées de
20 lignes, voir `conception/conception-plaques.md`). À la place :
- une ou plusieurs **plaques océaniques forcées** le long du bord nord et du bord sud (germes près
  des bords, croissance d'abord le long du bord), le reste des plaques réparti entre les deux ;
- but : plus de masses de terre collées aux bords, des mers polaires **navigables**, et **au moins
  un passage maritime qui fait le tour de la carte** (test : depuis une case de mer, faire le tour
  complet par la mer ; si une chaîne d'îles ferme le passage, raboter le relief) ;
- frontières réelles avec les plaques voisines (arcs d'îles, fosses, dorsales, chaînes côtières à
  haute latitude) ;
- retirer les marges cachées et le dosage « partie visible » ; garder le bord comme déversoir de
  l'eau ; revérifier la part des terres (~40 %) avec `bilan_tectonique`.

## 2. Mers fermées traitées comme les golfes (cas « mer Baltique »)
Constat (capture de la « Mer des Glaces ») : la partie sud de la mer n'était pas couverte et sa
sortie était bizarre (limite en dents de scie, doigt d'une case vers l'ouest). Causes : les mers
fermées viennent du découpage par distance à la côte (`seas.rs`, étape 2), qui ne connaît ni
l'entrée ni la ligne des caps ; la partie sud, atteinte par le chenal, était rattachée à l'océan et
son golfe rangé dans l'océan.
- Ne garder le découpage par distance à la côte que pour séparer les océans entre eux ; toutes les
  mers (fermées comprises) par la méthode des poches à l'abri (cercle venu du large, ligne des
  caps), avec les golfes dedans (golfe › mer › océan).
- Supprimer la règle « mer polaire » (le bord n'est plus une côte).
- Lisser aussi les mers fermées (pointes, doigts, trous).

## 3. Seuils sous-marins
Utiliser la profondeur pour délimiter les mers : un passage peu profond (plateau) entre deux bassins
profonds compte comme une demi-fermeture (Gibraltar pour la Méditerranée, détroits danois pour la
Baltique) ; préférer la ligne du seuil comme limite de la mer.

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
