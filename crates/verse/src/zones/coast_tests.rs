//! Coast entry uses the shared pack loader and preserves the plaza pose.

use super::{Intent, LoadState, ZoneId, coast, everglade_pack};
use crate::{controller::InputState, runtime::WorldRuntime};

#[test]
fn coast_identity_entry_and_return_preserve_the_plaza_pose() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!(
                "{}.{}",
                everglade_pack::PACK_SHA256,
                everglade_pack::PACK_EXTENSION
            )),
    )
    .unwrap();
    super::atmosphere(ZoneId::Coast).validate().unwrap();
    assert_eq!(ZoneId::from_name("coast"), Some(ZoneId::Coast));
    assert_eq!(ZoneId::Coast.world_id(), "verse-coast");
    assert_eq!(serde_json::to_string(&ZoneId::Coast).unwrap(), "\"coast\"");
    assert_eq!(ZoneId::Coast.half_extent(), 600.0);
    let mut runtime = WorldRuntime::new();
    runtime
        .set_spawn(super::COAST_ARCH - glam::Vec3::Z * 3.0, 0.7)
        .unwrap();
    let before = runtime.player;
    assert!(runtime.zone_snapshot(1.0).portal.near);
    // No configured loader: explicit entry refuses without moving anyone.
    assert!(runtime.enter_coast().is_err());
    assert_eq!(runtime.zone, ZoneId::Plaza);
    runtime.zone_cancel_loading();
    runtime.install_coast_bytes(&bytes).unwrap();
    assert_eq!(runtime.zone, ZoneId::Coast);
    assert_eq!(runtime.zone_load_state(), LoadState::Idle);
    assert_eq!(runtime.player.pos, coast::SPAWN);
    assert!(runtime.world.mesh.water.as_ref().unwrap().ocean.is_some());
    assert!(runtime.water_lab().is_none());
    runtime.tick(&InputState::default(), 0.02);
    assert!((runtime.player.pos.y - coast::SPAWN.y).abs() < 0.01);
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(runtime.player.pos, before.pos);
    assert_eq!(runtime.player.yaw, before.yaw);
    assert!(runtime.zone_state.coast.is_none());
}
