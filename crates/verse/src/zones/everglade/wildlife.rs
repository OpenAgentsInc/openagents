//! Everglade's ambient wildlife: songbirds circling over the commons and
//! the woods and one flitting between the park's trees, ducks paddling on
//! the ponds, frogs hopping round their banks, a cat sitting on the
//! orchard wall, rats running along Main Street and Foundry Road, a snake
//! in Walden Woods, and wasps at the beekeeper's hives.
//!
//! Each creature is one of the pack's forms (`beasts/<name>`, built by
//! `scripts/blender/wildlife.py`) on a route that is a function of the
//! clock alone, so it needs no state beyond its pose and every viewer of
//! the same clock sees the same thing. The creatures draw in the frame's
//! figure after the characters ([`Wildlife::figure`]); one farther than
//! [`CULL`] from the eye is folded away and not skinned, so the frame
//! poses only the few nearby.

use std::f32::consts::TAU;
use std::sync::Arc;

use glam::Vec3;

use super::layout::{PONDS, Placement};
use super::player::{Beast, Motion};
use super::{draw, height};
use crate::pbr::textured::{Figure, TexturedScene, TexturedVertex, UNBAKED};
use crate::pbr::textured_bake::AmbientProbes;
use crate::zones::everglade_pack::ZonePack;

/// Creatures farther than this from the eye are not drawn, m.
pub const CULL: f32 = 60.0;

/// Where a vertex goes that draws nothing: a creature out of range.
const FOLDED: TexturedVertex = TexturedVertex {
    pos: [0.0, -100.0, 0.0],
    normal: [0.0, 1.0, 0.0],
    uv: [0.0; 2],
    color: [0; 4],
    light: UNBAKED,
};

/// How a creature moves, as a function of the clock.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// Round a circle at a steady pace, bobbing: birds aloft, ducks on the
    /// water, wasps at their hives. `period` is seconds a lap; a negative
    /// period goes the other way.
    Circle {
        center: Vec3,
        radius: f32,
        period: f32,
        bob: f32,
        motion: Motion,
    },
    /// Back and forth between two points at `speed`, resting `rest`
    /// seconds at each end: rats and the snake.
    Pace {
        a: Vec3,
        b: Vec3,
        speed: f32,
        rest: f32,
    },
    /// Sitting at each point in turn for `rest` seconds, then leaping to
    /// the next in `leap` seconds along an arc `rise` meters high: frogs
    /// round a bank, and a songbird between treetops.
    Hop {
        points: Vec<Vec3>,
        rest: f32,
        leap: f32,
        rise: f32,
        motion: Motion,
    },
    /// Sitting still, facing `yaw`: the cat.
    Sit { at: Vec3, yaw: f32 },
}

/// Where a creature is and what it plays at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    pub at: Vec3,
    pub yaw: f32,
    pub motion: Motion,
    /// Seconds into the motion's clip, or, for a gait that carries the
    /// creature, meters it has moved, which the clip's stride turns into
    /// time.
    pub clip: f32,
    /// Whether `clip` is meters along the ground rather than seconds.
    pub along: bool,
}

/// The heading that faces a model's front along `d`.
fn heading(d: Vec3) -> f32 {
    d.x.atan2(d.z)
}

