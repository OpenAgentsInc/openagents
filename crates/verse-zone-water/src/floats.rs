//! Things that float: crates, barrels, and planks as rigid bodies in the
//! shared `physics` crate, held up by buoyancy and slowed by drag.
//!
//! Each body carries a cloud of sample points, each standing for an equal
//! share of its volume. A point under the surface pushes up with the weight
//! of the water it displaces (Archimedes), scaled by how deep it sits
//! within its own height, at that point, so a body tips toward the wave
//! that lifts one end and rights itself as the swell passes. Drag acts on
//! each point's velocity relative to the water, which carries a body along
//! with the waves' orbital motion and a river's current. The sea floor and
//! the banks push back as stiff, damped springs; bodies collide with one
//! another through the physics crate's contact solver. This is the
//! standard multi-point buoyancy of game engines (for example, Kerner,
//! "Water interaction model for boats in video games", *Game Developer*,
//! 2015), reimplemented here.

use glam::{DQuat, DVec3, Quat, Vec2, Vec3};
use physics::{Body, BodyId, Collider, Filter, Material, Shape, Uniform, World};
use verse_pbr::pbr::LitVertex;

/// Water's density, kg/m³.
pub const RHO: f64 = 1000.0;
/// The most bodies afloat at once; a new one replaces the oldest.
pub const MAX_BODIES: usize = 24;
/// The physics step, s.
pub const DT: f64 = 1.0 / 120.0;

/// What the water is doing at a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// The surface's height, m.
    pub height: f32,
    /// The water's velocity at the surface, m/s.
    pub velocity: Vec3,
}

/// The water and ground a body floats in and rests on.
pub trait Medium {
    /// The water over (x, z), if any.
    fn surface(&self, p: Vec2) -> Option<Surface>;
    /// The ground's height at (x, z).
    fn ground(&self, p: Vec2) -> f32;
}

/// The kinds of floating thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Crate,
    Barrel,
    Plank,
}

impl Kind {
    pub const ALL: [Self; 3] = [Self::Crate, Self::Barrel, Self::Plank];

    /// Half extents of the body's box, m (a barrel's axis is z).
    #[must_use]
    pub fn half(self) -> DVec3 {
        match self {
            Self::Crate => DVec3::splat(0.4),
            Self::Barrel => DVec3::new(0.34, 0.34, 0.47),
            Self::Plank => DVec3::new(0.16, 0.045, 1.25),
        }
    }

    /// Mass, kg: an empty slatted crate, a half-full barrel, and a pine
    /// plank, each lighter than the water it can displace.
    #[must_use]
    pub fn mass(self) -> f64 {
        match self {
            Self::Crate => 150.0,
            Self::Barrel => 95.0,
            Self::Plank => 22.0,
        }
    }

    /// Displaced volume when fully under, m³.
    #[must_use]
    pub fn volume(self) -> f64 {
        let h = self.half();
        match self {
            Self::Barrel => std::f64::consts::PI * h.x * h.y * 2.0 * h.z,
            _ => 8.0 * h.x * h.y * h.z,
        }
    }

    fn samples(self) -> Vec<DVec3> {
        let h = self.half();
        let (nx, ny, nz) = match self {
            Self::Crate => (3, 3, 3),
            Self::Barrel => (2, 2, 4),
            Self::Plank => (2, 1, 6),
        };
        let mut out = Vec::new();
        let at = |i: usize, n: usize, half: f64| {
            if n == 1 {
                0.0
            } else {
                (i as f64 / (n - 1) as f64 * 2.0 - 1.0) * half * 0.8
            }
        };
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    out.push(DVec3::new(at(i, nx, h.x), at(j, ny, h.y), at(k, nz, h.z)));
                }
            }
        }
        out
    }
}

/// One floating body.
#[derive(Clone, Debug)]
pub struct Float {
    pub kind: Kind,
    pub id: BodyId,
    samples: Vec<DVec3>,
    /// How deep the body sat last step, for splashes: under or not.
    pub wet: bool,
    /// Seconds until it may make another bobbing ripple.
    pub ripple_wait: f32,
    pub born: f32,
}

/// Something a body did that the lab shows: a splash or a ripple.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A body hit the water at this speed, m/s.
    Splash { at: Vec3, speed: f32, size: f32 },
    /// A body bobbing in the water.
    Ripple { at: Vec2, strength: f32 },
}

