//! Bakes the view the Grid's Everglade portal shows in its opening: Everglade
//! itself, installed from the committed pinned pack and drawn by the shared
//! renderer from where a player arrives (the stepping stones by the return
//! arch, looking up the path into the town), with no HUD. The frame is
//! squared, softened, and written as the portal's texture source.
//!
//! Usage: everglade_portal_view [OUT.png] [--raw RAW.png]
//!
//! Writes `assets/verse/grid-sources/everglade-portal.png` by default; then
//! `cargo run -p verse --example grid_pack` pins it into the Grid pack. The
//! Grid cannot draw Everglade live: its pack is downloaded only on entry,
//! and phones and browsers draw the Grid alone. `--raw` also keeps the
//! unsoftened render, for review.

use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use verse::{
    controller::InputState,
    render::View,
    runtime::{Action, WorldRuntime},
    town_clock,
    zones::{self, everglade_pack},
};

/// The render, square; it is softened and reduced to [`SIZE`].
const RENDER: u32 = 1024;
/// The baked texture's side, px.
const SIZE: u32 = 512;
/// The town clock's hour: golden hour, warm and long-shadowed.
const HOUR: f64 = 17.3;
/// Where the eye stands and looks: up the approach from the stepping stones.
const EYE: Vec3 = Vec3::new(0.0, 1.7, -31.0);
const TARGET: Vec3 = Vec3::new(0.0, 3.4, 0.0);
/// The vertical field of view, radians: wide, as a doorway's glimpse.
const FOV: f32 = 1.15;

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    // The engine admits eight-bit RGBA textures only.
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| e.to_string())
}

/// `pixels` (RGBA, `side` square) reduced by `factor` with a box filter.
fn reduce(pixels: &[u8], side: u32, factor: u32) -> Vec<f32> {
    let out = side / factor;
    let mut rgb = vec![0.0; (out * out * 3) as usize];
    for y in 0..side {
        for x in 0..side {
            let i = ((y * side + x) * 4) as usize;
            let o = (((y / factor) * out + x / factor) * 3) as usize;
            for c in 0..3 {
                rgb[o + c] += f32::from(pixels[i + c]);
            }
        }
    }
    let n = (factor * factor) as f32;
    rgb.iter_mut().for_each(|v| *v /= n);
    rgb
}

/// One box-blur pass of `radius` along rows or columns, `src` into `dst`.
fn box_pass(src: &[f32], dst: &mut [f32], side: u32, radius: i32, horizontal: bool) {
    let s = side as i32;
    for y in 0..s {
        for x in 0..s {
            let mut sum = [0.0; 3];
            for k in -radius..=radius {
                let (sx, sy) = if horizontal {
                    ((x + k).clamp(0, s - 1), y)
                } else {
                    (x, (y + k).clamp(0, s - 1))
                };
                let i = ((sy * s + sx) * 3) as usize;
                for c in 0..3 {
                    sum[c] += src[i + c];
                }
            }
            let o = ((y * s + x) * 3) as usize;
            for c in 0..3 {
                dst[o + c] = sum[c] / (2 * radius + 1) as f32;
            }
        }
    }
}

/// Three passes of a separable box blur of `radius`, about a Gaussian.
fn soften(rgb: &mut [f32], side: u32, radius: i32) {
    let mut tmp = vec![0.0; rgb.len()];
    for _ in 0..3 {
        box_pass(rgb, &mut tmp, side, radius, true);
        box_pass(&tmp, rgb, side, radius, false);
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/grid-sources/everglade-portal.png");
    let mut raw = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--raw" => raw = Some(PathBuf::from(args.next().ok_or("--raw takes a path")?)),
            other => out = PathBuf::from(other),
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
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(HOUR)));
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install from the pinned pack".into());
    }
    // The player stands at the eye in first person, so their character is
    // not drawn.
    runtime.set_spawn(Vec3::new(EYE.x, 0.0, EYE.z), 0.0)?;
    runtime.apply(Action::Zoom { lines: 100.0 })?;
    eprintln!("baking the light");
    runtime.settle_zone_light();
    let idle = InputState::default();
    for _ in 0..60 {
        runtime.tick(&idle, 1.0 / 60.0);
    }
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let mut renderer = verse::render::Offscreen::new(
        RENDER,
        RENDER,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let view = View {
        view_proj: Mat4::perspective_rh(FOV, 1.0, 0.1, 2000.0)
            * Mat4::look_at_rh(EYE, TARGET, Vec3::Y),
        eye: EYE,
    };
    let dynamic = runtime.dynamic_mesh();
    let mut pixels = Vec::new();
    // A few frames so temporal anti-aliasing settles.
    for _ in 0..24 {
        pixels = renderer.render(view, &dynamic, &ui)?;
    }
    if let Some(raw) = &raw {
        write_png(raw, RENDER, RENDER, &pixels)?;
    }
    let mut rgb = reduce(&pixels, RENDER, RENDER / SIZE);
    soften(&mut rgb, SIZE, 1);
    let rgba: Vec<u8> = rgb
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255.0].map(|v| v.round().clamp(0.0, 255.0) as u8))
        .collect();
    write_png(&out, SIZE, SIZE, &rgba)?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
