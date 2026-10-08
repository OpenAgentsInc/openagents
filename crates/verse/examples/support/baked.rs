//! Acceptance helpers for captures that must use the reviewed light layers.

use verse::zones::{everglade::Everglade, everglade_pack};

/// Verifies the scene before installation can start a fallback bake.
pub fn require_layers(pack: &everglade_pack::ZonePack) -> Result<(), String> {
    if std::env::var_os("VERSE_REQUIRE_BAKED").is_none() {
        return Ok(());
    }
    let layers = everglade_pack::kit_bake::offered()
        .ok_or("The capture requires offline light layers")?;
    let world = Everglade::world(pack)?;
    let scene = world.mesh.textured.ok_or("Everglade has no textured scene")?;
    let merged = scene.merge()?;
    let digest = verse::pbr::baked_layers::hex(
        &verse::pbr::baked_layers::scene_digest(&scene, &merged),
    );
    if digest != layers.scene || merged.vertices.len() != layers.vertex_count() {
        return Err(format!("Offline layers match scene {}, but the capture builds {digest}", layers.scene));
    }
    layers.validate()?;
    eprintln!("Verified offline layers: scene {digest}, {} vertices", merged.vertices.len());
    Ok(())
}

/// Uses the production clock adapter at a chosen hour, with no hour pin.
pub fn running_clock(hour: f64) -> town_clock::Clock {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs() as i64;
    town_clock::Clock {
        epoch_unix: unix - (hour.rem_euclid(24.0) * 3600.0).round() as i64,
        mode: town_clock::Mode::WallClock { utc_offset_minutes: 0 },
        pinned_second: None,
    }
}
