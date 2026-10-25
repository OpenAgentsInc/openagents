//! Versioned engine scene metadata. Normal builds use checked-in bindings.
//! Decoding establishes shape; validation establishes this contract's limits.

pub mod v1 {
    include!("generated/openagents.engine.v1.rs");
}
pub use v1::*;

pub const MAX_BYTES: usize = 16 * 1024;
pub const MAX_LANDMARKS: usize = 16;
pub const MAX_COLLIDERS: usize = 64;

fn id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 48
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn label(text: &str, required: bool) -> bool {
    (!required || !text.is_empty()) && text.len() <= 64 && text.bytes().all(|b| b >= 32 && b != 127)
}
fn point(p: &Vec3) -> bool {
    [p.x, p.y, p.z]
        .iter()
        .all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
}

/// Decode a bounded Protobuf payload, then check the supported version and values.
/// Unknown fields are accepted and discarded. Do not use this path to reserialize
/// signed messages or content-addressed source bytes.
pub fn decode(bytes: &[u8]) -> Result<SceneMetadata, &'static str> {
    use prost::Message;
    if bytes.len() > MAX_BYTES {
        return Err("scene metadata exceeds the byte limit");
    }
    let scene = SceneMetadata::decode(bytes).map_err(|_| "invalid scene metadata encoding")?;
    validate(&scene)?;
    Ok(scene)
}

pub fn validate(scene: &SceneMetadata) -> Result<(), &'static str> {
    use std::collections::BTreeSet;
    if scene.schema_version != 1
        || scene.coordinates != CoordinateConvention::YUpMetersZForward as i32
        || !id(&scene.scene_id)
        || scene.landmarks.len() > MAX_LANDMARKS
        || scene.colliders.len() > MAX_COLLIDERS
    {
        return Err("unsupported scene metadata header or limits");
    }
    let mut ids = BTreeSet::new();
    for landmark in &scene.landmarks {
        if !id(&landmark.id)
            || !ids.insert(&landmark.id)
            || !label(&landmark.label, true)
            || !label(&landmark.interaction_label, landmark.interaction.is_some())
            || !landmark.position.as_ref().is_some_and(point)
            || !landmark.label_height_m.is_finite()
            || !(0.0..=100.0).contains(&landmark.label_height_m)
        {
            return Err("invalid scene landmark");
        }
        if let Some(range) = &landmark.interaction
            && (!range.enter_m.is_finite()
                || !range.leave_m.is_finite()
                || range.enter_m <= 0.0
                || range.leave_m < range.enter_m
                || range.leave_m > 100.0)
        {
            return Err("invalid scene interaction range");
        }
    }
    ids.clear();
    for collider in &scene.colliders {
        let Some((min, max)) = collider.min.as_ref().zip(collider.max.as_ref()) else {
            return Err("missing scene collider bounds");
        };
        if !id(&collider.id)
            || !ids.insert(&collider.id)
            || !point(min)
            || !point(max)
            || min.x >= max.x
            || min.y >= max.y
            || min.z >= max.z
        {
            return Err("invalid scene collider");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn python_encoded_fixture_decodes_and_reencodes() {
        let source = include_bytes!("../fixtures/scene.pb");
        let scene = decode(source).unwrap();
        assert_eq!(scene.scene_id, "grid-fixture");
        assert_eq!(scene.landmarks[0].position.unwrap().z, 4.0);
        assert_eq!(scene.landmarks[0].interaction.unwrap().leave_m, 4.5);
        assert!(scene.landmarks[1].position.is_some());
        assert!(scene.landmarks[1].interaction.is_none());
        assert_eq!(scene.encode_to_vec(), source);
        assert_eq!(
            decode(include_bytes!("../fixtures/scene-unknown.pb")).unwrap(),
            scene
        );
    }

    #[test]
    fn cross_language_fixture_expectations_match() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "pb") {
                let valid = !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("invalid-");
                assert_eq!(
                    decode(&std::fs::read(&path).unwrap()).is_ok(),
                    valid,
                    "{}",
                    path.display()
                );
            }
        }
    }
}
