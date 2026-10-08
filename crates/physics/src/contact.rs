//! Sequential-impulse contact solver: non-penetration with restitution and
//! position correction, Coulomb friction on an elliptic cone (the tangential
//! impulse's magnitude, not each axis, is bounded by the friction limit),
//! and torsional friction about the normal (Genesis
//! `examples/rigid/friction_breakaway.py`, `torsional_grasp.py`).
//!
//! Each contact point starts from the impulses it carried last step (warm
//! starting, matched by collider pair and position), so tall stacks converge
//! and settle instead of creeping.
//!
//! Impulses act equally and oppositely at each contact point, so contacts
//! between moving bodies conserve linear and angular momentum.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::collision::{ColliderId, Manifold};
use crate::joint::JointKind;
use crate::world::{BodyId, World};

/// Solver tuning. Named, so a scene that needs a stiffer contact (the tight
/// thread in Genesis `bolt_nut_self_screw.py`) states it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverSettings {
    /// Velocity iterations per step.
    pub iterations: u32,
    /// Fraction of penetration beyond `slop` removed per step.
    pub erp: f64,
    /// Penetration tolerated without correction, m.
    pub slop: f64,
    /// Closing speed below which contacts do not bounce, m/s. Well above
    /// what one step of gravity adds (about 0.1 m/s at 100 Hz), so resting
    /// contacts do not chatter.
    pub bounce_threshold: f64,
    /// Contacts start this far before touching, m, plus the distance the
    /// pair can close in one step.
    pub margin: f64,
}

impl Default for SolverSettings {
    fn default() -> Self {
        Self {
            iterations: 20,
            erp: 0.2,
            slop: 0.005,
            bounce_threshold: 0.5,
            margin: 0.01,
        }
    }
}

/// A contact point's accumulated impulses from the last step, kept to warm
/// start the next one. Matched by collider pair and position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WarmContact {
    pub a: ColliderId,
    pub b: ColliderId,
    pub point: DVec3,
    pub normal: f64,
    /// World-frame tangential impulse, so it survives a change of basis.
    pub tangent: DVec3,
    pub twist: f64,
}

fn warm_pairs(
    warm: &[WarmContact],
) -> std::collections::HashMap<(ColliderId, ColliderId), Vec<&WarmContact>> {
    let mut pairs = std::collections::HashMap::<_, Vec<_>>::new();
    for contact in warm {
        pairs
            .entry((contact.a, contact.b))
            .or_default()
            .push(contact);
    }
    pairs
}

fn nearest_warm(pair: &[&WarmContact], point: DVec3) -> Option<WarmContact> {
    pair.iter()
        .map(|w| (w.point.distance_squared(point), *w))
        .filter(|(distance, _)| *distance <= WARM_RADIUS * WARM_RADIUS)
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, contact)| *contact)
}

/// A cached contact matches a new one within this distance, m.
const WARM_RADIUS: f64 = 0.02;

/// What one contact point did in the last step.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContactReport {
    pub a: ColliderId,
    pub b: ColliderId,
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub point: DVec3,
    /// From `a` to `b`.
    pub normal: DVec3,
    pub separation: f64,
    /// Impulse on `b` at `point`, N s; `a` received the opposite.
    pub impulse: DVec3,
    /// Angular impulse on `b` from torsional friction, N m s; `a` received
    /// the opposite.
    pub twist: DVec3,
}

/// Velocity state of one body during the solve, world frame.
#[derive(Clone, Copy)]
struct Motion {
    vel: DVec3,
    omega: DVec3,
    inverse_mass: f64,
    inverse_inertia: DMat3,
    revision: u64,
}

fn vector_bits(v: DVec3) -> [u64; 3] {
    v.to_array().map(f64::to_bits)
}

impl Motion {
    fn point_velocity(&self, r: DVec3) -> DVec3 {
        self.vel + self.omega.cross(r)
    }

    fn push(&mut self, impulse: DVec3, r: DVec3) {
        if self.inverse_mass == 0.0 {
            return;
        }
        let old = (self.vel, self.omega);
        self.vel += impulse * self.inverse_mass;
        self.omega += self.inverse_inertia * r.cross(impulse);
        self.record_change(old);
    }

    fn twist(&mut self, angular: DVec3) {
        if self.inverse_mass == 0.0 {
            return;
        }
        let old = vector_bits(self.omega);
        self.omega += self.inverse_inertia * angular;
        if vector_bits(self.omega) != old {
            self.revision += 1;
        }
    }

    fn push_axis(&mut self, impulse: DVec3, angular: DVec3, delta: f64) {
        if self.inverse_mass == 0.0 {
            return;
        }
        let old = (self.vel, self.omega);
        self.vel += impulse * self.inverse_mass;
        self.omega += angular * delta;
        self.record_change(old);
    }

    fn push_tangents(&mut self, impulse: DVec3, angular: [DVec3; 2], delta: [f64; 2]) {
        if self.inverse_mass == 0.0 {
            return;
        }
        let old = (self.vel, self.omega);
        self.vel += impulse * self.inverse_mass;
        self.omega += angular[0] * delta[0] + angular[1] * delta[1];
        self.record_change(old);
    }

    fn record_change(&mut self, old: (DVec3, DVec3)) {
        if vector_bits(self.vel) != vector_bits(old.0)
            || vector_bits(self.omega) != vector_bits(old.1)
        {
            self.revision += 1;
        }
    }
}

#[derive(Clone, Copy)]
struct ContactAxis {
    arms: [DVec3; 2],
    angular: [DVec3; 2],
    mass: f64,
}

impl ContactAxis {
    fn new(a: &Motion, b: &Motion, ra: DVec3, rb: DVec3, direction: DVec3) -> Self {
        let arms = [ra.cross(direction), rb.cross(direction)];
        let angular = [a.inverse_inertia * arms[0], b.inverse_inertia * arms[1]];
        let k = a.inverse_mass + b.inverse_mass + arms[0].dot(angular[0]) + arms[1].dot(angular[1]);
        Self {
            arms,
            angular,
            mass: if k > 0.0 { 1.0 / k } else { 0.0 },
        }
    }

    fn velocity(&self, direction: DVec3, linear: DVec3, a: DVec3, b: DVec3) -> f64 {
        direction.dot(linear) + self.arms[1].dot(b) - self.arms[0].dot(a)
    }
}

