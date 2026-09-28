//! A deterministic rope: a chain of particles solved with extended
//! position-based dynamics (XPBD).
//!
//! The method follows the public literature: Verlet-style particles with
//! position projection (Jakobsen, "Advanced Character Physics", 2001;
//! Müller et al., "Position Based Dynamics", 2007), compliant constraints
//! with a Lagrange multiplier (Macklin, Müller, and Chentanez, "XPBD", 2016),
//! many substeps of one iteration each (Macklin et al., "Small Steps in
//! Physics Simulation", 2019), and one-sided long-range attachments that keep
//! every particle within its rest path length of each pinned end (Kim,
//! Chentanez, and Müller, "Long Range Attachments", 2012).
//!
//! Both ends are pinned to anchor points the caller supplies each step, and
//! the rope interpolates them across its substeps. Each particle may be at
//! most its rest path length from each end, so when the anchors are a full
//! rope length apart the chain lies straight along the line between them;
//! when they are closer, the rope curves and carries transverse waves.
//!
//! By default the rope is one-way: it follows its anchors and never pushes
//! them. With [`Rope::coupled`], [`Rope::step_between`] solves each end with
//! its body's effective inverse mass and applies the rope's pull to the end
//! bodies as equal and opposite impulses, so the rope's linear momentum plus
//! the bodies' is conserved.
//!
//! A rope may also collide with solids ([`Solid`](crate::solid::Solid),
//! passed with their bounds as [`Bounded`]): after the constraints
//! in each substep, every segment that comes within the rope's radius of a
//! solid is pushed back out along the solid's normal, the push shared
//! between the segment's two particles by where it touched (position-based
//! collision, Müller et al., 2007), with Coulomb friction on the
//! particles' tangential motion (Macklin et al., "Unified Particle
//! Physics", 2014). A rope pulled around a solid therefore wraps it rather
//! than passing through, and [`Rope::touch`] reports which solid each
//! particle rests on, so an owner can find where the rope bends.
//!
//! The far end may also be loose ([`Rope::loosen`]): it is then an ordinary
//! particle, and only the first end is pinned.
//!
//! Every step is a fixed sequence of arithmetic, and the rope serializes, so
//! a saved rope restores and continues bit for bit.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::body::BodyKind;
use crate::ledger::Momentum;
use crate::solid::Bounded;
use crate::world::{BodyId, Field, World};

/// Rope material and solver settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RopeSettings {
    /// Mass per unit rest length, kg/m.
    pub linear_density: f64,
    /// Compliance of each segment against stretching, m/N. Zero is
    /// inextensible.
    pub stretch_compliance: f64,
    /// Compliance of the bending constraint between every other particle,
    /// m/N. Larger is floppier.
    pub bend_compliance: f64,
    /// Rate at which a segment's stretching speed decays, 1/s. Only the
    /// relative speed along each segment is damped, so waves across the
    /// rope ring on and momentum is untouched.
    pub damping: f64,
    /// Substeps per step, each one iteration.
    pub substeps: u32,
    /// Radius the rope keeps from solids, m.
    #[serde(default)]
    pub radius: f64,
    /// Coulomb coefficient between the rope and solids.
    #[serde(default)]
    pub friction: f64,
}

impl Default for RopeSettings {
    fn default() -> Self {
        Self {
            linear_density: 0.05,
            stretch_compliance: 0.0,
            bend_compliance: 1e-2,
            damping: 2.0,
            substeps: 6,
            radius: 0.0,
            friction: 0.0,
        }
    }
}

/// A rope pinned at both ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rope {
    pub settings: RopeSettings,
    /// Rest length, m. The owner may change it between steps (a reel);
    /// the next step moves the rest length to it linearly across its
    /// substeps, so paying out never compresses the rope.
    pub length: f64,
    /// Rest length at the end of the last step, m.
    #[serde(default)]
    pub stepped_length: f64,
    /// Particle positions after the last step, m. The first and last are
    /// the anchors.
    pub pos: Vec<DVec3>,
    /// Particle positions before the last step, for render interpolation.
    pub prev: Vec<DVec3>,
    /// Particle velocities, m/s.
    pub vel: Vec<DVec3>,
    /// Anchor positions at the end of the last step.
    pub anchors: [DVec3; 2],
    /// Whether [`Rope::step_between`] couples the rope to its end bodies.
    #[serde(default)]
    pub coupled: bool,
    /// Impulse the rope exerted on each anchor in the last step, N s.
    #[serde(default)]
    pub end_impulse: [DVec3; 2],
    /// Moment of those impulses about the world origin, N m s: the sum of
    /// `point × impulse` over the substeps.
    #[serde(default)]
    pub end_moment: [DVec3; 2],
    /// The far end is a free particle rather than pinned.
    #[serde(default)]
    pub loose: bool,
    /// For each particle, one more than the index of the solid it touched
    /// in the last step, or 0.
    #[serde(default)]
    pub touch: Vec<u32>,
    /// Momentum the solids gave the rope's particles in the last step,
    /// about the world origin.
    #[serde(default)]
    pub contact: Momentum,
}

