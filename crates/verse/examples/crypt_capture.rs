//! Offline visual acceptance of the crypt lab as a zone, and its frame
//! times, with the shared renderer.
//!
//! Usage: crypt_capture OUT_DIR [--frames N] [--size WxH]
//!
//! Installs the crypt as `verse --crypt` does after the Everglade pack
//! loads: the hall from the models built into the binary, walked as
//! Everglade's character. Writes one picture per view with Everglade's
//! hotbar (the spawn, walks through the study, the brewing quarter, and up
//! the dais in third person, first-person views, the door with a hotbar
//! card, and the camera pulled in by a wall), then walks a loop of the hall
//! for `N` frames (240 by default) and prints the frame times: the
//! simulation and frame building on the CPU, and the GPU render with its
//! readback.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::Vec3;
use verse::controller::InputState;
use verse::render::Offscreen;
use verse::runtime::{Action, WorldRuntime};
use verse::zones::{self, everglade::hotbar, everglade_pack};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let mut frames = 240usize;
    let (mut width, mut height) = (1280u32, 800u32);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--frames" => {
                frames = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--frames takes a count")?;
            }
            "--size" => {
                let v = args.next().ok_or("--size takes WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                width = w.parse().map_err(|_| "bad width")?;
                height = h.parse().map_err(|_| "bad height")?;
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
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
    runtime.install_crypt(&pack);
    if runtime.zone != zones::ZoneId::Crypt {
        return Err(format!(
            "The crypt did not install: {:?}",
            runtime.zone_snapshot(1.0).error
        ));
    }
    eprintln!(
        "installed in {:.0} ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let mut atlas = verse::ui::Atlas::new(16.0);
    hotbar::add_sprites(&mut atlas)?;
    let mut renderer = Offscreen::new(
        width,
        height,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(zones::ZoneId::Crypt),
    )?;
    let aspect = width as f32 / height as f32;
    let size = [width as f32, height as f32];
    let idle = InputState::default();
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    let run = |runtime: &mut WorldRuntime, input: &InputState, seconds: f32| {
        for _ in 0..(seconds / 0.02).round() as usize {
            runtime.tick(input, 0.02);
        }
    };
    let facing = |from: Vec3, to: Vec3| (to.x - from.x).atan2(to.z - from.z);
    // Name, where the player stands, what it faces, seconds of walking,
    // the camera's orbit and zoom, and a hotbar card to show.
    type Shot = (&'static str, Vec3, Vec3, f32, [f32; 2], f32, Option<usize>);
    let shots: [Shot; 8] = [
        (
            "spawn",
            zones::crypt::SPAWN,
            Vec3::new(0.0, 0.0, -8.0),
            0.0,
            [0.0, 0.0],
            0.0,
            None,
        ),
        (
            "walk-study",
            Vec3::new(0.6, 0.0, 4.6),
            Vec3::new(-4.4, 0.0, 2.0),
            0.7,
            [-90.0, 10.0],
            0.0,
            None,
        ),
        (
            "walk-brewing",
            Vec3::new(0.3, 0.0, -0.4),
            Vec3::new(-3.4, 0.0, -5.2),
            0.6,
            [70.0, 0.0],
            0.0,
            Some(2),
        ),
        (
            "walk-dais",
            Vec3::new(0.0, 0.0, -3.0),
            Vec3::new(0.0, 0.0, -9.0),
            1.4,
            [0.0, 0.0],
            0.0,
            None,
        ),
        (
            "first-moonbeam",
            Vec3::new(1.4, 0.0, 2.5),
            Vec3::new(0.4, 0.0, -4.0),
            0.0,
            [0.0, 0.0],
            100.0,
            None,
        ),
        (
            "first-dissection",
            Vec3::new(0.8, 0.0, -1.2),
            Vec3::new(3.6, 0.0, -4.4),
            0.0,
            [0.0, 0.0],
            100.0,
            None,
        ),
        (
            "door",
            Vec3::new(0.0, 0.0, 6.6),
            Vec3::new(0.0, 0.0, 9.0),
            0.4,
            [0.0, 0.0],
            0.0,
            Some(0),
        ),
        (
            "camera-wall",
            Vec3::new(-4.6, 0.0, 6.9),
            Vec3::new(-1.0, 0.0, 4.0),
            0.0,
            [180.0, -10.0],
            -30.0,
            None,
        ),
    ];
    for (name, at, toward, walking, [orbit, pitch], zoom, tip) in shots {
        runtime.camera = verse::camera::FollowCamera::default();
        runtime.set_spawn(at, facing(at, toward))?;
        run(&mut runtime, &idle, 0.2);
        if zoom != 0.0 {
            for _ in 0..4 {
                runtime.apply(Action::Zoom { lines: zoom / 4.0 })?;
                run(&mut runtime, &idle, 0.4);
            }
        }
        run(&mut runtime, &forward, walking);
        run(&mut runtime, &idle, 0.1);
        if orbit != 0.0 || pitch != 0.0 {
            runtime.apply(Action::Orbit {
                dx: orbit,
                dy: pitch,
            })?;
        }
        let mut ui = verse::ui::UiBatch::default();
        if let Some(slots) = runtime.everglade_hotbar() {
            hotbar::draw(&mut ui, &atlas, size, 0.0, &slots);
            if let Some(index) = tip {
                hotbar::draw_tip(&mut ui, &atlas, size, 0.0, index);
            }
        }
        let pixels = renderer.render(runtime.view(aspect), &runtime.dynamic_mesh(), &ui)?;
        write_png(&out.join(format!("{name}.png")), width, height, &pixels)?;
        let snapshot = runtime.zone_snapshot(aspect);
        eprintln!(
            "{name}: player {:.2} first person {} caption {:?}",
            runtime.player.pos,
            runtime.first_person(),
            snapshot.caption
        );
    }
    times(&mut runtime, &mut renderer, aspect, frames)
}

/// Walks a loop of the hall for `frames` frames at 60 Hz and prints the
/// CPU and GPU frame times.
fn times(
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
    frames: usize,
) -> Result<(), String> {
    runtime.camera = verse::camera::FollowCamera::default();
    runtime.set_spawn(zones::crypt::SPAWN, PI)?;
    let walk = InputState {
        forward: true,
        right: true,
        ..InputState::default()
    };
    let ui = verse::ui::UiBatch::default();
    let mut cpu = Vec::with_capacity(frames);
    let mut gpu = Vec::with_capacity(frames);
    for i in 0..frames {
        let started = Instant::now();
        // Turn now and then, so the walk circles the hall.
        let input = if i % 90 < 60 {
            InputState {
                right: false,
                ..walk
            }
        } else {
            walk
        };
        runtime.tick(&input, 1.0 / 60.0);
        let view = runtime.view(aspect);
        let dynamic = runtime.dynamic_mesh();
        let built = Instant::now();
        renderer.render(view, &dynamic, &ui)?;
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
