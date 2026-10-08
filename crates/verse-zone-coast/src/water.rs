//! The coast's shared-clock tide, water bodies, shelter, and currents.

use glam::{DVec2, DVec3, Vec2};
use physics::water::{Sample, Water as PhysicsWater, WaterBody, WaterId};
use verse_pbr::water::field::{Field, PAGE, Texel};
use verse_water_spells::terrain::smoothstep;
use verse_zone_water::coast as fixture;

use crate::terrain;

pub const OCEAN: WaterId = WaterId(0);
pub const ESTUARY: WaterId = WaterId(1);
pub const MARSH: WaterId = WaterId(2);
pub const TIDE_PERIOD: u64 = 24 * 60 * physics::water::TICK_HZ;
pub const TIDE_AMPLITUDE: f64 = 1.2;

/// The shared harbor mask, uploaded in the renderer's existing water array.
pub const HARBOR_SHELTER: verse_pbr::water::shelter::Shelter = verse_pbr::water::shelter::Shelter {
    center: fixture::HARBOR,
    inner: 55.0,
    outer: 95.0,
    gain: 0.1,
};

/// Tide height at a shared 120 Hz Unix tick. Fold before converting to
/// floating point so a long-running clock retains submillimeter precision.
#[must_use]
pub fn tide(tick: u64) -> f64 {
    let phase = (tick % TIDE_PERIOD) as f64 / TIDE_PERIOD as f64;
    TIDE_AMPLITUDE * (std::f64::consts::TAU * phase).sin()
}

fn tide_velocity(tick: u64) -> f64 {
    let phase = (tick % TIDE_PERIOD) as f64 / TIDE_PERIOD as f64;
    TIDE_AMPLITUDE * std::f64::consts::TAU / (24.0 * 60.0) * (std::f64::consts::TAU * phase).cos()
}

/// Harbor swell amplitude: one tenth inside, smoothly joining the open sea.
#[must_use]
pub fn shelter(p: Vec2) -> f32 {
    HARBOR_SHELTER.sample(p)[0]
}

/// Estuary discharge and the inward current in the outer 40 m of the zone.
#[must_use]
pub fn current(p: Vec2) -> Vec2 {
    let inward = |x: f32| -x.signum() * 2.0 * ((x.abs() - 560.0) / 40.0).clamp(0.0, 1.0);
    let mut flow = Vec2::new(inward(p.x), inward(p.y)).clamp_length_max(2.0);
    let (distance, _) = terrain::segment(p, terrain::ESTUARY_MOUTH, terrain::ESTUARY_GATE);
    let discharge = 1.0 - smoothstep(3.0, 12.0, distance);
    flow += (terrain::ESTUARY_MOUTH - terrain::ESTUARY_GATE).normalize() * (1.2 * discharge);
    flow
}

/// Gameplay water. The sea, estuary, marsh, and isolated pools share the
/// same height function and clock on every client.
pub struct CoastalWater {
    sea: WaterBody,
}

impl CoastalWater {
    pub fn new(state: &str) -> Result<Self, String> {
        Ok(Self {
            sea: fixture::ocean(state)?,
        })
    }

    fn still(body: WaterId, height: f64, flow: Vec2, density: f64) -> Sample {
        Sample {
            body,
            height,
            normal: DVec3::Y,
            surface_velocity: DVec3::ZERO,
            flow: DVec3::new(f64::from(flow.x), 0.0, f64::from(flow.y)),
            density,
        }
    }
}

impl PhysicsWater for CoastalWater {
    fn sample(&self, x: f64, z: f64, tick: u64) -> Option<Sample> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        let p = Vec2::new(x as f32, z as f32);
        let level = tide(tick);
        let (river, along) = terrain::segment(p, terrain::ESTUARY_MOUTH, terrain::ESTUARY_GATE);
        let mut sample = if river <= 3.0 {
            let height = f64::from(terrain::river_level(p)) + level * f64::from(1.0 - along);
            Self::still(ESTUARY, height, current(p), 1010.0)
        } else if ((p - terrain::MARSH) / Vec2::new(20.0, 26.0)).length() < 1.0 {
            Self::still(
                MARSH,
                f64::from(terrain::river_level(terrain::MARSH)),
                Vec2::ZERO,
                1010.0,
            )
        } else if level < f64::from(terrain::POOL_RIM)
            && let Some(index) = terrain::POOLS
                .iter()
                .position(|center| p.distance(*center) < 4.0)
        {
            Self::still(
                WaterId(3 + index as u32),
                f64::from(terrain::POOL_RIM),
                Vec2::ZERO,
                physics::water::SALT,
            )
        } else {
            let mut sample = self.sea.sample(x, z, tick)?;
            let wave_height = sample.height;
            let gain = f64::from(shelter(p));
            let e = 0.1;
            let dx = f64::from((shelter(p + Vec2::X * e) - shelter(p - Vec2::X * e)) / (2.0 * e));
            let dz = f64::from((shelter(p + Vec2::Y * e) - shelter(p - Vec2::Y * e)) / (2.0 * e));
            let slope = DVec2::new(-sample.normal.x, -sample.normal.z) / sample.normal.y.max(1e-6);
            let gradient = slope * gain + DVec2::new(dx, dz) * wave_height;
            sample.normal = DVec3::new(-gradient.x, 1.0, -gradient.y).normalize();
            sample.height = level + wave_height * gain;
            sample.surface_velocity *= gain;
            sample.surface_velocity.y += tide_velocity(tick);
            let flow = current(p);
            sample.flow = DVec3::new(f64::from(flow.x), 0.0, f64::from(flow.y));
            sample
        };
        if sample.height <= f64::from(terrain::ground(p.x, p.y)) {
            return None;
        }
        // Brackish bodies ride the tide near their mouth; their vertical
        // velocity follows the same tide rather than a separate local clock.
        if sample.body == ESTUARY {
            sample.surface_velocity.y = tide_velocity(tick) * f64::from(1.0 - along);
        }
        Some(sample)
    }

    fn bodies_overlapping(&self, min: DVec2, max: DVec2) -> Vec<WaterId> {
        let overlaps = |center: Vec2, radius: Vec2| {
            let lo = (center - radius).as_dvec2();
            let hi = (center + radius).as_dvec2();
            lo.x <= max.x && lo.y <= max.y && hi.x >= min.x && hi.y >= min.y
        };
        let mut out = vec![OCEAN];
        if overlaps(
            (terrain::ESTUARY_MOUTH + terrain::ESTUARY_GATE) * 0.5,
            (terrain::ESTUARY_GATE - terrain::ESTUARY_MOUTH).abs() * 0.5 + Vec2::splat(3.0),
        ) {
            out.push(ESTUARY);
        }
        if overlaps(terrain::MARSH, Vec2::new(20.0, 26.0)) {
            out.push(MARSH);
        }
        for (index, center) in terrain::POOLS.into_iter().enumerate() {
            if overlaps(center, Vec2::splat(4.0)) {
                out.push(WaterId(3 + index as u32));
            }
        }
        out
    }
}