impl Rope {
    /// A rope of `count` particles (at least 3) between `a` and `b`, at
    /// rest, with rest length `length`. It starts on a sine bow as long as
    /// the rope, with particles at equal arc length, so no constraint is
    /// violated; a rope no longer than the gap keeps a trace of the bow so
    /// it can still buckle.
    #[must_use]
    pub fn new(a: DVec3, b: DVec3, length: f64, count: usize, settings: RopeSettings) -> Self {
        let count = count.max(3);
        let last = (count - 1) as f64;
        let span = b - a;
        let side = span.normalize_or(DVec3::X).any_orthonormal_vector();
        // A fine polyline of a sine bow, and its length.
        let fine = (count * 64) as u32;
        let curve = |bow: f64| -> Vec<DVec3> {
            (0..=fine)
                .map(|k| {
                    let t = f64::from(k) / f64::from(fine);
                    a + span * t + side * (bow * (std::f64::consts::PI * t).sin())
                })
                .collect()
        };
        let arc = |points: &[DVec3]| points.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
        // A bow as long as the rope, found by bisection, or a trace of one
        // on a taut rope so it can still buckle.
        let mut bow = length * 1e-4;
        if length > span.length() + bow {
            let (mut lo, mut hi) = (0.0, length);
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                if arc(&curve(mid)) < length {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            bow = lo;
        }
        // Particles at equal arc length along it.
        let points = curve(bow);
        let total = arc(&points);
        let mut pos = Vec::with_capacity(count);
        let (mut walked, mut k) = (0.0, 0);
        for i in 0..count {
            let target = total * i as f64 / last;
            while k + 1 < points.len() - 1 && walked + points[k].distance(points[k + 1]) < target {
                walked += points[k].distance(points[k + 1]);
                k += 1;
            }
            let piece = points[k].distance(points[k + 1]);
            let t = if piece > 0.0 {
                ((target - walked) / piece).clamp(0.0, 1.0)
            } else {
                0.0
            };
            pos.push(points[k].lerp(points[k + 1], t));
        }
        pos[0] = a;
        pos[count - 1] = b;
        Self {
            settings,
            length,
            stepped_length: length,
            prev: pos.clone(),
            vel: vec![DVec3::ZERO; count],
            pos,
            anchors: [a, b],
            coupled: false,
            end_impulse: [DVec3::ZERO; 2],
            end_moment: [DVec3::ZERO; 2],
            loose: false,
            touch: vec![0; count],
            contact: Momentum::ZERO,
        }
    }

    /// Let the far end go: from the next step it moves as a free particle
    /// with the velocity it has.
    pub fn loosen(&mut self) {
        self.loose = true;
    }

    /// Pin the far end again. The next step moves it from where it lies to
    /// the anchor it is given.
    pub fn tie(&mut self) {
        self.loose = false;
        let n = self.pos.len();
        self.anchors[1] = self.pos[n - 1];
    }

    /// The last particle that moves with the rope, exclusive: the far end
    /// counts when it is loose.
    fn movable_end(&self) -> usize {
        let n = self.pos.len();
        if self.loose { n } else { n - 1 }
    }

    /// This rope with two-way coupling on.
    #[must_use]
    pub fn coupled(mut self) -> Self {
        self.coupled = true;
        self
    }

    /// Mass of one interior particle, kg. The end particles belong to the
    /// anchors; a loose far end has the same mass as the others.
    #[must_use]
    pub fn particle_mass(&self) -> f64 {
        self.settings.linear_density * self.length / (self.movable_end() - 1) as f64
    }

    /// Rest length of one segment, m, stretched to the anchor gap when the
    /// anchors are farther apart than the rest length.
    #[must_use]
    pub fn segment(&self, a: DVec3, b: DVec3) -> f64 {
        self.length.max(a.distance(b)) / (self.pos.len() - 1) as f64
    }

    /// Whether the anchors are a full rope length apart, within `tolerance`
    /// of the rest length, so the rope lies straight.
    #[must_use]
    pub fn taut(&self, tolerance: f64) -> bool {
        self.anchors[0].distance(self.anchors[1]) >= self.length - tolerance
    }

    /// Tension at the ends in the last step, N: the mean pull on the two
    /// anchors over `dt`.
    #[must_use]
    pub fn tension(&self, dt: f64) -> f64 {
        (self.end_impulse[0].length() + self.end_impulse[1].length()) * 0.5 / dt
    }

    /// Positions between the last two steps; `alpha` in [0, 1].
    pub fn interpolated(&self, alpha: f64) -> impl Iterator<Item = DVec3> + '_ {
        let alpha = alpha.clamp(0.0, 1.0);
        self.prev
            .iter()
            .zip(&self.pos)
            .map(move |(p, q)| p.lerp(*q, alpha))
    }

    /// Linear momentum of the interior particles (and a loose end), and
    /// their angular momentum about `origin`.
    #[must_use]
    pub fn momentum(&self, origin: DVec3) -> Momentum {
        let m = self.particle_mass();
        let n = self.movable_end();
        self.pos[1..n]
            .iter()
            .zip(&self.vel[1..n])
            .fold(Momentum::ZERO, |sum, (x, v)| {
                sum + Momentum {
                    linear: *v * m,
                    angular: (*x - origin).cross(*v * m),
                }
            })
    }

