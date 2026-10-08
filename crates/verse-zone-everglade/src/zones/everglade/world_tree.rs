//! Everglade's world tree (`docs/verse/generative-agents.md`, item 3): a
//! pure function from the layout tables to `openagents.verse-world-tree.v1`
//! ([`world_tree::Tree`]).
//!
//! The zone is the root. Under it stand the districts
//! ([`layout::districts`]), and under each its buildings: every doorway of
//! [`layout::doors`] (an open door), every closed front of
//! [`layout::fronts`], and the workshop hall with its yard. Each building
//! has a door object, and the places agents work have rooms and objects:
//! the workshop hall, strongroom, and yard with the Agent Studio's ten
//! [`STATIONS`] and four desks; the owner's great room with Alice's
//! workstation, console, and lectern; the Civic Hall's council chamber;
//! and the Agora's trading floor, Paul's office, and training room. The
//! fixtures of `estate::LIGHTS`, `civic::LIGHTS`, and `agora::LIGHTS` are
//! lamp objects. The Pylon Field in the wilds ([`layout::pylon_field`])
//! holds the Wellspring and one pylon object per site; their states come
//! from the zone's compute source (`zones::everglade::compute`). Every node's standing point is a point navigation reaches
//! from the approach; the tests route to each one.
//!
//! The generated tree is checked in as `crates/world-tree/data/everglade.json`,
//! so `coder` reads it without linking a renderer. A test here fails when
//! the snapshot is stale, and `WORLD_TREE_WRITE=1 cargo test -p
//! verse-zone-everglade world_tree` rewrites it. [`conditions`] derives
//! object states from the town clock and the studio snapshot, and
//! [`perceive`] fills an agent's known subgraph by sight and routes to a
//! node.

pub mod conditions;
mod interiors;
pub mod perceive;

use std::collections::BTreeMap;

use world_tree::{Affordance, Kind, Node, Object, Tree, slug};

use super::layout::districts::{self, District};
use super::layout::estate::{Fixture, OWNERS_HOUSE};
use super::layout::generated::Instance;
use super::layout::pylon_field::Field;
use super::layout::{self, agora, civic, estate};
use super::{STATIONS, Station};

/// The zone's root ID.
pub const ZONE: &str = "everglade";
/// The workshop hall's name, a building of the Commons.
pub const WORKSHOP: &str = "workshop hall";
/// The Pylon Field's name, a place in the wilds.
pub const PYLON_FIELD: &str = "pylon field";

