//! The Draconia courtyard battle, phase D1 (`docs/verse/draconia/shots.md`,
//! shots 1–6, 0–9.7 s of the reference): one scene kind, one set per shot.
//!
//! - **1:** Draconia's lower face in wind and embers, firelit from the left.
//! - **2:** a girl crouched against a stone wall in cold shadow as a green
//!   bolt flashes past and dust falls.
//! - **3** (with 4): a boy with his back to a brick wall; a green bolt
//!   passes just over it, then the wall top blows apart in golden dust and
//!   he ducks and runs.
//! - **5:** a dark cloak crossing the lens, a wipe.
//! - **6:** a high wide shot of students dueling on the cobbles.
//!
//! Characters are the pack's: Alice recolored (Draconia's platinum hair and
//! black coat, or her own auburn for the girl) and the male ranger for the
//! boys, until the Draconia cast exists (`character.md`). Everything else
//! is lit geometry and particles on the physical renderer's neon stage.

use std::sync::Arc;

use glam::Vec3;
use serde::Deserialize;
use verse::fx::{Particles, Spawn, feathers};
use verse::mesh::Mesh;
use verse::pbr::textured::{Figure, TexturedScene, TexturedVertex};
use verse::pbr::{Lamp, LitVertex, MAX_LAMPS, Neon};
use verse::zones::everglade::demolition::join;
use verse::zones::everglade::player::Beast;
use verse::zones::everglade_pack::ZonePack;

use crate::peacock::{cobbles, draconia, hash, key, push_box};

/// Which shot of D1.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shot {
    Face,
    Girl,
    Boy,
    Cloak,
    Duel,
}

/// The green of the reference's bolts, cd/m².
const GREEN: [f32; 3] = [0.6, 6.0, 1.2];

/// One cast member: a rig, how it's placed and posed each frame.
struct Member {
    beast: Beast,
    /// Vertices hidden every frame (Alice's staff).
    hidden: Vec<usize>,
}

/// The cast: every member's rig and the one joined scene they draw as.
struct Cast {
    members: Vec<Member>,
    scene: Arc<TexturedScene>,
}

/// How a cast member looks.
#[derive(Clone, Copy)]
enum Look {
    /// Alice as Draconia: platinum hair, black coat.
    Draconia,
    /// Alice in her own colors.
    Alice,
    /// The pack's male ranger, his outfit darkened toward a school coat.
    Boy,
}

fn member(pack: &ZonePack, look: Look) -> Result<(Member, TexturedScene), String> {
    let character = match look {
        Look::Draconia | Look::Alice => pack.form("npc/alice"),
        Look::Boy => pack.character.as_ref(),
    }
    .ok_or("the pack lacks a cast member")?;
    let beast = Beast::new(pack, character)?;
    let mut scene = beast.figure().scene.as_ref().clone();
    let hidden = match look {
        Look::Draconia | Look::Alice => {
            if matches!(look, Look::Draconia) {
                draconia(&mut scene);
            }
            beast
                .figure()
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.pos[2] < -0.12 && v.pos[0] > 0.1 && v.pos[1] > 1.1)
                .map(|(i, _)| i)
                .collect()
        }
        Look::Boy => Vec::new(),
    };
    Ok((Member { beast, hidden }, scene))
}

impl Cast {
    fn new(pack: &ZonePack, looks: &[Look]) -> Result<Self, String> {
        let mut members = Vec::new();
        let mut joined: Option<TexturedScene> = None;
        for &look in looks {
            let (m, scene) = member(pack, look)?;
            joined = Some(match joined {
                None => scene,
                Some(j) => join(&j, &scene),
            });
            members.push(m);
        }
        Ok(Self {
            members,
            scene: Arc::new(joined.unwrap_or_default()),
        })
    }

    fn figure(&self) -> Figure {
        let mut vertices: Vec<TexturedVertex> = Vec::new();
        for m in &self.members {
            let start = vertices.len();
            vertices.extend_from_slice(m.beast.vertices());
            if let Some(&first) = m.hidden.first() {
                let point = vertices[start + first].pos;
                for &i in &m.hidden {
                    vertices[start + i].pos = point;
                }
            }
        }
        Figure {
            scene: self.scene.clone(),
            vertices: Arc::new(vertices),
        }
    }
}