struct Row {
    a: usize,
    b: usize,
    ra: DVec3,
    rb: DVec3,
    normal: DVec3,
    tangents: [DVec3; 2],
    axes: [ContactAxis; 3],
    twist_mass: f64,
    target: f64,
    friction: f64,
    torsional: f64,
    normal_impulse: f64,
    tangent_impulse: [f64; 2],
    twist_impulse: f64,
    fixed_at: Option<[u64; 2]>,
    report: ContactReport,
}

#[inline(always)]
fn friction_inside(next: [f64; 2], limit: f64) -> bool {
    if next[0] == 0.0 && next[1] == 0.0 {
        return limit >= 0.0 && limit.is_finite();
    }
    if limit <= 0.0 {
        return false;
    }
    let squares = [next[0] * next[0], next[1] * next[1]];
    let size_squared = squares[0] + squares[1];
    let limit_squared = limit * limit;
    // Stay well inside the cone, beyond squared-arithmetic and hypot
    // rounding. Underflow, overflow, and boundary cases retain hypot.
    size_squared.is_normal()
        && limit_squared.is_normal()
        && (squares[0].is_normal() || next[0] == 0.0)
        && (squares[1].is_normal() || next[1] == 0.0)
        && size_squared < limit_squared * (1.0 - 32.0 * f64::EPSILON)
}

#[inline(always)]
fn project_friction<const GUARDED: bool>(mut next: [f64; 2], limit: f64) -> [f64; 2] {
    if GUARDED && friction_inside(next, limit) {
        return next;
    }
    let size = next[0].hypot(next[1]);
    if size > limit {
        let scale = if size > 0.0 { limit / size } else { 0.0 };
        next = [next[0] * scale, next[1] * scale];
    }
    next
}

impl Row {
    #[inline(always)]
    fn solve<const OPTIMIZED: bool>(&mut self, motions: &mut [Motion]) -> bool {
        let (a, b) = (self.a, self.b);
        let revisions = [motions[a].revision, motions[b].revision];
        if OPTIMIZED && self.fixed_at == Some(revisions) {
            return true;
        }
        let old_impulses = [
            self.normal_impulse,
            self.tangent_impulse[0],
            self.tangent_impulse[1],
            self.twist_impulse,
        ]
        .map(f64::to_bits);
        // Friction first, bounded by the current normal impulse.
        let relative = motions[b].vel - motions[a].vel;
        let tangent_speed = [
            self.axes[1].velocity(
                self.tangents[0],
                relative,
                motions[a].omega,
                motions[b].omega,
            ),
            self.axes[2].velocity(
                self.tangents[1],
                relative,
                motions[a].omega,
                motions[b].omega,
            ),
        ];
        let limit = self.friction * self.normal_impulse;
        let old = self.tangent_impulse;
        let next = [
            old[0] - tangent_speed[0] * self.axes[1].mass,
            old[1] - tangent_speed[1] * self.axes[2].mass,
        ];
        let next = project_friction::<OPTIMIZED>(next, limit);
        self.tangent_impulse = next;
        let delta = self.tangents[0] * (next[0] - old[0]) + self.tangents[1] * (next[1] - old[1]);
        motions[a].push_tangents(
            -delta,
            [self.axes[1].angular[0], self.axes[2].angular[0]],
            [old[0] - next[0], old[1] - next[1]],
        );
        motions[b].push_tangents(
            delta,
            [self.axes[1].angular[1], self.axes[2].angular[1]],
            [next[0] - old[0], next[1] - old[1]],
        );
        if self.torsional > 0.0 {
            let spin = (motions[b].omega - motions[a].omega).dot(self.normal);
            let limit = self.torsional * self.normal_impulse;
            let old = self.twist_impulse;
            let next = (old - spin * self.twist_mass).clamp(-limit, limit);
            self.twist_impulse = next;
            let delta = self.normal * (next - old);
            motions[a].twist(-delta);
            motions[b].twist(delta);
        }
        let speed = self.axes[0].velocity(
            self.normal,
            motions[b].vel - motions[a].vel,
            motions[a].omega,
            motions[b].omega,
        );
        let old = self.normal_impulse;
        let next = (old + (self.target - speed) * self.axes[0].mass).max(0.0);
        self.normal_impulse = next;
        let delta = self.normal * (next - old);
        motions[a].push_axis(-delta, self.axes[0].angular[0], old - next);
        motions[b].push_axis(delta, self.axes[0].angular[1], next - old);
        let impulses = [
            self.normal_impulse,
            self.tangent_impulse[0],
            self.tangent_impulse[1],
            self.twist_impulse,
        ]
        .map(f64::to_bits);
        // A changing row must run again: its new normal impulse changes
        // the friction limits even if no other constraint touches it.
        self.fixed_at = (impulses == old_impulses
            && revisions == [motions[a].revision, motions[b].revision])
        .then_some(revisions);
        false
    }
}

#[derive(Default)]
struct SolveWork {
    #[cfg(test)]
    skipped_rows: usize,
}

/// A tether: one scalar constraint along the line between its anchors.
struct JointRow {
    a: usize,
    b: usize,
    ra: DVec3,
    rb: DVec3,
    dir: DVec3,
    /// 1 / (K + gamma).
    mass: f64,
    bias: f64,
    gamma: f64,
    lower: f64,
    upper: f64,
    impulse: f64,
}

impl JointRow {
    fn velocity(&self, motions: &[Motion]) -> f64 {
        let (a, b) = (&motions[self.a], &motions[self.b]);
        self.dir
            .dot(b.point_velocity(self.rb) - a.point_velocity(self.ra))
    }

    fn apply(&self, motions: &mut [Motion], delta: f64) {
        let d = self.dir * delta;
        motions[self.a].push(-d, self.ra);
        motions[self.b].push(d, self.rb);
    }

    fn solve(&mut self, motions: &mut [Motion]) {
        let v = self.velocity(motions);
        let old = self.impulse;
        let next =
            (old - (v + self.bias + self.gamma * old) * self.mass).clamp(self.lower, self.upper);
        self.impulse = next;
        self.apply(motions, next - old);
    }
}

