//! Captures the production town clock and measures baked sun blending.
//! Usage: VERSE_REQUIRE_BAKED=1 baked_light_capture OUTPUT_DIR [PAIRS]
//!
//! Keeps one offscreen renderer for every frame. Measurements alternate
//! identical frames with the sun blend enabled and disabled, after warmup.
//! The GPU wait includes pixel readback; report it separately from CPU time.

#[path = "support/baked.rs"]
mod baked;

use std::path::{Path, PathBuf};
use verse::{controller::InputState, runtime::{Action, WorldRuntime}, zones::everglade_pack};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let pairs = args.next().map(|n| n.parse::<usize>().map_err(|e| e.to_string()))
        .transpose()?.unwrap_or(60).max(1);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let pack = everglade_pack::ZonePack::load_local(&path)?;
    baked::require_layers(&pack)?;
    let mut runtime = WorldRuntime::new();
    runtime.set_town_clock(baked::running_clock(12.0));
    runtime.install_everglade(&pack);
    runtime.settle_zone_light();
    if !runtime.everglade_zone_mut().is_some_and(|z| z.uses_baked_light()) {
        return Err("The timelapse requires verified offline layers".into());
    }
    runtime.set_spawn(glam::Vec3::new(-60.0, 0.0, 30.0), std::f32::consts::PI)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 30.0 })?;
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let air = verse::zones::atmosphere(runtime.zone);
    let mut renderer = verse::render::Offscreen::new(1280, 800, &runtime.world.mesh, &atlas, air)?;
    let idle = InputState::default();
    // The named phases and both sides of every baked sun sample use the
    // unpinned production adapter, including its stepped sky schedule.
    for (name, hour) in [
        ("dawn", 6.0), ("noon", 12.0), ("dusk", 18.0), ("night", 0.0),
        ("sun-08-before", 7.95), ("sun-08-after", 8.05),
        ("sun-12-before", 11.95), ("sun-12-after", 12.05),
        ("sun-1530-before", 15.45), ("sun-1530-after", 15.55),
        ("sun-1730-before", 17.45), ("sun-1730-after", 17.55),
    ] {
        runtime.set_town_clock(baked::running_clock(hour));
        runtime.tick(&idle, 1.0 / 60.0);
        let dynamic = runtime.dynamic_mesh();
        let pixels = renderer.render(runtime.view(1.6), &dynamic, &ui)?;
        write_png(&dir.join(format!("{name}.png")), &pixels)?;
        let blend = dynamic.neon.as_ref().ok_or("Everglade has no light stage")?.baked_sun;
        println!("{{\"capture\":\"{name}\",\"hour\":{hour},\"baked_sun\":{blend:?}}}");
    }
    runtime.set_town_clock(baked::running_clock(9.0));
    runtime.tick(&idle, 1.0 / 60.0);
    let view = runtime.view(1.6);
    let on = runtime.dynamic_mesh();
    let mut off = on.clone();
    off.neon.as_mut().ok_or("Everglade has no light stage")?.baked_sun = [0.0; 4];
    let mut samples = [Vec::new(), Vec::new()];
    for pair in 0..pairs + 12 {
        // Alternate order so steady thermal drift affects both modes.
        for mode in [pair % 2, 1 - pair % 2] {
            renderer.render(view, if mode == 0 { &off } else { &on }, &ui)?;
            if pair >= 12 { samples[mode].push(renderer.last_timing()); }
        }
    }
    for (name, samples) in ["disabled", "blended"].into_iter().zip(samples) {
        let mut cpu: Vec<_> = samples.iter().map(|x| x.0).collect();
        let mut gpu: Vec<_> = samples.iter().map(|x| x.1).collect();
        cpu.sort_by(f32::total_cmp);
        gpu.sort_by(f32::total_cmp);
        println!("{{\"mode\":\"{name}\",\"pairs\":{pairs},\"cpu_median_ms\":{},\"gpu_readback_median_ms\":{}}}", cpu[pairs / 2], gpu[pairs / 2]);
    }
    Ok(())
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 1280, 800);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().map_err(|e| e.to_string())?
        .write_image_data(pixels).map_err(|e| e.to_string())
}