/// A chunk of masonry: where it is, how it moves and turns, its size.
#[derive(Clone, Copy)]
struct Chunk {
    min: Vec3,
    max: Vec3,
    velocity: Vec3,
    spin: f32,
    shade: f32,
}

pub struct Set {
    shot: Shot,
    time: f32,
    fx: Particles,
    cast: Cast,
    lamps: Vec<Lamp>,
    /// The wall top that blows apart in shot 3, as dynamic blocks.
    chunks: Vec<Chunk>,
    blasted: bool,
}

/// When the bolt hits the wall in the boy's shot, s.
const BLAST: f32 = 0.8;

impl Set {
    /// The shot's static stones.
    pub fn world(shot: Shot) -> Mesh {
        let mut lit = Vec::new();
        cobbles(&mut lit);
        match shot {
            Shot::Face => {}
            Shot::Girl => brick_wall(&mut lit, -4.0, 4.0, -0.45, &|_| 2.6, 1),
            Shot::Boy => brick_wall(&mut lit, -4.0, 4.0, -0.4, &|x| broken_top(x), 2),
            Shot::Cloak | Shot::Duel => {
                brick_wall(
                    &mut lit,
                    -9.0,
                    9.0,
                    -5.0,
                    &|x| 3.0 + 1.2 * broken_top(x * 0.7),
                    3,
                );
                brick_wall(&mut lit, -8.0, -2.5, 3.5, &|x| 1.2 + 0.4 * broken_top(x), 4);
            }
        }
        Mesh {
            lit,
            ..Mesh::default()
        }
    }

