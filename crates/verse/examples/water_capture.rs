//! Offline pictures and frame times of the Water Lab with the shared
//! renderer.
//!
//! Usage: water_capture OUT_DIR [--only NAME,...] [--frames N] [--size WxH]
//! [--sequence N] [--spells]
//!
//! Installs the lab as `verse --water-lab` does after the Everglade pack
//! loads, then renders fixed cinematic views: the cove at golden hour and
//! at noon, the shore's foam, the sun's glitter, floating bodies, a splash
//! frame by frame, the falls, the river, and the view under water. With
//! `--spells` it also casts each water spell and renders it into
//! `OUT_DIR/spells`. `--sequence N` writes N frames of a slow pan across
//! the bay into `OUT_DIR/sequence`. Finally it renders `N` frames (240 by
//! default) from the beach and prints the frame times; `VERSE_QUALITY`
//! (`low`, `medium`, `high`) picks the tier.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::{Mat4, Vec3};
use verse::render::{Offscreen, View};
use verse::runtime::WorldRuntime;
use verse::zones::{self, everglade_pack, water};

struct Args {
    out: PathBuf,
    only: Option<Vec<String>>,
    frames: usize,
    width: u32,
    height: u32,
    sequence: usize,
    spells: bool,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        out: PathBuf::from(it.next().ok_or("Expected an output directory")?),
        only: None,
        frames: 240,
        width: 1600,
        height: 900,
        sequence: 0,
        spells: false,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--only" => {
                a.only = Some(
                    it.next()
                        .ok_or("--only needs names")?
                        .split(',')
                        .map(str::to_owned)
                        .collect(),
                );
            }
            "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()).ok_or("--frames N")?,
            "--sequence" => {
                a.sequence = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--sequence N")?;
            }
            "--spells" => a.spells = true,
            "--size" => {
                let v = it.next().ok_or("--size WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.width = w.parse().map_err(|_| "bad width")?;
                a.height = h.parse().map_err(|_| "bad height")?;
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(a)
}

fn view(eye: Vec3, target: Vec3, fov: f32, aspect: f32) -> View {
    let proj = Mat4::perspective_rh(fov, aspect, 0.1, 2000.0);
    View {
        view_proj: proj * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    }
}

fn main() -> Result<(), String> {
    let a = args()?;
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let mut runtime = WorldRuntime::new();
    let started = Instant::now();
    runtime.install_water_lab(&pack);
    if runtime.zone != zones::ZoneId::WaterLab {
        return Err(format!(
            "The Water Lab did not install: {:?}",
            runtime.zone_snapshot(1.0).error
        ));
    }
    eprintln!(
        "installed in {:.0} ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let atlas = verse::ui::Atlas::new(16.0);
    let mut renderer = Offscreen::new(
        a.width,
        a.height,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(zones::ZoneId::WaterLab),
    )?;
    let aspect = a.width as f32 / a.height as f32;
    let ui = verse::ui::UiBatch::default();
    let wanted = |name: &str| a.only.as_ref().is_none_or(|o| o.iter().any(|n| n == name));
    let idle = verse::controller::InputState::default();
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / (1.0 / 60.0)).round() as usize {
            runtime.tick(&idle, 1.0 / 60.0);
        }
    };
    let ground = |x: f32, z: f32| water::ground(x, z);
    let shoot = |runtime: &mut WorldRuntime,
                 renderer: &mut Offscreen,
                 path: &Path,
                 eye: Vec3,
                 target: Vec3,
                 fov: f32|
     -> Result<(), String> {
        let pixels =
            renderer.render(view(eye, target, fov, aspect), &runtime.dynamic_mesh(), &ui)?;
        write_png(path, a.width, a.height, &pixels)?;
        eprintln!("wrote {}", path.display());
        Ok(())
    };
    let place = |runtime: &mut WorldRuntime, x: f32, z: f32, yaw: f32| {
        let _ = runtime.set_spawn(Vec3::new(x, ground(x, z), z), yaw);
    };
    // Let the sea and the floats settle.
    place(&mut runtime, 4.0, 16.0, PI);
    run(&mut runtime, 1.0);

    type Shot = (&'static str, Vec3, Vec3, f32, [f32; 3], bool);
    // Name, eye, target, vertical field of view, the player's x, z, yaw,
    // and noon.
    let shots: Vec<Shot> = vec![
        (
            "cove-golden",
            Vec3::new(30.0, 16.0, 38.0),
            Vec3::new(-6.0, 0.0, -30.0),
            0.9,
            [4.0, 12.0, PI],
            false,
        ),
        (
            "cove-noon",
            Vec3::new(30.0, 16.0, 38.0),
            Vec3::new(-6.0, 0.0, -30.0),
            0.9,
            [4.0, 12.0, PI],
            true,
        ),
        (
            "glint",
            Vec3::new(2.0, 2.2, 10.0),
            Vec3::new(-14.0, 0.0, -60.0),
            0.75,
            [3.0, 7.0, PI],
            false,
        ),
        (
            "shore-foam",
            Vec3::new(6.0, 2.4, 11.5),
            Vec3::new(-2.0, 0.0, -2.0),
            0.85,
            [9.0, 9.0, PI],
            false,
        ),
        (
            "shallows-noon",
            Vec3::new(8.0, 4.5, 8.0),
            Vec3::new(4.0, -0.6, -4.0),
            0.9,
            [10.0, 12.0, PI],
            true,
        ),
        (
            "reef-noon",
            Vec3::new(22.0, 9.0, -12.0),
            Vec3::new(16.0, -1.0, -30.0),
            0.85,
            [4.0, 12.0, PI],
            true,
        ),
        (
            "floats",
            Vec3::new(10.0, 3.2, 2.0),
            Vec3::new(2.0, 0.0, -10.0),
            0.8,
            [4.0, 12.0, PI],
            false,
        ),
        (
            "falls",
            Vec3::new(-30.0, 7.0, 36.0),
            Vec3::new(-46.0, 6.0, 53.0),
            0.95,
            [-38.0, 40.0, 0.6],
            false,
        ),
        (
            "falls-noon",
            Vec3::new(-30.0, 7.0, 36.0),
            Vec3::new(-46.0, 6.0, 53.0),
            0.95,
            [-38.0, 40.0, 0.6],
            true,
        ),
        (
            "river",
            Vec3::new(-28.0, 4.0, 16.0),
            Vec3::new(-38.0, 1.5, 30.0),
            0.9,
            [-30.0, 18.0, 0.0],
            false,
        ),
        (
            "plateau",
            Vec3::new(-50.0, 16.0, 66.0),
            Vec3::new(-42.0, 5.0, 40.0),
            0.9,
            [-50.0, 66.0, 0.0],
            false,
        ),
        (
            "headland",
            Vec3::new(-20.0, 3.0, -2.0),
            Vec3::new(-55.0, 2.0, -20.0),
            0.9,
            [-12.0, 2.0, 0.0],
            false,
        ),
    ];
    for (name, eye, target, fov, [px, pz, yaw], noon) in shots {
        if !wanted(name) {
            continue;
        }
        set_noon(&mut runtime, noon);
        place(&mut runtime, px, pz, yaw);
        run(&mut runtime, 0.3);
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join(format!("{name}.png")),
            eye,
            target,
            fov,
        )?;
    }
    set_noon(&mut runtime, false);

    if wanted("splash") {
        // A crate dropped from a few meters, frame by frame.
        place(&mut runtime, 6.0, 8.0, PI);
        run(&mut runtime, 0.3);
        if let Some(lab) = runtime.water_lab_mut() {
            lab.floats.spawn(
                water::FloatKind::Crate,
                Vec3::new(3.0, 3.6, -3.0),
                0.4,
                Vec3::new(0.0, -2.0, 0.0),
            );
        }
        for k in 0..18 {
            run(&mut runtime, 1.0 / 15.0);
            shoot(
                &mut runtime,
                &mut renderer,
                &a.out.join(format!("splash-{k:02}.png")),
                Vec3::new(8.0, 2.0, 4.0),
                Vec3::new(3.0, 0.6, -3.0),
                0.6,
            )?;
        }
    }

    if wanted("underwater") {
        // With Water Breathing, out past the reef, on the bed.
        runtime.water_press(4, false)?;
        place(&mut runtime, 10.0, -26.0, 0.0);
        run(&mut runtime, 0.5);
        let y = ground(10.0, -26.0);
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join("underwater.png"),
            Vec3::new(8.0, y + 1.6, -22.0),
            Vec3::new(16.0, y + 0.8, -31.0),
            1.0,
        )?;
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join("underwater-up.png"),
            Vec3::new(6.0, y + 1.0, -20.0),
            Vec3::new(10.0, 2.0, -28.0),
            1.1,
        )?;
        runtime.water_press(4, false)?;
    }

    if a.spells {
        spells(&a, &mut runtime, &mut renderer, aspect)?;
    }

    if a.sequence > 0 {
        let dir = a.out.join("sequence");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        place(&mut runtime, 4.0, 12.0, PI);
        for k in 0..a.sequence {
            run(&mut runtime, 1.0 / 24.0);
            let t = k as f32 / a.sequence.max(1) as f32;
            let eye = Vec3::new(26.0 - 30.0 * t, 5.0 + 2.0 * t, 22.0 - 6.0 * t);
            let target = Vec3::new(-10.0 + 8.0 * t, 0.0, -40.0);
            shoot(
                &mut runtime,
                &mut renderer,
                &dir.join(format!("{k:04}.png")),
                eye,
                target,
                0.85,
            )?;
        }
    }
    times(&mut runtime, &mut renderer, aspect, a.frames)
}

