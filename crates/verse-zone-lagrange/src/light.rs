//! Light, sky, and the bounce-light bake for Lagrange 1.
//!
//! The Sun at L1 delivers about 130,000 lux; everything else a shadowed face
//! receives is sunlight bounced off the station. Probes are baked on a worker
//! thread from the fixed structure plus parts resting in the rack or latched
//! in the jig, and baked again whenever that assembly changes.

use std::sync::{Arc, OnceLock, mpsc};

use glam::{DVec3, Vec3};
use verse_lagrange::{PartState, Station, orbit, station::attitude};

use crate::pbr::{
    Body, Camera, LitVertex, ProbeGrid, SUN_ILLUMINANCE, Sky,
    bake::{self, Bvh, ProbeSettings},
    sky,
};

/// The region that casts and receives sun shadows and holds the probes.
pub const BOUNDS_MIN: Vec3 = Vec3::new(-34.0, -14.0, -16.0);
pub const BOUNDS_MAX: Vec3 = Vec3::new(34.0, 18.0, 28.0);

pub struct Light {
    bvh: Arc<Bvh>,
    probes: Option<Arc<ProbeGrid>>,
    pending: Option<mpsc::Receiver<ProbeGrid>>,
    key: Option<Vec<(usize, u8, [i32; 3])>>,
    version: u64,
}

/// A rotating-frame vector in station body axes (the scene).
fn body_axes(v: DVec3) -> DVec3 {
    attitude() * sky::to_scene(v)
}

/// The same mapping as a matrix, for bases.
fn attitude_matrix() -> glam::DMat3 {
    glam::DMat3::from_quat(attitude())
}

/// The static structure's BVH, shared by every visit.
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

    /// Collects a finished bake and starts a new one when the resting parts
    /// changed. `parts` returns the lit geometry of resting parts.
    pub fn update(
        &mut self,
        station: &Station,
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
        let key: Vec<_> = station
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.state, PartState::Stowed | PartState::Installed))
            .map(|(i, p)| {
                let at = station.body(p).pos;
                (
                    i,
                    p.state as u8,
                    [at.x, at.y, at.z].map(|x| (x * 4.0).round() as i32),
                )
            })
            .collect();
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.key = Some(key);
        self.version += 1;
        let mut geometry = structure.to_vec();
        geometry.extend(parts());
        let sun_dir = body_axes(station.orbit.sun_direction()).as_vec3();
        let version = self.version;
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        let spawned = std::thread::Builder::new()
            .name("lagrange-light-bake".into())
            .spawn(move || {
                let bvh = Bvh::new(&geometry);
                let grid = bake::bake_probes(
                    &bvh,
                    &ProbeSettings {
                        min: BOUNDS_MIN,
                        max: BOUNDS_MAX,
                        cell: 3.0,
                        rays: 64,
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

    /// Whether a point outside the structure sees the Sun.
    pub fn sunlit(&self, point: Vec3, sun_dir: Vec3) -> bool {
        !self.bvh.occluded(point, sun_dir, 1.0e4)
    }

    /// The sky for this frame, seen from `eye`.
    pub fn sky(&self, station: &Station, eye: Vec3, camera: Camera, time: f32) -> Sky {
        let o = &station.orbit;
        let sun_vector = (DVec3::new(-o.l1.mu, 0.0, 0.0) - o.state.pos) * orbit::AU;
        let sun_distance = sun_vector.length();
        let sun_dir = body_axes(sun_vector / sun_distance).as_vec3();
        let earth = o.earth_vector();
        let moon = o.moon_vector();
        let longitude = sky::earth_longitude(sky::EPOCH) + orbit::mean_motion() * o.mission_seconds;
        let jd = sky::EPOCH + o.mission_seconds / 86_400.0;
        let turn = attitude_matrix();
        let body = |v: DVec3, radius: f64, axes: glam::DMat3| Body {
            dir: body_axes(v.normalize()).as_vec3(),
            angular_radius: (radius / v.length()).asin() as f32,
            distance: v.length(),
            axes: sky::single(turn * axes),
        };
        Sky {
            sun_dir,
            // Inverse square from 1 AU, where the solar constant applies.
            sun_illuminance: SUN_ILLUMINANCE * ((orbit::AU * 0.99) / sun_distance).powi(2) as f32,
            sun_angular_radius: (orbit::SUN_RADIUS / sun_distance).asin() as f32,
            sun_visible: bake::sun_visibility(&self.bvh, eye, sun_dir, 0.0047),
            earth: body(earth, orbit::EARTH_RADIUS, sky::earth_axes(jd, longitude)),
            moon: body(
                moon,
                orbit::MOON_RADIUS,
                sky::moon_axes(sky::to_scene(earth - moon)),
            ),
            celestial: sky::single(turn * sky::celestial(longitude)),
            shadow_center: (BOUNDS_MIN + BOUNDS_MAX) / 2.0,
            shadow_half: (BOUNDS_MAX - BOUNDS_MIN).max_element() / 2.0,
            camera,
            probes: self.probes.clone(),
            time,
        }
    }
}
