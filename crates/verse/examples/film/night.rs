//! The Draconia night sky, phase D3 (`docs/verse/draconia/shots.md`, the
//! outline after 26 s): hooded wraiths circling the moon in a vortex of
//! cloud above the courtyard, wraiths crossing the moonlit sky, Draconia
//! from behind looking up at them, and her boots walking toward us before a
//! wall of fire and dueling silhouettes. Close-ups of faces are plates the
//! cut slates until the face work renders them.

use std::sync::Arc;

use glam::{Mat3, Vec3};
use serde::Deserialize;
use verse::fx::{Particles, Spawn, Sprite, feathers, sheet};
use verse::mesh::Mesh;
use verse::pbr::textured::{Figure, TexturedScene};
use verse::pbr::{Lamp, LitVertex, MAX_LAMPS, Material, Neon};
use verse::zones::everglade::player::Beast;
use verse::zones::everglade_pack::ZonePack;

use crate::peacock::{cobbles, draconia, hash, key, ruins};

/// Which shot of D3.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shot {
    /// 26–31 s and 47–51 s: a warm plate behind a face close-up (slated).
    Plate,
    /// 31–34 s: the wraiths' vortex round the moon, she tiny below.
    Vortex,
    /// 35–37 s: wraiths crossing the moonlit sky.
    Sky,
    /// 43–47 s: Draconia from behind, looking up at them.
    Behind,
    /// 51–56 s: her boots walking toward us before a wall of fire.
    Boots,
}

/// The moon, far up the sky.
const MOON: Vec3 = Vec3::new(0.0, 60.0, -40.0);
/// Where the vortex wheels, the moon at its eye: nearer and lower, so
/// the camera looking up from the courtyard holds it whole.
const VORTEX: Vec3 = Vec3::new(0.0, 30.0, -20.0);

/// The moon for a shot: at the vortex's eye, or far up the sky.
fn moon(shot: Shot) -> Vec3 {
    if shot == Shot::Vortex { VORTEX } else { MOON }
}

pub struct Set {
    shot: Shot,
    time: f32,
    fx: Particles,
    her: Option<(Beast, Arc<TexturedScene>, Vec<usize>)>,
    lamps: Vec<Lamp>,
}

impl Set {
    pub fn world(shot: Shot) -> Mesh {
        let mut lit = Vec::new();
        cobbles(&mut lit);
        if shot != Shot::Sky {
            ruins(&mut lit);
        }
        Mesh {
            lit,
            ..Mesh::default()
        }
    }

