//! Wiki du jeu : contenu (construit à partir des données et des règles, il reste donc à jour
//! quand on change un chiffre) et fenêtre d'affichage (bouton « Wiki » ou touche F1).

use crate::combat::{self, CHARGE_BONUS, FLANK_BONUS, HIGHER_GROUND_DEFENSE, HIGH_GROUND_PERCENT, TERRAIN_DEFENSE};
use crate::hostile::{CAMP_CAPTURE_GOLD, CAMP_MAX_UNITS, CAMP_SPAWN_INTERVAL, HUNT_RANGE, WANDER_RADIUS, WOUNDED_HP};
use crate::map::{
    CLIMB_GENTLE_M, CLIMB_STEEP_M, HILL_MOVE_COST, HILL_PRODUCTION, MOUNTAIN_MOVE_COST, MOUNTAIN_YIELDS, RUGGED_MOVE_COST, Relief, Terrain,
    UNKNOWN_MOVE_COST,
};
use crate::rules::{DETOUR_TOLERANCE_PERCENT, MAX_HP, MIN_CITY_DISTANCE, growth_threshold};
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

fn waters() -> Article {
    use crate::hydro::{FLEUVE_M3S, RIVER_M3S, SMALL_LAKE_KM2};
    use crate::map::{FLEUVE_MOVE_COST, FLEUVE_YIELDS, FRESH_WATER_FOOD, RIVER_CROSSING_COST};
    use crate::combat::RIVER_DEFENSE;
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
                "Les fleuves portent le nom d'un grand fleuve réel qui coule à une latitude voisine : les plus \
                 abondants reçoivent les plus grands noms (Amazone sous l'équateur, Mississippi ou Danube aux latitudes \
                 moyennes, Ienisseï ou Léna dans le Grand Nord). Chaque affluent porte son propre nom (un long affluent \
                 compte au moins {} cases de fleuve), et les rivières prennent des noms de rivières réelles plus modestes \
                 (Garonne, Tage, Ohio…) ; un nom peut resservir loin de son premier emploi. Le nom apparaît sur la carte une fois une partie de son cours découverte, près de son \
                 milieu ; de loin, seuls les grands fleuves sont nommés (en plus gros), les longs affluents apparaissent \
                 en s'approchant, toutes les rivières en zoom fin, et un nom qui en chevaucherait un plus important est décalé le long du cours ou masqué.",
                crate::basins::MIN_NAMED_CELLS
            )),
            p("Un fleuve garde son nom de l'embouchure à sa source, à travers les petits lacs d'une ou deux cases ; ses \
               petits bras portent le nom du cours où ils se jettent. Les étendues d'eau portent le nom d'une étendue \
               réelle de latitude voisine, selon leur nature : un lac d'eau douce se déverse dans un cours d'eau (lac \
               Supérieur, Victoria, Baïkal…) ; un lac salé n'a pas d'issue, l'eau s'en évapore (lac Tchad, Grand Lac \
               Salé…) ; une mer intérieure est une étendue de mer enclavée dans les terres (mer Caspienne, mer d'Aral…). \
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
                 douce : +{FRESH_WATER_FOOD} nourriture. Un fleuve rapporte {} nourriture, {} production et {} or \
                 (pêche, transport) ; on ne fonde pas de ville dans son lit, mais sur une rive. Les villes pourront plus \
                 tard y aménager des ponts et des moulins à eau.",
                FLEUVE_YIELDS.food, FLEUVE_YIELDS.production, FLEUVE_YIELDS.gold
            )),
        ],
    }
}