    pub fn new(shot: Shot, pack: &ZonePack) -> Result<Self, String> {
        let looks: &[Look] = match shot {
            Shot::Face => &[Look::Draconia],
            Shot::Girl => &[Look::Alice],
            Shot::Boy => &[Look::Boy],
            Shot::Cloak => &[Look::Boy, Look::Alice, Look::Boy, Look::Boy],
            Shot::Duel => &[
                Look::Boy,
                Look::Alice,
                Look::Boy,
                Look::Alice,
                Look::Boy,
                Look::Boy,
                Look::Alice,
                Look::Boy,
                Look::Alice,
            ],
        };
        let mut cast = Cast::new(pack, looks)?;
        if shot == Shot::Cloak {
            // The cloak crossing the lens is the first member, all black.
            darken_first(&mut cast);
        }
        let mut fx = Particles::new(0xD1);
        let mut lamps = Vec::new();
        let fire = |fx: &mut Particles, lamps: &mut Vec<Lamp>, at: Vec3, power: f32| {
            fx.start("ruin_fire", Spawn::at(at));
            lamps.push(Lamp {
                position: at + Vec3::Y * 0.6,
                color: [1.0, 0.45, 0.14],
                intensity: power,
                range: 9.0,
            });
        };
        match shot {
            Shot::Face => {
                fire(&mut fx, &mut lamps, Vec3::new(-2.2, 1.2, -0.6), 0.0);
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.8, 0.4)).scaled(0.25),
                );
                lamps.push(Lamp {
                    position: Vec3::new(-1.0, 1.6, 0.7),
                    color: [1.0, 0.55, 0.22],
                    intensity: 4.0,
                    range: 4.0,
                });
            }
            Shot::Girl => {
                fx.start("debris_dust", Spawn::at(Vec3::new(0.6, 2.6, -0.2)));
            }
            Shot::Boy => {}
            Shot::Cloak | Shot::Duel => {
                for at in [
                    Vec3::new(-6.0, 0.2, -3.0),
                    Vec3::new(5.0, 0.2, -3.5),
                    Vec3::new(-3.0, 0.4, 2.8),
                ] {
                    fire(&mut fx, &mut lamps, at, 30.0);
                }
                fx.start("night_embers", Spawn::at(Vec3::new(0.0, 0.3, 0.0)));
                fx.start(
                    "ground_mist",
                    Spawn::at(Vec3::new(0.0, 0.0, 0.0)).scaled(0.6),
                );
            }
        }
        let chunks = if shot == Shot::Boy {
            wall_top_chunks()
        } else {
            Vec::new()
        };
        let mut set = Self {
            shot,
            time: 0.0,
            fx,
            cast,
            lamps,
            chunks,
            blasted: false,
        };
        for _ in 0..90 {
            set.fx.tick(1.0 / 30.0, |_, _| 0.0);
        }
        set.advance(1.0 / 30.0, 0.0);
        Ok(set)
    }

    /// Advances to film time `t`, `dt` seconds on.
    pub fn advance(&mut self, dt: f32, t: f32) {
        self.time = t;
        if self.shot == Shot::Boy && !self.blasted && t >= BLAST {
            self.blasted = true;
            self.fx
                .start("wall_blast", Spawn::at(Vec3::new(0.3, 2.0, -0.3)));
        }
        if self.blasted {
            for c in &mut self.chunks {
                c.velocity.y -= 9.8 * dt;
                let step = c.velocity * dt;
                c.min += step;
                c.max += step;
            }
        }
        let steps = (dt * 120.0).ceil().max(1.0) as usize;
        for _ in 0..steps {
            self.fx.tick(dt / steps as f32, |_, _| 0.0);
        }
        let shot = self.shot;
        for (k, m) in self.cast.members.iter_mut().enumerate() {
            let b = &mut m.beast;
            match (shot, k) {
                (Shot::Face, _) => {
                    b.pose_clip(Vec3::ZERO, 0.0, 1.0, "idle", t + 1.0, true, dt);
                }
                (Shot::Girl, _) => {
                    // Crouched against the wall, her face turned to it: no
                    // stand-in face shows in a medium shot.
                    b.pose_clip(
                        Vec3::new(0.4, -0.25, 0.35),
                        std::f32::consts::PI + 0.5,
                        1.0,
                        "swing",
                        0.4,
                        false,
                        dt,
                    );
                }
                (Shot::Boy, _) => {
                    if t < BLAST + 0.1 {
                        b.pose_clip(Vec3::new(0.0, 0.0, 0.1), 0.0, 1.0, "idle", t, true, dt);
                    } else if t < BLAST + 0.35 {
                        b.pose_clip(
                            Vec3::new(0.0, -0.15, 0.15),
                            0.0,
                            1.0,
                            "swing",
                            0.4,
                            false,
                            dt,
                        );
                    } else {
                        // He runs off to frame right.
                        let run = t - BLAST - 0.35;
                        b.pose_clip(
                            Vec3::new(run * 3.2, 0.0, 0.3 + run * 1.2),
                            1.2,
                            1.0,
                            "run",
                            run,
                            true,
                            dt,
                        );
                    }
                }
                (Shot::Cloak, 0) => {
                    // A dark figure walking right across the lens.
                    b.pose_clip(
                        Vec3::new(-1.6 + t * 3.6, 0.0, 3.4),
                        1.57,
                        1.0,
                        "walk",
                        t,
                        true,
                        dt,
                    );
                }
                (Shot::Cloak | Shot::Duel, _) => {
                    let (_, yaw, clip, phase, _) = DUELERS[k % DUELERS.len()];
                    b.pose_clip(dueler_at(k, t), yaw, 1.0, clip, phase + t, true, dt);
                }
            }
        }
    }

    pub fn frame(&self) -> Mesh {
        let t = self.time;
        let mut mesh = Mesh {
            neon: Some(self.stage(t)),
            figure: Some(self.cast.figure()),
            ..Mesh::default()
        };
        // The wall top, whole or flying apart.
        for c in &self.chunks {
            push_box(
                &mut mesh.lit,
                c.min,
                c.max,
                if self.blasted {
                    c.spin * (t - BLAST)
                } else {
                    0.0
                },
                [c.shade * 1.2, c.shade, c.shade * 0.85],
                0.85,
            );
        }
        // The bolts.
        match self.shot {
            Shot::Girl if t < 0.45 => {
                let k = (t / 0.45).clamp(0.0, 1.0);
                mesh.ribbons.extend(feathers::beam(
                    Vec3::new(-4.5, 1.7, 1.6),
                    Vec3::new(-4.5, 1.7, 1.6).lerp(Vec3::new(1.0, 1.1, -0.3), 0.4 + 0.6 * k),
                    0.05,
                    GREEN,
                ));
            }
            Shot::Boy if (0.15..BLAST).contains(&t) => {
                let k = ((t - 0.15) / 0.3).clamp(0.0, 1.0);
                let from = Vec3::new(-6.0, 2.4, -1.4);
                let to = Vec3::new(0.3, 2.0, -0.3);
                mesh.ribbons
                    .extend(feathers::beam(from, from.lerp(to, k), 0.05, GREEN));
            }
            Shot::Cloak | Shot::Duel => {
                for (i, &(a, b)) in BOLTS.iter().enumerate() {
                    let from = dueler_at(a, t) + Vec3::new(0.3, 1.4, 0.2);
                    let to = dueler_at(b, t) + Vec3::new(0.0, 1.3, 0.0);
                    let flicker = 0.85 + 0.15 * (t * 23.0 + i as f32 * 2.0).sin();
                    // The second bolt is red, as the reference's duels mix.
                    let color = if i == 0 { GREEN } else { [6.0, 1.0, 0.6] };
                    mesh.ribbons
                        .extend(feathers::beam(from, to, 0.04, color.map(|c| c * flicker)));
                    // A shield shimmer where it lands.
                    mesh.sprites.push(feathers::eyespot(
                        to + (from - to).normalize_or_zero() * 0.3,
                        0.55,
                        [0.6, 1.6, 1.0],
                    ));
                }
            }
            _ => {}
        }
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    fn stage(&self, t: f32) -> Neon {
        let mut neon = Neon::neutral(t);
        neon.fog_start = 30.0;
        neon.fog_end = 120.0;
        neon.vignette = 0.35;
        neon.bloom = 0.3;
        neon.grade.contrast = 1.2;
        let mut lamps = self.lamps.clone();
        match self.shot {
            Shot::Face => {
                neon.field = [0.01, 0.006, 0.004];
                neon.key = Some(key(Vec3::new(-0.8, 0.4, 0.5), 6.0, 0.02));
                neon.key_color = [1.0, 0.55, 0.25];
                neon.rim_color = [1.0, 0.6, 0.3];
            }
            Shot::Girl => {
                neon.field = [0.004, 0.012, 0.012];
                neon.key = Some(key(Vec3::new(0.4, 0.9, 0.5), 1.6, 0.08));
                neon.key_color = [0.55, 0.85, 0.8];
                neon.rim_color = [0.4, 0.8, 0.7];
                if t < 0.5 {
                    lamps.push(Lamp {
                        position: Vec3::new(-1.0, 1.4, 0.6),
                        color: [0.3, 1.0, 0.45],
                        intensity: 40.0 * (1.0 - t / 0.5),
                        range: 6.0,
                    });
                }
            }
            Shot::Boy => {
                neon.field = [0.01, 0.012, 0.01];
                // Lit from behind the wall, so the boy is a silhouette: no
                // stand-in face shows in a medium shot.
                neon.key = Some(key(Vec3::new(0.2, 0.7, -0.7), 2.5, 0.01));
                neon.key_color = [0.7, 0.85, 0.75];
                if (0.15..BLAST).contains(&t) {
                    let k = ((t - 0.15) / 0.3).clamp(0.0, 1.0);
                    lamps.push(Lamp {
                        position: Vec3::new(-6.0, 2.4, -1.6).lerp(Vec3::new(0.3, 2.3, -1.2), k),
                        color: [0.3, 1.0, 0.45],
                        intensity: 70.0,
                        range: 6.0,
                    });
                    // Its green on the wall's face either side of him.
                    for x in [-1.6_f32, 2.2] {
                        lamps.push(Lamp {
                            position: Vec3::new(x, 1.8, 0.15),
                            color: [0.3, 1.0, 0.45],
                            intensity: 18.0 * k,
                            range: 1.6,
                        });
                    }
                }
                if t >= BLAST {
                    // The blast's golden light, fading.
                    let k = (-(t - BLAST) / 0.6).exp();
                    lamps.push(Lamp {
                        position: Vec3::new(0.3, 2.4, -1.3),
                        color: [1.0, 0.7, 0.35],
                        intensity: 160.0 * k + 12.0,
                        range: 9.0,
                    });
                    neon.key_color = [1.0, 0.8, 0.55];
                    // The burst lights the wall's face either side of him
                    // (short-range lamps clear of his face).
                    for x in [-1.6_f32, 2.2] {
                        lamps.push(Lamp {
                            position: Vec3::new(x, 1.6, 0.15),
                            color: [1.0, 0.65, 0.3],
                            intensity: 30.0 * k + 2.0,
                            range: 1.6,
                        });
                    }
                }
            }
            Shot::Cloak | Shot::Duel => {
                neon.field = [0.012, 0.009, 0.007];
                neon.key = Some(key(Vec3::new(-0.4, 0.7, 0.5), 3.0, 0.06));
                neon.key_color = [1.0, 0.7, 0.4];
                for (i, &(a, b)) in BOLTS.iter().enumerate() {
                    let from = dueler_at(a, t) + Vec3::new(0.3, 1.4, 0.2);
                    let to = dueler_at(b, t) + Vec3::new(0.0, 1.3, 0.0);
                    lamps.push(Lamp {
                        position: from.lerp(to, 0.5),
                        color: if i == 0 {
                            [0.3, 1.0, 0.45]
                        } else {
                            [1.0, 0.25, 0.15]
                        },
                        intensity: 50.0,
                        range: 6.0,
                    });
                }
            }
        }
        for (i, lamp) in lamps.iter().take(MAX_LAMPS).enumerate() {
            neon.lamps[i] = if lamp.color[0] > 0.9 {
                lamp.flickering(t, i as u32)
            } else {
                *lamp
            };
        }
        neon
    }
}

