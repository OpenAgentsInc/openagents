//! One in-world shot of the launch film, from a shot file (issue #11432).
//!
//! Usage: film_shot SHOT.json [--out PATH] [--stills DIR] [--every K]
//! [--subframes N] [--no-video] [--from SECONDS] [--to SECONDS]
//!
//! A shot file names a scene, a keyframed camera rig
//! ([`verse_engine::camera_rig`]), its length, frame rate, size, and how
//! many sub-frames each frame averages; see `docs/verse/film/verse-shots/`.
//! The scenes are the ritual chamber's cold open (`ritual`), the great
//! crypt (`crypt`), the Meteor Showcase (`meteor`), the druid's Grove
//! (`grove`), the Lagrange 1 station (`lagrange`), the Grid (`grid`),
//! the OpenAgents app's bare world drawn from its pinned engine pack as the
//! apps draw it, with its Everglade portal standing, and the Draconia
//! peacock guardian (`peacock`, `docs/verse/draconia/shots.md`).
//!
//! Each output frame is the mean of `subframes` renders spread over the
//! shutter (`shutter`, the fraction of the frame interval it is open, 0.5
//! by default), the simulation and the camera both advancing between
//! them, so moving things and camera moves blur as a film camera's do.
//! When the rig sets an f-number, each sub-frame also looks through a
//! different point on the lens with its focal plane held fixed
//! ([`verse_engine::camera_rig::lens_view_projection`]), so the mean has
//! true depth of field. The mean is taken in linear light. There is no HUD,
//! no nameplates, and no overlay text: the frame is the world alone.
//!
//! It writes H.264 (`libx264`, `yuv420p`, CRF `crf`, 12 by default) to
//! `~/work/film-out/verse/NAME.mp4` unless `--out` says otherwise.
//! `--stills DIR` also writes every `K`th frame (`--every`, 15 by default)
//! as a PNG; `--no-video` with `--every` renders only those frames, for
//! framing a shot quickly, and `--subframes` overrides the shot's count.
//!
//! Run it with `VERSE_QUALITY=high` for the high tier; the world scenes
//! keep temporal anti-aliasing on unless `VERSE_TAA=0`.

#[path = "film/peacock.rs"]
mod peacock;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use glam::{Vec2, Vec3};
use serde::Deserialize;
use verse::{
    controller::InputState,
    imported::{Renderer, chamber, play::Game},
    render::View,
    runtime::WorldRuntime,
    zones::{self, everglade_pack},
};
use verse_engine::camera_rig::{self, CameraRig, CameraSample};
use verse_engine::{director::Scene, source_position};

fn default_fps() -> f32 {
    30.0
}
fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_subframes() -> usize {
    8
}
fn default_shutter() -> f32 {
    0.5
}
fn default_crf() -> u8 {
    12
}
fn default_near() -> f32 {
    0.1
}
fn default_far() -> f32 {
    2000.0
}
fn default_prewarm() -> usize {
    24
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    name: String,
    scene: SceneSpec,
    /// Seconds of film.
    duration: f32,
    #[serde(default = "default_fps")]
    fps: f32,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_subframes")]
    subframes: usize,
    #[serde(default = "default_shutter")]
    shutter: f32,
    #[serde(default = "default_crf")]
    crf: u8,
    #[serde(default = "default_near")]
    near: f32,
    #[serde(default = "default_far")]
    far: f32,
    /// Renders of the first frame before the film starts, so exposure and
    /// temporal history settle.
    #[serde(default = "default_prewarm")]
    prewarm: usize,
    /// The town clock's hour for Everglade's scenes, such as 17.35 for
    /// golden hour.
    #[serde(default)]
    hour: Option<f64>,
    /// Strips the world's guide lines (the Lagrange station's reticles,
    /// outlines, and portal wireframe), which are interface, not scenery.
    #[serde(default)]
    hide_guides: bool,
    /// A light grade applied in linear light before encoding.
    #[serde(default)]
    grade: Grade,
    /// The camera, in film time.
    rig: CameraRig,
    /// Free notes on the shot; ignored.
    #[serde(default)]
    #[allow(dead_code)]
    notes: Option<String>,
}

