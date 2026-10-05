//! The crypt lab: a closed, vaulted laboratory hall lit by candles,
//! braziers, glowing cauldrons, and moonlight through one barred window
//! (`docs/verse/zones.md`, Crypt).
//!
//! The hall and its 25 props are the original models
//! `scripts/blender/chamber_lab.py` writes to
//! `assets/verse/generated/chamber/`, placed by [`LAYOUT`]. [`Hall`] imports
//! them into one textured scene and finds the room's light sources: a lamp
//! at every candle cluster, cauldron, brazier, and the desk's crystal, which
//! flicker ([`Hall::stage`]). Steam, bubbles, fire, dust in the moonbeam,
//! and low fog run through the particle pipeline (`docs/verse/particles.md`).
//!
//! [`solids`] is what a character runs into and stands on: the walls,
//! pillars, props, and the dais, whose two steps are walkable. The vault
//! is a ceiling. The third-person camera stays in the room by the shared
//! camera-collision step over these solids (`verse::zones::sight`). The
//! heavy door at [`DOOR`] is the way out.
//!
//! The `crypt_lab` capture in `crates/verse/examples` renders the same hall
//! offline; `verse` re-exports this crate as `zones::crypt`.

use std::f32::consts::FRAC_PI_2;
use std::path::Path;

use glam::{Mat4, Vec3};
use verse_core::fx::{Particles, Spawn};
use verse_pbr::mesh::Mesh;
use verse_pbr::pbr::textured::TexturedScene;
use verse_pbr::pbr::{GlowVertex, Grade, HeightFog, Key, Lamp, MAX_LAMPS, Neon};
use verse_world::social::controller::Footprint;
use verse_world::social::solids::{Roof, Solids};

#[cfg(feature = "embedded")]
mod embedded;
#[cfg(test)]
mod tests;

/// The hall's inner faces, glTF meters: walls at x = ±6 and z = ±8.6, the
/// alcoves' backs at x = ±6.55, the vault springing at 4.4 m and rising
/// 2.7 m more at the crown.
pub const HALF_X: f32 = 6.0;
pub const HALF_Z: f32 = 8.6;
pub const NICHE_BACK: f32 = 6.55;
pub const SPRING: f32 = 4.4;
pub const RISE: f32 = 2.7;
/// The barred window in the far gable, and the floor the moonlight reaches.
pub const WINDOW: Vec3 = Vec3::new(0.0, 5.8, -9.0);
pub const MOONLIT: Vec3 = Vec3::new(0.5, 0.0, -0.7);
/// The zone's walkable square's half extent, m: the hall and its walls.
pub const HALF_EXTENT: f32 = 12.0;
/// Where the player arrives: just inside the door, facing the hall (-z).
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, 6.6);
pub const SPAWN_YAW: f32 = std::f32::consts::PI;
/// The middle of the heavy door's inner face, on the floor.
pub const DOOR: Vec3 = Vec3::new(0.0, 0.0, HALF_Z);
/// How near the door the player must stand to open it, m.
pub const DOOR_REACH: f32 = 1.8;
/// How far below the vault a character's feet stay, m: its height and a
/// little air.
const HEADROOM: f32 = 1.9;

/// The fog and clear color of the hall: near black, a little warm.
pub const FIELD: [f32; 3] = [0.006, 0.005, 0.0045];
pub const FOG_START: f32 = 4.0;
pub const FOG_END: f32 = 45.0;
/// Fog lying low over the floor.
pub const HEIGHT_FOG: HeightFog = HeightFog {
    density: 0.05,
    base: 0.0,
    falloff: 1.1,
    start: 1.5,
    max_opacity: 0.55,
    sun_strength: 0.0,
    sun_exponent: 1.0,
};

