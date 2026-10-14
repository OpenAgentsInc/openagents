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
    /// 41–43 s: giant spiders behind a great web in the ruined garden, a
    /// green bolt and then a red one striking among them.
    Spiders,
    /// 58–60 s: Draconia striding away from us through the fire, a green
    /// bolt cutting across behind her.
    Stride,
    /// 67–70 s: a blue star flaring in the night sky, sparks falling.
    Star,
    /// 70–73 s: a wraith lunging at a student by the fires, others fallen.
    Attack,
    /// 93–95 s: wide under the moon, wraiths filling the sky, frost
    /// spreading over the courtyard from where she stands, tiny.
    Frost,
    /// 118–119 s: three wraiths recoiling from a cold blue-white light.
    Recoil,
    /// 92–93 s: a wraith's hood close, empty dark inside it.
    Hood,
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
        if !matches!(shot, Shot::Sky | Shot::Star) {
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
        let her = if matches!(
            shot,
            Shot::Vortex | Shot::Behind | Shot::Boots | Shot::Stride | Shot::Attack | Shot::Frost
        ) {
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
            Shot::Sky | Shot::Star | Shot::Recoil | Shot::Hood => {}
            Shot::Frost => {
                for at in [
                    Vec3::new(-9.0, 0.2, -10.0),
                    Vec3::new(-4.0, 0.2, -12.0),
                    Vec3::new(5.0, 0.2, -11.0),
                    Vec3::new(10.0, 0.2, -9.0),
                ] {
                    fire(&mut fx, at, 30.0);
                }
            }
            Shot::Stride => {
                for at in [
                    Vec3::new(-2.5, 0.2, -4.0),
                    Vec3::new(2.0, 0.2, -6.0),
                    Vec3::new(-1.0, 0.2, -8.0),
                ] {
                    fire(&mut fx, at, 40.0);
                }
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.4, -2.0)).scaled(0.6),
                );
            }
            Shot::Attack => {
                for at in [Vec3::new(-3.5, 0.2, -2.0), Vec3::new(3.5, 0.2, -3.0)] {
                    fire(&mut fx, at, 45.0);
                }
                fx.start(
                    "night_embers",
                    Spawn::at(Vec3::new(0.0, 0.4, -1.0)).scaled(0.6),
                );
            }
            Shot::Spiders => {
                fire(&mut fx, Vec3::new(-3.0, 0.2, -5.0), 30.0);
                fire(&mut fx, Vec3::new(2.5, 0.2, -6.0), 30.0);
            }
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
                // Tiny, far off, facing the wraiths.
                Shot::Frost => {
                    beast.pose_clip(
                        Vec3::new(0.0, 0.0, -8.0),
                        std::f32::consts::PI,
                        1.0,
                        "idle",
                        t,
                        true,
                        dt,
                    );
                }
                // She strides away from us.
                Shot::Stride => {
                    beast.pose_clip(
                        Vec3::new(0.0, 0.0, -0.5 - t * 1.3),
                        std::f32::consts::PI,
                        1.0,
                        "walk",
                        t,
                        true,
                        dt,
                    );
                }
                // The student, backlit, staggering back from the wraith.
                Shot::Attack => {
                    beast.pose_clip(
                        Vec3::new(-0.6, 0.0, -1.5 - 0.6 * t),
                        2.6,
                        1.0,
                        "backpedal",
                        t,
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
        let sky = matches!(
            self.shot,
            Shot::Vortex | Shot::Sky | Shot::Behind | Shot::Frost
        );
        if self.shot == Shot::Star {
            // A cloudy night sky behind the star.
            clouds(&mut mesh.sprites, false, t);
        }
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
                    let size = 3.0 + 2.0 * hash(i * 5 + 5);
                    wraith(
                        &mut mesh.lit,
                        at,
                        if i % 2 == 0 { 0.0 } else { 3.14 },
                        size,
                        t + i as f32,
                    );
                    wisps(&mut mesh.sprites, at, size, t, i);
                }
                if self.shot == Shot::Behind {
                    // The stand-off: a field of wraiths hanging low over the
                    // far courtyard, facing her, ranks fading into the dark.
                    for i in 0..40u32 {
                        let row = (i / 10) as f32;
                        let at = Vec3::new(
                            -14.0 + 28.0 * ((i % 10) as f32 + 0.5 * hash(i + 300)) / 10.0,
                            // They sink and press closer as the shot runs.
                            2.4 + 1.2 * row + 0.24 * (t * 0.8 + i as f32).sin() - 0.15 * t,
                            -9.0 - 4.0 * row - 2.0 * hash(i + 301) + 1.3 * t,
                        );
                        wraith(&mut mesh.lit, at, 0.0, 1.1, t + i as f32 * 1.7);
                        wisps(&mut mesh.sprites, at, 1.1, t, i + 100);
                    }
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
            Shot::Frost => {
                // Wraiths filling the sky.
                for i in 0..30u32 {
                    let at = Vec3::new(
                        -30.0 + 60.0 * hash(i * 3 + 801),
                        8.0 + 20.0 * hash(i * 3 + 802),
                        -20.0 - 20.0 * hash(i * 3 + 803),
                    );
                    let size = 1.5 + 1.5 * hash(i * 3 + 804);
                    wraith(&mut mesh.lit, at, hash(i + 805) * 6.28, size, t + i as f32);
                    wisps(&mut mesh.sprites, at, size, t, i + 300);
                }
                // Frost spreading over the cobbles from her: pale, ground-
                // laid patches, an expanding ring of them.
                let reach = 3.0 + 9.0 * (t / 2.0).min(1.0);
                for i in 0..160u32 {
                    let a = hash(i + 900) * std::f32::consts::TAU;
                    let r = reach * hash(i + 901).sqrt();
                    let at = Vec3::new(a.cos() * r, 0.04, -8.0 + a.sin() * r * 0.8);
                    mesh.sprites.push(Sprite {
                        facing: verse::fx::Facing::Ground,
                        angle: hash(i + 902) * 6.28,
                        ..feathers::dot(at, 0.9 + 0.8 * hash(i + 903), [0.25, 0.32, 0.45], 1.0)
                    });
                }
            }
            Shot::Recoil | Shot::Hood => {
                // Wraiths, close, lit cold from the guardian's light, the
                // nearest recoiling back from it.
                let k = (t / 0.8).min(1.0);
                let set: &[(Vec3, f32)] = if self.shot == Shot::Hood {
                    &[(Vec3::new(0.0, 2.1, -0.6), 1.0)]
                } else {
                    &[
                        (Vec3::new(-1.2, 2.3, -2.0 - 0.6 * k), 1.1),
                        (Vec3::new(1.0, 2.4, -2.4 - 0.7 * k), 1.1),
                        (Vec3::new(0.0, 2.6, -4.0 - 0.5 * k), 1.0),
                    ]
                };
                for (i, &(at, size)) in set.iter().enumerate() {
                    // Facing us, leaning back as they recoil.
                    wraith(&mut mesh.lit, at, 0.0, size, 2.3 + i as f32 * 0.4 - k);
                    wisps(&mut mesh.sprites, at, size, t, i as u32 + 500);
                    // The hood's opening: a void with nothing in it.
                    mesh.sprites.push(Sprite {
                        additive: 0.0,
                        alpha: 0.95,
                        priority: 9,
                        ..feathers::dot(
                            at + Vec3::new(0.0, -0.32 * size, 0.38 * size),
                            0.2 * size,
                            [0.0, 0.0, 0.0],
                            1.0,
                        )
                    });
                }
            }
            Shot::Stride => {
                if (0.6..1.5).contains(&t) {
                    let k = ((t - 0.6) / 0.25).clamp(0.0, 1.0);
                    let from = Vec3::new(-7.0, 1.6, -5.0);
                    let to = Vec3::new(7.0, 1.4, -5.5);
                    mesh.ribbons.extend(feathers::beam(
                        from,
                        from.lerp(to, k),
                        0.06,
                        [0.6, 6.0, 1.2],
                    ));
                }
            }
            Shot::Star => {
                // A blue-white star flaring with long cross spikes, sparks
                // drifting down from it.
                let at = Vec3::new(0.0, 22.0, -40.0);
                let swell = (t / 1.5).min(1.0);
                mesh.sprites.push(feathers::dot(
                    at,
                    0.6 + 0.8 * swell,
                    [30.0, 40.0, 60.0],
                    1.0,
                ));
                mesh.sprites
                    .push(feathers::dot(at, 6.0 * swell + 1.0, [0.4, 0.6, 1.2], 1.0));
                for (dir, len) in [(Vec3::X, 14.0_f32), (Vec3::Y, 6.0)] {
                    let l = len * (0.4 + 0.6 * swell);
                    mesh.ribbons.push(feathers::glow_line(
                        &[at - dir * l, at, at + dir * l],
                        0.25,
                        [1.5, 2.2, 4.0],
                    ));
                }
                for i in 0..40u32 {
                    let fall = (t * 0.4 + hash(i + 700)).fract();
                    let p = at
                        + Vec3::new(
                            (hash(i + 701) - 0.5) * 3.0,
                            -fall * 10.0,
                            (hash(i + 702) - 0.5) * 2.0,
                        );
                    mesh.sprites.push(feathers::dot(
                        p,
                        0.08,
                        [2.0 * (1.0 - fall), 3.0 * (1.0 - fall), 6.0 * (1.0 - fall)],
                        1.0,
                    ));
                }
            }
            Shot::Attack => {
                // The wraith lunges in from the right, low and reaching.
                let k = (t / 1.5).min(1.0);
                let at = Vec3::new(2.5 - 2.0 * k, 2.6 - 0.6 * k, -2.5);
                // Upright, reaching: its own sway held near level.
                wraith(&mut mesh.lit, at, 1.2, 1.2, -1.75 + 0.2 * (t * 3.0).sin());
                wisps(&mut mesh.sprites, at, 1.4, t, 9);
                // Fallen students: dark heaps on the cobbles.
                for (x, z) in [(-2.0_f32, -3.5_f32), (1.2, -4.5), (-0.2, -6.0)] {
                    wraith(
                        &mut mesh.lit,
                        Vec3::new(x, 0.35, z),
                        std::f32::consts::FRAC_PI_2,
                        0.45,
                        0.0,
                    );
                }
            }
            Shot::Spiders => {
                web(&mut mesh.ribbons, Vec3::new(0.0, 1.7, 1.2), 2.4, t);
                for (k, (at, yaw)) in [
                    (Vec3::new(-1.6, 0.0, -3.5), 0.4_f32),
                    (Vec3::new(1.4, 0.0, -4.2), -0.5),
                    (Vec3::new(0.2, 0.0, -6.5), 0.1),
                ]
                .into_iter()
                .enumerate()
                {
                    spider(&mut mesh.lit, at, yaw, 1.3, t + k as f32 * 0.9);
                }
                // A green bolt, then a red one, from the left.
                let (from, to, color) = if t < 1.0 {
                    (
                        Vec3::new(-6.0, 1.5, -2.0),
                        Vec3::new(-1.6, 1.0, -3.5),
                        [0.6, 6.0, 1.2],
                    )
                } else {
                    (
                        Vec3::new(6.0, 1.6, -2.5),
                        Vec3::new(1.4, 1.0, -4.2),
                        [6.0, 1.0, 0.6],
                    )
                };
                let k = ((t % 1.0) / 0.2).clamp(0.0, 1.0);
                mesh.ribbons
                    .extend(feathers::beam(from, from.lerp(to, k), 0.05, color));
            }
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
        if matches!(self.shot, Shot::Recoil | Shot::Hood) {
            // The guardian's cold light, from behind the camera.
            // Mostly from behind and above them: dark shapes with cold
            // rims, a little fill on their fronts.
            neon.key = Some(key(Vec3::new(0.2, 0.6, -0.8), 3.0, 0.01));
            neon.key_color = [0.75, 0.85, 1.0];
            neon.field = [0.01, 0.016, 0.03];
        }
        if self.shot == Shot::Spiders {
            // The bolt's light among the spiders.
            let (at, color) = if t < 1.0 {
                (Vec3::new(-2.0, 1.4, -3.0), [0.3, 1.0, 0.45])
            } else {
                (Vec3::new(2.0, 1.4, -3.6), [1.0, 0.25, 0.15])
            };
            neon.flash_lamps[0] = Lamp {
                position: at,
                color,
                intensity: 80.0,
                range: 8.0,
            };
        }
        if matches!(
            self.shot,
            Shot::Plate | Shot::Boots | Shot::Stride | Shot::Attack
        ) {
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
            VORTEX
                + Vec3::new(
                    a.cos() * r * 0.7,
                    2.0 - 4.0 * hash(i * 7 + 3),
                    a.sin() * r * 0.45,
                )
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
    let (around, down) = (20usize, 12usize);
    // Each wraith swoops and banks on its own rhythm (`t` carries its
    // phase), so a crowd of them never hangs in one pose.
    let lean = 0.3 + 0.45 * (t * 0.9).sin();
    let roll = 0.4 * (t * 0.7 + 1.3).sin();
    let turn =
        Mat3::from_rotation_y(yaw) * Mat3::from_rotation_z(roll) * Mat3::from_rotation_x(lean);
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
        let rag = 1.1 * v * v * hash((i as u32 % 20) * 13 + 1);
        let stir = 0.12 * v * (t * 2.5 + theta * 3.0).sin();
        let local = Vec3::new(
            r * theta.sin() + stir,
            -2.0 * v - rag,
            r * theta.cos() * 0.8,
        );
        at + turn * (local * scale)
    };
    let color = [0.003, 0.003, 0.004];
    for j in 0..down {
        for i in 0..around {
            // Tears: below the waist, some columns are gone, so the
            // cloak hangs in separate rags with sky between.
            if j as f32 >= down as f32 * 0.5
                && hash(i as u32 * 7 + 3) > 0.5 + 0.4 * (1.0 - j as f32 / down as f32)
            {
                continue;
            }
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

/// The smoke a wraith trails from its rags: dark wisps drifting below and
/// behind its hem, at `at` (its hood) and `scale`.
fn wisps(out: &mut Vec<Sprite>, at: Vec3, scale: f32, t: f32, seed: u32) {
    let (rect, layer) = (
        sheet::find("smoke").map(|s| s.rect(11)).unwrap_or_default(),
        sheet::layer("smoke").unwrap_or_default(),
    );
    for k in 0..5u32 {
        let drift = (t * 0.6 + k as f32 * 0.7 + seed as f32).fract();
        let below = at
            + Vec3::new(
                0.3 * (hash(seed * 11 + k) - 0.5) * scale,
                (-1.6 - 1.4 * drift) * scale,
                -0.2 * scale,
            );
        out.push(Sprite {
            rect_a: rect,
            rect_b: rect,
            layer,
            additive: 0.0,
            alpha: 0.6 * (1.0 - drift),
            angle: hash(seed * 13 + k) * 6.28 + t,
            priority: 3,
            ..feathers::dot(
                below,
                (0.35 + 0.5 * drift) * scale,
                [0.01, 0.01, 0.015],
                1.0,
            )
        });
    }
}

/// A great orb web centered at `center`, `radius` m across, facing the
/// camera along +Z: radial threads and a spiral, silvered by the moon,
/// trembling a little.
fn web(out: &mut Vec<verse::fx::Ribbon>, center: Vec3, radius: f32, t: f32) {
    let silver = [0.22, 0.26, 0.34];
    let spokes = 18;
    let at = |a: f32, r: f32| {
        let sway = 0.02 * (t * 2.0 + a * 3.0).sin() * r / radius;
        center + Vec3::new(a.cos() * r * 1.1, a.sin() * r, sway)
    };
    for k in 0..spokes {
        let a = k as f32 / spokes as f32 * std::f32::consts::TAU + 0.07 * hash(k + 500);
        let points: Vec<Vec3> = (0..=6).map(|j| at(a, radius * j as f32 / 6.0)).collect();
        out.push(feathers::glow_line(&points, 0.012, silver));
    }
    // The capture spiral, broken in places as an old web is.
    let turns = 11.0;
    let steps = 360;
    let mut run = Vec::new();
    for i in 0..=steps {
        let u = i as f32 / steps as f32;
        let a = u * turns * std::f32::consts::TAU;
        let r = radius * (0.12 + 0.88 * u);
        let broken = hash(i / 9 + 900) < 0.12;
        if broken {
            if run.len() > 1 {
                out.push(feathers::glow_line(&run, 0.008, silver.map(|c| c * 0.8)));
            }
            run.clear();
        } else {
            run.push(at(a, r));
        }
    }
    if run.len() > 1 {
        out.push(feathers::glow_line(&run, 0.008, silver.map(|c| c * 0.8)));
    }
}

/// A prism of `sides` faces from `a` to `b`, `radius` thick at `a` and
/// `tip` at `b`: a spider's leg segment.
fn limb(out: &mut Vec<LitVertex>, a: Vec3, b: Vec3, radius: f32, tip: f32, color: [f32; 3]) {
    let axis = (b - a).normalize_or(Vec3::Y);
    let side = axis.any_orthonormal_vector();
    let up = axis.cross(side);
    let sides = 6;
    for k in 0..sides {
        let a0 = k as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let d0 = side * a0.cos() + up * a0.sin();
        let d1 = side * a1.cos() + up * a1.sin();
        let quad = [a + d0 * radius, a + d1 * radius, b + d1 * tip, b + d0 * tip];
        let normal = (d0 + d1).normalize_or(side);
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(LitVertex {
                pos: quad[i].to_array(),
                normal: normal.to_array(),
                tangent: axis.to_array(),
                local: (quad[i] - a).to_array(),
                color,
                params: [0.0, 0.5, Material::Dark.code(), 1.0],
            });
        }
    }
}

/// An ellipsoid of radii `r` at `at`: a spider's body part.
fn blob(out: &mut Vec<LitVertex>, at: Vec3, r: Vec3, color: [f32; 3]) {
    let (rings, segs) = (10, 14);
    let p = |i: usize, j: usize| {
        let th = i as f32 / rings as f32 * std::f32::consts::PI;
        let ph = j as f32 / segs as f32 * std::f32::consts::TAU;
        let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
        (at + n * r, n)
    };
    for i in 0..rings {
        for j in 0..segs {
            let q = [p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1)];
            for k in [0, 1, 2, 0, 2, 3] {
                out.push(LitVertex {
                    pos: q[k].0.to_array(),
                    normal: q[k].1.to_array(),
                    tangent: Vec3::Y.to_array(),
                    local: (q[k].0 - at).to_array(),
                    color,
                    params: [0.0, 0.45, Material::Dark.code(), 1.0],
                });
            }
        }
    }
}

/// A giant spider standing at `at`, facing `yaw`, `size` m long, its legs
/// stepping with `t`: a hairy dark abdomen, a head, and eight jointed legs.
fn spider(out: &mut Vec<LitVertex>, at: Vec3, yaw: f32, size: f32, t: f32) {
    let turn = Mat3::from_rotation_y(yaw);
    let body = at + Vec3::Y * size * 0.75;
    let color = [0.02, 0.016, 0.012];
    blob(
        out,
        body + turn * Vec3::new(0.0, 0.1, -0.6) * size,
        Vec3::new(0.45, 0.4, 0.6) * size,
        color,
    );
    blob(
        out,
        body + turn * Vec3::new(0.0, 0.0, 0.25) * size,
        Vec3::new(0.28, 0.24, 0.3) * size,
        color,
    );
    for side in [-1.0_f32, 1.0] {
        for k in 0..4 {
            let kk = k as f32;
            // Legs fan from front to back, each lifting in its turn.
            let fan = 0.9 - kk * 0.55;
            let lift = 0.15 * ((t * 6.0 + kk * 1.6 + side).sin()).max(0.0);
            let hip = body + turn * Vec3::new(side * 0.2, 0.0, 0.2 - kk * 0.12) * size;
            let out_dir = turn * Vec3::new(side * fan.cos(), 0.0, fan.sin()).normalize();
            let knee = hip + (out_dir * 0.7 + Vec3::Y * (0.55 + lift)) * size;
            let foot = hip + out_dir * 1.45 * size - Vec3::Y * (0.75 * size - lift * size * 0.3);
            limb(out, hip, knee, 0.06 * size, 0.045 * size, color);
            limb(out, knee, foot, 0.045 * size, 0.012 * size, color);
        }
    }
}
