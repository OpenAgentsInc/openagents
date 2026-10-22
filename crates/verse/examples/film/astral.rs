//! The Verse trailer's vision sets (#12458, the second cut, after Kiki's
//! brief): the astral layer, where the Grid and a rendered world share one
//! set of coordinates. The `grid` layer draws a world as the Grid draws
//! things, white edges over black faces on an endless floor; the `world`
//! layer draws the same pieces lit and colored under a sky. A `reveal`
//! spreads the world layer over the Grid layer from a point, so the same
//! camera watches the world change. People and agents walking the set are
//! ghosts, see-through wireframes, wherever the Grid layer is, and solid
//! characters wherever the world is.
//!
//! The worlds are concept stand-ins built from simple shapes in the
//! Haunted House's vintage-Halloween palette
//! (`docs/verse/games/the-haunted-house.md`) and Blue Rush's main street
//! from the greco-futurism kit; none of them is a built zone, so the cut
//! labels every shot from here VISION.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use serde::Deserialize;
use verse::fx::{Particles, Spawn, feathers};
use verse::grid_frame::Walker;
use verse::mesh::{Mesh, Vertex};
use verse::pbr::textured::TexturedScene;
use verse::pbr::{Daylight, Lamp, LitVertex, MAX_LAMPS, Material, Neon};

use crate::peacock::{hash, key};

/// Which world the set holds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum World {
    /// The Grid alone, with a portal that opens.
    Void,
    /// The Haunted House stand-in: a crooked house, jack-o'-lanterns,
    /// gnarled trees, tombstones, and a werewolf.
    Haunt,
    /// The Camping Adventure: tents round a fire under a mountain range.
    Camp,
    /// Blue Rush's main street in autumn: a theater, a shop with its
    /// keeper, a museum, a legal hall, and a power plant.
    Street,
    /// Home plots on the Grid, the creator's in the middle holding the
    /// Haunted House.
    Plots,
    /// The galaxy of worlds: the OpenAgents planet and its neighbors.
    Galaxy,
}

/// Which of the two layers a set draws where no reveal has reached.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    #[default]
    Grid,
    World,
}

/// The world layer spreading over the Grid layer: a circle on the ground
/// from `center` `[x, z]`, starting at `at` seconds and growing to `reach`
/// meters at `speed` meters a second, eased at both ends.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reveal {
    pub center: [f32; 2],
    pub at: f32,
    pub speed: f32,
    pub reach: f32,
}

impl Reveal {
    /// The circle's radius at `t`, m.
    #[must_use]
    pub fn radius(&self, t: f32) -> f32 {
        if self.speed <= 0.0 || self.reach <= 0.0 {
            return 0.0;
        }
        let k = ((t - self.at) * self.speed / self.reach).clamp(0.0, 1.0);
        self.reach * smooth(k)
    }

    /// How far the reveal has gone, 0 to 1.
    #[must_use]
    pub fn progress(&self, t: f32) -> f32 {
        if self.reach <= 0.0 {
            return 0.0;
        }
        self.radius(t) / self.reach
    }

    fn covers(&self, at: Vec3, t: f32) -> bool {
        let d = glam::Vec2::new(at.x - self.center[0], at.z - self.center[1]).length();
        d < self.radius(t)
    }
}

/// The pieces rising out of the floor, group after group: group `g` grows
/// over [`RISE`] seconds from `at + g * every`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub at: f32,
    pub every: f32,
}

/// How long one group takes to rise, s.
pub const RISE: f32 = 0.7;

impl Build {
    /// How far group `group` has risen at `t`, 0 to 1.
    #[must_use]
    pub fn height(&self, group: u8, t: f32) -> f32 {
        let start = self.at + f32::from(group) * self.every;
        smooth(((t - start) / RISE).clamp(0.0, 1.0))
    }
}

/// One shot's set.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub world: World,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub reveal: Option<Reveal>,
    #[serde(default)]
    pub build: Option<Build>,
    /// When the haunt's werewolf, a ghost until then, is summoned solid.
    #[serde(default)]
    pub summon: Option<f32>,
    /// When the werewolf throws its head back and howls.
    #[serde(default)]
    pub howl: Option<f32>,
    /// When the void's portal starts to open.
    #[serde(default)]
    pub portal: Option<f32>,
    /// When the creator's plot is published: its outline lights and a
    /// beam rises.
    #[serde(default)]
    pub publish: Option<f32>,
    /// When gold motes start rising from the visitors in the world.
    #[serde(default)]
    pub sparks: Option<f32>,
    /// People and agents walking their routes.
    #[serde(default)]
    pub walkers: Vec<Walker>,
}

/// A simple shape, its base on `y = 0` in a unit box from `-0.5` to `0.5`
/// across and `0` to `1` up, before its transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Cube,
    /// A gable: a triangle across x, its ridge along z.
    Roof,
    Pyramid,
    Cylinder(u8),
    Cone(u8),
    /// A low-poly ball, `n` facets round.
    Sphere(u8),
}

/// One shape placed and colored. `glow` above zero makes it a light
/// (a window, a jack-o'-lantern), and `group` orders a build.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub shape: Shape,
    pub at: Mat4,
    pub color: [f32; 3],
    pub glow: f32,
    pub group: u8,
}

impl Piece {
    fn new(shape: Shape, pos: Vec3, size: Vec3, yaw: f32, color: [f32; 3]) -> Self {
        Self {
            shape,
            at: Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw) * Mat4::from_scale(size),
            color,
            glow: 0.0,
            group: 0,
        }
    }

    fn placed(shape: Shape, at: Mat4, color: [f32; 3]) -> Self {
        Self {
            shape,
            at,
            color,
            glow: 0.0,
            group: 0,
        }
    }

    fn group(mut self, group: u8) -> Self {
        self.group = group;
        self
    }

    fn glowing(mut self, glow: f32) -> Self {
        self.glow = glow;
        self
    }

    /// Where the piece's middle is.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        self.at.transform_point3(Vec3::new(0.0, 0.5, 0.0))
    }

    /// The piece squashed toward its base to `k` of its height.
    fn risen(&self, k: f32) -> Self {
        let base = self.at.transform_point3(Vec3::ZERO);
        let squash = Mat4::from_translation(base)
            * Mat4::from_scale(Vec3::new(1.0, k.max(1e-3), 1.0))
            * Mat4::from_translation(-base);
        Self {
            at: squash * self.at,
            ..*self
        }
    }
}

/// An sRGB hex color in linear light.
#[must_use]
pub fn hex(rgb: u32) -> [f32; 3] {
    let c = |s: u32| ((rgb >> s) & 0xFF) as f32 / 255.0;
    [c(16), c(8), c(0)].map(|v| v.powf(2.2))
}

fn smooth(k: f32) -> f32 {
    k * k * (3.0 - 2.0 * k)
}

/// The palette of the Haunted House spec, linear.
mod haunt {
    use super::hex;
    pub fn paper() -> [f32; 3] {
        hex(0xF1E4C8)
    }
    pub fn pumpkin() -> [f32; 3] {
        hex(0xE07A2E)
    }
    pub fn licorice() -> [f32; 3] {
        hex(0x1B1714)
    }
    pub fn mustard() -> [f32; 3] {
        hex(0xC99A3B)
    }
    pub fn brick() -> [f32; 3] {
        hex(0x8E3B2A)
    }
    pub fn dusk() -> [f32; 3] {
        hex(0xA9A3C4)
    }
    pub fn plum() -> [f32; 3] {
        hex(0x4A3F5E)
    }
    pub fn moss() -> [f32; 3] {
        hex(0x56664F)
    }
    pub fn night_teal() -> [f32; 3] {
        hex(0x2B3346)
    }
    pub fn candle() -> [f32; 3] {
        hex(0xFFC56B)
    }
}

/// The Grid's black.
const FIELD: [f32; 3] = [0.0022, 0.0022, 0.0026];
/// The Grid's white edge.
const EDGE: [f32; 3] = [0.86, 0.88, 0.92];
/// A ghost's edge: cool and dim, with no faces behind it.
const GHOST: [f32; 3] = [0.34, 0.52, 0.66];
/// The reveal's front: a warm white seam on the ground.
const SEAM: [f32; 3] = [1.6, 1.3, 0.8];
/// The floor grid's cell, m, and how far it runs.
const CELL: f32 = 4.0;
const FLOOR: f32 = 240.0;

/// The OpenAgents planet's center and radius in the galaxy.
const HOME_CENTER: Vec3 = Vec3::new(0.0, -60.0, 0.0);
const HOME_RADIUS: f32 = 60.0;

/// The creator's plot on the Grid: its center `[x, z]` and half size, m.
pub const PLOT: ([f32; 2], f32) = ([0.0, 0.0], 15.0);
/// Plots' spacing on the Grid, m.
pub const PLOT_PITCH: f32 = 36.0;

pub struct Set {
    spec: Spec,
    time: f32,
    pieces: Vec<Piece>,
    fx: Particles,
    lamps: Vec<Lamp>,
    textured: Option<Arc<TexturedScene>>,
}

impl Set {
    pub fn new(spec: Spec) -> Result<Self, String> {
        if let Some(r) = &spec.reveal
            && !(r.speed > 0.0 && r.reach > 0.0 && r.at.is_finite())
        {
            return Err("A reveal needs a positive speed and reach".into());
        }
        if let Some(b) = &spec.build
            && !(b.every >= 0.0 && b.at.is_finite())
        {
            return Err("A build needs a start and a non-negative step".into());
        }
        let mut pieces = pieces(spec.world);
        if spec.world == World::Street && spec.layer == Layer::Grid {
            // The greco buildings' footprints, where the world layer stands
            // their models.
            pieces.extend(street_footprints()?);
        }
        let mut fx = Particles::new(0x4A57);
        let mut lamps = Vec::new();
        let world_lit = spec.layer == Layer::World || spec.reveal.is_some();
        for p in pieces.iter().filter(|p| p.glow > 0.0) {
            if lamps.len() < MAX_LAMPS - 2 && world_lit {
                lamps.push(Lamp {
                    position: p.center() + Vec3::Y * 0.3,
                    color: p.color,
                    intensity: 6.0 * p.glow,
                    range: 7.0,
                });
            }
        }
        match spec.world {
            World::Camp if world_lit => {
                fx.start("grove_campfire", Spawn::at(Vec3::new(0.0, 0.1, 0.0)));
                lamps.push(Lamp {
                    position: Vec3::new(0.0, 0.8, 0.0),
                    color: [1.0, 0.5, 0.18],
                    intensity: 40.0,
                    range: 12.0,
                });
            }
            World::Street if world_lit => {
                fx.start("chimney_smoke", Spawn::at(Vec3::new(14.0, 30.0, -112.0)));
                fx.start("chimney_smoke", Spawn::at(Vec3::new(0.0, 23.0, -112.0)));
            }
            World::Haunt if world_lit => {
                fx.start(
                    "ground_mist",
                    Spawn::at(Vec3::new(0.0, 0.1, 10.0)).scaled(1.6),
                );
            }
            _ => {}
        }
        let textured = if spec.world == World::Street && spec.layer == Layer::World {
            Some(Arc::new(street_kit()?))
        } else {
            None
        };
        let mut set = Self {
            spec,
            time: 0.0,
            pieces,
            fx,
            lamps,
            textured,
        };
        for _ in 0..90 {
            set.fx.tick(1.0 / 30.0, |_, _| 0.0);
        }
        Ok(set)
    }