fn climate() -> Article {
    use crate::map::{
        ARID_BELOW, COLD_WINTER, DESERT_ABOVE, FOREST_ABOVE, JUNGLE_WET, SEMI_ARID_BELOW, SEMI_ARID_WOODLAND, SNOW_BELOW,
        TAIGA_ABOVE, TROPICAL_WINTER, TUNDRA_BELOW,
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
        row(Terrain::Grassland, "tempéré".into(), format!("sous {FOREST_ABOVE}")),
        row(Terrain::Forest, "tempéré".into(), format!("{FOREST_ABOVE} et plus (humide)")),
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
               d'altitude (les terres vont du niveau de la mer à près de 4 800 m). Les \
               courants marins modifient les côtes : eau froide le long des côtes ouest vers 15 à 35° de latitude, eau \
               chaude le long des côtes est, dérive chaude qui adoucit les côtes ouest des hautes latitudes. L'écart \
               entre le jour et la nuit est faible près de la mer et grand dans les régions sèches."),
            h("Vents"),
            p("Le vent naît des différences de pression : ceintures alternées de hautes et basses pressions, effet des \
               continents (une terre chaude attire l'air, une terre froide le repousse), dépressions et anticyclones \
               mobiles qui changent tous les dix jours simulés. L'air va des hautes vers les basses pressions en \
               étant fortement dévié par la rotation de la planète (vers la droite au nord, vers la gauche au sud). En \
               altitude, les vents soufflent surtout de l'ouest. Les grandes cellules de circulation font descendre \
               l'air vers 30° de latitude (anticyclones) et monter à l'équateur et vers 60°. L'air qui converge \
               quelque part doit monter, celui qui diverge est remplacé par de l'air qui descend."),
            table_icons(&["Latitude", "Effet", "Circulation"], belts),
            h("L'air en deux couches"),
            p("La couche basse (jusqu'à 2 km au-dessus du sol) suit le relief et porte l'essentiel de la vapeur ; la \
               couche haute est bien plus froide et peut en contenir beaucoup moins (la capacité de l'air augmente \
               d'environ 7 % par degré). Il pleut quand l'air humide monte vers le froid et sature : là où les vents \
               convergent, sous les fronts des perturbations, dans les orages des tropiques, ou quand le relief force \
               l'air à s'élever. Une partie des gouttes s'évapore en tombant si l'air est sec. Là où l'air descend \
               (anticyclones, derrière les massifs), l'air sec d'altitude assèche la couche basse. Sous un \
               anticyclone, l'air chaud qui descend forme un couvercle : les orages sont étouffés et l'air qui monte \
               ne forme que des nuages bas. Un air bas saturé sans être soulevé (air marin poussé sur une côte froide) \
               forme du brouillard, qui bruine à peine."),
            h("Relief"),
            p("Sur un versant exposé au vent, l'air qui monte en suivant le sol se refroidit et la pluie augmente ; elle \
               culmine sur les pentes hautes. L'air épouse un relief adouci (il contourne en partie les sommets) et la \
               neige, qui tombe lentement, est en partie portée au-delà de la crête. Derrière, l'air redescend en se \
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
            (swatch(t.color()), vec![t.name().into(), y.food.to_string(), y.production.to_string(), y.gold.to_string(), moves, defense])
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
                    format!("{MOUNTAIN_MOVE_COST} PM"),
                    format!("{} / {} / {}", MOUNTAIN_YIELDS.food, MOUNTAIN_YIELDS.production, MOUNTAIN_YIELDS.gold),
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
            table_icons(&["Terrain", "Nourriture", "Production", "Or", "Déplacement", "Défense"], rows),
            p("La côte, l'océan et les lacs ne se traversent pas encore (pas d'unités navales), mais une ville \
               peut exploiter leurs rendements. Les rivières, fleuves et lacs sont décrits dans « Cours d'eau et \
               lacs »."),
            h("Relief"),
            p(format!(
                "Le relief est interne à la case et s'ajoute au terrain : une forêt peut être vallonnée, une toundra \
                 montagneuse. Il dépend de la dénivelée avec les cases voisines et de l'altitude. Les coûts d'un relief \
                 vallonné ou accidenté s'ajoutent au terrain sans dépasser {MOUNTAIN_MOVE_COST} PM ; une montagne coûte \
                 toujours {MOUNTAIN_MOVE_COST} PM. Collines, relief accidenté et montagne empêchent la charge de \
                 cavalerie ; leur bonus de défense se cumule avec la forêt, la jungle ou la taïga. Seuls les pics, les \
                 plus hauts sommets, sont infranchissables ; on ne fonde pas de ville en montagne."
            )),
            table_icons(&["Relief", "Déplacement", "Rendements", "Défense"], reliefs),
            h("Altitude"),
            p(format!(
                "Chaque case de terre a une altitude en mètres (carte de l'altitude). Pente : monter de moins de \
                 {CLIMB_GENTLE_M:.0} m vers une case voisine ne coûte rien de plus, jusqu'à {CLIMB_STEEP_M:.0} m +1 PM, \
                 au-delà +2 PM ; descendre est gratuit. Entre deux cases dont l'altitude diffère d'environ 1 000 m ou \
                 plus, une falaise est infranchissable dans les deux sens : on ne peut ni y combattre au corps à corps \
                 ni flanquer à travers. Être plus haut d'au moins {} m donne +{HIGHER_GROUND_DEFENSE} en défense de \
                 mêlée et +{HIGH_GROUND_PERCENT} % de force de tir ; un tireur plus bas d'autant perd 1 de portée.",
                combat::HIGHER_GROUND_M
            )),
            p("Sur la carte : en 2D, une ombre en dégradé marque une descente et une arête sombre à dents de hachure \
               marque une falaise ; en 3D, le sol est à sa hauteur réelle en mètres, les falaises sont des parois rocheuses, \
               et le relief des cases (collines, montagnes, pics) est modelé dans le sol : crêtes et ravines continues \
               d'une case à l'autre, roche sur les pentes raides, neige en hauteur (plus bas dans le froid), arbres \
               sur les versants jusqu'à la limite des arbres. Les cours d'eau coulent dans des vallées. Les côtes \
               basses descendent en plage jusqu'à la mer, les plus hautes tombent en falaise."),
            h("Plaques tectoniques"),
            p("La planète est découpée en une douzaine de plaques (carte des plaques, touche T) : continentales \
               (teintes chaudes, épaisses, elles portent les continents) ou océaniques (teintes froides). Chacune tourne \
               autour de son propre pôle de rotation, à 1 à 9 cm par an (flèches) : sa vitesse change d'un endroit \
               à l'autre. Les plaques sont calculées sur la sphère : près des pôles, la carte les étire en largeur, \
               comme un planisphère. Le long d'une frontière, deux plaques qui se rapprochent \
               forment une collision (deux continents : grande chaîne de montagnes et haut plateau), une \
               subduction (l'océan plonge sous le continent : chaîne côtière, fosse au large) ou un arc d'îles \
               (deux océans) ; deux plaques qui s'écartent ouvrent un rift continental ou une dorsale ; deux \
               plaques qui glissent l'une contre l'autre forment une faille transformante. Les points chauds, loin \
               des frontières, sont des volcans isolés. Pour l'instant, les plaques sont seulement décrites : le \
               relief et les continents en découleront."),
            h("Brouillard de guerre"),
            p("Une case inexplorée est noire. Une fois découverte, elle reste connue mais apparaît assombrie hors de \
               vue : on y voit le terrain, pas les unités ennemies. Chaque unité voit à 2 cases (3 pour l'éclaireur, \
               la cavalerie légère et l'archer monté)."),
            h("Cartes et vues"),
            p("Le bouton « Carte » (touche T) choisit la carte : normale, de l'altitude (couleur selon les mètres), du \
               relief (plat à pic), du climat (pluies et vents), des bassins versants, des débits des cours d'eau ou du réseau hydrographique. \
               Le bouton « Vue 3D » (touche V) passe en vue 3D ; la carte choisie s'applique aux deux vues. Le bouton \
               « Options de carte » (touche O) règle les noms affichés (cours d'eau, lacs, villes ; la touche N les \
               masque ou les remet tous), la surbrillance des cours d'eau (au survol du cours d'eau, au survol de son \
               étiquette, ou aucune) et les rubriques de l'infobulle, pour chaque carte : par défaut le terrain, le \
               climat, les rendements et le coût de déplacement, plus ce qui concerne la carte affichée (débit sur la \
               carte des débits, bassin et affluents sur celle des bassins versants…)."),
        ],
    }
}

