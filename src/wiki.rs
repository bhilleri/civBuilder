//! Wiki du jeu : contenu (construit à partir des données et des règles, il reste donc à jour
//! quand on change un chiffre) et fenêtre d'affichage (bouton « Wiki » ou touche F1).

use crate::pointer::UiPointer;
use crate::combat::{self, CHARGE_BONUS, FLANK_BONUS, HIGHER_GROUND_DEFENSE, HIGH_GROUND_PERCENT, TERRAIN_DEFENSE};
use crate::hostile::{CAMP_BUTIN_DENIERS, CAMP_MAX_UNITS, CAMP_SPAWN_INTERVAL, HUNT_RANGE, WANDER_RADIUS, WOUNDED_HP};
use crate::map::{
    CLIMB_GENTLE_M, CLIMB_STEEP_M, HILL_MOVE_COST, HILL_PRODUCTION, MOUNTAIN_MIN_M, MOUNTAIN_MOVE_MAX, MOUNTAIN_MOVE_MIN, MOUNTAIN_STEP_M,
    MOUNTAIN_YIELDS, RELIEF_MOVE_CAP, RUGGED_MOVE_COST, Relief, Terrain, UNKNOWN_MOVE_COST,
};
use crate::rules::{DETOUR_TOLERANCE_PERCENT, HABITANTS_COLON, MAX_HP, MIN_CITY_DISTANCE, habitants_du_palier};
use crate::sites::{CAMP_MIN_FROM_START, LAND_PER_CAMP, LAND_PER_VILLAGE, VILLAGE_MIN_FROM_START};
use crate::units::{UnitClass, UnitKind};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

// ---------------------------------------------------------------- Contenu

/// Petite illustration devant un titre ou une ligne de tableau.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Icon {
    /// Image blanche (dossier assets/icons), posée telle quelle.
    Image(&'static str),
    /// Icône d'unité, sur une pastille bleue comme les jetons de la carte.
    Unit(&'static str),
    /// Pastille de couleur (sRGB) : terrain, niveau d'altitude.
    Swatch(f32, f32, f32),
}

/// Élément d'un article.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(String, Option<Icon>),
    Para(String),
    /// Tableau : en-têtes, lignes, et icône éventuelle devant chaque ligne.
    Table(Vec<String>, Vec<Vec<String>>, Vec<Option<Icon>>),
}

/// Une rubrique du wiki.
#[derive(Clone, Debug)]
pub struct Article {
    pub title: &'static str,
    pub icon: &'static str,
    pub blocks: Vec<Block>,
}

fn h(s: &str) -> Block {
    Block::Heading(s.to_string(), None)
}
fn hi(s: &str, icon: Icon) -> Block {
    Block::Heading(s.to_string(), Some(icon))
}
fn p(s: impl Into<String>) -> Block {
    Block::Para(s.into())
}
fn table(header: &[&str], rows: Vec<Vec<String>>) -> Block {
    let icons = vec![None; rows.len()];
    Block::Table(header.iter().map(|s| s.to_string()).collect(), rows, icons)
}
fn table_icons(header: &[&str], rows: Vec<(Icon, Vec<String>)>) -> Block {
    let (icons, rows): (Vec<_>, Vec<_>) = rows.into_iter().map(|(i, r)| (Some(i), r)).unzip();
    Block::Table(header.iter().map(|s| s.to_string()).collect(), rows, icons)
}

fn swatch((r, g, b): (f32, f32, f32)) -> Icon {
    Icon::Swatch(r, g, b)
}

/// Toutes les rubriques, dans l'ordre du menu.
pub fn articles() -> Vec<Article> {
    vec![terrain(), climate(), waters(), units(), mechanics(), cities(), villages(), barbarians()]
}

/// Rubrique de la console, montrée seulement en mode debug ; construite à partir de la liste des
/// commandes (elle reste à jour quand on en ajoute une).
pub fn debug_article() -> Article {
    use crate::debug_cmd::{Group, SPECS};
    let mut blocks = vec![
        h("Ouvrir la console"),
        p("En mode debug (menu Échap → Paramètres), la touche ² (à gauche du 1) ouvre une ligne de commande sous le bandeau du mode debug ; ² ou Échap la referme. Tab complète le mot en cours (commande, terrain, relief, carte, critère) et montre les propositions ; ↑ ↓ rappellent les commandes précédentes ; la ligne du bas rappelle la forme de la commande tapée. « aide » liste les commandes, « aide <commande> » donne le détail et les valeurs possibles."),
        p("Les cases sont données par leur colonne (0 à l'ouest, la colonne fait le tour de la planète) puis leur ligne (0 au nord). En mode debug, l'infobulle commence par les coordonnées de la case survolée : [colonne ligne]."),
        h("Modifications brutes ou locales"),
        p("Brute (par défaut), une modification ne change que le champ demandé : le biome ne suit pas le climat, les cours d'eau et le climat des voisines ne sont pas recalculés. C'est voulu : on prépare vite un cas à tester (une forêt en montagne près d'un pic, un désert glacé…), même impossible dans un monde généré. Le rendu, lui, suit : la case et ses voisines sont redessinées en 2D et en 3D (relief partagé, arêtes entre sommets, décor, eau). « pinceau » étend les modifications aux cases proches, « annuler » revient en arrière."),
        p("Précédée de ~ (« ~altitude 52 37 3000 »), une modification est locale : ce qui en découle directement est recalculé. L'altitude change la température (-6,5 °C par 1 000 m) et le relief, recalculé comme à la génération pour la case et ses voisines : une montagne naît surtout de la dénivelée avec ses voisines, et de l'altitude (au moins 1 500 m ; plus bas, le relief est accidenté) ; un pic est un sommet local de montagne d'au moins 2 400 m. Un plateau relevé d'un bloc garde donc un dessus plat et des bords escarpés. Température et pluie changent le biome. Imposer une montagne ou un pic relève la case à 1 500 ou 2 400 m, et un pic voisin n'en est plus un. Au pinceau, « ~relief … pic » pose un pic au centre et des montagnes autour. Les cours d'eau ne suivent pas : « verifier » signale ce qui ne va plus (rivière qui remonte, cuvette sans lac, biome que le climat ne donne pas, pics côte à côte, montagne trop basse, lac à deux niveaux). Une carte générée n'en a aucune."),
        h("Refaire la simulation"),
        p("« regenerer eau » refait les cours d'eau, les lacs, les marais et les noms avec le climat actuel : un barrage posé sur un fleuve devient un lac, ou le fleuve le contourne. « regenerer climat » refait tout depuis les altitudes, les terres et les mers actuelles : courants marins, climat, biomes, eau, noms, eaux poissonneuses (une dizaine de secondes sur une planète). C'est le même calcul qu'à la génération : sur une carte intacte, il la redonne à l'identique. Les biomes et le climat modifiés à la main sont remplacés ; les altitudes, les reliefs imposés et les terres ou mers posées restent. « fleuve » trace à la main un fleuve de case en case, sans bassin ni nom."),
        h("Partie"),
        p("Les commandes d'unité commencent toutes par « unite » (Tab propose la suite) : « unite creer » (du joueur, ou hostile), « unite supprimer », « unite pv » (0 : dispersée), « unite pm », « unite dette » (PM dus, comme après une montagne) et « unite soigner » préparent un combat ou un passage difficile ; « info » donne aussi l'unité de la case et son état. « ville » et « tour » complètent la situation (tours joués avec leurs animations) ; « chemin » donne le trajet le plus rapide d'une unité, le coût de chaque pas et le tour d'arrivée, sur la carte entière."),
    ];
    for g in Group::ALL {
        blocks.push(h(g.title()));
        blocks.push(table(&["Commande", "Effet"], SPECS.iter().filter(|c| c.group == g).map(|c| vec![c.usage(), c.help.to_string()]).collect()));
    }
    blocks.push(h("Scripts (agents IA, cas à rejouer)"));
    blocks.push(p("CIVBUILDER_SCRIPT=<fichier> cargo run exécute les commandes du fichier, une par image, en mode debug (lignes vides et lignes commençant par # ignorées). Chaque commande et sa réponse sont écrites dans CIVBUILDER_SCRIPT_OUT (sinon <script>.reponses.txt à côté du script) et dans la console du terminal. Une réponse commence par OK ou ERREUR, suivie de lignes clé=valeur. « capture » attend que le relief 3D soit construit puis enregistre l'écran, sans la console ; « quitter » ferme le jeu. « journal <fichier> » écrit les commandes de la séance en script, avec la graine et la taille de la carte en tête : un cas trouvé à la main se rejoue tel quel."));
    blocks.push(p("Exemple : trouver relief=montagne terrain=foret voisin=pic max=1 · aller 52 37 · vue 3d · camera 300 · capture montagne · terrain 52 37 neige · capture montagne_neige · quitter"));
    blocks.push(h("Mode joueur pilotable (agents de test)"));
    blocks.push(p("scripts/pilote.sh lancer <dossier> 777 lance une partie normale, sans mode debug, qui suit le fichier d'ordres CIVBUILDER_PILOTE pendant la partie ; scripts/pilote.sh <dossier> \"etat\" \"clic 66 39\" \"touche b\" envoie des ordres et rend les réponses. Ordres : etat, alentours, survol, clic, clic-droit, touche, interface, bouton, clic-ecran, zoom, et les commandes de caméra, de carte, de capture et de fin de tour (« aide » les liste). Les réponses ne donnent que ce que le joueur peut savoir. CIVBUILDER_FENETRE=1920x1080 règle la taille de la fenêtre."));
    blocks.push(h("GIF des actions"));
    blocks.push(p("scripts/animations.sh (ou CIVBUILDER_CAPTURE=docs/animations CIVBUILDER_CAPTURE_GIF=1 cargo run --release) joue chaque action — déplacement, trajet de plusieurs tours, échange, corps à corps, charge, tir, attaque perdue, attaque hostile en fin de tour, village (commerce, habitants, embuscade), camp — avec une souris pilotée, en 2D puis en 3D, et en fait des GIF avec ffmpeg (page docs/ANIMATIONS.md du dépôt). CIVBUILDER_CAPTURE_GIF=tir,camp limite aux scènes nommées."));
    Article { title: "Console de debug", icon: "icons/wiki_mechanics.png", blocks }
}