    pub fn new(shot: Shot, pack: &ZonePack) -> Result<Self, String> {
        let mut fx = Particles::new(0xD3);
        let mut lamps = Vec::new();
        let her = if matches!(shot, Shot::Vortex | Shot::Behind | Shot::Boots) {
            let form = pack
                .form("npc/alice")
                .ok_or("the pack carries no npc/alice")?;
            let beast = Beast::new(pack, form)?;
            let mut scene = beast.figure().scene.as_ref().clone();
            draconia(&mut scene);
            let staff = beast
                .figure()
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, v)| v.pos[2] < -0.12 && v.pos[0] > 0.1 && v.pos[1] > 1.1)
                .map(|(i, _)| i)
                .collect();
            Some((beast, Arc::new(scene), staff))
        } else {
            None
        };
        let mut fire = |fx: &mut Particles, at: Vec3, power: f32| {
            fx.start("ruin_fire", Spawn::at(at));
            lamps.push(Lamp {
                position: at + Vec3::Y * 0.6,
                color: [1.0, 0.45, 0.14],
                intensity: power,
                range: 9.0,
            });
        };
        match shot {
            Shot::Plate => {
                fire(&mut fx, Vec3::new(-1.5, 0.3, -2.0), 30.0);
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.5, 1.0)).scaled(0.4),
                );
            }
            Shot::Vortex => {
                for i in 0..10 {
                    let a = i as f32 / 10.0 * std::f32::consts::TAU;
                    fire(
                        &mut fx,
                        Vec3::new(a.cos() * 7.0, 0.2, a.sin() * 7.0 - 2.0),
                        12.0,
                    );
                }
            }
            Shot::Sky => {}
            Shot::Behind => {
                fire(&mut fx, Vec3::new(-4.0, 0.2, -3.0), 25.0);
                fire(&mut fx, Vec3::new(5.0, 0.2, -5.0), 25.0);
            }
            Shot::Boots => {
                // A wall of fire behind her, from one side to the other.
                for i in 0..9 {
                    fire(&mut fx, Vec3::new(-8.0 + i as f32 * 2.0, 0.1, -5.0), 25.0);
                }
                fx.start("night_embers", Spawn::at(Vec3::new(0.0, 0.2, -1.0)));
            }
        }
        let mut set = Self {
            shot,
            time: 0.0,
            fx,
            her,
            lamps,
        };
        for _ in 0..90 {
            set.fx.tick(1.0 / 30.0, |_, _| 0.0);
        }
        set.advance(1.0 / 30.0, 0.0);
        Ok(set)
    }

    pub fn advance(&mut self, dt: f32, t: f32) {
        self.time = t;
        let steps = (dt * 120.0).ceil().max(1.0) as usize;
        for _ in 0..steps {
            self.fx.tick(dt / steps as f32, |_, _| 0.0);
        }
        let shot = self.shot;
        if let Some((beast, _, _)) = &mut self.her {
            match shot {
                // She stands with her back to us, looking up.
                Shot::Vortex | Shot::Behind => {
                    beast.pose_clip(
                        Vec3::ZERO,
                        std::f32::consts::PI,
                        1.0,
                        "idle",
                        t + 3.0,
                        true,
                        dt,
                    );
                }
                // She walks toward us.
                Shot::Boots => {
                    beast.pose_clip(
                        Vec3::new(0.0, 0.0, -1.2 + t * 0.9),
                        0.0,
                        1.0,
                        "walk",
                        t,
                        true,
                        dt,
                    );
                }
                _ => {}
            }
        }
    }

    pub fn frame(&self) -> Mesh {
        let t = self.time;
        let mut mesh = Mesh {
            neon: Some(self.stage(t)),
            ..Mesh::default()
        };
        if let Some((beast, scene, staff)) = &self.her {
            let mut vertices = beast.vertices().to_vec();
            if let Some(&first) = staff.first() {
                let point = vertices[first].pos;
                for &i in staff {
                    vertices[i].pos = point;
                }
            }
            mesh.figure = Some(Figure {
                scene: scene.clone(),
                vertices: Arc::new(vertices),
            });
        }
        let sky = matches!(self.shot, Shot::Vortex | Shot::Sky | Shot::Behind);
        if sky {
            // The moon and its halo.
            mesh.sprites
                .push(feathers::dot(moon(self.shot), 3.0, [40.0, 40.0, 38.0], 1.0));
            mesh.sprites
                .push(feathers::dot(moon(self.shot), 14.0, [0.35, 0.42, 0.6], 1.0));
            clouds(&mut mesh.sprites, self.shot == Shot::Vortex, t);
        }
        match self.shot {
            Shot::Vortex => {
                // Wraiths circling the moon in a slow spiral.
                for i in 0..36u32 {
                    let ring = 9.0 + 14.0 * hash(i * 3 + 1);
                    let a = i as f32 / 36.0 * std::f32::consts::TAU + t * (0.4 - ring * 0.006);
                    let at = VORTEX
                        + Vec3::new(
                            a.cos() * ring * 0.8,
                            1.0 - 3.0 * hash(i * 3 + 2),
                            a.sin() * ring * 0.5,
                        );
                    wraith(
                        &mut mesh.lit,
                        at,
                        a + 1.57,
                        1.8 + 1.0 * hash(i * 3 + 3),
                        t + i as f32,
                    );
                }
            }
            Shot::Sky | Shot::Behind => {
                for i in 0..14u32 {
                    let lane = hash(i * 5 + 1);
                    let speed = 3.0 + 4.0 * hash(i * 5 + 2);
                    let x = -40.0
                        + 80.0 * hash(i * 5 + 3)
                        + speed * t * if i % 2 == 0 { 1.0 } else { -1.0 };
                    let at = Vec3::new(x * 0.6, 14.0 + 22.0 * lane, -18.0 - 14.0 * hash(i * 5 + 4));
                    wraith(
                        &mut mesh.lit,
                        at,
                        if i % 2 == 0 { 0.0 } else { 3.14 },
                        3.0 + 2.0 * hash(i * 5 + 5),
                        t + i as f32,
                    );
                }
            }
            Shot::Boots => {
                // Dueling silhouettes in front of the fire, and their bolts.
                for (k, x) in [-5.0_f32, -2.6, 2.2, 4.8].into_iter().enumerate() {
                    let at = Vec3::new(x, 0.0, -4.0);
                    wraith(&mut mesh.lit, at + Vec3::Y * 1.0, 0.0, 1.0, t + k as f32);
                }
                let flicker = 0.85 + 0.15 * (t * 23.0).sin();
                mesh.ribbons.extend(feathers::beam(
                    Vec3::new(-5.0, 1.4, -4.0),
                    Vec3::new(-2.6, 1.3, -4.0),
                    0.04,
                    [6.0 * flicker, 1.0, 0.6],
                ));
                mesh.ribbons.extend(feathers::beam(
                    Vec3::new(2.2, 1.4, -4.0),
                    Vec3::new(4.8, 1.3, -4.0),
                    0.04,
                    [0.6, 6.0 * flicker, 1.2],
                ));
            }
            Shot::Plate => {}
        }
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    fn stage(&self, t: f32) -> Neon {
        let mut neon = Neon::neutral(t);
        neon.fog_start = 60.0;
        neon.fog_end = 400.0;
        neon.vignette = 0.4;
        neon.bloom = 0.3;
        neon.grade.contrast = 1.2;
        neon.field = [0.012, 0.022, 0.045];
        // The moon as the key, from up and behind.
        neon.key = Some(key((MOON - Vec3::Y * 20.0).normalize(), 2.5, 0.1));
        neon.key_color = [0.6, 0.7, 1.0];
        if matches!(self.shot, Shot::Plate | Shot::Boots) {
            neon.field = [0.014, 0.009, 0.006];
            neon.key = Some(key(Vec3::new(0.0, 0.5, -0.8), 3.0, 0.03));
            neon.key_color = [1.0, 0.55, 0.25];
        }
        for (i, lamp) in self.lamps.iter().take(MAX_LAMPS).enumerate() {
            neon.lamps[i] = lamp.flickering(t, i as u32);
        }
        neon
    }
}