/// The duelers: where each stands, which way it faces, its clip, and the
/// clip's phase. The first casts the bolt at the second.
const DUELERS: [(Vec3, f32, &str, f32, Vec3); 9] = [
    (Vec3::new(-1.6, 0.0, 0.4), 1.2, "swing", 0.3, Vec3::ZERO),
    (Vec3::new(1.4, 0.0, -0.2), -1.9, "idle", 0.0, Vec3::ZERO),
    (
        Vec3::new(-0.6, 0.0, -2.2),
        0.3,
        "run",
        0.2,
        Vec3::new(0.6, 0.0, 2.2),
    ),
    (Vec3::new(2.6, 0.0, 1.6), -2.4, "swing", 0.7, Vec3::ZERO),
    (Vec3::new(-2.8, 0.0, -1.2), 0.9, "idle", 0.5, Vec3::ZERO),
    (Vec3::new(0.6, 0.0, 2.0), -2.8, "swing", 0.1, Vec3::ZERO),
    (
        Vec3::new(-0.4, 0.0, -0.4),
        0.4,
        "backpedal",
        0.3,
        Vec3::new(-0.3, 0.0, -1.0),
    ),
    (
        Vec3::new(3.4, 0.0, -1.4),
        -1.2,
        "run",
        0.6,
        Vec3::new(-2.6, 0.0, 0.8),
    ),
    (
        Vec3::new(-3.6, 0.0, 1.4),
        1.6,
        "strafe_left",
        0.2,
        Vec3::new(0.0, 0.0, 1.2),
    ),
];