fn waters() -> Article {
    use crate::hydro::{FLEUVE_M3S, RIVER_M3S, SMALL_LAKE_KM2};
    use crate::map::{BRIDGE_MOVE_COST, FLEUVE_MOVE_COST, FLEUVE_YIELDS, FRESH_WATER_FOOD, RIVER_CROSSING_COST};
    use crate::combat::{
        BRIDGE_ARCHER_DEFENSE, BRIDGE_EXIT_ATTACK, BRIDGE_EXIT_CAVALRY_VS_SPEARS, BRIDGE_EXPOSED_DEFENSE, BRIDGE_HOLD_DEFENSE, BRIDGE_SHOT_ATTACK,
        BRIDGE_SPEAR_DEFENSE, RIVER_DEFENSE,
    };
    let water = |c: (f32, f32, f32)| swatch(c);
    let kinds = vec![
        (
            water((0.20, 0.45, 0.80)),
            vec![
                "Rivière".into(),
                format!("{RIVER_M3S:.0} à {FLEUVE_M3S:.0} m³/s"),
                "Sur l'arête entre deux cases : on voit de quel côté sont les unités".into(),
            ],
        ),
        (
            water((0.18, 0.40, 0.72)),
            vec![
                "Fleuve".into(),
                format!("{FLEUVE_M3S:.0} m³/s et plus"),
                "Occupe toute la case : une large bande d'eau entre deux berges".into(),
            ],
        ),
        (
            water((0.24, 0.50, 0.78)),
            vec!["Petit lac".into(), format!("{SMALL_LAKE_KM2:.0} km² à une demi-case"), "Élément d'une case de terre".into()],
        ),
        (
            swatch(Terrain::Lake.color()),
            vec!["Lac".into(), "une demi-case et plus".into(), "Cases d'eau, à toute altitude".into()],
        ),
    ];
    Article {
        title: "Cours d'eau et lacs",
        icon: "icons/wiki_water.png",
        blocks: vec![
            p("Les cours d'eau naissent du climat : la pluie qui n'est ni évaporée ni retenue par le sol ruisselle et \
               descend le relief jusqu'à la mer. Chaque case envoie son eau vers sa voisine la plus basse ; les débits \
               s'additionnent d'amont en aval. Les débits sont réels (en mètres cubes par seconde) : une rivière \
               comme la Charente débite environ 100 m³/s, le Rhône 1 700 m³/s, le Congo 41 000 m³/s."),
            h("Rivières, fleuves et lacs"),
            table_icons(&["Type", "Taille", "Sur la carte"], kinds),
            p("Sur la carte, un cours d'eau s'élargit avec son débit, et les plus grands sont d'un bleu plus \
               sombre. Les fleuves serpentent, d'autant plus en plaine, et s'y partagent par endroits en deux \
               bras autour d'une île, parfois de plusieurs côte à côte (plus souvent pour les grands fleuves) ; \
               les grandes îles ont des arbres, les petites sont des bancs de sable. La couleur de l'eau suit les terres \
               traversées : chargée de limon (olive) dans les déserts, les steppes et les savanes, claire dans la \
               taïga, la toundra et les hautes terres. Un lac garde le limon : l'eau en sort claire. Là où un cours \
               d'eau descend une forte pente, des rapides : eau vive, écume et rochers."),
            h("Les lacs"),
            p("Une cuvette sans issue recueille l'eau de son bassin, et la surface du lac en évapore (d'autant plus \
               qu'il fait chaud et sec). Si les apports dépassent ce que la cuvette pleine évapore, le lac se remplit \
               jusqu'au col le plus bas et déborde : une rivière ou un fleuve en repart. Sinon, il s'arrête à la taille \
               où évaporation et apports s'équilibrent : c'est un lac sans issue, comme la mer Morte ou le lac Tchad, \
               et les rivières qui s'y jettent n'en ressortent pas. Les grands lacs peuvent se trouver en altitude, \
               dans les bassins entourés de montagnes."),
            h("Climat sec"),
            p("Dans les régions arides, le lit des rivières perd de l'eau en s'infiltrant dans le sol. Cette perte \
               augmente avec la largeur du lit, donc beaucoup moins vite que le débit : les petits cours d'eau se \
               perdent dans le désert (oueds), les grands fleuves le traversent en s'affaiblissant, comme le Nil."),
            h("Bassins versants et noms"),
            p("Un bassin versant réunit toutes les cases dont l'eau aboutit au même endroit : l'embouchure d'un \
               fleuve, une petite côte, ou un lac sans issue (bassin fermé). Les lignes de partage des eaux sont les \
               limites entre bassins. La carte des bassins versants donne une couleur à chaque bassin (couleurs vives \
               pour les grands, teintes grises pour les petits) ; la carte des débits colore chaque case selon l'eau \
               qui la traverse, des sources aux embouchures. Une case hachurée dessert deux bassins : la moitié de \
               ses coins envoie l'eau, par les rivières, vers le bassin dont les hachures ont la couleur. La carte du réseau hydrographique trace tous les cours d'eau \
               et lacs connus, colorés selon leur rang par rapport à leur déversoir : bleu nuit pour ceux qui se jettent \
               dans la mer (ou un lac salé, une mer intérieure), puis bleu, turquoise, vert et olive pour les affluents \
               de plus en plus lointains ; le principal cours qui entre dans un lac garde le rang du lac. Des chevrons \
               y montrent le sens du courant."),
            p(format!(
                "Les noms des lieux viennent de huit langues en partie inventées (nordique, latine, slave, turque, \
                 berbère, bantoue, nahua, polynésienne) : les mots n'existent pas, mais ils ont les sons de la vraie famille \
                 de langues, et parfois une vraie racine choisie d'après le lieu (Kaldvik : « froid » en nordique ; \
                 Kumseydag : « sable » en turc). Chaque région du monde parle sa langue, là où le climat lui ressemble \
                 (nordique au froid, berbère au chaud et sec…) ; l'infobulle donne l'origine d'un nom. Certains lieux ont un \
                 nom descriptif tiré de leur nature : Lac Gelé, Grand Lac Salé, Rivière Noire (jungle ou marais), Rivière \
                 Blanche (née dans la neige) ; un affluent presque aussi abondant que le fleuve qu'il rejoint en devient le \
                 jumeau (Vaska Blanche, Petit Taldor). Un nom ne sert qu'une fois. Chaque affluent porte son propre nom (un \
                 long affluent compte au moins {} cases de fleuve). Le nom apparaît sur la carte une fois une partie de son \
                 cours découverte, près de son milieu ; de loin, seuls les grands fleuves sont nommés (en plus gros), les \
                 longs affluents apparaissent en s'approchant, toutes les rivières en zoom fin, et un nom qui en \
                 chevaucherait un plus important est décalé le long du cours ou masqué. Les villages tribaux et les camps \
                 barbares ont aussi leur nom (Repaire des Loups, Antre des Scorpions…), affiché de près.",
                crate::basins::MIN_NAMED_CELLS
            )),
            p("Un fleuve garde son nom de l'embouchure à sa source, à travers les petits lacs d'une ou deux cases ; ses \
               petits bras portent le nom du cours où ils se jettent. Les étendues d'eau sont de trois natures : un lac \
               d'eau douce se déverse dans un cours d'eau ; un lac salé n'a pas d'issue, l'eau s'en évapore ; une mer \
               intérieure est une étendue de mer enclavée dans les terres. \
               Un fleuve presque entouré par un lac en fait partie, et un petit lac qu'un fleuve traverse porte le nom du \
               fleuve. Survoler un cours d'eau ou un \
               lac le trace en rouge, ses affluents en orange, les affluents de ses affluents (de proche en proche) en \
               jaune, le cours d'eau ou le lac où il se jette en bleu, et la suite du chemin de l'eau jusqu'à la mer en \
               violet ; \
               les tracés sont reliés aux confluents, et des chevrons qui descendent le courant en montrent le sens."),
            h("Effets en jeu"),
            p(format!(
                "Traverser une rivière coûte +{RIVER_CROSSING_COST} PM, ce qui arrête la plupart des unités sur \
                 l'autre rive. Entrer dans un fleuve (à gué ou en barque) coûte {FLEUVE_MOVE_COST} PM : l'unité \
                 s'y arrête et en ressort au tour suivant, sur la rive de son choix. En mêlée, attaquer à travers une \
                 rivière ou depuis un fleuve donne +{RIVER_DEFENSE} en défense à la cible."
            )),
            p(format!(
                "Une case de terre au bord d'une rivière, d'un fleuve ou d'un lac, ou qui a un petit lac, a de l'eau \
                 douce : +{FRESH_WATER_FOOD} nourriture. Un fleuve rapporte {} nourriture, {} production et {} deniers \
                 (pêche, transport) ; on ne fonde pas de ville dans son lit, mais sur une rive. Les villes pourront plus \
                 tard y aménager des moulins à eau.",
                FLEUVE_YIELDS.food, FLEUVE_YIELDS.production, FLEUVE_YIELDS.deniers
            )),
            h("Ponts"),
            p(format!(
                "Un pont franchit la rivière d'une arête, ou traverse la case d'un fleuve d'une rive à la rive opposée. \
                 Il sert à tout le monde, ennemis compris. Par le pont, une rivière se traverse sans surcoût ; on monte \
                 sur le pont d'un fleuve par un de ses bouts pour {BRIDGE_MOVE_COST} PM, et l'on peut s'y arrêter ou \
                 continuer. Entrer dans le fleuve par un autre côté, c'est entrer dans l'eau ({FLEUVE_MOVE_COST} PM). \
                 Les villes les construisent (voir « Villes », Production) ; la console en pose aussi (« pont »). \
                 Une case de fleuve avec pont a deux niveaux : une unité sur le pont, une autre dans l'eau. Un clic \
                 droit sur le tablier y envoie l'unité, ailleurs dans la case dans l'eau ; on ne monte sur le pont et \
                 on n'en descend que par ses deux bouts, et une barque passe sous un pont ennemi. Entre les deux niveaux, \
                 on ne fait que tirer : depuis le pont vers l'eau avec le bonus de hauteur."
            )),
            p(format!(
                "Sur le pont d'un fleuve, une unité attaquée en mêlée ne peut être ni flanquée ni chargée ; \
                 l'infanterie et les lanciers y tiennent la position (+{BRIDGE_HOLD_DEFENSE} en défense), les lanciers \
                 encore +{BRIDGE_SPEAR_DEFENSE} face à la cavalerie, et les archers +{BRIDGE_ARCHER_DEFENSE} face aux \
                 unités à pied sans bouclier (les assaillants n'arrivent qu'un par un). Contre les tirs, elle est \
                 exposée ({BRIDGE_EXPOSED_DEFENSE} en défense). Attaquer depuis le pont, ou à travers une rivière par \
                 son pont, c'est une sortie de pont : {BRIDGE_EXIT_ATTACK} en mêlée contre l'infanterie, les lanciers et \
                 l'archer-épéiste ({BRIDGE_EXIT_CAVALRY_VS_SPEARS} et pas de charge pour la cavalerie contre des \
                 lanciers), {BRIDGE_SHOT_ATTACK} pour tirer depuis le pont ; seuls les alliés déjà sur la rive, à côté \
                 du pont, flanquent la cible ; la cible n'a pas le bonus de la rivière à franchir."
            )),
        ],
    }
}