/// The floating bodies and their physics world.
pub struct Floats {
    pub world: World,
    pub floats: Vec<Float>,
    time: f32,
    clock: physics::FixedStep,
}

impl Default for Floats {
    fn default() -> Self {
        Self::new()
    }
}

impl Floats {
    #[must_use]
    pub fn new() -> Self {
        let mut world = World::new(DT);
        // The sea never stops moving, so nothing it carries sleeps.
        world.sleep.enabled = false;
        Self {
            world,
            floats: Vec::new(),
            time: 0.0,
            clock: physics::FixedStep::new(DT, 12),
        }
    }

    /// Drops a `kind` at `at`, turned by `yaw` and tipped a little, moving
    /// at `vel`.
    pub fn spawn(&mut self, kind: Kind, at: Vec3, yaw: f32, vel: Vec3) {
        if self.floats.len() >= MAX_BODIES {
            let oldest = self.floats.remove(0);
            self.world.remove_body(oldest.id);
        }
        let h = kind.half();
        let mut body = Body::new(
            kind.mass(),
            Body::box_inertia(kind.mass(), h * 2.0),
            at.as_dvec3(),
        );
        body.orientation = DQuat::from_rotation_y(f64::from(yaw))
            * DQuat::from_rotation_x(0.25)
            * DQuat::from_rotation_z(0.15);
        body.prev_orientation = body.orientation;
        body.vel = vel.as_dvec3();
        body.omega = DVec3::new(0.6, -0.3, 0.9);
        let id = self.world.add(body);
        let shape = match kind {
            Kind::Barrel => Shape::Capsule {
                radius: h.x,
                half_length: (h.z - h.x).max(0.05),
            },
            _ => Shape::Cuboid { half: h },
        };
        self.world.add_collider(
            Collider::new(id, shape)
                .with_filter(Filter::ALL)
                .with_material(Material {
                    friction: 0.4,
                    torsional: 0.0,
                    restitution: 0.1,
                }),
        );
        self.floats.push(Float {
            kind,
            id,
            samples: kind.samples(),
            wet: false,
            ripple_wait: 0.0,
            born: self.time,
        });
    }

    /// Advances the bodies by `dt` seconds in `medium`, pushed by `push`
    /// (an extra horizontal acceleration at a point, from a spell or the
    /// player), and returns what they did.
    pub fn tick(
        &mut self,
        dt: f32,
        medium: &impl Medium,
        push: &dyn Fn(Vec3) -> Vec3,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if !dt.is_finite() || dt <= 0.0 {
            return events;
        }
        self.time += dt;
        let steps = self.clock.advance(f64::from(dt.min(0.1)));
        for _ in 0..steps {
            self.forces(medium, push);
            self.world.step(&Uniform(DVec3::new(0.0, -9.81, 0.0)));
        }
        for float in &mut self.floats {
            let body = &self.world.bodies()[float.id.0 as usize];
            let at = body.pos.as_vec3();
            float.ripple_wait -= dt;
            let Some(surface) = medium.surface(Vec2::new(at.x, at.z)) else {
                float.wet = false;
                continue;
            };
            // The body's lowest reach meets the water first.
            let wet = at.y - float.kind.half().max_element() as f32 * 0.8 < surface.height;
            if wet && !float.wet && body.vel.y < -1.5 {
                let speed = -body.vel.y as f32;
                events.push(Event::Splash {
                    at: Vec3::new(at.x, surface.height, at.z),
                    speed,
                    size: float.kind.half().max_element() as f32,
                });
            }
            float.wet = wet;
            let moving =
                (body.vel.as_vec3() - surface.velocity).length() + body.omega.length() as f32 * 0.3;
            if wet && float.ripple_wait <= 0.0 && moving > 0.25 {
                events.push(Event::Ripple {
                    at: Vec2::new(at.x, at.z),
                    strength: (0.012 * moving).min(0.04),
                });
                float.ripple_wait = 0.45;
            }
        }
        events
    }

