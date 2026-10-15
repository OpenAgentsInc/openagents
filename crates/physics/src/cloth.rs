//! Deterministic cloth: a grid of particles solved with extended
//! position-based dynamics (XPBD), the solver loop of [`crate::rope`] in two
//! dimensions.
//!
//! The method follows the public literature: compliant constraints with a
//! Lagrange multiplier, `alpha / h^2` per substep (Macklin, Müller, and
//! Chentanez, "XPBD", 2016), many substeps of one iteration each (Macklin et
//! al., "Small Steps in Physics Simulation", 2019), distance constraints
//! along the grid's edges (stretch) and the triangles' diagonals (shear),
//! and isometric bending across every inner edge (Bergou, Wardetzky,
//! Harmon, Zorin, and Grinspun, "A Quadratic Bending Model for Inextensible
//! Surfaces", 2006): the cotangent-weighted curvature vector of each
//! four-particle stencil, linear in the positions, which stays stable
//! where a dihedral-angle constraint flips at a full fold. Genesis's PBD
//! cloth (<https://github.com/Genesis-Embodied-AI/Genesis>, genesis-world
//! `df30b61a`, Apache-2.0, `pbd_solver.py`,
//! `materials/PBD/cloth.py`) has the same shape; it is credited here and
//! none of its code is translated.
//!
//! The cloth lives beside a [`World`] and steps after it, as a rope does:
//!
//! - **Pins** hold particles to world points or to points on bodies. A pin on
//!   a dynamic body is two-way: each substep solves it with the body's
//!   effective inverse mass at the pin, the pin point drifts by the velocity
//!   the cloth's pull gives it, and the summed impulses are applied to the
//!   body after the step, so the cloth's momentum plus the body's is
//!   conserved.
//! - **Collisions** push particles out of the world's colliders (spheres,
//!   capsules, boxes, hulls, and distance grids) to the cloth's thickness
//!   after each substep's velocities are taken, so the push never launches
//!   a particle; its closing speed is stopped with Coulomb friction on its
//!   sliding (the velocity update of Müller et al., "Position Based
//!   Dynamics", 2007). Against a dynamic body the push is shared by inverse
//!   mass and the body takes the opposite impulse.
//! - **Air** slows each particle (`ClothSettings::air`), as Genesis's
//!   cloth does; set it to zero for a closed system.
//! - **Self-collision**, when on, keeps particles that are not grid
//!   neighbours a thickness apart, found through a sorted spatial hash
//!   (particle against particle only, as in Genesis).
//!
//! Every step is a fixed sequence of arithmetic in index order on one thread,
//! and the cloth serializes, so a saved cloth restores and continues bit for
//! bit, and its result never depends on the world's thread count.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::BodyKind;
use crate::collision::Filter;
use crate::ledger::Momentum;
use crate::world::{BodyId, Field, World};

/// Cloth material and solver settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClothSettings {
    /// Mass per unit area, kg/m^2.
    pub density: f64,
    /// Compliance along the grid's edges, m/N. Zero is inextensible.
    pub stretch_compliance: f64,
    /// Compliance along the diagonals, m/N.
    pub shear_compliance: f64,
    /// Compliance of the isometric bending measure across each inner edge
    /// (about the fold angle; see `Bend`), 1/(N m). Larger is softer.
    /// Stiff bending on a fine grid fights contacts within a single
    /// iteration and pumps energy, so cloth defaults soft.
    pub bend_compliance: f64,
    /// Rate at which each edge's stretching speed decays, 1/s.
    pub damping: f64,
    /// Rate at which each particle's speed relative to still air decays,
    /// 1/s (Genesis's cloth has the same air resistance). Zero keeps a
    /// closed system's momentum exact.
    #[serde(default)]
    pub air: f64,
    /// Substeps per step, each one iteration.
    pub substeps: u32,
    /// Distance the cloth keeps from colliders and, with self-collision,
    /// from itself, m.
    pub thickness: f64,
    /// Coulomb coefficient against colliders.
    pub friction: f64,
    pub self_collision: bool,
    /// Which colliders the cloth touches, by their filters.
    pub filter: Filter,
}

impl Default for ClothSettings {
    fn default() -> Self {
        Self {
            density: 0.25,
            stretch_compliance: 0.0,
            shear_compliance: 1e-4,
            bend_compliance: 300.0,
            damping: 10.0,
            air: 1.0,
            substeps: 12,
            thickness: 0.01,
            friction: 0.4,
            self_collision: false,
            filter: Filter::ALL,
        }
    }
}

