//! Light and sky for Lagrange 2: the Sun, the Earth, the Moon, and the
//! stars from where the factory flies on its halo, and the bounce-light bake.
//!
//! Scene axes are the factory frame. The factory holds a fixed attitude in
//! the Earth–Moon rotating frame: its long axis (+X, the cradle's thrust
//! axis) along rotating +x, away from the Moon and the Earth, and its deck
//! tilted [`TILT`] toward the Sun's side of the orbit plane so the deck is
//! lit at a working angle rather than edge-on.

use std::sync::{Arc, OnceLock, mpsc};

use glam::{DMat3, DQuat, DVec3, Vec3};
use verse_lagrange::earth_moon::{self, HaloOrbit};
use verse_lagrange::orbit::{EARTH_RADIUS, MOON_RADIUS};

use crate::pbr::{
    Body, Camera, LitVertex, ProbeGrid, SUN_ILLUMINANCE, Sky,
    bake::{self, Bvh, ProbeSettings},
    sky,
};

/// The region that casts and receives sun shadows and holds the probes.
pub const BOUNDS_MIN: Vec3 = Vec3::new(-56.0, -8.0, -56.0);
pub const BOUNDS_MAX: Vec3 = Vec3::new(56.0, 40.0, 56.0);
/// How far the deck normal leans from the orbit's pole toward the Sun's
/// side, rad.
pub const TILT: f64 = 1.05;

/// Rotating-frame vectors to scene axes: x stays x, the orbit pole z
/// becomes the deck normal +Y before the tilt, and y becomes -Z.
#[must_use]
pub fn attitude() -> DMat3 {
    let base = DMat3::from_cols(DVec3::X, -DVec3::Z, DVec3::Y);
    DMat3::from_rotation_x(TILT) * base
}

/// A rotating-frame vector in scene axes.
#[must_use]
pub fn to_scene(v: DVec3) -> DVec3 {
    attitude() * v
}

pub struct Light {
    bvh: Arc<Bvh>,
    probes: Option<Arc<ProbeGrid>>,
    pending: Option<mpsc::Receiver<ProbeGrid>>,
    key: Option<Vec<i32>>,
    version: u64,
}

fn structure_bvh(structure: &[LitVertex]) -> Arc<Bvh> {
    static BVH: OnceLock<Arc<Bvh>> = OnceLock::new();
    BVH.get_or_init(|| Arc::new(Bvh::new(structure))).clone()
}

impl Light {
    pub fn new(structure: &[LitVertex]) -> Self {
        Self {
            bvh: structure_bvh(structure),
            probes: None,
            pending: None,
            key: None,
            version: 0,
        }
    }

    /// Collects a finished bake and starts a new one when the latched parts
    /// or the Sun's direction (in 5° steps) changed.
    pub fn update(
        &mut self,
        latched: usize,
        sun_dir: Vec3,
        structure: &'static [LitVertex],
        parts: impl FnOnce() -> Vec<LitVertex>,
    ) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(grid) => {
                    self.probes = Some(Arc::new(grid));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
        let step = 5.0_f32.to_radians();
        let key = vec![
            latched as i32,
            (sun_dir.x.atan2(sun_dir.z) / step).round() as i32,
            (sun_dir.y.asin() / step).round() as i32,
        ];
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.key = Some(key);
        self.version += 1;
        let mut geometry = structure.to_vec();
        geometry.extend(parts());
        let version = self.version;
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        let spawned = std::thread::Builder::new()
            .name("lagrange2-light-bake".into())
            .spawn(move || {
                let bvh = Bvh::new(&geometry);
                let grid = bake::bake_probes(
                    &bvh,
                    &ProbeSettings {
                        min: BOUNDS_MIN,
                        max: BOUNDS_MAX,
                        cell: 6.0,
                        rays: 48,
                        sun_dir,
                        sun_illuminance: SUN_ILLUMINANCE,
                        version,
                    },
                );
                let _ = tx.send(grid);
            });
        if spawned.is_err() {
            self.pending = None;
        }
    }

    /// Blocks until the current bake finishes (captures and tests).
    pub fn wait(&mut self) {
        if let Some(rx) = self.pending.take()
            && let Ok(grid) = rx.recv()
        {
            self.probes = Some(Arc::new(grid));
        }
    }

    /// The sky for this frame, seen from `eye`, with the factory turned by
    /// `turn` from its held attitude.
    pub fn sky(&self, orbit: &HaloOrbit, eye: Vec3, camera: Camera, time: f32, turn: DQuat) -> Sky {
        let scene = DMat3::from_quat(turn.inverse()) * attitude();
        let to_scene = |v: DVec3| scene * v;
        let sun_dir = to_scene(orbit.sun_direction()).normalize().as_vec3();
        let earth = orbit.earth_vector();
        let moon = orbit.moon_vector();
        let jd = sky::EPOCH + orbit.mission_seconds / 86_400.0;
        // The Moon's ecliptic longitude: the Sun's geocentric longitude less
        // the Sun's angle in the rotating frame (whose x runs Earth to Moon).
        let sun_rot = orbit.sun_direction();
        let sun_longitude = sky::earth_longitude(jd) + std::f64::consts::PI;
        let moon_longitude = sun_longitude - sun_rot.y.atan2(sun_rot.x);
        let celestial = scene
            * DMat3::from_rotation_z(-moon_longitude)
            * DMat3::from_rotation_x(-sky::OBLIQUITY);
        let north = to_scene(DVec3::Z);
        let moon_axes = {
            let x = to_scene((earth - moon).normalize());
            let y = north.cross(x).normalize();
            DMat3::from_cols(x, y, x.cross(y))
        };
        let body = |v: DVec3, radius: f64, axes: DMat3| Body {
            dir: to_scene(v.normalize()).as_vec3(),
            angular_radius: (radius / v.length()).asin() as f32,
            distance: v.length(),
            axes: sky::single(axes),
        };
        Sky {
            sun_dir,
            sun_illuminance: SUN_ILLUMINANCE,
            sun_angular_radius: 0.004_65,
            sun_visible: bake::sun_visibility(&self.bvh, eye, sun_dir, 0.0047),
            earth: body(
                earth,
                EARTH_RADIUS,
                celestial * DMat3::from_rotation_z(sky::sidereal_time(jd)),
            ),
            moon: body(moon, MOON_RADIUS, moon_axes),
            celestial: sky::single(celestial),
            shadow_center: (BOUNDS_MIN + BOUNDS_MAX) / 2.0,
            shadow_half: (BOUNDS_MAX - BOUNDS_MIN).max_element() / 2.0,
            camera,
            probes: self.probes.clone(),
            time,
        }
    }
}

/// The halo's shape in kilometers relative to L2, for the orbit display.
#[must_use]
pub fn halo_loop(samples: usize) -> Vec<DVec3> {
    let halo = earth_moon::Halo::earth_moon();
    let l2 = earth_moon::l2().point();
    (0..=samples)
        .map(|i| {
            let s = halo.state_at(halo.period * i as f64 / samples as f64);
            (s.pos - l2) * earth_moon::DISTANCE / 1_000.0
        })
        .collect()
}