/// The models, by file stem, in `assets/verse/generated/chamber`.
pub const MODELS: &[&str] = &[
    "crypt_hall",
    "slab_table",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "ritual_rug",
    "specimen_jar",
    "specimen_jar_bones",
    "alchemy_bench",
    "bone_scatter",
    "cobweb",
    "bookshelf",
    "jar_shelf",
    "writing_desk",
    "lectern",
    "chained_skeleton",
    "hanging_chains",
    "brazier",
    "crate",
    "barrel",
    "iron_cage",
    "sarcophagus",
];

/// Where each model stands: name, x, y, z (glTF meters), and yaw. A model
/// faces +Z at yaw 0; wall-hung models have their wall behind them.
pub const LAYOUT: &[(&str, f32, f32, f32, f32)] = &[
    ("crypt_hall", 0.0, 0.0, 0.0, 0.0),
    // The sarcophagus on its dais between two tall candelabra.
    ("sarcophagus", 0.0, 0.36, -7.65, 0.0),
    ("candelabrum_tall", -1.95, 0.18, -6.45, 0.3),
    ("candelabrum_tall", 1.95, 0.18, -6.45, -0.4),
    ("bone_scatter", 1.45, 0.36, -8.25, 0.8),
    ("candelabrum_short", -0.85, 0.18, -6.38, 0.3),
    ("candelabrum_short", 0.9, 0.18, -6.42, -2.0),
    ("cobweb", -1.75, 3.6, -8.55, 0.0),
    // Brewing, the far west quarter: three cauldrons over their fires,
    // the jar shelf in the alcove behind them, the alchemy bench against
    // the far wall.
    ("cauldron_green", -3.5, 0.0, -3.4, 0.0),
    ("cauldron_amber", -1.9, 0.0, -5.0, 1.2),
    ("cauldron_red", -4.0, 0.0, -6.6, 2.0),
    ("jar_shelf", -(NICHE_BACK - 0.2), 0.0, -3.75, FRAC_PI_2),
    ("alchemy_bench", -4.3, 0.0, -8.17, 0.0),
    ("candelabrum_short", -4.95, 0.86, -8.0, 0.0),
    ("specimen_jar", -5.0, 0.0, -2.75, -0.3),
    ("specimen_jar", -5.25, 0.0, -4.95, 0.4),
    ("barrel", -5.4, 0.0, -7.75, 0.4),
    ("floor_candles", -2.0, 0.0, -2.4, 0.0),
    ("cobweb", -HALF_X, SPRING - 0.1, -HALF_Z, 0.0),
    // Dissection, the far east quarter: the slab along the wall, the
    // chained skeleton in its alcove, specimens, a cage, bones, and a
    // brazier.
    ("slab_table", 3.6, 0.0, -3.75, FRAC_PI_2),
    ("chained_skeleton", NICHE_BACK, 0.0, -3.75, -FRAC_PI_2),
    ("hanging_chains", HALF_X, 0.0, -2.58, -FRAC_PI_2),
    ("specimen_jar_bones", 5.15, 0.0, -2.85, -0.5),
    ("specimen_jar", 4.75, 0.0, -5.2, 0.8),
    ("iron_cage", 4.4, 0.0, -7.3, 0.3),
    ("brazier", 3.0, 0.0, -6.2, 0.6),
    ("bone_scatter", 2.95, 0.0, -7.65, 2.1),
    ("candelabrum_tall", 2.55, 0.0, -5.05, 0.5),
    ("bone_scatter", 4.9, 0.0, -0.9, 4.0),
    ("cobweb", HALF_X, SPRING - 0.1, -HALF_Z, 0.0),
    // The study, the near west quarter: bookshelves in two alcoves, the
    // desk and the lectern on a rug.
    ("ritual_rug", -3.5, 0.0, 2.1, 0.0),
    ("bookshelf", -(NICHE_BACK - 0.22), 0.0, 0.0, FRAC_PI_2),
    ("bookshelf", -(NICHE_BACK - 0.22), 0.0, 3.75, FRAC_PI_2),
    ("writing_desk", -3.9, 0.0, 1.45, FRAC_PI_2),
    ("lectern", -2.5, 0.0, 3.5, 1.1),
    ("candelabrum_tall", -4.5, 0.0, 5.5, 0.2),
    ("crate", -5.15, 0.0, 6.2, 0.3),
    ("crate", -5.25, 0.7, 6.2, -0.1),
    ("cobweb", -HALF_X, SPRING - 0.1, HALF_Z, 0.0),
    // Storage, the near east quarter: barrels and crates in the alcoves
    // and stacked by the door.
    ("jar_shelf", NICHE_BACK - 0.2, 0.0, 3.75, -FRAC_PI_2),
    ("barrel", 4.8, 0.0, 2.6, 0.0),
    ("barrel", 4.9, 0.0, 4.7, 1.3),
    ("specimen_jar", 4.75, 0.0, 3.45, 2.2),
    ("crate", 6.15, 0.0, -0.37, 0.1),
    ("crate", 6.15, 0.0, 0.37, -0.12),
    ("crate", 6.15, 0.7, 0.0, 0.25),
    ("candelabrum_short", 6.1, 1.4, 0.05, -1.2),
    ("crate", 4.9, 0.0, 7.85, 0.2),
    ("crate", 4.1, 0.0, 8.05, -0.3),
    ("crate", 4.5, 0.7, 7.95, 0.6),
    ("candelabrum_short", 4.5, 1.4, 7.95, 0.4),
    ("barrel", 5.35, 0.0, 6.8, 0.0),
    ("barrel", 3.25, 0.0, 8.15, 0.9),
    ("barrel", 4.6, 0.0, 6.15, 2.0),
    ("floor_candles", 3.6, 0.0, 5.0, 0.0),
    ("cobweb", HALF_X, SPRING - 0.1, HALF_Z, 0.0),
    // The middle of the hall, where the moonlight falls, and the door.
    ("ritual_rug", 0.0, 0.0, -0.6, FRAC_PI_2),
    ("floor_candles", -1.55, 0.0, -0.25, 0.0),
    ("floor_candles", 1.55, 0.0, -1.0, 0.0),
    ("floor_candles", -1.5, 0.0, 7.6, 0.0),
    ("candelabrum_tall", 1.8, 0.0, 7.7, -0.2),
];

