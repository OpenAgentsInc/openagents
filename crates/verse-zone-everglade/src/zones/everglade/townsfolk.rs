//! Everglade's townsfolk (`docs/verse/generative-agents.md`, item 5): the
//! admitted villagers, placed from the town clock.
//!
//! The roster (`townsfolk/town.json`) and every definition file
//! (`townsfolk/npcs/*.json`) are compiled into the client, so every device
//! derives the same town: [`roster`] loads the definitions the roster
//! admits by digest against the world tree, and leaves out the rest.
//! [`Townsfolk::tick`] places each villager with the `townsfolk` crate's
//! pure routine at the town time, and walks one between nodes along the
//! route `world_tree::perceive::route` grounds over the zone's blockers,
//! at the progress the routine gives. The zone draws each villager near
//! the player as the pack's character in its tint ([`Townsfolk::figures`]),
//! under a nameplate with its name, what it is doing, and `CHARACTER`
//! ([`Townsfolk::draw`]): townsfolk are labeled characters, never people
//! or working agents.
//!
//! [`Routes`] is the zone's `townsfolk::validate::Router`, which
//! `openagents verse town` and this crate's tests route every leg with.
//! Rumors, villager memory, and dialogue are phase E2's; the
//! `townsfolk::routine::gatherings` query says who stands together.

use std::collections::HashMap;
use std::sync::LazyLock;

use ::townsfolk::routine::{TOWN_PER_REAL, Villager};
use ::townsfolk::validate::Router;
use ::townsfolk::{Activity, Code, Placement, Problem, Roster, Town};
use glam::Vec3;
use town_clock::TownTime;
use world_tree::Tree;

use super::height;
use super::studio::{Attention, Posture, SeatFigure, plate, plate_transform};
use super::world_tree::perceive;
use crate::controller::Footprint;
use crate::mesh::{Mesh, Vertex};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/townsfolk_files.rs"));
}

/// The roster as checked in.
pub const TOWN_JSON: &str = include_str!("../../../townsfolk/town.json");

/// Every definition file compiled in, admitted or not.
#[must_use]
pub fn files() -> &'static [&'static str] {
    embedded::NPC_FILES
}

/// The admitted town against Everglade's tree, and every definition or
/// entry it left out and why.
#[must_use]
pub fn roster() -> &'static (Roster, Vec<Problem>) {
    static ROSTER: LazyLock<(Roster, Vec<Problem>)> = LazyLock::new(|| {
        let tree = world_tree::everglade();
        Roster::load(TOWN_JSON, files(), tree).unwrap_or_else(|why| {
            (
                Roster {
                    town: Town::new(tree.zone(), 0),
                    villagers: Vec::new(),
                },
                vec![Problem::new("town.json", Code::Schema, why)],
            )
        })
    });
    &ROSTER
}

/// The zone's router: a leg's route from one node's standing point to
/// another's over `blockers`, as a villager walks it.
pub struct Routes<'a> {
    pub tree: &'a Tree,
    pub blockers: &'a [Footprint],
}

impl Routes<'_> {
    /// The leg's points, from `from`'s standing point to `to`'s.
    ///
    /// # Errors
    ///
    /// Why there is no route.
    pub fn points(&self, from: &str, to: &str) -> Result<Vec<[f32; 2]>, String> {
        let start = self
            .tree
            .node(from)
            .ok_or_else(|| format!("{from} isn't in the world tree"))?
            .stand;
        let (route, _) =
            perceive::route(self.tree, self.blockers, start, to).map_err(|e| e.to_string())?;
        let mut points = vec![start];
        points.extend(route.waypoints);
        if points.last() != Some(&route.destination) {
            points.push(route.destination);
        }
        Ok(points)
    }
}

impl Router for Routes<'_> {
    fn meters(&self, from: &str, to: &str) -> Result<f32, String> {
        Ok(Leg::new(self.points(from, to)?).length)
    }
}

/// The blockers villagers route around, from the pinned pack at `path`:
/// for `openagents verse town`, which routes without opening a window.
///
/// # Errors
///
/// When the pack doesn't load or the layout doesn't build from it.
pub fn blockers_from_pack(path: &std::path::Path) -> Result<Vec<Footprint>, String> {
    let pack = crate::zones::everglade_pack::ZonePack::load_local(path)?;
    Ok(super::Everglade::world(&pack)?.blockers)
}