fn climate() -> Article {
    use crate::map::{
        ARID_BELOW, COLD_WINTER, DESERT_ABOVE, FOREST_ABOVE, JUNGLE_WET, SEMI_ARID_BELOW, SEMI_ARID_WOODLAND, SNOW_BELOW,
        TAIGA_ABOVE, TREE_LINE_SUMMER, TROPICAL_WINTER, TUNDRA_BELOW,
    };
    let rain = |mm: f32| swatch(crate::render::rain_color(mm));
    let row = |t: Terrain, temp: String, ar: String| (swatch(t.color()), vec![temp, ar, t.name().into()]);
    let biomes = vec![
        row(Terrain::Snow, format!("moyenne sous {SNOW_BELOW:.0} °C"), "—".into()),
        row(Terrain::Tundra, format!("moyenne sous {TUNDRA_BELOW:.0} °C"), "—".into()),
        row(Terrain::Desert, format!("moyenne de {DESERT_ABOVE:.0} °C et plus"), format!("sous {ARID_BELOW} (aride)")),
        row(Terrain::Steppe, "saison froide sous 0 °C".into(), format!("sous {SEMI_ARID_BELOW} (semi-aride)")),
        row(Terrain::Plains, "saison froide de 0 °C et plus".into(), format!("sous {SEMI_ARID_BELOW}")),
        row(Terrain::Taiga, format!("saison froide sous {COLD_WINTER:.0} °C"), format!("{TAIGA_ABOVE} et plus (sinon steppe)")),
        row(Terrain::Savanna, format!("saison froide de {TROPICAL_WINTER:.0} °C et plus"), format!("{SEMI_ARID_WOODLAND} à {FOREST_ABOVE}")),
        row(Terrain::WoodedSavanna, format!("tropical (≥ {TROPICAL_WINTER:.0} °C), 2 saisons sèches ou plus"), format!("{FOREST_ABOVE} et plus")),
        row(Terrain::Jungle, "tropical, au plus 1 saison sèche".into(), format!("{FOREST_ABOVE} et plus (ou {JUNGLE_WET} et plus)")),
        row(Terrain::Mediterranean, "été sec, hiver pluvieux".into(), format!("{SEMI_ARID_WOODLAND} à {FOREST_ABOVE} (au-delà : forêt)")),
        row(Terrain::Grassland, format!("tempéré (en montagne : été sous {TREE_LINE_SUMMER:.0} °C)"), format!("sous {FOREST_ABOVE} (en montagne : toute)")),
        row(Terrain::Forest, format!("tempéré, été de {TREE_LINE_SUMMER:.0} °C et plus"), format!("{FOREST_ABOVE} et plus (humide)")),
        row(Terrain::Marsh, "terre basse et plate au bord d'un fleuve ou d'un lac, ne gèle pas".into(), "1 et plus".into()),
    ];
    let belts = vec![
        (rain(2_000.0), vec!["Équateur".into(), "Très arrosé".into(), "Les alizés convergent, l'air chaud monte (orages)".into()]),
        (rain(900.0), vec!["0 à 30°".into(), "Côtes est arrosées".into(), "Alizés venant de l'est (nord-est au nord, sud-est au sud)".into()]),
        (rain(150.0), vec!["Vers 30°".into(), "Ceinture des déserts".into(), "Anticyclones : l'air descend et forme un couvercle".into()]),
        (rain(1_000.0), vec!["30 à 60°".into(), "Côtes ouest arrosées".into(), "Vents d'ouest, dépressions et leurs fronts".into()]),
        (rain(300.0), vec!["60° aux pôles".into(), "Froid et plutôt sec".into(), "Vents d'est polaires".into()]),
    ];
    Article {
        title: "Climat",
        icon: "icons/wiki_climate.png",
        blocks: vec![
            p(format!(
                "La carte représente toute une planète, plus petite que la Terre : une case mesure {:.0} km (assez \
                 pour une ville et ses abords) et la carte va d'un pôle (en haut) à l'autre (en bas), l'équateur au \
                 milieu. Elle couvre environ {:.0} km d'ouest en est et {:.0} km du nord au sud. Le climat de chaque case \
                 est simulé à la création du monde, avec des grandeurs physiques (degrés, millimètres de pluie), et fait \
                 le tour complet de la planète. La carte aussi fait le tour : en allant toujours vers l'est, on \
                 revient par l'ouest (seuls les pôles sont des bords). Le survol d'une case de terre indique son climat, sa température \
                 moyenne, sa pluie annuelle et sa latitude ; la carte du climat (touche T) montre les précipitations et \
                 les vents dominants.",
                crate::climate::CELL_KM,
                crate::game::MAP_WIDTH as f32 * crate::climate::CELL_KM,
                crate::game::MAP_HEIGHT as f32 * crate::climate::CELL_KM * 0.866,
            )),
            h("Température"),
            p("Elle dépend de la latitude (environ 27 °C à l'équateur, 13 °C vers 40°, 6 °C vers 50° au niveau de la \
               mer), puis baisse de 6,5 °C par kilomètre \
               d'altitude (les terres vont du niveau de la mer à près de 4 800 m). La \
               température de la mer vient des courants marins (voir plus bas), et les côtes en prennent une bonne \
               part : un courant froid rafraîchit et assèche sa côte, un courant chaud l'adoucit et l'arrose. L'écart \
               entre le jour et la nuit est faible près de la mer et grand dans les régions sèches."),
            h("Courants marins"),
            p("Le vent entraîne l'eau de surface. Comme la force due à la rotation de la planète grandit vers les \
               pôles, l'eau des océans tourne en grands tourbillons : dans le sens des aiguilles d'une montre entre \
               l'équateur et 45° au nord (en sens inverse au sud), en sens inverse plus près des pôles. Ces \
               tourbillons sont serrés contre la côte ouest des océans, où passent des courants étroits et rapides \
               (plus d'1 m/s) qui portent l'eau chaude vers les pôles ; le retour vers l'équateur, le long des côtes \
               ouest des continents, est lent et froid, d'autant plus que le vent y écarte l'eau de la côte et fait \
               remonter l'eau profonde. Près de l'équateur, l'eau suit les alizés vers l'ouest. Aux latitudes \
               moyennes, l'eau de surface dérive aussi vers le pôle, où elle refroidit et plonge : elle porte l'eau \
               chaude jusqu'aux côtes ouest des continents des hautes latitudes, bien plus douces que leurs côtes \
               est (comme l'Europe face au Labrador). Si la mer fait le tour d'un pôle, un courant circumpolaire \
               en fait le tour, poussé par les vents. Seuls les \
               grands continents arrêtent l'eau : les petites îles la laissent passer. La carte des courants marins \
               montre la température de la mer et le sens des courants (flèches rouges : courant chaud, bleu clair : \
               courant froid). Les courants ne changent pas au fil de l'année."),
            h("Marées"),
            p("La Lune (et un peu le Soleil) attire l'eau des océans : deux fois par jour, la mer monte et descend. \
               En plein océan, l'écart entre pleine et basse mer, le marnage, ne dépasse guère un mètre. Sur un \
               plateau continental, l'onde de marée ralentit et grandit ; dans un golfe de la bonne taille, elle \
               résonne et dépasse 8 à 10 m (comme dans la baie de Fundy ou la baie du Mont-Saint-Michel). Une mer \
               fermée reliée à l'océan par un détroit étroit et peu profond n'a presque pas de marée (comme la \
               Méditerranée ou la Baltique). Avec la rotation de la planète, la marée tourne autour de points où \
               elle est nulle. La carte des marées montre le marnage de vives-eaux (quand la Lune et le Soleil \
               s'ajoutent) ; l'infobulle d'une mer le donne, avec le courant de marée quand il est fort (détroits, \
               plateaux). Deux boutons de sa légende ajoutent les lignes d'heure de pleine mer (la marée avance \
               d'une ligne à la suivante, ronds blancs : points sans marée) et des flèches du sens de la marée."),
            h("Rivage"),
            p("En vue 3D, le rivage ondule et ses angles s'arrondissent, sur les côtes, les lacs et la banquise, pour ne pas suivre exactement le bord des cases. Ce n'est que du dessin : \
               une case reste de la terre ou de la mer tout entière, pour les déplacements comme pour les rendements."),
            h("Embouchures"),
            p("Un fleuve apporte des sédiments, la houle et la marée les reprennent, et la forme de l'embouchure dépend \
               de celui qui l'emporte. Si le fleuve domine, il bâtit un delta digité dont les bras s'avancent en mer ; \
               si c'est la houle, un delta arqué bordé de plages, ou une lagune derrière un cordon de sable quand le \
               fleuve apporte peu ; si c'est la marée, un delta de marée en eau chargée, ou un estuaire en eau claire. \
               Une mer profonde tout de suite ne laisse pas de place à un delta : c'est aussi un estuaire. Une vallée \
               de relief envahie par la mer forme une ria, une vallée glaciaire dans le froid un fjord. Sur la carte, \
               un estuaire s'ouvre en entonnoir, un delta digité se divise en plusieurs bras, un delta arqué forme un \
               lobe de terre sur la côte, et dans une ria ou un fjord la mer remonte la vallée. Un delta gagne sur la \
               mer quand le fond est peu profond : de la terre basse que le fleuve traverse. \
               L'infobulle d'une case d'embouchure donne son type."),
            h("Houle"),
            p("Le vent lève les vagues. Leur hauteur dépend de sa force, de la distance de mer sur laquelle il \
               souffle et de sa durée. Quand elles quittent le vent qui les a formées, elles deviennent de la houle : \
               les plus longues voyagent le plus vite et traversent des océans en perdant peu d'énergie. La houle \
               des vents d'ouest, levée vers 40 à 60° de latitude, arrive ainsi jusque sous les tropiques. Une île \
               laisse une zone plus calme derrière elle, un plateau continental use la houle longue, une mer fermée \
               reste plus calme que le large, et la banquise l'arrête. La carte de la houle \
               montre la hauteur moyenne de l'année, et des flèches indiquent vers où elle va, plus longues pour \
               une longue période. L'infobulle d'une mer donne la hauteur, la plus forte de l'année, la période, \
               d'où vient la houle et sa hauteur quand elle déferle sur la côte."),
            h("Banquise"),
            p(format!("Près des pôles, au-delà de 72 à 80° de latitude selon l'endroit, la mer reste gelée toute l'année. Les courants \
               chauds repoussent un peu sa limite, les courants froids la rapprochent. La banquise arrête la houle. \
               Les unités peuvent y marcher ({ice} PM par case, sauf au pied d'une côte escarpée), ce qui ouvre un \
               passage entre des terres que la mer sépare ailleurs. On ne peut pas y fonder de ville, et aucune \
               structure n'y prend place, mais on y pêche comme en mer libre.", ice = crate::map::ICE_MOVE_COST)),
            h("Vents"),
            p("Le vent naît des différences de pression : ceintures alternées de hautes et basses pressions, effet des \
               continents (une terre chaude attire l'air, une terre froide le repousse), dépressions et anticyclones \
               mobiles qui changent tous les dix jours simulés. L'air va des hautes vers les basses pressions en \
               étant fortement dévié par la rotation de la planète (vers la droite au nord, vers la gauche au sud). Près \
               de l'équateur, la rotation ne dévie presque plus l'air, mais les écarts de pression y restent faibles : \
               le vent y descend la pente de pression sans s'emballer. L'été, la terre chaude des continents tropicaux \
               creuse une dépression qui remonte plus loin vers le pôle que sur la mer : elle aspire l'air marin, \
               c'est la mousson (pluies d'été des savanes). L'hiver, l'air froid des continents qui sort sur une mer \
               chaude (au large des côtes est, vers 30-45°) y fait naître des dépressions ; en passant, elles ramènent \
               l'air marin sur la côte, qui garde des pluies d'hiver. En altitude, les vents soufflent surtout de l'ouest. Les grandes cellules de circulation font descendre \
               l'air vers 30° de latitude (anticyclones) et monter à l'équateur et vers 60°. En été, l'air descend \
               surtout à l'ouest des continents (déserts côtiers) et bien moins à leur est (côtes est arrosées). L'air qui converge \
               quelque part doit monter, celui qui diverge est remplacé par de l'air qui descend. Le vent ralentit sur les \
               terres, plus rugueuses : l'air qui arrive de la mer s'y accumule et monte (côtes au vent arrosées)."),
            table_icons(&["Latitude", "Effet", "Circulation"], belts),
            h("L'air en deux couches"),
            p("La couche basse (jusqu'à 2 km au-dessus du sol) suit le relief et porte l'essentiel de la vapeur ; la \
               couche haute est bien plus froide et peut en contenir beaucoup moins (la capacité de l'air augmente \
               d'environ 7 % par degré). Il pleut quand l'air humide monte vers le froid et sature : là où les vents \
               convergent, sous les fronts des perturbations, dans les orages des tropiques, ou quand le relief force \
               l'air à s'élever. Les nuages montent d'autant plus haut que l'air est chaud (vers 6 km dans le froid, jusqu'à 14 km dans les orages des tropiques) : l'air tropical soulevé y perd presque toute sa vapeur, et l'essentiel tombe en pluie. Une partie des gouttes s'évapore en tombant si l'air est sec. Là où l'air descend \
               (anticyclones, derrière les massifs), l'air sec d'altitude assèche la couche basse. Sous un \
               anticyclone, l'air chaud qui descend forme un couvercle : les orages sont étouffés et l'air qui monte \
               ne forme que des nuages bas. Un air bas saturé sans être soulevé (air marin poussé sur une côte froide) \
               forme du brouillard, qui bruine à peine. Les pluies les plus fortes sont plafonnées (vers 10 000 à 15 000 mm par an) : l'eau qui ne tombe pas continue avec le vent et arrose les terres suivantes."),
            h("Relief"),
            p("Sur un versant exposé au vent, l'air qui monte en suivant le sol se refroidit et la pluie augmente ; elle \
               culmine sur les pentes hautes. L'air épouse un relief adouci (il contourne en partie les sommets) ; \
               l'air d'altitude passe au-dessus des massifs, sauf des plus hauts (au-delà de 2 000 m), qui le soulèvent \
               à son tour. Une montagne reçoit ainsi une à deux fois la pluie des plaines voisines, bien plus sur son \
               versant au vent que sur l'autre. La neige, qui tombe lentement, est en partie portée au-delà de la crête. Derrière, l'air redescend en se \
               réchauffant : il est loin de la saturation et il pleut peu, c'est l'ombre pluviométrique. Plus loin, \
               les perturbations et l'évaporation des sols lui rendent peu à peu de l'humidité. Un haut plateau est \
               froid et sec."),
            h("Sols et évaporation"),
            p("Chaque case a une réserve d'eau dans le sol (250 mm au plus). La pluie la remplit, l'évaporation la vide, \
               d'autant moins que le sol est sec ; le trop-plein ruisselle et alimente les rivières. L'évaporation \
               possible dépend du soleil, de la température, de l'écart jour-nuit et de l'humidité de l'air. Environ \
               un tiers à la moitié de la pluie des continents revient ainsi dans l'air et retombe plus loin : c'est ce \
               qui permet à la pluie de pénétrer loin dans les terres. La mer s'évapore d'autant plus qu'elle est \
               chaude ; la nuit, l'air humide peut déposer de la rosée."),
            h("Saisons"),
            p("Le climat est la moyenne de quatre moments de l'année (équinoxes et solstices) : le soleil se déplace \
               jusqu'aux tropiques et la ceinture de pluies équatoriale le suit, ce qui arrose les savanes une partie de \
               l'année."),
            h("Terrain qui en résulte"),
            p("Le terrain dépend de la température, de l'indice d'aridité (la pluie de l'année divisée par \
               l'évaporation possible, seuils du Programme des Nations unies pour l'environnement) et des saisons : \
               température de la saison la plus froide, nombre de saisons sèches (pluie de la saison inférieure \
               à 30 % de son évaporation possible), été sec. Les règles s'appliquent dans l'ordre du tableau."),
            table_icons(&["Température, saisons", "Indice d'aridité", "Terrain"], biomes),
            p("Les montagnes et les pics portent eux aussi un terrain selon leur climat : souvent toundra ou neige \
               en altitude, où il fait plus froid."),
        ],
    }
}

