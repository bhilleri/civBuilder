//! Carte géographique (logique pure) : le découpage du territoire en zones nommées, une famille
//! à la fois (terres, mers, relief, régions naturelles), à deux niveaux.
//!
//! - **Niveau du dessus** (vue lointaine) : les grandes zones (continents, îles et archipels ;
//!   océans et mers ; chaînes, plateaux et collines ; régions naturelles), une couleur chacune.
//! - **Niveau du dessous** (vue proche) : leurs parties (péninsules, isthmes, îles d'un archipel ;
//!   golfes, baies, détroits ; massifs, cols ; sous-régions), une couleur chacune ; le reste de la
//!   grande zone garde sa couleur, en teinte sourde.
//! - **Couleurs** : deux zones voisines n'ont jamais la même (coloriage de graphe « DSatur » : la
//!   zone qui a déjà le plus de couleurs différentes autour d'elle choisit d'abord).

use crate::hex::{DIRECTIONS, Hex};
use crate::map::WorldMap;
use bevy::platform::collections::{HashMap, HashSet};

/// Famille de zones montrée par la carte géographique.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GeoFamily {
    #[default]
    Lands,
    Seas,
    Reliefs,
    Regions,
}

impl GeoFamily {
    pub const ALL: [GeoFamily; 4] = [GeoFamily::Lands, GeoFamily::Seas, GeoFamily::Reliefs, GeoFamily::Regions];

    pub fn label(self) -> &'static str {
        match self {
            GeoFamily::Lands => "Terres",
            GeoFamily::Seas => "Mers",
            GeoFamily::Reliefs => "Relief",
            GeoFamily::Regions => "Régions naturelles",
        }
    }
}

/// Nombre de couleurs de zones.
pub const GEO_COLORS: usize = 10;

/// Teinte d'une case sur la carte géographique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoShade {
    /// Dans une zone montrée : sa couleur ; `muted` : la partie d'une grande zone hors de ses
    /// sous-zones (niveau du dessous).
    Zone { color: u8, muted: bool },
    /// Hors des zones de la famille.
    Land,
    Water,
}

/// Teintes de la carte géographique, et grande zone de chaque case (pour tracer leurs limites au
/// niveau du dessous).
#[derive(Clone, Debug, Default)]
pub struct GeoMap {
    pub shade: Vec<GeoShade>,
    pub top: Vec<Option<usize>>,
}