/// Solve `k x = b` for a symmetric positive-definite `n`-by-`n` system by
/// Gaussian elimination with partial pivoting; `None` if singular.
fn solve_dense(k: &[[f64; 6]; 6], b: &[f64; 6], n: usize) -> Option<[f64; 6]> {
    let mut m = *k;
    let mut x = *b;
    for col in 0..n {
        let pivot = (col..n).max_by(|i, j| m[*i][col].abs().total_cmp(&m[*j][col].abs()))?;
        if m[pivot][col].abs() < 1e-300 {
            return None;
        }
        m.swap(col, pivot);
        x.swap(col, pivot);
        for row in (col + 1)..n {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (target, source) in m[row][col..n].iter_mut().zip(&pivot_row[col..n]) {
                *target -= f * source;
            }
            x[row] -= f * x[col];
        }
    }
    for col in (0..n).rev() {
        let mut sum = x[col];
        for c in (col + 1)..n {
            sum -= m[col][c] * x[c];
        }
        x[col] = sum / m[col][col];
    }
    Some(x)
}

/// Cross-product matrix: `skew(r) * v == r.cross(v)`.
fn skew(r: DVec3) -> DMat3 {
    DMat3::from_cols(
        DVec3::new(0.0, r.z, -r.y),
        DVec3::new(-r.z, 0.0, r.x),
        DVec3::new(r.y, -r.x, 0.0),
    )
}

/// A point (three linear rows) or weld (three linear and three angular
/// rows) solved as one block with its full effective-mass matrix, so a soft
/// joint's damping ratio holds for every coupled mode.
struct BlockJoint {
    joint: usize,
    a: usize,
    b: usize,
    ra: DVec3,
    rb: DVec3,
    /// 3 for a point joint, 6 for a weld.
    size: usize,
    k: [[f64; 6]; 6],
    /// Position error: linear, then angular.
    error: [f64; 6],
    bias_rate: f64,
    mass_scale: f64,
    impulse_scale: f64,
    impulse: [f64; 6],
    force_limit: f64,
    torque_limit: f64,
}

impl BlockJoint {
    fn velocity(&self, motions: &[Motion]) -> [f64; 6] {
        let (a, b) = (&motions[self.a], &motions[self.b]);
        let linear = b.point_velocity(self.rb) - a.point_velocity(self.ra);
        let angular = b.omega - a.omega;
        [
            linear.x, linear.y, linear.z, angular.x, angular.y, angular.z,
        ]
    }

    fn apply(&self, motions: &mut [Motion], delta: &[f64; 6]) {
        let p = DVec3::new(delta[0], delta[1], delta[2]);
        let l = DVec3::new(delta[3], delta[4], delta[5]);
        motions[self.a].push(-p, self.ra);
        motions[self.b].push(p, self.rb);
        if self.size == 6 {
            motions[self.a].twist(-l);
            motions[self.b].twist(l);
        }
    }

    fn solve(&mut self, motions: &mut [Motion]) {
        let v = self.velocity(motions);
        let mut rhs = [0.0; 6];
        for i in 0..self.size {
            rhs[i] = v[i] + self.bias_rate * self.error[i];
        }
        let Some(x) = solve_dense(&self.k, &rhs, self.size) else {
            return;
        };
        let mut next = self.impulse;
        for i in 0..self.size {
            next[i] =
                self.impulse[i] - self.mass_scale * x[i] - self.impulse_scale * self.impulse[i];
        }
        // Force and torque limits cap the accumulated impulse's magnitude.
        for (range, limit) in [(0..3, self.force_limit), (3..6, self.torque_limit)] {
            let size = next[range.clone()]
                .iter()
                .map(|x| x * x)
                .sum::<f64>()
                .sqrt();
            if size > limit {
                let scale = limit / size;
                next[range].iter_mut().for_each(|x| *x *= scale);
            }
        }
        let mut delta = [0.0; 6];
        for i in 0..self.size {
            delta[i] = next[i] - self.impulse[i];
        }
        self.impulse = next;
        self.apply(motions, &delta);
    }

    fn saturated(&self) -> bool {
        let norm =
            |r: std::ops::Range<usize>| self.impulse[r].iter().map(|x| x * x).sum::<f64>().sqrt();
        norm(0..3) >= self.force_limit * (1.0 - 1e-9)
            || (self.size == 6 && norm(3..6) >= self.torque_limit * (1.0 - 1e-9))
    }
}

/// Soft-step coefficients for a spring of natural frequency `omega` rad/s
/// and damping ratio `zeta` over a step `dt` (Catto's mass-independent soft
/// constraint): bias rate, mass scale, impulse scale.
fn soft_step(omega: f64, zeta: f64, dt: f64) -> (f64, f64, f64) {
    let a1 = 2.0 * zeta + dt * omega;
    let a2 = dt * omega * a1;
    let a3 = 1.0 / (1.0 + a2);
    (omega / a1, a2 * a3, a3)
}

fn basis(n: DVec3) -> [DVec3; 2] {
    let t = if n.x.abs() < 0.57 {
        DVec3::X.cross(n)
    } else {
        DVec3::Y.cross(n)
    }
    .normalize();
    [t, n.cross(t)]
}

fn effective_mass(ma: &Motion, mb: &Motion, ra: DVec3, rb: DVec3, d: DVec3) -> f64 {
    let (ca, cb) = (ra.cross(d), rb.cross(d));
    let k = ma.inverse_mass
        + mb.inverse_mass
        + ca.dot(ma.inverse_inertia * ca)
        + cb.dot(mb.inverse_inertia * cb);
    if k > 0.0 { 1.0 / k } else { 0.0 }
}

impl World {
    /// Resolve joints and `manifolds` by changing body velocities for a step
    /// of `dt`. Records each joint's impulses and returns what each contact
    /// point did.
    pub(crate) fn solve(&mut self, manifolds: &[Manifold], dt: f64) -> Vec<ContactReport> {
        self.solve_with_optimizations::<true>(manifolds, dt).0
    }

