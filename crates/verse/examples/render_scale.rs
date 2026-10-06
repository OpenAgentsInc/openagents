//! Measures how much geometry Everglade and the Grove keep on the GPU and
//! what a frame costs, and captures the views it measures, so a renderer
//! change can be compared before and after (`docs/verse/rendering-scale.md`).
//! Usage: render_scale OUTPUT_DIR [FRAMES] [everglade|grove]
//!
//! Installs the zone from the committed, pinned pack and settles its light.
//! For each view it writes `<view>.png` at 1280 by 800 and prints one JSON
//! line: the median and slowest wall time of `FRAMES` frames (default 30),
//! each rendered and read back as a capture is, and the cells, draws, and
//! triangles the main view draws ([`TexturedScene::frame_cost`]). A last
//! line holds the scene's geometry: logical triangles, and the bytes its
//! GPU layout uploads ([`TexturedScene::gpu_bytes`]).
use glam::{Mat4, Vec3};
use std::path::{Path, PathBuf};
use std::time::Instant;
use verse::{
    controller::InputState,
    pbr::textured::TexturedScene,
    runtime::WorldRuntime,
    zones::{self, everglade_pack},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let frames: usize = args
        .next()
        .map(|f| {
            f.parse()
                .map_err(|_| format!("FRAMES is a number, got {f}"))
        })
        .transpose()?
        .unwrap_or(30)
        .max(1);
    let zone = args.next().unwrap_or_else(|| "everglade".into());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
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
    let views: &[(&str, Vec3, Vec3)] = match zone.as_str() {
        "grove" => {
            runtime.install_grove(&pack);
            &[
                (
                    "grove-entry",
                    Vec3::new(0.0, 2.6, -40.0),
                    Vec3::new(0.0, 1.5, 0.0),
                ),
                (
                    "grove-center",
                    Vec3::new(0.0, 2.6, 0.0),
                    Vec3::new(30.0, 1.5, 30.0),
                ),
                (
                    "grove-air",
                    Vec3::new(0.0, 60.0, -90.0),
                    Vec3::new(0.0, 0.0, 0.0),
                ),
            ]
        }
        "everglade" => {
            runtime.install_everglade(&pack);
            &EVERGLADE_VIEWS
        }
        other => return Err(format!("unknown zone {other}; use everglade or grove")),
    };
    runtime.settle_zone_light();
    let idle = InputState::default();
    for _ in 0..10 {
        runtime.tick(&idle, 0.05);
    }
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let dynamic = runtime.dynamic_mesh();
    let air = zones::atmosphere(runtime.zone);
    let mut offscreen = verse::render::Offscreen::new(1280, 800, &runtime.world.mesh, &atlas, air)?;
    let scene = runtime.world.mesh.textured.clone();
    for &(name, eye, target) in views {
        let mut view = runtime.view(1.6);
        view.eye = eye;
        view.view_proj = Mat4::perspective_rh(verse::camera::FOV_Y, 1.6, 0.1, verse::camera::FAR)
            * Mat4::look_at_rh(eye, target, Vec3::Y);
        let pixels = offscreen.render(view, &dynamic, &ui)?;
        write_png(&dir.join(format!("{name}.png")), &pixels)?;
        let mut times: Vec<f64> = (0..frames)
            .map(|_| {
                let start = Instant::now();
                offscreen
                    .render(view, &dynamic, &ui)
                    .map(|_| start.elapsed().as_secs_f64())
            })
            .collect::<Result<_, _>>()?;
        times.sort_by(f64::total_cmp);
        let cost = scene
            .as_deref()
            .map(|s| s.frame_cost(view.view_proj, eye, air.fog_end))
            .unwrap_or_default();
        let stats = offscreen.draw_stats().unwrap_or_default();
        println!(
            "{{\"zone\":\"{zone}\",\"view\":\"{name}\",\"fastest_ms\":{:.2},\"median_ms\":{:.2},\"slowest_ms\":{:.2},\
             \"cells\":{},\"draws\":{},\"triangles\":{},\"renderer\":{}}}",
            times[0] * 1e3,
            times[times.len() / 2] * 1e3,
            times[times.len() - 1] * 1e3,
            cost.cells,
            cost.draws,
            cost.triangles,
            serde_json::to_string(&stats).map_err(|e| e.to_string())?,
        );
    }
    if let Some(scene) = scene.as_deref() {
        print_geometry(&zone, scene, &dynamic);
    }
    Ok(())
}

/// Street views from the frame budget test, and views from above at
/// several distances.
const EVERGLADE_VIEWS: [(&str, Vec3, Vec3); 10] = [
    (
        "spawn",
        Vec3::new(0.0, 2.6, -24.0),
        Vec3::new(0.0, 1.0, 0.0),
    ),
    (
        "market",
        Vec3::new(0.0, 2.6, 50.0),
        Vec3::new(-40.0, 0.0, 50.0),
    ),
    (
        "lantern",
        Vec3::new(-60.0, 2.6, -8.0),
        Vec3::new(-20.0, 0.0, 0.0),
    ),
    (
        "brownstone",
        Vec3::new(-30.0, 2.6, -78.0),
        Vec3::new(10.0, -1.0, -66.0),
    ),
    (
        "foundry",
        Vec3::new(64.0, 2.6, 6.0),
        Vec3::new(24.0, 0.0, 6.0),
    ),
    (
        "observatory",
        Vec3::new(64.0, 9.0, -46.0),
        Vec3::new(24.0, 3.0, -14.0),
    ),
    (
        "south-edge",
        Vec3::new(0.0, 2.6, -125.0),
        Vec3::new(0.0, 0.6, -85.0),
    ),
    (
        "roof-30m",
        Vec3::new(-20.0, 30.0, -60.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    (
        "air-80m",
        Vec3::new(0.0, 80.0, -150.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
    (
        "air-200m",
        Vec3::new(0.0, 200.0, -230.0),
        Vec3::new(0.0, 0.0, 0.0),
    ),
];

fn print_geometry(zone: &str, scene: &TexturedScene, dynamic: &verse::mesh::Mesh) {
    let triangles: u64 = scene
        .placements
        .iter()
        .flat_map(|p| &scene.meshes[p.mesh].primitives)
        .map(|p| p.indices.len() as u64 / 3)
        .sum();
    let bytes = scene.gpu_bytes();
    let figure = dynamic.figure.as_ref().map_or(0, |f| f.vertices.len());
    println!(
        "{{\"zone\":\"{zone}\",\"placed_triangles\":{triangles},\"gpu_bytes\":{bytes},\
         \"gpu_mib\":{:.1},\"figure_vertices\":{figure}}}",
        bytes as f64 / f64::from(1 << 20)
    );
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 1280, 800);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}