fn terrain() -> Article {
    let rows = Terrain::ALL
        .iter()
        .map(|&t| {
            let y = t.yields();
            let moves = t.move_cost().map_or("infranchissable".to_string(), |c| format!("{c} PM"));
            let defense = if t.is_wooded() { format!("+{TERRAIN_DEFENSE}") } else { "—".into() };
            (swatch(t.color()), vec![t.name().into(), y.food.to_string(), y.production.to_string(), y.deniers.to_string(), moves, defense])
        })
        .collect();
    let reliefs = Relief::ALL
        .iter()
        .map(|&r| {
            let (moves, yields, defense) = match r {
                Relief::Flat => ("terrain".to_string(), "terrain".to_string(), "—".to_string()),
                Relief::Hills => (format!("+{HILL_MOVE_COST} PM"), format!("+{HILL_PRODUCTION} production"), format!("+{TERRAIN_DEFENSE}")),
                Relief::Rugged => (
                    format!("+{RUGGED_MOVE_COST} PM"),
                    format!("+{HILL_PRODUCTION} production, −1 nourriture"),
                    format!("+{TERRAIN_DEFENSE}"),
                ),
                Relief::Mountain => (
                    format!("{MOUNTAIN_MOVE_MIN} à {MOUNTAIN_MOVE_MAX} PM"),
                    format!("{} / {} / {}", MOUNTAIN_YIELDS.food, MOUNTAIN_YIELDS.production, MOUNTAIN_YIELDS.deniers),
                    format!("+{}", 2 * TERRAIN_DEFENSE),
                ),
                Relief::Peak => ("infranchissable".to_string(), "rien".to_string(), "—".to_string()),
            };
            (swatch(r.color()), vec![r.name().to_string(), moves, yields, defense])
        })
        .collect();
    Article {
        title: "Cases et terrain",
        icon: "icons/wiki_terrain.png",
        blocks: vec![
            p("Chaque case a un terrain (selon le climat), une altitude en mètres et un relief (plat, vallonné, \
               accidenté, montagne ou pic). Le survol d'une case affiche tout cela, avec ses rendements et son coût \
               de déplacement."),
            h("Terrains"),
            table_icons(&["Terrain", "Nourriture", "Production", "Deniers", "Déplacement", "Défense"], rows),
            p("La mer et les lacs ne se traversent pas encore (pas d'unités navales), mais une ville peut exploiter \
               leurs rendements. Les rivières, fleuves et lacs sont décrits dans « Cours d'eau et lacs »."),
            h("Fonds marins et pêche"),
            p("Le fond de la mer est calculé d'après les plaques : un plateau continental (moins de 200 m) longe la \
               plupart des côtes, plus ou moins large ; il manque devant les subductions, où une fosse (plus de \
               6 000 m) court au large ; au-delà, le talus, puis les abysses (plus de 3 000 m). L'infobulle donne la \
               profondeur ; la carte de l'altitude montre eaux peu profondes, haute mer et abysses. Comme sur Terre, \
               où les mers de plateau donnent l'essentiel de la pêche, les eaux peu profondes nourrissent mieux (2 \
               nourriture) que la haute mer (1)."),
            p("Certaines eaux sont poissonneuses (+1 nourriture) : les remontées d'eaux \
               froides, là où le vent pousse l'eau de surface vers le large et où l'eau profonde, riche, remonte \
               (surtout les côtes ouest des continents sous les alizés) ; les monts sous-marins, volcans qui montent \
               vers la surface en pleine mer (chaînes des points chauds, arcs d'îles), où courants et remous \
               concentrent le plancton et les thons ; les eaux des îles volcaniques isolées en haute mer ; la \
               rencontre d'un courant chaud et d'un courant froid près des côtes, où les eaux se mélangent (comme les \
               bancs de Terre-Neuve). Les \
               dorsales et les fosses, trop profondes, ne changent rien à la pêche. Les eaux poissonneuses d'un seul \
               tenant forment des bancs de poissons nommés (Bancs des Sardines, Banc de Kaldvik…) : la carte des \
               ressources montre leur zone, voilée de turquoise, et une icône de poisson par banc (contour doré au survol, l'infobulle dit d'où vient \
               leur richesse), et la recherche les trouve (filtre « Bancs de poissons »). Plus tard, la profondeur et la \
               distance à la côte décideront aussi de la navigation : barques et galères près des côtes et sur le \
               plateau, caravelles en haute mer."),
            h("Relief"),
            p(format!(
                "Le relief est interne à la case et s'ajoute au terrain : une forêt peut être vallonnée, une toundra \
                 montagneuse. Il dépend de la dénivelée avec les cases voisines et de l'altitude. Les coûts d'un relief \
                 vallonné ou accidenté s'ajoutent au terrain sans dépasser {RELIEF_MOVE_CAP} PM. Une montagne coûte \
                 {MOUNTAIN_MOVE_MIN} PM jusqu'à {} m, puis 2 PM de plus par tranche de {MOUNTAIN_STEP_M:.0} m, jusqu'à \
                 {MOUNTAIN_MOVE_MAX} PM : les cols et les vallées, plus bas que les crêtes, sont les passages les plus \
                 rapides (la cavalerie paie le double, l'éclaireur les deux tiers). Ce qui dépasse les PM de l'unité \
                 est dû sur les tours suivants (voir « Mécaniques »). Collines, relief accidenté et montagne empêchent la charge de \
                 cavalerie ; leur bonus de défense se cumule avec la forêt, la jungle ou la taïga. Seuls les pics, les \
                 plus hauts sommets, sont infranchissables ; on ne fonde pas de ville en montagne.",
                MOUNTAIN_MIN_M + MOUNTAIN_STEP_M
            )),
            table_icons(&["Relief", "Déplacement", "Rendements", "Défense"], reliefs),
            h("Altitude"),
            p(format!(
                "Chaque case de terre a une altitude en mètres (carte de l'altitude). Pente : monter de moins de \
                 {CLIMB_GENTLE_M:.0} m vers une case voisine ne coûte rien de plus, jusqu'à {CLIMB_STEEP_M:.0} m +2 PM, \
                 au-delà +4 PM ; descendre est gratuit. Entre deux cases dont l'altitude diffère de 1 000 m ou plus (la \
                 plus basse sous 1 500 m), une falaise est infranchissable dans les deux sens : on ne peut ni y combattre au corps à corps \
                 ni flanquer à travers. Être plus haut d'au moins {} m donne +{HIGHER_GROUND_DEFENSE} en défense de \
                 mêlée et +{HIGH_GROUND_PERCENT} % de force de tir ; un tireur plus bas d'autant perd 1 de portée.",
                combat::HIGHER_GROUND_M
            )),
            p("Les terres se répartissent en 9 niveaux d'altitude, plus serrés en bas (limites à 100, 250, 500, 750, \
               1 000, 1 300, 1 700 et 2 300 m) : ils teintent la carte 2D (plus sombre en bas, plus clair en haut) et \
               marquent les descentes. Chaque monde a son relief : l'un a de grandes plaines basses, l'autre de hauts \
               plateaux."),
            p("Sur la carte : en 2D, une ombre en dégradé marque une descente et une arête sombre à dents de hachure \
               marque une falaise ; en 3D, le sol est à sa hauteur réelle en mètres, les falaises sont des parois rocheuses, \
               et le relief des cases (collines, montagnes, pics) est modelé dans le sol : crêtes et ravines continues \
               d'une case à l'autre, roche sur les pentes raides, neige en hauteur (plus bas dans le froid), arbres \
               sur les versants jusqu'à la limite des arbres. Chaque montagne a un sommet rocheux arrondi, relié aux \
               montagnes voisines par une arête plus basse, et peu d'arbres (petits conifères au pied) ; un pic, \
               infranchissable, se reconnaît à son aiguille bien plus haute et sans arbres. Les cours d'eau coulent dans des vallées. Les côtes \
               basses descendent en plage jusqu'à la mer, les plus hautes tombent en falaise."),
            h("Plaques tectoniques"),
            p("La planète est découpée en une douzaine de plaques (carte des plaques, touche T), surtout \
               continentales (teintes chaudes) ou surtout océaniques (teintes froides). La croûte continentale, épaisse \
               et légère, porte les terres : elle forme quelques grands blocs de tailles très inégales et des \
               microcontinents, séparés par l'océan, posés sur les plaques sans suivre leurs frontières (un \
               continent peut chevaucher deux plaques, une plaque porter à la fois un continent et un océan ; \
               l'infobulle de la carte des plaques donne la croûte de la case). Chacune dérive \
               à 1 à 9 cm par an (flèches) en tournant un peu sur elle-même : sa vitesse change d'un endroit à \
               l'autre. Les plaques sont calculées sur la carte elle-même, qui fait le tour de la planète d'est en \
               ouest : une case a la même taille partout, rien n'est étiré près des pôles. Chaque pôle est occupé \
               par une ou deux plaques océaniques, en bande le long du bord : les mers polaires sont navigables et \
               au moins un passage maritime fait le tour de la planète (si une terre ou une chaîne d'îles le \
               fermait, elle est rabotée). L'eau qui atteint le bord s'en va. Le long d'une frontière, deux plaques qui se rapprochent \
               forment une collision (deux continents : grande chaîne de montagnes et haut plateau), une \
               subduction (l'océan plonge sous le continent : chaîne côtière, fosse au large) ou un arc d'îles \
               (deux océans) ; deux plaques qui s'écartent ouvrent un rift continental ou une dorsale ; deux \
               plaques qui glissent l'une contre l'autre forment une faille transformante. Les points chauds, loin \
               des frontières, sont des volcans isolés. La nature d'une frontière dépend de la croûte de part et \
               d'autre : la même frontière peut être une subduction le long d'un continent puis un arc d'îles en \
               plein océan. Le relief en découle : les continents sont posés sur la croûte continentale, dont la \
               mer recouvre les parties basses et le pourtour (plateau continental) (golfes, bras de mer, mers peu \
               profondes : environ un cinquième de la croûte continentale est sous l'eau, comme sur Terre) ; une \
               collision dresse une grande chaîne et un haut plateau, une \
               subduction une cordillère le long de la côte et une fosse au large, deux océans qui se rapprochent \
               un arc d'îles ; un rift qui s'ouvre lentement creuse une vallée perchée sur un bombement, au fond \
               découpé en bassins où l'eau peut former un chapelet de lacs (comme l'Afrique de l'Est) ; plus vite, \
               le fossé s'enfonce sous la mer en un bras de mer étroit (comme la mer Rouge) ; plus vite encore, le \
               continent s'est déchiré et un jeune océan s'est ouvert entre ses deux morceaux, de part et d'autre \
               d'une dorsale (comme l'Atlantique entre l'Afrique et l'Amérique du Sud) ; une dorsale soulève le \
               fond marin ; un \
               point chaud fait une île volcanique, suivie d'un chapelet d'îles plus anciennes là où la plaque \
               les a emportées."),
            p("Puis l'eau creuse ce relief (érosion) : les grands cours d'eau s'enfoncent dans des vallées au \
               profil régulier, les cuvettes s'ouvrent ou se comblent (peu de lacs, sauf dans les fossés et les \
               régions sèches), les crêtes sont découpées et leurs points bas forment des cols. Les montagnes, peu \
               drainées, gardent leur hauteur. Une fois le climat connu, les vrais cours d'eau creusent à nouveau leur vallée selon leur débit, plus fort près de la mer où la houle et la marée emportent les dépôts : les fleuves arrivent à la mer à son niveau. Une même graine (menu Échap) redonne exactement le même monde."),
            h("Brouillard de guerre"),
            p("Une case inexplorée est noire. Une fois découverte, elle reste connue mais apparaît assombrie hors de \
               vue : on y voit le terrain, pas les unités ennemies. Chaque unité voit à 2 cases (3 pour l'éclaireur, \
               la cavalerie légère et l'archer monté)."),
            h("Nouvelle partie"),
            p("Au lancement, l'écran de nouvelle partie règle le monde et la partie. Le code du monde (par exemple \
               777-M-T29-C0-R0 : graine, taille, terres, climat, relief) redonne la même planète : copiez-le pour \
               la partager, collez un code reçu puis Entrée. Barbares : rares, normaux ou nombreux (un camp pour \
               320, 160 ou 80 cases de terre). Les réglages grisés « à venir » ne sont pas encore pris en compte. \
               La carte reste cachée : « Révéler la carte » la montre, mais gâche la découverte. « Mes \
               configurations » garde les configurations enregistrées (nom, commentaire, favorite) et celles des \
               parties jouées ; en charger une demande s'il faut garder le même monde ou tirer une nouvelle graine. \
               En jeu, menu Échap → « Enregistrer la configuration de ce monde » la garde aussi, et « Nouvelle \
               partie » revient à cet écran."),
            h("Sauvegarde"),
            p("Menu Échap → « Sauvegarder la partie » enregistre la partie ; « Charger la dernière partie » la \
               reprend (le jeu se relance et regénère le monde depuis sa graine, puis remet la partie en place). \
               Une partie rechargée continue exactement comme elle l'aurait fait : le hasard vient de la graine. \
               Une sauvegarde faite avec une version du jeu qui générait un autre monde est refusée. Le journal \
               n'est pas gardé."),
            h("Cartes et vues"),
            p("Le bouton « Carte » (touche T) choisit la carte : normale, de l'altitude (couleur selon les mètres), du \
               relief (plat à pic), du climat (pluies et vents), des bassins versants, des débits des cours d'eau, du réseau hydrographique, \
               des plaques tectoniques, des courants marins (température de la mer, sens des courants), des \
               ressources (bancs de poissons) ou géographique. \
               La carte géographique montre le découpage du territoire, une couleur par zone (deux zones voisines \
               n'ont jamais la même) ; sa légende choisit la famille : terres, mers, relief ou régions naturelles. \
               De loin, les grandes zones (continents, océans, chaînes, régions) ; de près, leurs parties \
               (péninsules, golfes, massifs, sous-régions) en nuances de la couleur de leur grande zone, le reste \
               de la grande zone en teinte sourde et ses limites en trait sombre. Seuls les noms de la famille \
               montrée s'affichent, en blanc. \
               Le bouton « Vue 3D » (touche V) passe en vue 3D ; la carte choisie s'applique aux deux vues. En 3D, \
               chaque unité est une figurine (un petit groupe pour les militaires), plus grande quand on s'éloigne, \
               posée sur le relief et entourée d'un anneau aux couleurs de son camp \
               (bleu : vous, rouge : hostiles), dans une barque sur un fleuve ; de loin, l'icône de son type flotte \
               au-dessus. Le bouton \
               « Options de carte » (touche O) règle les noms affichés (cours d'eau, lacs, villes ; la touche N les \
               masque ou les remet tous), la surbrillance des cours d'eau (au survol du cours d'eau, au survol de son \
               étiquette, ou aucune) et les rubriques de l'infobulle, pour chaque carte : par défaut le terrain, le \
               climat, les rendements et le coût de déplacement, plus ce qui concerne la carte affichée (débit sur la \
               carte des débits, bassin et affluents sur celle des bassins versants…)."),
            h("Mers et océans"),
            p("La mer est partagée en étendues nommées, d'après ses côtes, de la plus vaste à la plus précise : les \
               océans (découpés là où la mer se resserre entre les terres ; au-delà du 60e parallèle, la mer de \
               chaque pôle forme un seul océan polaire ; le nom d'un grand océan est répété en plusieurs endroits) ; les mers, fermées (derrière une entrée étroite par rapport à leur largeur, \
               un haut-fond la fermant à demi ; elles commencent à ce seuil, avec leurs golfes et baies) ou parties d'un océan à l'abri entre plusieurs \
               terres (une côte et des îles, un arc d'îles), prolongées sur le plateau continental ; les golfes \
               (21 cases et plus, sans limite : un golfe peut être immense) et les baies (3 à 20 cases), échancrures \
               d'une même terre derrière une entrée plus étroite qu'elles ne sont profondes, jusqu'à la ligne entre \
               les deux caps (un golfe où se jette un fleuve peut en porter le nom : « Golfe de la Vaska ») ; les \
               détroits (passages de 1 à 4 cases entre deux terres distinctes, précieux à tenir ; un chenal qui se \
               resserre en plusieurs endroits forme un seul détroit). Une baie peut être \
               dans un golfe, un golfe dans une mer, elle-même dans un océan. Les \
               noms s'affichent à leur échelle : océans de très loin, mers de loin, golfes en s'approchant, baies et \
               détroits de près, en capitales espacées pour les plus grandes ; un nom n'apparaît que si l'étendue est \
               assez large à l'écran et qu'une partie en est explorée. L'infobulle d'une case de mer donne ses \
               étendues (« Eaux peu profondes — Baie des Phoques, Mer de Vodava, Grand Océan »)."),
            h("Terres et îles"),
            p("Les terres sont nommées elles aussi : les continents (les plus grandes terres d'un seul tenant ; \
               un très grand continent est coupé en deux à un isthme quand chaque côté en a la taille), les îles et \
               les îlots (1 à 3 cases), les archipels (au moins trois îles proches, nommées dans la même langue), les \
               péninsules et presqu'îles (parties d'une terre derrière un col étroit, entourées de mer, qui \
               s'avancent : faciles à défendre) et les isthmes (passages de 1 à 3 cases de terre entre deux mers, \
               qui relient deux grandes parties d'une terre : précieux à tenir). Une péninsule ou un isthme est dans \
               son continent ou son île, une île dans son archipel. Les noms s'écrivent en lettres droites couleur \
               sable (l'italique est réservé à l'eau) et s'affichent à leur échelle : continents en grandes \
               capitales de très loin, archipels et grandes îles de loin, péninsules et îles moyennes en \
               s'approchant, isthmes, petites îles et îlots de près. L'infobulle d'une case de terre donne ses \
               terres (« Prairie — Presqu'île de Lanza, Esinzie »)."),
            h("Régions naturelles"),
            p("Les grands paysages ont aussi leur nom : forêts, jungles, taïgas, déserts, steppes et plaines, \
               savanes, maquis, marais et toundras (une clairière ou une oasis ne coupe pas une région). Une région \
               de plus de 150 cases est partagée en sous-régions nommées d'après leur position (« Forêt de Liji du \
               Nord », « Grand Désert du Centre »). Les noms prennent la couleur du paysage (vert pour la \
               végétation, sable pour les terres sèches, vert d'eau pour les marais, blanc bleuté pour la \
               toundra) : les grandes régions en capitales de loin, leurs sous-régions en s'approchant, les \
               petites (un marais, un bois) de près. L'infobulle les donne après le relief (« Forêt — Forêt de \
               Liji de l'Ouest, Forêt de Liji, Esinzie »)."),
            h("Montagnes et relief"),
            p("Le relief a ses noms : les chaînes de montagnes (montagnes et pics d'un seul tenant), découpées en \
               massifs d'une vingtaine de cases quand elles sont longues ; les cols, passages moins chers entre deux \
               montagnes, là où les deux côtés de la chaîne ne se rejoignent pas de près (franchir une montagne \
               coûte de 8 à 16 PM selon son altitude, un pic ne se franchit pas : le col fait gagner des PM, il est à \
               tenir ; la recherche donne son altitude) ; les plateaux (terres plates ou vallonnées au-dessus de 1 500 m) et les \
               collines (amas de cases vallonnées, bonnes à défendre). Les noms s'écrivent en brun clair : \
               chaînes en capitales de loin, massifs, plateaux et collines en s'approchant, cols de près. \
               L'infobulle donne le relief d'une case avant ses terres (« Montagne — Col de Grausara, Massif de \
               Fiosona, Monts Grausara, Esinzie »)."),
            p("Pour que la carte reste lisible, chaque niveau de noms a un nombre maximal de noms à l'écran \
               (les plus grandes zones d'abord), et le niveau de zoom est le même pour toute la vue (celui du \
               centre) : de loin, océans et continents partout, même au premier plan en 3D. Les noms des chaînes \
               de montagnes, des massifs et des golfes allongés se courbent le long de leur forme quand ils y \
               tiennent."),
            h("Où suis-je ?"),
            p("Sous la barre du haut, la barre de lieu donne les lieux nommés au centre de la vue, du plus vaste \
               au plus précis : « Inetsie › Jungle de Shuchets › Bassin du Smurask » sur terre (continent ou île, \
               péninsule, région naturelle, relief, bassin versant), « Océan Nizxexco › Mer de X › Golfe de Y » en \
               mer. Elle se met à jour quand la caméra bouge, en 2D comme en 3D, et reste vide tant que le centre \
               de la vue n'est pas exploré. Chaque lieu est cliquable : le survoler fait ressortir sa zone sur \
               la carte, cliquer y amène la caméra. L'infobulle donne la même chose pour la case survolée."),
            h("Trouver un lieu"),
            p("Le bouton « Rechercher » (Ctrl+F) ouvre la recherche : tapez le début d'un nom (accents et majuscules \
               ne comptent pas) ; les puces filtrent par catégorie (villes, mers, terres et îles, relief, régions \
               naturelles, cours d'eau, lacs, bassins versants, plaques) et donnent le nombre de résultats de chacune. Flèches pour choisir, Entrée pour y \
               aller, Tab pour changer de filtre, Échap pour fermer. Le lieu choisi ressort sur la carte (pourtour \
               doré d'une zone, surbrillance d'un cours d'eau ou d'un lac) et la caméra le cadre. Seuls les lieux \
               déjà explorés sont listés (tous en mode debug)."),
            p("Les étiquettes de la carte mènent aussi à leur lieu : un clic sur le nom d'un cours d'eau, d'un lac \
               ou sur la bannière d'une ville y amène la caméra. Sur la carte des bassins versants, survoler un \
               bassin de la légende le fait ressortir, cliquer dessus le montre."),
        ],
    }
}