    /// The static world: the greco kit's buildings on the main street's
    /// world layer, which the renderer uploads once.
    pub fn world(&self) -> Mesh {
        Mesh {
            textured: self.textured.clone(),
            ..Mesh::default()
        }
    }

    pub fn advance(&mut self, dt: f32, t: f32) {
        self.time = t;
        let steps = (dt * 120.0).ceil().max(1.0) as usize;
        for _ in 0..steps {
            self.fx.tick(dt / steps as f32, |_, _| 0.0);
        }
    }

    /// How much of the world's sky shows, 0 (the Grid's black) to 1.
    fn sky(&self, t: f32) -> f32 {
        if self.spec.world == World::Plots {
            // A published plot is a lit island on the Grid, under its night.
            return 0.0;
        }
        match (self.spec.layer, &self.spec.reveal) {
            (Layer::World, _) => 1.0,
            (Layer::Grid, Some(r)) => smooth(r.progress(t)),
            (Layer::Grid, None) => 0.0,
        }
    }

    /// Whether the world layer draws at `at`.
    fn solid(&self, at: Vec3, t: f32) -> bool {
        self.spec.layer == Layer::World || self.spec.reveal.is_some_and(|r| r.covers(at, t))
    }

    pub fn frame(&self) -> Mesh {
        let t = self.time;
        let mut mesh = Mesh {
            neon: Some(self.stage(t)),
            ..Mesh::default()
        };
        let world = self.spec.world;
        if world == World::Galaxy {
            galaxy(&mut mesh, t);
        } else {
            self.ground(&mut mesh, t);
        }
        // The set's pieces, risen as far as the build has them.
        for p in &self.pieces {
            let k = self.spec.build.map_or(1.0, |b| b.height(p.group, t));
            if k <= 0.0 {
                continue;
            }
            let p = if k < 1.0 { p.risen(k) } else { *p };
            let solid = self.solid(p.center(), t);
            draw(&mut mesh, &p, if solid { Look::Solid } else { Look::Grid });
            if solid && p.glow > 0.0 {
                mesh.sprites.push(feathers::dot(
                    p.center(),
                    0.9,
                    p.color.map(|c| c * 1.4 * p.glow),
                    1.0,
                ));
            }
        }
        match world {
            World::Void => self.portal(&mut mesh, t),
            World::Haunt => self.werewolf(&mut mesh, t),
            World::Street => street_people(&mut mesh, t, self),
            World::Camp => camp_people(&mut mesh, t, self),
            World::Plots => {
                plot_outlines(&mut mesh);
                self.publishing(&mut mesh, t);
            }
            World::Galaxy => {}
        }
        if world == World::Haunt || world == World::Plots {
            // The moon over the haunt, once its sky has come in.
            let k = self.sky(t);
            if k > 0.0 {
                let moon = Vec3::new(40.0, 70.0, -160.0);
                mesh.sprites
                    .push(feathers::dot(moon, 9.0, [3.2 * k, 3.1 * k, 2.8 * k], 1.0));
                mesh.sprites
                    .push(feathers::dot(moon, 30.0, [0.2 * k, 0.18 * k, 0.3 * k], 1.0));
            }
        }
        // The walkers: ghosts on the Grid, solid in the world.
        for (i, w) in self.spec.walkers.iter().enumerate() {
            let (pos, rot, speed) = w.pose(t);
            let yaw = {
                let f = rot * Vec3::Z;
                f.x.atan2(f.z)
            };
            let walked = w.start + w.speed.max(0.0) * t.max(0.0);
            let solid = self.solid(pos, t);
            let mut parts = Vec::new();
            let figure = Figure {
                at: pos,
                yaw,
                stride: walked,
                moving: speed > 0.05,
                kind: if w.agent {
                    Kind::Agent
                } else {
                    Kind::Person(i as u32)
                },
            };
            figure.pieces(&mut parts);
            for p in &parts {
                draw(&mut mesh, p, if solid { Look::Solid } else { Look::Ghost });
            }
            if !solid {
                // A ghost's faint glow.
                mesh.sprites.push(feathers::dot(
                    pos + Vec3::Y * 1.1,
                    0.9,
                    [0.04, 0.07, 0.1],
                    1.0,
                ));
            } else if let Some(s) = self.spec.sparks
                && t > s
            {
                // Gold motes rising off a visitor: what they leave behind.
                for k in 0..3u32 {
                    let seed = i as u32 * 7 + k;
                    let life = ((t - s) * 0.7 + hash(seed)) % 1.0;
                    let drift = Vec3::new(
                        (hash(seed + 11) - 0.5) * 0.8,
                        2.0 + life * 3.5,
                        (hash(seed + 13) - 0.5) * 0.8,
                    );
                    let fade = (1.0 - life) * (life * 6.0).min(1.0);
                    mesh.sprites.push(feathers::dot(
                        pos + drift,
                        0.09,
                        [3.0 * fade, 2.2 * fade, 0.6 * fade],
                        1.0,
                    ));
                }
            }
        }
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    /// The floor: the Grid's lines wherever the Grid layer is, and the
    /// world's ground wherever the world is, with a seam where they meet.
    fn ground(&self, mesh: &mut Mesh, t: f32) {
        let world = self.spec.world;
        let ground = match world {
            World::Haunt | World::Plots => haunt::moss(),
            World::Camp => hex(0x5B6B3A),
            World::Street => hex(0x8C7A62),
            _ => FIELD,
        };
        let hole = match (self.spec.layer, &self.spec.reveal) {
            (Layer::World, _) => {
                lit_polygon(
                    &mut mesh.lit,
                    &[
                        Vec3::new(-FLOOR, 0.0, FLOOR),
                        Vec3::new(FLOOR, 0.0, FLOOR),
                        Vec3::new(FLOOR, 0.0, -FLOOR),
                        Vec3::new(-FLOOR, 0.0, -FLOOR),
                    ],
                    ground,
                    0.95,
                );
                return;
            }
            (Layer::Grid, Some(r)) => {
                let radius = r.radius(t);
                let c = Vec3::new(r.center[0], 0.0, r.center[1]);
                if radius > 0.0 {
                    let ring: Vec<Vec3> = (0..96)
                        .map(|i| {
                            let a = i as f32 / 96.0 * std::f32::consts::TAU;
                            c + Vec3::new(a.cos(), 0.0, -a.sin()) * radius
                        })
                        .collect();
                    lit_polygon(&mut mesh.lit, &ring, ground, 0.95);
                    if r.progress(t) < 1.0 {
                        // The seam, a little above the floor, and sparks
                        // along it.
                        for i in 0..96 {
                            let a = ring[i] + Vec3::Y * 0.03;
                            let b = ring[(i + 1) % 96] + Vec3::Y * 0.03;
                            line(mesh, a, b, SEAM);
                        }
                        for i in 0..40u32 {
                            let a = (hash(i * 3 + 1) + t * 0.05) * std::f32::consts::TAU;
                            let up = ((t * 1.3 + hash(i * 3 + 2)) % 1.0) * 1.6;
                            mesh.sprites.push(feathers::dot(
                                c + Vec3::new(a.cos(), 0.0, -a.sin()) * radius + Vec3::Y * up,
                                0.06,
                                [2.4, 1.9, 1.0],
                                1.0,
                            ));
                        }
                    }
                }
                Some((c, radius))
            }
            (Layer::Grid, None) => None,
        };
        floor_lines(mesh, hole);
    }

    fn stage(&self, t: f32) -> Neon {
        let mut neon = Neon::neutral(t);
        let k = self.sky(t);
        let world = self.spec.world;
        neon.field = FIELD;
        // The Grid fades to black well before its lines crowd the horizon.
        neon.fog_start = 20.0;
        neon.fog_end = 120.0;
        neon.line_gain = 1.25;
        neon.line_width = 1.5;
        neon.bloom = 0.08;
        neon.vignette = 0.3;
        neon.grade.contrast = 1.08;
        let (sun, sun_color, light, sky) = match world {
            World::Haunt | World::Plots | World::Void => (
                Vec3::new(0.35, 0.55, -0.75),
                [0.75, 0.78, 1.0],
                2.4,
                Daylight {
                    zenith: hex(0x2E2645),
                    horizon: hex(0xA9A3C4),
                    sun: [0.9, 0.85, 1.0],
                    clouds: 0.35,
                    ground: hex(0x56664F),
                    glow: 0.35,
                },
            ),
            World::Camp => (
                Vec3::new(-0.25, 0.22, -0.95),
                [1.0, 0.62, 0.34],
                3.0,
                Daylight {
                    zenith: hex(0x2C3F7A),
                    horizon: hex(0xF0A770),
                    sun: [1.0, 0.6, 0.3],
                    clouds: 0.4,
                    ground: hex(0x5B6B3A),
                    glow: 0.85,
                },
            ),
            World::Street => (
                Vec3::new(-0.55, 0.42, 0.6),
                [1.0, 0.78, 0.55],
                3.2,
                Daylight {
                    zenith: hex(0x3D6FC2),
                    horizon: hex(0xF3D3A8),
                    sun: [1.0, 0.75, 0.5],
                    clouds: 0.3,
                    ground: hex(0x8C7A62),
                    glow: 0.45,
                },
            ),
            World::Galaxy => (
                Vec3::new(0.6, 0.45, 0.65),
                [1.0, 0.95, 0.9],
                3.0,
                Daylight {
                    zenith: FIELD,
                    horizon: FIELD,
                    sun: [1.0; 3],
                    clouds: 0.0,
                    ground: FIELD,
                    glow: 0.0,
                },
            ),
        };
        let mut light_key = key(sun, light, 0.5);
        light_key.shadow_center = Vec3::new(0.0, 0.0, -10.0);
        light_key.shadow_half = 70.0;
        light_key.rim_illuminance = light * 0.25;
        neon.key = Some(light_key);
        neon.key_color = sun_color;
        if world == World::Galaxy {
            neon.fog_start = 4_000.0;
            neon.fog_end = 9_000.0;
            neon.vignette = 0.45;
            light_key.sky = 0.08;
            light_key.ground = 0.02;
            light_key.shadow_half = 600.0;
            neon.key = Some(light_key);
        } else if k > 0.0 {
            let scale = |c: [f32; 3]| [0, 1, 2].map(|i| FIELD[i] + (c[i] - FIELD[i]) * k);
            neon.daylight = Some(Daylight {
                zenith: scale(sky.zenith),
                horizon: scale(sky.horizon),
                ground: scale(sky.ground),
                ..sky
            });
            neon.fog_start = 20.0 + 60.0 * k;
            neon.fog_end = 120.0 + 500.0 * k;
        }
        for (i, lamp) in self.lamps.iter().take(MAX_LAMPS).enumerate() {
            neon.lamps[i] = lamp.flickering(t, i as u32);
        }
        neon
    }

    /// The void's portal: an arch whose edges light, then an oval of warm
    /// light opening inside it.
    fn portal(&self, mesh: &mut Mesh, t: f32) {
        let Some(at) = self.spec.portal else {
            return;
        };
        let center = Vec3::new(0.0, 2.1, -14.0);
        let open = smooth(((t - at - 0.6) / 1.2).clamp(0.0, 1.0));
        if open <= 0.0 {
            return;
        }
        // The oval's rim and the light within, swirling.
        let (rx, ry) = (1.45 * open, 1.85 * open);
        let rim: Vec<Vec3> = (0..64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                center + Vec3::new(a.cos() * rx, a.sin() * ry, 0.0)
            })
            .collect();
        for i in 0..64 {
            line(mesh, rim[i], rim[(i + 1) % 64], [2.2, 2.0, 1.7]);
        }
        let colors = [
            haunt::pumpkin(),
            haunt::dusk(),
            haunt::candle(),
            haunt::plum(),
            hex(0x7FB7E8),
        ];
        for i in 0..220u32 {
            let r = hash(i * 5 + 1).sqrt();
            let a = hash(i * 5 + 2) * std::f32::consts::TAU + t * (0.9 - 0.6 * r);
            let c = colors[(i % 5) as usize];
            let at = center + Vec3::new(a.cos() * rx * r, a.sin() * ry * r, -0.05);
            mesh.sprites.push(feathers::dot(
                at,
                0.32 * open,
                c.map(|v| v * 1.3 * open),
                1.0,
            ));
        }
        mesh.sprites
            .push(feathers::dot(center, 2.4 * open, [0.5, 0.42, 0.36], 1.0));
    }

