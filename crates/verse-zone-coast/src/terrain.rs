//! C1 extends W10's bay with the named depths, tidal shelf, and estuary.
//! The same height function serves collision and the water depth field.

use glam::Vec2;
use verse_water_spells::terrain::smoothstep;
use verse_zone_water::coast as fixture;

pub const ESTUARY_MOUTH: Vec2 = Vec2::new(100.0, -10.0);
pub const ESTUARY_GATE: Vec2 = Vec2::new(300.0, -280.0);
pub const MARSH: Vec2 = Vec2::new(170.0, -104.5);
pub const POOL_SHELF: Vec2 = Vec2::new(-230.0, -120.0);
pub const POOLS: [Vec2; 3] = [
    Vec2::new(-238.0, -125.0),
    Vec2::new(-222.0, -125.0),
    Vec2::new(-230.0, -111.0),
];
pub const POOL_RIM: f32 = -0.3;
pub const POOL_DEPTHS: [f32; 3] = [0.2, 0.5, 0.8];
pub const SANDBAR_START: Vec2 = Vec2::new(240.0, 180.0);
pub const TERN: Vec2 = Vec2::new(330.0, 330.0);

/// Distance from a segment and fraction along it, with clamped endpoints.
pub fn segment(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
    (p.distance(a.lerp(b, t)), t)
}

fn patch(h: f32, p: Vec2, center: Vec2, radii: Vec2, target: f32) -> f32 {
    let d = ((p - center) / radii).length();
    let weight = 1.0 - smoothstep(0.5, 1.0, d);
    h + (target - h) * weight
}

/// The estuary's level above mean sea level, increasing toward the gate.
pub fn river_level(p: Vec2) -> f32 {
    let (_, t) = segment(p, ESTUARY_MOUTH, ESTUARY_GATE);
    3.0 * t
}

/// Land and bathymetry in meters. Outside the playable sea, the bed
/// continues to the horizon at 40 m so refraction never sees a missing bed.
#[must_use]
pub fn ground(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let p = Vec2::new(x, z);
    let mut h = fixture::ground(x, z);
    h = patch(h, p, Vec2::new(-380.0, -50.0), Vec2::new(85.0, 65.0), -10.5);
    h = patch(h, p, Vec2::new(-100.0, -120.0), Vec2::splat(22.0), -12.0);
    h = patch(h, p, Vec2::new(-100.0, 120.0), Vec2::new(24.0, 45.0), -18.0);
    let (channel, along) = segment(p, Vec2::from(fixture::HARBOR), Vec2::new(-100.0, -150.0));
    h += (-5.0 - h) * (1.0 - smoothstep(5.0, 10.0, channel)) * smoothstep(0.0, 0.3, along);
    h = patch(h, p, POOL_SHELF, Vec2::splat(28.0), POOL_RIM);
    for (center, depth) in POOLS.into_iter().zip(POOL_DEPTHS) {
        let weight = 1.0 - smoothstep(2.0, 4.0, p.distance(center));
        h += ((POOL_RIM - depth) - h) * weight;
    }
    let (bar, _) = segment(p, SANDBAR_START, TERN);
    let bar_height = -0.6 - (bar / 5.0).powi(2);
    if bar < 18.0 {
        h = h.max(bar_height);
    }
    let island = p.distance(TERN);
    if island < 28.0 {
        h = h.max(-0.6 + 4.6 * (1.0 - smoothstep(12.0, 28.0, island)));
    }
    h = patch(h, p, MARSH, Vec2::new(24.0, 30.0), river_level(MARSH) - 0.4);
    let (river, _) = segment(p, ESTUARY_MOUTH, ESTUARY_GATE);
    if river < 12.0 {
        let bed = river_level(p) - 1.0;
        h += (bed - h) * (1.0 - smoothstep(3.0, 12.0, river));
    }
    h = patch(h, p, Vec2::new(120.0, -120.0), Vec2::splat(24.0), 14.0);
    if h < 0.0 {
        let edge = x.abs().max(z.abs());
        h += (-40.0 - h) * smoothstep(560.0, 600.0, edge);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_depths_match_the_coast_table() {
        for (p, depth) in [
            (Vec2::from(fixture::HARBOR), 4.0),
            (Vec2::new(-100.0, -150.0), 5.0),
            (Vec2::new(-380.0, -50.0), 10.5),
            (Vec2::new(-100.0, -120.0), 12.0),
            (Vec2::new(-100.0, 120.0), 18.0),
            (SANDBAR_START.lerp(TERN, 0.5), 0.6),
            (Vec2::new(0.0, 600.0), 40.0),
        ] {
            assert!(
                (-ground(p.x, p.y) - depth).abs() < 0.5,
                "{p}: {}",
                ground(p.x, p.y)
            );
        }
        let center = Vec2::from(fixture::BEACH_CENTER);
        let outward = (Vec2::new(200.0, 0.0) - center).normalize();
        for (distance, depth) in [(30.0, 1.0), (150.0, 5.0), (400.0, 20.0)] {
            let p = center + outward * (fixture::BEACH_RADIUS + distance);
            assert!((-ground(p.x, p.y) - depth).abs() < 0.5);
        }
        assert_eq!(ground(120.0, -120.0), 14.0);
        assert!(ground(-330.0, -260.0) > 35.0);
        for (p, depth) in POOLS.into_iter().zip(POOL_DEPTHS) {
            assert!((ground(p.x, p.y) - (POOL_RIM - depth)).abs() < 0.01);
        }
    }
}