/// Where a pin holds its particle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    /// A fixed world point, m.
    World(DVec3),
    /// A point in a body's frame, m.
    Body { body: BodyId, local: DVec3 },
}

/// A particle held to an anchor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub particle: u32,
    pub anchor: Anchor,
}

/// A distance constraint between two particles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Edge {
    a: u32,
    b: u32,
    rest: f64,
    /// Along the grid (stretch) rather than a diagonal (shear).
    stretch: bool,
}

/// Isometric bending across the edge `c`–`d` between triangles `a c d` and
/// `b d c`: the length of `v = sum k_i x_i` over `[c, d, a, b]`, the
/// discrete mean-curvature vector of the stencil (zero when flat), scaled
/// by `scale`, held at its rest value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Bend {
    ids: [u32; 4],
    k: [f64; 4],
    scale: f64,
    rest: f64,
}

/// The stencil weights of Bergou et al. ("A Quadratic Bending Model for
/// Inextensible Surfaces", 2006) for the edge `x0`–`x1` with opposite
/// corners `x2` and `x3`, and the scale `sqrt(3 / (A0 + A1))` that makes
/// `scale |sum k_i x_i|` the bending measure; `None` for a degenerate
/// stencil.
fn isometric(x: [DVec3; 4]) -> Option<([f64; 4], f64)> {
    let [x0, x1, x2, x3] = x;
    let cot = |a: DVec3, b: DVec3| {
        let s = a.cross(b).length();
        (s > 1e-18).then(|| a.dot(b) / s)
    };
    let (e0, e1, e2, e3, e4) = (x1 - x0, x2 - x0, x3 - x0, x2 - x1, x3 - x1);
    let (c01, c02) = (cot(e0, e1)?, cot(e0, e2)?);
    let (c03, c04) = (cot(-e0, e3)?, cot(-e0, e4)?);
    let area = 0.5 * (e0.cross(e1).length() + e0.cross(e2).length());
    if area <= 1e-18 {
        return None;
    }
    Some((
        [c03 + c04, c01 + c02, -c01 - c03, -c02 - c04],
        (3.0 / area).sqrt(),
    ))
}

/// A rectangular cloth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cloth {
    pub settings: ClothSettings,
    /// Particles along the first grid direction.
    pub columns: u32,
    /// Particles along the second.
    pub rows: u32,
    /// Rest spacing, m.
    pub spacing: f64,
    /// Particle positions after the last step, row by row, m.
    pub pos: Vec<DVec3>,
    /// Positions before the last step, for render interpolation.
    pub prev: Vec<DVec3>,
    pub vel: Vec<DVec3>,
    pub pins: Vec<Pin>,
    edges: Vec<Edge>,
    bends: Vec<Bend>,
    /// Momentum the colliders gave the cloth in the last step, about the
    /// world origin (body impulses included).
    #[serde(default)]
    pub contact: Momentum,
    /// The impulse and its moment about the world origin that the cloth put
    /// on each body in the last step, by body.
    #[serde(default)]
    pub body_impulses: Vec<(BodyId, Momentum)>,
}

/// The bending measure of a stencil and its gradient for each particle.
fn bending(b: &Bend, x: [DVec3; 4]) -> (f64, [DVec3; 4]) {
    let v = x.iter().zip(&b.k).map(|(p, k)| *p * *k).sum::<DVec3>();
    let length = v.length();
    let dir = if length > 1e-12 {
        v / length
    } else {
        DVec3::ZERO
    };
    (
        b.scale * length,
        [0, 1, 2, 3].map(|i| dir * (b.scale * b.k[i])),
    )
}