fn set_noon(runtime: &mut WorldRuntime, noon: bool) {
    if let Some(lab) = runtime.water_lab_mut() {
        let want = if noon {
            water::Hour::Noon
        } else {
            water::Hour::Golden
        };
        if lab.hour != want {
            lab.turn_hour();
        }
    }
}

/// Casts each spell and renders it into `OUT/spells`.
fn spells(
    a: &Args,
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
) -> Result<(), String> {
    let dir = a.out.join("spells");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let idle = verse::controller::InputState::default();
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / (1.0 / 60.0)).round() as usize {
            runtime.tick(&idle, 1.0 / 60.0);
        }
    };
    let ui = verse::ui::UiBatch::default();
    let mut shoot =
        |runtime: &mut WorldRuntime, name: &str, eye: Vec3, target: Vec3| -> Result<(), String> {
            let pixels =
                renderer.render(view(eye, target, 0.9, aspect), &runtime.dynamic_mesh(), &ui)?;
            let path = dir.join(format!("{name}.png"));
            write_png(&path, a.width, a.height, &pixels)?;
            eprintln!("wrote {}", path.display());
            Ok(())
        };
    let stand = |runtime: &mut WorldRuntime, x: f32, z: f32, yaw: f32| {
        let _ = runtime.set_spawn(Vec3::new(x, water::ground(x, z), z), yaw);
    };
    // Water Walk: out on the sea, the swell under the character's feet.
    stand(runtime, 2.0, 4.0, PI);
    runtime.water_press(0, false)?;
    let walk = verse::controller::InputState {
        forward: true,
        ..Default::default()
    };
    for _ in 0..240 {
        runtime.tick(&walk, 1.0 / 60.0);
    }
    let p = runtime.player.pos;
    shoot(
        runtime,
        "water-walk",
        p + Vec3::new(4.5, 2.2, 5.0),
        p + Vec3::new(0.0, 0.8, 0.0),
    )?;
    runtime.water_press(0, false)?;
    stand(runtime, 2.0, 3.0, PI);
    // Control Water, each mode in turn, cast toward the bay.
    for (k, name) in ["flood", "part-water", "redirect-flow", "whirlpool"]
        .iter()
        .enumerate()
    {
        runtime.water_press(1, false)?;
        run(runtime, if k == 0 { 9.0 } else { 6.0 });
        let (eye, target) = match k {
            0 => (Vec3::new(26.0, 7.0, 30.0), Vec3::new(-2.0, 0.0, 0.0)),
            1 => (Vec3::new(16.0, 10.0, 6.0), Vec3::new(2.0, -2.0, -12.0)),
            2 => (Vec3::new(16.0, 10.0, 6.0), Vec3::new(2.0, 0.0, -12.0)),
            _ => (Vec3::new(14.0, 12.0, 4.0), Vec3::new(2.0, -1.5, -11.0)),
        };
        if k == 2 || k == 3 {
            // Floats to show the current and the pull.
            if let Some(lab) = runtime.water_lab_mut() {
                for i in 0..4 {
                    lab.floats.spawn(
                        water::FloatKind::ALL[i % 3],
                        Vec3::new(-4.0 + i as f32 * 3.0, 1.0, -8.0 - i as f32),
                        i as f32,
                        Vec3::ZERO,
                    );
                }
            }
            run(runtime, 3.0);
        }
        shoot(runtime, name, eye, target)?;
    }
    runtime.water_press(1, true)?;
    run(runtime, 10.0);
    // Create Water: rain on the beach and the shallows; then Destroy.
    stand(runtime, 6.0, 10.0, PI);
    runtime.water_press(2, false)?;
    run(runtime, 5.0);
    shoot(
        runtime,
        "create-water",
        Vec3::new(14.0, 4.0, 14.0),
        Vec3::new(5.0, 0.0, 3.0),
    )?;
    runtime.water_press(2, true)?;
    run(runtime, 0.8);
    shoot(
        runtime,
        "destroy-water",
        Vec3::new(14.0, 4.0, 14.0),
        Vec3::new(5.0, 0.0, 3.0),
    )?;
    run(runtime, 6.0);
    // Sleet Storm over the bay.
    stand(runtime, 2.0, 10.0, PI);
    runtime.water_press(3, false)?;
    run(runtime, 5.0);
    shoot(
        runtime,
        "sleet-storm",
        Vec3::new(18.0, 8.0, 16.0),
        Vec3::new(2.0, 0.0, -4.0),
    )?;
    runtime.water_press(3, false)?;
    run(runtime, 2.5);
    shoot(
        runtime,
        "sleet-thaw",
        Vec3::new(18.0, 8.0, 16.0),
        Vec3::new(2.0, 0.0, -4.0),
    )?;
    run(runtime, 4.0);
    // Water Breathing: on the bed under the bay.
    runtime.water_press(4, false)?;
    stand(runtime, 6.0, -20.0, PI);
    run(runtime, 3.0);
    let p = runtime.player.pos;
    shoot(
        runtime,
        "water-breathing",
        p + Vec3::new(2.5, 1.6, 3.0),
        p + Vec3::new(0.0, 1.0, 0.0),
    )?;
    runtime.water_press(4, false)?;
    // Swimming without it.
    stand(runtime, 0.0, -24.0, PI);
    run(runtime, 3.0);
    let p = runtime.player.pos;
    shoot(
        runtime,
        "swimming",
        p + Vec3::new(-3.0, 1.8, 3.5),
        p + Vec3::new(0.0, 0.8, 0.0),
    )?;
    Ok(())
}