/// A walked leg: its points and the distance to each.
#[derive(Clone, Debug)]
struct Leg {
    points: Vec<[f32; 2]>,
    along: Vec<f32>,
    length: f32,
}

impl Leg {
    fn new(points: Vec<[f32; 2]>) -> Self {
        let mut along = Vec::with_capacity(points.len());
        let mut length = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let q = points[i - 1];
                length += (p[0] - q[0]).hypot(p[1] - q[1]);
            }
            along.push(length);
        }
        Self {
            points,
            along,
            length,
        }
    }

    /// The point `progress` of the way along, and the heading there.
    fn at(&self, progress: f32) -> ([f32; 2], f32) {
        let d = progress.clamp(0.0, 1.0) * self.length;
        let i = self
            .along
            .iter()
            .position(|&a| a >= d)
            .unwrap_or(self.points.len() - 1)
            .max(1)
            .min(self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let span = (self.along[i] - self.along[i - 1]).max(1e-4);
        let t = ((d - self.along[i - 1]) / span).clamp(0.0, 1.0);
        let yaw = (b[0] - a[0]).atan2(b[1] - a[1]);
        ([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], yaw)
    }
}

/// The farthest from the player a villager is drawn, m.
pub const DRAW_REACH: f32 = 90.0;
/// The most new legs routed in one tick; a leg not routed yet is walked
/// straight.
const PLANS_PER_TICK: usize = 2;

/// One villager where the last tick placed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Townsperson {
    pub id: String,
    pub name: String,
    pub pos: Vec3,
    pub yaw: f32,
    /// How fast it walks now, m per real second; zero standing.
    pub speed: f32,
    pub activity: Activity,
    /// The node it stands at, or walks to.
    pub node: String,
    pub walking: bool,
    pub tint: [f32; 3],
}

impl Townsperson {
    /// Its nameplate: its name, what it is doing, and that it is a
    /// character, with its role.
    #[must_use]
    pub fn plate(&self, role: &str) -> [String; 3] {
        [
            self.name.clone(),
            if self.walking {
                "walking".to_owned()
            } else {
                self.activity.doing().to_owned()
            },
            format!("character / {role}"),
        ]
    }
}

/// The townsfolk as Everglade draws them.
#[derive(Default)]
pub struct Townsfolk {
    /// Routed legs by (from, to); `None` where the zone has no route.
    legs: HashMap<(String, String), Option<Leg>>,
    people: Vec<Townsperson>,
    /// Each villager's nameplate text and its faces in plate space.
    plates: HashMap<String, ([String; 3], Mesh)>,
    player: Option<Vec3>,
}

/// The posture a villager holds at its activity.
fn posture(activity: Activity) -> Posture {
    match activity {
        Activity::Bake | Activity::Smith | Activity::Craft | Activity::Tend | Activity::Work => {
            Posture::Work
        }
        Activity::Read => Posture::Read,
        Activity::Pray | Activity::RingBell => Posture::Wait,
        Activity::Sell | Activity::Gather => Posture::Talk,
        Activity::Shop => Posture::Think,
        Activity::Sleep | Activity::Eat | Activity::Drink | Activity::Rest => Posture::Stand,
    }
}

impl Townsfolk {
    /// Places every admitted villager at `time` and routes the legs they
    /// walk over `blockers`, near `player`.
    pub fn tick(&mut self, time: TownTime, blockers: &[Footprint], player: Vec3) {
        let (roster, _) = roster();
        self.tick_roster(roster, world_tree::everglade(), time, blockers, player);
    }

    /// [`Self::tick`] for any roster and tree, for tests.
    pub fn tick_roster(
        &mut self,
        roster: &Roster,
        tree: &Tree,
        time: TownTime,
        blockers: &[Footprint],
        player: Vec3,
    ) {
        self.player = Some(player);
        let mut planned = 0;
        let mut people = Vec::with_capacity(roster.villagers.len());
        for villager in &roster.villagers {
            let placement = villager.at(roster.town.seed, time);
            let person = self.place(villager, &placement, tree, blockers, &mut planned);
            if let Some(person) = person {
                people.push(person);
            }
        }
        for person in &people {
            let role = roster
                .villager(&person.id)
                .map_or("", |v| v.npc.card.role.as_str());
            let text = person.plate(role);
            let stale = self.plates.get(&person.id).is_none_or(|(t, _)| *t != text);
            if stale {
                let mesh = plate(&text, Attention::Idle);
                self.plates.insert(person.id.clone(), (text, mesh));
            }
        }
        self.people = people;
    }

