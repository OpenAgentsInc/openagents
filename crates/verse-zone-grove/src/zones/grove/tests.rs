//! The Grove's rules against the pinned pack: the field, the hotbar, and
//! each spell's effect on a dummy.

use super::SPAWN;
use super::dummies::{Dummy, FIELD, Kind, POST, RESET_AFTER};
use glam::Vec3;

const DT: f32 = 0.02;

#[test]
fn the_field_stands_at_the_demo_distances() {
    let dummies = Dummy::field();
    assert_eq!(dummies.len(), 7);
    let distance = |i: usize| {
        let d = dummies[i].home - SPAWN;
        d.x.hypot(d.z)
    };
    for (i, wanted) in [(0, 10.0), (1, 20.0), (2, 30.0), (3, 15.0), (4, 25.0)] {
        assert!((distance(i) - wanted).abs() < 0.3, "{i}: {}", distance(i));
    }
    assert_eq!(FIELD[5].0, Kind::Big);
    let flying = &dummies[6];
    assert_eq!(flying.kind, Kind::Flying);
    assert!((flying.home.y - POST).abs() < 1e-3);
    // Every dummy stands inside the meadow, clear of the arch.
    for d in &dummies {
        assert!(d.home.x.hypot(d.home.z) < super::MEADOW_RADIUS);
    }
}

#[test]
fn repeated_roots_diminish_and_the_third_is_immune() {
    let mut dummy = Dummy::new(Kind::Straw, [0.0, 0.0]);
    assert_eq!(dummy.root(12.0, 0.0), 12.0);
    assert_eq!(dummy.root(12.0, 1.0), 6.0);
    assert_eq!(dummy.root(12.0, 2.0), 0.0);
    // Past the 15-second window the full duration returns.
    assert_eq!(dummy.root(12.0, 20.0), 12.0);
}

#[test]
fn a_dummy_resets_after_ten_seconds_untouched() {
    let mut dummy = Dummy::new(Kind::Straw, [0.0, 0.0]);
    dummy.damage(30.0, super::kit::Damage::Fire, 0.0);
    dummy.push(Vec3::Z, 2.0, 0.0);
    let mut now = 0.0;
    while now < RESET_AFTER - 0.1 {
        now += DT;
        dummy.tick(DT, now, None);
    }
    assert!(dummy.hp < dummy.kind.max_hp());
    for _ in 0..10 {
        now += DT;
        dummy.tick(DT, now, None);
    }
    assert_eq!(dummy.hp, dummy.kind.max_hp());
    assert_eq!(dummy.pos, dummy.home);
}
