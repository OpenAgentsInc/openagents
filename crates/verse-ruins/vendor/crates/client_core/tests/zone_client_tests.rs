use serial_test::serial;
use std::fs;

#[test]
#[serial]
fn load_zone_presentation_cc_demo() {
    // Create a tiny fake snapshot in a temp dir and point loader to it via env
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp
        .path()
        .join("packs")
        .join("zones")
        .join("cc_demo")
        .join("snapshot.v1");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("meta.json"), "{\n  \"schema\":\"snapshot.v1\", \"slug\": \"cc_demo\", \"version\": \"1.0.0\", \"counts\": {\"instances\":0,\"clusters\":0,\"colliders\":0,\"logic_triggers\":0,\"logic_spawns\":0} } ").unwrap();
    unsafe {
        std::env::set_var(
            "ROA_PACKS_ROOT_FOR_TESTS",
            tmp.path().join("packs").to_string_lossy().to_string(),
        );
    }

    let zp = client_core::zone_client::ZonePresentation::load("cc_demo").expect("load");
    assert_eq!(zp.slug, "cc_demo");
}