/// Traits particuliers d'une unité, en phrases.
fn unit_traits(k: UnitKind) -> Vec<String> {
    let mut traits = Vec::new();
    match k {
        UnitKind::Settler => traits.push(
            "Fonde une ville (touche B). Le produire coûte 1 habitant à la ville, qui doit en avoir au moins 2. \
             Ne peut pas attaquer."
                .to_string(),
        ),
        UnitKind::Scout => {
            traits.push("Rapide et voit loin : idéal pour l'exploration automatique (touche E). Combat très faiblement.".into())
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
                 case. Un ordre (clic droit) peut durer plusieurs tours : le numéro du tour d'arrivée s'affiche le long \
                 du trajet. Les trajets ne s'appuient que sur le terrain connu ; une case inexplorée est supposée \
                 franchissable mais coûte {UNKNOWN_MOVE_COST} PM. L'unité recalcule son trajet en découvrant le terrain ; \
                 l'ordre est suspendu s'il devient plus de {DETOUR_TOLERANCE_PERCENT} % plus long que prévu."
            )),
            h("Une unité par case"),
            p("Une case ne peut accueillir qu'une unité. Vos unités se contournent ; un clic droit sur une de vos \
               unités adjacentes échange leurs places (sauf à travers une falaise)."),
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
                 À 0 PV, l'unité est dispersée.",
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
        ],
    }
}

