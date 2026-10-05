//! Standalone crypt lab, rendered through the zone's textured path.
//!
//! Usage: crypt_lab OUT_DIR [--models DIR] [--only NAME,...] [--no-assets]
//! [--no-scene] [--no-video]
//!
//! Loads the original models in `assets/verse/generated/chamber` (or
//! `--models DIR`) and writes `assets/<name>.png` for each model,
//! `scene.png` and one picture per station from inside the hall, and
//! `scene.mp4`, a walk from the door past the study, the storage, the
//! dissection slab, the brewing cauldrons, and the sarcophagus. The current
//! ritual chamber is not involved.
//!
//! The hall is closed and lit by its own sources: a lamp at every candle
//! cluster, cauldron, and brazier, which flickers, and moonlight through one
//! barred window in the far gable. Steam, bubbles, fire, dust motes, and low
//! fog run through the particle pipeline (`docs/verse/particles.md`).

use std::f32::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use verse::fx::{Particles, Spawn};
use verse::mesh::Mesh;
use verse::pbr::textured::TexturedScene;
use verse::pbr::{GlowVertex, Grade, HeightFog, Key, Lamp, LitVertex, MAX_LAMPS, Material, Neon};
use verse::render::{Offscreen, View};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FPS: f32 = 24.0;

const MODELS: &[&str] = &[
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

/// The hall's inner faces, glTF meters: walls at x = ±6 and z = ±8.6, the
/// alcoves' backs at x = ±6.55, the vault springing at 4.4 m.
const HALF_X: f32 = 6.0;
const HALF_Z: f32 = 8.6;
const NICHE_BACK: f32 = 6.55;
const SPRING: f32 = 4.4;
/// The barred window in the far gable, and the floor the moonlight reaches.
const WINDOW: Vec3 = Vec3::new(0.0, 5.8, -9.0);
const MOONLIT: Vec3 = Vec3::new(0.5, 0.0, -0.7);

struct Args {
    out: PathBuf,
    models: PathBuf,
    only: Option<Vec<String>>,
    shots: Vec<(String, Vec3, Vec3)>,
    assets: bool,
    scene: bool,
    video: bool,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let out = PathBuf::from(it.next().ok_or("Expected an output directory")?);
    let mut args = Args {
        out,
        models: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated/chamber"),
        only: None,
        shots: Vec::new(),
        assets: true,
        scene: true,
        video: true,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--models" => {
                args.models = PathBuf::from(it.next().ok_or("--models needs a directory")?)
            }
            "--only" => {
                let list = it.next().ok_or("--only needs names")?;
                args.only = Some(list.split(',').map(str::to_owned).collect());
            }
            "--shot" => {
                let spec = it.next().ok_or("--shot needs NAME,EX,EY,EZ,TX,TY,TZ")?;
                let mut parts = spec.split(',');
                let name = parts.next().unwrap_or("shot").to_owned();
                let v: Vec<f32> = parts.filter_map(|p| p.parse().ok()).collect();
                if v.len() != 6 {
                    return Err(format!("Bad shot {spec}"));
                }
                args.shots.push((
                    name,
                    Vec3::new(v[0], v[1], v[2]),
                    Vec3::new(v[3], v[4], v[5]),
                ));
            }
            "--no-assets" => args.assets = false,
            "--no-scene" => args.scene = false,
            "--no-video" => args.video = false,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(args)
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::fs::create_dir_all(args.out.join("assets")).map_err(|e| e.to_string())?;
    if args.assets {
        render_assets(&args)?;
    }
    if !args.scene && !args.video {
        return Ok(());
    }
    let lab = Lab::build(&args.models)?;
    let world = Mesh {
        lit: Vec::new(),
        textured: Some(Arc::new(lab.scene.clone())),
        ..Mesh::default()
    };
    let mut renderer = Offscreen::new(
        WIDTH,
        HEIGHT,
        &world,
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )?;
    let mut fx = lab.particles();
    // Let the steam, fog, and dust fill in before the first picture.
    let mut time = 0.0;
    while time < 10.0 {
        fx.tick(1.0 / FPS, |_, _| 0.0);
        time += 1.0 / FPS;
    }
    if args.scene {
        let custom: Vec<(&str, Vec3, Vec3)> = args
            .shots
            .iter()
            .map(|(n, e, t)| (n.as_str(), *e, *t))
            .collect();
        let shots: Vec<(&str, Vec3, Vec3)> = if custom.is_empty() {
            SHOTS.to_vec()
        } else {
            custom
        };
        for (name, eye, target) in &shots {
            let pixels = lab.frame(&mut renderer, &fx, time, *eye, *target)?;
            write_png(&args.out.join(format!("{name}.png")), &pixels)?;
            eprintln!("{name}");
        }
    }
    if args.video {
        let path = args.out.join("scene.mp4");
        encode_walk(&path, &lab, &mut renderer, &mut fx, time)?;
        eprintln!("video {}", path.display());
    }
    Ok(())
}

/// The scene pictures: a wide view from inside the door, then one per
/// station.
const SHOTS: &[(&str, Vec3, Vec3)] = &[
    (
        "scene",
        Vec3::new(2.5, 1.75, 5.0),
        Vec3::new(-1.9, 1.05, -4.2),
    ),
    (
        "station_brewing",
        Vec3::new(0.2, 1.8, -1.4),
        Vec3::new(-3.6, 0.8, -5.6),
    ),
    (
        "station_dissection",
        Vec3::new(0.6, 1.8, -0.4),
        Vec3::new(4.4, 0.8, -4.6),
    ),
    (
        "station_study",
        Vec3::new(0.2, 1.75, 5.4),
        Vec3::new(-4.4, 0.9, 1.2),
    ),
    (
        "station_storage",
        Vec3::new(0.6, 1.75, 2.2),
        Vec3::new(5.0, 0.7, 6.0),
    ),
    (
        "station_sarcophagus",
        Vec3::new(0.7, 1.6, -3.4),
        Vec3::new(-0.1, 1.0, -7.8),
    ),
];

/// The walk-through: the camera's keys, eye then target, at even times:
/// in at the door, the study, the storage, the dissection slab, the
/// cauldrons, and up to the sarcophagus.
const WALK: &[(Vec3, Vec3)] = &[
    (Vec3::new(0.0, 1.75, 8.0), Vec3::new(0.0, 1.3, 0.0)),
    (Vec3::new(-0.4, 1.75, 5.8), Vec3::new(-4.2, 1.0, 2.6)),
    (Vec3::new(-1.0, 1.7, 3.8), Vec3::new(-4.8, 0.9, 0.8)),
    (Vec3::new(0.2, 1.7, 3.4), Vec3::new(4.6, 0.8, 5.0)),
    (Vec3::new(1.2, 1.7, 1.6), Vec3::new(5.2, 0.8, 2.6)),
    (Vec3::new(0.7, 1.75, -0.6), Vec3::new(4.4, 0.9, -3.8)),
    (Vec3::new(0.8, 1.7, -2.4), Vec3::new(4.6, 0.9, -5.8)),
    (Vec3::new(0.3, 1.7, -2.6), Vec3::new(-3.6, 0.8, -4.4)),
    (Vec3::new(-0.6, 1.7, -3.2), Vec3::new(-3.8, 0.9, -7.2)),
    (Vec3::new(-0.3, 1.8, -3.8), Vec3::new(0.0, 1.2, -8.0)),
    (Vec3::new(0.0, 1.9, -4.6), Vec3::new(0.0, 1.0, -7.8)),
];
const WALK_SECONDS: f32 = 26.0;

struct Lab {
    scene: TexturedScene,
    lamps: Vec<Lamp>,
    flames: Vec<Vec3>,
    emitters: Vec<(&'static str, Vec3)>,
}

impl Lab {
    fn build(dir: &Path) -> Result<Self, String> {
        let mut scene = TexturedScene::default();
        let mut index = std::collections::BTreeMap::new();
        for name in MODELS {
            let mesh = scene.import_gltf(&dir.join(format!("{name}.glb")))?;
            index.insert(*name, mesh);
        }
        let mut lab = Lab {
            scene,
            lamps: Vec::new(),
            flames: Vec::new(),
            emitters: Vec::new(),
        };
        for &(name, x, y, z, yaw) in LAYOUT {
            lab.place(index[name], name, Vec3::new(x, y, z), yaw);
        }
        lab.light();
        Ok(lab)
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

    fn particles(&self) -> Particles {
        let mut fx = Particles::new(0x0c0f_fee5);
        for &(name, at) in &self.emitters {
            fx.start(name, Spawn::at(at));
        }
        for &flame in &self.flames {
            fx.start("candle_glow", Spawn::at(flame + Vec3::Y * 0.035));
        }
        fx
    }

    fn stage(&self, time: f32) -> Neon {
        let mut neon = Neon::neutral(time);
        neon.field = [0.006, 0.005, 0.0045];
        neon.fog_start = 4.0;
        neon.fog_end = 45.0;
        neon.bloom = 0.07;
        neon.vignette = 0.42;
        neon.height_fog = Some(HeightFog {
            density: 0.05,
            base: 0.0,
            falloff: 1.1,
            start: 1.5,
            max_opacity: 0.55,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        });
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
        for (i, lamp) in self.lamps.iter().take(MAX_LAMPS).enumerate() {
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

    fn frame(
        &self,
        renderer: &mut Offscreen,
        fx: &Particles,
        time: f32,
        eye: Vec3,
        target: Vec3,
    ) -> Result<Vec<u8>, String> {
        let mut dynamic = Mesh {
            neon: Some(self.stage(time)),
            glow: moonbeam(eye),
            ..Mesh::default()
        };
        fx.draw(&mut dynamic.sprites);
        renderer.render(view(eye, target), &dynamic, &verse::ui::UiBatch::default())
    }
}

/// Where each model stands: name, x, y, z (glTF meters), and yaw. A model
/// faces +Z at yaw 0; wall-hung models have their wall behind them.
const LAYOUT: &[(&str, f32, f32, f32, f32)] = &[
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

/// A soft, faint beam from the window to the moonlit floor: a few
/// additive quads turned toward the camera around the beam's axis.
fn moonbeam(eye: Vec3) -> Vec<GlowVertex> {
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
        let quad = [
            corner(p0, -1.0, -1.0),
            corner(p1, 1.0, -1.0),
            corner(p2, 1.0, 1.0),
            corner(p0, -1.0, -1.0),
            corner(p2, 1.0, 1.0),
            corner(p3, -1.0, 1.0),
        ];
        out.extend_from_slice(&quad);
    }
    out
}

fn view(eye: Vec3, target: Vec3) -> View {
    View {
        view_proj: Mat4::perspective_rh(0.9, WIDTH as f32 / HEIGHT as f32, 0.08, 120.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    }
}

/// One picture per model, on a dark stage under a studio key and a warm
/// lamp, with its flames and glowing liquids lit.
fn render_assets(args: &Args) -> Result<(), String> {
    for name in MODELS {
        if args
            .only
            .as_ref()
            .is_some_and(|only| !only.iter().any(|n| n == name))
        {
            continue;
        }
        let mut scene = TexturedScene::default();
        let mesh = scene.import_gltf(&args.models.join(format!("{name}.glb")))?;
        let (min, max) = bounds(&scene, mesh);
        let lift = if *name == "crypt_hall" { 0.0 } else { -min.y };
        scene.place(mesh, Mat4::from_translation(Vec3::new(0.0, lift, 0.0)));
        let center = (min + max) * 0.5 + Vec3::new(0.0, lift, 0.0);
        let reach = ((max - min).length() * 0.8).max(0.5);
        let (eye, target) = if *name == "crypt_hall" {
            (Vec3::new(-3.8, 2.6, 7.6), Vec3::new(1.2, 2.2, -5.0))
        } else {
            (
                center + Vec3::new(reach * 0.95, reach * 0.62, reach * 1.45),
                center - Vec3::Y * reach * 0.04,
            )
        };
        let world = Mesh {
            lit: stage(reach * 2.0),
            textured: Some(Arc::new(scene)),
            ..Mesh::default()
        };
        let mut renderer = Offscreen::new(
            WIDTH,
            HEIGHT,
            &world,
            &verse::ui::Atlas::new(16.0),
            verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
        )?;
        let hall = *name == "crypt_hall";
        let dynamic = Mesh {
            neon: Some(studio(center, reach, hall)),
            ..Mesh::default()
        };
        let pixels = renderer.render(
            View {
                view_proj: Mat4::perspective_rh(
                    if hall { 1.05 } else { 0.72 },
                    WIDTH as f32 / HEIGHT as f32,
                    0.05,
                    200.0,
                ) * Mat4::look_at_rh(eye, target, Vec3::Y),
                eye,
            },
            &dynamic,
            &verse::ui::UiBatch::default(),
        )?;
        write_png(
            &args.out.join("assets").join(format!("{name}.png")),
            &pixels,
        )?;
        eprintln!("asset {name}");
    }
    Ok(())
}

fn studio(center: Vec3, reach: f32, hall: bool) -> Neon {
    let mut neon = Neon::neutral(0.0);
    neon.field = [0.012, 0.011, 0.01];
    neon.fog_start = 200.0;
    neon.fog_end = 400.0;
    neon.bloom = 0.06;
    neon.vignette = 0.3;
    neon.key = Some(Key {
        dir: Vec3::new(0.45, 0.75, 0.5).normalize(),
        illuminance: if hall { 400.0 } else { 2_200.0 },
        angular_radius: 0.04,
        rim_dir: Vec3::new(-0.5, 0.35, -0.7).normalize(),
        rim_illuminance: if hall { 120.0 } else { 900.0 },
        rim_angular_radius: 0.12,
        sky: if hall { 160.0 } else { 260.0 },
        ground: 40.0,
        ev100: 8.0,
        shadow_center: center,
        shadow_half: reach * 2.0,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    if hall {
        // Work lights along the empty hall.
        for (i, z) in [-6.0, -2.0, 2.0, 6.0].into_iter().enumerate() {
            neon.lamps[i] = Lamp {
                position: Vec3::new(if i % 2 == 0 { -3.0 } else { 3.0 }, 2.0, z),
                color: [1.0, 0.62, 0.32],
                intensity: 3_000.0,
                range: 12.0,
            };
        }
    } else {
        neon.lamps[0] = Lamp {
            position: center + Vec3::new(-reach * 0.7, reach * 0.5, reach * 0.6),
            color: [1.0, 0.62, 0.32],
            intensity: 60.0 * reach * reach,
            range: reach * 4.0,
        };
    }
    neon
}

fn bounds(scene: &TexturedScene, mesh: usize) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for primitive in &scene.meshes[mesh].primitives {
        for vertex in &primitive.vertices {
            let p = Vec3::from(vertex.pos);
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

fn encode_walk(
    path: &Path,
    lab: &Lab,
    renderer: &mut Offscreen,
    fx: &mut Particles,
    start: f32,
) -> Result<(), String> {
    let frames = (WALK_SECONDS * FPS) as u32;
    let mut encoder = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{WIDTH}x{HEIGHT}"),
            "-framerate",
            &format!("{FPS}"),
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let mut pipe = encoder.stdin.take().ok_or("ffmpeg has no stdin")?;
    let mut time = start;
    for i in 0..frames {
        let t = i as f32 / (frames - 1) as f32;
        let (eye, target) = walk(t);
        let pixels = lab.frame(renderer, fx, time, eye, target)?;
        use std::io::Write;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        fx.tick(1.0 / FPS, |_, _| 0.0);
        time += 1.0 / FPS;
        if i % 48 == 0 {
            eprintln!("frame {i}/{frames}");
        }
    }
    drop(pipe);
    let status = encoder.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited {status}"))
    }
}

/// The camera at `t` from 0 to 1 along [`WALK`]: Catmull-Rom through the
/// keys, eased at both ends.
fn walk(t: f32) -> (Vec3, Vec3) {
    let t = t * t * (3.0 - 2.0 * t);
    let n = WALK.len() - 1;
    let x = t * n as f32;
    let i = (x.floor() as usize).min(n - 1);
    let f = x - i as f32;
    let key = |k: isize| WALK[k.clamp(0, n as isize) as usize];
    let i = i as isize;
    let [a, b, c, d] = [key(i - 1), key(i), key(i + 1), key(i + 2)];
    let spline = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3| {
        0.5 * ((2.0 * p1)
            + (p2 - p0) * f
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * f * f
            + (3.0 * p1 - p0 - 3.0 * p2 + p3) * f * f * f)
    };
    (spline(a.0, b.0, c.0, d.0), spline(a.1, b.1, c.1, d.1))
}

fn stage(half: f32) -> Vec<LitVertex> {
    let color = [0.09, 0.085, 0.08];
    let corner = |x: f32, z: f32| LitVertex {
        pos: [x * half, -0.002, z * half],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        local: [x * half, 0.0, z * half],
        color,
        params: [0.0, 0.9, Material::WhitePaint.code(), 1.0],
    };
    let [a, b, c, d] = [
        corner(-1.0, -1.0),
        corner(-1.0, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
    ];
    vec![a, b, c, a, c, d]
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