impl Route {
    /// Where the creature is `t` seconds into the clock, with `phase`
    /// seconds of offset so that creatures on one route spread out.
    #[must_use]
    pub fn at(&self, t: f32) -> Moment {
        match self {
            Self::Circle {
                center,
                radius,
                period,
                bob,
                motion,
            } => {
                let angle = TAU * t / period;
                let (s, c) = angle.sin_cos();
                let at = *center + Vec3::new(c * radius, bob * (t * 1.7).sin(), s * radius);
                // The tangent the way it goes round.
                let d = Vec3::new(-s, 0.0, c) * period.signum();
                Moment {
                    at,
                    yaw: heading(d),
                    motion: *motion,
                    clip: t,
                    along: false,
                }
            }
            Self::Pace { a, b, speed, rest } => {
                let span = (*b - *a).length().max(1e-3);
                let walk = span / speed.max(1e-3);
                let cycle = 2.0 * (walk + rest);
                let u = t.rem_euclid(cycle);
                // Out, rest, back, rest.
                let (from, to, k) = if u < walk {
                    (*a, *b, u / walk)
                } else if u < walk + rest {
                    (*a, *b, 1.0)
                } else if u < 2.0 * walk + rest {
                    (*b, *a, (u - walk - rest) / walk)
                } else {
                    (*b, *a, 1.0)
                };
                let moving = k < 1.0;
                Moment {
                    at: from.lerp(to, k),
                    yaw: heading(to - from),
                    motion: if moving { Motion::Walk } else { Motion::Idle },
                    clip: if moving { k * span } else { t },
                    along: moving,
                }
            }
            Self::Hop {
                points,
                rest,
                leap,
                rise,
                motion,
            } => {
                let n = points.len().max(1);
                let leg = rest + leap;
                let u = t.rem_euclid(leg * n as f32);
                let i = (u / leg) as usize % n;
                let (from, to) = (points[i], points[(i + 1) % n]);
                let into = u - i as f32 * leg;
                let yaw = heading(to - from);
                if into < *rest {
                    Moment {
                        at: from,
                        yaw,
                        motion: Motion::Idle,
                        clip: t,
                        along: false,
                    }
                } else {
                    let k = ((into - rest) / leap).clamp(0.0, 1.0);
                    let arc = 4.0 * k * (1.0 - k) * rise;
                    Moment {
                        at: from.lerp(to, k) + Vec3::Y * arc,
                        yaw,
                        motion: *motion,
                        clip: into - rest,
                        along: false,
                    }
                }
            }
            Self::Sit { at, yaw } => Moment {
                at: *at,
                yaw: *yaw,
                motion: Motion::Idle,
                clip: t,
                along: false,
            },
        }
    }
}

/// One creature: its form, its route, its size, and its offset on the
/// clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Creature {
    pub form: &'static str,
    pub route: Route,
    pub scale: f32,
    pub phase: f32,
}

