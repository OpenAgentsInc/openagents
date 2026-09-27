//! Free-molecular plume impingement: the push a thruster's exhaust gives
//! the surfaces it strikes.
//!
//! Far from a small nozzle in vacuum, the exhaust flows radially from a
//! point source. Its momentum flux (dynamic pressure) falls as `1/r²` and
//! with angle from the plume axis as `cos^n(π θ / (2 θ_max))`, the angular
//! form of Simons' model ("Effect of Nozzle Boundary Layers on Rocket
//! Exhaust Plumes", AIAA Journal 10(11), 1972), with `n = 2 / (γ − 1)`;
//! for nitrogen (γ = 1.4) `n = 5`. With `θ_max` at 90° the form is the
//! plain `cos^n θ` lobe. The flux is normalized so the momentum the gas
//! carries through any sphere about the source, as a vector, equals the
//! thrust along the axis: the gas takes away exactly the momentum the
//! thruster gave its vehicle.
//!
//! A surface element facing the source intercepts the gas crossing it. A
//! fraction of the gas reflects specularly; the rest is absorbed and
//! re-emitted diffusely with some fraction of its normal momentum
//! (accommodation). Box faces are integrated with a small midpoint rule,
//! refined on faces that are large compared with their distance; spheres
//! and capsules use their projected area. Surfaces do not shadow each
//! other.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::collision::{Collider, ColliderId, Shape};
use crate::ledger::Momentum;
use crate::world::{BodyId, World};

/// A thruster's exhaust as a point source.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plume {
    /// Nozzle exit, m.
    pub origin: DVec3,
    /// Exhaust direction, unit.
    pub axis: DVec3,
    /// Thrust, N.
    pub thrust: f64,
    /// Exponent `n` of the angular lobe.
    pub exponent: f64,
    /// Limiting angle `θ_max`, rad: no gas beyond it.
    pub limit: f64,
    /// Surfaces farther than this are ignored, m.
    pub range: f64,
}

/// Nearest distance the flux is evaluated at, m, to keep `1/r²` finite.
const NEAR: f64 = 0.05;

impl Plume {
    /// A cold nitrogen plume: `n = 5`, `θ_max = 90°`, 30 m range.
    #[must_use]
    pub fn nitrogen(origin: DVec3, axis: DVec3, thrust: f64) -> Self {
        Self {
            origin,
            axis: axis.normalize_or_zero(),
            thrust,
            exponent: 5.0,
            limit: std::f64::consts::FRAC_PI_2,
            range: 30.0,
        }
    }

    /// Angular shape of the lobe at angle `theta` from the axis.
    #[must_use]
    pub fn lobe(&self, theta: f64) -> f64 {
        if theta >= self.limit {
            return 0.0;
        }
        (std::f64::consts::FRAC_PI_2 * theta / self.limit)
            .cos()
            .max(0.0)
            .powf(self.exponent)
    }

    /// `2π ∫ lobe(θ) cos θ sin θ dθ` over the lobe, by Simpson's rule: the
    /// axial momentum the lobe carries per unit flux coefficient.
    #[must_use]
    pub fn normalization(&self) -> f64 {
        const INTERVALS: u32 = 256;
        let h = self.limit / f64::from(INTERVALS);
        let f = |t: f64| self.lobe(t) * t.cos() * t.sin();
        let mut sum = f(0.0) + f(self.limit);
        for i in 1..INTERVALS {
            let w = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += w * f(h * f64::from(i));
        }
        std::f64::consts::TAU * sum * h / 3.0
    }

    /// Momentum flux at `at`, Pa: the momentum per unit time and per unit
    /// area normal to the flow. The flow direction is away from the origin.
    #[must_use]
    pub fn flux(&self, at: DVec3) -> f64 {
        self.flux_with(at, self.normalization())
    }

    fn flux_with(&self, at: DVec3, norm: f64) -> f64 {
        let d = at - self.origin;
        let r = d.length().max(NEAR);
        if r > self.range || norm <= 0.0 {
            return 0.0;
        }
        let theta = (d.dot(self.axis) / r).clamp(-1.0, 1.0).acos();
        self.thrust * self.lobe(theta) / (norm * r * r)
    }
}

/// How gas leaves a surface it strikes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reflection {
    /// Fraction reflected specularly.
    pub specular: f64,
    /// Of the diffusely accommodated gas, the normal momentum it leaves with
    /// as a fraction of the normal momentum it arrived with.
    pub reemission: f64,
}