    /// The haunt's werewolf: a ghost on the path until it is summoned, then
    /// solid, and at `howl` it throws its head back at the moon.
    fn werewolf(&self, mesh: &mut Mesh, t: f32) {
        let Some(summon) = self.spec.summon else {
            return;
        };
        let at = Vec3::new(3.2, 0.0, 9.0);
        let howl = self
            .spec
            .howl
            .map_or(0.0, |h| smooth(((t - h) / 0.5).clamp(0.0, 1.0)));
        let wolf = Figure {
            at,
            yaw: -0.35,
            stride: 0.0,
            moving: false,
            kind: Kind::Wolf(howl),
        };
        let mut parts = Vec::new();
        wolf.pieces(&mut parts);
        let solid = t >= summon && self.solid(at, t);
        // It fades in as a ghost a second before it is summoned.
        if !solid && t < summon - 1.2 {
            return;
        }
        for p in &parts {
            draw(mesh, p, if solid { Look::Solid } else { Look::Ghost });
        }
        // The summoning: a flash and a burst of motes.
        let since = t - summon;
        if (0.0..0.8).contains(&since) {
            let k = 1.0 - since / 0.8;
            mesh.sprites.push(feathers::dot(
                at + Vec3::Y * 1.4,
                2.6 * k + 0.4,
                [3.0 * k, 2.6 * k, 2.0 * k],
                1.0,
            ));
            for i in 0..40u32 {
                let a = hash(i + 50) * std::f32::consts::TAU;
                let r = since * (2.0 + 2.0 * hash(i + 51));
                mesh.sprites.push(feathers::dot(
                    at + Vec3::new(a.cos() * r, 0.4 + 2.4 * hash(i + 52), a.sin() * r),
                    0.07,
                    [2.4 * k, 2.0 * k, 1.2 * k],
                    1.0,
                ));
            }
        }
    }

    /// The plots: the creator's outline lighting gold and a beam rising
    /// when it is published.
    fn publishing(&self, mesh: &mut Mesh, t: f32) {
        let ([cx, cz], half) = PLOT;
        let Some(at) = self.spec.publish else {
            return;
        };
        let k = smooth(((t - at) / 0.6).clamp(0.0, 1.0));
        if k <= 0.0 {
            return;
        }
        let gold = [2.6 * k, 1.9 * k, 0.6 * k];
        for inset in [0.0_f32, 0.25, 0.5] {
            let h = half - inset;
            let c = [
                Vec3::new(cx - h, 0.04, cz - h),
                Vec3::new(cx + h, 0.04, cz - h),
                Vec3::new(cx + h, 0.04, cz + h),
                Vec3::new(cx - h, 0.04, cz + h),
            ];
            for i in 0..4 {
                line(mesh, c[i], c[(i + 1) % 4], gold);
            }
        }
        let rise = smooth(((t - at) / 1.2).clamp(0.0, 1.0));
        mesh.ribbons.extend(feathers::beam(
            Vec3::new(cx, 0.0, cz - half),
            Vec3::new(cx, 2.0 + 60.0 * rise, cz - half),
            0.25,
            [3.0 * k, 2.3 * k, 1.0 * k],
        ));
    }
}

/// How a piece is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    /// White edges over black faces.
    Grid,
    /// Dim cool edges and nothing behind them.
    Ghost,
    /// Lit and colored.
    Solid,
}

fn line(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    mesh.lines.push(Vertex {
        pos: a.to_array(),
        color,
        fog: 1.0,
    });
    mesh.lines.push(Vertex {
        pos: b.to_array(),
        color,
        fog: 1.0,
    });
}

/// The floor's grid lines, leaving out the cells inside `hole` (a center
/// and radius), where the world's ground is.
fn floor_lines(mesh: &mut Mesh, hole: Option<(Vec3, f32)>) {
    let n = (FLOOR / CELL) as i32;
    for axis in 0..2 {
        for i in -n..=n {
            let c = i as f32 * CELL;
            let point = |s: f32| {
                if axis == 0 {
                    Vec3::new(s, 0.0, c)
                } else {
                    Vec3::new(c, 0.0, s)
                }
            };
            // Where the hole is along this line, when the line passes near it.
            let cut = hole.and_then(|(h, r)| {
                let (across, along) = if axis == 0 { (h.z, h.x) } else { (h.x, h.z) };
                ((c - across).abs() < r + CELL).then_some((along, r, h))
            });
            let Some((along, r, h)) = cut else {
                line(mesh, point(-FLOOR), point(FLOOR), EDGE);
                continue;
            };
            let (lo, hi) = (along - r - CELL, along + r + CELL);
            line(mesh, point(-FLOOR), point(lo), EDGE);
            line(mesh, point(hi), point(FLOOR), EDGE);
            let inside = |p: Vec3| glam::Vec2::new(p.x - h.x, p.z - h.z).length() < r;
            let mut s = lo;
            while s < hi {
                let a = point(s);
                let b = point((s + CELL * 0.5).min(hi));
                if !inside(a) && !inside(b) {
                    line(mesh, a, b, EDGE);
                }
                s += CELL * 0.5;
            }
        }
    }
}

/// A shape's faces in its unit box, each a convex polygon, wound either
/// way.
fn polygons(shape: Shape) -> Vec<Vec<Vec3>> {
    let ring = |n: u8, y: f32, r: f32| -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = f32::from(i) / f32::from(n) * std::f32::consts::TAU;
                Vec3::new(a.cos() * r, y, a.sin() * r)
            })
            .collect()
    };
    match shape {
        Shape::Cube => {
            let c = |x: f32, y: f32, z: f32| Vec3::new(x - 0.5, y, z - 0.5);
            vec![
                vec![c(0., 0., 0.), c(1., 0., 0.), c(1., 0., 1.), c(0., 0., 1.)],
                vec![c(0., 1., 0.), c(1., 1., 0.), c(1., 1., 1.), c(0., 1., 1.)],
                vec![c(0., 0., 0.), c(1., 0., 0.), c(1., 1., 0.), c(0., 1., 0.)],
                vec![c(0., 0., 1.), c(1., 0., 1.), c(1., 1., 1.), c(0., 1., 1.)],
                vec![c(0., 0., 0.), c(0., 0., 1.), c(0., 1., 1.), c(0., 1., 0.)],
                vec![c(1., 0., 0.), c(1., 0., 1.), c(1., 1., 1.), c(1., 1., 0.)],
            ]
        }
        Shape::Roof => {
            let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
            vec![
                vec![p(-0.5, 0., -0.5), p(0.5, 0., -0.5), p(0., 1., -0.5)],
                vec![p(-0.5, 0., 0.5), p(0.5, 0., 0.5), p(0., 1., 0.5)],
                vec![
                    p(-0.5, 0., -0.5),
                    p(0., 1., -0.5),
                    p(0., 1., 0.5),
                    p(-0.5, 0., 0.5),
                ],
                vec![
                    p(0.5, 0., -0.5),
                    p(0., 1., -0.5),
                    p(0., 1., 0.5),
                    p(0.5, 0., 0.5),
                ],
                vec![
                    p(-0.5, 0., -0.5),
                    p(0.5, 0., -0.5),
                    p(0.5, 0., 0.5),
                    p(-0.5, 0., 0.5),
                ],
            ]
        }
        Shape::Pyramid => {
            let apex = Vec3::Y;
            let b = [
                Vec3::new(-0.5, 0.0, -0.5),
                Vec3::new(0.5, 0.0, -0.5),
                Vec3::new(0.5, 0.0, 0.5),
                Vec3::new(-0.5, 0.0, 0.5),
            ];
            let mut out: Vec<Vec<Vec3>> =
                (0..4).map(|i| vec![b[i], b[(i + 1) % 4], apex]).collect();
            out.push(b.to_vec());
            out
        }
        Shape::Cylinder(n) => {
            let n = n.max(3);
            let lo = ring(n, 0.0, 0.5);
            let hi = ring(n, 1.0, 0.5);
            let mut out: Vec<Vec<Vec3>> = (0..n as usize)
                .map(|i| {
                    let j = (i + 1) % n as usize;
                    vec![lo[i], lo[j], hi[j], hi[i]]
                })
                .collect();
            out.push(lo);
            out.push(hi);
            out
        }
        Shape::Cone(n) => {
            let n = n.max(3);
            let lo = ring(n, 0.0, 0.5);
            let mut out: Vec<Vec<Vec3>> = (0..n as usize)
                .map(|i| vec![lo[i], lo[(i + 1) % n as usize], Vec3::Y])
                .collect();
            out.push(lo);
            out
        }
        Shape::Sphere(n) => {
            let n = n.max(4) as usize;
            let rows = (n / 2).max(2);
            let at = |row: usize, col: usize| {
                let v = row as f32 / rows as f32 * std::f32::consts::PI;
                let a = col as f32 / n as f32 * std::f32::consts::TAU;
                Vec3::new(
                    v.sin() * a.cos() * 0.5,
                    0.5 - v.cos() * 0.5,
                    v.sin() * a.sin() * 0.5,
                )
            };
            let mut out = Vec::new();
            for row in 0..rows {
                for col in 0..n {
                    let next = (col + 1) % n;
                    out.push(if row == 0 {
                        vec![at(0, 0), at(1, col), at(1, next)]
                    } else if row + 1 == rows {
                        vec![at(row, col), at(rows, 0), at(row, next)]
                    } else {
                        vec![
                            at(row, col),
                            at(row + 1, col),
                            at(row + 1, next),
                            at(row, next),
                        ]
                    });
                }
            }
            out
        }
    }
}