/// Traits particuliers d'une unité, en phrases.
fn unit_traits(k: UnitKind) -> Vec<String> {
    let mut traits = Vec::new();
    match k {
        UnitKind::Settler => traits.push(
            "Fonde une ville (touche B). Le produire coûte 1 000 habitants à la ville, qui doit en avoir au moins 2 000. \
             Ne peut pas attaquer."
                .to_string(),
        ),
        UnitKind::Scout => {
            traits.push("Unité militaire légère et peu coûteuse, sans zone de contrôle. Rapide et voit loin : idéale pour l'exploration automatique (touche E). Combat très faiblement.".into())
        }
        UnitKind::ExpertArcher => traits.push("Ne peut pas tirer après s'être déplacé dans le même tour.".into()),
        UnitKind::HorseArcher => {
            traits.push("Après un tir, garde ses PM moins 1 : il peut se replier dans le même tour.".into())
        }
        _ => {}
    }
    if k.exerts_zoc() {
        traits.push("Exerce une zone de contrôle.".into());
    }
    if k.can_charge() {
        traits.push(format!("Charge : +{CHARGE_BONUS} en attaque s'il s'est déplacé ce tour-ci."));
    }
    match k.mountain_cost(6) {
        12 => traits.push("En montagne, les chevaux sont menés à la bride : le coût du terrain est doublé.".into()),
        4 => traits.push("Léger et habitué aux sentiers : la montagne ne lui coûte que les deux tiers.".into()),
        _ => {}
    }
    // Bonus de type : on interroge la règle de combat contre chaque famille
    let mut bonuses: Vec<String> = Vec::new();
    for other in UnitKind::ALL {
        if let Some((b, label)) = combat::bonus_vs(k, other) {
            let line = format!("+{b} {label}");
            if !bonuses.contains(&line) {
                bonuses.push(line);
            }
        }
    }
    if !bonuses.is_empty() {
        traits.push(format!("Bonus en mêlée : {}.", bonuses.join(", ")));
    }
    if k.class() == UnitClass::Ranged || k == UnitKind::HorseArcher {
        traits.push("Attaque toujours en tirant, même au contact : pas de riposte.".into());
    }
    traits
}