impl Reflection {
    /// Mostly diffuse, as on spacecraft paints, blankets, and cover glass.
    pub const DIFFUSE: Self = Self {
        specular: 0.1,
        reemission: 0.25,
    };
    /// Every molecule stops at the surface.
    pub const ABSORB: Self = Self {
        specular: 0.0,
        reemission: 0.0,
    };
}

impl Default for Reflection {
    fn default() -> Self {
        Self::DIFFUSE
    }
}

/// Force on one quadrature point of a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub collider: ColliderId,
    pub body: BodyId,
    /// World point, m.
    pub point: DVec3,
    /// Force, N.
    pub force: DVec3,
}

/// Total momentum a set of samples delivers over `dt`, about `origin`.
#[must_use]
pub fn momentum(samples: &[Sample], dt: f64, origin: DVec3) -> Momentum {
    samples.iter().fold(Momentum::ZERO, |sum, s| {
        sum + Momentum::impulse(s.force * dt, s.point, origin)
    })
}

/// Force on a flat element of area `area` and outward normal `normal` at
/// `point`, or zero when it faces away.
fn flat(plume: &Plume, norm: f64, point: DVec3, normal: DVec3, area: f64, r: Reflection) -> DVec3 {
    let d = point - plume.origin;
    let len = d.length();
    if len <= 0.0 {
        return DVec3::ZERO;
    }
    let incoming = d / len;
    let cos = -incoming.dot(normal);
    if cos <= 0.0 {
        return DVec3::ZERO;
    }
    let q = plume.flux_with(point, norm);
    if q == 0.0 {
        return DVec3::ZERO;
    }
    let s = r.specular.clamp(0.0, 1.0);
    let rate = q * cos * area;
    (incoming * (1.0 - s) - normal * ((1.0 - s) * r.reemission + 2.0 * s * cos)) * rate
}

/// Force on a convex curved body of projected area `area` at `point`:
/// absorbed momentum plus the diffuse re-emission averaged over the lit
/// hemisphere (two thirds of the normal momentum). Specular reflection off
/// a sphere adds no net force.
fn curved(plume: &Plume, norm: f64, point: DVec3, area: f64, r: Reflection) -> DVec3 {
    let d = point - plume.origin;
    let len = d.length();
    if len <= 0.0 {
        return DVec3::ZERO;
    }
    let q = plume.flux_with(point, norm);
    let s = r.specular.clamp(0.0, 1.0);
    d / len * (q * area * (1.0 + 2.0 / 3.0 * (1.0 - s) * r.reemission))
}

