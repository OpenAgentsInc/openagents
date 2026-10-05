//! Running effects: [`Particles`] starts effects from the [`Library`],
//! moves their emitters, steps every particle, and writes [`Sprite`]s.
//!
//! The simulation is deterministic: the same seed, the same calls, and the
//! same time steps give the same particles on every platform.

use std::sync::Arc;

use glam::{Quat, Vec3};

use super::def::{Animate, Effect, Emitter, Light, Orient, Shape};
use super::library::Library;
use super::sprite::{Facing, Sprite};

/// Most particles one [`Particles`] keeps alive; past it, new particles
/// replace the oldest.
pub const MAX_PARTICLES: usize = 4096;
/// Most effects running at once.
pub const MAX_EFFECTS: usize = 256;

/// A running effect, to move or stop it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle(u64);

/// Where and how an effect starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub at: Vec3,
    /// The effect's own velocity, which emitters with `inherit` pass on.
    pub velocity: Vec3,
    /// The effect's +Y axis: emitter directions turn with it.
    pub axis: Vec3,
    /// Scales sizes, speeds, radii, and tails, for a smaller or larger
    /// copy of the same effect.
    pub scale: f32,
}

impl Spawn {
    #[must_use]
    pub fn at(at: Vec3) -> Self {
        Self {
            at,
            velocity: Vec3::ZERO,
            axis: Vec3::Y,
            scale: 1.0,
        }
    }