/// Models that hang on the walls or lie flat, which nothing runs into.
const UNBLOCKING: &[&str] = &["cobweb", "ritual_rug"];

/// The pillar stations along each long wall, z, and their x.
pub const PILLARS_Z: [f32; 4] = [-5.6, -1.9, 1.9, 5.6];
pub const PILLAR_X: f32 = HALF_X - 0.3;

/// Whether this build carries the hall's models inside it.
pub const EMBEDDED: bool = cfg!(feature = "embedded");

/// The vault's inner surface height above `x`, m: an elliptical barrel
/// from the springing line at the walls to the crown.
#[must_use]
pub fn vault_height(x: f32) -> f32 {
    let c = (x / HALF_X).clamp(-1.0, 1.0);
    SPRING + RISE * (1.0 - c * c).max(0.0).sqrt()
}

/// The highest a character's feet may be at `x` under the vault, m.
#[must_use]
pub fn feet_ceiling(x: f32) -> f32 {
    vault_height(x) - HEADROOM
}

/// Whether a player standing at `at` is in reach of the door.
#[must_use]
pub fn near_door(at: Vec3) -> bool {
    let offset = at - DOOR;
    at.is_finite() && offset.x.hypot(offset.z) <= DOOR_REACH
}

/// The hall's static scene and its light sources.
pub struct Hall {
    pub scene: TexturedScene,
    /// The flickering point lights, brightest first, at most
    /// [`MAX_LAMPS`].
    pub lamps: Vec<Lamp>,
    /// Each candle flame's center, for its halo.
    pub flames: Vec<Vec3>,
    /// Each effect and where it plays.
    pub emitters: Vec<(&'static str, Vec3)>,
}

impl Hall {
    /// The hall from the models in `dir`, one `NAME.glb` per [`MODELS`]
    /// entry.
    ///
    /// # Errors
    ///
    /// Returns a message when a model is missing or cannot be imported.
    pub fn from_dir(dir: &Path) -> Result<Self, String> {
        Self::build(|scene, name| scene.import_gltf(&dir.join(format!("{name}.glb"))))
    }