    /// Kinetic energy of the interior particles (and a loose end), J.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        let n = self.movable_end();
        0.5 * self.particle_mass()
            * self.vel[1..n]
                .iter()
                .map(|v| v.length_squared())
                .sum::<f64>()
    }

    /// Where the rope would bend if pulled taut around `solids` without
    /// changing which side of each it passes: the particles of the shortest
    /// path from the first end to the last that follows the rope and whose
    /// chords clear every solid ("string pulling"). Walking from the first
    /// end, each chord runs to the farthest particle it can see; the
    /// particle where the view is blocked is a bend. Returns the bends in
    /// order, and the path's length from the first end to the last bend.
    #[must_use]
    pub fn bends(&self, solids: &[Bounded]) -> (Vec<usize>, f64) {
        let n = self.pos.len();
        let mut bends = Vec::new();
        let mut length = 0.0;
        if solids.is_empty() {
            return (bends, length);
        }
        let clear = |p: DVec3, q: DVec3| {
            let (lo, hi) = (p.min(q), p.max(q));
            solids.iter().all(|body| {
                !body.meets(lo, hi) || {
                    let t = body.solid.nearest_on_segment(p, q);
                    body.solid.distance(p.lerp(q, t)).0 > 0.0
                }
            })
        };
        let mut from = 0;
        while from < n - 1 {
            let mut to = from + 1;
            while to < n - 1 && clear(self.pos[from], self.pos[to + 1]) {
                to += 1;
            }
            if to == n - 1 {
                break;
            }
            length += self.pos[from].distance(self.pos[to]);
            bends.push(to);
            from = to;
        }
        (bends, length)
    }

    /// Advance by `dt` with the ends pinned, moving linearly from the last
    /// anchors to `a` and `b`. The field accelerates the interior particles.
    /// The rope never pushes its anchors; [`Rope::end_impulse`] reports the
    /// pull it would exert.
    pub fn step(&mut self, dt: f64, a: DVec3, b: DVec3, field: &impl Field) {
        self.advance(dt, [a, b], [0.0; 2], field, &[]);
    }

    /// [`Rope::step`] among `solids`, which the rope collides with. A loose
    /// far end ignores `b`.
    pub fn step_among(
        &mut self,
        dt: f64,
        a: DVec3,
        b: DVec3,
        field: &impl Field,
        solids: &[Bounded],
    ) {
        self.advance(dt, [a, b], [0.0; 2], field, solids);
    }

    /// Advance by `dt` tied to anchors fixed on two bodies (body-frame
    /// points `anchor_a` on `a` and `anchor_b` on `b`), after the world has
    /// stepped. When [`Rope::coupled`] is set, each end takes part in the
    /// solve with its body's effective inverse mass along the rope, and the
    /// impulses the rope exerted on the ends are then applied to the
    /// bodies; the momentum they received, about the world origin, is
    /// returned. Otherwise the ends are pinned and nothing is applied. The
    /// rope collides with `solids`.
    pub fn step_between(
        &mut self,
        world: &mut World,
        (a, anchor_a): (BodyId, DVec3),
        (b, anchor_b): (BodyId, DVec3),
        field: &impl Field,
        solids: &[Bounded],
    ) -> Momentum {
        let ends = [world[a].to_world(anchor_a), world[b].to_world(anchor_b)];
        let n = self.pos.len();
        let mut inverse = [0.0; 2];
        if self.coupled {
            for (end, (id, neighbor)) in [(a, self.pos[1]), (b, self.pos[n - 2])]
                .into_iter()
                .enumerate()
            {
                let body = &world[id];
                if body.kind != BodyKind::Dynamic || body.mass <= 0.0 {
                    continue;
                }
                let arm = ends[end] - body.pos;
                let along = (neighbor - ends[end]).normalize_or_zero();
                let turn = arm.cross(along);
                let local = body.orientation.inverse() * turn;
                inverse[end] = 1.0 / body.mass + (local * local / body.inertia).element_sum();
            }
        }
        self.advance(world.dt, ends, inverse, field, solids);
        if !self.coupled {
            return Momentum::ZERO;
        }
        let mut total = Momentum::ZERO;
        for (end, id) in [(0, a), (1, b)] {
            if inverse[end] == 0.0 {
                continue;
            }
            let (impulse, moment) = (self.end_impulse[end], self.end_moment[end]);
            let body = &mut world[id];
            body.wake();
            body.vel += impulse / body.mass;
            let spin = moment - body.pos.cross(impulse);
            body.apply_angular_impulse(spin);
            total += Momentum {
                linear: impulse,
                angular: moment,
            };
        }
        total
    }

    /// The solver. `inverse` is each end's inverse mass, zero for a pin.
    /// An end with mass drifts from its anchor path by the velocity its
    /// impulses would give it, so the rope and the end exchange momentum
    /// within the step; the owner applies the summed impulse afterward. A
    /// loose far end is an ordinary particle and ignores `b`.
    #[allow(clippy::too_many_lines)]
    fn advance(
        &mut self,
        dt: f64,
        [a, b]: [DVec3; 2],
        inverse: [f64; 2],
        field: &impl Field,
        solids: &[Bounded],
    ) {
        let n = self.pos.len();
        let loose = self.loose;
        // Particles the rope moves: the interior, and a loose far end.
        let movable = self.movable_end();
        let substeps = self.settings.substeps.max(1);
        let h = dt / f64::from(substeps);
        let mass = self.particle_mass();
        let w = 1.0 / mass;
        let inverse = if loose { [inverse[0], w] } else { inverse };
        let stretch = self.settings.stretch_compliance / (h * h);
        let bend = self.settings.bend_compliance / (h * h);
        let damp = (self.settings.damping * h).min(1.0);
        let [a0, b0] = self.anchors;
        let b = if loose { self.pos[n - 1] } else { b };
        let (l0, l1) = (
            if self.stepped_length > 0.0 {
                self.stepped_length
            } else {
                self.length
            },
            self.length,
        );
        let path_vel = [(a - a0) / dt, (b - b0) / dt];
        self.prev.clone_from(&self.pos);
        self.end_impulse = [DVec3::ZERO; 2];
        self.end_moment = [DVec3::ZERO; 2];
        self.contact = Momentum::ZERO;
        self.touch.clear();
        self.touch.resize(n, 0);
        let (near, margin) = self.near(solids, a, b, dt);
        // Segment and solid pairs that may touch within this step: particles
        // move less than the margin in a step, so each substep tests only
        // these.
        let mut pairs = Vec::new();
        if !near.is_empty() {
            for i in 0..n - 1 {
                let (p, q) = (self.pos[i], self.pos[i + 1]);
                let (lo, hi) = (p.min(q) - margin, p.max(q) + margin);
                for (k, body) in near.iter().enumerate() {
                    if body.bounds.meets(lo, hi) {
                        pairs.push((i, k));
                    }
                }
            }
        }
        let mut before = vec![DVec3::ZERO; n];
        // Extra velocity and displacement each end has taken from the rope.
        let mut kick = [DVec3::ZERO; 2];
        let mut drift = [DVec3::ZERO; 2];
        let inv = |i: usize| {
            if i == 0 {
                inverse[0]
            } else if i == n - 1 {
                inverse[1]
            } else {
                w
            }
        };
        let end_of = |i: usize| {
            if i == 0 {
                Some(0)
            } else if i == n - 1 && !loose {
                Some(1)
            } else {
                None
            }
        };
        let pinned = inverse == [0.0; 2];
        for s in 1..=substeps {
            let t = f64::from(s) / f64::from(substeps);
            for end in 0..2 {
                drift[end] += kick[end] * h;
            }
            let path = [a0.lerp(a, t), b0.lerp(b, t)];
            before.copy_from_slice(&self.pos);
            for i in 1..movable {
                let accel = field.accel(self.pos[i], self.vel[i]);
                self.vel[i] += accel * h;
                self.pos[i] += self.vel[i] * h;
            }
            self.pos[0] = path[0] + drift[0];
            if !loose {
                self.pos[n - 1] = path[1] + drift[1];
            }
            let (pa, pb) = (self.pos[0], self.pos[n - 1]);
            let rest = l0 + (l1 - l0) * t;
            let seg = if loose {
                rest / (n - 1) as f64
            } else {
                rest.max(pa.distance(pb)) / (n - 1) as f64
            };
            // Impulse times h the rope put on each end this substep.
            let mut pull = [DVec3::ZERO; 2];
            // Stretch, then bending, each a distance constraint solved once.
            for (gap, rest, compliance) in [(1, seg, stretch), (2, seg * 2.0, bend)] {
                for i in 0..n - gap {
                    let j = i + gap;
                    let (wi, wj) = (inv(i), inv(j));
                    let d = self.pos[j] - self.pos[i];
                    let len = d.length();
                    if len < 1e-12 || wi + wj == 0.0 {
                        continue;
                    }
                    let normal = d / len;
                    let lambda = -(len - rest) / (wi + wj + compliance);
                    // Impulse times h on j is lambda along the normal, and
                    // on i the opposite.
                    self.pos[i] -= normal * (wi * lambda);
                    self.pos[j] += normal * (wj * lambda);
                    if let Some(end) = end_of(i) {
                        pull[end] -= normal * lambda;
                    }
                    if let Some(end) = end_of(j) {
                        pull[end] += normal * lambda;
                    }
                }
            }
            // Long-range attachments: never farther than the rest path
            // length from either pinned end. At or past full length, pinned
            // ends leave one feasible point on the line between them, so
            // project there directly, unless a solid is near enough that the
            // line might pass through it.
            let straight =
                pinned && !loose && near.is_empty() && rest <= pa.distance(pb) * (1.0 + 1e-12);
            for i in 1..movable {
                let t = i as f64 / (n - 1) as f64;
                if straight {
                    let dx = pa.lerp(pb, t) - self.pos[i];
                    self.pos[i] += dx;
                    pull[0] -= dx * (mass * (1.0 - t));
                    pull[1] -= dx * (mass * t);
                    continue;
                }
                let ends: &[(usize, usize, f64)] = if loose {
                    &[(0, 0, seg * i as f64)]
                } else {
                    &[(0, 0, seg * i as f64), (1, n - 1, seg * (n - 1 - i) as f64)]
                };
                for &(end, index, limit) in ends {
                    let d = self.pos[i] - self.pos[index];
                    let len = d.length();
                    if len > limit && len > 0.0 {
                        let normal = d / len;
                        let lambda = (len - limit) / (w + inverse[end]);
                        self.pos[i] -= normal * (w * lambda);
                        self.pos[index] += normal * (inverse[end] * lambda);
                        pull[end] += normal * lambda;
                    }
                }
            }
            // Pushing one segment out of a corner can press its neighbor in;
            // a second pass in the last substep settles the pair.
            if !pairs.is_empty() {
                self.collide(&near, &pairs, &before, mass, h);
                if s == substeps {
                    self.collide(&near, &pairs, &before, mass, h);
                }
            }
            for ((v, x), x0) in self.vel[1..movable]
                .iter_mut()
                .zip(&self.pos[1..movable])
                .zip(&before[1..movable])
            {
                *v = (*x - *x0) / h;
            }
            // The ends' velocities: their path plus what the rope gave them.
            for end in 0..2 {
                kick[end] += pull[end] * inverse[end] / h;
            }
            self.vel[0] = path_vel[0] + kick[0];
            if !loose {
                self.vel[n - 1] = path_vel[1] + kick[1];
            }
            // Dashpots along each segment: equal and opposite, central.
            for i in 0..n - 1 {
                let j = i + 1;
                let (wi, wj) = (inv(i), inv(j));
                let d = self.pos[j] - self.pos[i];
                let len = d.length();
                if len < 1e-12 || wi + wj == 0.0 {
                    continue;
                }
                let normal = d / len;
                let closing = (self.vel[j] - self.vel[i]).dot(normal);
                // Impulse on j along the normal that removes `damp` of it.
                let impulse = -closing * damp / (wi + wj);
                self.vel[i] -= normal * (impulse * wi);
                self.vel[j] += normal * (impulse * wj);
                if let Some(end) = end_of(i) {
                    pull[end] -= normal * (impulse * h);
                    kick[end] -= normal * (impulse * wi);
                }
                if let Some(end) = end_of(j) {
                    pull[end] += normal * (impulse * h);
                    kick[end] += normal * (impulse * wj);
                }
            }
            for (end, point) in [(0, pa), (1, pb)] {
                // What the rope did to its end is what the anchor feels.
                let impulse = pull[end] / h;
                self.end_impulse[end] += impulse;
                self.end_moment[end] += point.cross(impulse);
            }
        }
        self.pos[0] = a;
        self.vel[0] = path_vel[0];
        if loose {
            self.anchors = [a, self.pos[n - 1]];
        } else {
            self.pos[n - 1] = b;
            self.vel[n - 1] = path_vel[1];
            self.anchors = [a, b];
        }
        self.stepped_length = self.length;
    }

    /// The solids that may touch the rope this step: those whose bounds
    /// meet the rope's, grown by its radius and by how far its anchors move.
    fn near(&self, solids: &[Bounded], a: DVec3, b: DVec3, dt: f64) -> (Vec<Near>, f64) {
        if solids.is_empty() {
            return (Vec::new(), 0.0);
        }
        let (mut lo, mut hi) = (a.min(b), a.max(b));
        for p in &self.pos {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        // Particles move at most a few meters per second; a step's worth of
        // that plus the anchors' travel is ample.
        let speed = self
            .vel
            .iter()
            .map(|v| v.length_squared())
            .fold(0.0, f64::max)
            .sqrt();
        let margin = self.settings.radius
            + (a - self.anchors[0])
                .length()
                .max((b - self.anchors[1]).length())
            + speed * dt
            + 0.05;
        let (lo, hi) = (lo - margin, hi + margin);
        let near = solids
            .iter()
            .enumerate()
            .filter(|(_, body)| body.meets(lo, hi))
            .map(|(index, body)| Near {
                index: u32::try_from(index).unwrap_or(u32::MAX - 1),
                bounds: *body,
            })
            .collect();
        (near, margin)
    }

    /// Push every segment that is within the rope's radius of a solid back
    /// out along the solid's normal, share the push between its particles by
    /// where it touched, and apply Coulomb friction to their motion since
    /// `before`, the start of the substep. Pinned and coupled ends follow
    /// their anchors, so only the rope's own particles move.
    fn collide(
        &mut self,
        near: &[Near],
        pairs: &[(usize, usize)],
        before: &[DVec3],
        mass: f64,
        h: f64,
    ) {
        let radius = self.settings.radius;
        let friction = self.settings.friction;
        let movable = self.movable_end();
        let inv = |i: usize| {
            if i == 0 || i >= movable {
                0.0
            } else {
                1.0 / mass
            }
        };
        for &(i, k) in pairs {
            let j = i + 1;
            let (wi, wj) = (inv(i), inv(j));
            let body = &near[k];
            let (p, q) = (self.pos[i], self.pos[j]);
            let (lo, hi) = (p.min(q) - radius, p.max(q) + radius);
            if wi + wj == 0.0 || !body.bounds.meets(lo, hi) {
                continue;
            }
            let solid = body.bounds.solid;
            let t = solid.nearest_on_segment(p, q);
            let (distance, normal) = solid.distance(p.lerp(q, t));
            let depth = radius - distance;
            if depth <= 0.0 {
                continue;
            }
            // Move the touching point out by `depth`: a particle moves by
            // its share of the touch and its inverse mass.
            let (si, sj) = (1.0 - t, t);
            let scale = depth / (wi * si * si + wj * sj * sj);
            for (k, share, wk) in [(i, si, wi), (j, sj, wj)] {
                if wk == 0.0 || share == 0.0 {
                    continue;
                }
                let push = normal * (wk * share * scale);
                let mut moved = push;
                // Friction resists sliding along the surface, up to the
                // coefficient times the push.
                let travel = self.pos[k] + push - before[k];
                let slide = travel - normal * travel.dot(normal);
                let length = slide.length();
                if length > 0.0 && friction > 0.0 {
                    let limit = friction * push.length();
                    moved -= slide * (limit / length).min(1.0);
                    // Near an edge the tangent plane is not the surface:
                    // keep the held particle outside.
                    let (clearance, out) = solid.distance(self.pos[k] + moved);
                    if clearance < radius {
                        moved += out * (radius - clearance);
                    }
                }
                self.pos[k] += moved;
                // The substep's velocity is (x - x0) / h, so moving x by d
                // adds m d / h of momentum and x0 × m d / h of angular
                // momentum.
                let momentum = moved * (mass / h);
                self.contact += Momentum {
                    linear: momentum,
                    angular: before[k].cross(momentum),
                };
                self.touch[k] = body.index + 1;
            }
        }
    }
}