/// Resident CPU fields. The optical field retains W10's RGBA16F depth,
/// shore distance, and current; shelter is an additional R8 plane.
pub struct Fields {
    pub optical: Field,
    pub shelter: Vec<u8>,
    pub side: usize,
    pub origin: f32,
}

pub fn fields() -> Result<Fields, String> {
    let side = (fixture::PAGES * PAGE) as usize;
    let origin = -(side as f32) * fixture::TEXEL * 0.5;
    let optical = Field::bake(
        [origin; 2],
        fixture::TEXEL,
        [fixture::PAGES; 2],
        Texel::open(40.0),
        |x, z| (-terrain::ground(x, z), current(Vec2::new(x, z)).to_array()),
    )?;
    let mut mask = Vec::with_capacity(side * side);
    for z in 0..side {
        for x in 0..side {
            let p = Vec2::new(
                origin + (x as f32 + 0.5) * fixture::TEXEL,
                origin + (z as f32 + 0.5) * fixture::TEXEL,
            );
            mask.push((shelter(p) * 255.0).round() as u8);
        }
    }
    Ok(Fields {
        optical,
        shelter: mask,
        side,
        origin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_tick_tide_has_exact_phase_and_period() {
        assert_eq!(tide(0), 0.0);
        assert!((tide(TIDE_PERIOD / 4) - 1.2).abs() < 1e-12);
        assert!((tide(TIDE_PERIOD * 3 / 4) + 1.2).abs() < 1e-12);
        assert_eq!(
            tide(1791374400 * physics::water::TICK_HZ),
            tide(1791374400 * physics::water::TICK_HZ + TIDE_PERIOD)
        );
    }

    #[test]
    fn tide_changes_sandbar_access_and_pool_identity() {
        let sea = CoastalWater::new("calm").unwrap();
        let p = terrain::SANDBAR_START.lerp(terrain::TERN, 0.5).as_dvec2();
        assert!(sea.sample(p.x, p.y, TIDE_PERIOD * 3 / 4).is_none());
        assert!(sea.sample(p.x, p.y, TIDE_PERIOD / 4).is_some());
        for (i, p) in terrain::POOLS.iter().enumerate() {
            assert_eq!(
                sea.sample(f64::from(p.x), f64::from(p.y), TIDE_PERIOD * 3 / 4)
                    .unwrap()
                    .body,
                WaterId(3 + i as u32)
            );
            assert_eq!(
                sea.sample(f64::from(p.x), f64::from(p.y), TIDE_PERIOD / 4)
                    .unwrap()
                    .body,
                OCEAN
            );
        }
    }

    #[test]
    fn current_pushes_inward_and_harbor_shelters_swell() {
        for p in [
            Vec2::new(600.0, 0.0),
            Vec2::new(-600.0, 0.0),
            Vec2::new(0.0, 600.0),
            Vec2::new(0.0, -600.0),
            Vec2::splat(600.0),
        ] {
            let flow = current(p);
            assert!(flow.dot(p) < 0.0);
            assert!((flow.length() - 2.0).abs() < 1e-5);
        }
        assert_eq!(shelter(Vec2::from(fixture::HARBOR)), 0.1);
        assert_eq!(shelter(Vec2::new(0.0, 150.0)), 1.0);
        let sea = CoastalWater::new("moderate").unwrap();
        let [x, z] = fixture::HARBOR.map(f64::from);
        let raw = sea.sea.sample(x, z, 1234).unwrap();
        let sheltered = sea.sample(x, z, 1234).unwrap();
        assert!((sheltered.height - tide(1234) - raw.height * 0.1).abs() < 1e-6);
    }

    #[test]
    fn generated_fields_repeat_and_match_ground_and_current() {
        let a = fields().unwrap();
        let b = fields().unwrap();
        assert_eq!(a.shelter, b.shelter);
        for z in (0..a.side).step_by(13) {
            for x in (0..a.side).step_by(11) {
                let px = a.origin + (x as f32 + 0.5) * fixture::TEXEL;
                let pz = a.origin + (z as f32 + 0.5) * fixture::TEXEL;
                let sample = a.optical.sample(px, pz);
                assert_eq!(sample, b.optical.sample(px, pz));
                assert!((sample.depth + terrain::ground(px, pz)).abs() < 0.04);
                assert!((Vec2::from(sample.flow) - current(Vec2::new(px, pz))).length() < 0.002);
            }
        }
    }
}