fn units() -> Article {
    let rows = UnitKind::ALL
        .iter()
        .map(|&k| {
            let s = k.stats();
            let ranged = if s.ranged_attack > 0 { format!("{} ({})", s.ranged_attack, s.range) } else { "—".into() };
            let melee = if s.melee_attack > 0 { format!("{} / {}", s.melee_attack, s.melee_defense) } else { format!("— / {}", s.melee_defense) };
            let row = vec![k.name().into(), s.cost.to_string(), melee, ranged, s.ranged_defense.to_string(), s.moves.to_string(), s.sight.to_string()];
            (Icon::Unit(k.icon()), row)
        })
        .collect();
    let mut blocks = vec![
        p(format!(
            "Toutes les unités ont {MAX_HP} PV. Elles sont produites dans les villes (panneau de production ou \
             touches 1 à 9) ; le coût est en production."
        )),
        table_icons(&["Unité", "Coût", "Mêlée att. / déf.", "Tir (portée)", "Déf. distance", "PM", "Vision"], rows),
    ];
    for k in UnitKind::ALL {
        blocks.push(hi(k.name(), Icon::Unit(k.icon())));
        let traits = unit_traits(k);
        blocks.push(p(if traits.is_empty() { "Unité de ligne, sans particularité.".to_string() } else { traits.join(" ") }));
    }
    Article { title: "Unités", icon: "icons/wiki_units.png", blocks }
}

fn mechanics() -> Article {
    Article {
        title: "Mécaniques",
        icon: "icons/wiki_mechanics.png",
        blocks: vec![
            h("Tour de jeu"),
            p("Chaque tour, vous déplacez vos unités et gérez vos villes, puis vous terminez le tour (Entrée ou \
               bouton). Les villes produisent et grandissent, les unités récupèrent leurs PM, puis la faction \
               barbare joue."),
            h("Déplacement"),
            p(format!(
                "Une unité peut entrer sur une case tant qu'il lui reste au moins 1 PM, quel que soit le coût de la \
                 case. Mais le terrain se paie en entier : les PM qui manquaient sont une dette, retirée des PM des \
                 tours suivants. Une montagne ({MOUNTAIN_MOVE_MIN} à {MOUNTAIN_MOVE_MAX} PM) peut ainsi immobiliser \
                 une unité plusieurs tours : une pastille orange sur l'unité donne le nombre de tours avant qu'elle \
                 reparte (il baisse de 1 à chaque tour). Traverser une rivière ou entrer dans un fleuve arrête seulement l'unité, sans dette. Un ordre (clic droit) peut durer plusieurs tours : le numéro du tour d'arrivée s'affiche le long \
                 du trajet. Les trajets ne s'appuient que sur le terrain connu ; une case inexplorée est supposée \
                 franchissable mais coûte {UNKNOWN_MOVE_COST} PM. L'unité recalcule son trajet en découvrant le terrain ; \
                 l'ordre est suspendu s'il devient plus de {DETOUR_TOLERANCE_PERCENT} % plus long que prévu."
            )),
            h("Une unité par case"),
            p("Une case ne peut accueillir qu'une unité. Vos unités se traversent : un trajet passe par la case \
               d'une de vos unités s'il reste assez de PM pour en repartir dans le même tour, sinon il la contourne. \
               Un clic droit sur une de vos unités adjacentes échange leurs places (sauf à travers une falaise)."),
            h("Groupes d'unités"),
            p("Maj + clic sur une de vos unités l'ajoute au groupe sélectionné (ou l'en retire) ; un clic simple \
               revient à une seule unité. Clic droit avec un groupe : l'unité principale (anneau jaune vif) va sur \
               la case visée, les autres (anneau pâle) sur les cases libres les plus proches autour d'elle. \
               Ctrl + chiffre range le groupe sous ce chiffre, le chiffre seul le rappelle (quand aucune ville \
               n'est choisie : les chiffres y choisissent la production), deux appuis rapides centrent la vue dessus."),
            h("Exploration automatique"),
            p("Touche E (unités militaires et éclaireur) : l'unité part vers la zone qui révèle le plus de cases \
               inconnues pour le moindre trajet, fait un détour pour visiter les villages et évite les abords des \
               ennemis et des camps connus. Plusieurs explorateurs se répartissent les zones. Un ordre manuel ou \
               Espace l'interrompt."),
            h("Combat"),
            p(format!(
                "Clic droit sur un ennemi visible : l'unité s'approche si besoin puis attaque ; une prévision du \
                 combat s'affiche au survol. Une attaque par tour ; elle consomme les PM restants. Dégâts ≈ \
                 {} × e^(écart de force ÷ {}) × aléa (0,8 à 1,2). Chaque tranche de 10 PV perdus retire 1 de force. \
                 À 0 PV, l'unité est dispersée. L'aléa est tiré de la graine de la partie : une même attaque, \
                 rejouée dans la même situation, donne le même résultat.",
                combat::BASE_DAMAGE, combat::STRENGTH_SCALE
            )),
            p("Corps à corps : le défenseur riposte, et le vainqueur avance sur la case libérée. Tir : force de tir \
               contre défense à distance, sans riposte."),
            p(format!(
                "Modificateurs : couvert boisé (forêt, jungle, taïga) +{TERRAIN_DEFENSE} et collines +{TERRAIN_DEFENSE} en défense (cumulables) ; \
                 défenseur plus haut +{HIGHER_GROUND_DEFENSE} en mêlée ; flanquement +{FLANK_BONUS} en attaque de mêlée \
                 par allié adjacent à la cible ; charge de cavalerie +{CHARGE_BONUS} (pas contre une cible en couvert \
                 boisé, dans un marais ou dans des collines) ; bonus de type (lances contre cavalerie…)."
            )),
            p(format!(
                "Tir : un archer plus bas que sa cible, ou une cible en couvert boisé, perd 1 de portée (jamais moins de 1) ; \
                 un archer plus haut gagne +{HIGH_GROUND_PERCENT} % de force de tir."
            )),
            h("Zone de contrôle"),
            p("L'infanterie de mêlée (milice, guerriers, lanciers, piquiers) contrôle les cases qui l'entourent : une \
               unité ennemie qui y entre s'arrête pour le tour (elle peut encore attaquer). Le trajet d'approche d'une \
               attaque ne traverse pas de zone de contrôle."),
            h("Soins"),
            p("Une unité qui ne bouge ni ne combat pendant un tour récupère 10 PV (20 en territoire ami)."),
            h("Sur la carte"),
            p("Les unités se déplacent case par case et les combats se voient : élan de l'attaquant ou trait du \
               tireur, PV perdus en chiffres rouges au-dessus des deux unités, « Dispersé ! » ou « Unité perdue » \
               quand une unité disparaît. La récompense d'un village ou d'un camp s'affiche sur place. En fin de \
               tour, vos unités bougent d'abord, puis les actions barbares visibles se jouent l'une après l'autre ; \
               votre tour commence quand tout est joué (le bouton affiche « Le tour se joue… » jusque-là, et vos \
               ordres attendent). \
               Vitesse des animations : menu Échap → Paramètres (Lente, Normale, Rapide ou Instantanée : \
               les unités sont aussitôt à leur place, seuls les chiffres s'affichent)."),
            p("Journal (en haut à droite) : la couleur du liseré dit l'importance du message — gris pour un retour \
               de commande, bleu pour une information, doré pour une bonne nouvelle, orange pour une alerte \
               (combat, famine), rouge pour un message critique (unité perdue, village hostile). Les alertes et \
               messages critiques clignotent à leur arrivée et restent teintés. Les noms soulignés (villes, \
               villages, camps, unités) mènent à eux : survolez-les pour les voir sur la carte, cliquez pour y \
               aller (et sélectionner votre ville ou votre unité). Une unité dispersée ou hostile hors de vue : la \
               caméra va à sa dernière position connue."),
        ],
    }
}

