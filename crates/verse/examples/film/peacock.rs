//! The Draconia peacock guardian (`docs/verse/draconia/shots.md`, P1–P3):
//! feathers of light drifting in a starry void, the flash, and Draconia
//! standing before the guardian's fanned tail in a ruined courtyard.
//!
//! Two parts, one per shot file:
//!
//! - `feathers` (P1 and P2): a dozen feathers of light from one fan whose
//!   center lies below the frame, turning slowly in a dark blue void
//!   dusted with stars; at `flash_at` the hero eyespot blows out to white.
//! - `tail` (P3): Alice, recolored as Draconia (platinum hair, a black
//!   coat), centered before a fan of about 100 feathers, on cobbles, with
//!   ruined walls and fires at the edges, mist on the ground, and embers.
//!
//! Everything is drawn through the physical renderer's neon stage: lit
//! geometry for the stones, the pack's Alice as the frame's figure, and
//! sprites for the feathers and effects (`verse::fx::feathers`).

use std::sync::Arc;

use glam::{Mat4, Vec3};
use serde::Deserialize;
use verse::fx::{Particles, Spawn, Sprite, feathers};
use verse::mesh::Mesh;
use verse::pbr::textured::{Figure, TexturedScene};
use verse::pbr::{Key, Lamp, LitVertex, MAX_LAMPS, Material, Neon};
use verse::zones::everglade::player::Beast;
use verse::zones::everglade_pack::ZonePack;

/// Which shot of the scene.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Feathers,
    Tail,
}

/// Where the hero eyespot of the feathers shot sits; the shot's camera
/// pushes in to it.
pub const HERO: Vec3 = Vec3::new(0.4, 1.2, -6.0);

/// The center the guardian's tail fans from, behind Draconia.
const TAIL_CENTER: Vec3 = Vec3::new(0.0, 0.9, -1.8);
const TAIL_RADIUS: f32 = 8.0;

/// The void's color and the courtyard's night.
const VOID: [f32; 3] = [0.012, 0.03, 0.07];
const NIGHT: [f32; 3] = [0.03, 0.075, 0.12];

pub struct Set {
    part: Part,
    time: f32,
    flash_at: Option<f32>,
    flash_started: bool,
    rise: f32,
    fx: Particles,
    alice: Option<(Beast, Arc<TexturedScene>, Vec<usize>)>,
    /// Her vertices this frame.
    posed: Vec<verse::pbr::textured::TexturedVertex>,
    /// Her crouch's vertices, which she rises from.
    crouch: Vec<verse::pbr::textured::TexturedVertex>,
    stars: Vec<(Vec3, f32, f32)>,
    lamps: Vec<Lamp>,
}

/// A small deterministic hash in 0..1.
fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32
}

impl Set {
    /// The scene's static stones: none for the void, the courtyard's
    /// cobbles and ruins for the tail.
    pub fn world(part: Part) -> Mesh {
        let mut lit = Vec::new();
        if part == Part::Tail {
            cobbles(&mut lit);
            ruins(&mut lit);
        }
        Mesh {
            lit,
            ..Mesh::default()
        }
    }

