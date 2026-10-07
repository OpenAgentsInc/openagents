//! The coastal test scene's checks (`docs/verse/water.md`, phase W10): its
//! shape against the coast specification, two clients on one sea, and the
//! field streaming while a player walks the bay.

use std::sync::Arc;

use glam::Vec2;
use physics::water::{Water, tick_at};
use verse_engine::quality::Tier;
use verse_net::mv;
use verse_pbr::water::field::{Stream, budget};
use verse_zone_everglade::zones::everglade_pack::RESIDENT_BYTES_BUDGET;

use crate::coast::{self, Buoys};

/// The bed and the land follow the coast specification's depth table and
/// layout within half a meter.
#[test]
fn the_scene_follows_the_coast_specification() {
    let depth_at = |x: f32, z: f32| -coast::ground(x, z);
    // A point `s` m seaward of the beach's middle, along its normal.
    let center = Vec2::from(coast::BEACH_CENTER);
    let middle = Vec2::new(200.0, 0.0);
    let out = (middle - center).normalize();
    let at = |s: f32| {
        let p = center + out * (coast::BEACH_RADIUS + s);
        depth_at(p.x, p.y)
    };
    assert!(at(-30.0) < 0.0, "the beach is dry");
    for (s, depth) in [(30.0, 1.0), (150.0, 5.0), (400.0, 20.0)] {
        assert!(
            (at(s) - depth).abs() < 0.5,
            "{s} m out: {} vs {depth}",
            at(s)
        );
    }
    assert!((depth_at(coast::HARBOR[0], coast::HARBOR[1]) - 4.0).abs() < 0.5);
    let terrace = coast::ground(coast::TERRACE[0], coast::TERRACE[2]);
    assert!((terrace - coast::TERRACE[1]).abs() < 2.5, "{terrace}");
    assert!(coast::ground(-330.0, -260.0) > 34.0, "the headland");
    assert!(coast::ground(coast::GULL_ISLAND[0], coast::GULL_ISLAND[1]) > 15.0);
    let wall = coast::BREAKWATER;
    let mid = (Vec2::from(wall[0]) + Vec2::from(wall[1])) * 0.5;
    assert!(
        coast::ground(mid.x, mid.y) > 2.0,
        "the breakwater stands above the sea"
    );
    // The field reads the same depth the ground gives.
    let field = coast::field().unwrap();
    let t = field.sample(0.0, 150.0);
    assert!((t.depth - depth_at(0.0, 150.0)).abs() < 0.3, "{t:?}");
}

/// Two clients build the coast's sea from the zone's constants alone and,
/// at each world tick their clocks give, sample the same gameplay surface
/// bit for bit, with nothing about the water exchanged. The buoyant bodies
/// one client simulates reach the other as ordinary NIP-MV shared-body
/// poses in a pose frame, the only message between them, and float on the
/// other client's own surface as they did on the first's.
#[test]
fn two_clients_sample_the_same_sea_at_shared_ticks() {
    let host = coast::water_set("moderate").unwrap();
    let guest = coast::water_set("moderate").unwrap();
    // Clocks 4 ms apart within one step give one tick.
    let unix = coast::TICK / physics::water::TICK_HZ * 1000;
    assert_eq!(tick_at(unix), tick_at(unix + 4));
    let start = tick_at(unix);
    let points: Vec<(f64, f64)> = (-6..=6)
        .flat_map(|i| (-6..=6).map(move |j| (f64::from(i) * 97.3, f64::from(j) * 88.1)))
        .chain([(-130.0, -230.0), (60.0, 40.0), (-100.0, 230.0)])
        .collect();
    for tick in [start, start + 1, start + 997, start + 30_720 * 3 + 5] {
        for &(x, z) in &points {
            let (a, b) = (host.sample(x, z, tick), guest.sample(x, z, tick));
            assert!(a.is_some());
            assert_eq!(a, b, "tick {tick} at ({x}, {z})");
        }
    }
    // The owner simulates the bodies on its surface for five seconds and
    // publishes their poses.
    let mut buoys = Buoys::new(start);
    buoys.step(&host, 600);
    let ms = (buoys.tick * 1000).div_ceil(physics::water::TICK_HZ);
    let frame = mv::Frame {
        v: 1,
        s: "owner".into(),
        n: 1,
        t: ms,
        e: buoys
            .poses()
            .iter()
            .enumerate()
            .map(|(i, (_, p, q))| mv::EntityPose {
                k: Some([1, 1]),
                ..mv::EntityPose::new(
                    &format!("buoy-{i}"),
                    mv::BODY_ROLE,
                    p.as_vec3(),
                    q.as_quat(),
                )
            })
            .collect(),
    };
    let wire = serde_json::to_string(&frame).unwrap();
    let got: mv::Frame = serde_json::from_str(&wire).unwrap();
    let tick = tick_at(got.t);
    assert_eq!(tick, buoys.tick);
    for (pose, (kind, _, _)) in got.e.iter().zip(buoys.poses()) {
        assert_eq!(pose.role, mv::BODY_ROLE);
        let (x, z) = (f64::from(pose.p[0]), f64::from(pose.p[2]));
        let theirs = guest.sample(x, z, tick).unwrap();
        assert_eq!(Some(theirs), host.sample(x, z, tick));
        // It rides the guest's water: its center within its own size of
        // the surface there.
        let off = f64::from(pose.p[1]) - theirs.height;
        assert!(off.abs() < kind.half().max_element(), "{kind:?} {off}");
    }
}