    /// The hall from in-memory binary glTF files, by [`MODELS`] name.
    ///
    /// # Errors
    ///
    /// Returns a message when a model is missing or cannot be imported.
    pub fn from_glbs(models: &[(&str, &[u8])]) -> Result<Self, String> {
        Self::build(|scene, name| {
            let (_, bytes) = models
                .iter()
                .find(|(n, _)| *n == name)
                .ok_or_else(|| format!("The crypt has no model {name}"))?;
            scene.import_glb(&format!("{name}.glb"), bytes)
        })
    }

    /// The hall from the models built into this binary.
    ///
    /// # Errors
    ///
    /// Returns a message when this build carries no models, or one cannot
    /// be imported.
    pub fn embedded() -> Result<Self, String> {
        #[cfg(feature = "embedded")]
        {
            Self::from_glbs(embedded::MODELS)
        }
        #[cfg(not(feature = "embedded"))]
        {
            Err("This build does not carry the crypt's models".into())
        }
    }

    fn build(
        mut import: impl FnMut(&mut TexturedScene, &str) -> Result<usize, String>,
    ) -> Result<Self, String> {
        let mut scene = TexturedScene::default();
        let mut index = std::collections::BTreeMap::new();
        for name in MODELS {
            let mesh = import(&mut scene, name)?;
            index.insert(*name, mesh);
        }
        let mut hall = Self {
            scene,
            lamps: Vec::new(),
            flames: Vec::new(),
            emitters: Vec::new(),
        };
        for &(name, x, y, z, yaw) in LAYOUT {
            hall.place(index[name], name, Vec3::new(x, y, z), yaw);
        }
        hall.light();
        hall.scene.validate()?;
        Ok(hall)
    }

    /// Places a model and records its flames, from the brightest emissive
    /// primitives, for the halos and the candle lamps.
    fn place(&mut self, mesh: usize, name: &str, at: Vec3, yaw: f32) {
        let transform = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
        self.scene.place(mesh, transform);
        let mut flames: Vec<(Vec3, f32)> = Vec::new();
        for primitive in &self.scene.meshes[mesh].primitives {
            if self.scene.materials[primitive.material].emissive < 1_200.0 {
                continue;
            }
            for vertex in &primitive.vertices {
                let p = transform.transform_point3(Vec3::from(vertex.pos));
                match flames.iter_mut().find(|(c, _)| c.distance(p) < 0.06) {
                    Some((c, n)) => {
                        *c = (*c * *n + p) / (*n + 1.0);
                        *n += 1.0;
                    }
                    None => flames.push((p, 1.0)),
                }
            }
        }
        let centers: Vec<Vec3> = flames.into_iter().map(|(c, _)| c).collect();
        let warm = [1.0, 0.56, 0.24];
        match name {
            "cauldron_green" => self.cauldron(at, [0.25, 1.0, 0.35], "green"),
            "cauldron_red" => self.cauldron(at, [1.0, 0.25, 0.12], "red"),
            "cauldron_amber" => self.cauldron(at, [1.0, 0.62, 0.2], "amber"),
            "brazier" => {
                self.lamps.push(Lamp {
                    position: at + Vec3::new(0.0, 1.35, 0.0),
                    color: [1.0, 0.48, 0.18],
                    intensity: 24.0,
                    range: 7.0,
                });
                self.emitters.push(("brazier_fire", at + Vec3::Y * 1.02));
            }
            "writing_desk" => {
                // The crystal's own glow.
                self.lamps.push(Lamp {
                    position: at + Vec3::new(0.5, 0.95, -0.25),
                    color: [0.35, 0.55, 1.0],
                    intensity: 1.2,
                    range: 2.5,
                });
            }
            _ => {}
        }
        if !centers.is_empty() {
            let center = centers.iter().copied().sum::<Vec3>() / centers.len() as f32;
            self.lamps.push(Lamp {
                position: center + Vec3::Y * 0.08,
                color: warm,
                intensity: 3.0 * (centers.len() as f32).sqrt(),
                range: 5.0,
            });
        }
        self.flames.extend(centers);
    }

