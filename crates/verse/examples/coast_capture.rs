//! Offscreen C1 bay and tidal-body captures from procedural coast geometry.
//! No licensed assets or lighting bake are loaded. Run the ignored capture
//! test under a GPU lease with COAST_CAPTURE_OUTPUT and VERSE_QUALITY set.

use glam::{Mat4, Vec3};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use verse::render::{Offscreen, View};
use verse::zones::{self, coast};

fn capture(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let world = coast::world()?;
    let atlas = verse::ui::Atlas::new(16.0);
    let mut renderer = Offscreen::new(
        960,
        540,
        &world.mesh,
        &atlas,
        zones::atmosphere(zones::ZoneId::Coast),
    )?;
    let ui = verse::ui::UiBatch::default();
    let mut records = Vec::new();
    for (name, eye, target, tick) in [
        (
            "bay",
            Vec3::new(100.0, 45.0, -100.0),
            Vec3::new(-40.0, -2.0, 120.0),
            coast::water::TIDE_PERIOD / 4,
        ),
        (
            "harbor",
            Vec3::new(-80.0, 8.0, -235.0),
            Vec3::new(-170.0, 0.0, -200.0),
            coast::water::TIDE_PERIOD / 4,
        ),
        (
            "estuary",
            Vec3::new(205.0, 90.0, -120.0),
            Vec3::new(175.0, 1.0, -110.0),
            coast::water::TIDE_PERIOD / 4,
        ),
        (
            "pools-low",
            Vec3::new(-230.0, 90.0, -145.0),
            Vec3::new(-230.0, -0.3, -120.0),
            coast::water::TIDE_PERIOD * 3 / 4,
        ),
        (
            "pools-high",
            Vec3::new(-230.0, 90.0, -145.0),
            Vec3::new(-230.0, -0.3, -120.0),
            coast::water::TIDE_PERIOD / 4,
        ),
    ] {
        let live = coast::Coast::new(tick)?;
        let dynamic = live.mesh(eye);
        let view = View {
            eye,
            view_proj: Mat4::perspective_rh(0.9, 960.0 / 540.0, 0.1, 2400.0)
                * Mat4::look_at_rh(eye, target, Vec3::Y),
        };
        // Let all requested optical-field pages become resident.
        for _ in 0..64 {
            renderer.render(view, &dynamic, &ui)?;
        }
        let pixels = renderer.render(view, &dynamic, &ui)?;
        let path = out.join(format!("{name}.png"));
        let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 960, 540);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&pixels))
            .map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        records.push(json!({ "name": name, "tick": tick, "eye": eye.to_array(), "target": target.to_array(), "tide_m": coast::water::tide(tick), "sha256": format!("{:x}", Sha256::digest(&bytes)), "bytes": bytes.len(), "gpu_frame_ms": renderer.last_gpu_ms() }));
    }
    let record = json!({ "adapter": format!("{:?}", renderer.adapter_info()), "tier": format!("{:?}", renderer.quality().tier), "licensed_assets": false, "lighting_bake": false, "captures": records });
    std::fs::write(
        out.join("capture.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let out = std::env::args()
        .nth(1)
        .ok_or("Usage: coast_capture OUTPUT_DIRECTORY")?;
    capture(Path::new(&out))
}

#[test]
#[ignore = "Offscreen raster capture; run explicitly under a GPU lease"]
fn capture_coast() {
    let out = std::env::var("COAST_CAPTURE_OUTPUT").expect("COAST_CAPTURE_OUTPUT");
    capture(Path::new(&out)).unwrap();
}
