//! A caster calls the Meteor Showcase's swarm down on the CAD Lab rover,
//! as one short film: the caster casting, the meteors raining down over
//! their shoulder onto the rover, and the hit in slow motion as the
//! rover's bolted joints fail and its parts are thrown, then the pieces
//! flying at full speed. The rover and the blast are simulated
//! (`verse::zones::meteor_rover`); the camera is the only thing staged.
//!
//! Usage: meteor_cast_capture OUT_DIR [--video PATH] [--caster-glb PATH]
//!        [--caster-lower PATH]
//!        [--paint KEY=RRGGBB]...
//!
//! - `--caster-glb PATH` draws a character on the Universal rig (a glb the
//!   film pipeline imports, as `characters::film_character` does) as the
//!   caster, playing the animation library's overhand throw as the cast
//!   goes up. Without it, the showcase's own caster casts.
//! - `--caster-lower PATH` adds an unskinned lower body built apart from
//!   the caster (legs, skirt, shoes; metres, feet at 0, facing +Z), bound
//!   to the rig's pelvis and legs by height.
//! - `--paint KEY=RRGGBB` repaints the rover: every solid whose CAD color
//!   is `KEY` (an sRGB hex such as `e8892f`), or whose instance is named
//!   `KEY` (such as `chassis`), takes the color `RRGGBB`. Repeatable; an
//!   instance name wins over a color.
//!
//! Renders 1920 by 1080 at 30 frames a second, about 13 s, to `PATH`
//! (`OUT_DIR/verse-meteor-cast.mp4` by default) through `ffmpeg`, with
//! the stills `verse-meteor-cast-{cast,hit,pieces}.png` in `OUT_DIR`.
//!
//! Run it with `VERSE_QUALITY=high`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use glam::Vec3;
use verse::{
    controller::InputState,
    imported::{characters, flat},
    pbr::textured::{Figure, TexturedScene},
    runtime::WorldRuntime,
    zones::{
        self,
        everglade::{self, demolition, player::Beast},
        everglade_pack::{self, Character, ZonePack},
        meteor_showcase as showcase,
    },
};
use verse_engine::assets::Pack;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: f32 = 30.0;
/// Simulation steps per frame at full speed.
const STEPS: usize = 2;
/// The slow-motion beat plays this many times slower.
const SLOW: f32 = 4.0;
/// The film's shots, in film seconds: the caster casting until `CAST`,
/// across the rover until `SLOW_AT`,
/// the hit slowed for `SLOW_FOR` of the scene's time, then the pieces at
/// full speed for `AFTER`.
const CAST: f32 = 4.2;
const SLOW_AT: f32 = 6.3;
const LEAD: f32 = 0.25;
const SLOW_FOR: f32 = 1.0;
const AFTER: f32 = 2.7;
/// The caster's cast clip's engine ID, and the library throw's it is made
/// from.
const CAST_CLIP: u16 = 902;
const THROW_CLIP: u16 = 903;
/// A separate lower body's heights, m from the feet: above `LEG_TOP` it
/// hangs from the pelvis, then the thighs to `KNEE`, the calves to
/// `ANKLE`, and the feet.
const LEG_TOP: f32 = 0.88;
const KNEE: f32 = 0.5;
const ANKLE: f32 = 0.11;
/// After the hit lands, how long its flash burns at full strength, s,
/// and how far round the rover its fire and smoke are then thinned, m.
const FLASH: f32 = 0.12;
const THIN_REACH: f32 = 12.0;
/// The caster's throw starts this long before the cast goes up, s.
const WIND_UP: f32 = 0.45;

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The character in `glb` (with the unskinned lower body in `lower`, when
/// it is built apart) as the pack form `film/caster`, with the player's
/// clips it has and the cast: the library's overhand throw in its right
/// arm over its idle.
fn caster_form(pack: &mut ZonePack, glb: &Path, lower: Option<&Path>) -> Result<Character, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let library = root.join("assets/verse/characters/quaternius");
    let dir = std::env::temp_dir().join(format!("verse-meteor-caster-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut engine = Pack {
        inventory: None,
        version: 1,
        source_revision: "meteor-caster".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    flat::white_texture(&mut engine, &dir)?;
    let mut model = characters::film_character(
        &mut engine,
        &dir,
        glb,
        &library,
        characters::ImportOptions {
            max_texture_edge: 1024,
        },
    )?;
    if let Some(lower) = lower {
        let legs = characters::import_with(
            &mut engine,
            &dir,
            lower,
            characters::ImportOptions {
                max_texture_edge: 1024,
            },
        )?;
        let skin = model.skin.as_ref().ok_or("The caster has no skin")?;
        let bone = |name: &str| {
            skin.names
                .iter()
                .position(|n| n == name)
                .map(|i| i as u32)
                .ok_or_else(|| format!("The caster's rig has no {name}"))
        };
        let pelvis = bone("pelvis")?;
        let leg = |side: &str| -> Result<[u32; 3], String> {
            Ok([
                bone(&format!("thigh_{side}"))?,
                bone(&format!("calf_{side}"))?,
                bone(&format!("foot_{side}"))?,
            ])
        };
        let (left, right) = (leg("l")?, leg("r")?);
        for mut surface in legs.surfaces {
            for v in &mut surface.vertices {
                let [x, y, _] = v.position;
                v.joints = [pelvis, 0, 0, 0];
                v.weights = [1.0, 0.0, 0.0, 0.0];
                // Below the hips everything follows the leg bones by height
                // and side (the body faces +Z, so its left is +X).
                if y > LEG_TOP {
                    continue;
                }
                let [thigh, calf, foot] = if x >= 0.0 { left } else { right };
                v.joints[0] = if y > KNEE {
                    thigh
                } else if y > ANKLE {
                    calf
                } else {
                    foot
                };
            }
            model.surfaces.push(surface);
        }
    }
    characters::retarget_clip(
        &mut model,
        &library.join("animations.glb"),
        THROW_CLIP,
        "OverhandThrow",
    )?;
    // The cast: the throw's right arm over the idle's body and legs, so
    // the caster stands their ground as they fling the swarm.
    let skin = model.skin.as_ref().ok_or("The caster has no skin")?;
    let arm = |bone: usize| {
        skin.names.get(bone).is_some_and(|n| {
            n.ends_with("_r")
                && [
                    "clavicle", "upperarm", "lowerarm", "hand", "index", "middle",
                ]
                .iter()
                .chain(&["pinky", "ring", "thumb"])
                .any(|p| n.starts_with(p))
        })
    };
    let throw = model
        .clips
        .iter()
        .find(|c| c.id == THROW_CLIP)
        .cloned()
        .ok_or("The throw did not retarget")?;
    let mut cast = model
        .clips
        .iter()
        .find(|c| c.id == 0)
        .cloned()
        .ok_or("The caster has no idle")?;
    cast.id = CAST_CLIP;
    cast.duration = throw.duration;
    cast.bones.retain(|b| !arm(b.bone));
    cast.bones
        .extend(throw.bones.iter().filter(|b| arm(b.bone)).cloned());
    model.clips.push(cast);
    let mut clips: Vec<(&str, u16)> = everglade_pack::compile::PLAYER_CLIPS
        .iter()
        .copied()
        .filter(|(_, id)| model.clips.iter().any(|c| c.id == *id))
        .collect();
    clips.push(("cast", CAST_CLIP));
    let form = everglade_pack::film::add_character(
        pack,
        &engine,
        &dir,
        &model,
        "film/caster",
        &clips,
        1024,
    )?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(form)
}

/// The caster's feet, and the rover's.
fn caster_at() -> Vec3 {
    let [x, z] = showcase::CASTER;
    Vec3::new(x, everglade::land(x, z), z)
}

fn rover_at() -> Vec3 {
    let [x, z] = showcase::rover_at();
    Vec3::new(x, everglade::land(x, z), z)
}

/// The level direction from the caster to the rover, and across it.
fn line() -> (Vec3, Vec3) {
    let d = rover_at() - caster_at();
    let away = Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X);
    (away, Vec3::new(-away.z, 0.0, away.x))
}

/// Where the rover's pieces are thrown (level), and the side the cameras
/// after the hit look from: across the throw, from the open lawn, as in
/// `meteor_rover_capture`.
fn throw_view() -> (Vec3, Vec3) {
    let (away, across) = line();
    let throw = (away * 0.27 - across * 0.96).normalize();
    let side = Vec3::new(-throw.z, 0.0, throw.x);
    let (sin, cos) = showcase::FACING.sin_cos();
    let out = Vec3::new(sin, 0.0, cos);
    let side = if side.dot(out) < 0.0 { -side } else { side };
    (throw, side)
}

/// The scene with the rover painted and the cast staged `delay` s in;
/// `own_caster` leaves the showcase's caster out for the film's.
fn scene(
    pack: &ZonePack,
    livery: &verse::zones::meteor_rover::Livery,
    delay: f32,
    own_caster: bool,
    bake: bool,
) -> Result<WorldRuntime, String> {
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_showcase(pack);
    if runtime.zone != zones::ZoneId::MeteorShowcase {
        return Err("The Meteor Showcase did not install from the pinned pack".into());
    }
    runtime.set_spawn(Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
    if !runtime.add_meteor_rover() || !runtime.stage_meteor_rover(delay) {
        return Err("The rover and the cast could not be staged".into());
    }
    if let Some(rover) = runtime.meteor_rover_mut() {
        rover.repaint(|instance, color| livery.paint(instance, color));
    }
    runtime.hide_meteor_caster(own_caster);
    if bake {
        eprintln!("baking the light");
        runtime.settle_zone_light();
    }
    Ok(runtime)
}

/// The film's camera at film time `t`: its eye and where it looks.
fn camera(t: f32) -> (Vec3, Vec3) {
    let c = caster_at();
    let r = rover_at();
    let (away, across) = line();
    if t < CAST {
        // In front of the caster, a little to their side, drawing back
        // and looking up as the meteors gather in the sky above them.
        let u = ease(t / CAST);
        let eye = c + away * (2.4 + 1.2 * u) + across * (1.1 + 0.3 * u) + Vec3::Y * 0.9;
        return (eye, c + Vec3::Y * (1.05 + 1.1 * u) - away * 0.2);
    }
    // Across the throw from the open lawn, close and low on the rover as
    // the meteors come down round it and through the slow hit, then
    // drawing back and up over where its pieces land.
    let (throw, side) = throw_view();
    let ground = Vec3::new(r.x, 0.0, r.z);
    let beat = SLOW_AT + SLOW_FOR * SLOW;
    let (along, out, up) = if t < SLOW_AT {
        let u = ease((t - CAST) / (SLOW_AT - CAST));
        (0.8, 5.2 - 0.7 * u, 1.3)
    } else if t < beat {
        let u = ease((t - SLOW_AT) / (beat - SLOW_AT));
        (0.8 + 1.7 * u, 4.5 + 2.5 * u, 1.3 + 1.1 * u)
    } else {
        let u = ease((t - beat) / AFTER);
        (2.5 + 4.5 * u, 7.0 + 9.0 * u, 2.4 + 3.6 * u)
    };
    let middle = ground + throw * along;
    (middle + side * out + Vec3::Y * up, middle + Vec3::Y * 0.45)
}

/// The fire, smoke and glow within `reach` m of `at` thinned to `keep`
/// of their opacity, so the pieces flying out of a blast show through it;
/// fire farther off is untouched.
fn thin_fire(mesh: &mut verse::mesh::Mesh, at: Vec3, reach: f32, keep: f32) {
    for sprite in &mut mesh.sprites {
        if sprite.at.distance(at) < reach {
            sprite.alpha *= keep;
            if sprite.additive > 0.0 {
                sprite.color = sprite.color.map(|c| c * keep);
            }
        }
    }
    if keep < 0.5 {
        mesh.ribbons.clear();
        for v in &mut mesh.glow {
            if Vec3::from(v.pos).distance(at) < reach {
                v.radiance = v.radiance.map(|c| c * keep);
            }
        }
    }
}

/// The caster, posed: their scene and this frame's vertices.
struct Caster {
    beast: Beast,
    scene: Arc<TexturedScene>,
    /// The zone's figure scene with the caster joined after it.
    joined: Option<(Arc<TexturedScene>, Arc<TexturedScene>)>,
}

impl Caster {
    fn new(pack: &ZonePack, form: &Character) -> Result<Self, String> {
        let beast = Beast::new(pack, form)?;
        let scene = beast.figure().scene.clone();
        Ok(Self {
            beast,
            scene,
            joined: None,
        })
    }

    /// Poses the caster `since` s after their throw starts (before it, at
    /// rest) and adds them to `figure`.
    fn draw(&mut self, figure: Option<Figure>, since: f32, dt: f32) -> Figure {
        let (at, yaw) = (caster_at(), {
            let (away, _) = line();
            away.x.atan2(away.z)
        });
        if since < 0.0 || !self.beast.pose_clip(at, yaw, 1.0, "cast", since, false, dt) {
            self.beast.pose_clip(at, yaw, 1.0, "idle", since, true, dt);
        }
        let posed = self.beast.vertices();
        let Some(cast) = figure else {
            return Figure {
                scene: self.scene.clone(),
                vertices: Arc::new(posed.to_vec()),
            };
        };
        if !self
            .joined
            .as_ref()
            .is_some_and(|(of, _)| Arc::ptr_eq(of, &cast.scene))
        {
            let joined = Arc::new(demolition::join(&cast.scene, &self.scene));
            self.joined = Some((cast.scene.clone(), joined));
        }
        let mut vertices = Vec::with_capacity(cast.vertices.len() + posed.len());
        vertices.extend_from_slice(&cast.vertices);
        vertices.extend_from_slice(posed);
        Figure {
            scene: self
                .joined
                .as_ref()
                .map(|j| j.1.clone())
                .unwrap_or(cast.scene),
            vertices: Arc::new(vertices),
        }
    }
}

fn encoder(path: &Path) -> Result<std::process::Child, String> {
    Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "rawvideo"])
        .args(["-pix_fmt", "rgba", "-s"])
        .arg(format!("{WIDTH}x{HEIGHT}"))
        .arg("-r")
        .arg(FPS.to_string())
        .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .args(["-crf", "16", "-movflags", "+faststart"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let mut video = out.join("verse-meteor-cast.mp4");
    let mut glb = None;
    let mut lower = None;
    let mut paint = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--video" => video = PathBuf::from(args.next().ok_or("--video needs a path")?),
            "--caster-glb" => {
                glb = Some(PathBuf::from(
                    args.next().ok_or("--caster-glb needs a path")?,
                ));
            }
            "--caster-lower" => {
                lower = Some(PathBuf::from(
                    args.next().ok_or("--caster-lower needs a path")?,
                ));
            }
            "--paint" => paint.push(args.next().ok_or("--paint needs KEY=RRGGBB")?),
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    let livery = verse::zones::meteor_rover::Livery::parse(&paint)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let mut pack = ZonePack::load_local(&pack)?;
    let form = match &glb {
        Some(glb) => Some(caster_form(&mut pack, glb, lower.as_deref())?),
        None => None,
    };
    let idle = InputState::default();
    let dt = 1.0 / (FPS * STEPS as f32);

    // A dry run, unlit, to find when the hardest meteor lands on the
    // rover; the scene is deterministic, so the film's cast is staged to
    // land it where the slow motion starts.
    let mut dry = scene(&pack, &livery, showcase::DELAY, form.is_some(), false)?;
    // Every meteor has landed well within this; the hardest of them on
    // the rover is the one to slow down for.
    let mut t = 0.0;
    while t < 12.0 {
        dry.tick(&idle, dt);
        t += dt;
    }
    let hit = dry
        .meteor_rover()
        .and_then(verse::zones::meteor_rover::Rover::hardest)
        .map(|h| h.0 as f32)
        .ok_or("No meteor landed on the rover")?;
    drop(dry);
    let delay = showcase::DELAY + (SLOW_AT + LEAD - hit);
    if delay < 0.2 {
        return Err(format!("The hit comes at {hit:.2} s, too late to stage"));
    }
    eprintln!("the hit lands {hit:.2} s after the cast; casting at {delay:.2} s");

    let mut runtime = scene(&pack, &livery, delay, form.is_some(), true)?;
    let mut caster = match &form {
        Some(form) => Some(Caster::new(&pack, form)?),
        None => None,
    };
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let (eye, target) = camera(0.0);
    runtime.set_shot(Some((eye, target)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let mut film = encoder(&video)?;
    let total = SLOW_AT + SLOW_FOR * SLOW + AFTER;
    let frames = (total * FPS).round() as usize;
    // The scene's clock runs at full speed but for the beat on the hit.
    let mut scene_t = 0.0_f32;
    let (mut cast_still, mut hit_still) = (false, false);
    let struck = hit_at(delay, hit);
    for k in 0..frames {
        let t = k as f32 / FPS;
        let slow = t >= SLOW_AT && t < SLOW_AT + SLOW_FOR * SLOW;
        if k > 0 {
            let step = if slow { dt / SLOW } else { dt };
            for _ in 0..STEPS {
                runtime.tick(&idle, step);
                scene_t += step;
            }
        }
        let (eye, target) = camera(t);
        runtime.set_shot(Some((eye, target)));
        let mut dynamic = runtime.dynamic_mesh();
        if let Some(caster) = &mut caster {
            let since = scene_t - (delay - WIND_UP);
            let step = if slow { dt / SLOW } else { dt } * STEPS as f32;
            dynamic.figure = Some(caster.draw(dynamic.figure.take(), since, step));
        }
        // The hit's flash at full strength, then its fire and smoke thinned
        // so the pieces show.
        let after = scene_t - struck;
        let keep = 1.0 - 0.92 * ((after - FLASH) / 0.25).clamp(0.0, 1.0);
        if keep < 1.0 {
            thin_fire(&mut dynamic, rover_at(), THIN_REACH, keep);
        }
        let view = runtime.view(aspect);
        let pixels = renderer.render(view, &dynamic, &ui)?;
        child_feed(&mut film, &pixels)?;
        if !cast_still && scene_t >= delay - 0.05 {
            write_png(&out.join("verse-meteor-cast-cast.png"), &pixels)?;
            cast_still = true;
        }
        if !hit_still && scene_t >= struck + 0.3 {
            write_png(&out.join("verse-meteor-cast-hit.png"), &pixels)?;
            hit_still = true;
        }
        if k == ((SLOW_AT + SLOW_FOR * SLOW + 0.35 * AFTER) * FPS) as usize {
            write_png(&out.join("verse-meteor-cast-pieces.png"), &pixels)?;
        }
        if k % 30 == 0 {
            eprintln!("{t:.1} s of film, {scene_t:.2} s of scene");
        }
    }
    drop(film.stdin.take());
    let status = film.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed".into());
    }
    if let Some(rover) = runtime.meteor_rover() {
        let (when, knock) = rover.hardest().unwrap_or_default();
        eprintln!(
            "hardest hit at {when:.2} s ({knock:.1} m/s); broke off: {:?}",
            rover.broken()
        );
    }
    eprintln!("wrote {}", video.display());
    Ok(())
}

/// When the hardest hit lands in the film's scene: `hit` s after a cast at
/// the showcase's delay, moved to a cast at `delay`.
fn hit_at(delay: f32, hit: f32) -> f32 {
    hit + delay - showcase::DELAY
}

fn child_feed(child: &mut std::process::Child, pixels: &[u8]) -> Result<(), String> {
    child
        .stdin
        .as_mut()
        .ok_or("ffmpeg has no input")?
        .write_all(pixels)
        .map_err(|e| e.to_string())
}