/// The layout tables the tree is made from. [`Tables::everglade`] reads
/// the zone's; a test changes one to see the digest follow.
#[derive(Clone, Debug)]
pub struct Tables {
    pub stations: Vec<Station>,
    /// Each studio desk's standing point.
    pub desk_seats: Vec<[f32; 2]>,
    /// [`layout::doors`]: name, a point outside, a point inside.
    pub doors: Vec<(&'static str, [f32; 2], [f32; 2])>,
    /// [`layout::fronts`]: name and the step outside a closed door.
    pub fronts: Vec<(&'static str, [f32; 2])>,
    /// Each lit building's instance and its fixtures, in its frame.
    pub lights: Vec<(Instance, Vec<(Fixture, [f32; 3])>)>,
    /// The Pylon Field, when the woods had room for it.
    pub field: Option<Field>,
}

impl Tables {
    /// Everglade's tables.
    #[must_use]
    pub fn everglade() -> Self {
        Self {
            stations: STATIONS.to_vec(),
            desk_seats: layout::DESKS.iter().map(|d| d.seat).collect(),
            doors: layout::doors(),
            fronts: layout::fronts(),
            lights: vec![
                (OWNERS_HOUSE, estate::LIGHTS.to_vec()),
                (civic::CIVIC, civic::LIGHTS.to_vec()),
                (agora::AGORA, agora::LIGHTS.to_vec()),
            ],
            field: layout::pylon_field::site().cloned(),
        }
    }
}

/// Everglade's tree from its own tables.
///
/// # Panics
///
/// When the layout's tables don't make a tree, which the tests refuse.
#[must_use]
pub fn everglade() -> Tree {
    generate(&Tables::everglade()).expect("Everglade's layout makes a world tree")
}

/// To the centimeter, so the snapshot reads cleanly and a float's last bit
/// never moves the digest.
fn cm(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn at(p: [f32; 2]) -> [f32; 2] {
    [cm(p[0]), cm(p[1])]
}

/// The heading from `from` toward `to`, as the controller's yaw.
fn heading(from: [f32; 2], to: [f32; 2]) -> f32 {
    cm((to[0] - from[0]).atan2(to[1] - from[1]))
}

/// A box in an instance's frame, x and z ranges, as a world box.
fn area(instance: &Instance, x: [f32; 2], z: [f32; 2]) -> [[f32; 2]; 2] {
    let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for corner in [[x[0], z[0]], [x[1], z[0]], [x[1], z[1]], [x[0], z[1]]] {
        let [wx, wz] = instance.world(corner);
        min = [min[0].min(wx), min[1].min(wz)];
        max = [max[0].max(wx), max[1].max(wz)];
    }
    [at(min), at(max)]
}

/// A center-and-half-extents rectangle as a box.
fn rect(([cx, cz], [hx, hz]): ([f32; 2], [f32; 2])) -> [[f32; 2]; 2] {
    [at([cx - hx, cz - hz]), at([cx + hx, cz + hz])]
}

/// What a building offers, from its name.
#[must_use]
pub fn affordances(name: &str) -> Vec<Affordance> {
    use Affordance::*;
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    let out: &[Affordance] = if has(&["bakery", "bakehouse"]) {
        &[BuyBread, Eat]
    } else if has(&["cafe", "tea house"]) {
        &[Eat, Drink]
    } else if has(&[
        "the lantern",
        "the fiddle",
        "the hearth",
        "the snug",
        "the lamplighter",
    ]) {
        &[Drink, Eat, Gather]
    } else if has(&["bookshop"]) {
        &[Shop, Read]
    } else if has(&[
        "grocer",
        "cheesemonger",
        "corner shop",
        "market hall",
        "tailor",
        "hardware",
        "music shop",
        "print shop",
    ]) {
        &[Shop]
    } else if has(&[
        "writing cabin",
        "code cabin",
        "sketch cabin",
        "prototype shed",
    ]) {
        &[Work]
    } else if has(&[
        "home",
        "townhouse",
        "brownstone",
        "row house",
        "cottage",
        "farmhouse",
        "cabin",
    ]) {
        &[Sleep]
    } else if has(&["agora"]) {
        &[Work, Sell, Gather]
    } else if has(&[
        "workshop",
        "forge",
        "smithy",
        "fab hall",
        "server barn",
        "makers hall",
        "studio",
        "atelier",
        "pottery",
        "barn",
        "windmill",
        "glasshouse",
        "beekeeper",
    ]) {
        &[Work]
    } else if has(&[
        "stacks",
        "archive",
        "reading room",
        "scriptorium",
        "map room",
        "college",
        "lecture hall",
        "seminar",
        "observatory",
    ]) {
        &[Read]
    } else if has(&["chapel"]) {
        &[Pray]
    } else if has(&[
        "meeting hall",
        "guild hall",
        "civic hall",
        "music hall",
        "choir house",
        "bandshell",
        "gazebo",
    ]) {
        &[Gather]
    } else if has(&[
        "fountain",
        "well house",
        "belvedere",
        "lookout",
        "boathouse",
    ]) {
        &[Rest]
    } else {
        &[]
    };
    out.to_vec()
}

/// The nodes as they are made, each after its parent.
struct Out {
    nodes: Vec<Node>,
}

impl Out {
    /// Adds a node under `parent` and returns its ID.
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        parent: &str,
        name: &str,
        kind: Kind,
        district: Option<&str>,
        stand: [f32; 2],
        source: String,
        fill: impl FnOnce(&mut Node),
    ) -> String {
        let id = format!("{parent}/{}", slug(name));
        let mut node = Node {
            id: id.clone(),
            name: name.to_owned(),
            kind,
            object: None,
            district: district.map(str::to_owned),
            stand: at(stand),
            facing: None,
            area: None,
            entry: None,
            affordances: vec![],
            exclusive: false,
            open: None,
            source,
        };
        fill(&mut node);
        self.nodes.push(node);
        id
    }

    /// Adds an object.
    #[allow(clippy::too_many_arguments)]
    fn object(
        &mut self,
        parent: &str,
        district: &str,
        name: &str,
        object: Object,
        stand: ([f32; 2], Option<f32>),
        affordances: &[Affordance],
        exclusive: bool,
        source: String,
    ) -> String {
        self.add(
            parent,
            name,
            Kind::Object,
            Some(district),
            stand.0,
            source,
            |n| {
                n.object = Some(object);
                n.facing = stand.1.map(cm);
                n.affordances = affordances.to_vec();
                n.exclusive = exclusive;
            },
        )
    }

    /// Adds a room with its floor.
    fn room(
        &mut self,
        parent: &str,
        district: &str,
        name: &str,
        stand: [f32; 2],
        floor: [[f32; 2]; 2],
        source: String,
    ) -> String {
        self.add(
            parent,
            name,
            Kind::Room,
            Some(district),
            stand,
            source,
            |n| n.area = Some(floor),
        )
    }
}

/// One building before it is placed under its district.
struct Site {
    name: &'static str,
    district: District,
    /// Where it is entered from: a point outside its door.
    stand: [f32; 2],
    /// Its door: a point inside when it opens, and its source.
    inside: Option<[f32; 2]>,
    /// Whether it has a door at all: an open place such as the Pylon
    /// Field has none.
    door: bool,
    source: String,
}

/// The tree made from `tables`.
///
/// # Errors
///
/// When a building has no district, a station the tree needs is missing,
/// or the nodes don't make a tree.
pub fn generate(tables: &Tables) -> Result<Tree, String> {
    let station = |id: &str| {
        tables
            .stations
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("no station {id}"))
    };
    let approach = station("approach")?;
    let mut sites = vec![Site {
        name: WORKSHOP,
        district: District::Commons,
        stand: [0.0, -14.5],
        inside: Some([0.0, 3.0]),
        door: true,
        source: format!("building:{WORKSHOP}"),
    }];
    for &(name, outside, inside) in &tables.doors {
        sites.push(Site {
            name,
            district: district_of(name)?,
            stand: outside,
            inside: Some(inside),
            door: true,
            source: format!("door:{name}"),
        });
    }
    for &(name, front) in &tables.fronts {
        sites.push(Site {
            name,
            district: district_of(name)?,
            stand: front,
            inside: None,
            door: true,
            source: format!("front:{name}"),
        });
    }
    if let Some(field) = &tables.field {
        sites.push(Site {
            name: PYLON_FIELD,
            district: District::Wilds,
            stand: field.stand(),
            inside: None,
            door: false,
            source: "pylon-field".into(),
        });
    }
    let mut by_district: BTreeMap<District, Vec<Site>> = BTreeMap::new();
    for site in sites {
        by_district.entry(site.district).or_default().push(site);
    }

    let mut out = Out { nodes: Vec::new() };
    out.nodes.push(Node {
        id: ZONE.into(),
        name: "Everglade".into(),
        kind: Kind::Zone,
        object: None,
        district: None,
        stand: at(approach.at),
        facing: Some(cm(approach.facing)),
        area: None,
        entry: None,
        affordances: vec![],
        exclusive: false,
        open: None,
        source: format!("zone:{ZONE}"),
    });
    for district in District::ALL {
        let Some(mut sites) = by_district.remove(&district) else {
            continue;
        };
        sites.sort_by_key(|s| slug(s.name));
        let d = district.slug();
        let stand = middle_stand(&sites);
        out.add(
            ZONE,
            district.name(),
            Kind::District,
            Some(d),
            stand,
            format!("district:{d}"),
            |n| n.id = format!("{ZONE}/{d}"),
        );
        let district_id = format!("{ZONE}/{d}");
        for site in &sites {
            let id = out.add(
                &district_id,
                site.name,
                Kind::Building,
                Some(d),
                site.stand,
                site.source.clone(),
                |n| {
                    n.affordances = affordances(site.name);
                    // The workshop's doorways line up with navigation's
                    // grid; every other open door is crossed straight.
                    if site.name != WORKSHOP {
                        n.entry = site.inside.map(|inside| [at(site.stand), at(inside)]);
                    }
                },
            );
            if !site.door {
                if let (PYLON_FIELD, Some(field)) = (site.name, &tables.field) {
                    interiors::pylon_field(&mut out, field, &id, d);
                }
                continue;
            }
            let door_source = format!("{}:door", site.source);
            let facing = site.inside.map(|inside| heading(site.stand, inside));
            let door_stand = if site.name == WORKSHOP {
                [0.0, -2.0]
            } else {
                site.stand
            };
            out.add(
                &id,
                "door",
                Kind::Object,
                Some(d),
                door_stand,
                door_source,
                |n| {
                    n.object = Some(Object::Door);
                    n.open = Some(site.inside.is_some());
                    n.facing = facing;
                },
            );
            match site.name {
                WORKSHOP => interiors::workshop(&mut out, tables, &id, d)?,
                "owner's house" => interiors::owners_house(&mut out, &id, d),
                "civic hall" => interiors::civic_hall(&mut out, &id, d),
                "agora" => interiors::agora_hall(&mut out, &id, d),
                _ => {}
            }
            interiors::lamps(&mut out, tables, site.name, &id, d);
        }
    }
    if let Some(district) = by_district.keys().next() {
        return Err(format!("{} isn't in District::ALL", district.name()));
    }
    Tree::new(ZONE, out.nodes).map_err(|e| e.to_string())
}

fn district_of(name: &str) -> Result<District, String> {
    districts::of_instance(name).ok_or_else(|| format!("{name} has no district"))
}

/// A district's standing point: its building nearest the middle of them.
fn middle_stand(sites: &[Site]) -> [f32; 2] {
    let n = sites.len() as f32;
    let mid = sites.iter().fold([0.0, 0.0], |acc, s| {
        [acc[0] + s.stand[0] / n, acc[1] + s.stand[1] / n]
    });
    sites
        .iter()
        .map(|s| s.stand)
        .min_by(|a, b| {
            let da = (a[0] - mid[0]).hypot(a[1] - mid[1]);
            let db = (b[0] - mid[0]).hypot(b[1] - mid[1]);
            da.total_cmp(&db)
        })
        .unwrap_or(mid)
}

#[cfg(test)]
mod tests;