impl World {
    /// Forces a plume puts on every collider `accept` allows, sampled at
    /// quadrature points. Colliders beyond the plume's range or outside its
    /// lobe, judged by their bounding spheres, are skipped. Forces are
    /// returned, not applied; the caller decides which bodies they move.
    #[must_use]
    pub fn impinge(
        &self,
        plume: &Plume,
        reflection: Reflection,
        accept: &dyn Fn(&Collider) -> bool,
    ) -> Vec<Sample> {
        let mut samples = Vec::new();
        if plume.thrust <= 0.0 || plume.axis == DVec3::ZERO {
            return samples;
        }
        let norm = plume.normalization();
        for (index, collider) in self.colliders().iter().enumerate() {
            if !accept(collider) {
                continue;
            }
            let (pos, rotation) = collider.pose(self);
            let bound = collider.shape.bound();
            let to = pos - plume.origin;
            let distance = to.length();
            if distance - bound > plume.range {
                continue;
            }
            if distance > bound {
                let theta = (to.dot(plume.axis) / distance).clamp(-1.0, 1.0).acos();
                if theta - (bound / distance).asin() >= plume.limit {
                    continue;
                }
            }
            let id = ColliderId(index as u32);
            let mut push = |point: DVec3, force: DVec3| {
                if force != DVec3::ZERO {
                    samples.push(Sample {
                        collider: id,
                        body: collider.body,
                        point,
                        force,
                    });
                }
            };
            match collider.shape {
                Shape::Sphere { radius } => {
                    let area = std::f64::consts::PI * radius * radius;
                    push(pos, curved(plume, norm, pos, area, reflection));
                }
                Shape::Capsule {
                    radius,
                    half_length,
                } => {
                    let axis = rotation * DVec3::Z;
                    let piece = 2.0 * half_length / 3.0;
                    for k in [-1.0, 0.0, 1.0] {
                        let at = pos + axis * (piece * k);
                        let incoming = (at - plume.origin).normalize_or_zero();
                        let across = incoming.cross(axis).length();
                        let area = 2.0 * radius * piece * across;
                        push(at, curved(plume, norm, at, area, reflection));
                    }
                    // The end caps make one sphere, half at each end.
                    let area = std::f64::consts::FRAC_PI_2 * radius * radius;
                    for cap in [pos - axis * half_length, pos + axis * half_length] {
                        push(cap, curved(plume, norm, cap, area, reflection));
                    }
                }
                Shape::Cuboid { half } => {
                    let axes = DMat3::from_quat(rotation);
                    for face in 0..6 {
                        let k = face / 2;
                        let sign = if face % 2 == 0 { -1.0 } else { 1.0 };
                        let normal = axes.col(k) * sign;
                        let center = pos + normal * half[k];
                        // Faces that look away from the source get nothing.
                        if (center - plume.origin).dot(normal) >= 0.0 {
                            continue;
                        }
                        let (i, j) = ((k + 1) % 3, (k + 2) % 3);
                        let (u, v) = (axes.col(i), axes.col(j));
                        let (hu, hv) = (half[i], half[j]);
                        // Nearest point of the face to the source.
                        let rel = plume.origin - center;
                        let nearest =
                            center + u * rel.dot(u).clamp(-hu, hu) + v * rel.dot(v).clamp(-hv, hv);
                        let near = nearest.distance(plume.origin).max(NEAR);
                        if near > plume.range {
                            continue;
                        }
                        let cells = (4.0 * hu.max(hv) / near).ceil().clamp(2.0, 8.0) as u32;
                        let n = f64::from(cells);
                        let area = 4.0 * hu * hv / (n * n);
                        for a in 0..cells {
                            for b in 0..cells {
                                let su = (2.0 * f64::from(a) + 1.0) / n - 1.0;
                                let sv = (2.0 * f64::from(b) + 1.0) / n - 1.0;
                                let at = center + u * (su * hu) + v * (sv * hv);
                                push(at, flat(plume, norm, at, normal, area, reflection));
                            }
                        }
                    }
                }
            }
        }
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{Body, BodyKind};

    fn plate(world: &mut World, center: DVec3, half: DVec3) -> BodyId {
        let body = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(body, Shape::Cuboid { half }));
        body
    }

    #[test]
    fn the_lobe_carries_exactly_the_thrust() {
        for (n, limit) in [(5.0, 90.0_f64), (2.0, 90.0), (5.0, 130.0)] {
            let plume = Plume {
                exponent: n,
                limit: limit.to_radians(),
                ..Plume::nitrogen(DVec3::ZERO, DVec3::Z, 10.0)
            };
            // Integrate flux times the axial direction cosine over a sphere.
            let r = 3.0;
            let (steps_t, steps_p) = (2000, 64);
            let mut axial = 0.0;
            let mut lateral = DVec3::ZERO;
            for i in 0..steps_t {
                let theta = (f64::from(i) + 0.5) * std::f64::consts::PI / f64::from(steps_t);
                for j in 0..steps_p {
                    let phi = (f64::from(j) + 0.5) * std::f64::consts::TAU / f64::from(steps_p);
                    let dir = DVec3::new(
                        theta.sin() * phi.cos(),
                        theta.sin() * phi.sin(),
                        theta.cos(),
                    );
                    let da = r
                        * r
                        * theta.sin()
                        * (std::f64::consts::PI / f64::from(steps_t))
                        * (std::f64::consts::TAU / f64::from(steps_p));
                    let q = plume.flux(dir * r);
                    axial += q * dir.z * da;
                    lateral += DVec3::new(dir.x, dir.y, 0.0) * q * da;
                }
            }
            assert!((axial - 10.0).abs() < 1e-3, "n {n}, limit {limit}: {axial}");
            assert!(lateral.length() < 1e-9, "{lateral}");
        }
    }