    pub fn new(
        part: Part,
        flash_at: Option<f32>,
        rise: f32,
        pack: Option<&ZonePack>,
    ) -> Result<Self, String> {
        let mut fx = Particles::new(0xD2AC);
        let mut lamps = Vec::new();
        let mut alice = None;
        let mut crouch = Vec::new();
        let mut stars = Vec::new();
        match part {
            Part::Feathers => {
                for i in 0..160 {
                    let u = hash(i * 3) * 2.0 - 1.0;
                    let v = hash(i * 3 + 1) * 2.0 - 1.0;
                    let at = Vec3::new(u * 70.0, v * 34.0 + 2.0, -70.0 - hash(i * 3 + 2) * 20.0);
                    let size = 0.05 + 0.18 * hash(i * 7 + 5).powi(6);
                    stars.push((at, size, 0.4 + 2.5 * hash(i * 11 + 1).powi(3)));
                }
            }
            Part::Tail => {
                let pack = pack.ok_or("the tail needs the Everglade pack for Alice")?;
                let form = pack
                    .form("npc/alice")
                    .ok_or("the pack carries no npc/alice")?;
                let beast = Beast::new(pack, form)?;
                let mut scene = beast.figure().scene.as_ref().clone();
                draconia(&mut scene);
                // Alice's staff, slung across her back and up past her
                // head, is no part of Draconia: its vertices (found in the
                // bind pose, behind the shoulder blades and out to her
                // left) collapse to a point every frame.
                let staff: Vec<usize> = beast
                    .figure()
                    .vertices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.pos[2] < -0.12 && v.pos[0] > 0.1 && v.pos[1] > 1.1)
                    .map(|(i, _)| i)
                    .collect();
                // A second rig holds the crouch, so neither rig blends
                // between clips it switches each frame.
                let mut low = Beast::new(pack, form)?;
                low.pose_clip(
                    Vec3::new(0.0, -0.1, 0.0),
                    0.0,
                    1.0,
                    "swing",
                    0.4,
                    false,
                    0.0,
                );
                crouch = low.vertices().to_vec();
                alice = Some((beast, Arc::new(scene), staff));
                fx.start("ground_mist", Spawn::at(Vec3::new(0.0, 0.0, 0.5)));
                fx.start(
                    "ground_mist",
                    Spawn::at(Vec3::new(0.0, 0.0, -3.0)).scaled(1.3),
                );
                fx.start("night_embers", Spawn::at(Vec3::new(0.0, 0.3, 0.0)));
                // One ragged cluster at the far left, as in the reference.
                for at in [Vec3::new(-3.15, 0.35, 2.3)] {
                    fx.start("ruin_fire", Spawn::at(at));
                    if at.z > 2.0 {
                        continue;
                    }
                    lamps.push(Lamp {
                        position: at + Vec3::new(0.0, 0.5, -0.6),
                        color: [1.0, 0.42, 0.12],
                        intensity: 3.0,
                        range: 6.0,
                    });
                }
                // The tail's light on the cobbles comes from low lamps out
                // at its sides, none near her, so she stays a silhouette.
                for x in [-5.5_f32, -3.2, 3.2, 5.5] {
                    lamps.push(Lamp {
                        position: Vec3::new(x, 0.6, -2.4),
                        color: [0.7, 0.82, 1.0],
                        intensity: 10.0,
                        range: 7.0,
                    });
                }
                for i in 0..90 {
                    let u = hash(i * 5) * 2.0 - 1.0;
                    let v = hash(i * 5 + 1);
                    let at = Vec3::new(u * 60.0, 8.0 + v * 30.0, -40.0);
                    stars.push((at, 0.04 + 0.06 * hash(i * 9), 0.3 + hash(i * 13) * 0.8));
                }
            }
        }
        let mut set = Self {
            part,
            time: 0.0,
            flash_at,
            flash_started: false,
            rise,
            fx,
            alice,
            posed: Vec::new(),
            crouch,
            stars,
            lamps,
        };
        // Let the mist, fires, and embers fill in before the first frame,
        // and pose her, so no frame shows her bind pose.
        if part == Part::Tail {
            set.advance(1.0 / 30.0, 0.0);
            for _ in 0..240 {
                set.fx.tick(1.0 / 30.0, |_, _| 0.0);
            }
        }
        Ok(set)
    }

    /// Advances the scene to film time `t`, `dt` seconds on.
    pub fn advance(&mut self, dt: f32, t: f32) {
        self.time = t;
        if let Some(at) = self.flash_at
            && !self.flash_started
            && t >= at
        {
            self.flash_started = true;
            self.fx
                .start("guardian_flash", Spawn::at(HERO + Vec3::Z * 0.2));
        }
        let steps = (dt * 120.0).ceil().max(1.0) as usize;
        for _ in 0..steps {
            self.fx.tick(dt / steps as f32, |_, _| 0.0);
        }
        if let Some((beast, _, _)) = &mut self.alice {
            // She lands in a crouch (the swing's wide, bent-knee stance,
            // sunk a little) and rises into her idle over `RISE` seconds
            // before `rise`: the two poses' vertices blend, since the pack
            // has no crouch-to-stand clip.
            const RISE: f32 = 0.25;
            beast.pose_clip(
                Vec3::ZERO,
                0.0,
                1.0,
                "idle",
                (t - self.rise).max(0.0),
                true,
                dt,
            );
            let k = ((t - (self.rise - RISE)) / RISE).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            // She stands with her feet apart: below the hips her idle
            // leans toward the crouch's wide stance (lifted back to the
            // ground).
            let mut stand = beast.vertices().to_vec();
            for (v, c) in stand.iter_mut().zip(&self.crouch) {
                let w = 0.5 * ((0.95 - v.pos[1]) / 0.3).clamp(0.0, 1.0);
                let wide = Vec3::from(c.pos) + Vec3::Y * 0.1;
                v.pos = Vec3::from(v.pos).lerp(wide, w).to_array();
            }
            self.posed = if k >= 1.0 {
                stand
            } else {
                let mut low = self.crouch.clone();
                for (v, s) in low.iter_mut().zip(&stand) {
                    v.pos = Vec3::from(v.pos).lerp(Vec3::from(s.pos), k).to_array();
                    v.normal = Vec3::from(v.normal)
                        .lerp(Vec3::from(s.normal), k)
                        .normalize_or(Vec3::Y)
                        .to_array();
                }
                low
            };
        }
    }

    /// Stops of exposure the flash adds at film time `t`.
    fn flash(&self, t: f32) -> f32 {
        match (self.part, self.flash_at) {
            (Part::Feathers, Some(at)) => {
                let x = t - at;
                if x < 0.0 {
                    0.0
                } else {
                    (x / 0.12).min(1.0) * 4.5
                }
            }
            // The tail opens out of the white.
            (Part::Tail, _) => 2.5 * (1.0 - t / 0.35).clamp(0.0, 1.0).powi(2),
            _ => 0.0,
        }
    }

    /// The frame: the stage, Draconia, and every sprite.
    pub fn frame(&self) -> Mesh {
        let t = self.time;
        let mut mesh = Mesh {
            neon: Some(self.stage(t)),
            ..Mesh::default()
        };
        let sprites = &mut mesh.sprites;
        for (i, (at, size, lum)) in self.stars.iter().enumerate() {
            let twinkle = 0.75 + 0.25 * (t * 3.0 + i as f32).sin();
            sprites.push(feathers::dot(
                *at,
                *size,
                [lum * twinkle * 0.8, lum * twinkle * 0.9, lum * twinkle],
                1.0,
            ));
        }
        match self.part {
            Part::Feathers => self.drifting(sprites, &mut mesh.ribbons),
            Part::Tail => {
                let open = 0.92 + 0.08 * (t / 3.0).min(1.0);
                sprites.extend(
                    feathers::Fan {
                        center: TAIL_CENTER,
                        right: Vec3::X,
                        // Leaning back, so the fan reads wider than tall.
                        up: Vec3::new(0.0, 0.78, -0.62),
                        radius: TAIL_RADIUS,
                        rows: 5,
                        feathers: 100,
                        droop: 0.3,
                        luminance: [4.4, 4.9, 5.8],
                        vane: 0.32,
                        slim: 0.62,
                        open,
                        time: t,
                    }
                    .sprites(),
                );
                // The core of light behind her.
                // A blinding center behind her torso and hips, clipped
                // white, so her silhouette pops against it.
                sprites.push(feathers::dot(
                    Vec3::new(0.0, 1.15, -0.7),
                    1.5,
                    [14.0, 14.5, 15.0],
                    1.0,
                ));
                // A band from her hips to her shoulders, so the guardian's
                // neck above stands against the fan, not the glare.
                for x in [-1.8_f32, -1.2, -0.6, 0.0, 0.6, 1.2, 1.8] {
                    sprites.push(feathers::dot(
                        TAIL_CENTER + Vec3::new(x, 0.35, 0.4),
                        1.8 - 0.25 * x.abs(),
                        [1.6, 1.8, 2.1],
                        1.0,
                    ));
                }
                sprites.push(feathers::dot(
                    TAIL_CENTER + Vec3::new(0.0, 1.5, 0.2),
                    6.0,
                    [0.12, 0.16, 0.24],
                    1.0,
                ));
                // The guardian's body, neck, head, and crest rising from the
                // tail's center just above her head.
                let lum = [2.2, 2.5, 3.0];
                for (i, (x, y, r)) in [
                    (0.0, 2.1, 0.55),
                    (-0.22, 2.35, 0.42),
                    (0.22, 2.35, 0.42),
                    (0.0, 2.55, 0.38),
                    (0.0, 1.95, 0.45),
                ]
                .into_iter()
                .enumerate()
                {
                    sprites.push(feathers::dot(
                        Vec3::new(x, y, -1.0 - 0.01 * i as f32),
                        r * 1.6,
                        lum,
                        1.0,
                    ));
                }
                // An S-curved neck, thick at the breast and thin at the
                // head, the head turned to her right with a fanned crest.
                let bright = lum.map(|c| c * 3.5);
                let neck: Vec<Vec3> = (0..=14)
                    .map(|i| {
                        let u = i as f32 / 14.0;
                        Vec3::new(
                            -0.28 * (u * std::f32::consts::PI).sin() * u + 0.1 * u,
                            2.5 + 1.1 * u,
                            -0.6,
                        )
                    })
                    .collect();
                // A stroke's bright core is about a fifth of its half width.
                // The neck as a soft fill between two bright edges, so it
                // reads as a shape.
                let side = |offset: f32| -> Vec<Vec3> {
                    neck.iter()
                        .enumerate()
                        .map(|(i, p)| *p + Vec3::X * offset * (1.0 - 0.55 * i as f32 / 14.0))
                        .collect()
                };
                mesh.ribbons
                    .push(feathers::glow_line(&neck, 0.9, bright.map(|c| c * 0.35)));
                for offset in [-0.16_f32, 0.16] {
                    mesh.ribbons
                        .push(feathers::glow_line(&side(offset), 0.12, bright));
                }
                let head = *neck.last().unwrap_or(&Vec3::ZERO) + Vec3::new(-0.1, 0.05, 0.0);
                sprites.push(feathers::dot(head, 0.34, bright, 1.0));
                mesh.ribbons.push(feathers::glow_line(
                    &[head, head + Vec3::new(-0.24, -0.04, 0.0)],
                    0.035,
                    bright,
                ));
                for k in 0..5 {
                    let a = 1.2 + 0.18 * k as f32;
                    let tip = head + Vec3::new(a.cos() * 0.4, a.sin() * 0.4, 0.0);
                    mesh.ribbons.push(feathers::glow_line(
                        &[head, tip],
                        0.012,
                        bright.map(|c| c * 0.5),
                    ));
                    sprites.push(feathers::dot(tip, 0.07, bright, 1.0));
                }
                // Cloudy light filling the fan between its feathers.
                for i in 0..24u32 {
                    let a = std::f32::consts::PI * hash(i * 7 + 1);
                    let rr = TAIL_RADIUS * (0.3 + 0.65 * hash(i * 7 + 2));
                    let at = TAIL_CENTER
                        + Vec3::new(
                            a.cos() * rr,
                            a.sin() * rr * 0.78,
                            -a.sin() * rr * 0.62 - 0.3,
                        );
                    let (rect, layer) = (
                        verse::fx::sheet::find("smoke")
                            .map(|s| s.rect(4 + i % 10))
                            .unwrap_or_default(),
                        verse::fx::sheet::layer("smoke").unwrap_or_default(),
                    );
                    sprites.push(verse::fx::Sprite {
                        rect_a: rect,
                        rect_b: rect,
                        layer,
                        angle: hash(i * 7 + 3) * 6.28,
                        priority: 3,
                        ..feathers::dot(at, 1.6 + 1.4 * hash(i * 7 + 4), [0.07, 0.1, 0.2], 1.0)
                    });
                }
                // The core's pool of light on the cobbles behind her.
                sprites.push(verse::fx::Sprite {
                    facing: verse::fx::Facing::Ground,
                    ..feathers::dot(Vec3::new(0.0, 0.07, -0.2), 5.5, [0.9, 1.0, 1.25], 1.0)
                });
                // A luminous haze filling the fan behind the feathers.
                for i in 0..9 {
                    let theta = std::f32::consts::PI * (i as f32 + 0.5) / 9.0;
                    let dir = Vec3::new(theta.cos(), theta.sin() * 0.78, -theta.sin() * 0.62);
                    sprites.push(feathers::dot(
                        TAIL_CENTER + dir * TAIL_RADIUS * 0.62 - Vec3::Z * 0.4,
                        TAIL_RADIUS * 0.55,
                        [0.12, 0.2, 0.4],
                        1.0,
                    ));
                }
                if let Some((_, scene, staff)) = &self.alice {
                    let mut vertices = self.posed.clone();
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
            }
        }
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    /// The feathers of the void: one fan's worth seen close, turning.
    fn drifting(&self, sprites: &mut Vec<Sprite>, ribbons_out: &mut Vec<verse::fx::Ribbon>) {
        let t = self.time;
        let center = Vec3::new(-5.0, -9.0, -9.0);
        // Angle from the fan's center, depth, length, and the hero.
        let set: [(f32, f32, f32); 5] = [
            (0.0, 0.0, 12.6),
            (0.5, -2.0, 14.0),
            (-0.45, -1.0, 13.5),
            (0.95, 1.5, 12.0),
            (-1.0, -4.0, 14.5),
        ];
        for (i, (angle, depth, length)) in set.into_iter().enumerate() {
            let turn = 0.035 * (t * 0.7 + i as f32 * 1.3).sin();
            // The fan leans in from the lower left, as the reference's does.
            let a = std::f32::consts::FRAC_PI_2 - angle - turn - 0.55;
            let dir = Vec3::new(a.cos(), a.sin(), 0.0);
            let base = center + Vec3::Z * depth;
            let mut eye = base + dir * length;
            if i == 0 {
                eye = HERO;
            }
            // Each feather lies flat to the camera, its quill at its
            // eyespot's depth, so its eyespot draws round.
            let base = Vec3::new(base.x, base.y, eye.z);
            // Each shaft bows the same way, so the feathers sweep in arcs.
            let side = Vec3::new(-dir.y, dir.x, 0.0);
            let bend = (eye + base) * 0.5 + side * length * 0.3;
            let lum = [1.8, 2.05, 2.5];
            // Each feather is strokes of light, sharp however close the
            // push-in comes.
            let (glows, strokes) =
                feathers::fine(eye, bend, base, length * 0.16, lum, i as u32 + 1);
            sprites.extend(glows);
            ribbons_out.extend(strokes);
            sprites.push(feathers::dot(
                eye,
                length * 0.05,
                lum.map(|c| c * 0.15),
                1.0,
            ));
        }
        // The rainbow fringe low across the frame.
        ribbons_out.extend(feathers::rainbow(
            Vec3::new(4.0, -6.0, -4.0),
            Vec3::X,
            Vec3::Y,
            9.0,
            0.8,
            2.0,
            2.9,
            0.025,
        ));
        // A blue nebula: soft clouds of light far behind.
        for i in 0..30u32 {
            let at = Vec3::new(
                (hash(i * 5 + 300) * 2.0 - 1.0) * 45.0,
                (hash(i * 5 + 301) * 2.0 - 1.0) * 22.0 + 2.0,
                -55.0,
            );
            let (rect, layer) = (
                verse::fx::sheet::find("smoke")
                    .map(|s| s.rect(5 + i % 9))
                    .unwrap_or_default(),
                verse::fx::sheet::layer("smoke").unwrap_or_default(),
            );
            sprites.push(Sprite {
                rect_a: rect,
                rect_b: rect,
                layer,
                angle: hash(i * 5 + 302) * 6.28,
                priority: 2,
                ..feathers::dot(
                    at,
                    12.0 + 10.0 * hash(i * 5 + 303),
                    [0.02, 0.038, 0.085],
                    1.0,
                )
            });
        }
        // A faint blue haze where the feathers gather.
        sprites.push(feathers::dot(
            Vec3::new(0.0, -6.0, -20.0),
            14.0,
            [0.06, 0.09, 0.16],
            1.0,
        ));
    }

    fn stage(&self, t: f32) -> Neon {
        let mut neon = Neon::neutral(t);
        neon.fog_start = 300.0;
        neon.fog_end = 900.0;
        neon.vignette = 0.3;
        neon.bloom = match self.part {
            Part::Feathers => 0.3,
            Part::Tail => 0.42,
        };
        neon.grade.exposure = self.flash(t);
        neon.grade.saturation = 0.92;
        neon.grade.contrast = 1.35;
        match self.part {
            Part::Feathers => {
                neon.field = VOID;
                neon.key = Some(key(Vec3::new(0.0, 0.3, 1.0), 0.0, 0.0));
            }
            Part::Tail => {
                neon.field = NIGHT;
                // No key: the tail backlights her and the lamps light the ground.
                neon.key = Some(key(Vec3::new(0.0, 0.42, -1.0), 0.0, 0.0));
                neon.key_color = [0.72, 0.84, 1.0];
                neon.rim_color = [1.0, 0.6, 0.3];
                for (i, lamp) in self.lamps.iter().take(MAX_LAMPS).enumerate() {
                    neon.lamps[i] = if lamp.color[0] > 0.9 {
                        lamp.flickering(t, i as u32)
                    } else {
                        *lamp
                    };
                }
            }
        }
        neon
    }
}

fn key(toward: Vec3, illuminance: f32, sky: f32) -> Key {
    Key {
        dir: toward.normalize(),
        illuminance,
        angular_radius: 0.2,
        rim_dir: Vec3::new(0.6, 0.35, -0.7).normalize(),
        rim_illuminance: illuminance * 0.6,
        rim_angular_radius: 0.2,
        sky,
        ground: sky * 0.2,
        ev100: 0.0,
        shadow_center: Vec3::new(0.0, 0.0, -1.0),
        shadow_half: 14.0,
        shadow_distance: None,
        cache_far_shadows: false,
    }
}

/// Recolors Alice as Draconia: pale platinum hair (the atlas's bottom
/// band holds the hair strips) and the forest-green coat to black.
fn draconia(scene: &mut TexturedScene) {
    let to_lin = |v: u8| (f32::from(v) / 255.0).powf(2.2);
    let to_srgb = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8;
    for image in &mut scene.images {
        let (w, h) = (image.width as usize, image.height as usize);
        let band = (h as f32 * 0.84) as usize;
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let [r, g, b] = [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2]].map(to_lin);
                let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                let out = if y >= band {
                    // Platinum: pale, a touch warm, keeping the strands'
                    // shading.
                    let k = (luma / 0.12).sqrt().min(1.6);
                    [0.62 * k, 0.58 * k, 0.5 * k]
                } else if g > r * 1.08 && g > b * 1.02 {
                    // The coat's forest green to a black wool.
                    let k = (luma / 0.05).min(1.5);
                    [0.018 * k, 0.018 * k, 0.022 * k]
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

fn push_quad(out: &mut Vec<LitVertex>, c: [Vec3; 4], color: [f32; 3], rough: f32) {
    let normal = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or(Vec3::Y);
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(LitVertex {
            pos: c[i].to_array(),
            normal: normal.to_array(),
            tangent: Vec3::X.to_array(),
            local: c[i].to_array(),
            color,
            params: [0.0, rough, Material::Dark.code(), 1.0],
        });
    }
}

/// A box from `min` to `max`, turned `yaw` about its center, every face
/// facing out.
fn push_box(out: &mut Vec<LitVertex>, min: Vec3, max: Vec3, yaw: f32, color: [f32; 3], rough: f32) {
    let center = (min + max) * 0.5;
    let m = Mat4::from_translation(center) * Mat4::from_rotation_y(yaw);
    let h = (max - min) * 0.5;
    let p = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x * h.x, y * h.y, z * h.z));
    let faces = [
        [
            p(-1., 1., -1.),
            p(-1., 1., 1.),
            p(1., 1., 1.),
            p(1., 1., -1.),
        ],
        [
            p(-1., -1., 1.),
            p(1., -1., 1.),
            p(1., 1., 1.),
            p(-1., 1., 1.),
        ],
        [
            p(1., -1., -1.),
            p(-1., -1., -1.),
            p(-1., 1., -1.),
            p(1., 1., -1.),
        ],
        [
            p(1., -1., 1.),
            p(1., -1., -1.),
            p(1., 1., -1.),
            p(1., 1., 1.),
        ],
        [
            p(-1., -1., -1.),
            p(-1., -1., 1.),
            p(-1., 1., 1.),
            p(-1., 1., -1.),
        ],
    ];
    for f in faces {
        push_quad(out, f, color, rough);
    }
}