fn times(
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
    frames: usize,
) -> Result<(), String> {
    let _ = runtime.set_spawn(water::spawn(), water::SPAWN_YAW);
    let ui = verse::ui::UiBatch::default();
    let idle = verse::controller::InputState::default();
    let mut cpu = Vec::with_capacity(frames);
    let mut gpu = Vec::with_capacity(frames);
    for i in 0..frames {
        let started = Instant::now();
        runtime.tick(&idle, 1.0 / 60.0);
        let t = i as f32 / frames.max(1) as f32;
        let v = view(
            Vec3::new(20.0 - 20.0 * t, 6.0, 26.0),
            Vec3::new(-4.0, 0.0, -30.0),
            0.9,
            aspect,
        );
        let dynamic = runtime.dynamic_mesh();
        let built = Instant::now();
        renderer.render(v, &dynamic, &ui)?;
        let done = Instant::now();
        cpu.push((built - started).as_secs_f64() * 1e3);
        gpu.push((done - built).as_secs_f64() * 1e3);
    }
    let summary = |name: &str, samples: &mut Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        println!(
            "{name}: median {:.2} ms, p95 {:.2} ms, max {:.2} ms over {} frames",
            at(0.5),
            at(0.95),
            at(1.0),
            samples.len()
        );
    };
    summary("cpu (tick and frame)", &mut cpu);
    summary("gpu (render and readback)", &mut gpu);
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