impl Cloth {
    /// A flat cloth of `columns` by `rows` particles (at least 2 each)
    /// `spacing` apart, from `origin` along unit directions `u` (columns)
    /// and `v` (rows), at rest and unpinned.
    #[must_use]
    pub fn grid(
        origin: DVec3,
        u: DVec3,
        v: DVec3,
        columns: u32,
        rows: u32,
        spacing: f64,
        settings: ClothSettings,
    ) -> Self {
        let (columns, rows) = (columns.max(2), rows.max(2));
        let u = u.normalize_or(DVec3::X);
        let v = v.normalize_or(DVec3::Z);
        let id = |i: u32, j: u32| j * columns + i;
        let mut pos = Vec::with_capacity((columns * rows) as usize);
        for j in 0..rows {
            for i in 0..columns {
                pos.push(origin + u * (f64::from(i) * spacing) + v * (f64::from(j) * spacing));
            }
        }
        let at = |k: u32| pos[k as usize];
        let mut edges = Vec::new();
        for j in 0..rows {
            for i in 0..columns {
                let mut add = |a: u32, b: u32, stretch: bool| {
                    edges.push(Edge {
                        a,
                        b,
                        rest: at(a).distance(at(b)),
                        stretch,
                    });
                };
                if i + 1 < columns {
                    add(id(i, j), id(i + 1, j), true);
                }
                if j + 1 < rows {
                    add(id(i, j), id(i, j + 1), true);
                }
                // Shear along the triangles' diagonal only: a second,
                // crossing diagonal would also lock the fold across the
                // first, fighting the bending constraint.
                if i + 1 < columns && j + 1 < rows {
                    add(id(i, j), id(i + 1, j + 1), false);
                }
            }
        }
        // Triangles split each quad along (i, j)-(i+1, j+1); each inner
        // edge between two of them is a fold.
        let mut bends = Vec::new();
        let mut fold = |a: u32, b: u32, c: u32, d: u32| {
            let ids = [c, d, a, b];
            if let Some((k, scale)) = isometric(ids.map(at)) {
                let mut bend = Bend {
                    ids,
                    k,
                    scale,
                    rest: 0.0,
                };
                bend.rest = bending(&bend, ids.map(at)).0;
                bends.push(bend);
            }
        };
        for j in 0..rows - 1 {
            for i in 0..columns - 1 {
                let (p00, p10, p11, p01) = (id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1));
                // The quad's own diagonal.
                fold(p10, p01, p00, p11);
                // Its right edge, with the next quad.
                if i + 2 < columns {
                    fold(p00, id(i + 2, j + 1), p10, p11);
                }
                // Its top edge, with the quad above.
                if j + 2 < rows {
                    fold(p00, id(i + 1, j + 2), p01, p11);
                }
            }
        }
        let count = pos.len();
        Self {
            settings,
            columns,
            rows,
            spacing,
            prev: pos.clone(),
            vel: vec![DVec3::ZERO; count],
            pos,
            pins: Vec::new(),
            edges,
            bends,
            contact: Momentum::ZERO,
            body_impulses: Vec::new(),
        }
    }

    /// The particle at column `i`, row `j`.
    #[must_use]
    pub fn index(&self, i: u32, j: u32) -> u32 {
        j * self.columns + i
    }

    /// Hold particle `(i, j)` where it is now.
    pub fn pin_here(&mut self, i: u32, j: u32) {
        let particle = self.index(i, j);
        let anchor = Anchor::World(self.pos[particle as usize]);
        self.pins.push(Pin { particle, anchor });
    }

    /// Hold particle `(i, j)` to the point of `body` under it now.
    pub fn pin_to_body(&mut self, world: &World, body: BodyId, i: u32, j: u32) {
        let particle = self.index(i, j);
        let b = &world[body];
        let local = b.orientation.inverse() * (self.pos[particle as usize] - b.pos);
        self.pins.push(Pin {
            particle,
            anchor: Anchor::Body { body, local },
        });
    }

    /// Mass of one particle, kg.
    #[must_use]
    pub fn particle_mass(&self) -> f64 {
        let area =
            f64::from(self.columns - 1) * f64::from(self.rows - 1) * self.spacing * self.spacing;
        self.settings.density * area / self.pos.len() as f64
    }

    /// Triangles of the grid, counterclockwise about `u` × `v`.
    #[must_use]
    pub fn triangles(&self) -> Vec<[u32; 3]> {
        let mut out = Vec::new();
        for j in 0..self.rows - 1 {
            for i in 0..self.columns - 1 {
                let (a, b, c, d) = (
                    self.index(i, j),
                    self.index(i + 1, j),
                    self.index(i + 1, j + 1),
                    self.index(i, j + 1),
                );
                out.push([a, b, c]);
                out.push([a, c, d]);
            }
        }
        out
    }

    /// Positions between the last two steps; `alpha` in [0, 1].
    pub fn interpolated(&self, alpha: f64) -> impl Iterator<Item = DVec3> + '_ {
        let alpha = alpha.clamp(0.0, 1.0);
        self.prev
            .iter()
            .zip(&self.pos)
            .map(move |(p, q)| p.lerp(*q, alpha))
    }

    fn world_pinned(&self) -> Vec<bool> {
        let mut pinned = vec![false; self.pos.len()];
        for pin in &self.pins {
            if matches!(pin.anchor, Anchor::World(_)) {
                pinned[pin.particle as usize] = true;
            }
        }
        pinned
    }

    /// Linear and angular momentum (about `origin`) of the particles that
    /// move.
    #[must_use]
    pub fn momentum(&self, origin: DVec3) -> Momentum {
        let m = self.particle_mass();
        let pinned = self.world_pinned();
        self.pos
            .iter()
            .zip(&self.vel)
            .zip(&pinned)
            .filter(|(_, p)| !**p)
            .fold(Momentum::ZERO, |sum, ((x, v), _)| {
                sum + Momentum {
                    linear: *v * m,
                    angular: (*x - origin).cross(*v * m),
                }
            })
    }

    /// Kinetic energy of the particles that move, J.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        let pinned = self.world_pinned();
        0.5 * self.particle_mass()
            * self
                .vel
                .iter()
                .zip(&pinned)
                .filter(|(_, p)| !**p)
                .map(|(v, _)| v.length_squared())
                .sum::<f64>()
    }

    /// Advance by the world's step, after the world has stepped: the field
    /// accelerates the particles, pins hold them (two-way on dynamic
    /// bodies), and they collide with the world's colliders. The impulses
    /// on bodies are applied to them; the momentum they received, about the
    /// world origin, is returned.
    #[allow(clippy::too_many_lines)]
    pub fn step(&mut self, world: &mut World, field: &impl Field) -> Momentum {
        let n = self.pos.len();
        let dt = world.dt;
        let substeps = self.settings.substeps.max(1);
        let h = dt / f64::from(substeps);
        let mass = self.particle_mass();
        let w = 1.0 / mass;
        let s = self.settings;
        let pinned = self.world_pinned();
        let inv = |k: usize| if pinned[k] { 0.0 } else { w };
        self.prev.clone_from(&self.pos);
        self.contact = Momentum::ZERO;
        // Bodies the cloth touches or hangs from: (body, effective inverse
        // mass function inputs), and the impulse each received.
        let mut bodies: Vec<(BodyId, Momentum)> = Vec::new();
        let give = |bodies: &mut Vec<(BodyId, Momentum)>, id: BodyId, impulse: DVec3, at: DVec3| {
            let m = Momentum {
                linear: impulse,
                angular: at.cross(impulse),
            };
            match bodies.iter_mut().find(|(b, _)| *b == id) {
                Some((_, sum)) => *sum += m,
                None => bodies.push((id, m)),
            }
        };
        let responds = |world: &World, id: BodyId| {
            let b = &world[id];
            b.kind == BodyKind::Dynamic && !b.removed && b.mass > 0.0
        };
        // Effective inverse mass of body `id` at `point` along `dir`.
        let effective = |world: &World, id: BodyId, point: DVec3, dir: DVec3| {
            let b = &world[id];
            let turn = (point - b.pos).cross(dir);
            1.0 / b.mass + turn.dot(b.inverse_inertia_world(turn))
        };
        // Colliders near the cloth this step, with their bodies.
        let (mut lo, mut hi) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
        let mut fastest: f64 = 0.0;
        for (x, v) in self.pos.iter().zip(&self.vel) {
            lo = lo.min(*x);
            hi = hi.max(*x);
            fastest = fastest.max(v.length());
        }
        let reach = s.thickness + fastest * dt * 2.0 + 0.05;
        let (lo, hi) = (lo - DVec3::splat(reach), hi + DVec3::splat(reach));
        // (surface, body, friction, bounding sphere center, its radius
        // grown by the thickness)
        let near: Vec<(crate::collision::Surface, BodyId, f64, DVec3, f64)> = world
            .colliders()
            .iter()
            .filter(|c| s.filter.allows(c.filter) && c.filter != Filter::NONE)
            .filter_map(|c| {
                let (center, _) = c.pose(world);
                let r = c.shape.bound();
                let inside = (center - center.clamp(lo, hi)).length() <= r;
                inside.then(|| {
                    (
                        c.surface(world),
                        c.body,
                        c.material.friction,
                        center,
                        r + s.thickness,
                    )
                })
            })
            .collect();
        // Body pins: where each holds, its drift and kick (the extra
        // displacement and velocity the cloth's pull gave the point).
        let mut drift = vec![DVec3::ZERO; self.pins.len()];
        let mut kick = vec![DVec3::ZERO; self.pins.len()];
        let mut before = vec![DVec3::ZERO; n];
        // Each particle's contact this substep: the collider, its normal,
        // and the total push out.
        let mut touched: Vec<Option<(usize, DVec3, f64, DVec3)>> = vec![None; n];
        let friction = s.friction;
        for _ in 0..substeps {
            for (d, k) in drift.iter_mut().zip(&kick) {
                *d += *k * h;
            }
            before.copy_from_slice(&self.pos);
            for k in 0..n {
                if pinned[k] {
                    continue;
                }
                let accel = field.accel(self.pos[k], self.vel[k]);
                self.vel[k] += accel * h;
                self.vel[k] *= 1.0 - (s.air * h).min(1.0);
                self.pos[k] += self.vel[k] * h;
            }
            for pin in &self.pins {
                if let Anchor::World(at) = pin.anchor {
                    self.pos[pin.particle as usize] = at;
                }
            }
            // Stretch and shear.
            let (stretch, shear) = (s.stretch_compliance / (h * h), s.shear_compliance / (h * h));
            for e in &self.edges {
                let (a, b) = (e.a as usize, e.b as usize);
                let (wa, wb) = (inv(a), inv(b));
                let d = self.pos[b] - self.pos[a];
                let len = d.length();
                if len < 1e-12 || wa + wb == 0.0 {
                    continue;
                }
                let alpha = if e.stretch { stretch } else { shear };
                let lambda = -(len - e.rest) / (wa + wb + alpha);
                let normal = d / len;
                self.pos[a] -= normal * (wa * lambda);
                self.pos[b] += normal * (wb * lambda);
            }
            // Bending.
            let bend = s.bend_compliance / (h * h);
            for f in &self.bends {
                let ids = f.ids.map(|k| k as usize);
                let (measure, grads) = bending(f, ids.map(|k| self.pos[k]));
                let c = measure - f.rest;
                let denom: f64 = ids
                    .iter()
                    .zip(&grads)
                    .map(|(&k, g)| inv(k) * g.length_squared())
                    .sum::<f64>()
                    + bend;
                if denom <= 0.0 {
                    continue;
                }
                let lambda = -c / denom;
                for (&k, g) in ids.iter().zip(&grads) {
                    self.pos[k] += *g * (inv(k) * lambda);
                }
            }
            // Body pins, two-way on dynamic bodies.
            for (p, pin) in self.pins.iter().enumerate() {
                let Anchor::Body { body, local } = pin.anchor else {
                    continue;
                };
                let k = pin.particle as usize;
                let anchor = world[body].to_world(local);
                let target = anchor + drift[p];
                let d = self.pos[k] - target;
                let len = d.length();
                if len < 1e-12 {
                    continue;
                }
                let dir = d / len;
                let wb = if responds(world, body) {
                    effective(world, body, anchor, dir)
                } else {
                    0.0
                };
                if w + wb == 0.0 {
                    continue;
                }
                let lambda = -len / (w + wb);
                self.pos[k] += dir * (w * lambda);
                if wb > 0.0 {
                    // The body's point moves toward the particle; its kick
                    // is the velocity of that impulse.
                    let impulse = dir * (-lambda / h);
                    kick[p] += impulse * wb;
                    give(&mut bodies, body, impulse, target);
                }
            }
            if s.self_collision {
                self.collide_self(&inv);
            }
            // Colliders, as constraints in the same pass: a particle inside
            // one is pushed out to the thickness, the push shared with a
            // dynamic body by inverse mass. Two passes settle a particle
            // caught between two colliders.
            for (k, hit) in touched.iter_mut().enumerate() {
                *hit = None;
                if pinned[k] {
                    continue;
                }
                let start = self.pos[k];
                for _ in 0..2 {
                    for (c, (surface, body, _, center, bound)) in near.iter().enumerate() {
                        if self.pos[k].distance_squared(*center) > bound * bound {
                            continue;
                        }
                        let (dist, normal) = surface.distance(self.pos[k]);
                        let depth = s.thickness - dist;
                        if depth <= 0.0 {
                            continue;
                        }
                        let wb = if responds(world, *body) {
                            effective(world, *body, self.pos[k], normal)
                        } else {
                            0.0
                        };
                        let push = depth * w / (w + wb);
                        self.pos[k] += normal * push;
                        let load = hit.map_or(0.0, |(_, _, l, _)| l) + push;
                        *hit = Some((c, normal, load, DVec3::ZERO));
                    }
                }
                // Friction at the position level (Macklin et al., "Unified
                // Particle Physics", 2014): the substep's sliding is undone
                // while it is under the coefficient times the push (static),
                // and cut by that much otherwise (kinetic).
                if let Some((c, normal, load, moved)) = hit.as_mut() {
                    let mu = (friction * near[*c].2).sqrt();
                    let travel = self.pos[k] - before[k];
                    let slide = travel - *normal * travel.dot(*normal);
                    let length = slide.length();
                    if length > 0.0 {
                        self.pos[k] -= slide * (mu * *load / length).min(1.0);
                    }
                    *moved = self.pos[k] - start;
                }
            }
            for k in 0..n {
                self.vel[k] = (self.pos[k] - before[k]) / h;
            }
            // The velocity update of Müller et al. (2007): a particle in
            // contact keeps no speed away from the surface, so the push out
            // never launches it. What a particle's contact did to it this
            // substep, the push included, a dynamic body takes the opposite
            // of.
            for k in 0..n {
                let Some((c, normal, _, moved)) = touched[k] else {
                    continue;
                };
                let body = &near[c].1;
                let away = self.vel[k].dot(normal).max(0.0);
                let change = -normal * away;
                self.vel[k] += change;
                let impulse = (moved / h + change) * mass;
                self.contact += Momentum {
                    linear: impulse,
                    angular: self.pos[k].cross(impulse),
                };
                if responds(world, *body) {
                    give(&mut bodies, *body, -impulse, self.pos[k]);
                }
            }
            // Dashpots along the grid's edges: equal and opposite.
            let damp = (s.damping * h).min(1.0);
            for e in self.edges.iter().filter(|e| e.stretch) {
                let (a, b) = (e.a as usize, e.b as usize);
                let (wa, wb) = (inv(a), inv(b));
                let d = self.pos[b] - self.pos[a];
                let len = d.length();
                if len < 1e-12 || wa + wb == 0.0 {
                    continue;
                }
                let normal = d / len;
                let closing = (self.vel[b] - self.vel[a]).dot(normal);
                let impulse = -closing * damp / (wa + wb);
                self.vel[a] -= normal * (impulse * wa);
                self.vel[b] += normal * (impulse * wb);
            }
        }
        for pin in &self.pins {
            if let Anchor::World(_) = pin.anchor {
                self.vel[pin.particle as usize] = DVec3::ZERO;
            }
        }
        // Apply what the cloth did to the bodies.
        let mut total = Momentum::ZERO;
        bodies.sort_by_key(|(b, _)| *b);
        for (id, m) in &bodies {
            let body = &mut world[*id];
            body.wake();
            body.vel += m.linear / body.mass;
            body.apply_angular_impulse(m.angular - body.pos.cross(m.linear));
            total += *m;
        }
        self.body_impulses = bodies;
        total
    }

    /// Particles that are not grid neighbours, closer than the thickness
    /// twice over, pushed apart equally along the line between them.
    fn collide_self(&mut self, inv: &dyn Fn(usize) -> f64) {
        let gap = 2.0 * self.settings.thickness;
        if gap <= 0.0 {
            return;
        }
        let cell = |x: DVec3| {
            let c = (x / gap).floor();
            (c.x as i64, c.y as i64, c.z as i64)
        };
        let mut keys: Vec<((i64, i64, i64), u32)> = self
            .pos
            .iter()
            .enumerate()
            .map(|(k, x)| (cell(*x), k as u32))
            .collect();
        keys.sort_unstable();
        let columns = self.columns as i64;
        for k in 0..self.pos.len() {
            let c = cell(self.pos[k]);
            let (ik, jk) = (k as i64 % columns, k as i64 / columns);
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let key = (c.0 + dx, c.1 + dy, c.2 + dz);
                        let start = keys.partition_point(|(q, _)| *q < key);
                        for &(q, other) in &keys[start..] {
                            if q != key {
                                break;
                            }
                            let o = other as usize;
                            if o <= k {
                                continue;
                            }
                            let (io, jo) = (o as i64 % columns, o as i64 / columns);
                            if (io - ik).abs() <= 1 && (jo - jk).abs() <= 1 {
                                continue;
                            }
                            let (wa, wb) = (inv(k), inv(o));
                            let d = self.pos[o] - self.pos[k];
                            let len = d.length();
                            if len >= gap || len < 1e-12 || wa + wb == 0.0 {
                                continue;
                            }
                            let normal = d / len;
                            let lambda = (gap - len) / (wa + wb);
                            self.pos[k] -= normal * (wa * lambda);
                            self.pos[o] += normal * (wb * lambda);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
