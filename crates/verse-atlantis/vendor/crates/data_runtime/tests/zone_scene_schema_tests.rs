use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/zones")
        .join(name)
}

#[test]
fn scene_json_passes_schema_minimal() {
    let p = fixture("cc_demo/scene.min.json");
    let txt = fs::read_to_string(&p).unwrap();
    data_runtime::zone_scene::validate_scene_against_schema(&txt)
        .expect("schema validation should pass");
    let scene: data_runtime::zone_scene::ZoneScene =
        serde_json::from_str(&txt).expect("serde load");
    assert_eq!(scene.version, "1.0.0");
    assert!(scene.instances.is_empty(), "cc_demo contains no props");
}

#[test]
fn scene_json_rejects_bad_fields() {
    let p = fixture("invalid/scene.bad.json");
    let txt = fs::read_to_string(&p).unwrap();
    let err = data_runtime::zone_scene::validate_scene_against_schema(&txt).unwrap_err();
    assert!(format!("{err:?}").contains("unknown field"), "{err:?}");
}

#[test]
fn scene_round_trip_preserves_semantics() {
    let p = fixture("forest_grove/scene.json");
    let txt = fs::read_to_string(&p).unwrap();
    let scene: data_runtime::zone_scene::ZoneScene = serde_json::from_str(&txt).unwrap();
    let txt2 = serde_json::to_string_pretty(&scene).unwrap();
    data_runtime::zone_scene::validate_scene_against_schema(&txt2).unwrap();
    let scene2: data_runtime::zone_scene::ZoneScene = serde_json::from_str(&txt2).unwrap();
    assert_eq!(scene.version, scene2.version);
    assert_eq!(scene.instances.len(), scene2.instances.len());
    assert_eq!(scene.logic.spawns.len(), scene2.logic.spawns.len());
}