/// A solid near the rope this step, with its index and bounds.
struct Near {
    index: u32,
    bounds: Bounded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{Body, BodyKind};
    use crate::solid::Solid;
    use crate::world::{NoField, Uniform};

    fn lateral(rope: &Rope) -> f64 {
        let (a, b) = (rope.anchors[0], rope.anchors[1]);
        let axis = (b - a).normalize();
        rope.pos
            .iter()
            .map(|p| {
                let d = *p - a;
                (d - axis * d.dot(axis)).length()
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn a_taut_rope_lies_straight_along_its_anchors() {
        let (a, b) = (DVec3::ZERO, DVec3::new(30.0, 4.0, -2.0));
        let length = a.distance(b);
        let mut rope = Rope::new(a, b, length * 1.2, 64, RopeSettings::default());
        // Pull the far anchor out to the full length and hold it there.
        let far = a + (b - a).normalize() * length * 1.2;
        for k in 1..=120 {
            let t = f64::from(k) / 120.0;
            rope.step(1.0 / 120.0, a, b.lerp(far, t), &NoField);
        }
        for _ in 0..120 {
            rope.step(1.0 / 120.0, a, far, &NoField);
        }
        assert!(rope.taut(1e-9));
        assert!(lateral(&rope) < 1e-6 * length, "{}", lateral(&rope));
        // Stretched past the rest length (a joint's stretch), still straight.
        let beyond = far + (far - a).normalize() * 0.1;
        rope.step(1.0 / 120.0, a, beyond, &NoField);
        assert!(lateral(&rope) < 1e-6 * length, "{}", lateral(&rope));
        for p in &rope.pos {
            assert!(p.distance(a) <= a.distance(beyond) + 1e-9);
        }
    }

    #[test]
    fn a_slack_rope_curves_and_carries_a_whip_wave() {
        let (a, b) = (DVec3::ZERO, DVec3::new(20.0, 0.0, 0.0));
        let mut rope = Rope::new(a, b, 20.0, 64, RopeSettings::default());
        // Bring the far end in quickly to half the length: slack.
        for k in 1..=60 {
            let t = f64::from(k) / 60.0;
            rope.step(
                1.0 / 120.0,
                a,
                b.lerp(DVec3::new(10.0, 0.0, 0.0), t),
                &NoField,
            );
        }
        for _ in 0..240 {
            rope.step(1.0 / 120.0, a, DVec3::new(10.0, 0.0, 0.0), &NoField);
        }
        assert!(!rope.taut(0.01));
        let curve = lateral(&rope);
        assert!(curve > 1.0, "a slack rope bows: {curve} m");
        // Every particle stays within its path length of each end.
        let seg = rope.segment(rope.anchors[0], rope.anchors[1]);
        let n = rope.pos.len();
        for (i, p) in rope.pos.iter().enumerate() {
            assert!(p.distance(rope.anchors[0]) <= seg * i as f64 + 1e-9);
            assert!(p.distance(rope.anchors[1]) <= seg * (n - 1 - i) as f64 + 1e-9);
        }
        // Flick the near end sideways: a transverse wave travels down the
        // rope, reaching the middle later than the end.
        let start: Vec<DVec3> = rope.pos.clone();
        let moved = |rope: &Rope, i: usize| rope.pos[i].distance(start[i]);
        let flick = DVec3::new(0.0, 0.0, 0.5);
        for k in 0..12 {
            let up = if k < 6 { flick } else { DVec3::ZERO };
            rope.step(1.0 / 120.0, a + up, DVec3::new(10.0, 0.0, 0.0), &NoField);
        }
        let (near, middle) = (moved(&rope, 3), moved(&rope, n / 2));
        assert!(
            near > middle,
            "the whip starts at the end: {near} vs {middle}"
        );
        let mut reached: f64 = 0.0;
        for _ in 0..240 {
            rope.step(1.0 / 120.0, a, DVec3::new(10.0, 0.0, 0.0), &NoField);
            reached = reached.max(moved(&rope, n / 2));
        }
        assert!(reached > 0.05, "the wave reaches the middle: {reached} m");
    }

    #[test]
    fn a_hanging_rope_sags_without_gaining_energy() {
        let (a, b) = (DVec3::ZERO, DVec3::new(8.0, 0.0, 0.0));
        let mut rope = Rope::new(a, b, 10.0, 40, RopeSettings::default());
        let g = 9.81;
        let field = Uniform(DVec3::new(0.0, -g, 0.0));
        let n = rope.pos.len();
        let energy = |rope: &Rope| {
            rope.kinetic_energy()
                + rope.pos[1..n - 1].iter().map(|p| p.y).sum::<f64>() * rope.particle_mass() * g
        };
        let start = energy(&rope);
        let (mut deepest, mut held) = (0.0_f64, DVec3::ZERO);
        let steps = 120 * 20;
        for _ in 0..steps {
            rope.step(1.0 / 120.0, a, b, &field);
            deepest = deepest.min(rope.pos.iter().map(|p| p.y).fold(0.0, f64::min));
            held += rope.end_impulse[0] + rope.end_impulse[1];
            assert!(
                energy(&rope) <= start + 1e-9,
                "energy grew: {} vs {start}",
                energy(&rope)
            );
        }
        // A 10 m chain between points 8 m apart hangs 2.6 m deep as a
        // catenary; it can never hang deeper than the 3 m V.
        assert!(deepest < -2.4 && deepest > -3.0, "{deepest}");
        // On average the anchors carry the rope's weight, 0.5 kg times g.
        let weight = rope.settings.linear_density * rope.length * g;
        let mean = -held.y / (f64::from(steps) / 120.0);
        assert!((mean - weight).abs() < 0.05 * weight, "{mean} vs {weight}");
    }

    #[test]
    fn a_restored_rope_continues_bit_for_bit() {
        let (a, b) = (DVec3::ZERO, DVec3::new(12.0, 3.0, 0.0));
        let mut rope = Rope::new(a, b, 15.0, 48, RopeSettings::default());
        let wiggle = |k: u32| DVec3::new(0.0, (f64::from(k) * 0.05).sin(), 0.0);
        for k in 0..100 {
            rope.step(1.0 / 120.0, a + wiggle(k), b, &NoField);
        }
        let json = serde_json::to_string(&rope).unwrap();
        let mut restored: Rope = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rope);
        for k in 100..300 {
            rope.step(1.0 / 120.0, a + wiggle(k), b, &NoField);
            restored.step(1.0 / 120.0, a + wiggle(k), b, &NoField);
        }
        assert_eq!(restored, rope);
    }

    /// Two free bodies tied by a coupled rope. Nothing external acts, so
    /// the bodies' momentum plus the rope's stays where it started.
    #[test]
    fn a_coupled_rope_conserves_momentum_with_its_bodies() {
        let mut world = World::new(1.0 / 120.0);
        let mut left = Body::new(100.0, DVec3::splat(10.0), DVec3::ZERO);
        left.vel = DVec3::new(-1.5, 0.2, 0.0);
        let mut right = Body::new(60.0, DVec3::splat(6.0), DVec3::new(9.0, 0.0, 0.0));
        right.vel = DVec3::new(1.0, -0.3, 0.4);
        right.omega = DVec3::new(0.0, 0.3, 0.0);
        let (l, r) = (world.add(left), world.add(right));
        world.add(Body::new(1.0, DVec3::ONE, DVec3::splat(50.0)).with_kind(BodyKind::Static));
        let offset = DVec3::new(0.5, 0.0, 0.0);
        let anchors = |world: &World| (world[l].to_world(offset), world[r].to_world(-offset));
        let (a, b) = anchors(&world);
        let settings = RopeSettings {
            linear_density: 0.5,
            ..RopeSettings::default()
        };
        let mut rope = Rope::new(a, b, 10.0, 32, settings).coupled();
        let total =
            |world: &World, rope: &Rope| world.momentum(DVec3::ZERO) + rope.momentum(DVec3::ZERO);
        let start = total(&world, &rope);
        let mut worst: [f64; 2] = [0.0; 2];
        let mut pulled = 0.0_f64;
        for _ in 0..(120 * 10) {
            world.step(&NoField);
            rope.step_between(&mut world, (l, offset), (r, -offset), &NoField, &[]);
            pulled = pulled.max(rope.tension(world.dt));
            let now = total(&world, &rope);
            let scale = start.linear.length().max(1.0);
            worst[0] = worst[0].max((now.linear - start.linear).length() / scale);
            worst[1] = worst[1]
                .max((now.angular - start.angular).length() / start.angular.length().max(1.0));
        }
        assert!(pulled > 10.0, "the rope went taut and pulled: {pulled} N");
        // The bodies were flying apart; the rope turned them around.
        assert!((world[r].pos - world[l].pos).length() < 12.0);
        assert!(worst[0] < 1e-12, "linear {worst:?}");
        // Position projection moves each particle along the line to its
        // anchor, so angular momentum holds to the solver's accuracy.
        assert!(worst[1] < 1e-2, "angular {worst:?}");
    }

    #[test]
    fn a_one_way_rope_leaves_its_bodies_alone() {
        let mut world = World::new(1.0 / 120.0);
        let mut body = Body::new(10.0, DVec3::ONE, DVec3::new(5.0, 0.0, 0.0));
        body.vel = DVec3::X;
        let id = world.add(body);
        let anchor = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        let mut rope = Rope::new(DVec3::ZERO, world[id].pos, 5.0, 16, RopeSettings::default());
        for _ in 0..240 {
            world.step(&NoField);
            let pulled = rope.step_between(
                &mut world,
                (anchor, DVec3::ZERO),
                (id, DVec3::ZERO),
                &NoField,
                &[],
            );
            assert_eq!(pulled, Momentum::ZERO);
        }
        assert_eq!(world[id].vel, DVec3::X);
        assert!(rope.tension(world.dt) > 0.0, "it still reports its pull");
    }

    fn wrapping() -> RopeSettings {
        RopeSettings {
            radius: 0.02,
            friction: 0.3,
            ..RopeSettings::default()
        }
    }

    /// Largest depth of any segment inside a solid, m.
    fn deepest(rope: &Rope, solid: &Solid) -> f64 {
        rope.pos
            .windows(2)
            .map(|w| {
                let t = solid.nearest_on_segment(w[0], w[1]);
                -solid.distance(w[0].lerp(w[1], t)).0
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// An end swung around a post drags the rope against it: the rope bends
    /// around the post instead of cutting through, and reports the touch.
    #[test]
    fn a_rope_swung_around_a_post_wraps_it() {
        let post = Solid::Cylinder {
            a: DVec3::new(0.0, 0.0, -3.0),
            b: DVec3::new(0.0, 0.0, 3.0),
            radius: 0.5,
        };
        let anchor = DVec3::new(-4.0, 0.0, 0.0);
        let swing = |angle: f64| DVec3::new(-4.9 * angle.cos(), 3.0 * angle.sin(), 0.0);
        let mut rope = Rope::new(anchor, swing(0.0), 9.0, 64, wrapping());
        // Carry the end in an arc over the post to its far side, where the
        // straight line back to the anchor runs through the post.
        let mut end = swing(0.0);
        for k in 0..=480 {
            end = swing(std::f64::consts::PI * f64::from(k) / 480.0);
            rope.step_among(1.0 / 120.0, anchor, end, &NoField, &[post.into()]);
            assert!(deepest(&rope, &post) < 1e-3, "{}", deepest(&rope, &post));
        }
        for _ in 0..120 {
            rope.step_among(1.0 / 120.0, anchor, end, &NoField, &[post.into()]);
        }
        // The rope's surface rests on the post: its center line stays a
        // radius out.
        assert!(deepest(&rope, &post) < -0.019, "{}", deepest(&rope, &post));
        // The straight line from anchor to end cuts the post; the rope goes
        // over it and rests on it.
        let t = post.nearest_on_segment(anchor, end);
        assert!(post.distance(anchor.lerp(end, t)).0 < 0.0);
        assert!(rope.touch.contains(&1));
        assert!(rope.contact.linear.is_finite());
    }

    #[test]
    fn a_loose_end_drifts_and_the_reel_takes_it_in() {
        let (a, b) = (DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0));
        let mut rope = Rope::new(a, b, 10.5, 32, wrapping());
        // Flick the far end sideways, then let it go.
        rope.step(1.0 / 120.0, a, b + DVec3::Y * 0.02, &NoField);
        rope.loosen();
        let n = rope.pos.len();
        let free = rope.pos[n - 1];
        rope.step(1.0 / 120.0, a, DVec3::splat(100.0), &NoField);
        // The anchor passed for a loose end is ignored; it keeps its motion.
        assert!(rope.pos[n - 1].distance(free) < 0.1);
        assert!(rope.pos[n - 1].y > free.y);
        // A block beside the rope's path: the swinging rope may not cross it.
        let block = Solid::aabb(DVec3::new(4.0, 0.3, -0.5), DVec3::new(5.0, 1.3, 0.5));
        for p in &mut rope.vel[n / 2..] {
            p.y += 1.0;
        }
        while rope.length > 0.5 {
            rope.length = (rope.length - 1.0 / 120.0).max(0.5);
            rope.step_among(1.0 / 120.0, a, DVec3::ZERO, &NoField, &[block.into()]);
            assert!(rope.pos.iter().all(|p| p.is_finite()));
            assert!(deepest(&rope, &block) < 1e-3, "{}", deepest(&rope, &block));
        }
        assert!(rope.touch.len() == n);
        // Reeled in, the end is within the line's length of the anchor.
        assert!(
            rope.pos[n - 1].length() <= 0.5 + 1e-6,
            "{}",
            rope.pos[n - 1]
        );
        // Tied again, the end follows its anchor.
        rope.tie();
        rope.step(1.0 / 120.0, a, DVec3::new(0.0, 0.0, 0.4), &NoField);
        assert_eq!(rope.pos[n - 1], DVec3::new(0.0, 0.0, 0.4));
    }

    #[test]
    fn a_wrapped_rope_restores_bit_for_bit() {
        let post = Solid::Capsule {
            a: DVec3::new(0.0, -1.0, -2.0),
            b: DVec3::new(0.0, -1.0, 2.0),
            radius: 0.4,
        };
        let a = DVec3::new(-3.0, 0.0, 0.0);
        let mut rope = Rope::new(a, DVec3::new(3.0, 0.0, 0.0), 6.7, 40, wrapping());
        let end = |k: u32| DVec3::new(3.0, -2.5 * (f64::from(k) * 0.01).min(1.0), 0.0);
        for k in 0..200 {
            rope.step_among(1.0 / 120.0, a, end(k), &NoField, &[post.into()]);
        }
        let mut restored: Rope =
            serde_json::from_str(&serde_json::to_string(&rope).unwrap()).unwrap();
        assert_eq!(restored, rope);
        for k in 200..400 {
            rope.step_among(1.0 / 120.0, a, end(k), &NoField, &[post.into()]);
            restored.step_among(1.0 / 120.0, a, end(k), &NoField, &[post.into()]);
        }
        assert_eq!(restored, rope);
        assert!(rope.touch.contains(&1));
    }

    #[test]
    fn a_taut_path_bends_only_where_a_solid_blocks_the_view() {
        let post = Solid::Cylinder {
            a: DVec3::new(0.0, 0.0, -3.0),
            b: DVec3::new(0.0, 0.0, 3.0),
            radius: 0.5,
        };
        // A slack rope bowed well clear of the post, then one draped over it.
        let (a, b) = (DVec3::new(-4.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 0.0));
        let clear = Rope::new(a, b + DVec3::Y * 3.0, 12.0, 48, wrapping());
        assert_eq!(clear.bends(&[post.into()]).0, Vec::<usize>::new());
        let mut rope = Rope::new(a, DVec3::new(-4.0, 3.0, 0.0), 9.0, 64, wrapping());
        let swing = |angle: f64| DVec3::new(-4.9 * angle.cos(), 3.0 * angle.sin(), 0.0);
        for k in 0..=480 {
            let end = swing(std::f64::consts::PI * f64::from(k) / 480.0);
            rope.step_among(1.0 / 120.0, a, end, &NoField, &[post.into()]);
        }
        let (bends, length) = rope.bends(&[post.into()]);
        assert!(!bends.is_empty());
        // The bends lie on the post's top, and the path is about as long as
        // the rope from the anchor to the last bend.
        for &k in &bends {
            assert!(rope.pos[k].y > 0.3 && post.distance(rope.pos[k]).0 < 0.05);
        }
        let last = *bends.last().unwrap();
        assert!(length > rope.pos[0].distance(rope.pos[last]) - 1e-9);
        assert!(length < rope.length);
    }
}