    fn forces(&mut self, medium: &impl Medium, push: &dyn Fn(Vec3) -> Vec3) {
        let g = 9.81;
        for float in &self.floats {
            let kind = float.kind;
            let share = kind.volume() / float.samples.len() as f64;
            // Each sample's own height, for its submerged fraction.
            let cell = kind.half().y.min(kind.half().x) * 2.0 / 2.0;
            let body = &mut self.world.bodies_mut()[float.id.0 as usize];
            let mut force = DVec3::ZERO;
            let mut torque = DVec3::ZERO;
            let w = body.omega_world();
            for local in &float.samples {
                let arm = body.orientation * *local;
                let p = body.pos + arm;
                let pv = body.vel + w.cross(arm);
                let xz = Vec2::new(p.x as f32, p.z as f32);
                let mut f = DVec3::ZERO;
                if let Some(surface) = medium.surface(xz) {
                    let under = f64::from(surface.height) - p.y;
                    let wet = ((under / cell) + 0.5).clamp(0.0, 1.0);
                    if wet > 0.0 {
                        f.y += RHO * g * share * wet;
                        let extra = push(p.as_vec3()).as_dvec3();
                        let rel = pv - surface.velocity.as_dvec3() - extra;
                        // Linear and quadratic drag, stronger vertically,
                        // so bobbing settles in a few swells.
                        let k = RHO * share * wet;
                        let lin = DVec3::new(1.2, 2.6, 1.2);
                        f -= k * (rel * lin + rel * rel.length() * 0.6);
                    }
                }
                // The ground pushes back.
                let floor = f64::from(medium.ground(xz));
                if p.y < floor {
                    let m = kind.mass() / float.samples.len() as f64;
                    let depth = floor - p.y;
                    f.y += m * (900.0 * depth - 40.0 * pv.y.min(0.0));
                    f.x -= m * 6.0 * pv.x;
                    f.z -= m * 6.0 * pv.z;
                }
                force += f;
                torque += arm.cross(f);
            }
            body.force += force;
            body.torque += torque;
        }
    }

    /// Each body's pose for drawing.
    pub fn poses(&self) -> impl Iterator<Item = (Kind, Vec3, Quat)> + '_ {
        let alpha = self.clock.alpha();
        self.floats.iter().map(move |f| {
            let (p, q) = self.world.bodies()[f.id.0 as usize].interpolated(alpha);
            (f.kind, p.as_vec3(), q.as_quat())
        })
    }

    /// Each body's position and velocity.
    pub fn states(&self) -> impl Iterator<Item = (Kind, Vec3, Vec3)> + '_ {
        self.floats.iter().map(move |f| {
            let b = &self.world.bodies()[f.id.0 as usize];
            (f.kind, b.pos.as_vec3(), b.vel.as_vec3())
        })
    }

    /// Shoves every body within `radius` of `at` away from it.
    pub fn shove(&mut self, at: Vec3, radius: f32, speed: f32) {
        for f in &self.floats {
            let body = &mut self.world.bodies_mut()[f.id.0 as usize];
            let d = body.pos.as_vec3() - at;
            let flat = Vec2::new(d.x, d.z);
            if flat.length() < radius && d.y.abs() < 1.5 {
                let push = flat.normalize_or_zero() * speed * (1.0 - flat.length() / radius);
                body.vel.x += f64::from(push.x) * 0.05;
                body.vel.z += f64::from(push.y) * 0.05;
            }
        }
    }

    /// The bodies as lit triangles.
    #[must_use]
    pub fn draw(&self) -> Vec<LitVertex> {
        let mut out = Vec::new();
        for (kind, at, q) in self.poses() {
            draw_body(&mut out, kind, at, q);
        }
        out
    }
}

/// Appends one body's triangles.
pub fn draw_body(out: &mut Vec<LitVertex>, kind: Kind, at: Vec3, q: Quat) {
    let h = kind.half().as_vec3();
    match kind {
        Kind::Crate => {
            // Slats: each face split in three, alternately lighter.
            let wood = [[0.46, 0.30, 0.16], [0.38, 0.24, 0.12], [0.50, 0.34, 0.18]];
            boxed(out, at, q, h, 3, &wood, 0.75);
            // Dark corner posts.
            for sx in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let c = Vec3::new(sx * (h.x - 0.04), 0.0, sz * (h.z - 0.04));
                    boxed(
                        out,
                        at + q * c,
                        q,
                        Vec3::new(0.05, h.y + 0.005, 0.05),
                        1,
                        &[[0.22, 0.13, 0.06]],
                        0.8,
                    );
                }
            }
        }
        Kind::Plank => boxed(
            out,
            at,
            q,
            h,
            4,
            &[[0.62, 0.46, 0.28], [0.56, 0.40, 0.24]],
            0.7,
        ),
        Kind::Barrel => barrel(out, at, q, h),
    }
}

