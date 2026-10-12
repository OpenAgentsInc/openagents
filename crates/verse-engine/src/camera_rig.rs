//! A keyframed cinematic camera: keys of time, eye, target, focal length,
//! focus distance, and aperture, joined by a centripetal Catmull-Rom spline
//! with easing per segment and an optional procedural shake (issue #11432).
//!
//! The rig is plain data, loaded from JSON, so a shot is authored without
//! code. [`CameraRig::sample`] gives the camera at a time;
//! [`lens_view_projection`] turns a sample and a point on the lens into a
//! view-projection whose focal plane stays fixed, so averaging frames over
//! the lens gives depth of field.
use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// The sensor's width, mm: full-frame 35 mm.
pub const SENSOR_WIDTH_MM: f32 = 36.0;
/// The sensor's height, mm, cropped to 16:9.
pub const SENSOR_HEIGHT_MM: f32 = SENSOR_WIDTH_MM * 9.0 / 16.0;
/// The most keys a rig holds.
pub const MAX_KEYS: usize = 256;

/// How a segment's time maps to its progress along the path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ease {
    /// Constant speed through the segment.
    #[default]
    Linear,
    /// Starts at rest.
    In,
    /// Comes to rest.
    Out,
    /// Starts and ends at rest.
    InOut,
}

impl Ease {
    /// Progress at segment fraction `x` in 0..=1.
    #[must_use]
    pub fn apply(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Self::Linear => x,
            Self::In => x * x * x,
            Self::Out => 1.0 - (1.0 - x).powi(3),
            // Smootherstep: zero velocity and acceleration at both ends.
            Self::InOut => x * x * x * (x * (x * 6.0 - 15.0) + 10.0),
        }
    }
}

fn default_focal() -> f32 {
    35.0
}

/// One key: where the camera is at time `t`, s, and how its lens is set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub t: f32,
    pub eye: Vec3,
    pub target: Vec3,
    /// Focal length, mm, on a [`SENSOR_HEIGHT_MM`]-high sensor.
    #[serde(default = "default_focal")]
    pub focal_mm: f32,
    /// Distance in focus, m; the distance from eye to target when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<f32>,
    /// The aperture's f-number; a pinhole, everything in focus, when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f_stop: Option<f32>,
    /// The easing of the segment from this key to the next.
    #[serde(default)]
    pub ease: Ease,
}

/// A burst of shake, such as an impact's: `amplitude` at `at`, s, decaying
/// by `1/e` every `decay` seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub at: f32,
    pub amplitude: f32,
    #[serde(default = "default_decay")]
    pub decay: f32,
}

fn default_decay() -> f32 {
    0.35
}

fn default_frequency() -> f32 {
    0.6
}

/// Low-frequency handheld drift: the aim turns by up to `amplitude`
/// radians and the eye moves by up to `position` m, varying at about
/// `frequency` Hz. Bursts add faster, decaying shake on top.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shake {
    #[serde(default)]
    pub amplitude: f32,
    #[serde(default)]
    pub position: f32,
    #[serde(default = "default_frequency")]
    pub frequency: f32,
    #[serde(default)]
    pub seed: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bursts: Vec<Burst>,
}

/// A keyframed camera.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraRig {
    pub keys: Vec<Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shake: Option<Shake>,
}

/// The camera at one time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSample {
    pub eye: Vec3,
    pub target: Vec3,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    /// Distance in focus, m.
    pub focus: f32,
    /// The lens's radius, m; zero for a pinhole.
    pub aperture: f32,
}

/// The vertical field of view, radians, of a `focal_mm` lens.
#[must_use]
pub fn fov_y(focal_mm: f32) -> f32 {
    2.0 * (SENSOR_HEIGHT_MM * 0.5 / focal_mm).atan()
}