/// The courtyard's cobbles: rows of rounded-off stones with mortar between,
/// each its own shade, wet enough to catch the tail's light.
fn cobbles(out: &mut Vec<LitVertex>) {
    push_quad(
        out,
        [
            Vec3::new(-40.0, -0.01, 30.0),
            Vec3::new(40.0, -0.01, 30.0),
            Vec3::new(40.0, -0.01, -40.0),
            Vec3::new(-40.0, -0.01, -40.0),
        ],
        [0.012, 0.012, 0.013],
        0.9,
    );
    let size = 0.34;
    let mut n = 0u32;
    let mut z = -14.0;
    while z < 9.0 {
        let row = ((z + 14.0) / size) as i32;
        let mut x = -16.0 + if row % 2 == 0 { 0.0 } else { size * 0.5 };
        while x < 16.0 {
            n += 1;
            let w = size * (0.78 + 0.18 * hash(n));
            let d = size * (0.78 + 0.18 * hash(n + 7_000));
            let top = 0.02 + 0.02 * hash(n + 9_000);
            let shade = 0.035 + 0.05 * hash(n + 13_000);
            let color = [shade, shade * 1.0, shade * 1.06];
            push_box(
                out,
                Vec3::new(x - w * 0.5, -0.01, z - d * 0.5),
                Vec3::new(x + w * 0.5, top, z + d * 0.5),
                (hash(n + 21_000) - 0.5) * 0.25,
                color,
                0.7 + 0.25 * hash(n + 17_000),
            );
            x += size;
        }
        z += size;
    }
}

