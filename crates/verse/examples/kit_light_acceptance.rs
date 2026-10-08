//! Private captures of Stoop Lane's offline layers and load-time fallback.
//! Compile the filtered ignored test, then run it under remote quiet/GPU leases.
//! `VERSE_KIT_LIGHT_MODE` is `layers` or `fallback`; keep every output outside Git.

use glam::{Mat4, Vec3};
use serde_json::json;
use std::path::{Path, PathBuf};
use verse::{
    render::{Offscreen, View},
    runtime::WorldRuntime,
    ui::{Atlas, UiBatch},
    zones::{self, everglade_pack},
};

fn private(path: &Path) {
    let path = path.canonicalize().unwrap();
    assert!(!path.ancestors().any(|p| p.join(".git").exists()));
}

fn write_png(path: &Path, rgba: &[u8]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1280, 800);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
}

fn capture() {
    let mode = std::env::var("VERSE_KIT_LIGHT_MODE").unwrap();
    assert!(matches!(mode.as_str(), "layers" | "fallback"));
    let expected = mode == "layers";
    let hour: f64 = std::env::var("VERSE_TOWN_HOUR").unwrap().parse().unwrap();
    let output = PathBuf::from(std::env::var_os("VERSE_KIT_LIGHT_OUTPUT").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    private(&output);
    private(Path::new(&std::env::var_os("VERSE_KIT_PACK").unwrap()));
    assert_eq!(std::env::var_os("VERSE_KIT_BAKE").is_some(), expected);
    if let Some(path) = std::env::var_os("VERSE_KIT_BAKE") {
        private(Path::new(&path));
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let pack = everglade_pack::ZonePack::load_local(&path).unwrap();
    let world = zones::everglade::Everglade::world(&pack).unwrap();
    let scene = world.mesh.textured.as_ref().unwrap();
    let merged = scene.merge().unwrap();
    let digest = verse::pbr::baked_layers::hex(&verse::pbr::baked_layers::scene_digest(
        scene, &merged,
    ));
    let vertices = merged.vertices.len();
    let layers = everglade_pack::kit_bake::offered();
    if expected {
        let layers = layers.as_ref().unwrap();
        layers.validate().unwrap();
        assert_eq!(layers.scene, digest, "Reject layers for another scene");
        assert_eq!(layers.vertex_count(), vertices);
    } else {
        assert!(layers.is_none());
    }
    drop(merged);
    drop(world);
    let mut runtime = WorldRuntime::new();
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(hour)));
    runtime.install_everglade(&pack);
    assert_eq!(runtime.zone, zones::ZoneId::Everglade);
    runtime.settle_zone_light();
    assert_eq!(
        runtime.everglade_zone_mut().unwrap().uses_baked_light(),
        expected,
        "Capture the requested production lighting path"
    );
    let (_, house) = zones::everglade::layout::city::kit_houses()
        .into_iter()
        .find(|(b, _)| b.name == "townhouse 1")
        .unwrap();
    let (outside, inside) = house.door_points();
    let floor = house.floor();
    let street = house.world([-8.0, house.depth / 2.0 + 10.0]);
    let front = house.front().0;
    runtime
        .set_spawn(Vec3::new(street[0], floor, street[1]), house.facing)
        .unwrap();
    let atlas = Atlas::new(16.0);
    let ui = UiBatch::default();
    let mut renderer = Offscreen::new(
        1280,
        800,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
    .unwrap();
    let mut records = Vec::new();
    for (name, eye, target) in [
        (
            "street",
            Vec3::new(street[0], floor + 2.8, street[1]),
            Vec3::new(front[0], floor + 2.0, front[1]),
        ),
        (
            "doorway",
            Vec3::new(outside[0], floor + 1.6, outside[1]),
            Vec3::new(inside[0], floor + 1.5, inside[1]),
        ),
        (
            "interior",
            Vec3::new(inside[0], floor + 1.6, inside[1]),
            Vec3::new(front[0], floor + 1.5, front[1]),
        ),
    ] {
        let view = View {
            eye,
            view_proj: Mat4::perspective_rh(0.8, 1.6, 0.1, 500.0)
                * Mat4::look_at_rh(eye, target, Vec3::Y),
        };
        let dynamic = runtime.dynamic_mesh();
        let intensity = dynamic.neon.as_ref().unwrap().baked_lamps;
        for _ in 0..8 {
            renderer.render(view, &dynamic, &ui).unwrap();
        }
        let full = renderer.render(view, &dynamic, &ui).unwrap();
        let file = format!("{name}-{mode}-{hour}.png");
        write_png(&output.join(&file), &full);
        let mut record = json!({
            "file": file, "eye": eye.to_array(), "target": target.to_array(),
            "baked_lamp_intensity": intensity,
        });
        if expected && intensity > 0.0 {
            let mut off = dynamic.clone();
            off.neon.as_mut().unwrap().baked_lamps = 0.0;
            for _ in 0..8 {
                renderer.render(view, &off, &ui).unwrap();
            }
            let without = renderer.render(view, &off, &ui).unwrap();
            let file = format!("{name}-without-baked-lamps-{hour}.png");
            write_png(&output.join(&file), &without);
            let changed = full.iter().zip(&without).filter(|(a, b)| a != b).count();
            record["without_baked_lamps"] = json!(file);
            record["changed_channels"] = json!(changed);
        }
        records.push(record);
    }
    let report = json!({
        "mode": mode, "hour": hour, "offline_layers_active": expected,
        "scene": digest, "vertices": vertices,
        "kit": everglade_pack::kit::KIT_SHA256,
        "public_pack": everglade_pack::PACK_SHA256,
        "bake_key": layers.as_ref().map(|x| &x.bake_key),
        "lamp_receivers": layers.as_ref().map(|x| x.lamps.len()),
        "captures": records,
        "storage": "per-vertex layers; no second-UV lightmap charts",
        "measurement": "capture with exposure settling; no frame-rate claim",
    });
    std::fs::write(
        output.join(format!("report-{mode}-{hour}.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}

fn main() {
    panic!("Run the filtered ignored private_light_tests test under its leases");
}

#[cfg(test)]
mod private_light_tests {
    #[test]
    #[ignore = "Private kit layers and remote offscreen GPU lease"]
    fn stoop_layers_and_fallback() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(super::capture)
            .unwrap()
            .join()
            .unwrap();
    }
}