/// A shape's faces placed by `at`, each wound so its normal faces out.
fn faces(shape: Shape, at: Mat4) -> Vec<Vec<Vec3>> {
    let middle = at.transform_point3(Vec3::new(0.0, 0.45, 0.0));
    polygons(shape)
        .into_iter()
        .map(|f| {
            let mut f: Vec<Vec3> = f.into_iter().map(|p| at.transform_point3(p)).collect();
            let center = f.iter().copied().sum::<Vec3>() / f.len() as f32;
            let normal = (f[1] - f[0]).cross(f[2] - f[0]);
            if normal.dot(center - middle) < 0.0 {
                f.reverse();
            }
            f
        })
        .collect()
}

/// A convex polygon as lit triangles, flat shaded.
fn lit_polygon(out: &mut Vec<LitVertex>, f: &[Vec3], color: [f32; 3], rough: f32) {
    if f.len() < 3 {
        return;
    }
    let normal = (f[1] - f[0]).cross(f[2] - f[0]).normalize_or(Vec3::Y);
    for i in 1..f.len() - 1 {
        for p in [f[0], f[i], f[i + 1]] {
            out.push(LitVertex {
                pos: p.to_array(),
                normal: normal.to_array(),
                tangent: Vec3::X.to_array(),
                local: p.to_array(),
                color,
                params: [0.0, rough, Material::Dark.code(), 1.0],
            });
        }
    }
}

fn draw(mesh: &mut Mesh, p: &Piece, look: Look) {
    let fs = faces(p.shape, p.at);
    match look {
        Look::Solid => {
            for f in &fs {
                lit_polygon(&mut mesh.lit, f, p.color, 0.85);
            }
        }
        Look::Grid | Look::Ghost => {
            let color = if look == Look::Grid { EDGE } else { GHOST };
            for f in &fs {
                if look == Look::Grid {
                    for i in 1..f.len() - 1 {
                        for q in [f[0], f[i], f[i + 1]] {
                            mesh.faces.push(Vertex {
                                pos: q.to_array(),
                                color: FIELD,
                                fog: 1.0,
                            });
                        }
                    }
                }
                for i in 0..f.len() {
                    line(mesh, f[i], f[(i + 1) % f.len()], color);
                }
            }
        }
    }
}

/// Someone on the set.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// A person in clothes picked by the seed.
    Person(u32),
    /// An agent: a hooded, robed figure.
    Agent,
    /// The werewolf, howling as far as the value says, 0 to 1.
    Wolf(f32),
    /// A person talking with their hands.
    Talker(u32),
    /// The shopkeeper in an apron, talking with her hands.
    Keeper,
    /// A dog.
    Dog,
}

/// A blocky character standing or walking at `at`, facing `yaw`, `stride`
/// meters into its walk.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Figure {
    at: Vec3,
    yaw: f32,
    stride: f32,
    moving: bool,
    kind: Kind,
}

impl Figure {
    fn pieces(&self, out: &mut Vec<Piece>) {
        let root = Mat4::from_translation(self.at) * Mat4::from_rotation_y(self.yaw);
        let phase = self.stride / 1.3 * std::f32::consts::TAU;
        let swing = if self.moving { phase.sin() * 0.55 } else { 0.0 };
        // A box hanging from `pivot`, swung by `angle` about x.
        let limb = |pivot: Vec3, angle: f32, size: Vec3| {
            root * Mat4::from_translation(pivot)
                * Mat4::from_rotation_x(angle)
                * Mat4::from_translation(Vec3::new(0.0, -size.y, 0.0))
                * Mat4::from_scale(size)
        };
        let block =
            |pos: Vec3, size: Vec3| root * Mat4::from_translation(pos) * Mat4::from_scale(size);
        let skins = [hex(0xE7C3A0), hex(0xC48D64), hex(0x8D5A3B), hex(0xF1D4B8)];
        let shirts = [
            haunt::pumpkin(),
            hex(0x3F6E8C),
            haunt::mustard(),
            haunt::brick(),
            hex(0x6A8D5B),
            haunt::plum(),
            hex(0xD9D2C3),
        ];
        let pants = [
            haunt::night_teal(),
            hex(0x3B3A40),
            hex(0x5A4636),
            hex(0x2F3E5A),
        ];
        match self.kind {
            Kind::Person(_) | Kind::Talker(_) | Kind::Keeper => {
                let (seed, talk, apron) = match self.kind {
                    Kind::Person(s) => (s, false, false),
                    Kind::Talker(s) => (s, true, false),
                    _ => (3, true, true),
                };
                let skin = skins[(seed % 4) as usize];
                let shirt = shirts[((seed / 2 + seed) % 7) as usize];
                let pant = pants[((seed / 3) % 4) as usize];
                let bob = if self.moving {
                    (phase * 2.0).cos().abs() * 0.04
                } else {
                    0.0
                };
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(-0.12, 0.92 + bob, 0.0),
                        swing,
                        Vec3::new(0.18, 0.92, 0.2),
                    ),
                    pant,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(0.12, 0.92 + bob, 0.0),
                        -swing,
                        Vec3::new(0.18, 0.92, 0.2),
                    ),
                    pant,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 0.9 + bob, 0.0), Vec3::new(0.48, 0.62, 0.28)),
                    shirt,
                ));
                let wave = if talk {
                    // Talking with her hands.
                    -1.1 - 0.35 * (self.stride * 2.6).sin()
                } else {
                    -swing * 0.8
                };
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(-0.31, 1.5 + bob, 0.0),
                        swing * 0.8,
                        Vec3::new(0.14, 0.6, 0.16),
                    ),
                    shirt,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(0.31, 1.5 + bob, 0.0),
                        wave,
                        Vec3::new(0.14, 0.6, 0.16),
                    ),
                    shirt,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 1.53 + bob, 0.0), Vec3::new(0.3, 0.32, 0.3)),
                    skin,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 1.8 + bob, -0.02), Vec3::new(0.33, 0.1, 0.33)),
                    pant,
                ));
                if apron {
                    out.push(Piece::placed(
                        Shape::Cube,
                        block(Vec3::new(0.0, 0.7, 0.15), Vec3::new(0.44, 0.75, 0.03)),
                        haunt::paper(),
                    ));
                }
            }
            Kind::Agent => {
                let robe = hex(0xC9CDE0);
                let bob = (self.stride * 0.8).sin() * 0.05;
                out.push(Piece::placed(
                    Shape::Cone(8),
                    block(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.75, 1.55, 0.75)),
                    robe,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 0.9 + bob, 0.0), Vec3::new(0.42, 0.55, 0.3)),
                    robe,
                ));
                out.push(Piece::placed(
                    Shape::Pyramid,
                    block(Vec3::new(0.0, 1.42 + bob, 0.0), Vec3::new(0.42, 0.55, 0.42)),
                    hex(0x8C93B8),
                ));
                out.push(
                    Piece::placed(
                        Shape::Cube,
                        block(
                            Vec3::new(0.0, 1.55 + bob, 0.17),
                            Vec3::new(0.16, 0.06, 0.02),
                        ),
                        [0.5, 0.9, 1.0],
                    )
                    .glowing(0.0),
                );
            }
            Kind::Wolf(howl) => {
                let fur = hex(0x4A3F38);
                let belly = hex(0x7A6A5C);
                let s = 1.25;
                let lean = 0.35 * (1.0 - howl) - 0.15 * howl;
                let torso = root
                    * Mat4::from_translation(Vec3::new(0.0, 1.05 * s, 0.0))
                    * Mat4::from_rotation_x(lean);
                let part = |pos: Vec3, angle: f32, size: Vec3| {
                    torso
                        * Mat4::from_translation(pos)
                        * Mat4::from_rotation_x(angle)
                        * Mat4::from_scale(size)
                };
                // Haunches and legs.
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(-0.2 * s, 1.05 * s, 0.0),
                        0.25,
                        Vec3::new(0.26, 1.05 * s, 0.32),
                    ),
                    fur,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    limb(
                        Vec3::new(0.2 * s, 1.05 * s, 0.0),
                        -0.1,
                        Vec3::new(0.26, 1.05 * s, 0.32),
                    ),
                    fur,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    part(Vec3::ZERO, 0.0, Vec3::new(0.7, 0.85, 0.45) * s),
                    fur,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    part(Vec3::new(0.0, 0.1, 0.2), 0.0, Vec3::new(0.45, 0.6, 0.1) * s),
                    belly,
                ));
                // Arms: hanging, then flung wide for the howl.
                for side in [-1.0_f32, 1.0] {
                    let arm = torso
                        * Mat4::from_translation(Vec3::new(side * 0.42 * s, 0.8 * s, 0.0))
                        * Mat4::from_rotation_z(side * (0.15 + 0.9 * howl))
                        * Mat4::from_rotation_x(-0.3 - 0.4 * howl)
                        * Mat4::from_translation(Vec3::new(0.0, -0.85 * s, 0.0))
                        * Mat4::from_scale(Vec3::new(0.2, 0.85, 0.22) * s);
                    out.push(Piece::placed(Shape::Cube, arm, fur));
                }
                // The head, thrown back at the moon as it howls.
                let head = torso
                    * Mat4::from_translation(Vec3::new(0.0, 0.85 * s, 0.05))
                    * Mat4::from_rotation_x(0.25 - 1.15 * howl);
                let hp = |pos: Vec3, size: Vec3| {
                    head * Mat4::from_translation(pos) * Mat4::from_scale(size * s)
                };
                out.push(Piece::placed(
                    Shape::Cube,
                    hp(Vec3::ZERO, Vec3::new(0.42, 0.4, 0.42)),
                    fur,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    hp(Vec3::new(0.0, 0.06, 0.32), Vec3::new(0.24, 0.2, 0.34)),
                    belly,
                ));
                out.push(Piece::placed(
                    Shape::Pyramid,
                    hp(Vec3::new(-0.13, 0.4, -0.05), Vec3::new(0.14, 0.24, 0.12)),
                    fur,
                ));
                out.push(Piece::placed(
                    Shape::Pyramid,
                    hp(Vec3::new(0.13, 0.4, -0.05), Vec3::new(0.14, 0.24, 0.12)),
                    fur,
                ));
                for x in [-0.1_f32, 0.1] {
                    out.push(
                        Piece::placed(
                            Shape::Cube,
                            hp(Vec3::new(x, 0.24, 0.215), Vec3::new(0.07, 0.05, 0.01)),
                            [1.0, 0.75, 0.2],
                        )
                        .glowing(0.0),
                    );
                }
                // The tail.
                out.push(Piece::placed(
                    Shape::Cube,
                    root * Mat4::from_translation(Vec3::new(0.0, 0.95 * s, -0.25))
                        * Mat4::from_rotation_x(-2.3)
                        * Mat4::from_scale(Vec3::new(0.16, 0.7, 0.16) * s),
                    fur,
                ));
            }
            Kind::Dog => {
                let coat = hex(0xB07A43);
                let trot = if self.moving { phase.sin() * 0.5 } else { 0.0 };
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 0.32, 0.0), Vec3::new(0.24, 0.24, 0.6)),
                    coat,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 0.48, 0.34), Vec3::new(0.2, 0.2, 0.22)),
                    coat,
                ));
                out.push(Piece::placed(
                    Shape::Cube,
                    block(Vec3::new(0.0, 0.48, 0.5), Vec3::new(0.11, 0.1, 0.12)),
                    hex(0x3A2A1E),
                ));
                for (x, z, k) in [
                    (-0.08, 0.22, 1.0),
                    (0.08, 0.22, -1.0),
                    (-0.08, -0.22, -1.0),
                    (0.08, -0.22, 1.0),
                ] {
                    out.push(Piece::placed(
                        Shape::Cube,
                        limb(Vec3::new(x, 0.32, z), trot * k, Vec3::new(0.07, 0.32, 0.07)),
                        coat,
                    ));
                }
                out.push(Piece::placed(
                    Shape::Cube,
                    root * Mat4::from_translation(Vec3::new(0.0, 0.4, -0.3))
                        * Mat4::from_rotation_x(-0.7 + 0.3 * (self.stride * 9.0).sin())
                        * Mat4::from_scale(Vec3::new(0.05, 0.28, 0.05)),
                    coat,
                ));
            }
        }
    }
}