    fn solve_with_optimizations<const OPTIMIZED: bool>(
        &mut self,
        manifolds: &[Manifold],
        dt: f64,
    ) -> (Vec<ContactReport>, SolveWork) {
        #[allow(unused_mut)]
        let mut work = SolveWork::default();
        let settings = self.solver;
        let mut motions: Vec<Motion> = self
            .bodies()
            .iter()
            .map(|b| {
                let inverse_mass = b.inverse_mass();
                let inverse = if inverse_mass > 0.0 {
                    let r = DMat3::from_quat(b.orientation);
                    r * DMat3::from_diagonal(b.inertia.recip()) * r.transpose()
                } else {
                    DMat3::ZERO
                };
                Motion {
                    vel: b.vel,
                    omega: b.omega_world(),
                    inverse_mass,
                    inverse_inertia: inverse,
                    revision: 0,
                }
            })
            .collect();
        let mut rows = Vec::new();
        let warm_pairs = warm_pairs(&self.warm);
        let mut warm_candidates = 0;
        for manifold in manifolds {
            let warm_pair = warm_pairs
                .get(&(manifold.a, manifold.b))
                .map_or(&[][..], Vec::as_slice);
            let (ca, cb) = (
                self.colliders()[manifold.a.0 as usize],
                self.colliders()[manifold.b.0 as usize],
            );
            let (a, b) = (ca.body.0 as usize, cb.body.0 as usize);
            let friction = (ca.material.friction * cb.material.friction).sqrt();
            let torsional = (ca.material.torsional * cb.material.torsional).sqrt();
            let restitution = ca.material.restitution.max(cb.material.restitution);
            for c in &manifold.points {
                let ra = c.point - self.bodies()[a].pos;
                let rb = c.point - self.bodies()[b].pos;
                let (ma, mb) = (&motions[a], &motions[b]);
                let closing = (mb.point_velocity(rb) - ma.point_velocity(ra)).dot(c.normal);
                // Bounce when the surfaces meet within this step, including
                // a speculative contact about to close; otherwise a fast body
                // stopped at the surface one step early would lose it.
                let meets = c.separation <= 0.0 || -closing * dt >= c.separation;
                let bounce = if meets && closing < -settings.bounce_threshold {
                    -restitution * closing
                } else {
                    0.0
                };
                let target = if c.separation > 0.0 {
                    // Speculative: allow closing until the surfaces meet, or
                    // bounce if they meet within the step.
                    if bounce > 0.0 {
                        bounce
                    } else {
                        -c.separation / dt
                    }
                } else {
                    let push = settings.erp * (-c.separation - settings.slop).max(0.0) / dt;
                    push.max(bounce)
                };
                let tangents = basis(c.normal);
                let twist_k = c.normal.dot(ma.inverse_inertia * c.normal)
                    + c.normal.dot(mb.inverse_inertia * c.normal);
                // Warm start from last step's nearest point on this pair.
                warm_candidates += warm_pair.len();
                let warm = nearest_warm(warm_pair, c.point);
                rows.push(Row {
                    a,
                    b,
                    ra,
                    rb,
                    normal: c.normal,
                    tangents,
                    axes: [
                        ContactAxis::new(ma, mb, ra, rb, c.normal),
                        ContactAxis::new(ma, mb, ra, rb, tangents[0]),
                        ContactAxis::new(ma, mb, ra, rb, tangents[1]),
                    ],
                    twist_mass: if twist_k > 0.0 { 1.0 / twist_k } else { 0.0 },
                    target,
                    friction,
                    torsional,
                    normal_impulse: warm.map_or(0.0, |w| w.normal),
                    tangent_impulse: warm.map_or([0.0; 2], |w| {
                        [w.tangent.dot(tangents[0]), w.tangent.dot(tangents[1])]
                    }),
                    twist_impulse: warm.map_or(0.0, |w| w.twist),
                    fixed_at: None,
                    report: ContactReport {
                        a: manifold.a,
                        b: manifold.b,
                        body_a: ca.body,
                        body_b: cb.body,
                        point: c.point,
                        normal: c.normal,
                        separation: c.separation,
                        impulse: DVec3::ZERO,
                        twist: DVec3::ZERO,
                    },
                });
            }
        }
        drop(warm_pairs);
        // Apply the warm-start impulses before iterating; the iterations can
        // take them back, since each row's accumulated impulse starts there.
        for row in &rows {
            let impulse = row.normal * row.normal_impulse
                + row.tangents[0] * row.tangent_impulse[0]
                + row.tangents[1] * row.tangent_impulse[1];
            motions[row.a].push(-impulse, row.ra);
            motions[row.b].push(impulse, row.rb);
            let twist = row.normal * row.twist_impulse;
            motions[row.a].twist(-twist);
            motions[row.b].twist(twist);
        }
        let (mut tethers, mut blocks) = self.joint_groups(&motions, dt);
        for _ in 0..settings.iterations {
            for block in &mut blocks {
                block.solve(&mut motions);
            }
            for (_, row) in &mut tethers {
                row.solve(&mut motions);
            }
            for row in &mut rows {
                if row.solve::<OPTIMIZED>(&mut motions) {
                    #[cfg(test)]
                    {
                        work.skipped_rows += 1;
                    }
                }
            }
        }
        for (index, row) in &tethers {
            let point = self.bodies()[row.b].pos + row.rb;
            if let Some(joint) = self.joints[*index].as_mut() {
                joint.impulse = row.dir * row.impulse;
                joint.point = point;
                joint.angular_impulse = DVec3::ZERO;
                joint.saturated = row.lower.is_finite() && row.impulse <= row.lower * (1.0 - 1e-9);
            }
        }
        for block in &blocks {
            let point = self.bodies()[block.b].pos + block.rb;
            if let Some(joint) = self.joints[block.joint].as_mut() {
                joint.impulse = DVec3::new(block.impulse[0], block.impulse[1], block.impulse[2]);
                joint.point = point;
                joint.angular_impulse =
                    DVec3::new(block.impulse[3], block.impulse[4], block.impulse[5]);
                joint.saturated = block.saturated();
            }
        }
        for (body, motion) in self.bodies_mut().iter_mut().zip(&motions) {
            if motion.inverse_mass > 0.0 {
                body.vel = motion.vel;
                body.omega = body.orientation.inverse() * motion.omega;
            }
        }
        self.stats.warm_candidates = warm_candidates;
        self.warm = rows
            .iter()
            .map(|row| WarmContact {
                a: row.report.a,
                b: row.report.b,
                point: row.report.point,
                normal: row.normal_impulse,
                tangent: row.tangents[0] * row.tangent_impulse[0]
                    + row.tangents[1] * row.tangent_impulse[1],
                twist: row.twist_impulse,
            })
            .collect();
        let reports = rows
            .into_iter()
            .map(|row| ContactReport {
                impulse: row.normal * row.normal_impulse
                    + row.tangents[0] * row.tangent_impulse[0]
                    + row.tangents[1] * row.tangent_impulse[1],
                twist: row.normal * row.twist_impulse,
                ..row.report
            })
            .collect();
        (reports, work)
    }

