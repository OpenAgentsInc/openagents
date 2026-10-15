use glam::DVec3;

use crate::earth_moon::{self, DISTANCE, Halo, HaloOrbit};
use crate::orbit::{self, State};

const DAY: f64 = 86_400.0;

#[test]
fn l2_sits_where_the_earth_moon_system_puts_it() {
    let mu = earth_moon::mu();
    assert!((mu - 0.012_150_6).abs() < 1e-6, "mu {mu}");
    let l2 = earth_moon::l2();
    // About 64,515 km beyond the Moon's center in the CR3BP.
    let km = l2.gamma * DISTANCE / 1_000.0;
    assert!((64_400.0..64_600.0).contains(&km), "{km} km");
    let rest = State {
        pos: l2.point(),
        vel: DVec3::ZERO,
    };
    assert!(orbit::acceleration(mu, &rest).length() < 1e-12);
    // Richardson's coefficient and the linear frequencies for Earth–Moon L2.
    assert!((l2.c2 - 3.1904).abs() < 2e-3, "c2 {}", l2.c2);
    assert!((l2.lambda - 2.1587).abs() < 2e-3, "lambda {}", l2.lambda);
    assert!((l2.omega_p - 1.8626).abs() < 2e-3, "omega_p {}", l2.omega_p);
    let snapshot = HaloOrbit::new().snapshot();
    // Errors grow e-fold in about two days, ten times faster than at
    // Sun–Earth L1.
    assert!(
        (1.9..2.1).contains(&snapshot.instability_days),
        "{}",
        snapshot.instability_days
    );
    // Light takes about 1.5 s from L2 to the Earth.
    let light = (l2.x + mu) * DISTANCE / orbit::LIGHT_SPEED;
    assert!((1.4..1.6).contains(&light), "{light} s");
    // The time unit is about 4.34 days.
    let days = earth_moon::time_unit() / DAY;
    assert!((days - 4.3425).abs() < 1e-3, "{days} d");
}

#[test]
fn earth_moon_l1_matches_the_published_offset() {
    // L1 sits 15.09% of the Earth–Moon distance short of the Moon.
    let l1 = earth_moon::l1();
    assert!((l1.gamma - 0.1509).abs() < 1e-4, "gamma {}", l1.gamma);
    assert!((l1.c2 - 5.1478).abs() < 2e-3, "c2 {}", l1.c2);
    // And the Sun–Earth L1 through the shared solver is unchanged.
    let se = orbit::L1::sun_earth();
    let general = orbit::Collinear::new(se.mu, orbit::CollinearPoint::L1);
    assert_eq!(se.gamma, general.gamma);
    assert_eq!(se.lambda, general.lambda);
}

#[test]
fn the_l2_unstable_left_eigenvector_is_exact() {
    let l2 = earth_moon::l2();
    let [_, tau, _, _] = l2.unstable_direction();
    let eps = 1e-7;
    let state = State {
        pos: l2.point() + DVec3::new(eps, eps * tau, 0.0),
        vel: DVec3::new(eps * l2.lambda, eps * l2.lambda * tau, 0.0),
    };
    assert!((l2.unstable_component(&state) - eps).abs() < 1e-15);
    let orbit = l2.lissajous(1e-3, 5e-3, 0.3, 1.1);
    assert!(l2.unstable_component(&orbit).abs() < 1e-15);
}

#[test]
fn l2_uncontrolled_motion_conserves_jacobi_and_leaves_l2() {
    let mu = earth_moon::mu();
    let l2 = earth_moon::l2();
    let unit = earth_moon::time_unit();
    // Thirty days on the halo with no control and no disturbance.
    let halo = Halo::earth_moon();
    let c0 = orbit::jacobi(mu, &halo.initial);
    let later = earth_moon::propagate(mu, &halo.initial, 30.0 * DAY / unit);
    let drift = (orbit::jacobi(mu, &later) - c0).abs();
    assert!(drift / c0.abs() < 1e-9, "Jacobi drift {drift}");
    // At the point itself a 1 cm/s kick grows e-fold about every two days.
    let mut s = State {
        pos: l2.point(),
        vel: DVec3::new(0.01 / earth_moon::velocity_unit(), 0.0, 0.0),
    };
    let distance = |s: &State| s.pos.distance(l2.point()) * DISTANCE / 1_000.0;
    s = earth_moon::propagate(mu, &s, 6.0 * DAY / unit);
    let d6 = distance(&s);
    s = earth_moon::propagate(mu, &s, 4.0 * DAY / unit);
    let d10 = distance(&s);
    let efold = 4.0 / (d10 / d6).ln();
    assert!((1.8..2.2).contains(&efold), "e-folding {efold} d");
    s = earth_moon::propagate(mu, &s, 30.0 * DAY / unit);
    assert!(
        distance(&s) > 50_000.0,
        "the collinear point is unstable: {} km",
        distance(&s)
    );
}