fn cities() -> Article {
    let paliers = [1, 2, 3, 5, 10, 15, 20, 25, 30].iter().map(|&p| vec![p.to_string(), habitants_du_palier(p).to_string()]).collect();
    Article {
        title: "Villes",
        icon: "icons/wiki_cities.png",
        blocks: vec![
            h("Fonder une ville"),
            p(format!(
                "Un colon fonde une ville sur sa case (touche B), sur n'importe quel terrain franchissable, à {MIN_CITY_DISTANCE} \
                 cases au moins d'une autre ville. Elle naît en hameau : quelques maisons, un grand toit et un mât à vos couleurs, sans \
                 enceinte. Elle devient bourg au palier 3, puis ville au palier 6, en ajoutant des bâtiments. Ses matériaux viennent du lieu, et \
                 elle s'adapte à la case : rien sur les rives, maisons dans le sens de la pente, ponton si l'eau est proche. Une ville \
                 fortifiée a une muraille et plusieurs portes, qui suivent le terrain. La garnison se tient sur la place. Dans un marais, les \
                 maisons sont sur pilotis, reliées par des passerelles, et les mares se réduisent quand la ville grandit. Au stade Ville, une \
                 ville sur un cours d'eau jette un pont vers l'autre rive, gardé par une tour si elle est fortifiée (décor pour l'instant : \
                 il ne change pas les déplacements)."
            )),
            h("Rendements"),
            p("La case centrale rapporte au moins 2 nourriture, 2 production et 1 denier, sans travailleurs. La moitié \
               des habitants travaille sur les cases du territoire, placée par l'intendant par blocs de 100 là où le gain \
               est le plus fort : la nourriture pèse lourd tant que la ville ne se nourrit pas (puis pour croître), la \
               production et les deniers prennent ensuite le relais ; les cases lointaines rapportent un peu moins."),
            p("Chaque case a une capacité en travailleurs : 500 sans aménagement (cueillette, chasse), 1 500 pour un \
               champ, 1 000 pour un pâturage ou une coupe de bois. Son rendement croît avec les travailleurs, de moins en \
               moins vite (la moitié de la capacité donne 62 % du rendement de la case) : une petite ville se répartit \
               sur plusieurs cases. Au-delà des capacités, les habitants sont sans emploi : il faut aménager des cases."),
            table(
                &["Aménagement", "Cases", "Rendement à pleine capacité"],
                vec![
                    vec!["champ".into(), "relief plat ou collines, assez de pluie ou une rivière voisine".into(), "nourriture × 2".into()],
                    vec!["pâturage".into(), "herbe (ni désert, ni neige, ni marais)".into(), "nourriture × 1,5, +1 production".into()],
                    vec!["coupe de bois".into(), "forêt, jungle, taïga, savane arborée".into(), "production × 2".into()],
                ],
            ),
            p("Un champ ou un pâturage pris sur la forêt la défriche : la case devient une prairie (le bois est perdu). \
               La construction des aménagements par la ville arrive ; pour l'instant, la console les pose \
               (`amenagement`), et règle les consignes de l'intendant (`ville priorite`, `ville forcer`)."),
            h("Croissance"),
            p(format!(
                "La population compte les habitants de toute l'aire de la ville, ses campagnes comprises. Un point de \
                 nourriture nourrit 1 000 habitants. Avec un surplus, la population grandit de 1,5 % par tour au plus \
                 (atteint avec 50 % de nourriture en plus du besoin) ; en famine, elle perd de 2 à 5 % par tour. Le palier \
                 résume la taille : il fixe le territoire et le stade de la ville. Un colon \
                 emporte {HABITANTS_COLON} habitants, d'une ville qui en compte au moins le double."
            )),
            table(&["Palier", "Habitants de l'aire"], paliers),
            h("Écran de la ville"),
            p("Touche C (ou le bouton « Écran de la ville » du panneau de production) avec une ville sélectionnée :                l'écran de la ville s'ouvre en plein écran ; Échap le ferme (d'abord la fiche d'une case). En haut, le                palier, les habitants, la croissance et la construction en cours. L'onglet Rendements donne la                nourriture (comparée au besoin), la production et les deniers, et d'où ils viennent (centre, cases) ;                l'onglet Population, les habitants, les actifs et les sans-emploi. Les grandeurs et les onglets qui ne                sont pas encore dans le jeu (eau, recherche, culture, défense, bien-être, commerce, histoire) sont                affichés comme tels."),
            p("À droite, la carte des cases de la ville : couleur de la grandeur principale de chaque case (pâle si                personne n'y travaille), ou remplissage dans l'onglet Population (orange : presque pleine) ; rond vert :                aménagement (bouton de la légende pour le masquer). Survol : l'infobulle de la case ; clic : sa fiche                (aménagement, travailleurs, rendements, aménagements possibles) ; molette ou + et − : zoom ; glisser :                déplacer. La liste dessous éclaire sa case au survol."),
            p("Clic sur une grandeur (ou touches 1 à 3) : son détail. Le graphique montre les derniers tours (10, 20                ou 40) et la prévision au rythme actuel, avec une bande d'incertitude et des repères (besoin et                croissance pleine pour la nourriture, équilibre pour les deniers) ; dessous, d'où vient la grandeur                et où elle va (réserve pour croître, excédent perdu, construction, trésor). La carte prend la couleur                de la grandeur et donne sa valeur par case ; la fiche d'une case dit ce que 100 travailleurs de plus                y rapporteraient. Échap remonte d'un cran (case, grandeur, écran)."),
            p("L'onglet Population montre les habitants, les actifs (la moitié) et les sans-emploi, l'évolution de \
               la population en échelle logarithmique avec sa prévision et ce que la ville peut porter : les \
               habitants que sa nourriture nourrit (la croissance s'y arrête) et ceux qui trouvent un emploi (au-delà : \
               des sans-emploi). Sa carte montre au choix le remplissage des cases en travailleurs ou la répartition \
               des habitants (pour l'instant, chacun vit là où il travaille ; les sans-emploi au centre)."),
            p("Sous l'en-tête, « Ce qui freine la croissance » donne une seule cause (famine, objectif de \
               l'intendant, limite de la nourriture et le tour où elle sera atteinte) et des remèdes chiffrés : un \
               clic ouvre le placement de l'aménagement à la case conseillée, ou le réglage de l'intendant. Les \
               autres alertes suivent (rien à produire, sans-emploi, long projet)."),
            p("L'onglet Histoire raconte la ville : fondation, paliers atteints, famines, ponts et aménagements \
               achevés, projets lancés, combats décisifs sur son territoire. Les lieux des événements sont cerclés \
               d'or sur la carte ; un clic sur une case ne garde que son histoire."),
            p("Réglages de l'intendant, dans le détail des grandeurs : l'objectif de nourriture (nourrir seulement, \
               croître, croissance maximale, excédent : 1 ; 1,25 ; 1,5 ; 2 fois le besoin) qu'il vise d'abord, puis la \
               priorité de la production et des deniers (normale, prioritaire, maximale). Chaque choix montre son \
               effet sur toute la ville et sur la croissance. Dans la fiche d'une case, « Forcer l'effectif » fixe ses \
               travailleurs (aperçu de l'effet, puis confirmer) ; l'intendant ne la change plus jusqu'à ce qu'on la \
               lui rende."),
            h("Territoire"),
            p("Le territoire s'étend à 1 case autour de la ville, puis à 2 cases à partir du palier 3. Il sert \
               aux cases exploitées et aux soins (+20 PV par tour en territoire ami)."),
            h("Production"),
            p("Chaque ville a une file de construction : unités, ponts et aménagements, chacun avec la production \
               déjà investie (perdue s'il est retiré) ; le surplus d'un élément fini passe au suivant. Une nouvelle \
               ville commence avec une file vide : à vous d'indiquer quoi produire (panneau de production ou écran \
               de la ville, touche B). Une unité apparaît sur la ville ou sur une case voisine libre ; si tout est \
               occupé, elle attend en tête de file. Un colon coûte en plus 1 000 habitants."),
            p("Quand la file est vide, la ville suit son projet de ville, sans fin : « Peupler la ville » ajoute la \
               production au surplus de nourriture pour croître (2 production = 1 nourriture, jamais pour le \
               besoin) ; « Grands jeux », « Projet de recherche » et « Développement de l'art et de la culture » ne \
               sont pas encore dans le jeu. Le journal le rappelle tous les 10 tours. Sans projet ni construction, la \
               production est perdue, et le journal le dit à chaque tour."),
            p("Aménager une case : dans l'écran de la ville, Construire puis un aménagement ; la carte de la ville \
               montre le gain de chaque case à pleine capacité (anneau doré : le conseil de l'intendant). Clic pour \
               choisir la case, double-clic ou Entrée pour mettre en file. La fiche d'une case propose aussi ses \
               aménagements possibles."),
            p(format!(
                "Une ville peut aussi construire un pont sur une rivière ou un fleuve de son territoire (pour une \
                 rivière, une des deux cases suffit). « Pont… » dans le panneau de production surligne les emplacements \
                 possibles ; un clic sur l'un d'eux le choisit, un clic droit annule. Un pont de bois coûte de {} \
                 (petite rivière) à {} de production (grand fleuve), selon le débit ; un pont de pierre, {} fois plus. \
                 Le matériau dépend du lieu : bois si une rive est boisée (forêt, taïga, jungle, savane, marais, \
                 steppe), pierre sinon. Le pont est posé à la fin du tour où il est terminé, puis la ville revient à la \
                 milice ; si elle est fortifiée, une tour à ses couleurs garde l'autre bout.",
                crate::rules::BRIDGE_COST.0,
                crate::rules::BRIDGE_COST.1,
                crate::rules::BRIDGE_STONE_COST
            )),
        ],
    }
}

fn villages() -> Article {
    Article {
        title: "Villages tribaux",
        icon: "icons/village.png",
        blocks: vec![
            p(format!(
                "Des villages tribaux sont dispersés sur la carte (environ une case de terre sur {LAND_PER_VILLAGE}, à \
                 {VILLAGE_MIN_FROM_START} cases au moins du départ). Ils apparaissent quand leur case est explorée. \
                 Personne ne vit là où la saison la plus chaude reste sous 5 °C ; là où elle reste sous 10 °C, ce sont des éleveurs."
            )),
            h("Visiter un village"),
            p("La première unité qui entre dans un village déclenche un événement, puis le village disparaît :"),
            table(
                &["Événement", "Chance", "Effet"],
                vec![
                    vec!["Les habitants vous rejoignent".into(), "35 %".into(), "+1 000 habitants dans la ville la plus proche".into()],
                    vec!["Échanges commerciaux".into(), "45 %".into(), "+15 à 35 deniers".into()],
                    vec!["Village hostile".into(), "20 %".into(), "1 ou 2 guerriers barbares surgissent autour".into()],
                ],
            ),
            p("Sans ville, les habitants ne peuvent rejoindre personne : l'événement devient un échange commercial."),
            p("L'exploration automatique fait volontiers un détour pour visiter un village repéré."),
        ],
    }
}

fn barbarians() -> Article {
    Article {
        title: "Barbares",
        icon: "icons/camp.png",
        blocks: vec![
            p("La faction barbare est hostile envers tout le monde. Ses guerriers ne sont visibles qu'à portée de \
               vue de vos unités."),
            h("Camps"),
            p(format!(
                "Les camps (environ un pour {LAND_PER_CAMP} cases de terre, à {CAMP_MIN_FROM_START} cases au moins du \
                 départ) recrutent une milice tous les {CAMP_SPAWN_INTERVAL} tours, dans la limite de {CAMP_MAX_UNITS} \
                 guerriers par camp. Un camp vide est dispersé quand une de vos unités y entre : +{CAMP_BUTIN_DENIERS} deniers. \
                 Son nom annonce sa forme : un Fort (palissade et tour de guet) sur une case plate et dégagée, un Repaire ou un \
                 Antre adossé aux rochers sur un relief, un Camp ailleurs. Sa fumée s'épaissit dans les deux tours qui précèdent \
                 un recrutement."
            )),
            h("Comportement"),
            p(format!(
                "Les guerriers errent à {WANDER_RADIUS} cases au plus de leur camp. Ils attaquent une de vos unités à \
                 {HUNT_RANGE} cases ou moins, seulement si le combat leur est favorable, en tenant compte du terrain et \
                 du flanquement. Blessé (moins de {WOUNDED_HP} PV) et sans cible favorable, un guerrier reste sur place \
                 pour se soigner."
            )),
            h("Se protéger"),
            p("Les explorateurs automatiques évitent les abords des barbares et des camps connus. Placez vos unités \
               en forêt, dans des collines ou en hauteur pour les défendre, et attaquez à plusieurs pour profiter du \
               flanquement."),
        ],
    }
}

// ---------------------------------------------------------------- Fenêtre

/// Wiki ouvert, rubrique affichée.
#[derive(Resource, Default)]
pub struct WikiState {
    pub open: bool,
    pub article: usize,
}

#[derive(Component)]
struct WikiWindow;
#[derive(Component)]
struct WikiTab(usize);
#[derive(Component)]
struct WikiContent;
#[derive(Component)]
struct WikiClose;
#[derive(Component)]
pub struct WikiButton;