/// Exposure in stops, a vignette's strength at the corners (0 for none),
/// and saturation (1 for none).
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct Grade {
    exposure: f32,
    vignette: f32,
    saturation: f32,
}

impl Default for Grade {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            vignette: 0.0,
            saturation: 1.0,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cast {
    at: f32,
    slot: u8,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SceneSpec {
    /// The ritual chamber before the cut; `start` is the scene time of the
    /// film's first frame.
    Ritual {
        #[serde(default)]
        start: f32,
    },
    /// The great crypt; the player walks down the stair and the nave for
    /// `walk` seconds while the acolytes chant.
    Crypt {
        #[serde(default)]
        walk: f32,
        #[serde(default)]
        start: f32,
    },
    /// The Meteor Showcase; the staged caster casts `cast_at` seconds into
    /// the film.
    Meteor {
        #[serde(default)]
        cast_at: f32,
        /// Film seconds the swarm has already run at the first frame.
        #[serde(default)]
        start: f32,
    },
    /// The Grove, with the druid at `spawn` facing `yaw`, casting the bar's
    /// slots at their times.
    Grove {
        spawn: [f32; 3],
        #[serde(default)]
        yaw: f32,
        #[serde(default)]
        casts: Vec<Cast>,
    },
    /// The Lagrange 1 station.
    Lagrange,
    /// The Grid, with the Everglade portal standing and nobody's avatar
    /// drawn, so the rig is the eye. `bob` walks the eye: a step's rise
    /// and fall and a half-rate sway, scaled by how fast the rig moves.
    Grid {
        #[serde(default)]
        bob: Option<Bob>,
    },
    /// The Draconia peacock guardian: the feathers of light in the void
    /// (`part` `feathers`, flashing white at `flash_at`), or Draconia
    /// before the fanned tail (`tail`), rising from a crouch at `rise`.
    Peacock {
        part: peacock::Part,
        #[serde(default)]
        flash_at: Option<f32>,
        #[serde(default)]
        rise: f32,
    },
}

/// A walking eye's motion on top of the rig.
#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bob {
    /// Steps a second.
    steps: f32,
    /// The rise and fall each step, m.
    rise: f32,
    /// The side-to-side sway over two steps, m.
    #[serde(default)]
    sway: f32,
    /// The rig speed, m/s, at which the bob is full; slower, it fades.
    walk: f32,
}

struct Args {
    shot: Shot,
    out: PathBuf,
    stills: Option<PathBuf>,
    every: usize,
    video: bool,
    from: f32,
    to: Option<f32>,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let path = PathBuf::from(it.next().ok_or("Expected a shot file")?);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let shot: Shot =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    shot.rig.validate()?;
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let mut args = Args {
        out: PathBuf::from(home)
            .join("work/film-out/verse")
            .join(format!("{}.mp4", shot.name)),
        shot,
        stills: None,
        every: 15,
        video: true,
        from: 0.0,
        to: None,
    };
    let number = |v: Option<String>, flag: &str| -> Result<f32, String> {
        v.and_then(|v| v.parse().ok())
            .ok_or(format!("{flag} takes a number"))
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--out" => args.out = PathBuf::from(it.next().ok_or("--out takes a path")?),
            "--stills" => {
                args.stills = Some(PathBuf::from(it.next().ok_or("--stills takes a dir")?))
            }
            "--every" => args.every = number(it.next(), "--every")?.max(1.0) as usize,
            "--subframes" => args.shot.subframes = number(it.next(), "--subframes")? as usize,
            "--from" => args.from = number(it.next(), "--from")?,
            "--to" => args.to = Some(number(it.next(), "--to")?),
            "--no-video" => args.video = false,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    let s = &args.shot;
    if !(s.duration > 0.0 && s.duration <= 120.0)
        || !(1.0..=120.0).contains(&s.fps)
        || !(1..=256).contains(&s.subframes)
        || !(0.0..=1.0).contains(&s.shutter)
        || s.width == 0
        || s.height == 0
        || s.width > 8192
        || s.height > 8192
        || s.crf > 51
        || !(s.near > 0.0 && s.far > s.near)
    {
        return Err("The shot's settings are out of range".into());
    }
    Ok(args)
}

/// The world as one of the two renderers draws it.
enum Stage {
    /// A zone of the world runtime, drawn by the shared physical renderer.
    World {
        runtime: Box<WorldRuntime>,
        renderer: Box<verse::render::Offscreen>,
        ui: verse::ui::UiBatch,
        /// Use the zone's own impact shake and keep the eye above ground.
        zone_camera: bool,
        hide_guides: bool,
    },
    /// The Grid, drawn by the imported renderer from its pinned pack.
    Grid {
        runtime: Box<WorldRuntime>,
        renderer: Box<Renderer>,
        ui: verse::ui::UiBatch,
        /// Keeps the zone cache that stands the Everglade portal.
        _cache: tempfile::TempDir,
    },
    /// The Draconia peacock guardian, drawn by the shared physical
    /// renderer on its own stage.
    Peacock {
        renderer: Box<verse::render::Offscreen>,
        set: Box<peacock::Set>,
        ui: verse::ui::UiBatch,
    },
    /// The ritual chamber or the great crypt, drawn by the imported
    /// renderer.
    Imported {
        renderer: Box<Renderer>,
        scene: Imported,
        ui: verse::ui::UiBatch,
        dir: PathBuf,
    },
}

enum Imported {
    /// The ritual before the cut, a function of the director's clock;
    /// `start` is its time at the film's first frame.
    Ritual {
        pack: Box<verse_engine::assets::Pack>,
        game: Box<Game>,
        start: f32,
    },
    /// The great crypt, simulated.
    Crypt {
        fight: Box<verse::imported::crypt_fight::Fight>,
        walk: f32,
    },
}

fn load_everglade() -> Result<everglade_pack::ZonePack, String> {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    everglade_pack::ZonePack::load_local(&pack)
}

/// The ritual chamber's pack and scene, as `verse_play` builds them.
fn ritual_pack(dir: &Path) -> Result<(verse_engine::assets::Pack, Scene), String> {
    let mut pack = verse::imported::original::generate(dir)?;
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let snapshot = dir.join("source-quaternius");
    verse::imported::inventory::snapshot_characters(
        &manifest.join("../../assets/verse/characters/quaternius"),
        &snapshot,
    )?;
    verse::imported::characters::install(&mut pack, dir, &snapshot, "male-ranger")?;
    pack.source_revision = "verse-universal-ritual-v1".into();
    let props = dir.join("source-fantasy-props");
    verse::imported::inventory::snapshot_props(
        &manifest.join("../../assets/verse/props/quaternius"),
        &props,
    )?;
    verse::imported::props::install(&mut pack, dir, &props)?;
    let mut scene = Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json"))?;
    let mut index = 0;
    for actor in &mut scene.actors {
        if actor.model == "cultist" {
            actor.model = [
                "cultist",
                "cultist-female",
                "cultist-peasant",
                "cultist-peasant-female",
            ][index % 4]
                .into();
            index += 1;
        }
    }
    verse::imported::inventory::compile(&mut pack, dir, None)?;
    Ok((pack, scene))
}

impl Stage {
    fn new(shot: &Shot) -> Result<Self, String> {
        let (w, h) = (shot.width, shot.height);
        match &shot.scene {
            SceneSpec::Ritual { start } => {
                let dir =
                    std::env::temp_dir().join(format!("verse-film-ritual-{}", std::process::id()));
                eprintln!("building the ritual chamber in {}", dir.display());
                let (pack, mut scene) = ritual_pack(&dir)?;
                // The rig is in film time; the director keeps scene time.
                let mut rig = shot.rig.clone();
                for key in &mut rig.keys {
                    key.t += start;
                }
                if let Some(shake) = &mut rig.shake {
                    for burst in &mut shake.bursts {
                        burst.at += start;
                    }
                }
                scene.camera = Some(rig);
                scene.validate()?;
                let mut game = Game::new(scene)?;
                verse::imported::props::admit_collision(&pack, &mut game)?;
                game.time = *start;
                let atlas = chamber::original_portrait_atlas(&dir, &pack)?;
                let renderer = Renderer::new(
                    pack.clone(),
                    &dir,
                    w,
                    h,
                    &atlas,
                    &chamber::static_instances(&pack, source_position(game.scene.origin)),
                )?;
                Ok(Self::Imported {
                    renderer: Box::new(renderer),
                    scene: Imported::Ritual {
                        pack: Box::new(pack),
                        game: Box::new(game),
                        start: *start,
                    },
                    ui: verse::ui::UiBatch::default(),
                    dir,
                })
            }
            SceneSpec::Crypt { walk, start } => {
                let dir =
                    std::env::temp_dir().join(format!("verse-film-crypt-{}", std::process::id()));
                eprintln!("building the great crypt in {}", dir.display());
                let mut fight = verse::imported::crypt_fight::Fight::load(&dir)?;
                let renderer = fight.renderer(w, h)?;
                // Let the chant run to `start` before the film opens.
                let mut t = 0.0;
                while t < *start {
                    fight.game.tick(1.0 / 30.0, [0.0; 2])?;
                    t += 1.0 / 30.0;
                }
                Ok(Self::Imported {
                    renderer: Box::new(renderer),
                    scene: Imported::Crypt {
                        fight: Box::new(fight),
                        walk: *walk,
                    },
                    ui: verse::ui::UiBatch::default(),
                    dir,
                })
            }
            SceneSpec::Meteor { cast_at, start } => {
                let pack = load_everglade()?;
                let mut runtime = Box::new(WorldRuntime::new());
                runtime.install_meteor_showcase(&pack);
                if runtime.zone != zones::ZoneId::MeteorShowcase {
                    return Err("The Meteor Showcase did not install".into());
                }
                runtime.set_spawn(Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
                if !runtime.stage_meteor_showcase(cast_at + start) {
                    return Err("The showcase's caster could not be staged".into());
                }
                eprintln!("baking the light");
                runtime.settle_zone_light();
                let idle = InputState::default();
                let steps = (start * 60.0).round() as usize;
                for _ in 0..steps {
                    runtime.tick(&idle, 1.0 / 60.0);
                }
                Self::world(runtime, w, h, true, shot.hide_guides)
            }
            SceneSpec::Grove { spawn, yaw, .. } => {
                let pack = load_everglade()?;
                let mut runtime = Box::new(WorldRuntime::new());
                if let Some(hour) = shot.hour {
                    runtime.set_town_clock(verse::town_clock::Clock::DAYTIME.pinned(Some(hour)));
                }
                runtime.install_grove(&pack);
                if runtime.zone != zones::ZoneId::Grove {
                    return Err("The Grove did not install".into());
                }
                if let Some(hour) = shot.hour {
                    runtime.set_town_clock(verse::town_clock::Clock::DAYTIME.pinned(Some(hour)));
                }
                runtime.set_spawn(Vec3::from(*spawn), *yaw)?;
                runtime.set_grove_film(true);
                eprintln!("baking the light");
                runtime.settle_zone_light();
                let idle = InputState::default();
                for _ in 0..30 {
                    runtime.tick(&idle, 1.0 / 60.0);
                }
                eprintln!("the druid stands at {}", runtime.player.pos);
                Self::world(runtime, w, h, true, shot.hide_guides)
            }
            SceneSpec::Grid { .. } => {
                // A spectator's Grid: the robots and arches, no avatar.
                let mut runtime = Box::new(WorldRuntime::unoccupied());
                let cache = tempfile::tempdir().map_err(|e| e.to_string())?;
                runtime.configure_zone_cache(cache.path().to_path_buf());
                let Some(gate) = runtime.everglade_gate() else {
                    return Err("The Grid's Everglade portal did not stand".into());
                };
                // For aiming: where the portal stands and which way it opens.
                eprintln!("everglade portal at {} yaw {}", gate.at, gate.yaw);
                let pack = verse::grid_pack::load_pinned()?;
                let statics = verse::grid_frame::statics(&pack);
                let atlas = verse::ui::Atlas::new(16.0);
                let renderer = Renderer::new(
                    pack,
                    &verse::grid_pack::pinned_dir(),
                    w,
                    h,
                    &atlas,
                    &statics,
                )?;
                Ok(Self::Grid {
                    runtime,
                    renderer: Box::new(renderer),
                    ui: verse::ui::UiBatch::default(),
                    _cache: cache,
                })
            }
            SceneSpec::Lagrange => {
                let mut runtime = Box::new(WorldRuntime::new());
                runtime.set_spawn(Vec3::new(12.0, 0.0, 9.0), 0.0)?;
                runtime.zone_intent(zones::Intent::Enter)?;
                let idle = InputState::default();
                for _ in 0..4 {
                    runtime.tick(&idle, 0.05);
                }
                runtime.settle_zone_light();
                if let Some(sky) = &runtime.dynamic_mesh().sky {
                    // For aiming: where the Earth and the Sun are.
                    eprintln!("earth {} sun {}", sky.earth.dir, sky.sun_dir);
                }
                Self::world(runtime, w, h, false, shot.hide_guides)
            }
            SceneSpec::Peacock {
                part,
                flash_at,
                rise,
            } => {
                let pack = if *part == peacock::Part::Tail {
                    Some(load_everglade()?)
                } else {
                    None
                };
                let set = peacock::Set::new(*part, *flash_at, *rise, pack.as_ref())?;
                let renderer = verse::render::Offscreen::new(
                    w,
                    h,
                    &peacock::Set::world(*part),
                    &verse::ui::Atlas::new(16.0),
                    zones::atmosphere(zones::ZoneId::Plaza),
                )?;
                eprintln!("quality tier: {}", renderer.quality().tier.name());
                Ok(Self::Peacock {
                    renderer: Box::new(renderer),
                    set: Box::new(set),
                    ui: verse::ui::UiBatch::default(),
                })
            }
        }
    }

    fn world(
        runtime: Box<WorldRuntime>,
        w: u32,
        h: u32,
        zone_camera: bool,
        hide_guides: bool,
    ) -> Result<Self, String> {
        let atlas = verse::ui::Atlas::new(16.0);
        let mut world = runtime.world.mesh.clone();
        if hide_guides {
            world.lines.clear();
        }
        let renderer = verse::render::Offscreen::new(w, h, &world, &atlas, runtime.atmosphere())?;
        eprintln!("quality tier: {}", renderer.quality().tier.name());
        Ok(Self::World {
            runtime,
            renderer: Box::new(renderer),
            ui: verse::ui::UiBatch::default(),
            zone_camera,
            hide_guides,
        })
    }

    /// Advances the scene by `dt` seconds of film time, `t` being the film
    /// time it reaches.
    fn advance(&mut self, shot: &Shot, dt: f32, t: f32) -> Result<(), String> {
        if dt <= 0.0 {
            return Ok(());
        }
        match self {
            Self::World { runtime, .. } => {
                let idle = InputState::default();
                let steps = (dt * 120.0).ceil().max(1.0) as usize;
                for _ in 0..steps {
                    runtime.tick(&idle, dt / steps as f32);
                }
                if let SceneSpec::Grove { casts, .. } = &shot.scene {
                    for cast in casts {
                        if cast.at > t - dt && cast.at <= t {
                            runtime.zone_intent(zones::Intent::GroveSlot(cast.slot))?;
                        }
                    }
                }
            }
            Self::Grid { runtime, .. } => {
                let steps = (dt * 120.0).ceil().max(1.0) as usize;
                for _ in 0..steps {
                    runtime.tick_unoccupied(dt / steps as f32);
                }
            }
            Self::Peacock { set, .. } => set.advance(dt, t),
            Self::Imported { scene, .. } => match scene {
                Imported::Ritual { game, start, .. } => game.time = (*start + t).max(0.0),
                Imported::Crypt { fight, walk } => {
                    let steps = (dt * 60.0).ceil().max(1.0) as usize;
                    for _ in 0..steps {
                        let movement = if t < *walk {
                            fight.game.yaw = 0.0;
                            [0.0, 1.0]
                        } else {
                            [0.0, 0.0]
                        };
                        fight.game.tick(dt / steps as f32, movement)?;
                    }
                }
            },
        }
        Ok(())
    }

    /// The camera at film time `t`: the rig's, or the director's for the
    /// ritual, whose scene holds the rig.
    fn camera(&self, shot: &Shot, t: f32) -> CameraSample {
        let rig = &shot.rig;
        let mut camera = rig.sample(t);
        if let SceneSpec::Grid { bob: Some(bob) } = &shot.scene {
            // How fast the rig walks here, from a short central difference.
            let h = 1.0 / 120.0;
            let ahead = rig.sample(t + h).eye - rig.sample((t - h).max(0.0)).eye;
            let speed = Vec3::new(ahead.x, 0.0, ahead.z).length() / (2.0 * h);
            let k = (speed / bob.walk).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let phase = std::f32::consts::TAU * bob.steps * t;
            // A step lifts the eye and sets it down: lowest at each footfall.
            let rise = bob.rise * k * (0.5 - 0.5 * phase.cos());
            let forward = (camera.target - camera.eye).with_y(0.0).normalize_or_zero();
            let side = Vec3::new(-forward.z, 0.0, forward.x);
            let sway = side * bob.sway * k * (phase * 0.5).sin();
            let lift = Vec3::Y * rise + sway;
            camera.eye += lift;
            // The head turns a little less than it moves.
            camera.target += lift * 0.6;
        }
        if let Self::Imported {
            scene: Imported::Ritual { game, .. },
            ..
        } = self
        {
            let frame = game.frame();
            camera.eye = frame.eye;
            camera.target = frame.target;
            camera.fov_y = frame.fov;
        }
        camera
    }

    /// Renders one sub-frame through `lens` and returns its sRGB RGBA bytes.
    fn render(
        &mut self,
        shot: &Shot,
        mut camera: CameraSample,
        lens: Vec2,
    ) -> Result<Vec<u8>, String> {
        let aspect = shot.width as f32 / shot.height as f32;
        match self {
            Self::World {
                runtime,
                renderer,
                ui,
                zone_camera,
                hide_guides,
            } => {
                if *zone_camera {
                    // The zone's own blast shake, and the eye kept above
                    // its ground, as the runtime's director camera has them.
                    runtime.set_shot(Some((camera.eye, camera.target)));
                    let moved = runtime.view(aspect).eye - camera.eye;
                    camera.eye += moved;
                    camera.target += moved * (1.0 + 0.04 * camera.eye.distance(camera.target));
                }
                let (view_proj, eye) =
                    camera_rig::lens_view_projection(&camera, aspect, shot.near, shot.far, lens);
                let mut dynamic = runtime.dynamic_mesh();
                if *hide_guides {
                    dynamic.lines.clear();
                }
                renderer.render(View { view_proj, eye }, &dynamic, ui)
            }
            Self::Grid {
                runtime,
                renderer,
                ui,
                ..
            } => {
                let (view_proj, eye) =
                    camera_rig::lens_view_projection(&camera, aspect, shot.near, shot.far, lens);
                let dynamic = verse::grid_frame::dynamic(runtime, &[], &[]);
                let lighting = verse::grid_frame::lighting(&runtime.atmosphere());
                renderer.draw(View { view_proj, eye }, &dynamic, ui, &lighting)
            }
            Self::Peacock { renderer, set, ui } => {
                let (view_proj, eye) =
                    camera_rig::lens_view_projection(&camera, aspect, shot.near, shot.far, lens);
                renderer.render(View { view_proj, eye }, &set.frame(), ui)
            }
            Self::Imported {
                renderer,
                scene,
                ui,
                ..
            } => {
                let (view_proj, eye) =
                    camera_rig::lens_view_projection(&camera, aspect, shot.near, shot.far, lens);
                let view = View { view_proj, eye };
                let (instances, lighting) = match scene {
                    Imported::Crypt { fight, .. } => {
                        let c = fight.compose(
                            1.0,
                            [shot.width, shot.height],
                            [1280.0, 720.0],
                            [-100.0; 2],
                            Some((camera.eye, camera.target)),
                        )?;
                        (c.instances, c.lighting)
                    }
                    Imported::Ritual { pack, game, .. } => {
                        let frame = game.frame();
                        let mut instances = chamber::instances(pack, &frame)?;
                        let visuals = verse_world::visuals::Combat::extract(game);
                        instances.extend(chamber::spell_instances_from_visuals(&visuals));
                        instances.extend(chamber::blocker_instances(pack, game));
                        instances.extend(chamber::environment_instances(pack, game));
                        instances.extend(chamber::prop_instances(pack, game, 1.0));
                        let lighting = chamber::lighting_from_visuals(
                            &visuals,
                            source_position(game.scene.origin),
                            false,
                            game.player,
                        );
                        (instances, lighting)
                    }
                };
                renderer.draw(view, &instances, ui, &lighting)
            }
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Self::Imported { dir, .. } = self {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| e.to_string())
}

fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// A small integer hash in 0..1, for dither.
fn dither(i: usize, frame: usize) -> f32 {
    let mut x = (i as u32) ^ (frame as u32).wrapping_mul(0x9E37_79B9);
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}

fn ffmpeg(shot: &Shot, out: &Path) -> Result<Child, String> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
        ])
        .arg(format!("{}x{}", shot.width, shot.height))
        .arg("-r")
        .arg(shot.fps.to_string())
        .args(["-i", "-", "-an", "-c:v", "libx264", "-preset", "slow"])
        .args(["-pix_fmt", "yuv420p", "-crf"])
        .arg(shot.crf.to_string())
        .args(["-movflags", "+faststart"])
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))
}