impl CameraRig {
    /// Parses and validates a rig.
    ///
    /// # Errors
    ///
    /// Returns a message when the JSON or the rig is invalid.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let rig: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        rig.validate()?;
        Ok(rig)
    }

    /// Checks the keys are finite, ordered in time, and physically sensible.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.keys.is_empty() || self.keys.len() > MAX_KEYS {
            return Err(format!("A camera rig needs 1 to {MAX_KEYS} keys"));
        }
        for (i, k) in self.keys.iter().enumerate() {
            let bad = !k.t.is_finite()
                || !k.eye.is_finite()
                || !k.target.is_finite()
                || k.eye.distance(k.target) < 1e-3
                || !(4.0..=2400.0).contains(&k.focal_mm)
                || k.focus.is_some_and(|f| !f.is_finite() || f <= 0.0)
                || k.f_stop.is_some_and(|f| !f.is_finite() || f < 0.5);
            if bad {
                return Err(format!("Camera key {i} is invalid"));
            }
        }
        if self.keys.windows(2).any(|w| w[1].t <= w[0].t) {
            return Err("Camera keys must be strictly ordered in time".into());
        }
        if let Some(s) = &self.shake {
            let bad = [s.amplitude, s.position, s.frequency]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
                || s.amplitude > 0.5
                || s.frequency > 30.0
                || s.bursts.len() > 64
                || s.bursts.iter().any(|b| {
                    !b.at.is_finite()
                        || !b.amplitude.is_finite()
                        || !(0.0..=0.5).contains(&b.amplitude)
                        || !b.decay.is_finite()
                        || b.decay <= 0.0
                });
            if bad {
                return Err("Camera shake is invalid".into());
            }
        }
        Ok(())
    }

    /// The camera at time `t`, s. Before the first key and after the last
    /// the camera holds still (apart from shake).
    #[must_use]
    pub fn sample(&self, t: f32) -> CameraSample {
        let t = if t.is_finite() { t } else { 0.0 };
        let keys = &self.keys;
        let n = keys.len();
        let (i, u) = if n == 1 || t <= keys[0].t {
            (0, 0.0)
        } else if t >= keys[n - 1].t {
            (n - 2, 1.0)
        } else {
            let i = keys.windows(2).position(|w| t < w[1].t).unwrap_or(n - 2);
            let (a, b) = (&keys[i], &keys[i + 1]);
            (i, a.ease.apply((t - a.t) / (b.t - a.t)))
        };
        let (eye, target, focal, focus, f_stop) = if n == 1 {
            let k = &keys[0];
            (
                k.eye,
                k.target,
                k.focal_mm,
                k.focus.unwrap_or_else(|| k.eye.distance(k.target)),
                k.f_stop,
            )
        } else {
            let point = |f: fn(&Key) -> Vec3| {
                let p1 = f(&keys[i]);
                let p2 = f(&keys[i + 1]);
                let p0 = if i == 0 {
                    2.0 * p1 - p2
                } else {
                    f(&keys[i - 1])
                };
                let p3 = if i + 2 >= n {
                    2.0 * p2 - p1
                } else {
                    f(&keys[i + 2])
                };
                centripetal(p0, p1, p2, p3, u)
            };
            let (a, b) = (&keys[i], &keys[i + 1]);
            let lerp = |x: f32, y: f32| x + (y - x) * u;
            let focus_of = |k: &Key| k.focus.unwrap_or_else(|| k.eye.distance(k.target));
            // Focal length eases geometrically, so a zoom reads as steady.
            let focal = (a.focal_mm.ln() + (b.focal_mm.ln() - a.focal_mm.ln()) * u).exp();
            let f_stop = match (a.f_stop, b.f_stop) {
                (Some(x), Some(y)) => Some(lerp(x, y)),
                (x, y) => x.or(y),
            };
            (
                point(|k| k.eye),
                point(|k| k.target),
                focal,
                lerp(focus_of(a), focus_of(b)),
                f_stop,
            )
        };
        let (eye, target) = match &self.shake {
            Some(shake) => shake.apply(t, eye, target),
            None => (eye, target),
        };
        CameraSample {
            eye,
            target,
            fov_y: fov_y(focal),
            focus: focus.max(0.05),
            aperture: f_stop.map_or(0.0, |n| focal / 1000.0 / n * 0.5),
        }
    }

    /// The time of the last key.
    #[must_use]
    pub fn end(&self) -> f32 {
        self.keys.last().map_or(0.0, |k| k.t)
    }
}