    fn joint_groups(
        &self,
        motions: &[Motion],
        dt: f64,
    ) -> (Vec<(usize, JointRow)>, Vec<BlockJoint>) {
        let erp = self.solver.erp;
        let mut tethers = Vec::new();
        let mut blocks = Vec::new();
        for (index, joint) in self.joints.iter().enumerate() {
            let Some(joint) = joint else { continue };
            let (a, b) = (joint.a.0 as usize, joint.b.0 as usize);
            if motions[a].inverse_mass == 0.0 && motions[b].inverse_mass == 0.0 {
                // Neither side can move: nothing to solve.
                continue;
            }
            let (pa, pb) = joint.anchors(self);
            // Point and weld impulses act at the anchors' midpoint, so the
            // pair is equal, opposite, and collinear: momentum is exact even
            // while the anchors are apart. A tether pulls along the line
            // between its anchors, which is already collinear.
            let (qa, qb) = match joint.kind {
                JointKind::Tether { .. } => (pa, pb),
                JointKind::Point | JointKind::Weld { .. } => {
                    let mid = (pa + pb) * 0.5;
                    (mid, mid)
                }
            };
            let (ra, rb) = (qa - self.bodies()[a].pos, qb - self.bodies()[b].pos);
            let (ma, mb) = (&motions[a], &motions[b]);
            // Row parameters from the effective mass `m`, the position error
            // `c`, and the joint's spring, if any.
            let row = |dir: DVec3, c: f64, lower: f64, upper: f64| {
                let k = 1.0 / effective_mass(ma, mb, ra, rb, dir).max(f64::MIN_POSITIVE);
                if !(k.is_finite() && k > 0.0) {
                    return None;
                }
                let (bias, gamma) = match joint.spring {
                    Some(spring) => {
                        let m = 1.0 / k;
                        let stiffness = m * spring.frequency * spring.frequency;
                        let damping = 2.0 * m * spring.damping_ratio * spring.frequency;
                        let denominator = damping + dt * stiffness;
                        (c * stiffness / denominator, 1.0 / (dt * denominator))
                    }
                    None => (c * erp / dt, 0.0),
                };
                Some(JointRow {
                    a,
                    b,
                    ra,
                    rb,
                    dir,
                    mass: 1.0 / (k + gamma),
                    bias,
                    gamma,
                    lower,
                    upper,
                    impulse: 0.0,
                })
            };
            let force_limit = joint.max_force * dt;
            let torque_limit = joint.max_torque * dt;
            match joint.kind {
                JointKind::Point | JointKind::Weld { .. } => {
                    let size = if matches!(joint.kind, JointKind::Weld { .. }) {
                        6
                    } else {
                        3
                    };
                    let (ia, ib) = (ma.inverse_inertia, mb.inverse_inertia);
                    let (sa, sb) = (skew(ra), skew(rb));
                    let ll = DMat3::from_diagonal(DVec3::splat(ma.inverse_mass + mb.inverse_mass))
                        - sb * ib * sb
                        - sa * ia * sa;
                    let la = -(sb * ib) - sa * ia;
                    let al = ib * sb + ia * sa;
                    let aa = ia + ib;
                    let mut k = [[0.0; 6]; 6];
                    for r in 0..3 {
                        for c in 0..3 {
                            k[r][c] = ll.col(c)[r];
                            k[r][c + 3] = la.col(c)[r];
                            k[r + 3][c] = al.col(c)[r];
                            k[r + 3][c + 3] = aa.col(c)[r];
                        }
                    }
                    let linear_error = pb - pa;
                    let angular_error = joint.angle_error(self);
                    let (bias_rate, mass_scale, impulse_scale) = match joint.spring {
                        Some(spring) => soft_step(spring.frequency, spring.damping_ratio, dt),
                        None => (erp / dt, 1.0, 0.0),
                    };
                    blocks.push(BlockJoint {
                        joint: index,
                        a,
                        b,
                        ra,
                        rb,
                        size,
                        k,
                        error: [
                            linear_error.x,
                            linear_error.y,
                            linear_error.z,
                            angular_error.x,
                            angular_error.y,
                            angular_error.z,
                        ],
                        bias_rate,
                        mass_scale,
                        impulse_scale,
                        impulse: [0.0; 6],
                        force_limit,
                        torque_limit,
                    });
                }
                JointKind::Tether { length } => {
                    let offset = pb - pa;
                    let distance = offset.length();
                    if distance > 1e-9 {
                        let dir = offset / distance;
                        let stretch = distance - length;
                        // Slack: allow closing on the limit within this step.
                        if let Some(mut r) = row(dir, stretch, -force_limit, 0.0) {
                            if stretch < 0.0 {
                                r.bias = stretch / dt;
                                r.mass = 1.0 / (1.0 / r.mass - r.gamma);
                                r.gamma = 0.0;
                            }
                            tethers.push((index, r));
                        }
                    }
                }
            }
        }
        (tethers, blocks)
    }
}

#[cfg(test)]
mod warm_tests {
    use super::*;

    fn assert_friction_projection(next: [f64; 2], limit: f64) {
        let expected = project_friction::<false>(next, limit);
        let actual = project_friction::<true>(next, limit);
        assert_eq!(
            actual.map(f64::to_bits),
            expected.map(f64::to_bits),
            "projection of {next:?} with limit {limit:?}"
        );
        if friction_inside(next, limit) {
            assert!(next[0].hypot(next[1]) <= limit);
            assert_eq!(actual.map(f64::to_bits), next.map(f64::to_bits));
        }
    }

    #[test]
    fn friction_interior_preserves_bits_and_boundaries_keep_hypot() {
        for (next, limit, inside) in [
            ([1.0, 2.0], 3.0, true),
            ([-3.0, 4.0], 6.0, true),
            ([1.0, -0.0], 2.0, true),
            ([-0.0, 0.0], 0.0, true),
            ([0.0, -0.0], -0.0, true),
            ([3.0, 4.0], 5.0, false),
            ([3.0, 4.0], 5.0f64.next_up(), false),
            ([3.0, 4.0], 5.0f64.next_down(), false),
            ([1.0, 0.0], 1.0f64.next_up(), false),
            ([1.0, 0.0], 0.0, false),
            ([-0.0, 0.0], -1.0, false),
        ] {
            assert_eq!(friction_inside(next, limit), inside);
            assert_friction_projection(next, limit);
        }
    }

