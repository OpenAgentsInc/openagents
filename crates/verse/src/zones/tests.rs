use super::*;
use crate::{controller::InputState, runtime::WorldRuntime};

fn assets() -> assets::LoadedAssets {
    assets::LoadedAssets::load_local(&std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/forest/7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp")).unwrap()
}

#[test]
fn manifest_requires_the_exact_supported_profiles_and_content() {
    let valid = Manifest::forest().unwrap();
    valid.validate().unwrap();
    for field in ["schema", "world", "ruleset", "physics", "asset_sha256"] {
        let mut v = serde_json::to_value(&valid).unwrap();
        v[field] = "unknown".into();
        assert!(
            serde_json::from_value::<Manifest>(v)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut invalid = valid.clone();
    invalid.asset_bytes += 1;
    assert!(invalid.validate().is_err());
    let mut v = serde_json::to_value(valid).unwrap();
    v["script"] = "run something".into();
    assert!(serde_json::from_value::<Manifest>(v).is_err());
}

#[test]
fn entering_and_returning_replace_only_the_zone_and_preserve_plaza_choices() {
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(glam::Vec3::new(-12.0, 0.0, 9.0), 0.7)
        .unwrap();
    runtime.hold_door_item(crate::doors::DemoItem::Bolt);
    let pose = runtime.player;
    let document = runtime.doors.document();
    let plaza_bytes = runtime.world.mesh.faces.len();
    assert!(runtime.zone_state.forest.is_none());
    assert!(!runtime.zone_loading());
    runtime.install_forest(assets());
    assert_eq!(runtime.zone, ZoneId::Forest);
    assert_eq!(runtime.zone_revision, 1);
    assert_ne!(runtime.world.mesh.faces.len(), plaza_bytes);
    assert!(!runtime.computer(1.0).near);
    assert!(!runtime.gym(1.0).inside);
    assert!(!runtime.door(crate::doors::DoorId::Spark, 1.0).near);
    assert!(
        runtime.world.mesh.faces.len() * std::mem::size_of::<crate::mesh::Vertex>()
            < 96 * 1024 * 1024
    );
    assert!(
        runtime
            .world
            .mesh
            .faces
            .iter()
            .any(|v| v.color[1] > v.color[0])
    );
    runtime.zone_intent(Intent::Fireball).unwrap();
    runtime.tick(
        &InputState {
            forward: true,
            ..Default::default()
        },
        0.05,
    );
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.zone_revision, 2);
    assert_eq!(runtime.player.pos, pose.pos);
    assert_eq!(runtime.player.yaw, pose.yaw);
    assert_eq!(runtime.doors.document(), document);
    assert_eq!(runtime.world.mesh.faces.len(), plaza_bytes);
    assert!(runtime.zone_state.forest.is_none());
}

#[test]
fn loading_requires_explicit_nearby_entry_and_cancellation_keeps_the_plaza() {
    let mut runtime = WorldRuntime::new();
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    assert_eq!(runtime.zone, ZoneId::Plaza);
    runtime
        .set_spawn(glam::Vec3::new(-12.0, 0.0, 9.0), 0.0)
        .unwrap();
    assert!(runtime.zone_intent(Intent::Enter).is_err());
    runtime.zone_cancel_loading();
    assert_eq!(runtime.zone_state.loading, LoadState::Idle);
    assert!(runtime.zone_state.forest.is_none());
}

#[test]
fn fog_profiles_are_independent_and_bounded() {
    assert_ne!(atmosphere(ZoneId::Forest), atmosphere(ZoneId::Plaza));
    atmosphere(ZoneId::Forest).validate().unwrap();
    let mut bad = atmosphere(ZoneId::Forest);
    bad.fog_start = bad.fog_end;
    assert!(bad.validate().is_err());
    bad = atmosphere(ZoneId::Forest);
    bad.color[1] = f32::NAN;
    assert!(bad.validate().is_err());
}
