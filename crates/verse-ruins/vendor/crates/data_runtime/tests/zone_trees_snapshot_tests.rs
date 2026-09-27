use std::fs;

#[test]
fn load_trees_snapshot_models_present() {
    // Prepare a fake packs/zones/<slug>/snapshot.v1/trees.json
    let tmp = tempfile::TempDir::new().unwrap();
    let zones_root = tmp.path().join("packs").join("zones");
    let slug = "campaign_builder";
    let snap = zones_root.join(slug).join("snapshot.v1");
    fs::create_dir_all(&snap).unwrap();
    let trees = r#"{ "models": [
      [[1,0,0,0],[0,1,0,0],[0,0,1,0],[1,0,-2,1]],
      [[0,0,1,0],[0,1,0,0],[-1,0,0,0],[0,0,0,1]]
    ] }"#;
    fs::write(snap.join("trees.json"), trees).unwrap();
    // Load snapshot via data_runtime
    let zs = data_runtime::zone_snapshot::ZoneSnapshot::load(zones_root, slug)
        .expect("load zone snapshot");
    let t = zs.trees.expect("trees present");
    assert_eq!(t.models.len(), 2);
    assert!((t.models[0][3][0] - 1.0).abs() < 1e-6);
}
