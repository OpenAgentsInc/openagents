//! Any destructible assembly in the Meteor Showcase, as a film: a plain
//! binary glTF of solids and its spec (`scripts/blender/destructible_glb.py`
//! writes both from any glTF) stands on the lot between the kit houses in
//! the CAD rover's place (`WorldRuntime::add_meteor_assembly`), the caster
//! calls down the swarm, and a meteor lands on it: every solid a rigid
//! body, every touching pair bolted, the bolts failing under the blast
//! (`verse::zones::meteor_rover`). Simulated, not animated.
//!
//! Usage: meteor_assembly_capture ASSEMBLY.glb SPEC.json OUT.mp4
//! [--stills DIR] [--paint KEY=RRGGBB]...
//!
//! One continuous run at 1920 by 1080, 30 frames a second:
//!
//! 1. close round the assembly with the houses behind, as the swarm comes
//!    down;
//! 2. the strike, low from the side, at a quarter speed;
//! 3. the pieces flying and landing, wide;
//! 4. the wreck settling, time hurried, then held at real speed on every
//!    piece at rest.
//!
//! The film fails if any piece ends floating, sunk or still falling.
//! `--stills DIR` writes `<stem>-{establishing,strike,flight,rest}.png`.
//! Run it with `VERSE_QUALITY=high`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glam::Vec3;
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    ui::{Atlas, UiBatch},
    zones::{self, everglade_pack, meteor_rover::Livery, meteor_showcase as showcase},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: f32 = 30.0;
const DT: f32 = 1.0 / 120.0;

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}

struct Args {
    glb: PathBuf,
    spec: PathBuf,
    out: PathBuf,
    stills: Option<PathBuf>,
    paint: Vec<String>,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut positional = Vec::new();
    let (mut stills, mut paint) = (None, Vec::new());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--stills" => stills = Some(PathBuf::from(it.next().ok_or("--stills takes a dir")?)),
            "--paint" => paint.push(it.next().ok_or("--paint takes KEY=RRGGBB")?),
            _ if a.starts_with("--") => return Err(format!("Unknown argument {a}")),
            _ => positional.push(PathBuf::from(a)),
        }
    }
    let [glb, spec, out]: [PathBuf; 3] = positional
        .try_into()
        .map_err(|_| "Usage: meteor_assembly_capture ASSEMBLY.glb SPEC.json OUT.mp4")?;
    Ok(Args {
        glb,
        spec,
        out,
        stills,
        paint,
    })
}

