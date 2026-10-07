//! The owner's private characters through the world runtime
//! (`docs/verse/private-assets.md`), with a stand-in pack and no broker.

use std::path::Path;
use std::time::{Duration, Instant};

use glam::Vec3;
use verse_private::placements::{self, Placement, Placements};

use super::ZoneId;
use super::everglade_pack::compile::private;
use super::everglade_tests::pack;
use crate::runtime::WorldRuntime;

fn at_portal() -> WorldRuntime {
    let mut runtime = WorldRuntime::new();
    let portal = ZoneId::Plaza
        .portals()
        .into_iter()
        .find(|(destination, _)| *destination == ZoneId::Everglade)
        .expect("a plaza arch to Everglade")
        .1;
    runtime.set_spawn(portal - Vec3::Z * 3.0, 0.0).unwrap();
    runtime
}

/// A placements file in `home` for a pack of `bytes` with `sha256`, asking a
/// broker that isn't there.
fn place(home: &Path, sha256: &str, bytes: u64) {
    let mut file = Placements::new("https://127.0.0.1:9", "private-test");
    file.place(Placement {
        asset: "sample-guest".into(),
        sha256: sha256.into(),
        bytes,
        zone: "everglade".into(),
        at: [6.0, -14.0],
        yaw: 0.0,
        scale: 1.0,
    });
    placements::save(home, &file).unwrap();
}

/// Ticks until `done` or a few seconds pass.
fn tick_until(runtime: &mut WorldRuntime, done: impl Fn(&WorldRuntime) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        runtime.zone_tick();
        if done(runtime) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn a_cached_private_pack_stands_its_character_in_everglade() {
    let home = tempfile::tempdir().unwrap();
    let bytes = private::sample();
    let sha256 = verse_private::sha256_hex(&bytes);
    place(home.path(), &sha256, bytes.len() as u64);
    let cache = home.path().join(placements::CACHE);
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join(format!("{sha256}.vtp")), &bytes).unwrap();

    let mut runtime = at_portal();
    runtime.configure_private_assets(home.path().to_owned());
    runtime.install_everglade(pack());
    assert_eq!(runtime.zone, ZoneId::Everglade);
    assert!(
        tick_until(&mut runtime, |r| r.private_guests() == 1),
        "the cached private pack never stood its character"
    );
    // No key file was created for the profile: a read never writes one.
    assert!(!home.path().join("private-test.key").exists());
}

#[test]
fn without_authorization_or_configuration_everglade_is_unchanged() {
    // The broker can't be reached and nothing is cached: nothing stands.
    let home = tempfile::tempdir().unwrap();
    place(home.path(), &"cd".repeat(32), 1234);
    let mut runtime = at_portal();
    runtime.configure_private_assets(home.path().to_owned());
    runtime.install_everglade(pack());
    assert!(tick_until(&mut runtime, |r| r
        .zone_state
        .private_loader
        .is_none()));
    assert_eq!(runtime.private_guests(), 0);
    assert_eq!(runtime.zone, ZoneId::Everglade);
    let cache = home.path().join(placements::CACHE);
    assert_eq!(std::fs::read_dir(cache).map_or(0, |d| d.count()), 0);

    // Not configured, as in a test or a browser: nothing is read at all.
    let mut runtime = at_portal();
    runtime.install_everglade(pack());
    runtime.zone_tick();
    assert!(runtime.zone_state.private_loader.is_none());
    assert_eq!(runtime.private_guests(), 0);
}