    fn cauldron(&mut self, at: Vec3, color: [f32; 3], kind: &'static str) {
        let surface = at + Vec3::Y * 0.9;
        self.lamps.push(Lamp {
            position: surface + Vec3::Y * 0.45,
            color,
            intensity: 7.0,
            range: 5.0,
        });
        // The fire beneath it.
        self.lamps.push(Lamp {
            position: at + Vec3::Y * 0.2,
            color: [1.0, 0.42, 0.14],
            intensity: 3.0,
            range: 3.0,
        });
        self.emitters.push(("crypt_steam", surface));
        self.emitters.push((
            match kind {
                "green" => "cauldron_bubbles_green",
                "red" => "cauldron_bubbles_red",
                _ => "cauldron_bubbles_amber",
            },
            surface,
        ));
    }

    /// Dust in the moonbeam, and fog low over the floor.
    fn light(&mut self) {
        for t in [0.35, 0.55, 0.75, 0.92] {
            self.emitters.push(("crypt_dust", WINDOW.lerp(MOONLIT, t)));
        }
        for (x, z) in [(-3.0, -4.0), (3.0, -4.0), (-3.0, 2.0), (3.0, 2.0)] {
            self.emitters.push(("crypt_fog", Vec3::new(x, 0.12, z)));
        }
        if self.lamps.len() > MAX_LAMPS {
            // Keep the brightest.
            self.lamps
                .sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
            self.lamps.truncate(MAX_LAMPS);
        }
    }

    /// The hall's effects, started: steam, bubbles, fire, dust, fog, and a
    /// halo at every flame.
    #[must_use]
    pub fn particles(&self) -> Particles {
        let mut fx = Particles::new(0x0c0f_fee5);
        for &(name, at) in &self.emitters {
            fx.start(name, Spawn::at(at));
        }
        for &flame in &self.flames {
            fx.start("candle_glow", Spawn::at(flame + Vec3::Y * 0.035));
        }
        fx
    }