/// Dark clouds: a layer across the sky, or a vortex wheeling round the
/// moon.
fn clouds(out: &mut Vec<Sprite>, vortex: bool, t: f32) {
    let (rect, layer) = (
        sheet::find("smoke").map(|s| s.rect(9)).unwrap_or_default(),
        sheet::layer("smoke").unwrap_or_default(),
    );
    for i in 0..60u32 {
        let at = if vortex {
            // A clear eye round the moon, the cloud wheeling outside it.
            let r = 16.0 + 40.0 * hash(i * 7 + 1);
            let a = hash(i * 7 + 2) * std::f32::consts::TAU + t * 0.25 * (30.0 / r);
            VORTEX + Vec3::new(a.cos() * r * 0.7, 2.0 - 4.0 * hash(i * 7 + 3), a.sin() * r * 0.45)
        } else {
            Vec3::new(
                -60.0 + 120.0 * hash(i * 7 + 1) + t * 1.5,
                25.0 + 25.0 * hash(i * 7 + 2),
                -50.0 - 30.0 * hash(i * 7 + 3),
            )
        };
        out.push(Sprite {
            rect_a: rect,
            rect_b: rect,
            layer,
            additive: 0.0,
            alpha: 0.75,
            angle: hash(i * 7 + 4) * 6.28,
            priority: 2,
            ..feathers::dot(at, 7.0 + 6.0 * hash(i * 7 + 5), [0.03, 0.04, 0.06], 1.0)
        });
    }
}

/// A hooded wraith: a tattered cloak with a hood and no face, hanging from
/// its hood at `at`, turned `yaw`, `scale` times human size, its rags
/// stirring with `t`.
fn wraith(out: &mut Vec<LitVertex>, at: Vec3, yaw: f32, scale: f32, t: f32) {
    let (around, down) = (16usize, 10usize);
    let turn = Mat3::from_rotation_y(yaw) * Mat3::from_rotation_x(0.3);
    let point = |i: usize, j: usize| -> Vec3 {
        let u = i as f32 / around as f32;
        let v = j as f32 / down as f32;
        let theta = u * std::f32::consts::TAU;
        // A hood above, then a body that widens, its hem torn into rags of
        // different lengths that trail and stir.
        let r = if v < 0.25 {
            0.32 * (v / 0.25).sqrt() + 0.02
        } else {
            0.34 + 0.25 * (v - 0.25)
        };
        let rag = 0.5 * v * v * hash((i as u32) * 13 + 1);
        let stir = 0.12 * v * (t * 2.5 + theta * 3.0).sin();
        let local = Vec3::new(
            r * theta.sin() + stir,
            -2.0 * v - rag,
            r * theta.cos() * 0.8,
        );
        at + turn * (local * scale)
    };
    let color = [0.006, 0.006, 0.008];
    for j in 0..down {
        for i in 0..around {
            let [a, b, c, d] = [
                point(i, j),
                point(i + 1, j),
                point(i + 1, j + 1),
                point(i, j + 1),
            ];
            let normal = (b - a).cross(d - a).normalize_or(Vec3::Y);
            for p in [a, b, c, a, c, d] {
                out.push(LitVertex {
                    pos: p.to_array(),
                    normal: normal.to_array(),
                    tangent: Vec3::Y.to_array(),
                    local: (p - at).to_array(),
                    color,
                    params: [0.0, 0.95, Material::Dark.code(), 1.0],
                });
            }
        }
    }
}
