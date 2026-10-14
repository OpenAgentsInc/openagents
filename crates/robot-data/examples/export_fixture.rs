//! Write original pixels and states for offline reference-loader checks.
use robot_data::export::{self, Episode, Frame, Spec, Termination, Unit};
use serde_json::json;
use std::{collections::BTreeMap, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args()
        .nth(1)
        .ok_or("Choose a new dataset folder")?;
    let spec = Spec {
        robot_type: "test-arm".into(),
        joints: vec!["shoulder.pos".into(), "wrist.pos".into()],
        units: vec![Unit::Radians; 2],
        fps: 30,
        task: "Move the cube into the target".into(),
        cameras: BTreeMap::from([("observation.images.front".into(), [2, 1])]),
        license: "cc0-1.0".into(),
        license_text: "Original fixture released under CC0-1.0".into(),
        notice: "Original test geometry and pixels; no imported assets".into(),
        source: json!({"scene":{"document":"test-scene","version":"a".repeat(64)}}),
    };
    let episodes = (0..2)
        .map(|episode| Episode {
            frames: (0..4)
                .map(|i| {
                    let mut bytes = Vec::new();
                    {
                        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
                        encoder.set_color(png::ColorType::Rgb);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()
                            .unwrap()
                            .write_image_data(&[20 + 10 * i, 40, 80, 20 + 10 * i, 80, 120])
                            .unwrap();
                    }
                    Frame {
                        state: vec![i as f32 * 0.1, episode as f32 * 0.2],
                        action: vec![0.3, 0.2],
                        cameras: BTreeMap::from([("observation.images.front".into(), bytes)]),
                    }
                })
                .collect(),
            termination: if episode == 0 {
                Termination::Complete
            } else {
                Termination::Interrupted
            },
        })
        .collect::<Vec<_>>();
    let exported = export::write(Path::new(&destination), &spec, &episodes)?;
    println!("{}", serde_json::to_string(&exported)?);
    Ok(())
}