/// Grande zone et zone montrée d'une case (indices dans les zones de la famille), et si la zone
/// montrée est la grande zone, en teinte sourde, faute de sous-zone.
fn zone_of(map: &WorldMap, family: GeoFamily, deep: bool, c: usize) -> (Option<usize>, Option<(usize, bool)>) {
    let at = |v: &[Option<u16>]| v.get(c).copied().flatten().map(usize::from);
    // (grande zone, sous-zone ; une grande zone sans partie n'est pas en teinte sourde)
    let (top, sub, has_parts): (Option<usize>, Option<usize>, bool) = match family {
        GeoFamily::Lands => {
            let l = map.lands();
            let top = at(&l.group_of).or(at(&l.mass_of));
            // (dans un archipel, ses îles sont ses parties)
            let sub = at(&l.isthmus_of).or(at(&l.part_of)).or(at(&l.group_of).and(at(&l.mass_of)));
            (top, sub, top.is_some_and(|t| l.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Seas => {
            let s = map.seas();
            let top = at(&s.coastal_of).or(at(&s.basin_of));
            let sub = at(&s.strait_of).or(at(&s.bay_of)).or(at(&s.inlet_of));
            (top, sub, top.is_some_and(|t| s.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Reliefs => {
            let r = map.reliefs();
            let top = at(&r.range_of).or(at(&r.plateau_of)).or(at(&r.hills_of));
            let sub = at(&r.pass_of).or(at(&r.massif_of));
            (top, sub, top.is_some_and(|t| r.zones.iter().any(|z| z.parent == Some(t))))
        }
        GeoFamily::Regions => {
            let r = map.regions();
            let top = at(&r.region_of);
            let sub = at(&r.part_of);
            (top, sub, top.is_some_and(|t| r.zones.iter().any(|z| z.parent == Some(t))))
        }
    };
    let shown = match (deep, sub) {
        (true, Some(s)) => Some((s, false)),
        (true, None) => top.map(|t| (t, has_parts)),
        (false, _) => top.map(|t| (t, false)),
    };
    (top, shown)
}

/// Carte géographique d'une famille, au niveau du dessus ou du dessous (`deep`).
pub fn compute(map: &WorldMap, family: GeoFamily, deep: bool) -> GeoMap {
    let n = (map.width * map.height) as usize;
    let mut top = vec![None; n];
    let mut shown: Vec<Option<(usize, bool)>> = vec![None; n];
    for c in 0..n {
        (top[c], shown[c]) = zone_of(map, family, deep, c);
    }
    // Zones voisines (deux cases voisines dans deux zones montrées différentes)
    let mut size: HashMap<usize, usize> = HashMap::new();
    let mut touching: HashMap<usize, HashSet<usize>> = HashMap::new();
    for c in 0..n {
        let Some((z, _)) = shown[c] else { continue };
        *size.entry(z).or_default() += 1;
        let h = map.hex_at(c);
        for d in DIRECTIONS {
            let nb = map.canonical(h + d);
            if !map.contains(nb) {
                continue;
            }
            if let Some((o, _)) = shown[map.cell(nb)]
                && o != z
            {
                touching.entry(z).or_default().insert(o);
                touching.entry(o).or_default().insert(z);
            }
        }
    }
    let colors = color_zones(&size, &touching);
    let shade = (0..n)
        .map(|c| match shown[c] {
            Some((z, muted)) => GeoShade::Zone { color: colors[&z], muted },
            None if map.get(map.hex_at(c)).is_some_and(|t| t.is_water()) => GeoShade::Water,
            None => GeoShade::Land,
        })
        .collect();
    GeoMap { shade, top }
}

/// Coloriage des zones (DSatur) : à chaque pas, la zone qui touche le plus de couleurs déjà
/// données (puis la plus grande) prend, parmi les couleurs libres autour d'elle, la moins donnée
/// jusque-là (des zones qui ne se touchent pas, des îles, n'ont pas toutes la même) ; s'il n'y en
/// a pas, la moins présente autour d'elle.
fn color_zones(size: &HashMap<usize, usize>, touching: &HashMap<usize, HashSet<usize>>) -> HashMap<usize, u8> {
    let mut colors: HashMap<usize, u8> = HashMap::new();
    let mut given = [0usize; GEO_COLORS];
    let mut left: Vec<usize> = size.keys().copied().collect();
    left.sort_unstable();
    let empty = HashSet::new();
    while !left.is_empty() {
        let around = |z: usize, colors: &HashMap<usize, u8>| -> Vec<u8> { touching.get(&z).unwrap_or(&empty).iter().filter_map(|o| colors.get(o).copied()).collect() };
        let saturation = |z: usize| around(z, &colors).into_iter().collect::<HashSet<u8>>().len();
        let (k, &z) = left.iter().enumerate().max_by_key(|&(_, &z)| (saturation(z), size[&z], std::cmp::Reverse(z))).unwrap();
        left.swap_remove(k);
        let used = around(z, &colors);
        let color = (0..GEO_COLORS as u8)
            .filter(|c| !used.contains(c))
            .min_by_key(|&c| (given[usize::from(c)], c))
            .unwrap_or_else(|| (0..GEO_COLORS as u8).min_by_key(|c| used.iter().filter(|u| *u == c).count()).unwrap_or(0));
        given[usize::from(color)] += 1;
        colors.insert(z, color);
    }
    colors
}

/// Limites des grandes zones (niveau du dessous) : arêtes entre deux cases explorées de grandes
/// zones différentes, ou entre une grande zone et le dehors (monde 2D) ; pas les côtes, déjà
/// visibles (une case de terre et une d'eau).
pub fn top_borders(map: &WorldMap, geo: &GeoMap, explored: impl Fn(Hex) -> bool) -> Vec<[bevy::math::Vec2; 2]> {
    let mut edges = Vec::new();
    for (c, &zone) in geo.top.iter().enumerate() {
        let Some(zone) = zone else { continue };
        let hex = map.hex_at(c);
        if !explored(hex) {
            continue;
        }
        for (d, &step) in DIRECTIONS.iter().enumerate() {
            let nb = map.canonical(hex + step);
            if map.contains(nb) && geo.top[map.cell(nb)] == Some(zone) {
                continue;
            }
            let wet = |h: Hex| map.get(h).is_some_and(|t| t.is_water());
            if map.contains(nb) && wet(hex) != wet(nb) {
                continue;
            }
            // (chaque arête une fois : du côté de la plus petite zone, ou de la case de la zone)
            let other = map.contains(nb).then(|| geo.top[map.cell(nb)]).flatten();
            if other.is_some_and(|o| o < zone) && explored(nb) {
                continue;
            }
            let (a, b) = hex.edge_corners(d);
            edges.push([bevy::math::Vec2::from(a), bevy::math::Vec2::from(b)]);
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_zones_voisines_n_ont_pas_la_meme_couleur() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        for family in GeoFamily::ALL {
            for deep in [false, true] {
                let geo = compute(&m, family, deep);
                let mut zones = 0;
                for c in 0..geo.shade.len() {
                    let GeoShade::Zone { color, .. } = geo.shade[c] else { continue };
                    zones += 1;
                    let (_, mine) = zone_of(&m, family, deep, c);
                    for d in DIRECTIONS {
                        let nb = m.canonical(m.hex_at(c) + d);
                        if !m.contains(nb) {
                            continue;
                        }
                        let j = m.cell(nb);
                        let (_, theirs) = zone_of(&m, family, deep, j);
                        if let (GeoShade::Zone { color: other, .. }, true) = (geo.shade[j], mine.map(|z| z.0) != theirs.map(|z| z.0)) {
                            assert_ne!(color, other, "{family:?} {deep} : cases {c} et {j}");
                        }
                    }
                }
                assert!(zones > 0, "{family:?} {deep} : aucune zone");
            }
        }
    }

    #[test]
    fn niveau_du_dessous_les_parties_et_la_grande_zone_en_sourdine() {
        let m = WorldMap::generate_in(96, 56, 777, crate::climate::Region::planet(crate::climate::CELL_KM, 56));
        let geo = compute(&m, GeoFamily::Regions, true);
        let r = m.regions();
        // Une case d'une sous-région : pas en sourdine ; d'une région découpée hors de ses
        // sous-régions : impossible (une région découpée l'est entièrement) ; d'une région non
        // découpée : pleine couleur
        for c in 0..geo.shade.len() {
            if let GeoShade::Zone { muted, .. } = geo.shade[c] {
                assert!(!muted || r.part_of[c].is_none(), "case {c}");
            }
        }
        let top = compute(&m, GeoFamily::Regions, false);
        assert!(top.shade.iter().all(|s| !matches!(s, GeoShade::Zone { muted: true, .. })));
    }
}