/// The town's creatures, from the layout's `placements` and the pack's
/// models: perches on its park trees, the orchard wall, and the hives.
#[must_use]
pub fn creatures(pack: &ZonePack, placements: &[Placement]) -> Vec<Creature> {
    let top = |p: &Placement| {
        let lift = pack
            .model(p.model)
            .map_or(1.0, |m| m.bounds().1[1] * p.scale);
        Vec3::new(p.at[0], height(p.at[0], p.at[1]) + p.lift + lift, p.at[1])
    };
    let water = |x: f32, z: f32| Vec3::new(x, height(x, z) + draw::WATER_LIFT, z);
    let mut out = Vec::new();
    // Songbirds: pairs circling over the commons, the woods, and Main
    // Street, high enough to clear the roofs.
    for (k, (center, radius, period)) in [
        ([0.0_f32, 20.0], 14.0_f32, 16.0_f32),
        ([-108.0, -86.0], 18.0, -20.0),
        ([10.0, 62.0], 22.0, 22.0),
        ([96.0, 72.0], 12.0, -14.0),
    ]
    .into_iter()
    .enumerate()
    {
        let [x, z] = center;
        for pair in 0..2 {
            out.push(Creature {
                form: "beasts/songbird",
                route: Route::Circle {
                    center: Vec3::new(x, height(x, z) + 13.0 + 2.5 * pair as f32, z),
                    radius: radius - 3.0 * pair as f32,
                    period,
                    bob: 0.6,
                    motion: Motion::Walk,
                },
                scale: 1.4,
                phase: period.abs() * (0.37 * pair as f32 + 0.11 * k as f32),
            });
        }
    }
    // One songbird flits between the commons' park trees, perching on
    // each crown in turn.
    let crowns: Vec<Vec3> = placements
        .iter()
        .filter(|p| {
            (p.model.starts_with("nature/CommonTree")
                || super::layout::foliage::PARK_TREES.contains(&p.model))
                && p.at[1] > 0.0
                && p.at[1] < 44.0
        })
        .filter(|p| p.at[0].abs() < 34.0)
        .take(5)
        .map(|p| top(p) - Vec3::Y * 0.6)
        .collect();
    if crowns.len() >= 2 {
        out.push(Creature {
            form: "beasts/songbird",
            route: Route::Hop {
                points: crowns,
                rest: 4.0,
                leap: 3.0,
                rise: 3.0,
                motion: Motion::Walk,
            },
            scale: 1.4,
            phase: 0.0,
        });
    }
    // Ducks paddling on the ponds, and a frog hopping round each bank.
    for (k, &([x, z], r)) in PONDS.iter().enumerate() {
        let ducks = if k == 0 { 2 } else { 1 };
        for d in 0..ducks {
            let radius = r * (0.55 - 0.2 * d as f32);
            out.push(Creature {
                form: "beasts/duck",
                route: Route::Circle {
                    center: water(x, z),
                    radius,
                    period: if d == 0 { 40.0 } else { -28.0 },
                    bob: 0.01,
                    motion: Motion::Walk,
                },
                scale: 1.3,
                phase: 7.0 * k as f32 + 13.0 * d as f32,
            });
        }
        let bank: Vec<Vec3> = (0..4)
            .map(|i| {
                let a = 0.9 * k as f32 + i as f32 * 0.35;
                let (px, pz) = (x + a.cos() * (r + 0.45), z + a.sin() * (r + 0.45));
                Vec3::new(px, height(px, pz), pz)
            })
            .collect();
        out.push(Creature {
            form: "beasts/frog",
            route: Route::Hop {
                points: bank,
                rest: 5.0,
                leap: 0.5,
                rise: 0.25,
                motion: Motion::Jump,
            },
            scale: 1.0,
            phase: 2.3 * k as f32,
        });
    }
    // A cat on the orchard's dry-stone wall, facing out over the lane, or
    // on the first fence where the town has no such wall.
    let walls = [
        "generated/stone_wall",
        "generated/picket_fence",
        "generated/rail_fence",
        "generated/hedge",
    ];
    if let Some(wall) = walls
        .iter()
        .find_map(|w| placements.iter().find(|p| p.model == *w))
    {
        out.push(Creature {
            form: "beasts/cat",
            route: Route::Sit {
                at: top(wall),
                yaw: wall.yaw + std::f32::consts::FRAC_PI_2,
            },
            scale: 1.0,
            phase: 0.0,
        });
    }
    // Rats running along the edges of Main Street and Foundry Road.
    for (k, (a, b)) in [
        ([30.0_f32, 43.1_f32], [37.0_f32, 43.1_f32]),
        ([28.0, 1.7], [36.0, 1.7]),
    ]
    .into_iter()
    .enumerate()
    {
        let at = |[x, z]: [f32; 2]| Vec3::new(x, height(x, z), z);
        out.push(Creature {
            form: "beasts/rat",
            route: Route::Pace {
                a: at(a),
                b: at(b),
                speed: 0.9,
                rest: 3.0,
            },
            scale: 1.0,
            phase: 4.0 * k as f32,
        });
    }
    // A snake in Walden Woods.
    let at = |x: f32, z: f32| Vec3::new(x, height(x, z), z);
    out.push(Creature {
        form: "beasts/snake",
        route: Route::Pace {
            a: at(-104.0, -82.0),
            b: at(-100.0, -78.0),
            speed: 0.25,
            rest: 6.0,
        },
        scale: 1.0,
        phase: 0.0,
    });
    // Wasps at the beekeeper's hives.
    if let Some(hives) = placements.iter().find(|p| p.model == "generated/beehives") {
        let center = Vec3::new(hives.at[0], height(hives.at[0], hives.at[1]), hives.at[1]);
        for w in 0..3 {
            out.push(Creature {
                form: "beasts/wasp",
                route: Route::Circle {
                    center: center + Vec3::Y * (1.1 + 0.25 * w as f32),
                    radius: 0.9 + 0.35 * w as f32,
                    period: if w % 2 == 0 { 3.5 } else { -4.5 },
                    bob: 0.15,
                    motion: Motion::Idle,
                },
                scale: 1.0,
                phase: 1.1 * w as f32,
            });
        }
    }
    out
}

/// The town's creatures, posed for each frame.
pub struct Wildlife {
    creatures: Vec<Creature>,
    beasts: Vec<Beast>,
    /// Each creature's vertex count in the figure, in order.
    counts: Vec<usize>,
    /// Every creature's primitives in one mesh, in creature order.
    scene: Arc<TexturedScene>,
    /// This frame's vertices, every creature's in turn.
    posed: Vec<TexturedVertex>,
    /// The characters' scene and the scene of it and the creatures.
    combined: Option<(Arc<TexturedScene>, Arc<TexturedScene>)>,
    clock: f32,
}

