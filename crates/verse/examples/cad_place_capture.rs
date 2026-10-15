//! A CAD Lab version standing in Everglade (#11642): installs Everglade
//! from the pinned pack, loads the CAD placements file the zone loads
//! (`verse::zones::cad_place`; written by `cad_place` or `openagents cad
//! place`), and photographs the first model placed there from beside it.
//!
//! Usage: cad_place_capture OUT.png [--placements FILE] [--hour H]
//!
//! Renders 1920 by 1080 through the offscreen renderer. Run it with
//! `VERSE_QUALITY=high`.

use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    town_clock,
    zones::{self, cad_place, everglade_pack},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

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

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected the output PNG")?);
    let mut placements = cad_place::path();
    let mut hour = 16.5_f64;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--placements" => {
                placements = PathBuf::from(args.next().ok_or("--placements needs a file")?);
            }
            "--hour" => {
                hour = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--hour needs a number")?;
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
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
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(hour)));
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    let standing = runtime.refresh_cad_placements_from(&placements);
    let placed = runtime
        .cad_placed()
        .first()
        .ok_or_else(|| format!("Nothing is placed in Everglade in {}", placements.display()))?;
    let (doc, version, origin, size, heading) = (
        placed.placement.doc.clone(),
        placed.placement.version,
        placed.origin,
        placed.size,
        placed.placement.heading_deg.to_radians(),
    );
    eprintln!("{standing} placed; {doc} v{version} at {origin}, {size} m");
    // From the model's front quarter, a little above it, with the player
    // standing beside it for scale.
    let forward = Vec3::new(heading.cos(), 0.0, -heading.sin());
    let side = Vec3::new(forward.z, 0.0, -forward.x);
    let reach = size.max_element().max(0.5);
    let eye = origin + forward * reach * 2.2 + side * reach * 1.5 + Vec3::Y * reach * 0.95;
    let target = origin + Vec3::Y * size.y * 0.42;
    let stand = origin - side * reach * 1.2 + forward * reach * 0.3;
    runtime.set_spawn(stand, heading)?;
    eprintln!("baking the light");
    runtime.settle_zone_light();
    let idle = InputState::default();
    for _ in 0..10 {
        runtime.tick(&idle, 0.05);
    }
    runtime.set_shot(Some((eye, target)));
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let dynamic = runtime.dynamic_mesh();
    let pixels = renderer.render(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, &ui)?;
    write_png(&out, &pixels)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
