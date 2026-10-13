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
use verse::pbr::{Lamp, LitVertex, MAX_LAMPS, Material, Neon};
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
    /// D2 shot 7: Draconia's profile against the duel (a plate until her
    /// face exists: embers, and the duelers out of focus behind).
    Profile,
    /// D2 shot 8: a boy hauling a girl away as a bolt cuts across, fire in
    /// the foreground, seen from behind.
    Haul,
    /// D2 shot 9: a boy running toward us past fire, backlit.
    Runner,
    /// D2 shot 10: a student's shouting mouth (a fire plate until faces
    /// exist).
    Shout,
    /// D2 shots 11-12: Draconia's eye (a fire plate until her face exists).
    Eye,
    /// D2 shot 13: Draconia alone in the ruins, low and wide.
    Alone,
    /// D2 shot 14: a dark wave rolling across the cobbles toward us.
    Wave,
    /// D2 shot 15: the whole courtyard, fires and duels, a hooded figure
    /// in the right foreground.
    Wide,
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
            } else {
                uniform(&mut scene);
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
        Look::Boy => {
            uniform(&mut scene);
            Vec::new()
        }
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
            Shot::Haul | Shot::Runner | Shot::Profile | Shot::Shout | Shot::Eye => {
                brick_wall(&mut lit, -9.0, 9.0, -4.0, &|x| 2.4 + 0.6 * broken_top(x), 5);
            }
            Shot::Alone | Shot::Wave | Shot::Wide => {
                crate::peacock::ruins(&mut lit);
                // The courtyard's Gothic ranges: one across the far side
                // and a wing down each flank, with a tower.
                facade(
                    &mut lit,
                    Vec3::new(-18.0, 0.0, -13.0),
                    Vec3::X,
                    36.0,
                    9.0,
                    7,
                );
                facade(
                    &mut lit,
                    Vec3::new(-14.0, 0.0, -12.0),
                    Vec3::Z,
                    16.0,
                    8.0,
                    8,
                );
                facade(
                    &mut lit,
                    Vec3::new(14.0, 0.0, 4.0),
                    Vec3::NEG_Z,
                    16.0,
                    8.0,
                    9,
                );
                tower(&mut lit, Vec3::new(5.0, 0.0, -14.0), 3.2, 17.0);
            }
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
            Shot::Shout | Shot::Eye | Shot::Wave => &[],
            Shot::Haul => &[Look::Boy, Look::Alice],
            Shot::Runner => &[Look::Boy],
            Shot::Alone => &[Look::Draconia],
            Shot::Profile | Shot::Wide | Shot::Duel => &[
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
        let cast = Cast::new(pack, looks)?;
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
            Shot::Shout | Shot::Eye => {
                fire(&mut fx, &mut lamps, Vec3::new(-1.6, 0.9, -1.5), 0.0);
                fire(&mut fx, &mut lamps, Vec3::new(1.2, 0.6, -2.5), 0.0);
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.8, 0.0)).scaled(0.4),
                );
            }
            Shot::Profile => {
                for at in [Vec3::new(-3.0, 0.3, -3.0), Vec3::new(3.0, 0.3, -2.5)] {
                    fire(&mut fx, &mut lamps, at, 30.0);
                }
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.8, 4.0)).scaled(0.4),
                );
            }
            Shot::Haul => {
                fire(&mut fx, &mut lamps, Vec3::new(-1.3, 0.1, 3.2), 25.0);
                fire(&mut fx, &mut lamps, Vec3::new(2.5, 0.2, -2.0), 30.0);
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.4, 1.0)).scaled(0.5),
                );
            }
            Shot::Runner => {
                for at in [
                    Vec3::new(-1.5, 0.2, -3.0),
                    Vec3::new(1.8, 0.2, -2.0),
                    Vec3::new(-2.2, 0.2, -4.5),
                ] {
                    fire(&mut fx, &mut lamps, at, 40.0);
                }
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.4, 0.0)).scaled(0.6),
                );
            }
            Shot::Alone => {
                for at in [
                    Vec3::new(-3.5, 0.2, -1.5),
                    Vec3::new(2.8, 0.2, -2.5),
                    Vec3::new(4.5, 0.3, 0.5),
                    Vec3::new(-1.5, 0.2, -4.0),
                ] {
                    fire(&mut fx, &mut lamps, at, 25.0);
                }
                fx.start("night_embers", Spawn::at(Vec3::new(0.0, 0.3, 0.0)));
            }
            Shot::Wave => {
                fire(&mut fx, &mut lamps, Vec3::new(-4.0, 0.2, -3.0), 20.0);
            }
            Shot::Wide => {
                for at in [
                    Vec3::new(-6.0, 0.2, -3.0),
                    Vec3::new(5.0, 0.2, -3.5),
                    Vec3::new(-2.5, 0.3, -5.0),
                    Vec3::new(1.5, 0.3, -6.0),
                    Vec3::new(-4.5, 0.2, 1.0),
                ] {
                    fire(&mut fx, &mut lamps, at, 30.0);
                }
                // Fires along the far range's foot light its face warm.
                for at in [
                    Vec3::new(-9.0, 0.2, -11.4),
                    Vec3::new(-2.5, 0.2, -11.8),
                    Vec3::new(4.0, 0.2, -11.4),
                    Vec3::new(10.0, 0.2, -11.0),
                ] {
                    fire(&mut fx, &mut lamps, at, 140.0);
                }
                fx.start("night_embers", Spawn::at(Vec3::new(0.0, 0.3, -1.0)));
                fx.start(
                    "ground_mist",
                    Spawn::at(Vec3::new(0.0, 0.0, -2.0)).scaled(0.7),
                );
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
        if shot == Shot::Wave {
            // The wave starts with the shot, after the fires have settled.
            set.fx
                .start("shadow_wave", Spawn::at(Vec3::new(0.0, 0.2, -3.5)));
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
                (Shot::Shout | Shot::Eye | Shot::Wave, _) => {}
                (Shot::Haul, 0) => {
                    // He hauls her away from us, toward the far wall.
                    let at = Vec3::new(0.3, 0.0, 1.0 - t * 2.6);
                    b.pose_clip(at, std::f32::consts::PI, 1.0, "run", t, true, dt);
                }
                (Shot::Haul, _) => {
                    let at = Vec3::new(-0.35, 0.0, 1.5 - t * 2.4);
                    b.pose_clip(
                        at,
                        std::f32::consts::PI + 0.2,
                        1.0,
                        "run",
                        t + 0.3,
                        true,
                        dt,
                    );
                }
                (Shot::Runner, _) => {
                    // Toward us, backlit by the fires behind him.
                    // He pulls up short of the lens, the fires behind him.
                    let at = Vec3::new(0.2, 0.0, (-3.0 + t * 3.8).min(-0.2));
                    b.pose_clip(at, 0.15, 1.0, "run", t, true, dt);
                }
                (Shot::Alone, _) => {
                    b.pose_clip(Vec3::ZERO, -0.3, 1.0, "idle", t + 2.0, true, dt);
                }
                (Shot::Profile | Shot::Wide, _) => {
                    let (_, yaw, clip, phase, _) = DUELERS[k % DUELERS.len()];
                    let at = dueler_at(k, t) + wide_offset(shot);
                    b.pose_clip(at, yaw, 1.0, clip, phase + t, true, dt);
                }
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
            figure: (!self.cast.members.is_empty()).then(|| self.cast.figure()),
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
            Shot::Haul if t > 0.2 => {
                let k = ((t - 0.2) / 0.25).clamp(0.0, 1.0);
                let from = Vec3::new(5.0, 1.7, -1.0);
                let to = Vec3::new(-5.0, 1.4, 0.4);
                mesh.ribbons
                    .extend(feathers::beam(from, from.lerp(to, k), 0.05, GREEN));
            }
            Shot::Cloak | Shot::Duel | Shot::Wide | Shot::Profile => {
                let offset = wide_offset(self.shot);
                let cast = self.cast.members.len();
                for (i, &(a, b)) in BOLTS.iter().enumerate() {
                    if a >= cast || b >= cast {
                        continue;
                    }
                    let from = dueler_at(a, t) + offset + Vec3::new(0.3, 1.4, 0.2);
                    let to = dueler_at(b, t) + offset + Vec3::new(0.0, 1.3, 0.0);
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
        if matches!(self.shot, Shot::Wide | Shot::Alone | Shot::Wave) {
            // Firelight in some of the far range's windows.
            for i in 0..9u32 {
                if hash(i * 31 + 7) < 0.45 {
                    continue;
                }
                let flicker = 0.85 + 0.15 * (t * 7.0 + i as f32).sin();
                let x = -18.0 + i as f32 * 4.0 + 2.0;
                let mut glow = feathers::dot(
                    Vec3::new(x, 4.0, -12.5),
                    0.9,
                    [9.0 * flicker, 4.5 * flicker, 1.3 * flicker],
                    1.0,
                );
                glow.tail = Vec3::Y * 2.6;
                mesh.sprites.push(glow);
            }
        }
        if self.shot == Shot::Wide {
            // A hooded figure filling the right foreground.
            cloak(&mut mesh.lit, Vec3::new(1.05, 0.6, 5.0), 0.4);
        }
        if self.shot == Shot::Cloak {
            // A dark wool cloak sweeping right across the lens.
            cloak(&mut mesh.lit, Vec3::new(-1.7 + t * 4.2, 0.0, 3.6), t);
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
                            intensity: 30.0 * k + 9.0,
                            range: 1.6,
                        });
                    }
                }
            }
            Shot::Shout | Shot::Eye | Shot::Profile => {
                neon.field = [0.012, 0.007, 0.004];
                neon.key = Some(key(Vec3::new(-0.6, 0.6, 0.5), 3.0, 0.04));
                neon.key_color = [1.0, 0.6, 0.3];
            }
            Shot::Haul | Shot::Runner => {
                // Firelight from behind them: they read as silhouettes.
                neon.field = [0.014, 0.009, 0.006];
                neon.key = Some(key(Vec3::new(0.2, 0.5, -0.8), 4.0, 0.02));
                neon.key_color = [1.0, 0.55, 0.25];
            }
            Shot::Alone | Shot::Wave | Shot::Wide => {
                neon.field = [0.012, 0.02, 0.04];
                // Moonlight from behind the camera's left, so the ranges'
                // faces read.
                neon.key = Some(if self.shot == Shot::Alone {
                    // For her, the moon is behind: a rim, and no stand-in
                    // face to read.
                    key(Vec3::new(0.3, 0.6, -0.75), 4.0, 0.03)
                } else {
                    key(Vec3::new(-0.4, 0.7, 0.6), 4.5, 0.12)
                });
                neon.key_color = [0.55, 0.65, 1.0];
                neon.fog_start = 15.0;
                neon.fog_end = 90.0;
            }
            Shot::Cloak | Shot::Duel => {
                neon.field = [0.012, 0.009, 0.007];
                neon.key = Some(key(Vec3::new(-0.4, 0.7, 0.5), 3.0, 0.06));
                neon.key_color = [1.0, 0.7, 0.4];
                for (i, &(a, b)) in BOLTS.iter().enumerate() {
                    if a >= self.cast.members.len() || b >= self.cast.members.len() {
                        continue;
                    }
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

/// Where a shot moves the duel: back, for the whole courtyard.
fn wide_offset(shot: Shot) -> Vec3 {
    if shot == Shot::Wide {
        Vec3::new(0.0, 0.0, -4.0)
    } else {
        Vec3::ZERO
    }
}

/// Where dueler `k` stands at film time `t`.
fn dueler_at(k: usize, t: f32) -> Vec3 {
    let d = DUELERS[k % DUELERS.len()];
    d.0 + d.4 * t
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

/// Recolors a stand-in's atlas toward a school uniform: green cloth to a
/// black wool robe, saturated browns and leathers to charcoal grey, and the
/// rest (skin, linen, red trim) left alone.
fn uniform(scene: &mut TexturedScene) {
    let to_lin = |v: u8| (f32::from(v) / 255.0).powf(2.2);
    let to_srgb = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
    for image in &mut scene.images {
        let (w, h) = (image.width as usize, image.height as usize);
        // Alice's hair strips live in the atlas's bottom band.
        let band = (h as f32 * 0.84) as usize;
        for y in 0..band.min(h) {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let [r, g, b] = [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2]].map(to_lin);
                let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                let sat = if max > 1e-4 { (max - min) / max } else { 0.0 };
                let out = if g > r * 1.08 && g > b * 1.02 {
                    let k = (luma / 0.05).min(1.5);
                    [0.016 * k, 0.016 * k, 0.019 * k]
                } else if r > g && g > b && sat > 0.35 && luma < 0.09 {
                    let k = (luma / 0.05).min(1.5);
                    [0.035 * k, 0.035 * k, 0.038 * k]
                } else {
                    continue;
                };
                image.rgba[i] = to_srgb(out[0]);
                image.rgba[i + 1] = to_srgb(out[1]);
                image.rgba[i + 2] = to_srgb(out[2]);
            }
        }
    }
}

/// A hooded cloak of dark wool at `at`, its hem swinging with `t`: a
/// drape round a half cylinder with deep folds, flaring toward the hem.
fn cloak(out: &mut Vec<LitVertex>, at: Vec3, t: f32) {
    let (around, down) = (28usize, 14usize);
    // Rows above the shoulders (negative `v`) close into a hood.
    let hood = 5usize;
    let point = |i: usize, j: usize| -> Vec3 {
        let u = i as f32 / around as f32;
        let v = (j as f32 - hood as f32) / down as f32;
        let theta = std::f32::consts::PI * (u - 0.5) * 1.6;
        let flare = if v < 0.0 {
            // A rounded hood, narrowing to its peak.
            0.3 * (1.0 - (-v / (hood as f32 / down as f32)).powi(2))
                .max(0.0)
                .sqrt()
                + 0.02
        } else {
            0.32 + 0.22 * v
        };
        let fold = 0.05 * (theta * 9.0 + v * 2.0 + t * 3.0).sin() * (0.3 + v);
        let r = flare + fold;
        let swing = 0.08 * v * v * (t * 5.0).sin();
        at + Vec3::new(
            r * theta.sin() - swing,
            1.85 - 1.7 * v,
            r * theta.cos() * 0.7,
        )
    };
    let color = [0.008, 0.008, 0.01];
    for j in 0..down + hood {
        for i in 0..around {
            let [a, b, c, d] = [
                point(i, j),
                point(i + 1, j),
                point(i + 1, j + 1),
                point(i, j + 1),
            ];
            let normal = (b - a).cross(d - a).normalize_or(Vec3::Z);
            for p in [a, b, c, a, c, d] {
                out.push(LitVertex {
                    pos: p.to_array(),
                    normal: normal.to_array(),
                    tangent: Vec3::Y.to_array(),
                    local: (p - at).to_array(),
                    color,
                    params: [0.0, 0.9, Material::Dark.code(), 1.0],
                });
            }
        }
    }
}

/// A Gothic range: a wall `length` m long from `start` along `along`,
/// `height` m tall, in bays of tall pointed windows between buttresses,
/// under a crenellated parapet. Its face looks to the left of `along`.
fn facade(out: &mut Vec<LitVertex>, start: Vec3, along: Vec3, length: f32, height: f32, seed: u32) {
    let face = Vec3::Y.cross(along).normalize_or(Vec3::Z);
    let place = |u: f32, y: f32, d: f32| start + along * u + Vec3::Y * y + face * d;
    let stone = |n: u32| {
        let k = 0.05 + 0.04 * hash(seed * 1000 + n);
        [k * 1.04, k, k * 0.92]
    };
    // A box between two along-positions, two heights, and two depths.
    let slab =
        |out: &mut Vec<LitVertex>, u0: f32, u1: f32, y0: f32, y1: f32, d0: f32, d1: f32, n: u32| {
            let a = place(u0, y0, d0);
            let b = place(u1, y1, d1);
            push_box(out, a.min(b), a.max(b), 0.0, stone(n), 0.9);
        };
    let bay = 4.0;
    let bays = (length / bay).floor() as u32;
    let mut n = 0;
    for i in 0..bays {
        let u = i as f32 * bay;
        // The wall either side of the window, under it, and over its head.
        let (w0, w1) = (u + 1.2, u + bay - 1.2);
        let (sill, head) = (2.2, height - 2.4);
        slab(out, u, w0, 0.0, height, -0.5, 0.0, n);
        slab(out, w1, u + bay, 0.0, height, -0.5, 0.0, n + 1);
        slab(out, w0, w1, 0.0, sill, -0.5, 0.0, n + 2);
        // The pointed head: steps narrowing to the apex.
        for k in 0..6 {
            let kk = k as f32 / 6.0;
            let inset = (w1 - w0) * 0.5 * (1.0 - (1.0 - kk).sqrt());
            slab(
                out,
                w0,
                w0 + inset + 0.02,
                head + kk * 1.6,
                head + (kk + 1.0 / 6.0) * 1.6,
                -0.5,
                0.0,
                n + 3,
            );
            slab(
                out,
                w1 - inset - 0.02,
                w1,
                head + kk * 1.6,
                head + (kk + 1.0 / 6.0) * 1.6,
                -0.5,
                0.0,
                n + 4,
            );
        }
        slab(out, w0, w1, head + 1.6, height, -0.5, 0.0, n + 5);
        // Dark glass set back in the opening.
        slab(out, w0, w1, sill, head + 1.6, -0.45, -0.4, n + 6);
        // A buttress and a pinnacle.
        slab(
            out,
            u - 0.3,
            u + 0.3,
            0.0,
            height + 1.2,
            0.0,
            0.9 - 0.2 * hash(i),
            n + 7,
        );
        slab(
            out,
            u - 0.15,
            u + 0.15,
            height + 1.2,
            height + 2.4,
            0.2,
            0.5,
            n + 8,
        );
        // Crenels along the top.
        let mut c = u;
        while c < u + bay {
            slab(out, c, c + 0.5, height, height + 0.7, -0.5, -0.1, n + 9);
            c += 1.0;
        }
        n += 10;
    }
}

/// A square tower `width` m across and `height` m tall at `at`, with a
/// pointed roof.
fn tower(out: &mut Vec<LitVertex>, at: Vec3, width: f32, height: f32) {
    let h = width * 0.5;
    push_box(
        out,
        at - Vec3::new(h, 0.0, h),
        at + Vec3::new(h, height, h),
        0.0,
        [0.05, 0.048, 0.045],
        0.9,
    );
    // The roof: stacked, narrowing blocks.
    for k in 0..8 {
        let kk = k as f32;
        let w = h * (1.0 - kk / 8.0);
        push_box(
            out,
            at + Vec3::new(-w, height + kk * 0.7, -w),
            at + Vec3::new(w, height + kk * 0.7 + 0.7, w),
            0.0,
            [0.03, 0.03, 0.034],
            0.8,
        );
    }
}