fn cities() -> Article {
    let thresholds = (1..=6).map(|pop| vec![pop.to_string(), growth_threshold(pop).to_string()]).collect();
    Article {
        title: "Villes",
        icon: "icons/wiki_cities.png",
        blocks: vec![
            h("Fonder une ville"),
            p(format!(
                "Un colon fonde une ville sur sa case (touche B), sur n'importe quel terrain franchissable, à {MIN_CITY_DISTANCE} \
                 cases au moins d'une autre ville. Elle est posée sur une terrasse en 3D."
            )),
            h("Rendements"),
            p("La case centrale rapporte au moins 2 nourriture, 2 production et 1 or. Chaque habitant exploite en \
               plus une case du territoire, choisie automatiquement parmi les meilleures."),
            h("Croissance"),
            p("Chaque habitant consomme 2 nourriture. Le surplus s'accumule ; quand le stock atteint le seuil, la \
               ville gagne un habitant. En cas de famine, elle en perd un."),
            table(&["Population", "Nourriture pour grandir"], thresholds),
            h("Territoire"),
            p("Le territoire s'étend à 1 case autour de la ville, puis à 2 cases à partir de 3 habitants. Il sert \
               aux cases exploitées et aux soins (+20 PV par tour en territoire ami)."),
            h("Production"),
            p("La production s'accumule pour l'unité choisie dans le panneau de la ville. L'unité apparaît sur la \
               ville ou sur une case voisine libre ; si tout est occupé, la production attend. Un colon coûte en plus \
               1 habitant."),
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
                 {VILLAGE_MIN_FROM_START} cases au moins du départ). Ils apparaissent quand leur case est explorée."
            )),
            h("Visiter un village"),
            p("La première unité qui entre dans un village déclenche un événement, puis le village disparaît :"),
            table(
                &["Événement", "Chance", "Effet"],
                vec![
                    vec!["Les habitants vous rejoignent".into(), "35 %".into(), "+1 habitant dans la ville la plus proche".into()],
                    vec!["Échanges commerciaux".into(), "45 %".into(), "+15 à 35 or".into()],
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
                 guerriers par camp. Un camp vide est dispersé quand une de vos unités y entre : +{CAMP_CAPTURE_GOLD} or."
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

#[derive(Resource)]
struct WikiArticles(Vec<Article>);

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
            .insert_resource(WikiArticles(articles()))
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
        articles.0.iter().enumerate().map(|(i, a)| (i, a.title, a.icon)).collect();
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
            Interaction::default(),
        ))
        .with_children(|w| {
            // Barre de titre
            w.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() })
                .with_children(|bar| {
                    bar.spawn(label("Wiki de CivBuilder", 22.0, TEXT));
                    bar.spawn((
                        WikiClose,
                        Button,
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
                                Button,
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
    open_button: Query<Ref<Interaction>, With<WikiButton>>,
    close_button: Query<Ref<Interaction>, With<WikiClose>>,
    mut tabs: Query<(Ref<Interaction>, &WikiTab, &mut BackgroundColor), Without<WikiClose>>,
    mut close_bg: Query<&mut BackgroundColor, (With<WikiClose>, Without<WikiTab>)>,
) {
    let pressed = |i: &Ref<Interaction>| i.is_changed() && **i == Interaction::Pressed;
    // Pas de wiki par-dessus le menu Échap
    if !menu.open && (keys.just_pressed(KeyCode::F1) || open_button.iter().any(|i| pressed(&i))) {
        state.open = !state.open;
    }
    if state.open && (keys.just_pressed(KeyCode::Escape) || close_button.iter().any(|i| pressed(&i))) {
        state.open = false;
    }
    for (interaction, tab, mut bg) in &mut tabs {
        if pressed(&interaction) {
            state.article = tab.0;
        }
        let color = if state.article == tab.0 {
            TAB_ACTIVE
        } else if *interaction == Interaction::Hovered {
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
            let color = if *i == Interaction::None { TAB_BG } else { TAB_HOVER };
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
    let Some(article) = articles.0.get(state.article) else { return };
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
        let all = articles();
        let titles: Vec<&str> = all.iter().map(|a| a.title).collect();
        for t in ["Cases et terrain", "Climat", "Unités", "Mécaniques", "Villes", "Villages tribaux", "Barbares"] {
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
