//! Offscreen C2 kit captures at their coast placements.
//! No licensed assets or lighting bake are loaded. Run the ignored capture
//! test under a GPU lease with COAST_KIT_CAPTURE_OUTPUT and VERSE_QUALITY set.

use glam::{Mat4, Vec3};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use verse::render::{Offscreen, View};
use verse::zones::{self, coast};

fn capture(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut world = coast::world()?;
    let pack = coast::pack::bundled()?;
    let mut views = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for placement in coast::kit::placements() {
        if !seen.insert(placement.name) {
            continue;
        }
        let (lo, hi) = pack.model(placement.name).unwrap().bounds();
        let lo = Vec3::from(lo) * placement.scale;
        let hi = Vec3::from(hi) * placement.scale;
        let turn = glam::Quat::from_rotation_y(placement.yaw);
        let target = Vec3::from(placement.at) + turn * ((lo + hi) * 0.5);
        let radius = (hi - lo).length().max(1.0);
        let eye = if placement.name == "rocks/tide_pool_shelf" {
            Vec3::new(-230.0, 60.0, -145.0)
        } else {
            target + turn * Vec3::new(radius * 0.85, radius * 0.55, radius)
        };
        views.push((placement.name.replace('/', "-"), eye, target));
    }
    // C2 validates the admitted rigs in their bind pose. Live behavior is C4.
    let scene = std::sync::Arc::make_mut(world.mesh.textured.as_mut().unwrap());
    let material_base = scene.materials.len() - pack.materials.len();
    for (name, x, z, above) in [
        ("crab", 55., -145., 0.),
        ("gull", -320., -250., 0.),
        ("seal", -100., 230., 0.),
        ("fish", -100., 120., 1.),
    ] {
        use verse_pbr::pbr::textured::{Primitive, TexturedMesh, TexturedVertex, UNBAKED};
        let form = pack.form(&format!("beasts/{name}")).unwrap();
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        let mesh = scene.add_mesh(TexturedMesh {
            primitives: form
                .primitives
                .iter()
                .map(|p| Primitive {
                    material: material_base + p.material as usize,
                    indices: p.indices.clone(),
                    vertices: p
                        .vertices
                        .iter()
                        .map(|v| {
                            let v = &v.vertex;
                            let pos = Vec3::from(v.position);
                            lo = lo.min(pos);
                            hi = hi.max(pos);
                            TexturedVertex {
                                pos: v.position,
                                normal: v.normal,
                                uv: v.uv,
                                color: v.color,
                                light: UNBAKED,
                            }
                        })
                        .collect(),
                })
                .collect(),
        });
        let at = Vec3::new(x, coast::ground(x, z) + above, z);
        scene.place(mesh, Mat4::from_translation(at));
        let target = at + (lo + hi) * 0.5;
        let radius = (hi - lo).length().max(0.4);
        views.push((
            format!("wildlife-{name}"),
            target + Vec3::new(radius * 0.85, radius * 0.55, radius),
            target,
        ));
    }
    scene.validate()?;
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
    for (name, eye, target) in views {
        let tick = 0;
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
    let record = json!({ "adapter": format!("{:?}", renderer.adapter_info()), "tier": format!("{:?}", renderer.quality().tier), "licensed_assets": false, "lighting_bake": false, "wildlife_bind_pose_only": true, "pack_sha256": coast::pack::SHA256, "scene_gpu_bytes": world.mesh.textured.as_ref().unwrap().gpu_bytes(), "captures": records });
    std::fs::write(
        out.join("capture.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let out = std::env::args()
        .nth(1)
        .ok_or("Usage: coast_kit_capture OUTPUT_DIRECTORY")?;
    capture(Path::new(&out))
}

#[test]
#[ignore = "Offscreen raster capture; run explicitly under a GPU lease"]
fn capture_coast_kit() {
    let out = std::env::var("COAST_KIT_CAPTURE_OUTPUT").expect("COAST_KIT_CAPTURE_OUTPUT");
    capture(Path::new(&out)).unwrap();
}