/// Broken walls at the frame's edges and rubble at their feet.
fn ruins(out: &mut Vec<LitVertex>) {
    // A ruined wall as a jagged silhouette: vertical strips whose tops
    // break irregularly, with deep notches, and pointed window holes.
    let wall = |out: &mut Vec<LitVertex>,
                x0: f32,
                x1: f32,
                z: f32,
                seed: u32,
                height: &dyn Fn(f32) -> f32,
                windows: &[(f32, f32, f32)]| {
        let strip = 0.22;
        let mut i = 0u32;
        let mut x = x0;
        while x < x1 {
            i += 1;
            let w = strip * (0.7 + 0.6 * hash(seed + i));
            let notch = if hash(seed + i * 7) > 0.9 {
                -1.4 * hash(seed + i * 11)
            } else {
                0.0
            };
            let top = (height(x + w * 0.5) + 0.5 * (hash(seed + i * 3) - 0.5) + notch).max(0.3);
            // Where a window cuts this strip: from its sill up to its
            // pointed head.
            let mut spans = vec![(0.0_f32, top)];
            for &(cx, sill, half) in windows {
                let d = ((x + w * 0.5) - cx).abs() / half;
                if d < 1.0 {
                    let head = sill + half * 2.2 + half * 1.2 * (1.0 - d).sqrt();
                    spans = vec![(0.0, sill.min(top)), (head.min(top), top)];
                }
            }
            for (y0, y1) in spans {
                if y1 - y0 > 0.02 {
                    let shade = 0.018 + 0.015 * hash(seed + i * 5);
                    push_box(
                        out,
                        Vec3::new(x, y0, z - 0.5),
                        Vec3::new(x + w, y1, z + 0.5),
                        0.0,
                        [shade, shade, shade * 1.08],
                        0.9,
                    );
                }
            }
            x += w;
        }
    };
    // Left: a tall broken Gothic wall stepping down toward the middle, two
    // pointed windows, and a broken pillar at its inner end.
    wall(
        out,
        -16.0,
        -5.6,
        -3.0,
        5_000,
        &|x: f32| {
            let k = ((x + 5.6) / -4.0).clamp(0.0, 1.0);
            1.0 + 9.0 * k.powf(0.8) + 0.9 * ((x * 2.3).sin() * (x * 0.9).cos())
        },
        &[(-8.4, 3.0, 0.55), (-11.2, 3.4, 0.6)],
    );
    // Right: lower, more broken, one window.
    wall(
        out,
        5.8,
        16.0,
        -3.4,
        9_000,
        &|x: f32| {
            let k = ((x - 5.8) / 4.0).clamp(0.0, 1.0);
            0.8 + 7.5 * k.powf(0.9) + 0.8 * ((x * 1.7).sin() * (x * 1.1).cos())
        },
        &[(9.0, 2.6, 0.5)],
    );
    // Broken pillars with slanted tops.
    for (k, (x, z, h)) in [
        (-7.4_f32, -3.2_f32, 4.6_f32),
        (7.6, -3.4, 3.4),
        (-9.6, -3.0, 6.0),
    ]
    .into_iter()
    .enumerate()
    {
        let shade = [0.02, 0.02, 0.022];
        push_box(
            out,
            Vec3::new(x - 0.3, 0.0, z - 0.3),
            Vec3::new(x + 0.3, h, z + 0.3),
            0.0,
            shade,
            0.9,
        );
        // The break: a wedge of smaller blocks stepping down.
        for j in 0..4 {
            let jj = j as f32;
            push_box(
                out,
                Vec3::new(x - 0.3 + 0.15 * jj, h, z - 0.3),
                Vec3::new(
                    x - 0.15 + 0.15 * jj,
                    h + 0.5 - 0.12 * jj - 0.1 * k as f32,
                    z + 0.3,
                ),
                0.0,
                shade,
                0.9,
            );
        }
    }
    // Rubble.
    for i in 0..70u32 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let x = side * (2.2 + 3.0 * hash(i * 3 + 100));
        let z = 0.5 + 3.0 * hash(i * 3 + 101);
        let s = 0.2 + 0.55 * hash(i * 3 + 102).powi(2);
        let shade = 0.025 + 0.03 * hash(i + 300);
        push_box(
            out,
            Vec3::new(x - s, -0.01, z - s * 0.8),
            Vec3::new(x + s, s * 0.9, z + s * 0.8),
            hash(i + 400) * 3.0,
            [shade, shade, shade * 1.05],
            0.8,
        );
    }
}