/// The scene: the showcase, the assembly on the lot, the cast staged, the
/// light baked.
fn scene(pack: &everglade_pack::ZonePack, a: &Args) -> Result<WorldRuntime, String> {
    let glb = std::fs::read(&a.glb).map_err(|e| format!("{}: {e}", a.glb.display()))?;
    let spec =
        std::fs::read_to_string(&a.spec).map_err(|e| format!("{}: {e}", a.spec.display()))?;
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_showcase(pack);
    if runtime.zone != zones::ZoneId::MeteorShowcase {
        return Err("The Meteor Showcase did not install from the pinned pack".into());
    }
    runtime.set_spawn(Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
    runtime.add_meteor_assembly(&glb, &spec)?;
    if !a.paint.is_empty() {
        let livery = Livery::parse(&a.paint)?;
        if let Some(r) = runtime.meteor_rover_mut() {
            r.repaint(|instance, color| livery.paint(instance, color));
        }
    }
    if !runtime.stage_meteor_rover(showcase::DELAY) {
        return Err("The cast could not be staged".into());
    }
    eprintln!("baking the light");
    runtime.settle_zone_light();
    Ok(runtime)
}

/// The lot's directions: along the houses' front, and out from them.
fn lot_axes() -> (Vec3, Vec3) {
    let (sin, cos) = showcase::FACING.sin_cos();
    (Vec3::new(cos, 0.0, -sin), Vec3::new(sin, 0.0, cos))
}

/// The pieces' middle and spread (the farthest piece from the middle).
fn spread(runtime: &WorldRuntime) -> (Vec3, f32) {
    let Some(r) = runtime.meteor_rover() else {
        return (Vec3::ZERO, 1.0);
    };
    let at: Vec<Vec3> = r.bodies().iter().map(|b| b.2.as_vec3()).collect();
    let mut sorted: Vec<f32> = at.iter().map(|p| p.y).collect();
    sorted.sort_by(f32::total_cmp);
    let middle = at.iter().copied().sum::<Vec3>() / at.len().max(1) as f32;
    // The spread that holds nine in ten pieces, so a stray wheel far off
    // doesn't pull the camera away.
    let mut far: Vec<f32> = at
        .iter()
        .map(|p| Vec3::new(p.x - middle.x, 0.0, p.z - middle.z).length())
        .collect();
    far.sort_by(f32::total_cmp);
    (
        middle,
        far[(far.len() * 9 / 10).min(far.len() - 1)].max(2.0),
    )
}

fn main() -> Result<(), String> {
    let a = args()?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let atlas = Atlas::new(30.0);
    let idle = InputState::default();

    // A first run without filming: when the meteor strikes, and when the
    // last piece comes to rest.
    eprintln!("finding the strike");
    let mut runtime = scene(&pack, &a)?;
    let stood = runtime
        .meteor_rover()
        .map(|r| r.center())
        .ok_or("no assembly")?;
    let mut t = 0.0f32;
    let mut hit = None;
    let mut moving_until = 0.0f32;
    while t < 40.0 {
        runtime.tick(&idle, DT);
        t += DT;
        let r = runtime.meteor_rover().ok_or("no assembly")?;
        if hit.is_none() && !r.struck().is_empty() {
            hit = Some(r.hardest().map_or(t, |h| h.0 as f32));
        }
        if r.resting()
            .iter()
            .any(|(_, _, _, speed, held)| *speed > 0.05 || !held)
        {
            moving_until = t;
        }
        if hit.is_some_and(|h| t > h + 4.0 && t > moving_until + 2.0) {
            break;
        }
    }
    let hit = hit.ok_or("Nothing struck the assembly")?;
    let r = runtime.meteor_rover().ok_or("no assembly")?;
    for (k, name, height, speed, held) in r.resting() {
        if height < -zones::meteor_rover::REST_CONTACT || !held || speed > 0.05 {
            return Err(format!(
                "{name} ({k}) does not come to rest: {height:.3} m up, {speed:.3} m/s"
            ));
        }
    }
    eprintln!(
        "struck at {hit:.2} s; {} pieces, all at rest by {moving_until:.2} s",
        r.counts().0
    );

    // The film's clock: scene seconds for each film second.
    let slow_from = hit - 0.25;
    let slow_to = hit + 0.6;
    let fly_to = slow_to + 3.2;
    let settle_from = fly_to;
    let settle_to = (moving_until + 0.5).max(settle_from + 2.0);
    // (film time, scene time) keys.
    let lead = slow_from;
    let keys = [
        (0.0, 0.0),
        (lead, slow_from),
        (lead + (slow_to - slow_from) * 4.0, slow_to),
        (
            lead + (slow_to - slow_from) * 4.0 + (fly_to - slow_to),
            fly_to,
        ),
    ];
    let settle_film = 2.5;
    let hold = 2.5;
    let mut keys = keys.to_vec();
    let last = keys[keys.len() - 1].0;
    keys.push((last + settle_film, settle_to));
    keys.push((last + settle_film + hold, settle_to + hold));
    let length = keys[keys.len() - 1].0;
    let scene_at = |f: f32| -> f32 {
        for w in keys.windows(2) {
            if f <= w[1].0 {
                let u = ((f - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
                return w[0].1 + (w[1].1 - w[0].1) * u;
            }
        }
        keys[keys.len() - 1].1
    };
    let strike_film = keys[1].0;
    let flight_film = keys[2].0;
    let settle_film_at = keys[3].0;

    // The film, in one run.
    let mut runtime = scene(&pack, &a)?;
    let (along, out) = lot_axes();
    let ground = Vec3::new(stood.x, 0.0, stood.z);
    let camera = |f: f32, runtime: &WorldRuntime| -> (Vec3, Vec3) {
        if f < strike_film {
            // Round the assembly, close and high enough to clear the
            // lawn's shrubs, the houses behind.
            let u = ease(f / strike_film);
            let angle = -0.35 + 0.7 * u;
            let dir = out * angle.cos() + along * angle.sin();
            let eye = ground + dir * (8.0 - 1.5 * u) + Vec3::Y * (3.2 - 0.8 * u);
            (eye, ground + Vec3::Y * (1.5 + 1.2 * u))
        } else if f < flight_film {
            // Low beside it at the strike.
            let u = (f - strike_film) / (flight_film - strike_film);
            let eye = ground + out * 8.5 + along * (5.5 - 1.0 * u) + Vec3::Y * 1.0;
            (eye, ground + Vec3::Y * (1.5 + 1.5 * u))
        } else if f < settle_film_at {
            // Wide, the pieces flying.
            let (middle, reach) = spread(runtime);
            let u = ease((f - flight_film) / (settle_film_at - flight_film));
            let eye = ground + out * (14.0 + reach * 0.6) + along * 6.0 + Vec3::Y * (4.0 + 2.0 * u);
            (eye, middle.lerp(ground, 0.4) + Vec3::Y * (3.0 - 1.5 * u))
        } else {
            // The wreck at rest, low, drifting across where it stood.
            let (middle, reach) = spread(runtime);
            let middle = middle.lerp(ground, 0.5);
            let reach = reach.min(14.0);
            let u = ease((f - settle_film_at) / (length - settle_film_at));
            let eye = middle + out * (reach * 0.6 + 4.0) - along * (reach * 0.4 - reach * 0.8 * u)
                + Vec3::Y * 5.0;
            (eye, Vec3::new(middle.x, 0.2, middle.z))
        }
    };
    let (eye, target) = camera(0.0, &runtime);
    runtime.set_shot(Some((eye, target)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut film = Command::new("ffmpeg")
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
        .arg(format!("{WIDTH}x{HEIGHT}"))
        .args([
            "-r", "30", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p",
        ])
        .args(["-crf", "16", "-movflags", "+faststart"])
        .arg(&a.out)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))?;
    let frames = (length * FPS).round() as usize;
    let stem = a
        .out
        .file_stem()
        .map_or("assembly".into(), |s| s.to_string_lossy().to_string());
    let stills = [
        ((strike_film * 0.5 * FPS) as usize, "establishing"),
        (((strike_film + 0.12) * FPS) as usize, "strike"),
        (((flight_film + 1.0) * FPS) as usize, "flight"),
        (frames - 2, "rest"),
    ];
    let mut scene_t = 0.0f32;
    for k in 0..frames {
        let f = k as f32 / FPS;
        let want = scene_at(f);
        while scene_t + DT <= want + 1e-5 {
            runtime.tick(&idle, DT);
            scene_t += DT;
        }
        let (eye, target) = camera(f, &runtime);
        runtime.set_shot(Some((eye, target)));
        let view = runtime.view(aspect);
        let pixels = renderer.render(view, &runtime.dynamic_mesh(), &UiBatch::default())?;
        film.stdin
            .as_mut()
            .ok_or("ffmpeg has no input")?
            .write_all(&pixels)
            .map_err(|e| e.to_string())?;
        if let Some(dir) = &a.stills {
            for (at, name) in stills {
                if at == k {
                    write_png(&dir.join(format!("{stem}-{name}.png")), &pixels)?;
                }
            }
        }
        if k % 60 == 0 {
            eprintln!("frame {k}/{frames}");
        }
    }
    drop(film.stdin.take());
    let status = film.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed".into());
    }
    println!("wrote {} ({length:.1} s)", a.out.display());
    Ok(())
}