    fn place(
        &mut self,
        villager: &Villager,
        placement: &Placement<'_>,
        tree: &Tree,
        blockers: &[Footprint],
        planned: &mut usize,
    ) -> Option<Townsperson> {
        let npc = &villager.npc;
        let (at, yaw, speed, node, walking) = match placement {
            Placement::At {
                node,
                offset,
                activity,
                ..
            } => {
                let n = tree.node(node)?;
                let at = [n.stand[0] + offset[0], n.stand[1] + offset[1]];
                // Trading or chatting, face the node's standing point, so
                // villagers there face each other; otherwise face the node:
                // its heading, or into a building's doorway.
                let inward = (-offset[0]).atan2(-offset[1]);
                let social = matches!(
                    activity,
                    Activity::Sell | Activity::Shop | Activity::Gather | Activity::Rest
                );
                let yaw = match (n.facing, n.entry) {
                    _ if social && *offset != [0.0, 0.0] => inward,
                    (Some(f), _) => f,
                    (None, Some([_, inside])) => (inside[0] - at[0]).atan2(inside[1] - at[1]),
                    (None, None) => inward,
                };
                (at, yaw, 0.0, *node, false)
            }
            Placement::Walking {
                from,
                to,
                progress,
                row,
                ..
            } => {
                let key = ((*from).to_owned(), (*to).to_owned());
                if !self.legs.contains_key(&key)
                    && !blockers.is_empty()
                    && *planned < PLANS_PER_TICK
                {
                    *planned += 1;
                    let routes = Routes { tree, blockers };
                    self.legs
                        .insert(key.clone(), routes.points(from, to).ok().map(Leg::new));
                }
                let leg = match self.legs.get(&key) {
                    Some(Some(leg)) => leg.clone(),
                    _ => Leg::new(vec![tree.node(from)?.stand, tree.node(to)?.stand]),
                };
                let (at, yaw) = leg.at(*progress);
                let real = villager.walk(*row) / TOWN_PER_REAL;
                let speed = if real > 0.0 {
                    (f64::from(leg.length) / real) as f32
                } else {
                    0.0
                };
                (at, yaw, speed, *to, true)
            }
        };
        Some(Townsperson {
            id: npc.id.clone(),
            name: npc.name.clone(),
            pos: Vec3::new(at[0], height(at[0], at[1]), at[1]),
            yaw,
            speed,
            activity: placement.activity(),
            node: node.to_owned(),
            walking,
            tint: npc.look.tint,
        })
    }

    /// Every villager where the last tick placed it.
    #[must_use]
    pub fn people(&self) -> &[Townsperson] {
        &self.people
    }

    fn near(&self) -> impl Iterator<Item = &Townsperson> {
        let player = self.player;
        self.people.iter().filter(move |p| {
            player.is_none_or(|q| (p.pos.x - q.x).hypot(p.pos.z - q.z) <= DRAW_REACH)
        })
    }

    /// The villagers within [`DRAW_REACH`] of the player, as the zone draws
    /// a studio seat: the pack's character in the villager's tint.
    #[must_use]
    pub fn figures(&self) -> Vec<SeatFigure> {
        self.near()
            .map(|p| SeatFigure {
                name: format!("townsfolk:{}", p.id),
                pos: p.pos,
                yaw: p.yaw,
                speed: p.speed,
                posture: posture(p.activity),
                look: None,
                tint: p.tint,
                form: None,
            })
            .collect()
    }

    /// The nameplates over the villagers near the player, seen from `eye`.
    #[must_use]
    pub fn draw(&self, eye: Vec3) -> Mesh {
        let mut mesh = Mesh::default();
        for person in self.near() {
            let (Some((_, plate)), Some(transform)) = (
                self.plates.get(&person.id),
                plate_transform(person.pos, eye),
            ) else {
                continue;
            };
            mesh.faces.extend(plate.faces.iter().map(|v| Vertex {
                pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                ..*v
            }));
        }
        mesh
    }
}

#[cfg(test)]
mod tests;