    #[test]
    fn friction_extremes_preserve_unconditional_hypot_bits() {
        let nan = f64::from_bits(0x7ff8_0000_0000_0123);
        let values = [
            -f64::INFINITY,
            -f64::MAX,
            -1e160,
            -1.0,
            -f64::MIN_POSITIVE,
            -f64::from_bits(1),
            -0.0,
            0.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE.sqrt().next_down(),
            f64::MIN_POSITIVE.sqrt(),
            f64::MIN_POSITIVE.sqrt().next_up(),
            1e-160,
            1.0,
            1e160,
            f64::MAX.sqrt().next_down(),
            f64::MAX.sqrt(),
            f64::MAX.sqrt().next_up(),
            f64::MAX,
            f64::INFINITY,
            nan,
            -nan,
        ];
        for x in values {
            for y in values {
                for limit in values {
                    assert_friction_projection([x, y], limit);
                }
            }
        }
        for (next, limit) in [
            ([f64::from_bits(1), 0.0], 1.0),
            ([f64::MIN_POSITIVE, f64::MIN_POSITIVE], 1.0),
            ([1e-160, 1e-160], 1.0),
            ([1e160, 1e160], 1e161),
            ([1.0, 1.0], f64::MAX),
            ([1.0, 1.0], f64::INFINITY),
            ([0.0, -0.0], f64::INFINITY),
            ([nan, 1.0], 2.0),
            ([1.0, 1.0], nan),
        ] {
            assert!(!friction_inside(next, limit));
        }
    }

