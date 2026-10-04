//! Active spell areas that push on bodies and characters inside them.
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The shape of a spell's area, world frame, m.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Area {
    /// A box turned `yaw` radians about the vertical through `center`.
    Box {
        center: DVec3,
        half: DVec3,
        yaw: f64,
    },
    /// An upright cylinder standing on `base`.
    Cylinder {
        base: DVec3,
        radius: f64,
        height: f64,
    },
    /// A wall along a ground polyline, `thickness` across and `height` up.
    Wall {
        points: Vec<DVec3>,
        thickness: f64,
        height: f64,
    },
    Sphere {
        center: DVec3,
        radius: f64,
    },
}

impl Area {
    pub fn contains(&self, p: DVec3) -> bool {
        match self {
            Self::Box { center, half, yaw } => {
                let local = glam::DQuat::from_rotation_y(-*yaw) * (p - *center);
                local.abs().cmple(*half).all()
            }
            Self::Cylinder {
                base,
                radius,
                height,
            } => {
                let d = p - *base;
                (0. ..=*height).contains(&d.y) && d.x * d.x + d.z * d.z <= radius * radius
            }
            Self::Wall {
                points,
                thickness,
                height,
            } => points.windows(2).any(|pair| {
                let (a, b) = (pair[0], pair[1]);
                let floor = a.y.min(b.y);
                if !(floor..=floor + *height).contains(&p.y) {
                    return false;
                }
                let flat = |v: DVec3| DVec3::new(v.x, 0., v.z);
                let (a, b, q) = (flat(a), flat(b), flat(p));
                let ab = b - a;
                let t = ((q - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0., 1.);
                q.distance(a + ab * t) <= thickness * 0.5
            }),
            Self::Sphere { center, radius } => p.distance_squared(*center) <= radius * radius,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let ok = match self {
            Self::Box { center, half, yaw } => {
                center.is_finite() && half.is_finite() && half.min_element() > 0. && yaw.is_finite()
            }
            Self::Cylinder {
                base,
                radius,
                height,
            } => base.is_finite() && *radius > 0. && *height > 0. && radius.is_finite(),
            Self::Wall {
                points,
                thickness,
                height,
            } => {
                (2..=64).contains(&points.len())
                    && points.iter().all(|p| p.is_finite())
                    && *thickness > 0.
                    && *height > 0.
                    && height.is_finite()
            }
            Self::Sphere { center, radius } => center.is_finite() && *radius > 0.,
        };
        if !ok {
            return Err("Invalid spell area".into());
        }
        Ok(())
    }
}

/// One active spell area: its owner, its cast, when it expires, and the
/// acceleration it adds to bodies and characters inside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpellField {
    pub cast: u64,
    pub spell: String,
    /// Scene actor ID of the caster.
    pub owner: u64,
    pub area: Area,
    /// m/s², world frame.
    pub acceleration: DVec3,
    /// Scene seconds.
    pub expires: f32,
    /// Ends with the caster's concentration.
    pub concentration: bool,
}

impl SpellField {
    /// What this field adds, on top of `gravity`, to a body at `pos` moving
    /// at `vel`: its constant acceleration, except in a Reverse Gravity
    /// cylinder's top band, where a hover spring replaces both.
    pub fn accel_at(&self, pos: DVec3, vel: DVec3, gravity: DVec3) -> DVec3 {
        match crate::reverse_gravity::game::hover_top(self) {
            Some(top) => {
                crate::reverse_gravity::game::field_accel(top, pos, vel, gravity, self.acceleration)
            }
            None => self.acceleration,
        }
    }
}

/// Gravity plus the acceleration of every field containing a point: the
/// `physics::world::Field` the spell world steps its bodies under.
pub struct Fields<'a> {
    pub gravity: DVec3,
    pub fields: &'a [SpellField],
}

impl Fields<'_> {
    /// The spell fields' share on a character, without gravity. Reverse
    /// Gravity moves characters itself (`crate::reverse_gravity::game`).
    pub fn spell_accel(&self, p: DVec3) -> DVec3 {
        self.fields
            .iter()
            .filter(|f| f.area.contains(p) && crate::reverse_gravity::game::hover_top(f).is_none())
            .map(|f| f.acceleration)
            .sum()
    }
}

impl physics::Field for Fields<'_> {
    fn accel(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        self.gravity
            + self
                .fields
                .iter()
                .filter(|f| f.area.contains(pos))
                .map(|f| f.accel_at(pos, vel, self.gravity))
                .sum::<DVec3>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_area_shape_contains_its_inside_and_not_its_outside() {
        let cube = Area::Box {
            center: DVec3::new(0., 1., 2.),
            half: DVec3::splat(1.),
            yaw: std::f64::consts::FRAC_PI_4,
        };
        assert!(cube.contains(DVec3::new(0., 1., 3.3)));
        assert!(!cube.contains(DVec3::new(1.0, 1., 3.0)));
        let cylinder = Area::Cylinder {
            base: DVec3::ZERO,
            radius: 2.,
            height: 3.,
        };
        assert!(cylinder.contains(DVec3::new(1.9, 2.9, 0.)) && !cylinder.contains(DVec3::Y * 3.1));
        let wall = Area::Wall {
            points: vec![DVec3::ZERO, DVec3::X * 5., DVec3::new(5., 0., 5.)],
            thickness: 0.4,
            height: 3.,
        };
        assert!(wall.contains(DVec3::new(2., 1., 0.15)) && wall.contains(DVec3::new(5.1, 1., 3.)));
        assert!(!wall.contains(DVec3::new(2., 1., 0.3)));
        let sphere = Area::Sphere {
            center: DVec3::ONE,
            radius: 1.,
        };
        assert!(sphere.contains(DVec3::new(1., 1.9, 1.)) && !sphere.contains(DVec3::ZERO));
        for area in [cube, cylinder, wall, sphere] {
            area.validate().unwrap();
        }
    }
}