    #[must_use]
    pub fn scaled(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    #[must_use]
    pub fn moving(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    #[must_use]
    pub fn along(mut self, axis: Vec3) -> Self {
        self.axis = axis.normalize_or(Vec3::Y);
        self
    }
}

#[derive(Clone, Debug)]
struct Running {
    handle: Handle,
    effect: usize,
    spawn: Spawn,
    age: f32,
    /// Fractional particles owed to each emitter's rate.
    owed: Vec<f32>,
    /// Whether each emitter's burst has fired.
    burst: Vec<bool>,
    stopped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Particle {
    /// The running effect's handle.
    owner: u64,
    effect: u16,
    emitter: u16,
    at: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    half: f32,
    angle: f32,
    spin: f32,
    /// The frame offset for random and looping frames.
    frame: u32,
    scale: f32,
    /// Where the effect stood at its birth and its axis, which `swirl` and
    /// `pull` turn it about.
    center: Vec3,
    axis: Vec3,
}

/// The particles of every running effect.
pub struct Particles {
    library: Arc<Library>,
    generation: u64,
    running: Vec<Running>,
    particles: Vec<Particle>,
    next_handle: u64,
    rng: u32,
}

impl Particles {
    /// An empty system over the shared effect library, its randomness
    /// seeded by `seed`.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        let (library, generation) = Library::shared();
        Self::with_library(library, generation, seed)
    }

    /// An empty system over `library`.
    #[must_use]
    pub fn with_library(library: Arc<Library>, generation: u64, seed: u32) -> Self {
        Self {
            library,
            generation,
            running: Vec::new(),
            particles: Vec::new(),
            next_handle: 1,
            rng: seed.max(1),
        }
    }

    /// Ends every effect and particle.
    pub fn clear(&mut self) {
        self.running.clear();
        self.particles.clear();
    }

    /// Particles alive.
    #[must_use]
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// Effects running, including stopped ones whose particles live on.
    #[must_use]
    pub fn effects(&self) -> usize {
        self.running.len()
    }

    /// Starts the effect called `name`. Returns `None` for an unknown
    /// effect, or when [`MAX_EFFECTS`] are running.
    pub fn start(&mut self, name: &str, spawn: Spawn) -> Option<Handle> {
        let effect = self.library.index(name)?;
        if self.running.len() >= MAX_EFFECTS || !spawn.at.is_finite() {
            return None;
        }
        let count = self.library.effects[effect].emitters.len();
        let handle = Handle(self.next_handle);
        self.next_handle += 1;
        self.running.push(Running {
            handle,
            effect,
            spawn: Spawn {
                axis: spawn.axis.normalize_or(Vec3::Y),
                scale: spawn.scale.max(0.0),
                ..spawn
            },
            age: 0.0,
            owed: vec![0.0; count],
            burst: vec![false; count],
            stopped: false,
        });
        Some(handle)
    }

    /// Moves a running effect, so a trail follows its meteor.
    pub fn place(&mut self, handle: Handle, at: Vec3, velocity: Vec3) {
        if let Some(r) = self.running.iter_mut().find(|r| r.handle == handle)
            && at.is_finite()
            && velocity.is_finite()
        {
            r.spawn.at = at;
            r.spawn.velocity = velocity;
        }
    }

    /// Stops an effect's emitters; its particles live out their lives.
    pub fn stop(&mut self, handle: Handle) {
        if let Some(r) = self.running.iter_mut().find(|r| r.handle == handle) {
            r.stopped = true;
        }
    }

    /// Whether the effect still emits or has particles alive.
    #[must_use]
    pub fn alive(&self, handle: Handle) -> bool {
        self.running.iter().any(|r| r.handle == handle)
    }

    /// Steps every effect and particle by `dt` seconds; `ground` gives the
    /// ground's height under a point, for particles that bounce.
    pub fn tick(&mut self, dt: f32, ground: impl Fn(f32, f32) -> f32) {
        let (library, generation) = Library::shared_if_newer(self.generation);
        if let Some(library) = library {
            // A reloaded library may have other effects and emitters; the
            // running ones would point at the wrong ones.
            self.library = library;
            self.generation = generation;
            self.clear();
        }
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        let library = self.library.clone();
        // Emit.
        let mut running = std::mem::take(&mut self.running);
        for r in &mut running {
            let effect = &library.effects[r.effect];
            let before = r.age;
            r.age += dt;
            if r.stopped {
                continue;
            }
            for (i, emitter) in effect.emitters.iter().enumerate() {
                if r.age < emitter.delay {
                    continue;
                }
                if !r.burst[i] {
                    r.burst[i] = true;
                    for _ in 0..emitter.burst {
                        self.emit(r.handle, r.effect, i, emitter, &r.spawn);
                    }
                }
                if emitter.rate > 0.0 {
                    let start = before.max(emitter.delay);
                    let end = if emitter.duration > 0.0 {
                        r.age.min(emitter.delay + emitter.duration)
                    } else {
                        r.age
                    };
                    if end > start {
                        r.owed[i] += emitter.rate * (end - start);
                        let n = r.owed[i].floor();
                        r.owed[i] -= n;
                        // Spread along the step so a fast trail stays
                        // continuous between frames.
                        let total = n as u32;
                        for k in 0..total {
                            let back = r.spawn.velocity * dt * (k as f32 / total.max(1) as f32);
                            let spawn = Spawn {
                                at: r.spawn.at - back,
                                ..r.spawn
                            };
                            self.emit(r.handle, r.effect, i, emitter, &spawn);
                        }
                    }
                }
            }
            // Emitters done: the effect stops by itself.
            let done = effect
                .emitters
                .iter()
                .all(|e| e.rate <= 0.0 || (e.duration > 0.0 && r.age >= e.delay + e.duration))
                && r.burst.iter().all(|b| *b);
            if done {
                r.stopped = true;
            }
        }
        // Step.
        let mut rng = self.rng;
        for p in &mut self.particles {
            let e = &library.effects[p.effect as usize].emitters[p.emitter as usize];
            p.age += dt;
            if e.wander > 0.0 {
                let mut unit = || {
                    rng ^= rng << 13;
                    rng ^= rng >> 17;
                    rng ^= rng << 5;
                    (rng >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
                };
                let push = Vec3::new(unit(), unit(), unit());
                p.vel += push * e.wander * dt;
            }
            p.vel.y += e.gravity * p.scale.max(0.05) * dt;
            p.vel *= (1.0 - e.drag * dt).max(0.0);
            p.at += p.vel * dt;
            if e.swirl != 0.0 || e.pull > 0.0 {
                let offset = p.at - p.center;
                let along = p.axis * offset.dot(p.axis);
                let turn = Quat::from_axis_angle(p.axis, e.swirl * dt);
                let keep = (1.0 - e.pull * dt).max(0.0);
                p.at = p.center + along + turn * (offset - along) * keep;
                p.vel = turn * p.vel;
            }
            p.angle += p.spin * dt;
            if e.bounce > 0.0 {
                let floor = ground(p.at.x, p.at.z) + 0.03;
                if p.at.y < floor {
                    p.at.y = floor;
                    p.vel = Vec3::new(
                        p.vel.x * e.bounce,
                        -p.vel.y * e.bounce * 0.5,
                        p.vel.z * e.bounce,
                    );
                }
            }
        }
        self.rng = rng;
        self.particles.retain(|p| p.age < p.life);
        // A stopped effect is gone once its last particle is.
        let mut live: Vec<u64> = self.particles.iter().map(|p| p.owner).collect();
        live.sort_unstable();
        live.dedup();
        running.retain(|r| !r.stopped || live.binary_search(&r.handle.0).is_ok());
        self.running = running;
    }

    fn emit(&mut self, owner: Handle, effect: usize, index: usize, e: &Emitter, spawn: &Spawn) {
        if self.particles.len() >= MAX_PARTICLES {
            self.particles.remove(0);
        }
        let s = spawn.scale;
        let turn = Quat::from_rotation_arc(Vec3::Y, spawn.axis);
        let offset = match e.shape {
            Shape::Point => Vec3::ZERO,
            Shape::Sphere => self.in_ball() * e.radius,
            Shape::Disc => {
                let (a, r) = (self.unit() * std::f32::consts::TAU, self.unit().sqrt());
                Vec3::new(a.cos() * r, 0.0, a.sin() * r) * e.radius
            }
            Shape::Ring => {
                let a = self.unit() * std::f32::consts::TAU;
                Vec3::new(a.cos(), 0.0, a.sin()) * e.radius
            }
        } * s;
        let direction = self.in_cone(Vec3::from(e.direction).normalize(), e.spread);
        let speed = self.between(e.speed) * s;
        let radial = offset.normalize_or_zero() * self.between(e.radial) * s;
        let vel = turn * (direction * speed + radial) + spawn.velocity * e.inherit;
        let (first, last) = e.frame_range();
        let span = last - first + 1;
        let frame = match e.animate {
            Animate::Life => 0,
            Animate::Random | Animate::Loop => self.next() % span,
        };
        let angle = if e.rotate {
            self.unit() * std::f32::consts::TAU
        } else {
            0.0
        };
        let spin = self.between(e.spin);
        let life = self.between(e.life);
        let half = self.between(e.size) * s;
        self.particles.push(Particle {
            owner: owner.0,
            effect: effect as u16,
            emitter: index as u16,
            at: spawn.at + turn * offset,
            vel,
            age: 0.0,
            life: life.max(1e-3),
            half,
            angle,
            spin,
            frame,
            scale: s,
            center: spawn.at,
            axis: spawn.axis,
        });
    }

    /// Appends a sprite for every live particle.
    pub fn draw(&self, out: &mut Vec<Sprite>) {
        out.reserve(self.particles.len());
        for p in &self.particles {
            let e = &self.library.effects[p.effect as usize].emitters[p.emitter as usize];
            out.push(sprite(e, p));
        }
    }
}

fn sprite(e: &Emitter, p: &Particle) -> Sprite {
    let t = (p.age / p.life).clamp(0.0, 1.0);
    let sheet = e.sheet();
    let (first, last) = e.frame_range();
    let span = last - first + 1;
    // Which two frames, and how far between them.
    let (a, b, mix) = match e.animate {
        Animate::Life => {
            let f = t * (span - 1) as f32;
            let a = f.floor().min((span - 1) as f32);
            (a as u32, (a as u32 + 1).min(span - 1), f - a)
        }
        Animate::Random => (p.frame, p.frame, 0.0),
        Animate::Loop => {
            let f = p.frame as f32 + p.age * e.fps;
            let a = f.floor();
            (a as u32 % span, (a as u32 + 1) % span, f - a)
        }
    };
    let color = e.color.at(t);
    let alpha = e.alpha.at(t).clamp(0.0, 1.0);
    let luminance = match e.light {
        Light::Emit => e.luminance,
        Light::Lit => 1.0,
    };
    let tail = if e.stretch > 0.0 {
        let tail = -p.vel * e.stretch;
        tail.clamp_length_max(e.stretch_max * p.scale)
    } else {
        Vec3::ZERO
    };
    Sprite {
        at: p.at,
        half: p.half * e.scale.at(t).max(0.0),
        angle: p.angle,
        tail,
        facing: match e.orient {
            Orient::Camera => Facing::Camera,
            Orient::Ground => Facing::Ground,
        },
        color: color.map(|c| c * luminance),
        alpha,
        additive: e.additive_at(t),
        lit: e.light == Light::Lit,
        layer: super::sheet::layer(&e.sheet).unwrap_or(0),
        rect_a: sheet.rect(first + a),
        rect_b: sheet.rect(first + b),
        mix,
        priority: e.priority,
    }
}

impl Particles {
    /// A value in `0..1`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, [a, b]: [f32; 2]) -> f32 {
        a + (b - a) * self.unit()
    }

    fn in_ball(&mut self) -> Vec3 {
        loop {
            let p = Vec3::new(self.unit(), self.unit(), self.unit()) * 2.0 - Vec3::ONE;
            if p.length_squared() <= 1.0 {
                return p;
            }
        }
    }

    /// A direction within `spread` degrees of `axis`, uniform over the cap.
    fn in_cone(&mut self, axis: Vec3, spread: f32) -> Vec3 {
        let cos_max = spread.to_radians().cos();
        let z = 1.0 - self.unit() * (1.0 - cos_max);
        let r = (1.0 - z * z).max(0.0).sqrt();
        let a = self.unit() * std::f32::consts::TAU;
        let local = Vec3::new(r * a.cos(), z, r * a.sin());
        Quat::from_rotation_arc(Vec3::Y, axis) * local
    }

    /// xorshift32.
    fn next(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }
}

/// The library's effect called `name`, for tools that inspect one.
#[must_use]
pub fn effect(name: &str) -> Option<Effect> {
    let (library, _) = Library::shared();
    library.get(name).cloned()
}

/// An effect's look for particles another simulation moves, such as the
/// demolition site's dust: the emitter's sheet, frames, curves, and blend,
/// applied at a life fraction the caller gives.
pub struct Style {
    library: Arc<Library>,
    effect: usize,
}

impl Style {
    /// The style of the effect called `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let (library, _) = Library::shared();
        let effect = library.index(name)?;
        Some(Self { library, effect })
    }

    /// A sprite of emitter `emitter` at `at`, `t` of the way through its
    /// life, `half` m across before the scale curve, its color times
    /// `tint`. `seed` picks the angle and, for random frames, the frame.
    #[must_use]
    pub fn sprite(
        &self,
        emitter: usize,
        at: Vec3,
        vel: Vec3,
        t: f32,
        half: f32,
        tint: [f32; 3],
        seed: u32,
    ) -> Option<Sprite> {
        let e = self.library.effects[self.effect].emitters.get(emitter)?;
        let (first, last) = e.frame_range();
        let hash = seed.wrapping_mul(0x9E37_79B9) ^ (seed >> 15);
        let p = Particle {
            owner: 0,
            effect: self.effect as u16,
            emitter: emitter as u16,
            at,
            vel,
            age: t.clamp(0.0, 1.0),
            life: 1.0,
            half,
            angle: if e.rotate {
                (hash >> 8) as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU
            } else {
                0.0
            },
            spin: 0.0,
            frame: hash % (last - first + 1),
            scale: 1.0,
            center: at,
            axis: Vec3::Y,
        };
        let mut s = sprite(e, &p);
        for (c, k) in s.color.iter_mut().zip(tint) {
            *c *= k;
        }
        Some(s)
    }
}