/// Every world's pieces.
#[must_use]
pub fn pieces(world: World) -> Vec<Piece> {
    let mut out = Vec::new();
    match world {
        World::Void | World::Galaxy => {
            if world == World::Void {
                // The arch the portal opens in: two posts and a lintel,
                // rising in turn.
                let c = Vec3::new(0.0, 0.0, -14.0);
                out.push(
                    Piece::new(
                        Shape::Cube,
                        c + Vec3::new(-1.9, 0.0, 0.0),
                        Vec3::new(0.35, 4.6, 0.35),
                        0.0,
                        EDGE,
                    )
                    .group(0),
                );
                out.push(
                    Piece::new(
                        Shape::Cube,
                        c + Vec3::new(1.9, 0.0, 0.0),
                        Vec3::new(0.35, 4.6, 0.35),
                        0.0,
                        EDGE,
                    )
                    .group(1),
                );
                out.push(
                    Piece::new(
                        Shape::Cube,
                        c + Vec3::new(0.0, 4.6, 0.0),
                        Vec3::new(4.15, 0.35, 0.35),
                        0.0,
                        EDGE,
                    )
                    .group(2),
                );
            }
        }
        World::Haunt => haunted_house(&mut out, Vec3::ZERO, 1.0),
        World::Plots => {
            haunted_house(&mut out, Vec3::new(PLOT.0[0], 0.0, PLOT.0[1]), 1.0);
            out.retain(|p| p.group != 4);
            neighbor_plots(&mut out);
        }
        World::Camp => camp(&mut out),
        World::Street => street(&mut out),
    }
    out
}

/// The Haunted House stand-in at `at`, its door facing +z, in the build
/// groups a voice prompt lists: ground and path, the house, the lanterns,
/// the trees and stones, then the far hills.
fn haunted_house(out: &mut Vec<Piece>, at: Vec3, s: f32) {
    let p = |x: f32, y: f32, z: f32| at + Vec3::new(x, y, z) * s;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * s;
    // 0: the path and the steps.
    for i in 0..8 {
        let z = 5.2 + i as f32 * 1.5;
        let wob = (hash(i + 700) - 0.5) * 0.5;
        out.push(
            Piece::new(
                Shape::Cube,
                p(wob * 0.6, 0.0, z),
                v(1.8, 0.08, 1.2),
                wob * 0.3,
                haunt::mustard(),
            )
            .group(0),
        );
    }
    out.push(
        Piece::new(
            Shape::Cube,
            p(0.0, 0.0, 4.4),
            v(3.2, 0.35, 1.0),
            0.0,
            haunt::dusk(),
        )
        .group(0),
    );
    out.push(
        Piece::new(
            Shape::Cube,
            p(0.0, 0.0, 3.8),
            v(3.2, 0.7, 0.6),
            0.0,
            haunt::dusk(),
        )
        .group(0),
    );
    // 1: the house: a crooked body, its roof, a leaning tower, a chimney,
    // a porch roof on posts, and the door ajar.
    let lean = |m: Mat4, a: f32| {
        let base = m.transform_point3(Vec3::ZERO);
        Mat4::from_translation(base) * Mat4::from_rotation_z(a) * Mat4::from_translation(-base) * m
    };
    let body = Piece::new(
        Shape::Cube,
        p(0.0, 0.0, -0.5),
        v(10.0, 6.2, 8.0),
        0.03,
        haunt::plum(),
    );
    out.push(body.group(1));
    out.push(
        Piece::new(
            Shape::Roof,
            p(0.0, 6.2, -0.5),
            v(11.2, 3.6, 9.0),
            0.03,
            haunt::night_teal(),
        )
        .group(1),
    );
    let tower = Piece::new(
        Shape::Cube,
        p(-5.4, 0.0, 0.6),
        v(3.0, 10.5, 3.0),
        0.0,
        haunt::brick(),
    );
    out.push(
        Piece {
            at: lean(tower.at, 0.04),
            ..tower
        }
        .group(1),
    );
    let spire = Piece::new(
        Shape::Pyramid,
        p(-5.85, 10.5, 0.6),
        v(3.6, 4.4, 3.6),
        0.0,
        haunt::night_teal(),
    );
    out.push(
        Piece {
            at: lean(spire.at, 0.07),
            ..spire
        }
        .group(1),
    );
    out.push(
        Piece::new(
            Shape::Cube,
            p(3.2, 6.5, -2.0),
            v(1.0, 4.2, 1.0),
            0.0,
            haunt::brick(),
        )
        .group(1),
    );
    out.push(
        Piece::new(
            Shape::Cube,
            p(0.0, 3.0, 3.9),
            v(4.4, 0.25, 1.9),
            0.0,
            haunt::night_teal(),
        )
        .group(1),
    );
    for x in [-1.9_f32, 1.9] {
        out.push(
            Piece::new(
                Shape::Cube,
                p(x, 0.0, 4.6),
                v(0.22, 3.0, 0.22),
                0.0,
                haunt::paper(),
            )
            .group(1),
        );
    }
    out.push(
        Piece::new(
            Shape::Cube,
            p(-0.4, 0.0, 3.52),
            v(1.0, 2.3, 0.08),
            0.5,
            haunt::licorice(),
        )
        .group(1),
    );
    out.push(
        Piece::new(
            Shape::Cube,
            p(0.35, 0.0, 3.48),
            v(0.5, 2.2, 0.04),
            0.0,
            haunt::candle(),
        )
        .glowing(1.2)
        .group(1),
    );
    // Lit windows, two floors.
    for (x, y) in [
        (-3.0_f32, 1.4_f32),
        (3.0, 1.4),
        (-3.0, 4.1),
        (0.0, 4.1),
        (3.0, 4.1),
    ] {
        out.push(
            Piece::new(
                Shape::Cube,
                p(x, y, 3.52),
                v(1.1, 1.4, 0.06),
                0.0,
                haunt::candle(),
            )
            .glowing(1.0)
            .group(1),
        );
    }
    out.push(
        Piece::new(
            Shape::Cube,
            p(-5.4, 6.8, 2.12),
            v(0.9, 1.3, 0.06),
            0.0,
            haunt::candle(),
        )
        .glowing(1.0)
        .group(1),
    );
    // 2: jack-o'-lanterns up the path and on the steps.
    for (i, (x, z)) in [
        (-1.6_f32, 4.0_f32),
        (1.6, 4.0),
        (-1.8, 7.0),
        (1.9, 8.6),
        (-1.7, 10.4),
        (1.8, 12.2),
        (-1.9, 14.0),
    ]
    .into_iter()
    .enumerate()
    {
        let r = 0.55 + 0.2 * hash(i as u32 + 40);
        let y = if z < 5.0 { 0.7 } else { 0.0 };
        out.push(
            Piece::new(
                Shape::Sphere(8),
                p(x, y, z),
                v(r * 1.25, r, r * 1.25),
                hash(i as u32 + 41),
                haunt::pumpkin(),
            )
            .group(2),
        );
        out.push(
            Piece::new(
                Shape::Cube,
                p(x, y + r, z),
                v(0.08, 0.18, 0.08),
                0.0,
                hex(0x4E5B2A),
            )
            .group(2),
        );
        out.push(
            Piece::new(
                Shape::Cube,
                p(x, y + r * 0.35, z + r * 0.6),
                v(0.4, 0.16, 0.05),
                0.0,
                haunt::candle(),
            )
            .glowing(0.8)
            .group(2),
        );
    }
    // 3: gnarled trees, tombstones, and the iron fence.
    for (i, (x, z, h)) in [
        (-11.0_f32, 6.0_f32, 7.0_f32),
        (11.5, 4.0, 8.0),
        (-14.0, -6.0, 9.0),
        (13.0, -8.0, 7.5),
    ]
    .into_iter()
    .enumerate()
    {
        let k = i as u32;
        out.push(
            Piece::new(
                Shape::Cylinder(6),
                p(x, 0.0, z),
                v(0.7, h, 0.7),
                0.0,
                haunt::night_teal(),
            )
            .group(3),
        );
        for b in 0..4u32 {
            let y = h * (0.45 + 0.13 * b as f32);
            let a = hash(k * 9 + b) * std::f32::consts::TAU;
            let bend = Mat4::from_translation(p(x, y, z))
                * Mat4::from_rotation_y(a)
                * Mat4::from_rotation_z(0.9 + 0.4 * hash(k * 9 + b + 3))
                * Mat4::from_scale(v(0.28, 2.6 + 1.5 * hash(k * 9 + b + 5), 0.28));
            out.push(Piece::placed(Shape::Cube, bend, haunt::night_teal()).group(3));
        }
    }
    for i in 0..6u32 {
        let x = -9.0 + 2.2 * (i % 3) as f32 + hash(i + 80) * 0.6;
        let z = 8.0 + 2.5 * (i / 3) as f32;
        let mut t = Piece::new(
            Shape::Cube,
            p(x, 0.0, z),
            v(0.9, 1.2 + 0.4 * hash(i + 81), 0.25),
            (hash(i + 82) - 0.5) * 0.4,
            haunt::dusk(),
        );
        t.at = lean(t.at, (hash(i + 83) - 0.5) * 0.25);
        out.push(t.group(3));
    }
    for i in 0..15 {
        let x = -14.0 + i as f32 * 2.0;
        if x.abs() < 1.6 {
            continue;
        }
        out.push(
            Piece::new(
                Shape::Cube,
                p(x, 0.0, 16.5),
                v(0.12, 1.7, 0.12),
                0.0,
                haunt::licorice(),
            )
            .group(3),
        );
    }
    for y in [0.5_f32, 1.4] {
        out.push(
            Piece::new(
                Shape::Cube,
                p(-8.0, y, 16.5),
                v(12.0, 0.08, 0.08),
                0.0,
                haunt::licorice(),
            )
            .group(3),
        );
        out.push(
            Piece::new(
                Shape::Cube,
                p(8.0, y, 16.5),
                v(12.0, 0.08, 0.08),
                0.0,
                haunt::licorice(),
            )
            .group(3),
        );
    }
    // 4: flat triangle hills far behind.
    for i in 0..7u32 {
        let x = -120.0 + i as f32 * 40.0 + hash(i + 90) * 15.0;
        let h = 22.0 + 20.0 * hash(i + 91);
        out.push(
            Piece::new(
                Shape::Pyramid,
                p(x, 0.0, -110.0 - 20.0 * hash(i + 92)),
                v(48.0, h, 18.0),
                0.0,
                haunt::plum(),
            )
            .group(4),
        );
    }
}