fn vertex(p: Vec3, n: Vec3, local: Vec3, color: [f32; 3], rough: f32, metal: f32) -> LitVertex {
    let t = if n.y.abs() < 0.9 {
        Vec3::Y.cross(n).normalize()
    } else {
        Vec3::X
    };
    LitVertex {
        pos: p.to_array(),
        normal: n.to_array(),
        tangent: t.to_array(),
        local: local.to_array(),
        color,
        params: [metal, rough, 0.0, 1.0],
    }
}

/// A box of half extents `h`, each face cut into `strips` bands colored in
/// turn from `colors`.
fn boxed(
    out: &mut Vec<LitVertex>,
    at: Vec3,
    q: Quat,
    h: Vec3,
    strips: usize,
    colors: &[[f32; 3]],
    rough: f32,
) {
    let faces = [
        (Vec3::X, Vec3::Z, Vec3::Y),
        (-Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (-Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (-Vec3::Z, Vec3::X, Vec3::Y),
    ];
    for (n, u, v) in faces {
        let center = n * h;
        let (hu, hv) = (u * h, v * h);
        for s in 0..strips {
            let a = s as f32 / strips as f32 * 2.0 - 1.0;
            let b = (s + 1) as f32 / strips as f32 * 2.0 - 1.0;
            let color = colors[s % colors.len()];
            let corner = |cu: f32, cv: f32| {
                let local = center + hu * cu + hv * cv;
                vertex(at + q * local, q * n, local, color, rough, 0.0)
            };
            let quad = [
                corner(a, -1.0),
                corner(b, -1.0),
                corner(b, 1.0),
                corner(a, 1.0),
            ];
            out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
        }
    }
}

/// A barrel: staves around the z axis bulging at the middle, two iron
/// hoops, and flat heads.
fn barrel(out: &mut Vec<LitVertex>, at: Vec3, q: Quat, h: Vec3) {
    let segments = 14;
    let rings = [-1.0f32, -0.82, -0.7, -0.35, 0.0, 0.35, 0.7, 0.82, 1.0];
    let radius = |z: f32| h.x * (0.86 + 0.14 * (1.0 - z * z));
    let color = |k: usize, z: f32| -> ([f32; 3], f32, f32) {
        let hoop = (z.abs() - 0.76).abs() < 0.06 || z.abs() > 0.95;
        if hoop {
            ([0.12, 0.11, 0.10], 0.45, 1.0)
        } else if k % 2 == 0 {
            ([0.42, 0.24, 0.12], 0.7, 0.0)
        } else {
            ([0.36, 0.20, 0.10], 0.7, 0.0)
        }
    };
    for k in 0..segments {
        let a0 = k as f32 / segments as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        for r in rings.windows(2) {
            let (z0, z1) = (r[0], r[1]);
            let (c, rough, metal) = color(k, (z0 + z1) * 0.5);
            let p = |a: f32, z: f32| {
                let rr = radius(z);
                Vec3::new(a.cos() * rr, a.sin() * rr, z * h.z)
            };
            let nrm = |a: f32, z: f32| Vec3::new(a.cos(), a.sin(), -0.3 * z).normalize();
            let v = |a: f32, z: f32| {
                let local = p(a, z);
                vertex(at + q * local, q * nrm(a, z), local, c, rough, metal)
            };
            let quad = [v(a0, z0), v(a1, z0), v(a1, z1), v(a0, z1)];
            out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
        }
        for z in [-1.0f32, 1.0] {
            let n = Vec3::new(0.0, 0.0, z);
            let c = [0.40, 0.25, 0.13];
            let center = Vec3::new(0.0, 0.0, z * h.z);
            let rr = radius(z);
            let e0 = Vec3::new(a0.cos() * rr, a0.sin() * rr, z * h.z);
            let e1 = Vec3::new(a1.cos() * rr, a1.sin() * rr, z * h.z);
            for local in [center, e0, e1] {
                out.push(vertex(at + q * local, q * n, local, c, 0.75, 0.0));
            }
        }
    }
}