fn main() -> Result<(), String> {
    let args = args()?;
    let shot = &args.shot;
    if std::env::var("VERSE_QUALITY").as_deref() != Ok("high") {
        eprintln!("film_shot: set VERSE_QUALITY=high for the high tier");
    }
    // A film frame's cost is no matter: let every sprite and ribbon draw.
    verse::fx::set_budget(Some(16_384));
    if let Some(dir) = &args.stills {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let started = Instant::now();
    let mut stage = Stage::new(shot)?;
    eprintln!("staged in {:.1} s", started.elapsed().as_secs_f32());
    let frames = (shot.duration * shot.fps).round() as usize;
    let first = (args.from * shot.fps).round() as usize;
    let last = args
        .to
        .map_or(frames, |to| ((to * shot.fps).round() as usize).min(frames));
    let n = shot.subframes;
    let window = shot.shutter / shot.fps;
    // When each sub-frame of frame `k` is taken, film seconds.
    let at =
        |k: usize, i: usize| k as f32 / shot.fps + ((i as f32 + 0.5) / n as f32 - 0.5) * window;
    let mut clock = 0.0_f32.min(at(first, 0));
    stage.advance(shot, at(first, 0) - clock, at(first, 0))?;
    clock = at(first, 0);
    eprintln!("warming up");
    for _ in 0..shot.prewarm {
        stage.render(shot, stage.camera(shot, clock), Vec2::ZERO)?;
    }
    let mut encoder = if args.video {
        Some(ffmpeg(shot, &args.out)?)
    } else {
        None
    };
    let pixels_len = (shot.width * shot.height) as usize;
    let lut: Vec<f32> = (0..256).map(|v| linear(v as f32 / 255.0)).collect();
    let mut sum = vec![0.0_f32; pixels_len * 3];
    let mut out = vec![255_u8; pixels_len * 4];
    let render_started = Instant::now();
    let mut rendered = 0;
    for k in first..last {
        let wanted = encoder.is_some() || (args.stills.is_some() && k % args.every == 0);
        if !wanted {
            // Keep the simulation moving through frames nobody keeps.
            let next = at(k + 1, 0);
            stage.advance(shot, next - clock, next)?;
            clock = next;
            continue;
        }
        sum.fill(0.0);
        // Each frame turns the lens pattern, so its sampling varies frame
        // to frame instead of repeating the same few copies.
        let turn = k as f32 * 2.399_963_1 * 0.37;
        // The time samples go out in a scrambled order against the lens
        // samples, so motion and focus do not correlate.
        for i in 0..n {
            let t = at(k, i);
            stage.advance(shot, t - clock, t)?;
            clock = t;
            let camera = stage.camera(shot, t);
            let lens = if camera.aperture > 0.0 {
                camera_rig::lens_sample((i * 5 + 3) % n.max(1), n, turn)
            } else {
                Vec2::ZERO
            };
            let pixels = stage.render(shot, camera, lens)?;
            for (s, p) in sum.chunks_exact_mut(3).zip(pixels.chunks_exact(4)) {
                s[0] += lut[usize::from(p[0])];
                s[1] += lut[usize::from(p[1])];
                s[2] += lut[usize::from(p[2])];
            }
        }
        let scale = 2f32.powf(shot.grade.exposure) / n as f32;
        let (w, h) = (shot.width as usize, shot.height as usize);
        let half = Vec2::new(w as f32, h as f32) * 0.5;
        for (i, (o, s)) in out.chunks_exact_mut(4).zip(sum.chunks_exact(3)).enumerate() {
            let d = dither(i, k) - 0.5;
            // A cos^4-like falloff toward the corners.
            let p = (Vec2::new((i % w) as f32, (i / w) as f32) - half) / half.length();
            let fall = 1.0 - shot.grade.vignette * p.length_squared() * 2.0;
            let rgb = [s[0] * scale, s[1] * scale, s[2] * scale];
            let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
            for c in 0..3 {
                let graded =
                    (luma + (rgb[c] - luma) * shot.grade.saturation).max(0.0) * fall.max(0.0);
                let v = encode(graded.min(1.0)) * 255.0 + d;
                o[c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
        rendered += 1;
        if let Some(dir) = &args.stills
            && k % args.every == 0
        {
            write_png(
                &dir.join(format!("{}-{k:04}.png", shot.name)),
                shot.width,
                shot.height,
                &out,
            )?;
        }
        if let Some(child) = &mut encoder {
            child
                .stdin
                .as_mut()
                .ok_or("ffmpeg has no input")?
                .write_all(&out)
                .map_err(|e| format!("ffmpeg: {e}"))?;
        }
        if k % shot.fps as usize == 0 {
            eprintln!(
                "t {:.1} s · {:.0} ms a frame",
                k as f32 / shot.fps,
                render_started.elapsed().as_secs_f32() * 1000.0 / rendered as f32
            );
        }
    }
    if let Some(mut child) = encoder {
        drop(child.stdin.take());
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("ffmpeg failed: {status}"));
        }
        eprintln!("wrote {}", args.out.display());
    }
    eprintln!(
        "{} frames × {n} sub-frames in {:.1} s ({:.1} s with staging)",
        rendered,
        render_started.elapsed().as_secs_f32(),
        started.elapsed().as_secs_f32()
    );
    Ok(())
}
