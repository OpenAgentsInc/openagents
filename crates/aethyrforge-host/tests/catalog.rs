use aethyrforge_host::{FANTASY_SOURCE_COUNT, FantasyCatalog, MAX_CATALOG_BYTES};

fn catalog() -> FantasyCatalog {
    FantasyCatalog::decode(include_bytes!(
        "../../../docs/verse/fantasy-props-catalog.json"
    ))
    .unwrap()
}

#[test]
fn reviewed_download_has_94_unique_sources_and_isolated_custom_face() {
    let c = catalog();
    assert_eq!(c.entries.len(), FANTASY_SOURCE_COUNT);
    assert_eq!(c.runtime.len(), 35);
    let face = c
        .runtime
        .iter()
        .find(|a| a.id == "custom:haunt:werewolf")
        .unwrap();
    assert_eq!(face.editable_surfaces[0].id, "werewolf.face");
    assert_eq!(face.rights, "openagents-project-runtime");
    assert!(!face.editable_surfaces[0].shared_with_other_assets);
    assert!(c.runtime.iter().any(|a| a.id == "medieval-town:door-4"));
    assert!(c.transfer_bytes < 10 * 1024 * 1024);
}

#[test]
fn custom_art_cannot_inherit_the_vendor_cc0_license() {
    let mut c = catalog();
    c.runtime
        .iter_mut()
        .find(|a| a.id == "custom:haunt:werewolf")
        .unwrap()
        .rights = "quaternius-cc0-1.0".into();
    assert!(c.validate().is_err());
}

#[test]
fn duplicate_sources_bad_hashes_and_paths_fail() {
    let mut c = catalog();
    c.entries[1] = c.entries[0].clone();
    assert!(c.validate().is_err());
    let mut c = catalog();
    c.entries[0].resources[0].sha256 = "not-a-hash".into();
    assert!(c.validate().is_err());
    for path in [
        "../secret.bin",
        "/secret.bin",
        "https://example.com/file",
        "a/%2e%2e/file",
    ] {
        let mut c = catalog();
        c.entries[0].resources[0].path = path.into();
        assert!(c.validate().is_err());
    }
}

#[test]
fn incompatible_animation_channels_and_unsafe_surface_fail() {
    let mut c = catalog();
    c.entries
        .iter_mut()
        .find(|a| a.name == "Chest_Wood")
        .unwrap()
        .animation_policy = "animated".into();
    assert!(c.validate().is_err());
    let mut c = catalog();
    c.source_channels = c.runtime_channels.clone();
    assert!(c.validate().is_err());
    let mut c = catalog();
    let surface = &mut c
        .runtime
        .iter_mut()
        .find(|a| a.id == "custom:haunt:werewolf")
        .unwrap()
        .editable_surfaces[0];
    surface.shared_with_other_assets = true;
    assert!(c.validate().is_err());
}

#[test]
fn nonfinite_bounds_missing_outputs_and_oversized_metadata_fail() {
    let mut c = catalog();
    c.entries[0].bounds.min[0] = f64::NAN;
    assert!(c.validate().is_err());
    let mut c = catalog();
    c.prepared_resources.pop();
    assert!(c.validate().is_err());
    assert!(FantasyCatalog::decode(&vec![b' '; MAX_CATALOG_BYTES + 1]).is_err());
}

#[test]
fn synthetic_catalog_needs_no_private_files() {
    let mut c = catalog();
    for (i, entry) in c.entries.iter_mut().enumerate() {
        entry.name = format!("Synthetic{i}");
        entry.id = format!("quaternius:fantasy-standard:synthetic{i}");
        entry.source = format!("synthetic/{i}.gltf");
        entry.resources = vec![aethyrforge_host::CatalogResource {
            path: entry.source.clone(),
            sha256: "0".repeat(64),
            bytes: 100,
        }];
        entry.animation_policy = "static".into();
        entry.source_clips.clear();
    }
    c.runtime.retain(|a| a.id.starts_with("custom:"));
    let encoded = serde_json::to_vec(&c).unwrap();
    FantasyCatalog::decode(&encoded).unwrap();
}