/// Every home plot's outline and corner posts on the Grid.
fn plot_outlines(mesh: &mut Mesh) {
    let half = PLOT.1;
    let dim = [0.55, 0.57, 0.62];
    for gx in -4..=4 {
        for gz in -4..=2 {
            let c = Vec3::new(gx as f32 * PLOT_PITCH, 0.03, gz as f32 * PLOT_PITCH);
            let corners = [
                c + Vec3::new(-half, 0.0, -half),
                c + Vec3::new(half, 0.0, -half),
                c + Vec3::new(half, 0.0, half),
                c + Vec3::new(-half, 0.0, half),
            ];
            for i in 0..4 {
                line(mesh, corners[i], corners[(i + 1) % 4], dim);
                line(mesh, corners[i], corners[i] + Vec3::Y * 1.2, dim);
            }
        }
    }
}

/// Other people's home plots round the creator's: small worlds of plain
/// shapes, and some still empty.
fn neighbor_plots(out: &mut Vec<Piece>) {
    for gx in -4..=4 {
        for gz in -4..=2 {
            if gx == 0 && gz == 0 {
                continue;
            }
            let seed = ((gx + 10) * 31 + gz + 10) as u32;
            if hash(seed) < 0.35 {
                continue;
            }
            let c = Vec3::new(gx as f32 * PLOT_PITCH, 0.0, gz as f32 * PLOT_PITCH);
            let n = 1 + (hash(seed + 1) * 4.0) as u32;
            for k in 0..n {
                let off = Vec3::new(
                    (hash(seed + k * 5 + 2) - 0.5) * 18.0,
                    0.0,
                    (hash(seed + k * 5 + 3) - 0.5) * 18.0,
                );
                let size = Vec3::new(
                    3.0 + 6.0 * hash(seed + k * 5 + 4),
                    2.0 + 9.0 * hash(seed + k * 5 + 5),
                    3.0 + 6.0 * hash(seed + k * 5 + 6),
                );
                let shape = match (seed + k) % 4 {
                    0 => Shape::Pyramid,
                    1 => Shape::Cylinder(8),
                    _ => Shape::Cube,
                };
                out.push(Piece::new(shape, c + off, size, hash(seed + k) * 1.5, EDGE).group(5));
            }
        }
    }
}

/// The Camping Adventure: tents round a fire by a lake, pines, and a
/// mountain range behind, each mountain capped with snow.
fn camp(out: &mut Vec<Piece>) {
    // The fire's ring of stones and logs.
    for i in 0..8u32 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        out.push(Piece::new(
            Shape::Sphere(6),
            Vec3::new(a.cos() * 0.8, -0.05, a.sin() * 0.8),
            Vec3::splat(0.32),
            a,
            hex(0x8A8790),
        ));
    }
    for (x, z, yaw) in [
        (-2.2_f32, 0.6_f32, 0.3_f32),
        (2.0, 1.0, -0.4),
        (0.3, -2.3, 1.5),
    ] {
        out.push(Piece::new(
            Shape::Cube,
            Vec3::new(x, 0.0, z),
            Vec3::new(1.8, 0.4, 0.45),
            yaw,
            hex(0x6B4A2E),
        ));
    }
    // Tents.
    for (x, z, yaw, color) in [
        (-5.0_f32, -3.5_f32, 0.5_f32, haunt::pumpkin()),
        (4.6, -4.4, -0.4, hex(0x3E7F8A)),
    ] {
        out.push(Piece::new(
            Shape::Roof,
            Vec3::new(x, 0.0, z),
            Vec3::new(3.0, 2.2, 3.6),
            yaw,
            color,
        ));
        out.push(Piece::new(
            Shape::Cube,
            Vec3::new(x, 0.0, z) + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 1.82,
            Vec3::new(0.7, 1.2, 0.04),
            yaw,
            haunt::licorice(),
        ));
    }
    // The lake.
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(6.0, -0.02, -26.0),
        Vec3::new(40.0, 0.04, 18.0),
        0.1,
        hex(0x3C6F9A),
    ));
    // Pines.
    for i in 0..44u32 {
        let a = hash(i + 200) * std::f32::consts::TAU;
        let r = 11.0 + 30.0 * hash(i + 201);
        let at = Vec3::new(a.cos() * r, 0.0, a.sin() * r * 0.8 - 6.0);
        if at.z < -16.0 && at.z > -36.0 && at.x > -14.0 && at.x < 26.0 {
            continue;
        }
        let h = 6.0 + 6.0 * hash(i + 202);
        out.push(Piece::new(
            Shape::Cylinder(5),
            at,
            Vec3::new(0.45, h * 0.3, 0.45),
            0.0,
            hex(0x5A3D27),
        ));
        for (k, y) in [0.25_f32, 0.5, 0.72].into_iter().enumerate() {
            let w = h * (0.55 - 0.13 * k as f32);
            out.push(Piece::new(
                Shape::Cone(7),
                at + Vec3::Y * h * y,
                Vec3::new(w, h * 0.42, w),
                a,
                hex(0x2F5A3A),
            ));
        }
    }
    // The mountain range, snow on each peak.
    for i in 0..8u32 {
        let x = -170.0 + i as f32 * 48.0 + hash(i + 300) * 20.0;
        let z = -120.0 - 50.0 * hash(i + 301);
        let h = 50.0 + 45.0 * hash(i + 302);
        let w = 70.0 + 30.0 * hash(i + 303);
        let yaw = hash(i + 304) * 0.8;
        out.push(Piece::new(
            Shape::Pyramid,
            Vec3::new(x, 0.0, z),
            Vec3::new(w, h, w * 0.8),
            yaw,
            hex(0x58607E),
        ));
        out.push(Piece::new(
            Shape::Pyramid,
            Vec3::new(x, h * 0.66, z),
            Vec3::new(w * 0.34, h * 0.34, w * 0.8 * 0.34),
            yaw,
            hex(0xEEF1F6),
        ));
    }
}

/// Blue Rush's main street in autumn, along -z: the theater and the
/// museum on the left, the shop and the legal hall on the right (the greco
/// kit's buildings, drawn from their footprints on the Grid layer), and the
/// power plant at the end.
fn street(out: &mut Vec<Piece>) {
    // The road, the curbs, and the sidewalks.
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(0.0, 0.0, -40.0),
        Vec3::new(12.0, 0.02, 150.0),
        0.0,
        hex(0x6E6258),
    ));
    for x in [-7.5_f32, 7.5] {
        out.push(Piece::new(
            Shape::Cube,
            Vec3::new(x, 0.0, -40.0),
            Vec3::new(3.0, 0.16, 150.0),
            0.0,
            hex(0xCBBBA0),
        ));
    }
    // The theater: a tall hall with a marquee and its bulbs.
    let theater = Vec3::new(-17.0, 0.0, 22.0);
    out.push(Piece::new(
        Shape::Cube,
        theater,
        Vec3::new(16.0, 13.0, 14.0),
        0.0,
        hex(0x7A2E35),
    ));
    out.push(Piece::new(
        Shape::Roof,
        theater + Vec3::Y * 13.0,
        Vec3::new(14.4, 3.0, 16.4),
        std::f32::consts::FRAC_PI_2,
        hex(0x4B1D23),
    ));
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(-8.6, 4.4, 22.0),
        Vec3::new(1.4, 1.6, 10.0),
        0.0,
        haunt::paper(),
    ));
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(-8.85, 6.2, 22.0),
        Vec3::new(0.6, 5.0, 1.4),
        0.0,
        haunt::mustard(),
    ));
    for i in 0..12 {
        out.push(
            Piece::new(
                Shape::Sphere(6),
                Vec3::new(-7.85, 4.25, 17.5 + i as f32 * 0.82),
                Vec3::splat(0.2),
                0.0,
                haunt::candle(),
            )
            .glowing(0.6),
        );
    }
    out.push(
        Piece::new(
            Shape::Cube,
            Vec3::new(-9.05, 0.0, 22.0),
            Vec3::new(0.1, 3.4, 4.0),
            0.0,
            haunt::candle(),
        )
        .glowing(0.7),
    );
    // The shop: a boutique with a striped awning, lit windows, and a
    // counter out front.
    let shop = Vec3::new(14.0, 0.0, 8.0);
    out.push(Piece::new(
        Shape::Cube,
        shop,
        Vec3::new(8.0, 7.5, 11.0),
        0.0,
        hex(0xE9DCC4),
    ));
    out.push(Piece::new(
        Shape::Roof,
        shop + Vec3::Y * 7.5,
        Vec3::new(11.4, 2.6, 8.4),
        std::f32::consts::FRAC_PI_2,
        haunt::brick(),
    ));
    for i in 0..6 {
        let color = if i % 2 == 0 {
            haunt::brick()
        } else {
            haunt::paper()
        };
        let z = 3.5 + i as f32 * 1.5;
        let m = Mat4::from_translation(Vec3::new(9.7, 3.4, z))
            * Mat4::from_rotation_z(-0.45)
            * Mat4::from_scale(Vec3::new(2.6, 0.08, 1.5));
        out.push(Piece::placed(Shape::Cube, m, color));
    }
    for z in [5.0_f32, 11.0] {
        out.push(
            Piece::new(
                Shape::Cube,
                Vec3::new(9.95, 0.6, z),
                Vec3::new(0.08, 2.0, 3.2),
                0.0,
                haunt::candle(),
            )
            .glowing(0.5),
        );
    }
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(9.95, 0.0, 8.0),
        Vec3::new(0.1, 2.6, 1.3),
        0.0,
        hex(0x3F2A20),
    ));
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(7.4, 0.0, 8.0),
        Vec3::new(1.0, 1.05, 3.2),
        0.0,
        hex(0x7B5234),
    ));
    for (dz, color) in [
        (-1.0_f32, haunt::pumpkin()),
        (0.0, haunt::mustard()),
        (1.0, hex(0xB0473A)),
    ] {
        out.push(Piece::new(
            Shape::Sphere(6),
            Vec3::new(7.4, 1.05, 8.0 + dz),
            Vec3::splat(0.32),
            dz,
            color,
        ));
    }
    // The power plant at the street's end: a cooling tower and a stack.
    out.push(Piece::new(
        Shape::Cylinder(14),
        Vec3::new(0.0, 0.0, -112.0),
        Vec3::new(18.0, 23.0, 18.0),
        0.0,
        hex(0xB8B2A6),
    ));
    out.push(Piece::new(
        Shape::Cube,
        Vec3::new(14.0, 0.0, -112.0),
        Vec3::new(8.0, 9.0, 12.0),
        0.0,
        hex(0x8D877C),
    ));
    out.push(Piece::new(
        Shape::Cylinder(8),
        Vec3::new(14.0, 9.0, -112.0),
        Vec3::new(2.2, 21.0, 2.2),
        0.0,
        hex(0x9E5A44),
    ));
    // Autumn trees and lamps along the sidewalks.
    for i in 0..14u32 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let z = 30.0 - (i / 2) as f32 * 14.0 - 5.0 * (i % 2) as f32;
        if side > 0.0 && (2.0..14.0).contains(&z) {
            continue;
        }
        let at = Vec3::new(side * 7.6, 0.16, z);
        let leaves = [
            haunt::pumpkin(),
            hex(0xC0392B),
            haunt::mustard(),
            hex(0xD9822B),
        ][(i % 4) as usize];
        out.push(Piece::new(
            Shape::Cylinder(6),
            at,
            Vec3::new(0.4, 3.2, 0.4),
            0.0,
            hex(0x5A3D27),
        ));
        out.push(Piece::new(
            Shape::Sphere(8),
            at + Vec3::Y * 2.6,
            Vec3::new(3.4, 3.2, 3.4),
            hash(i),
            leaves,
        ));
        let lamp = Vec3::new(side * 6.4, 0.16, z - 7.0);
        out.push(Piece::new(
            Shape::Cylinder(6),
            lamp,
            Vec3::new(0.16, 4.2, 0.16),
            0.0,
            haunt::licorice(),
        ));
        out.push(
            Piece::new(
                Shape::Cube,
                lamp + Vec3::Y * 4.2,
                Vec3::new(0.45, 0.55, 0.45),
                0.0,
                haunt::candle(),
            )
            .glowing(0.4),
        );
    }
}