/// Walking the bay from the terrace along the beach to the harbor, the
/// headland, and out to Gull Island, each tier's field pages stream in
/// around the player, the page underfoot is in place on nearly every
/// frame, and the resident bytes never pass the tier's budget, which fits
/// the water's and the zone's resident budgets.
#[test]
fn walking_the_coast_streams_the_field_within_budget() {
    let field = Arc::new(coast::field().unwrap());
    assert!(field.bytes() < 4 * 1024 * 1024, "{}", field.bytes());
    let path = [
        Vec2::new(coast::TERRACE[0], coast::TERRACE[2]),
        Vec2::new(60.0, -30.0),
        Vec2::new(-80.0, -170.0),
        Vec2::new(-130.0, -230.0),
        Vec2::new(-330.0, -260.0),
        Vec2::new(-200.0, 60.0),
        Vec2::new(-100.0, 230.0),
        Vec2::new(240.0, 180.0),
    ];
    let water_budget = |tier| -> u64 {
        match tier {
            Tier::Low => 8,
            Tier::Medium => 32,
            Tier::High => 96,
        }
    };
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        let b = budget(tier);
        assert!(b.gpu_bytes <= water_budget(tier) * 1024 * 1024 / 4);
        assert!(b.gpu_bytes + field.bytes() as u64 <= RESIDENT_BYTES_BUDGET / 16);
        let mut stream = Stream::new(field.clone(), tier).unwrap();
        let (mut frames, mut underfoot) = (0, 0);
        for leg in path.windows(2) {
            let (a, b) = (leg[0], leg[1]);
            // A meter an update: ten times what a run at 6 m/s covers in
            // a frame at 60 frames a second.
            let steps = (a.distance(b) / 1.0).ceil() as usize;
            for k in 0..=steps {
                let eye = a.lerp(b, k as f32 / steps as f32);
                stream.update(eye, &mut |_, _, _| {}).unwrap();
                let m = stream.metrics();
                assert!(m.gpu_bytes <= stream.budget().gpu_bytes, "{tier:?}");
                assert!(m.cpu_bytes <= stream.budget().cpu_bytes, "{tier:?}");
                frames += 1;
                if stream.sample(eye.x, eye.y) == field.sample(eye.x, eye.y) {
                    underfoot += 1;
                }
            }
        }
        let m = stream.metrics();
        assert!(m.gpu_high_water <= stream.budget().gpu_bytes);
        assert!(m.evictions > 0, "{tier:?}: the walk leaves pages behind");
        assert!(
            underfoot * 100 >= frames * 99,
            "{tier:?}: {underfoot} of {frames}"
        );
    }
}