/// The point at `u` in 0..=1 between `p1` and `p2` on the centripetal
/// Catmull-Rom spline through `p0..p3` (Barry and Goldman's form), which
/// neither overshoots into cusps nor loops between close keys.
#[must_use]
pub fn centripetal(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, u: f32) -> Vec3 {
    let knot = |a: Vec3, b: Vec3| a.distance(b).sqrt().max(1e-4);
    let t0 = 0.0;
    let t1 = t0 + knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + (t2 - t1) * u.clamp(0.0, 1.0);
    let mix = |a: Vec3, b: Vec3, ta: f32, tb: f32| {
        a * ((tb - t) / (tb - ta)) + b * ((t - ta) / (tb - ta))
    };
    let a1 = mix(p0, p1, t0, t1);
    let a2 = mix(p1, p2, t1, t2);
    let a3 = mix(p2, p3, t2, t3);
    let b1 = mix(a1, a2, t0, t2);
    let b2 = mix(a2, a3, t1, t3);
    mix(b1, b2, t1, t2)
}

fn hash(seed: u32, i: i32) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B1) ^ seed.wrapping_mul(0x85EB_CA77);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Smooth value noise in -1..=1 at `x`, with zero slope at the lattice.
fn noise(seed: u32, x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let s = f * f * (3.0 - 2.0 * f);
    let i = i as i32;
    hash(seed, i) * (1.0 - s) + hash(seed, i + 1) * s
}

/// Two octaves of noise, about -1..=1.
fn drift(seed: u32, x: f32) -> f32 {
    (noise(seed, x) + 0.5 * noise(seed ^ 0x5bd1_e995, x * 2.13 + 17.0)) / 1.5
}

impl Shake {
    /// The eye and target shaken at time `t`.
    #[must_use]
    pub fn apply(&self, t: f32, eye: Vec3, target: Vec3) -> (Vec3, Vec3) {
        let x = t * self.frequency;
        let burst: f32 = self
            .bursts
            .iter()
            .filter(|b| t >= b.at)
            .map(|b| b.amplitude * (-(t - b.at) / b.decay).exp())
            .sum();
        // Bursts shake faster than the drift: about 9 Hz.
        let fast = t * 9.0;
        let yaw = self.amplitude * drift(self.seed, x) + burst * noise(self.seed ^ 0xA1, fast);
        let pitch = self.amplitude * drift(self.seed ^ 0x77, x + 31.0)
            + burst * noise(self.seed ^ 0xB2, fast + 5.0);
        let forward = (target - eye).normalize_or(Vec3::NEG_Z);
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(forward);
        let reach = eye.distance(target);
        let offset = Vec3::new(
            drift(self.seed ^ 0x13, x + 7.0),
            drift(self.seed ^ 0x29, x + 11.0),
            drift(self.seed ^ 0x3F, x + 13.0),
        ) * self.position
            + (right * noise(self.seed ^ 0xC3, fast + 9.0) + up * noise(self.seed ^ 0xD4, fast))
                * burst
                * 0.5;
        let aim = (forward + right * yaw.tan() + up * pitch.tan()).normalize();
        (eye + offset, eye + offset + aim * reach)
    }
}