    /// The lit stage at `time`, s: moonlight through the window as the
    /// key, the lamps flickering, low fog, and the hall's warm, contrasty
    /// grade.
    #[must_use]
    pub fn stage(&self, time: f32) -> Neon {
        stage(&self.lamps, time)
    }
}

/// The lit stage for `lamps` at `time`, s ([`Hall::stage`]).
#[must_use]
pub fn stage(lamps: &[Lamp], time: f32) -> Neon {
    let mut neon = Neon::neutral(time);
    neon.field = FIELD;
    neon.fog_start = FOG_START;
    neon.fog_end = FOG_END;
    neon.bloom = 0.07;
    neon.vignette = 0.42;
    neon.height_fog = Some(HEIGHT_FOG);
    let to_window = (WINDOW - MOONLIT).normalize();
    neon.key = Some(Key {
        dir: to_window,
        illuminance: 30.0,
        angular_radius: 0.01,
        rim_dir: Vec3::new(0.0, -1.0, 0.0),
        rim_illuminance: 0.0,
        rim_angular_radius: 0.1,
        sky: 0.25,
        ground: 0.08,
        ev100: 0.9,
        shadow_center: Vec3::ZERO,
        shadow_half: 14.0,
        shadow_distance: Some(26.0),
        cache_far_shadows: false,
    });
    for (i, lamp) in lamps.iter().take(MAX_LAMPS).enumerate() {
        neon.lamps[i] = lamp.flickering(time, i as u32);
    }
    neon.grade = Grade {
        exposure: 0.0,
        balance: Vec3::new(1.04, 0.98, 0.9),
        saturation: 0.86,
        contrast: 1.2,
        gain: Vec3::new(1.0, 0.96, 0.9),
        ..Grade::STAGE
    };
    neon
}

/// A soft, faint beam from the window to the moonlit floor: a few
/// additive quads turned toward the camera at `eye` around the beam's
/// axis.
#[must_use]
pub fn moonbeam(eye: Vec3) -> Vec<GlowVertex> {
    let mut out = Vec::new();
    let axis = (MOONLIT - WINDOW).normalize();
    for (k, (t0, t1, width)) in [(0.0, 0.62, 0.55), (0.3, 0.95, 0.7), (0.6, 1.08, 0.85)]
        .into_iter()
        .enumerate()
    {
        let a = WINDOW.lerp(MOONLIT, t0);
        let b = WINDOW.lerp(MOONLIT, t1);
        let mid = (a + b) * 0.5;
        let side = axis.cross(eye - mid).normalize_or(Vec3::X) * width;
        let radiance = [0.05, 0.062, 0.09].map(|c| c * (1.0 - 0.15 * k as f32));
        let corner = |p: Vec3, u: f32, v: f32| GlowVertex {
            pos: p.to_array(),
            radiance,
            uv: [u, v],
        };
        let [p0, p1, p2, p3] = [a - side, a + side, b + side, b - side];
        out.extend_from_slice(&[
            corner(p0, -1.0, -1.0),
            corner(p1, 1.0, -1.0),
            corner(p2, 1.0, 1.0),
            corner(p0, -1.0, -1.0),
            corner(p2, 1.0, 1.0),
            corner(p3, -1.0, 1.0),
        ]);
    }
    out
}

/// The live hall: its clock, its flickering lamps, and its effects.
pub struct Crypt {
    lamps: Vec<Lamp>,
    fx: Particles,
    time: f32,
}

impl Crypt {
    /// The hall's light and effects, already running for a few seconds so
    /// the steam, fog, and dust have filled in.
    #[must_use]
    pub fn new(hall: &Hall) -> Self {
        let mut crypt = Self {
            lamps: hall.lamps.clone(),
            fx: hall.particles(),
            time: 0.0,
        };
        for _ in 0..40 {
            crypt.tick(0.25);
        }
        crypt
    }

    /// Advances the clock and the effects by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.time = (self.time + dt) % 1000.0;
        self.fx.tick(dt, |_, _| 0.0);
    }

    /// This frame's lit stage, moonbeam, and particles, seen from `eye`.
    #[must_use]
    pub fn mesh(&self, eye: Vec3) -> Mesh {
        let mut mesh = Mesh {
            neon: Some(stage(&self.lamps, self.time)),
            glow: moonbeam(eye),
            ..Mesh::default()
        };
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    /// How many lamps light the hall.
    #[must_use]
    pub fn lamp_count(&self) -> usize {
        self.lamps.len().min(MAX_LAMPS)
    }
}

#[derive(serde::Deserialize)]
struct FootprintFile {
    boxes: Vec<BoxSpec>,
}

#[derive(serde::Deserialize)]
struct BoxSpec {
    name: String,
    center: [f32; 3],
    half_extents: [f32; 3],
}

/// Each model's collision boxes, glTF meters, as `chamber_lab.py` wrote
/// them beside it.
fn footprints(name: &str) -> Result<Vec<BoxSpec>, String> {
    let text = FOOTPRINTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, text)| *text)
        .ok_or_else(|| format!("The crypt has no footprint for {name}"))?;
    serde_json::from_str::<FootprintFile>(text)
        .map(|file| file.boxes)
        .map_err(|e| format!("{name}.footprint.json: {e}"))
}