/// Who casts at whom in the duel: caster and target, by `DUELERS` index.
const BOLTS: [(usize, usize); 2] = [(0, 1), (5, 6)];

/// Where dueler `k` stands at film time `t`.
fn dueler_at(k: usize, t: f32) -> Vec3 {
    let d = DUELERS[k % DUELERS.len()];
    d.0 + d.4 * t
}

/// The first member's atlas darkened to near black: a cloak in shadow.
fn darken_first(cast: &mut Cast) {
    let mut scene = cast.scene.as_ref().clone();
    if let Some(first) = cast.members.first() {
        let images = first.beast.figure().scene.images.len();
        for image in scene.images.iter_mut().take(images) {
            for px in image.rgba.chunks_exact_mut(4) {
                for c in &mut px[..3] {
                    *c = (f32::from(*c) * 0.12) as u8;
                }
            }
        }
    }
    cast.scene = Arc::new(scene);
}

/// A broken wall top's height at `x`: ragged, lower where it broke.
fn broken_top(x: f32) -> f32 {
    2.1 + 0.35 * (x * 1.7).sin() * (x * 0.6).cos() - 0.4 * (-(x - 1.5).powi(2)).exp()
}

/// A brick wall from `x0` to `x1` along `z`, its top at `height(x)`, in
/// courses of staggered bricks of varied shade.
fn brick_wall(
    out: &mut Vec<LitVertex>,
    x0: f32,
    x1: f32,
    z: f32,
    height: &dyn Fn(f32) -> f32,
    seed: u32,
) {
    // Dark mortar behind the stones, so the joints read as shadow.
    let tallest = (0..40)
        .map(|i| height(x0 + (x1 - x0) * i as f32 / 39.0))
        .fold(0.0_f32, f32::max);
    push_box(
        out,
        Vec3::new(x0, 0.0, z - 0.26),
        Vec3::new(x1, tallest - 0.1, z + 0.17),
        0.0,
        [0.012, 0.011, 0.01],
        1.0,
    );
    let course = 0.24;
    let mut n = seed * 100_000;
    let mut row = 0;
    let mut y = 0.0;
    while y < 6.0 {
        let mut x = x0 - if row % 2 == 0 { 0.0 } else { 0.25 };
        while x < x1 {
            n += 1;
            let w = 0.46 + 0.1 * hash(n);
            let top = height(x + w * 0.5);
            // A few stones are gone, and some sit proud or sunk, chipped
            // smaller than their neighbors.
            let missing = hash(n + 11) < 0.03;
            if y + course <= top + 0.1 * hash(n + 7) && !missing {
                let chip = 0.012 + 0.03 * hash(n + 13).powi(3);
                let proud = 0.04 * (hash(n + 9) - 0.5);
                // Weathered: grime darkens toward the ground and in
                // streaks, and the stones vary from cool grey to warm.
                let grime = 0.55 + 0.45 * (y / 2.2).min(1.0);
                let streak = 0.75 + 0.25 * ((x * 3.1).sin() * 0.5 + 0.5);
                let shade = (0.06 + 0.1 * hash(n + 3)) * grime * streak;
                let warm = hash(n + 5);
                push_box(
                    out,
                    Vec3::new(x.max(x0) + chip, y + chip, z - 0.25),
                    Vec3::new((x + w - chip).min(x1), y + course - chip, z + 0.2 + proud),
                    0.0,
                    [
                        shade * (0.95 + 0.15 * warm),
                        shade * (0.95 + 0.03 * warm),
                        shade * (1.0 - 0.15 * warm),
                    ],
                    0.92,
                );
            }
            x += w;
        }
        y += course;
        row += 1;
    }
}

/// The boy's wall top around where the bolt hits, as loose blocks that fly
/// when it does.
fn wall_top_chunks() -> Vec<Chunk> {
    let mut out = Vec::new();
    for i in 0..34u32 {
        let x = -0.8 + 2.2 * hash(i * 3 + 900);
        let y = 1.6 + 0.5 * hash(i * 3 + 901);
        let s = Vec3::new(0.22, 0.12, 0.2) * (0.8 + 0.6 * hash(i * 3 + 902));
        let at = Vec3::new(x, y, -0.4);
        let out_dir = (at - Vec3::new(0.3, 2.0, -0.6)).normalize_or(Vec3::Y);
        out.push(Chunk {
            min: at - s,
            max: at + s,
            velocity: out_dir * (3.0 + 5.0 * hash(i + 77)) + Vec3::new(0.0, 4.0, 1.2),
            spin: 4.0 * (hash(i + 55) - 0.5),
            shade: 0.1 + 0.06 * hash(i + 33),
        });
    }
    out
}