/// Rubriques ; celles au-delà de `public` ne sont montrées qu'en mode debug.
#[derive(Resource)]
struct WikiArticles {
    list: Vec<Article>,
    public: usize,
}

const BG: Color = Color::srgb(0.05, 0.06, 0.09);
const TAB_BG: Color = Color::srgba(0.16, 0.18, 0.24, 0.95);
const TAB_HOVER: Color = Color::srgb(0.24, 0.28, 0.38);
const TAB_ACTIVE: Color = Color::srgb(0.18, 0.40, 0.72);
const TEXT: Color = Color::srgb(0.92, 0.92, 0.95);
const MUTED: Color = Color::srgb(0.70, 0.72, 0.78);
const HEADING: Color = Color::srgb(0.98, 0.85, 0.55);

pub struct WikiPlugin;

/// Systèmes du wiki (le menu Échap passe avant eux).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WikiSet;

impl Plugin for WikiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WikiState>()
            .insert_resource({
                let list = articles();
                let public = list.len();
                WikiArticles { list: list.into_iter().chain([debug_article()]).collect(), public }
            })
            .add_systems(Startup, setup_wiki)
            .add_systems(Update, (wiki_controls, show_article, scroll_wiki).chain().in_set(WikiSet));
    }
}

fn label(s: &str, size: f32, color: Color) -> impl Bundle + use<> {
    (Text::new(s.to_string()), TextFont::from_font_size(size), TextColor(color))
}

const TOKEN_BLUE: Color = Color::srgb(0.20, 0.45, 0.95);

/// Nœud d'une icône de taille `size` (pixels).
fn spawn_icon(parent: &mut ChildSpawnerCommands, icon: Icon, size: f32, assets: &AssetServer) {
    let square = Node { width: Val::Px(size), height: Val::Px(size), flex_shrink: 0.0, ..default() };
    match icon {
        Icon::Image(path) => {
            parent.spawn((square, ImageNode::new(assets.load(path))));
        }
        Icon::Unit(path) => {
            let inner = size * 0.74;
            parent.spawn((
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(size / 2.0)),
                    ..square
                },
                BackgroundColor(TOKEN_BLUE),
                children![(
                    Node { width: Val::Px(inner), height: Val::Px(inner), ..default() },
                    ImageNode::new(assets.load(path)),
                )],
            ));
        }
        Icon::Swatch(r, g, b) => {
            let s = size * 0.8;
            parent.spawn((
                Node {
                    width: Val::Px(s),
                    height: Val::Px(s),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(r, g, b)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
            ));
        }
    }
}

/// Ligne « icône + texte ».
fn icon_row() -> Node {
    Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() }
}

fn setup_wiki(mut commands: Commands, articles: Res<WikiArticles>, assets: Res<AssetServer>) {
    let tabs: Vec<(usize, &'static str, &'static str)> =
        articles.list.iter().enumerate().map(|(i, a)| (i, a.title, a.icon)).collect();
    commands
        .spawn((
            WikiWindow,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(8.0),
                right: Val::Percent(8.0),
                top: Val::Percent(7.0),
                bottom: Val::Percent(7.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(10.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BG),
            GlobalZIndex(100),
            ZIndex(100),
            // Bloque les clics et la molette vers la carte
            UiPointer::default(),
        ))
        .with_children(|w| {
            // Barre de titre
            w.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() })
                .with_children(|bar| {
                    bar.spawn(label("Wiki de CivBuilder", 22.0, TEXT));
                    bar.spawn((
                        WikiClose,
                        UiPointer::default(),
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(TAB_BG),
                        children![label("Fermer (Échap)", 14.0, TEXT)],
                    ));
                });
            // Rubriques à gauche, article à droite
            w.spawn(Node { flex_grow: 1.0, column_gap: Val::Px(14.0), min_height: Val::Px(0.0), ..default() })
                .with_children(|body| {
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        width: Val::Px(210.0),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|col| {
                        for (i, title, icon) in tabs {
                            col.spawn((
                                WikiTab(i),
                                UiPointer::default(),
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                    ..icon_row()
                                },
                                BackgroundColor(TAB_BG),
                            ))
                            .with_children(|tab| {
                                spawn_icon(tab, Icon::Image(icon), 22.0, &assets);
                                tab.spawn(label(title, 16.0, TEXT));
                            });
                        }
                    });
                    body.spawn((
                        WikiContent,
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            row_gap: Val::Px(8.0),
                            padding: UiRect::new(Val::Px(4.0), Val::Px(16.0), Val::Px(0.0), Val::Px(16.0)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollPosition::default(),
                    ));
                });
        });
}

/// Ouverture / fermeture (bouton, F1, Échap) et choix de la rubrique.
fn wiki_controls(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Res<crate::menu::GameMenu>,
    mut state: ResMut<WikiState>,
    mut window: Single<&mut Node, With<WikiWindow>>,
    open_button: Query<Ref<UiPointer>, With<WikiButton>>,
    close_button: Query<Ref<UiPointer>, With<WikiClose>>,
    mut tabs: Query<(Ref<UiPointer>, &WikiTab, &mut BackgroundColor, &mut Node), (Without<WikiClose>, Without<WikiWindow>)>,
    mut close_bg: Query<&mut BackgroundColor, (With<WikiClose>, Without<WikiTab>)>,
    (debug, articles): (Res<crate::game::DebugMode>, Res<WikiArticles>),
) {
    let pressed = |i: &Ref<UiPointer>| i.is_changed() && **i == UiPointer::Pressed;
    // Rubriques du mode debug : onglets cachés hors du mode debug
    if !debug.0 && state.article >= articles.public {
        state.article = 0;
    }
    // Pas de wiki par-dessus le menu Échap
    if !menu.open && (keys.just_pressed(KeyCode::F1) || open_button.iter().any(|i| pressed(&i))) {
        state.open = !state.open;
    }
    if state.open && (keys.just_pressed(KeyCode::Escape) || close_button.iter().any(|i| pressed(&i))) {
        state.open = false;
    }
    for (interaction, tab, mut bg, mut node) in &mut tabs {
        let display = if tab.0 < articles.public || debug.0 { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if pressed(&interaction) {
            state.article = tab.0;
        }
        let color = if state.article == tab.0 {
            TAB_ACTIVE
        } else if *interaction == UiPointer::Hovered {
            TAB_HOVER
        } else {
            TAB_BG
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
    for i in &close_button {
        for mut bg in &mut close_bg {
            let color = if *i == UiPointer::None { TAB_BG } else { TAB_HOVER };
            if bg.0 != color {
                bg.0 = color;
            }
        }
    }
    let display = if state.open { Display::Flex } else { Display::None };
    if window.display != display {
        window.display = display;
    }
}

/// Remplit la zone de lecture quand la rubrique change.
fn show_article(
    mut commands: Commands,
    state: Res<WikiState>,
    articles: Res<WikiArticles>,
    assets: Res<AssetServer>,
    content: Single<(Entity, &mut ScrollPosition), With<WikiContent>>,
    mut shown: Local<Option<usize>>,
) {
    if *shown == Some(state.article) {
        return;
    }
    *shown = Some(state.article);
    let (entity, mut scroll) = content.into_inner();
    scroll.0 = Vec2::ZERO;
    let Some(article) = articles.list.get(state.article) else { return };
    commands.entity(entity).despawn_related::<Children>();
    commands.entity(entity).with_children(|c| {
        c.spawn(icon_row()).with_children(|row| {
            spawn_icon(row, Icon::Image(article.icon), 30.0, &assets);
            row.spawn(label(article.title, 22.0, TEXT));
        });
        for block in &article.blocks {
            match block {
                Block::Heading(s, icon) => {
                    c.spawn(Node { margin: UiRect::top(Val::Px(8.0)), ..icon_row() }).with_children(|row| {
                        if let Some(icon) = icon {
                            spawn_icon(row, *icon, 24.0, &assets);
                        }
                        row.spawn(label(s, 17.0, HEADING));
                    });
                }
                Block::Para(s) => {
                    c.spawn(label(s, 14.0, TEXT));
                }
                Block::Table(header, rows, icons) => {
                    let with_icons = icons.iter().any(Option::is_some);
                    // Colonnes de largeur commune : la première plus large (noms)
                    let last = header.len() - 1;
                    let width = move |i: usize| if i == 0 { 190.0 } else if i == last { 0.0 } else { 115.0 };
                    // Une ligne : icône éventuelle (colonne de gauche) puis cellules.
                    const ICON: f32 = 20.0;
                    let line = |t: &mut ChildSpawnerCommands, cells: &[String], icon: Option<Icon>, color: Color, bg: Color| {
                        t.spawn((
                            Node {
                                column_gap: Val::Px(6.0),
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(bg),
                        ))
                        .with_children(|r| {
                            if with_icons {
                                match icon {
                                    Some(icon) => spawn_icon(r, icon, ICON, &assets),
                                    None => {
                                        r.spawn(Node { width: Val::Px(ICON), flex_shrink: 0.0, ..default() });
                                    }
                                }
                            }
                            for (i, cell) in cells.iter().enumerate() {
                                // Dernière colonne : prend la place restante (textes longs)
                                let node = if width(i) > 0.0 {
                                    Node { width: Val::Px(width(i)), flex_shrink: 0.0, ..default() }
                                } else {
                                    Node { flex_grow: 1.0, min_width: Val::Px(115.0), ..default() }
                                };
                                r.spawn((node, children![(Text::new(cell.clone()), TextFont::from_font_size(13.5), TextColor(color))]));
                            }
                        });
                    };
                    c.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                        line(t, header, None, MUTED, Color::srgba(1.0, 1.0, 1.0, 0.06));
                        for (i, row) in rows.iter().enumerate() {
                            let bg = if i % 2 == 1 { Color::srgba(1.0, 1.0, 1.0, 0.03) } else { Color::NONE };
                            line(t, row, icons[i], TEXT, bg);
                        }
                    });
                }
            }
        }
    });
}

/// Molette sur la fenêtre : fait défiler l'article.
fn scroll_wiki(
    state: Res<WikiState>,
    scroll: Res<AccumulatedMouseScroll>,
    mut content: Single<&mut ScrollPosition, With<WikiContent>>,
) {
    if !state.open || scroll.delta.y == 0.0 {
        return;
    }
    let step = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y * 40.0,
        MouseScrollUnit::Pixel => scroll.delta.y,
    };
    content.0.y = (content.0.y - step).max(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toutes_les_rubriques_sont_remplies() {
        let all: Vec<Article> = articles().into_iter().chain([debug_article()]).collect();
        let titles: Vec<&str> = all.iter().map(|a| a.title).collect();
        for t in ["Cases et terrain", "Climat", "Unités", "Mécaniques", "Villes", "Villages tribaux", "Barbares", "Console de debug"] {
            assert!(titles.contains(&t), "rubrique manquante : {t}");
        }
        assert!(all.iter().all(|a| a.blocks.len() >= 3));
        // Chaque unité a sa fiche, chaque terrain sa ligne
        let by_title = |t: &str| all.iter().find(|a| a.title == t).unwrap();
        let units = by_title("Unités");
        for k in UnitKind::ALL {
            assert!(units.blocks.contains(&Block::Heading(k.name().to_string(), Some(Icon::Unit(k.icon())))), "{}", k.name());
        }
        let Block::Table(_, rows, icons) = &by_title("Cases et terrain").blocks[2] else { panic!("tableau des terrains attendu") };
        assert!(icons.iter().all(Option::is_some), "une pastille par terrain");
        assert_eq!(rows.len(), Terrain::ALL.len());
        // Tableaux bien formés
        for a in &all {
            for b in &a.blocks {
                if let Block::Table(header, rows, icons) = b {
                    assert!(rows.iter().all(|r| r.len() == header.len()), "{}", a.title);
                    assert_eq!(rows.len(), icons.len(), "{}", a.title);
                }
            }
        }
    }
}