macro_rules! footprint {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../../../assets/verse/generated/chamber/",
                $name,
                ".footprint.json"
            )),
        )
    };
}

const FOOTPRINTS: &[(&str, &str)] = &[
    footprint!("crypt_hall"),
    footprint!("slab_table"),
    footprint!("cauldron_green"),
    footprint!("cauldron_red"),
    footprint!("cauldron_amber"),
    footprint!("candelabrum_tall"),
    footprint!("candelabrum_short"),
    footprint!("floor_candles"),
    footprint!("ritual_rug"),
    footprint!("specimen_jar"),
    footprint!("specimen_jar_bones"),
    footprint!("alchemy_bench"),
    footprint!("bone_scatter"),
    footprint!("cobweb"),
    footprint!("bookshelf"),
    footprint!("jar_shelf"),
    footprint!("writing_desk"),
    footprint!("lectern"),
    footprint!("chained_skeleton"),
    footprint!("hanging_chains"),
    footprint!("brazier"),
    footprint!("crate"),
    footprint!("barrel"),
    footprint!("iron_cage"),
    footprint!("sarcophagus"),
];

/// The ground footprint of a box at `center` with `half` extents, turned
/// by `yaw` about the model's origin and placed at `at`.
fn ground_box(at: Vec3, yaw: f32, center: [f32; 3], half: [f32; 3]) -> Footprint {
    let turn = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let p = turn.transform_point3(Vec3::new(
            center[0] + sx * half[0],
            0.0,
            center[2] + sz * half[2],
        ));
        min = [min[0].min(p.x), min[1].min(p.z)];
        max = [max[0].max(p.x), max[1].max(p.z)];
    }
    Footprint { min, max }
}

/// What a character runs into and stands on in the hall, each a footprint
/// and its top, m: the walls (as tall as anything), the pillars, the
/// dais's two steps, and every standing prop.
///
/// # Errors
///
/// Returns a message when a footprint file does not parse.
pub fn blocks() -> Result<Vec<(Footprint, f32)>, String> {
    let mut out = Vec::new();
    for spec in footprints("crypt_hall")? {
        let footprint = ground_box(Vec3::ZERO, 0.0, spec.center, spec.half_extents);
        let top = spec.center[1] + spec.half_extents[1];
        if spec.name.starts_with("wall_face_") || spec.name.starts_with("wall_shell_") {
            out.push((footprint, f32::INFINITY));
        } else if spec.name.starts_with("pillar_plinth_") {
            // The whole pillar, up to the abacus the ribs spring from.
            out.push((footprint, SPRING + 0.01));
        } else if spec.name.starts_with("dais_") {
            out.push((footprint, top));
        }
    }
    for &(name, x, y, z, yaw) in LAYOUT {
        if name == "crypt_hall" || UNBLOCKING.contains(&name) {
            continue;
        }
        let at = Vec3::new(x, y, z);
        for spec in footprints(name)? {
            let top = y + spec.center[1] + spec.half_extents[1];
            out.push((ground_box(at, yaw, spec.center, spec.half_extents), top));
        }
    }
    Ok(out)
}

/// The hall's flat floor, m.
fn floor(_: f32, _: f32) -> f32 {
    0.0
}

/// The hall's solids: [`blocks`] over the flat floor, under the vault.
///
/// # Errors
///
/// Returns a message when a footprint file does not parse.
pub fn solids() -> Result<Solids, String> {
    let mut solids = Solids::over(floor);
    for (footprint, top) in blocks()? {
        solids.add_block(footprint, top);
    }
    // The vault as a ceiling a character rising into it strikes: a gable
    // inside the barrel, from the springing line to the crown.
    solids.add_roof(Roof {
        center: [0.0, 0.0],
        across: [1.0, 0.0],
        half: [HALF_X, HALF_Z],
        eave: SPRING,
        ridge: SPRING + RISE,
    });
    Ok(solids)
}