/// The view-projection of `camera` seen through the point `lens` on its
/// lens, `lens` in the unit disk, and the eye it is seen from. Points at
/// the focus distance project to the same pixel from every point on the
/// lens; nearer and farther ones spread over their circle of confusion, so
/// the mean of frames over the lens has depth of field.
#[must_use]
pub fn lens_view_projection(
    camera: &CameraSample,
    aspect: f32,
    near: f32,
    far: f32,
    lens: Vec2,
) -> (Mat4, Vec3) {
    let forward = (camera.target - camera.eye).normalize_or(Vec3::NEG_Z);
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(forward);
    let o = lens * camera.aperture;
    let eye = camera.eye + right * o.x + up * o.y;
    let view = Mat4::look_to_rh(eye, forward, Vec3::Y);
    // Shear the view so the focal plane, at view depth -focus, keeps its
    // place: x' = x - o.x * z / focus.
    let shear = Mat4::from_cols(
        Vec4::X,
        Vec4::Y,
        Vec4::new(-o.x / camera.focus, -o.y / camera.focus, 1.0, 0.0),
        Vec4::W,
    );
    (
        Mat4::perspective_rh(camera.fov_y, aspect, near, far) * shear * view,
        eye,
    )
}

/// The `i`th of `n` points spread evenly over the unit disk (Vogel's
/// spiral), turned by `turn` radians.
#[must_use]
pub fn lens_sample(i: usize, n: usize, turn: f32) -> Vec2 {
    let n = n.max(1) as f32;
    let r = ((i as f32 + 0.5) / n).sqrt();
    let a = i as f32 * 2.399_963_1 + turn;
    Vec2::new(r * a.cos(), r * a.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(t: f32, eye: [f32; 3], ease: Ease) -> Key {
        Key {
            t,
            eye: Vec3::from(eye),
            target: Vec3::ZERO,
            focal_mm: 35.0,
            focus: None,
            f_stop: None,
            ease,
        }
    }

    fn rig() -> CameraRig {
        CameraRig {
            keys: vec![
                key(0.0, [0.0, 2.0, 10.0], Ease::Linear),
                key(2.0, [5.0, 2.0, 8.0], Ease::Linear),
                key(4.0, [8.0, 3.0, 3.0], Ease::InOut),
                key(6.0, [9.0, 3.0, -2.0], Ease::Linear),
            ],
            shake: None,
        }
    }

    #[test]
    fn the_path_passes_through_every_key_and_holds_outside_them() {
        let r = rig();
        for k in &r.keys {
            assert!(r.sample(k.t).eye.distance(k.eye) < 1e-4, "key at {}", k.t);
        }
        assert_eq!(r.sample(-3.0).eye, r.keys[0].eye);
        assert!(r.sample(60.0).eye.distance(r.keys[3].eye) < 1e-4);
    }

    #[test]
    fn the_path_is_continuous_across_keys() {
        let r = rig();
        for k in &r.keys[1..3] {
            let before = r.sample(k.t - 1e-3).eye;
            let after = r.sample(k.t + 1e-3).eye;
            assert!(before.distance(after) < 0.02);
        }
        // A smooth segment between keys bends toward the next key rather
        // than cutting the chord.
        let mid = r.sample(3.0).eye;
        let chord = (r.keys[1].eye + r.keys[2].eye) * 0.5;
        assert!(mid.distance(chord) > 1e-3);
    }

    #[test]
    fn easing_starts_and_ends_at_rest() {
        assert_eq!(Ease::InOut.apply(0.0), 0.0);
        assert_eq!(Ease::InOut.apply(1.0), 1.0);
        assert!((Ease::InOut.apply(0.5) - 0.5).abs() < 1e-6);
        assert!(Ease::InOut.apply(0.01) < 1e-4);
        assert!(Ease::In.apply(0.1) < 0.1 && Ease::Out.apply(0.1) > 0.1);
        let r = rig();
        // The InOut segment from t = 4 barely moves in its first 10 ms.
        let step = r.sample(4.01).eye.distance(r.sample(4.0).eye);
        let linear = r.sample(2.01).eye.distance(r.sample(2.0).eye);
        assert!(step < linear * 0.01);
    }

    #[test]
    fn centripetal_does_not_overshoot_close_keys() {
        // Two keys almost together between far ones: uniform Catmull-Rom
        // loops past them; centripetal stays near the chord.
        let p = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.1, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
        ];
        for i in 0..=10 {
            let q = centripetal(p[0], p[1], p[2], p[3], i as f32 / 10.0);
            assert!(q.x >= 9.99 && q.x <= 10.11, "{q}");
        }
    }

    #[test]
    fn lens_settings_interpolate() {
        let mut r = rig();
        r.keys[0].focal_mm = 24.0;
        r.keys[1].focal_mm = 96.0;
        r.keys[0].f_stop = Some(2.0);
        r.keys[1].f_stop = Some(4.0);
        let mid = r.sample(1.0);
        // Geometric: 24 to 96 passes 48 halfway.
        assert!((mid.fov_y - fov_y(48.0)).abs() < 1e-5);
        assert!((mid.aperture - 0.048 / 3.0 * 0.5).abs() < 1e-6);
        assert!(fov_y(18.0) > fov_y(50.0));
        assert_eq!(r.sample(5.0).aperture, 0.0);
    }

    #[test]
    fn the_focal_plane_stays_put_across_the_lens() {
        let camera = CameraSample {
            eye: Vec3::new(0.0, 1.5, 6.0),
            target: Vec3::new(0.0, 1.5, 0.0),
            fov_y: fov_y(50.0),
            focus: 6.0,
            aperture: 0.05,
        };
        let project = |m: Mat4, p: Vec3| {
            let c = m * p.extend(1.0);
            Vec2::new(c.x / c.w, c.y / c.w)
        };
        let (centre, _) = lens_view_projection(&camera, 1.78, 0.1, 100.0, Vec2::ZERO);
        let (edge, eye) = lens_view_projection(&camera, 1.78, 0.1, 100.0, Vec2::new(1.0, 0.0));
        assert!(eye.distance(camera.eye) > 0.049);
        let sharp = Vec3::new(0.7, 1.9, 0.0);
        assert!(project(centre, sharp).distance(project(edge, sharp)) < 1e-5);
        let far = Vec3::new(0.7, 1.9, -20.0);
        assert!(project(centre, far).distance(project(edge, far)) > 1e-3);
        for i in 0..32 {
            assert!(lens_sample(i, 32, 0.3).length() <= 1.0);
        }
    }

    #[test]
    fn shake_is_bounded_and_rigs_round_trip_through_json() {
        let mut r = rig();
        r.shake = Some(Shake {
            amplitude: 0.01,
            position: 0.02,
            frequency: 0.7,
            seed: 3,
            bursts: vec![Burst {
                at: 1.0,
                amplitude: 0.03,
                decay: 0.3,
            }],
        });
        r.validate().unwrap();
        let plain = rig();
        for i in 0..200 {
            let t = i as f32 * 0.03;
            let (a, b) = (r.sample(t), plain.sample(t));
            assert!(a.eye.distance(b.eye) < 0.1);
            let turn = (a.target - a.eye)
                .normalize()
                .angle_between((b.target - b.eye).normalize());
            assert!(turn < 0.1, "{turn}");
        }
        let json = serde_json::to_vec(&r).unwrap();
        assert_eq!(CameraRig::from_json(&json).unwrap(), r);
        let minimal = br#"{"keys":[{"t":0,"eye":[0,1,5],"target":[0,1,0]}]}"#;
        let one = CameraRig::from_json(minimal).unwrap();
        assert_eq!(one.sample(3.0).focus, 5.0);
        assert!(CameraRig::from_json(br#"{"keys":[]}"#).is_err());
        let unordered = br#"{"keys":[{"t":1,"eye":[0,1,5],"target":[0,1,0]},{"t":1,"eye":[0,1,4],"target":[0,1,0]}]}"#;
        assert!(CameraRig::from_json(unordered).is_err());
    }
}