impl Wildlife {
    /// The creatures whose forms `pack` holds.
    ///
    /// # Errors
    ///
    /// Returns a message when a form cannot play.
    pub fn new(pack: &ZonePack, creatures: Vec<Creature>) -> Result<Self, String> {
        let mut kept = Vec::new();
        let mut beasts = Vec::new();
        let mut counts = Vec::new();
        let mut scene: Option<TexturedScene> = None;
        for creature in creatures {
            let Some(form) = pack.form(creature.form) else {
                continue;
            };
            let beast = Beast::new(pack, form)?;
            let figure = beast.figure();
            counts.push(figure.vertices.len());
            scene = Some(match scene {
                None => figure.scene.as_ref().clone(),
                Some(scene) => super::demolition::join(&scene, &figure.scene),
            });
            beasts.push(beast);
            kept.push(creature);
        }
        let scene = scene.unwrap_or_default();
        let posed = vec![FOLDED; counts.iter().sum()];
        Ok(Self {
            creatures: kept,
            beasts,
            counts,
            scene: Arc::new(scene),
            posed,
            combined: None,
            clock: 0.0,
        })
    }

    /// The creatures, for tests and captures.
    #[must_use]
    pub fn creatures(&self) -> &[Creature] {
        &self.creatures
    }

    /// How many creatures are posed this frame.
    #[must_use]
    pub fn drawn(&self) -> usize {
        let mut at = 0;
        let mut drawn = 0;
        for &count in &self.counts {
            if self.posed.get(at).is_some_and(|v| v.pos[1] > -50.0) {
                drawn += 1;
            }
            at += count;
        }
        drawn
    }

    /// Advances the clock by `dt` and poses each creature within [`CULL`]
    /// of `eye`; the rest fold away.
    pub fn tick(&mut self, dt: f32, eye: Vec3) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.clock = (self.clock + dt) % 3600.0;
        let mut at = 0;
        for ((creature, beast), &count) in self
            .creatures
            .iter()
            .zip(&mut self.beasts)
            .zip(&self.counts)
        {
            let moment = creature.route.at(self.clock + creature.phase);
            let slot = &mut self.posed[at..at + count];
            at += count;
            if moment.at.distance(eye) > CULL {
                slot.fill(FOLDED);
                continue;
            }
            let time = if moment.along {
                let (distance, duration) = beast.stride(moment.motion);
                if distance > 1e-3 {
                    moment.clip / (distance * creature.scale) * duration
                } else {
                    self.clock + creature.phase
                }
            } else {
                moment.clip
            };
            beast.pose(
                moment.at,
                moment.yaw,
                creature.scale,
                moment.motion,
                time,
                dt,
            );
            let posed = beast.vertices();
            if posed.len() == count {
                slot.copy_from_slice(posed);
            } else {
                slot.fill(FOLDED);
            }
        }
    }

    /// Joins the creatures to the characters' `scene` once per scene.
    pub fn prepare(&mut self, scene: &Arc<TexturedScene>) {
        if self.creatures.is_empty()
            || self
                .combined
                .as_ref()
                .is_some_and(|(cast, _)| Arc::ptr_eq(cast, scene))
        {
            return;
        }
        let joined = super::demolition::join(scene, &self.scene);
        self.combined = Some((scene.clone(), Arc::new(joined)));
    }

    /// The scene of the characters and the creatures, once prepared for
    /// the characters' `scene`.
    #[must_use]
    pub fn joined(&self, scene: &Arc<TexturedScene>) -> Option<Arc<TexturedScene>> {
        self.combined
            .as_ref()
            .filter(|(cast, _)| Arc::ptr_eq(cast, scene))
            .map(|(_, joined)| joined.clone())
    }

    /// The frame's figure: the characters' `cast` figure followed by the
    /// creatures, lit by `probes` when the bake has them.
    #[must_use]
    pub fn figure(&self, cast: Figure, probes: Option<&AmbientProbes>) -> Figure {
        let Some(joined) = self.joined(&cast.scene) else {
            return cast;
        };
        let mut vertices = Vec::with_capacity(cast.vertices.len() + self.posed.len());
        vertices.extend_from_slice(&cast.vertices);
        let start = vertices.len();
        vertices.extend_from_slice(&self.posed);
        if let Some(probes) = probes {
            probes.shade(&mut vertices[start..]);
        }
        Figure {
            scene: joined,
            vertices: Arc::new(vertices),
        }
    }
}