/// The greco kit's buildings on the main street, as placed by
/// [`street_placements`].
fn street_kit() -> Result<TexturedScene, String> {
    let mut scene = TexturedScene::default();
    for (name, at, yaw) in street_placements() {
        // Everglade's copies, whose textures are PNG files beside them.
        let path = greco_dir().join(format!("../../everglade/generated/{name}.gltf"));
        let mesh = scene.import_gltf(&path)?;
        scene.place(
            mesh,
            Mat4::from_translation(at) * Mat4::from_rotation_y(yaw),
        );
    }
    scene.validate()?;
    Ok(scene)
}

fn greco_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated/greco")
}

/// Which greco buildings stand on the main street: the museum (the agora)
/// and the legal hall (the civic hall) on the left, and another shop (the
/// greco house) on the right, each facing the road.
#[must_use]
pub fn street_placements() -> [(&'static str, Vec3, f32); 3] {
    let left = std::f32::consts::FRAC_PI_2;
    [
        ("agora", Vec3::new(-9.2, 0.0, -10.0), left),
        ("civic_hall", Vec3::new(-9.2, 0.0, -50.0), left),
        ("greco_house", Vec3::new(9.2, 0.0, -24.0), -left),
    ]
}

/// The greco buildings as boxes from their footprints, for the Grid layer:
/// the same coordinates as the world layer's models.
pub fn street_footprints() -> Result<Vec<Piece>, String> {
    #[derive(Deserialize)]
    struct Footprint {
        boxes: Vec<Box3>,
    }
    #[derive(Deserialize)]
    struct Box3 {
        center: [f32; 3],
        half_extents: [f32; 3],
    }
    let mut out = Vec::new();
    for (name, at, yaw) in street_placements() {
        let path = greco_dir().join(format!("{name}.footprint.json"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let print: Footprint =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let place = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
        for b in print.boxes {
            let c = Vec3::from(b.center);
            let h = Vec3::from(b.half_extents);
            let m = place * Mat4::from_translation(c - Vec3::Y * h.y) * Mat4::from_scale(h * 2.0);
            out.push(Piece::placed(Shape::Cube, m, EDGE));
        }
    }
    Ok(out)
}

/// The main street's people: the shopkeeper behind her counter, and the
/// walkers' dog trotting beside the first of them.
fn street_people(mesh: &mut Mesh, t: f32, set: &Set) {
    let solid = set.spec.layer == Layer::World;
    let keeper = Figure {
        at: Vec3::new(8.6, 0.16, 8.0),
        yaw: -std::f32::consts::FRAC_PI_2,
        stride: t,
        moving: false,
        kind: Kind::Keeper,
    };
    let mut parts = Vec::new();
    keeper.pieces(&mut parts);
    if let Some(w) = set.spec.walkers.first() {
        let (pos, rot, speed) = w.pose(t);
        let f = rot * Vec3::Z;
        let side = Vec3::new(f.z, 0.0, -f.x) * 0.7;
        Figure {
            at: pos + side,
            yaw: f.x.atan2(f.z),
            stride: w.start + w.speed * t,
            moving: speed > 0.05,
            kind: Kind::Dog,
        }
        .pieces(&mut parts);
    }
    for p in &parts {
        draw(mesh, p, if solid { Look::Solid } else { Look::Ghost });
    }
}

/// The camp's storyteller by the fire.
fn camp_people(mesh: &mut Mesh, t: f32, set: &Set) {
    let solid = set.spec.layer == Layer::World;
    let mut parts = Vec::new();
    Figure {
        at: Vec3::new(1.9, 0.2, -1.6),
        yaw: -0.6,
        stride: t,
        moving: false,
        kind: Kind::Talker(5),
    }
    .pieces(&mut parts);
    for p in &parts {
        draw(mesh, p, if solid { Look::Solid } else { Look::Ghost });
    }
}

/// The galaxy: the OpenAgents planet, gridded, with the creator's plot
/// glowing on top; Blue Rush's planet with a main street on it; an Earth
/// twin; brand planets; threads of light between them; and stars.
fn galaxy(mesh: &mut Mesh, t: f32) {
    // The home planet: the Grid wrapped round a sphere.
    let home = Mat4::from_translation(HOME_CENTER - Vec3::Y * HOME_RADIUS)
        * Mat4::from_scale(Vec3::splat(HOME_RADIUS * 2.0));
    draw(
        mesh,
        &Piece::placed(Shape::Sphere(48), home, EDGE),
        Look::Grid,
    );
    // Plots on it, lit: the creator's haunt on top, and others round.
    let on_home = |dir: Vec3| -> Mat4 {
        let up = dir.normalize();
        let rot = glam::Quat::from_rotation_arc(Vec3::Y, up);
        Mat4::from_translation(HOME_CENTER + up * HOME_RADIUS) * Mat4::from_quat(rot)
    };
    let mut haunt_pieces = Vec::new();
    haunted_house(&mut haunt_pieces, Vec3::ZERO, 0.5);
    let top = on_home(Vec3::Y);
    lit_square(mesh, top, 7.0, haunt::moss());
    for p in haunt_pieces.iter().filter(|p| p.group < 4) {
        draw(
            mesh,
            &Piece {
                at: top * p.at,
                ..*p
            },
            Look::Solid,
        );
        if p.glow > 0.0 {
            mesh.sprites.push(feathers::dot(
                (top * p.at).transform_point3(Vec3::new(0.0, 0.5, 0.0)),
                0.4,
                p.color.map(|c| c * 1.5),
                1.0,
            ));
        }
    }
    for i in 0..26u32 {
        let dir = Vec3::new(
            hash(i + 500) - 0.5,
            0.25 + hash(i + 501),
            hash(i + 502) - 0.5,
        );
        let m = on_home(dir);
        let colors = [
            hex(0x7FB7E8),
            haunt::pumpkin(),
            hex(0x6A8D5B),
            haunt::mustard(),
            hex(0xC06C84),
            hex(0x9A86C8),
        ];
        lit_square(mesh, m, 3.0 + 3.0 * hash(i + 503), colors[(i % 6) as usize]);
        let h = 1.0 + 4.0 * hash(i + 504);
        draw(
            mesh,
            &Piece::placed(
                Shape::Cube,
                m * Mat4::from_scale(Vec3::new(2.0, h, 2.0)),
                colors[((i + 2) % 6) as usize],
            ),
            Look::Solid,
        );
    }
    // The other worlds.
    struct Planet {
        at: Vec3,
        r: f32,
        sea: [f32; 3],
        land: [f32; 3],
        land_share: f32,
        ring: bool,
        seed: u32,
    }
    let planets = [
        // Blue Rush Studios.
        Planet {
            at: Vec3::new(260.0, -20.0, -240.0),
            r: 48.0,
            sea: hex(0x2D5BD6),
            land: hex(0xE9DCC4),
            land_share: 0.3,
            ring: false,
            seed: 1,
        },
        // An Earth twin.
        Planet {
            at: Vec3::new(-250.0, 30.0, -300.0),
            r: 56.0,
            sea: hex(0x1F4F8F),
            land: hex(0x4F8A45),
            land_share: 0.42,
            ring: false,
            seed: 2,
        },
        Planet {
            at: Vec3::new(120.0, 110.0, -560.0),
            r: 34.0,
            sea: hex(0xE07A5F),
            land: hex(0xF2CC8F),
            land_share: 0.5,
            ring: true,
            seed: 3,
        },
        Planet {
            at: Vec3::new(-150.0, -90.0, -520.0),
            r: 28.0,
            sea: hex(0x6B4FA0),
            land: hex(0xC9B6E4),
            land_share: 0.4,
            ring: false,
            seed: 4,
        },
        Planet {
            at: Vec3::new(480.0, 60.0, -620.0),
            r: 40.0,
            sea: hex(0x2A9D8F),
            land: hex(0xE9C46A),
            land_share: 0.35,
            ring: false,
            seed: 5,
        },
        Planet {
            at: Vec3::new(-480.0, -10.0, -680.0),
            r: 36.0,
            sea: hex(0xB5838D),
            land: hex(0xFFCDB2),
            land_share: 0.5,
            ring: true,
            seed: 6,
        },
        Planet {
            at: Vec3::new(40.0, -160.0, -820.0),
            r: 30.0,
            sea: hex(0x3A5A40),
            land: hex(0xA3B18A),
            land_share: 0.5,
            ring: false,
            seed: 7,
        },
    ];
    for p in &planets {
        planet(mesh, p.at, p.r, p.sea, p.land, p.land_share, p.seed);
        if p.ring {
            let n = 64;
            for i in 0..n {
                let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                let tilt = glam::Quat::from_rotation_x(0.45);
                let q = |a: f32, r: f32| p.at + tilt * Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                let quad = [
                    q(a0, p.r * 1.5),
                    q(a1, p.r * 1.5),
                    q(a1, p.r * 2.0),
                    q(a0, p.r * 2.0),
                ];
                lit_polygon(&mut mesh.lit, &quad, p.land, 0.9);
                let back: Vec<Vec3> = quad.iter().rev().copied().collect();
                lit_polygon(&mut mesh.lit, &back, p.land, 0.9);
            }
        }
        // A thread of light home.
        let pulse = 0.7 + 0.3 * (t * 1.7 + p.seed as f32).sin();
        mesh.ribbons.extend(feathers::beam(
            HOME_CENTER,
            p.at,
            0.6,
            [0.9 * pulse, 0.85 * pulse, 0.7 * pulse],
        ));
    }
    // Blue Rush's main street on top of its planet.
    let br = &planets[0];
    let top = Mat4::from_translation(br.at + Vec3::Y * br.r);
    let mut street_pieces = Vec::new();
    street(&mut street_pieces);
    let shrink = Mat4::from_scale(Vec3::splat(0.22));
    for p in street_pieces
        .iter()
        .filter(|p| p.center().y > 0.2 || p.shape != Shape::Cube)
    {
        let at = top * shrink * p.at;
        let base = at.transform_point3(Vec3::ZERO) - br.at;
        if glam::Vec2::new(base.x, base.z).length() < br.r * 0.35 {
            draw(mesh, &Piece { at, ..*p }, Look::Solid);
        }
    }
    // Stars.
    for i in 0..1400u32 {
        let dir = Vec3::new(
            hash(i * 3 + 9001) - 0.5,
            hash(i * 3 + 9002) - 0.5,
            hash(i * 3 + 9003) - 0.5,
        )
        .normalize_or(Vec3::Y);
        let lum = 0.4 + 2.2 * hash(i + 9100).powi(3);
        let twinkle = 0.8 + 0.2 * (t * 2.0 + i as f32).sin();
        mesh.sprites.push(feathers::dot(
            dir * 3_500.0,
            2.4 + 3.0 * hash(i + 9200),
            [lum * twinkle, lum * twinkle, lum * 1.1 * twinkle],
            1.0,
        ));
    }
}

/// A lit square of ground `half` meters across, laid flat in `frame`.
fn lit_square(mesh: &mut Mesh, frame: Mat4, half: f32, color: [f32; 3]) {
    let c = |x: f32, z: f32| frame.transform_point3(Vec3::new(x, 0.02, z));
    lit_polygon(
        &mut mesh.lit,
        &[
            c(-half, half),
            c(half, half),
            c(half, -half),
            c(-half, -half),
        ],
        color,
        0.95,
    );
}

/// A low-poly planet: facets of sea and land, the land's share picked by
/// a few seeded continents.
fn planet(mesh: &mut Mesh, at: Vec3, r: f32, sea: [f32; 3], land: [f32; 3], share: f32, seed: u32) {
    let continents: Vec<Vec3> = (0..5)
        .map(|i| {
            Vec3::new(
                hash(seed * 40 + i * 3) - 0.5,
                hash(seed * 40 + i * 3 + 1) - 0.5,
                hash(seed * 40 + i * 3 + 2) - 0.5,
            )
            .normalize_or(Vec3::Y)
        })
        .collect();
    let m = Mat4::from_translation(at - Vec3::Y * r) * Mat4::from_scale(Vec3::splat(r * 2.0));
    for f in faces(Shape::Sphere(28), m) {
        let mid = (f.iter().copied().sum::<Vec3>() / f.len() as f32 - at).normalize_or(Vec3::Y);
        let near = continents
            .iter()
            .map(|c| c.dot(mid))
            .fold(-1.0_f32, f32::max);
        let color = if near > 1.0 - share * 1.4 { land } else { sea };
        lit_polygon(&mut mesh.lit, &f, color, 0.8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(world: World, layer: Layer) -> Spec {
        Spec {
            world,
            layer,
            reveal: None,
            build: None,
            summon: None,
            howl: None,
            portal: None,
            publish: None,
            sparks: None,
            walkers: Vec::new(),
        }
    }

    #[test]
    fn every_shape_faces_out_from_its_middle() {
        let at = Mat4::from_translation(Vec3::new(3.0, 1.0, -2.0))
            * Mat4::from_scale(Vec3::new(2.0, 3.0, 1.5));
        for shape in [
            Shape::Cube,
            Shape::Roof,
            Shape::Pyramid,
            Shape::Cylinder(8),
            Shape::Cone(7),
            Shape::Sphere(12),
        ] {
            let middle = at.transform_point3(Vec3::new(0.0, 0.45, 0.0));
            let fs = faces(shape, at);
            assert!(!fs.is_empty(), "{shape:?}");
            for f in fs {
                assert!(f.len() >= 3, "{shape:?}");
                let n = (f[1] - f[0]).cross(f[2] - f[0]);
                let c = f.iter().copied().sum::<Vec3>() / f.len() as f32;
                assert!(n.dot(c - middle) >= 0.0, "{shape:?} has a face turned in");
            }
        }
    }

    #[test]
    fn a_reveal_grows_from_its_center_to_its_reach() {
        let r = Reveal {
            center: [2.0, 3.0],
            at: 1.0,
            speed: 10.0,
            reach: 20.0,
        };
        assert_eq!(r.radius(0.5), 0.0);
        assert!(
            (r.radius(2.0) - 10.0).abs() < 1e-4,
            "halfway, eased symmetric"
        );
        assert_eq!(r.radius(10.0), 20.0);
        assert!(r.covers(Vec3::new(2.0, 0.0, 3.0), 1.5));
        assert!(!r.covers(Vec3::new(30.0, 0.0, 3.0), 10.0));
        assert_eq!(Reveal { speed: 0.0, ..r }.radius(5.0), 0.0);
    }

    #[test]
    fn a_build_raises_its_groups_in_turn() {
        let b = Build {
            at: 1.0,
            every: 0.5,
        };
        assert_eq!(b.height(0, 0.9), 0.0);
        assert_eq!(b.height(0, 1.0 + RISE), 1.0);
        assert_eq!(b.height(2, 1.9), 0.0);
        assert!(b.height(2, 2.0 + RISE * 0.5) > 0.0);
        let p = Piece::new(
            Shape::Cube,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 4.0, 1.0),
            0.0,
            EDGE,
        );
        let half = p.risen(0.5);
        assert!((half.at.transform_point3(Vec3::Y).y - 2.0).abs() < 1e-5);
        assert!(
            half.at.transform_point3(Vec3::ZERO).y.abs() < 1e-5,
            "it grows from its base"
        );
    }

    #[test]
    fn the_grid_layer_draws_edges_and_the_world_layer_draws_light() {
        let grid = Set::new(spec(World::Haunt, Layer::Grid)).unwrap().frame();
        assert!(grid.lit.is_empty(), "nothing is lit on the Grid");
        assert!(!grid.lines.is_empty() && !grid.faces.is_empty());
        assert!(grid.neon.unwrap().daylight.is_none(), "the Grid has no sky");
        let world = Set::new(spec(World::Haunt, Layer::World)).unwrap().frame();
        assert!(!world.lit.is_empty());
        assert!(
            world.faces.is_empty() && world.lines.is_empty(),
            "no Grid edges in the world"
        );
        assert!(world.neon.unwrap().daylight.is_some());
    }

    #[test]
    fn the_same_pieces_stand_in_both_layers() {
        // The astral layer's promise: the Grid and the world share
        // coordinates, so a toggle at one camera keeps everything in place.
        for world in [World::Haunt, World::Camp, World::Street, World::Plots] {
            let a = pieces(world);
            assert!(!a.is_empty(), "{world:?}");
            assert_eq!(a, pieces(world), "{world:?} is deterministic");
        }
    }

    #[test]
    fn a_reveal_turns_the_ground_and_pieces_inside_it_into_the_world() {
        let mut s = spec(World::Haunt, Layer::Grid);
        s.reveal = Some(Reveal {
            center: [0.0, 0.0],
            at: 0.0,
            speed: 8.0,
            reach: 40.0,
        });
        let mut set = Set::new(s).unwrap();
        set.advance(1.0 / 30.0, 0.0);
        let before = set.frame();
        assert!(before.lit.is_empty());
        set.advance(2.0, 2.0);
        let during = set.frame();
        assert!(!during.lit.is_empty(), "the near pieces are lit");
        assert!(!during.lines.is_empty(), "the far ones are still the Grid");
        let k = during.neon.unwrap().daylight.unwrap().horizon;
        assert!(
            k[0] > FIELD[0] && k[0] < hex(0xA9A3C4)[0],
            "the sky is coming in"
        );
        set.advance(10.0, 12.0);
        assert!(set.sky(12.0) > 0.999);
    }

    #[test]
    fn walkers_are_ghosts_on_the_grid_and_solid_in_the_world() {
        let walker = Walker {
            route: vec![[0.0, 30.0], [0.0, 60.0]],
            speed: 1.4,
            start: 0.0,
            looped: false,
            agent: false,
        };
        let mut s = spec(World::Void, Layer::Grid);
        s.walkers = vec![walker.clone()];
        let ghost = Set::new(s.clone()).unwrap().frame();
        assert!(ghost.lit.is_empty());
        assert!(ghost.lines.iter().any(|v| v.color == GHOST));
        s.layer = Layer::World;
        let solid = Set::new(s).unwrap().frame();
        assert!(!solid.lit.is_empty());
        assert!(!solid.lines.iter().any(|v| v.color == GHOST));
    }

    #[test]
    fn the_werewolf_is_summoned_solid_and_howls() {
        let mut s = spec(World::Haunt, Layer::World);
        s.summon = Some(1.0);
        s.howl = Some(1.5);
        let mut set = Set::new(s).unwrap();
        set.advance(0.1, 0.5);
        let early = set.frame();
        assert!(
            early.lines.iter().any(|v| v.color == GHOST),
            "a ghost before it is summoned"
        );
        set.advance(1.5, 2.0);
        let late = set.frame();
        assert!(!late.lines.iter().any(|v| v.color == GHOST), "solid after");
        let wolf = |howl| {
            let mut parts = Vec::new();
            Figure {
                at: Vec3::ZERO,
                yaw: 0.0,
                stride: 0.0,
                moving: false,
                kind: Kind::Wolf(howl),
            }
            .pieces(&mut parts);
            parts
        };
        // The head's snout points higher in the howl.
        let snout = |parts: &[Piece]| parts[7].center().y;
        assert!(snout(&wolf(1.0)) > snout(&wolf(0.0)));
    }

    #[test]
    fn the_street_kit_stands_where_its_footprints_say() {
        let boxes = street_footprints().unwrap();
        assert!(boxes.len() > 50);
        // Every footprint stands off the road, on its own side.
        for b in &boxes {
            assert!(b.center().x.abs() > 8.0, "{:?}", b.center());
        }
        assert!(street_kit().is_ok(), "the kit's models import");
    }

    #[test]
    fn the_galaxy_has_planets_threads_and_stars() {
        let set = Set::new(spec(World::Galaxy, Layer::World)).unwrap();
        let frame = set.frame();
        assert!(!frame.lit.is_empty());
        assert!(frame.ribbons.len() >= 7 * 3);
        assert!(frame.sprites.len() > 1000);
        assert!(!frame.lines.is_empty(), "the home planet is the Grid");
    }

    #[test]
    fn a_published_plot_lights_gold_and_raises_a_beam() {
        let mut s = spec(World::Plots, Layer::Grid);
        s.publish = Some(1.0);
        let mut set = Set::new(s).unwrap();
        set.advance(0.5, 0.5);
        assert!(set.frame().ribbons.is_empty());
        set.advance(1.5, 2.0);
        let f = set.frame();
        assert_eq!(f.ribbons.len(), 3);
        assert!(f.lines.iter().any(|v| v.color[0] > 2.0));
    }

    #[test]
    fn colors_come_in_linear_light() {
        assert_eq!(hex(0xFFFFFF), [1.0; 3]);
        assert_eq!(hex(0x000000), [0.0; 3]);
        assert!((hex(0x808080)[0] - 0.2195).abs() < 1e-3);
    }
}