#[test]
fn a_corrected_halo_closes() {
    let halo = Halo::earth_moon();
    let unit = earth_moon::time_unit();
    let days = halo.period * unit / DAY;
    // ARTEMIS flew a 15.47-day L2 orbit; the linear periods are 14.6 and
    // 15.3 days.
    assert!((14.0..16.0).contains(&days), "period {days} d");
    let end = earth_moon::propagate(halo.mu, &halo.initial, halo.period);
    let km = end.pos.distance(halo.initial.pos) * DISTANCE / 1_000.0;
    let mm_s = end.vel.distance(halo.initial.vel) * earth_moon::velocity_unit() * 1_000.0;
    assert!(km < 1.0, "closes within {km} km");
    assert!(mm_s < 1.0, "closes within {mm_s} mm/s");
    // A large southern halo: well clear of the Moon's shadow on the Earth.
    let [ax, ay, az] = halo.amplitudes_km();
    eprintln!("halo period {days:.3} d, half widths {ax:.0} {ay:.0} {az:.0} km");
    assert!(halo.initial.pos.z < 0.0);
    assert!(az > 10_000.0 && ay > 20_000.0);
}

#[test]
fn a_halo_keeps_the_earth_in_view() {
    let halo = Halo::earth_moon();
    let mut s = halo.initial;
    let n = 10 * 400;
    for _ in 0..n {
        assert!(earth_moon::earth_visible(halo.mu, &s), "{s:?}");
        s = earth_moon::propagate(halo.mu, &s, halo.period / 400.0);
        // Restart each revolution from the closed orbit, so ten revolutions
        // test the orbit rather than the instability.
        if (s.pos - halo.initial.pos).length() * DISTANCE < 1_000.0 {
            s = halo.initial;
        }
    }
    // At L2 itself the Moon hides the Earth.
    let l2 = earth_moon::l2();
    let at = State {
        pos: l2.point(),
        vel: DVec3::ZERO,
    };
    assert!(!earth_moon::earth_visible(halo.mu, &at));
}

#[test]
fn station_keeping_holds_a_halo_for_a_year() {
    let mut orbit = HaloOrbit::new();
    let mut worst: f64 = 0.0;
    for _ in 0..365 {
        orbit.advance(DAY);
        let s = orbit.snapshot();
        worst = worst.max(s.deviation_km);
        assert!(s.earth_visible, "day {}", s.mission_days);
    }
    let s = orbit.snapshot();
    eprintln!(
        "halo station-keeping {:.2} m/s per year, {} burns of {} evaluations, worst deviation {worst:.1} km",
        s.keeping_dv_m_s, s.burns, orbit.evaluations
    );
    assert!(worst < 100.0, "bounded: {worst} km");
    assert!(orbit.evaluations >= 364);
    assert!(s.burns > 100, "{} burns", s.burns);
    assert!(
        s.keeping_dv_m_s > 2.0 && s.keeping_dv_m_s < 15.0,
        "{} m/s per year",
        s.keeping_dv_m_s
    );
    // The halo swings the factory 410,000 to 490,000 km from the Earth.
    assert!((400_000.0..500_000.0).contains(&s.earth_distance_km));
}

#[test]
fn without_station_keeping_the_halo_is_lost() {
    let mut orbit = HaloOrbit::new();
    // A zero burn turns station-keeping off.
    orbit.apply_burn(DVec3::ZERO);
    orbit.advance(40.0 * DAY);
    assert_eq!(orbit.burns, 0);
    // Solar pressure alone pushes it off within a few revolutions.
    assert!(
        orbit.snapshot().deviation_km > 1_000.0,
        "{}",
        orbit.snapshot().deviation_km
    );
}

#[test]
fn a_burn_departs_the_halo() {
    let mut orbit = HaloOrbit::new();
    orbit.advance(3.0 * DAY);
    let kept = orbit.burns;
    orbit.apply_burn(DVec3::new(20.0, 0.0, 0.0));
    let mut left = None;
    for day in 0..30 {
        orbit.advance(DAY);
        if orbit.snapshot().deviation_km > 50_000.0 {
            left = Some(day + 1);
            break;
        }
    }
    let days = left.expect("the ship leaves the halo within 30 days");
    eprintln!("left the halo neighbourhood after {days} days");
    assert_eq!(
        orbit.burns, kept,
        "station-keeping is off after the test burn"
    );
}

#[test]
fn a_saved_halo_orbit_restores_and_continues_identically() {
    let mut a = HaloOrbit::new();
    a.advance(5.5 * DAY);
    let text = serde_json::to_string(&a).unwrap();
    let mut b: HaloOrbit = serde_json::from_str(&text).unwrap();
    a.advance(3.0 * DAY);
    b.advance(3.0 * DAY);
    assert_eq!(a, b);
}