    #[test]
    fn a_plate_across_the_axis_takes_the_dynamic_pressure() {
        let mut world = World::new(1.0 / 120.0);
        // A small plate 2 m down the plume, facing back at it.
        plate(
            &mut world,
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(0.05, 0.05, 0.01),
        );
        let plume = Plume::nitrogen(DVec3::ZERO, DVec3::Z, 10.0);
        let q = 10.0 * 7.0 / (std::f64::consts::TAU * 1.99 * 1.99);
        let area = 0.1 * 0.1;
        let absorbed: DVec3 = world
            .impinge(&plume, Reflection::ABSORB, &|_| true)
            .iter()
            .map(|s| s.force)
            .sum();
        assert!(
            (absorbed.z - q * area).abs() < 0.01 * q * area,
            "{absorbed} vs {}",
            q * area
        );
        let mirror = Reflection {
            specular: 1.0,
            reemission: 0.0,
        };
        let reflected: DVec3 = world
            .impinge(&plume, mirror, &|_| true)
            .iter()
            .map(|s| s.force)
            .sum();
        // Specular reflection at normal incidence doubles the push.
        assert!((reflected.z - 2.0 * absorbed.z).abs() < 1e-3 * absorbed.z);
    }

    #[test]
    fn an_enclosure_catches_the_whole_thrust() {
        let mut world = World::new(1.0 / 120.0);
        // Six walls 0.2 m thick around a 10 m room with the thruster inside.
        for k in 0..3 {
            for sign in [-1.0, 1.0] {
                let mut center = DVec3::ZERO;
                center[k] = sign * 5.1;
                // Inner faces tile the room's walls exactly.
                let mut half = DVec3::splat(5.0);
                half[k] = 0.1;
                plate(&mut world, center, half);
            }
        }
        let axis = DVec3::new(0.3, -0.2, 1.0).normalize();
        let plume = Plume::nitrogen(DVec3::new(1.0, 0.5, -0.5), axis, 10.0);
        let samples = world.impinge(&plume, Reflection::ABSORB, &|_| true);
        let total: DVec3 = samples.iter().map(|s| s.force).sum();
        assert!(
            (total - axis * 10.0).length() < 0.03 * 10.0,
            "the walls take the gas's momentum: {total}"
        );
        let caught = momentum(&samples, 0.5, DVec3::ZERO);
        assert_eq!(caught.linear, total * 0.5);
    }

    #[test]
    fn surfaces_behind_out_of_range_or_facing_away_are_left_alone() {
        let mut world = World::new(1.0 / 120.0);
        let behind = plate(&mut world, DVec3::new(0.0, 0.0, -3.0), DVec3::splat(0.5));
        let far = plate(&mut world, DVec3::new(0.0, 0.0, 60.0), DVec3::splat(0.5));
        let near = plate(&mut world, DVec3::new(0.0, 0.0, 3.0), DVec3::splat(0.5));
        let plume = Plume::nitrogen(DVec3::ZERO, DVec3::Z, 10.0);
        let samples = world.impinge(&plume, Reflection::DIFFUSE, &|_| true);
        assert!(samples.iter().all(|s| s.body == near));
        assert!(!samples.is_empty());
        // Only the face toward the source is struck, and it is pushed away.
        assert!(
            samples
                .iter()
                .all(|s| (s.point.z - 2.5).abs() < 1e-9 && s.force.z > 0.0)
        );
        let excluded = world.impinge(&plume, Reflection::DIFFUSE, &|c| c.body != near);
        assert!(excluded.is_empty());
        let _ = (behind, far);
    }

    #[test]
    fn a_sphere_and_a_capsule_take_their_projected_area() {
        let mut world = World::new(1.0 / 120.0);
        let ball = world.add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.0, 4.0)));
        world.add_collider(Collider::new(ball, Shape::Sphere { radius: 0.1 }));
        let plume = Plume::nitrogen(DVec3::ZERO, DVec3::Z, 10.0);
        let force: DVec3 = world
            .impinge(&plume, Reflection::ABSORB, &|_| true)
            .iter()
            .map(|s| s.force)
            .sum();
        let expected = plume.flux(DVec3::new(0.0, 0.0, 4.0)) * std::f64::consts::PI * 0.01;
        assert!((force.z - expected).abs() < 1e-12 * expected.max(1.0));
        let mut world = World::new(1.0 / 120.0);
        let tank = world.add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.0, 6.0)));
        // Lying across the plume.
        world.add_collider(
            Collider::new(
                tank,
                Shape::Capsule {
                    radius: 0.5,
                    half_length: 1.0,
                },
            )
            .at(
                DVec3::ZERO,
                glam::DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
            ),
        );
        let force: DVec3 = world
            .impinge(&plume, Reflection::ABSORB, &|_| true)
            .iter()
            .map(|s| s.force)
            .sum();
        assert!(force.z > 0.0 && force.x.abs() < 1e-9 * force.z, "{force}");
    }
}