    #[test]
    fn friction_scaled_boundaries_and_interior_match_hypot_exactly() {
        for exponent in [
            -1074, -1073, -1022, -700, -539, -538, -537, -536, -512, -511, -500, -100, 0, 100, 500,
            511, 512, 513, 700, 1023,
        ] {
            let scale = if exponent < -1022 {
                f64::from_bits(1 << (exponent + 1074))
            } else {
                f64::from_bits(((exponent + 1023) as u64) << 52)
            };
            for vector in [
                [0.0, 1.0],
                [3.0, 4.0],
                [-1.0, 1.0],
                [1.0, 1e-15],
                [1e-15, -1.0],
            ] {
                let next = vector.map(|v| v * scale);
                let norm = next[0].hypot(next[1]);
                for limit in [norm.next_down(), norm, norm.next_up()] {
                    assert!(!friction_inside(next, limit), "boundary at {exponent}");
                    assert_friction_projection(next, limit);
                }
                for limit in [norm * 0.5, norm * 1.5] {
                    assert_friction_projection(next, limit);
                }
            }
        }
        // Vary normal mantissas and exponents without a random dependency.
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut interiors = 0;
        for i in 0..4096 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = f64::from_bits((1023u64 << 52) | (state >> 12));
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = f64::from_bits((1023u64 << 52) | (state >> 12));
            let exponent = i % 1001 - 500;
            let scale = f64::from_bits(((exponent + 1023) as u64) << 52);
            let next = [x * scale, -y * scale];
            let norm = next[0].hypot(next[1]);
            for limit in [
                norm * 0.9,
                norm * 1.1,
                norm.next_down(),
                norm,
                norm.next_up(),
            ] {
                interiors += usize::from(friction_inside(next, limit));
                assert_friction_projection(next, limit);
            }
        }
        assert!(interiors >= 4096, "the normal interior guard must run");
    }

    #[test]
    fn motion_revisions_track_actual_changes_in_every_impulse_path() {
        let mut motion = Motion {
            vel: DVec3::ZERO,
            omega: DVec3::ZERO,
            inverse_mass: 1.0,
            inverse_inertia: DMat3::IDENTITY,
            revision: 0,
        };
        motion.push(DVec3::ZERO, DVec3::Y);
        motion.twist(DVec3::ZERO);
        motion.push_axis(DVec3::ZERO, DVec3::X, 0.0);
        motion.push_tangents(DVec3::ZERO, [DVec3::X, DVec3::Z], [0.0; 2]);
        assert_eq!(motion.revision, 0);
        motion.push(DVec3::X, DVec3::Y);
        assert_eq!(motion.revision, 1);
        motion.twist(DVec3::Y);
        assert_eq!(motion.revision, 2);
        motion.push_axis(DVec3::X * 2.0, DVec3::Z * 3.0, 0.2);
        assert_eq!(motion.revision, 3);
        motion.push_tangents(DVec3::Y, [DVec3::X, DVec3::Z], [0.25, -0.4]);
        assert_eq!(motion.revision, 4);

        // A nonzero impulse can round away without changing either input.
        motion.vel = DVec3::splat(1e100);
        motion.omega = DVec3::splat(1e100);
        motion.push(DVec3::ONE, DVec3::Y);
        motion.twist(DVec3::ONE);
        motion.push_axis(DVec3::ONE, DVec3::ONE, 1.0);
        motion.push_tangents(DVec3::ONE, [DVec3::X, DVec3::Z], [1.0; 2]);
        assert_eq!(motion.revision, 4);

        // Signed zero must invalidate an exact input checkpoint.
        motion.vel = DVec3::new(-0.0, 0.0, 0.0);
        motion.omega = DVec3::ZERO;
        motion.push(DVec3::ZERO, DVec3::ZERO);
        assert_eq!(motion.vel.x.to_bits(), 0.0f64.to_bits());
        assert_eq!(motion.revision, 5);
        motion.inverse_mass = 0.0;
        let before = (vector_bits(motion.vel), vector_bits(motion.omega));
        motion.push(DVec3::ONE, DVec3::Y);
        motion.twist(DVec3::ONE);
        motion.push_axis(DVec3::ONE, DVec3::ONE, 1.0);
        motion.push_tangents(DVec3::ONE, [DVec3::X, DVec3::Z], [1.0; 2]);
        assert_eq!((vector_bits(motion.vel), vector_bits(motion.omega)), before);
        assert_eq!(motion.revision, 5);
    }

    #[test]
    fn a_changing_normal_must_run_friction_again_and_other_motion_invalidates_convergence() {
        let mut motions = [
            Motion {
                vel: DVec3::ZERO,
                omega: DVec3::ZERO,
                inverse_mass: 0.0,
                inverse_inertia: DMat3::ZERO,
                revision: 0,
            },
            Motion {
                vel: DVec3::X,
                omega: DVec3::ZERO,
                inverse_mass: 1.0,
                inverse_inertia: DMat3::IDENTITY,
                revision: 0,
            },
        ];
        let normal = DVec3::Y;
        let tangents = basis(normal);
        let axes = [normal, tangents[0], tangents[1]].map(|direction| {
            ContactAxis::new(
                &motions[0],
                &motions[1],
                DVec3::ZERO,
                DVec3::ZERO,
                direction,
            )
        });
        let mut row = Row {
            a: 0,
            b: 1,
            ra: DVec3::ZERO,
            rb: DVec3::ZERO,
            normal,
            tangents,
            axes,
            twist_mass: 1.0,
            target: 1.0,
            friction: 0.5,
            torsional: 0.3,
            normal_impulse: 0.0,
            tangent_impulse: [0.0; 2],
            twist_impulse: 0.0,
            fixed_at: None,
            report: ContactReport {
                a: ColliderId(0),
                b: ColliderId(1),
                body_a: BodyId(0),
                body_b: BodyId(1),
                point: DVec3::ZERO,
                normal,
                separation: -0.05,
                impulse: DVec3::ZERO,
                twist: DVec3::ZERO,
            },
        };
        assert!(!row.solve::<true>(&mut motions));
        assert_eq!(row.normal_impulse, 1.0);
        assert!(row.fixed_at.is_none());
        assert!(!row.solve::<true>(&mut motions));
        assert_eq!(motions[1].vel.x, 0.5);
        assert!(row.fixed_at.is_none());
        assert!(!row.solve::<true>(&mut motions));
        assert!(row.fixed_at.is_some());
        assert!(row.solve::<true>(&mut motions));

        motions[1].twist(DVec3::Y);
        assert!(!row.solve::<true>(&mut motions));
        assert_eq!(row.twist_impulse, -0.3);
        assert!(row.fixed_at.is_none());
        for _ in 0..3 {
            row.solve::<true>(&mut motions);
        }
        assert!(row.fixed_at.is_some());
        motions[1].push_axis(-DVec3::Y * 2.0, DVec3::ZERO, -2.0);
        assert!(!row.solve::<true>(&mut motions));
        assert_eq!(row.normal_impulse, 3.0);
        assert!(row.fixed_at.is_none());
    }

    #[test]
    fn exact_convergence_matches_all_iterations_with_coupled_contacts_and_joints() {
        use crate::body::{Body, BodyKind};
        use crate::collision::{Collider, ContactPoint, Shape};
        use crate::joint::Joint;
        use glam::DQuat;

        let mut reference = World::new(1.0 / 120.0);
        let mut fixed = Body::new(1.0, DVec3::ONE, DVec3::ZERO);
        fixed.kind = BodyKind::Static;
        let ground = reference.add(fixed);
        let floor = reference.add_collider(Collider::new(
            ground,
            Shape::Cuboid {
                half: DVec3::new(100.0, 0.1, 100.0),
            },
        ));
        let mut platform = Body::new(1.0, DVec3::ONE, DVec3::Y);
        platform.kind = BodyKind::Kinematic;
        platform.vel = DVec3::new(0.07, 0.0, -0.03);
        platform.omega = DVec3::new(0.0, 0.2, 0.0);
        let platform = reference.add(platform);
        let mut bodies = Vec::new();
        let mut colliders = Vec::new();
        for i in 0..7 {
            let t = f64::from(i) * 0.3;
            let mut body = Body::new(
                1.0 + t,
                DVec3::new(0.7 + t, 1.1, 0.9),
                DVec3::new(f64::from(i) * 1.5, 0.8, 0.0),
            );
            body.orientation = DQuat::from_rotation_y(t) * DQuat::from_rotation_x(0.2);
            body.vel = DVec3::new(t.sin(), -0.3, t.cos() * 0.2);
            body.omega = DVec3::new(0.1, -0.3 + t, 0.2);
            let id = reference.add(body);
            let mut collider = Collider::new(id, Shape::Sphere { radius: 0.5 });
            collider.material.friction = 0.65;
            collider.material.torsional = 0.25;
            collider.material.restitution = 0.3;
            colliders.push(reference.add_collider(collider));
            bodies.push(id);
        }
        reference.add_joint(
            Joint::new(
                bodies[0],
                DVec3::X * 0.5,
                bodies[1],
                -DVec3::X * 0.5,
                JointKind::Point,
            )
            .soft(7.0, 0.7)
            .limited(30.0, 10.0),
        );
        reference.add_joint(
            Joint::weld_here(&reference, bodies[3], bodies[4], DVec3::new(5.2, 1.0, 0.2))
                .soft(9.0, 0.8)
                .limited(60.0, 0.3),
        );
        reference.add_joint(
            Joint::new(
                platform,
                DVec3::X * 0.2,
                bodies[5],
                DVec3::Y * 0.3,
                JointKind::Tether { length: 2.0 },
            )
            .limited(20.0, 10.0),
        );
        let mut manifolds = Vec::new();
        for (i, (&body, &collider)) in bodies.iter().zip(&colliders).enumerate() {
            manifolds.push(Manifold {
                a: floor,
                b: collider,
                points: vec![
                    ContactPoint {
                        point: reference[body].pos - DVec3::Y * 0.5 + DVec3::X * 0.2,
                        normal: DVec3::Y,
                        separation: if i % 3 == 0 { 0.012 } else { -0.025 },
                    },
                    ContactPoint {
                        point: reference[body].pos - DVec3::Y * 0.5 - DVec3::X * 0.2,
                        normal: DVec3::Y,
                        separation: -0.01,
                    },
                ],
            });
            if i > 0 {
                manifolds.push(Manifold {
                    a: colliders[i - 1],
                    b: collider,
                    points: vec![ContactPoint {
                        point: (reference[bodies[i - 1]].pos + reference[body].pos) * 0.5
                            + DVec3::Y * 0.2,
                        normal: DVec3::X,
                        separation: -0.005,
                    }],
                });
            }
        }
        reference.warm = manifolds
            .iter()
            .flat_map(|m| {
                m.points.iter().map(|c| WarmContact {
                    a: m.a,
                    b: m.b,
                    point: c.point,
                    normal: 0.17,
                    tangent: DVec3::new(0.2, 0.0, -0.1),
                    twist: 0.07,
                })
            })
            .collect();
        // An independent speculative row actually exercises the skip path.
        let resting = reference.add(Body::new(1.0, DVec3::ONE, DVec3::new(60.0, 0.5, 0.0)));
        let resting = reference.add_collider(Collider::new(resting, Shape::Sphere { radius: 0.5 }));
        manifolds.push(Manifold {
            a: floor,
            b: resting,
            points: vec![ContactPoint {
                point: DVec3::X * 60.0,
                normal: DVec3::Y,
                separation: 0.01,
            }],
        });
        let mut optimized = reference.clone();
        let mut skipped = 0;
        for step in 0..24 {
            let dt = reference.dt;
            let (expected, reference_work) =
                reference.solve_with_optimizations::<false>(&manifolds, dt);
            let (actual, work) = optimized.solve_with_optimizations::<true>(&manifolds, dt);
            assert_eq!(reference_work.skipped_rows, 0);
            skipped += work.skipped_rows;
            assert_eq!(
                serde_json::to_vec(&actual).unwrap(),
                serde_json::to_vec(&expected).unwrap(),
                "reports at solve {step}"
            );
            assert_eq!(
                serde_json::to_vec(&optimized).unwrap(),
                serde_json::to_vec(&reference).unwrap(),
                "world at solve {step}"
            );
            for world in [&mut reference, &mut optimized] {
                world[bodies[step % bodies.len()]].vel += DVec3::new(0.02, -0.07, 0.01);
                world[bodies[(step + 2) % bodies.len()]].omega += DVec3::new(0.01, 0.02, -0.03);
            }
        }
        assert!(
            skipped >= 24 * 19,
            "unchanged rows must skip later iterations"
        );
    }

    #[test]
    fn cached_contact_axes_match_vector_impulses_and_keep_kinematic_motion() {
        for i in 0..128 {
            let t = f64::from(i) * 0.13;
            let ra = DVec3::new(t.sin(), (t * 0.7).cos(), 0.3);
            let rb = DVec3::new(-0.2, (t * 1.3).sin(), t.cos());
            let rotation = DMat3::from_quat(glam::DQuat::from_rotation_y(t));
            let a = Motion {
                vel: DVec3::new(1.0, t.cos(), -0.4),
                omega: DVec3::new(t.sin(), -0.7, 0.2),
                inverse_mass: 0.4,
                inverse_inertia: rotation
                    * DMat3::from_diagonal(DVec3::new(0.3, 0.7, 0.9))
                    * rotation.transpose(),
                revision: 0,
            };
            let b = Motion {
                vel: DVec3::new(-0.3, 0.8, t.sin()),
                omega: DVec3::new(0.6, t.cos(), -0.2),
                inverse_mass: if i % 3 == 0 { 0.0 } else { 0.2 },
                inverse_inertia: if i % 3 == 0 {
                    DMat3::ZERO
                } else {
                    DMat3::from_diagonal(DVec3::new(0.8, 0.4, 0.6))
                },
                revision: 0,
            };
            let normal = DVec3::new(0.3, 1.0, t.sin() * 0.2).normalize();
            let directions = [normal, basis(normal)[0], basis(normal)[1]];
            let axes = directions.map(|d| ContactAxis::new(&a, &b, ra, rb, d));
            let relative = b.point_velocity(rb) - a.point_velocity(ra);
            for (axis, direction) in axes.iter().zip(directions) {
                assert!(
                    (axis.velocity(direction, b.vel - a.vel, a.omega, b.omega)
                        - relative.dot(direction))
                    .abs()
                        < 1e-12
                );
                assert_eq!(axis.mass, effective_mass(&a, &b, ra, rb, direction));
            }

            let mut reference = [a, b];
            let mut cached = reference;
            let tangent_delta = [0.27, -0.31];
            let impulse = directions[1] * tangent_delta[0] + directions[2] * tangent_delta[1];
            reference[0].push(-impulse, ra);
            reference[1].push(impulse, rb);
            cached[0].push_tangents(
                -impulse,
                [axes[1].angular[0], axes[2].angular[0]],
                [-tangent_delta[0], -tangent_delta[1]],
            );
            cached[1].push_tangents(
                impulse,
                [axes[1].angular[1], axes[2].angular[1]],
                tangent_delta,
            );
            let normal_delta = 0.63;
            reference[0].push(-normal * normal_delta, ra);
            reference[1].push(normal * normal_delta, rb);
            cached[0].push_axis(-normal * normal_delta, axes[0].angular[0], -normal_delta);
            cached[1].push_axis(normal * normal_delta, axes[0].angular[1], normal_delta);
            for (actual, expected) in cached.iter().zip(reference) {
                assert!((actual.vel - expected.vel).length() < 1e-12);
                assert!((actual.omega - expected.omega).length() < 1e-12);
            }
            if b.inverse_mass == 0.0 {
                assert_eq!(cached[1].vel, b.vel);
                assert_eq!(cached[1].omega, b.omega);
            }
        }
    }

    #[test]
    fn pair_index_preserves_nearest_point_and_ties_without_scanning_other_pairs() {
        let contact = |a, x, normal| WarmContact {
            a: ColliderId(a),
            b: ColliderId(a + 1),
            point: DVec3::X * x,
            normal,
            tangent: DVec3::ZERO,
            twist: 0.0,
        };
        let mut warm: Vec<_> = (10..1010).map(|a| contact(a, 0.0, a as f64)).collect();
        warm.extend([
            contact(1, -0.01, 11.0),
            contact(1, 0.01, 12.0),
            contact(1, 0.05, 13.0),
        ]);
        let index = warm_pairs(&warm);
        let pair = &index[&(ColliderId(1), ColliderId(2))];
        assert_eq!(
            pair.len(),
            3,
            "unrelated contacts never enter the distance search"
        );
        for point in [
            DVec3::ZERO,
            DVec3::X * 0.015,
            DVec3::X * 0.05,
            DVec3::X * 0.09,
        ] {
            let reference = warm
                .iter()
                .filter(|w| w.a == ColliderId(1) && w.b == ColliderId(2))
                .map(|w| (w.point.distance_squared(point), w))
                .filter(|(d, _)| *d <= WARM_RADIUS * WARM_RADIUS)
                .min_by(|x, y| x.0.total_cmp(&y.0))
                .map(|(_, w)| *w);
            assert_eq!(nearest_warm(pair, point), reference);
        }
        assert_eq!(
            nearest_warm(pair, DVec3::ZERO).unwrap().normal,
            11.0,
            "ties retain insertion order"
        );
    }
}
