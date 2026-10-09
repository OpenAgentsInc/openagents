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
}

impl Motion {
    fn point_velocity(&self, r: DVec3) -> DVec3 {
        self.vel + self.omega.cross(r)
    }

    fn push(&mut self, impulse: DVec3, r: DVec3) {
        self.vel += impulse * self.inverse_mass;
        self.omega += self.inverse_inertia * r.cross(impulse);
    }

    fn twist(&mut self, angular: DVec3) {
        self.omega += self.inverse_inertia * angular;
    }
}

#[derive(Clone, Copy)]
struct Row {
    a: usize,
    b: usize,
    ra: DVec3,
    rb: DVec3,
    normal: DVec3,
    tangents: [DVec3; 2],
    normal_mass: f64,
    tangent_mass: [f64; 2],
    twist_mass: f64,
    target: f64,
    friction: f64,
    torsional: f64,
    normal_impulse: f64,
    tangent_impulse: [f64; 2],
    twist_impulse: f64,
    report: ContactReport,
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

/// Contact row `row`'s Gauss-Seidel update: friction bounded by the
/// current normal impulse, the twist, then the normal impulse.
fn solve_row(row: &mut Row, motions: &mut [Motion]) {
    let (a, b) = (row.a, row.b);
    // Friction first, bounded by the current normal impulse.
    let relative = motions[b].point_velocity(row.rb) - motions[a].point_velocity(row.ra);
    let limit = row.friction * row.normal_impulse;
    let old = row.tangent_impulse;
    let mut next = [
        old[0] - relative.dot(row.tangents[0]) * row.tangent_mass[0],
        old[1] - relative.dot(row.tangents[1]) * row.tangent_mass[1],
    ];
    let size = next[0].hypot(next[1]);
    if size > limit {
        let scale = if size > 0.0 { limit / size } else { 0.0 };
        next = [next[0] * scale, next[1] * scale];
    }
    row.tangent_impulse = next;
    let delta = row.tangents[0] * (next[0] - old[0]) + row.tangents[1] * (next[1] - old[1]);
    motions[a].push(-delta, row.ra);
    motions[b].push(delta, row.rb);
    if row.torsional > 0.0 {
        let spin = (motions[b].omega - motions[a].omega).dot(row.normal);
        let limit = row.torsional * row.normal_impulse;
        let old = row.twist_impulse;
        let next = (old - spin * row.twist_mass).clamp(-limit, limit);
        row.twist_impulse = next;
        let delta = row.normal * (next - old);
        motions[a].twist(-delta);
        motions[b].twist(delta);
    }
    let relative = motions[b].point_velocity(row.rb) - motions[a].point_velocity(row.ra);
    let old = row.normal_impulse;
    let next = (old + (row.target - relative.dot(row.normal)) * row.normal_mass).max(0.0);
    row.normal_impulse = next;
    let delta = row.normal * (next - old);
    motions[a].push(-delta, row.ra);
    motions[b].push(delta, row.rb);
}

/// Every constraint once, `iterations` times: the joint blocks, the
/// tethers, then the contact rows, each in order.
fn sweep(
    iterations: u32,
    motions: &mut [Motion],
    rows: &mut [Row],
    tethers: &mut [(usize, JointRow)],
    blocks: &mut [BlockJoint],
) {
    for _ in 0..iterations {
        for block in blocks.iter_mut() {
            block.solve(motions);
        }
        for (_, row) in tethers.iter_mut() {
            row.solve(motions);
        }
        for row in rows.iter_mut() {
            solve_row(row, motions);
        }
    }
}

mod regions;

/// Contact rows that make a solve worth splitting over threads.
const SPLIT_ROWS: usize = 256;

/// A union-find root.
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// One thread's share of a split solve: constraints with their places.
struct Part {
    rows: Vec<(usize, Row)>,
    tethers: Vec<(usize, (usize, JointRow))>,
    blocks: Vec<(usize, BlockJoint)>,
}

/// The solve's passes over every constraint. An island too large to share
/// out ([`regions`]) solves by regions; the rest by islands
/// ([`by_islands`]). The islands share no body that moves, so solving
/// them apart changes nothing.
fn iterate(
    iterations: u32,
    motions: &mut [Motion],
    rows: &mut Vec<Row>,
    tethers: &mut Vec<(usize, JointRow)>,
    blocks: &mut Vec<BlockJoint>,
    positions: &[DVec3],
) {
    if rows.len() < regions::REGION_ROWS {
        by_islands(iterations, motions, rows, tethers, blocks);
        return;
    }
    let moves: Vec<bool> = motions.iter().map(|m| m.inverse_mass > 0.0).collect();
    let mut parent: Vec<usize> = (0..motions.len()).collect();
    let pairs = rows
        .iter()
        .map(|r| (r.a, r.b))
        .chain(tethers.iter().map(|(_, r)| (r.a, r.b)))
        .chain(blocks.iter().map(|b| (b.a, b.b)));
    for (a, b) in pairs {
        if moves[a] && moves[b] {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let mut island = |a: usize, b: usize| root(&mut parent, if moves[a] { a } else { b });
    let row_islands: Vec<usize> = rows.iter().map(|r| island(r.a, r.b)).collect();
    // Islands with joints solve whole.
    let ends: Vec<(usize, usize)> = tethers
        .iter()
        .map(|(_, r)| (r.a, r.b))
        .chain(blocks.iter().map(|b| (b.a, b.b)))
        .collect();
    let jointed: std::collections::BTreeSet<usize> =
        ends.into_iter().map(|(a, b)| island(a, b)).collect();
    let mut counts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for &i in &row_islands {
        *counts.entry(i).or_default() += 1;
    }
    let large: std::collections::BTreeSet<usize> = counts
        .into_iter()
        .filter(|&(i, n)| n >= regions::REGION_ROWS && !jointed.contains(&i))
        .map(|(i, _)| i)
        .collect();
    if large.is_empty() {
        by_islands(iterations, motions, rows, tethers, blocks);
        return;
    }
    let total = rows.len();
    let mut rest: Vec<(usize, Row)> = Vec::new();
    let mut apart: std::collections::BTreeMap<usize, Vec<(usize, Row)>> =
        std::collections::BTreeMap::new();
    for (k, row) in std::mem::take(rows).into_iter().enumerate() {
        if large.contains(&row_islands[k]) {
            apart.entry(row_islands[k]).or_default().push((k, row));
        } else {
            rest.push((k, row));
        }
    }
    let (rest_places, mut rest_rows): (Vec<usize>, Vec<Row>) = rest.into_iter().unzip();
    by_islands(iterations, motions, &mut rest_rows, tethers, blocks);
    let mut all: Vec<Option<Row>> = vec![None; total];
    for (k, row) in rest_places.into_iter().zip(rest_rows) {
        all[k] = Some(row);
    }
    for (_, mut island_rows) in apart {
        regions::solve(iterations, motions, &mut island_rows, positions);
        for (k, row) in island_rows {
            all[k] = Some(row);
        }
    }
    *rows = all.into_iter().flatten().collect();
}

/// [`sweep`], with the constraints split over threads by island when there
/// are enough of them (issue #10937). An island is the bodies that move and
/// the constraints between them, joined through bodies that move; a fixed,
/// kinematic, or sleeping body's motion never changes (its inverse mass and
/// inertia are zero, so every impulse adds zero), so it joins nothing.
/// Each thread sweeps whole islands, each island's constraints in their
/// order, so every body sees the same updates in the same order as one
/// sweep of everything: the result is the same bit for bit.
fn by_islands(
    iterations: u32,
    motions: &mut [Motion],
    rows: &mut Vec<Row>,
    tethers: &mut Vec<(usize, JointRow)>,
    blocks: &mut Vec<BlockJoint>,
) {
    let threads = crate::parallel::threads(rows.len(), SPLIT_ROWS);
    if threads <= 1 {
        sweep(iterations, motions, rows, tethers, blocks);
        return;
    }
    let moves: Vec<bool> = motions.iter().map(|m| m.inverse_mass > 0.0).collect();
    let mut parent: Vec<usize> = (0..motions.len()).collect();
    let pairs = rows
        .iter()
        .map(|r| (r.a, r.b))
        .chain(tethers.iter().map(|(_, r)| (r.a, r.b)))
        .chain(blocks.iter().map(|b| (b.a, b.b)));
    for (a, b) in pairs {
        if moves[a] && moves[b] {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    // A constraint's island: that of a body of it that moves.
    let mut island = |a: usize, b: usize| root(&mut parent, if moves[a] { a } else { b });
    let row_islands: Vec<usize> = rows.iter().map(|r| island(r.a, r.b)).collect();
    let tether_islands: Vec<usize> = tethers.iter().map(|(_, r)| island(r.a, r.b)).collect();
    let block_islands: Vec<usize> = blocks.iter().map(|b| island(b.a, b.b)).collect();
    // Islands to threads, largest first, each to the least loaded.
    let mut work: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for &i in row_islands.iter().chain(&tether_islands) {
        *work.entry(i).or_default() += 1;
    }
    for &i in &block_islands {
        *work.entry(i).or_default() += 6;
    }
    if work.len() < 2 {
        sweep(iterations, motions, rows, tethers, blocks);
        return;
    }
    let mut sizes: Vec<(usize, usize)> = work.into_iter().map(|(i, w)| (w, i)).collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let mut load = vec![0usize; threads];
    let mut thread_of: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::with_capacity(sizes.len());
    for (w, i) in sizes {
        let t = (0..threads).min_by_key(|&t| (load[t], t)).unwrap_or(0);
        load[t] += w;
        thread_of.insert(i, t);
    }
    let mut parts: Vec<Part> = (0..threads)
        .map(|_| Part {
            rows: Vec::new(),
            tethers: Vec::new(),
            blocks: Vec::new(),
        })
        .collect();
    for (k, row) in std::mem::take(rows).into_iter().enumerate() {
        parts[thread_of[&row_islands[k]]].rows.push((k, row));
    }
    for (k, tether) in std::mem::take(tethers).into_iter().enumerate() {
        parts[thread_of[&tether_islands[k]]]
            .tethers
            .push((k, tether));
    }
    for (k, block) in std::mem::take(blocks).into_iter().enumerate() {
        parts[thread_of[&block_islands[k]]].blocks.push((k, block));
    }
    let shared: &[Motion] = motions;
    let done = crate::parallel::each(parts, |part: Part| {
        let mut local = shared.to_vec();
        let (places, mut r): (Vec<usize>, Vec<Row>) = part.rows.into_iter().unzip();
        let (tether_places, mut t): (Vec<usize>, Vec<(usize, JointRow)>) =
            part.tethers.into_iter().unzip();
        let (block_places, mut b): (Vec<usize>, Vec<BlockJoint>) = part.blocks.into_iter().unzip();
        sweep(iterations, &mut local, &mut r, &mut t, &mut b);
        let part = Part {
            rows: places.into_iter().zip(r).collect(),
            tethers: tether_places.into_iter().zip(t).collect(),
            blocks: block_places.into_iter().zip(b).collect(),
        };
        (part, local)
    });
    let mut all_rows: Vec<Option<Row>> = Vec::new();
    all_rows.resize_with(row_islands.len(), || None);
    let mut all_tethers: Vec<Option<(usize, JointRow)>> = Vec::new();
    all_tethers.resize_with(tether_islands.len(), || None);
    let mut all_blocks: Vec<Option<BlockJoint>> = Vec::new();
    all_blocks.resize_with(block_islands.len(), || None);
    for (t, (part, local)) in done.into_iter().enumerate() {
        // The bodies that move in this thread's islands take its motion.
        for (i, m) in local.into_iter().enumerate() {
            if moves[i] && thread_of.get(&root(&mut parent, i)) == Some(&t) {
                motions[i] = m;
            }
        }
        for (k, row) in part.rows {
            all_rows[k] = Some(row);
        }
        for (k, row) in part.tethers {
            all_tethers[k] = Some(row);
        }
        for (k, block) in part.blocks {
            all_blocks[k] = Some(block);
        }
    }
    *rows = all_rows.into_iter().flatten().collect();
    *tethers = all_tethers.into_iter().flatten().collect();
    *blocks = all_blocks.into_iter().flatten().collect();
}

impl World {
    /// Resolve joints and `manifolds` by changing body velocities for a step
    /// of `dt`. Records each joint's impulses and returns what each contact
    /// point did.
    pub(crate) fn solve(&mut self, manifolds: &[Manifold], dt: f64) -> Vec<ContactReport> {
        let settings = self.solver;
        let mut motions: Vec<Motion> = self
            .bodies()
            .iter()
            .map(|b| {
                let r = DMat3::from_quat(b.orientation);
                let inverse = if b.inverse_mass() > 0.0 {
                    r * DMat3::from_diagonal(b.inertia.recip()) * r.transpose()
                } else {
                    DMat3::ZERO
                };
                Motion {
                    vel: b.vel,
                    omega: b.omega_world(),
                    inverse_mass: b.inverse_mass(),
                    inverse_inertia: inverse,
                }
            })
            .collect();
        // Last step's contacts by collider pair, in their order, so a
        // contact finds its warm start among its own pair's rather than
        // scanning every contact (quadratic with thousands of contacts, as
        // in a meteor swarm's rubble).
        let mut warm_of: std::collections::HashMap<(u32, u32), Vec<usize>> =
            std::collections::HashMap::with_capacity(self.warm.len());
        for (k, w) in self.warm.iter().enumerate() {
            warm_of.entry((w.a.0, w.b.0)).or_default().push(k);
        }
        let mut rows = Vec::new();
        for manifold in manifolds {
            let warm_pair = warm_of
                .get(&(manifold.a.0, manifold.b.0))
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
                let warm = warm_pair
                    .iter()
                    .map(|&k| &self.warm[k])
                    .map(|w| (w.point.distance_squared(c.point), w))
                    .filter(|(d, _)| *d <= WARM_RADIUS * WARM_RADIUS)
                    .min_by(|x, y| x.0.total_cmp(&y.0))
                    .map(|(_, w)| *w);
                rows.push(Row {
                    a,
                    b,
                    ra,
                    rb,
                    normal: c.normal,
                    tangents,
                    normal_mass: effective_mass(ma, mb, ra, rb, c.normal),
                    tangent_mass: [
                        effective_mass(ma, mb, ra, rb, tangents[0]),
                        effective_mass(ma, mb, ra, rb, tangents[1]),
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
        let positions: Vec<DVec3> = self.bodies().iter().map(|b| b.pos).collect();
        iterate(
            settings.iterations,
            &mut motions,
            &mut rows,
            &mut tethers,
            &mut blocks,
            &positions,
        );
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
        rows.into_iter()
            .map(|row| ContactReport {
                impulse: row.normal * row.normal_impulse
                    + row.tangents[0] * row.tangent_impulse[0]
                    + row.tangents[1] * row.tangent_impulse[1],
                twist: row.normal * row.twist_impulse,
                ..row.report
            })
            .collect()
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
