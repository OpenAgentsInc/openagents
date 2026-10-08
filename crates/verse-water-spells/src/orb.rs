//! Water Orb, our own spell (homebrew), after SRD 5.2.1's Create or Destroy
//! Water (water from nothing, or from the air) and Control Water (water
//! moved and shaped at will). The SRD has no spell that shapes a sphere of
//! water and throws it, so every number here is ours.
//!
//! - **Grow.** While the key or the slot is held, a sphere of water forms
//!   in front of the caster and grows, by [`GROW_FROM_WATER`] m of radius a
//!   second while open water lies within [`DRAW_REACH`] (it draws streams up
//!   out of the sea, the river, or the pool), else by [`GROW_FROM_AIR`]
//!   (it condenses droplets out of the air), up to [`MAX_RADIUS`].
//! - **Throw.** Letting go throws it along the aim: the point under the
//!   pointer, else ahead. It flies a ballistic arc and bursts where it meets
//!   the ground, the water, a dummy, or its target, in a splash as wide as
//!   [`splash_radius`]: it wets the ground, throws floating bodies and
//!   dummies back, and, on water, spills a wave of ripples.
//! - **Hold.** Letting go with Shift sets it hovering where it is for
//!   [`HOVER_TIME`] s. Pressing the key again near a hovering orb takes it
//!   back in hand, to grow or throw.
//! - **Engulf.** Floating bodies and dummies inside a forming or hovering
//!   orb are carried in it, their weight borne by its water, swirling with
//!   it and thrown with it ([`crate::floats::Hold`]).
//! - **Lightning.** A Thunderbolt that strikes an orb electrifies all of it
//!   ([`crate::bolt`]): everything inside takes the bolt's damage, and
//!   nothing outside does.
//!
//! The orb draws through the water pass as [`Kind::Orb`] vertices: refraction
//! as a ball lens, Fresnel reflection, caustics, a wobbling skin, and the
//! lightning's glow and arcs. Droplets circle it, a size ring and its
//! diameter float with it, and the aim's ring marks where it will land.

use glam::{Vec2, Vec3};
use physics::BodyId;
use verse_core::fx::Spawn;
use verse_pbr::mesh::{Mesh, Vertex};
use verse_pbr::pbr::GlowVertex;
use verse_pbr::pbr::water::WaterVertex;

use crate::floats::Hold;
use crate::{WaterLab, ground, terrain};

/// The smallest orb, as it first forms, m of radius.
pub const MIN_RADIUS: f32 = 0.35;
/// The largest orb, m of radius: 12 m across.
pub const MAX_RADIUS: f32 = 6.0;
/// How fast the radius grows while the caster draws from open water, and
/// while the orb condenses from the air, m/s.
pub const GROW_FROM_WATER: f32 = 1.6;
pub const GROW_FROM_AIR: f32 = 0.9;
/// How near open water must be to draw from it, m.
pub const DRAW_REACH: f32 = 16.0;
/// How long a set orb hovers before it falls, s.
pub const HOVER_TIME: f32 = 60.0;
/// The most orbs at once; past it the oldest hovering one bursts.
pub const MAX_ORBS: usize = 5;
/// How fast a throw carries a small orb, m/s; a big one flies slower.
pub const THROW_SPEED: f32 = 16.0;
/// How far ahead a throw lands with nothing under the pointer, m.
pub const THROW_REACH: f32 = 22.0;
/// How near a hovering orb must be to take it back in hand, m.
pub const REGRAB_REACH: f32 = 14.0;
/// How long lightning's charge lasts in an orb, s.
pub const CHARGE_TIME: f32 = 1.8;
/// The longest an orb flies before it falls apart, s.
const FLIGHT_LIMIT: f32 = 6.0;
const GRAVITY: f32 = 9.81;

/// How far a burst of an orb of `radius` reaches, m: what it wets and
/// throws back.
#[must_use]
pub fn splash_radius(radius: f32) -> f32 {
    2.0 + 2.2 * radius
}

/// The radius an orb reaches after growing `held` seconds at `rate` m/s
/// from [`MIN_RADIUS`], clamped to [`MAX_RADIUS`].
#[must_use]
pub fn grown(held: f32, rate: f32) -> f32 {
    (MIN_RADIUS + held.max(0.0) * rate).min(MAX_RADIUS)
}

/// The velocity that carries an orb from `from` to `to` on a ballistic arc,
/// and the flight's time, s: about [`THROW_SPEED`] along the way for a small
/// orb, slower for a big one.
#[must_use]
pub fn throw(from: Vec3, to: Vec3, radius: f32) -> (Vec3, f32) {
    let time = (from.distance(to) / THROW_SPEED).clamp(0.35, 2.2) * (1.0 + radius * 0.06);
    let velocity = (to - from) / time + Vec3::Y * (0.5 * GRAVITY * time);
    (velocity, time)
}

/// What an orb is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// Held in front of the caster, growing while the key is down.
    Forming,
    /// Thrown toward `target`.
    Flying { target: Vec3 },
    /// Set in place until `until`, s.
    Hovering { until: f32 },
}

/// Where a forming orb draws its water from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Sea,
    River,
    /// No open water near: it condenses from the air.
    Air,
}

impl Source {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Sea => "drawing from the sea",
            Self::River => "drawing from the river",
            Self::Air => "condensing from the air",
        }
    }
}

/// One Water Orb.
#[derive(Clone, Debug, PartialEq)]
pub struct Orb {
    pub id: u32,
    pub center: Vec3,
    pub velocity: Vec3,
    pub radius: f32,
    pub state: State,
    pub born: f32,
    /// How electrified it is, 1 just struck down to 0.
    pub charge: f32,
    /// How hard its skin wobbles, 0 to 1, over its calm swell.
    pub wobble: f32,
    /// Where it draws water from while forming: a point on the water's
    /// surface, or none from the air.
    pub draw: Option<Vec3>,
    pub source: Source,
    /// How strongly its streams run, 0 to 1, easing in and out.
    pub feed: f32,
    /// The floating bodies and the dummies (by index) inside it.
    pub floats: Vec<BodyId>,
    pub dummies: Vec<usize>,
    /// Where it was set hovering.
    rest: Vec3,
    flown: f32,
    seed: u32,
}

impl Orb {
    #[must_use]
    pub fn new(id: u32, center: Vec3, now: f32) -> Self {
        Self {
            id,
            center,
            velocity: Vec3::ZERO,
            radius: MIN_RADIUS,
            state: State::Forming,
            born: now,
            charge: 0.0,
            wobble: 0.6,
            draw: None,
            source: Source::Air,
            feed: 0.0,
            floats: Vec::new(),
            dummies: Vec::new(),
            rest: center,
            flown: 0.0,
            seed: id.wrapping_mul(0x9E37_79B9),
        }
    }

    /// Whether `p` lies inside it, `margin` m in from its skin.
    #[must_use]
    pub fn contains(&self, p: Vec3, margin: f32) -> bool {
        p.distance(self.center) < self.radius - margin
    }

    /// The hold its water puts on what it carries.
    #[must_use]
    pub fn hold(&self) -> Hold {
        Hold {
            center: self.center,
            velocity: self.velocity,
            radius: self.radius,
        }
    }

    /// Where a ray from `origin` along unit `direction` first meets its
    /// skin, as a distance along the ray.
    #[must_use]
    pub fn ray(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let to = origin - self.center;
        let b = to.dot(direction);
        let c = to.length_squared() - self.radius * self.radius;
        let h = b * b - c;
        if h < 0.0 {
            return None;
        }
        let t = -b - h.sqrt();
        if t >= 0.0 {
            Some(t)
        } else {
            // From inside, it is the skin ahead.
            let t = -b + h.sqrt();
            (t >= 0.0).then_some(t)
        }
    }

    /// Whether it hovers.
    #[must_use]
    pub fn hovering(&self) -> bool {
        matches!(self.state, State::Hovering { .. })
    }
}

/// Where an orb of `radius` forms for a caster at `feet` facing `forward`:
/// its near side a little ahead and its bottom near the caster's feet.
#[must_use]
pub fn in_hand(feet: Vec3, forward: Vec3, radius: f32) -> Vec3 {
    let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::NEG_Z);
    feet + flat * (1.2 + radius) + Vec3::Y * (1.3 + 0.75 * radius)
}

impl WaterLab {
    /// The surface a thrown orb or a bolt meets at (x, z): the water's,
    /// else the ground's.
    #[must_use]
    pub fn top_at(&self, x: f32, z: f32) -> f32 {
        self.surface_at(Vec2::new(x, z))
            .map_or_else(|| ground(x, z), |s| s.height.max(ground(x, z)))
    }

    /// Where a ray from `origin` along `direction` first meets the ground,
    /// the water, an orb (other than `skip`), or a dummy, within 90 m, and
    /// the orb it met, if any.
    #[must_use]
    pub fn ray_hit(
        &self,
        origin: Vec3,
        direction: Vec3,
        skip: Option<u32>,
    ) -> Option<(Vec3, Option<usize>)> {
        let direction = direction.try_normalize()?;
        let mut best: Option<(f32, Option<usize>)> = None;
        for (i, orb) in self.orbs.iter().enumerate() {
            if Some(orb.id) == skip {
                continue;
            }
            if let Some(t) = orb.ray(origin, direction)
                && best.is_none_or(|(b, _)| t < b)
            {
                best = Some((t, Some(i)));
            }
        }
        for target in &self.targets {
            let d = &target.dummy;
            let top = d.top();
            // A dummy is an upright cylinder 0.45 m across its scale.
            let r = 0.45 * d.kind.scale();
            let o = Vec2::new(origin.x - d.pos.x, origin.z - d.pos.z);
            let v = Vec2::new(direction.x, direction.z);
            let a = v.length_squared();
            if a < 1e-6 {
                continue;
            }
            let b = o.dot(v);
            let c = o.length_squared() - r * r;
            let h = b * b - a * c;
            if h < 0.0 {
                continue;
            }
            let t = (-b - h.sqrt()) / a;
            let y = origin.y + direction.y * t;
            if t > 0.0 && y > d.pos.y && y < top && best.is_none_or(|(b, _)| t < b) {
                best = Some((t, None));
            }
        }
        // March the ground and the water.
        let limit = best.map_or(90.0, |(t, _)| t);
        let mut t = 0.0;
        let step = 0.25;
        let under = |p: Vec3| p.y < self.top_at(p.x, p.z);
        if !under(origin) {
            while t < limit {
                let next = (t + step).min(limit);
                if under(origin + direction * next) {
                    let (mut lo, mut hi) = (t, next);
                    for _ in 0..12 {
                        let mid = (lo + hi) * 0.5;
                        if under(origin + direction * mid) {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                    }
                    best = Some((hi, None));
                    break;
                }
                t = next;
            }
        }
        best.map(|(t, orb)| (origin + direction * t, orb))
    }

    /// Where the pointer aims, or `reach` m ahead of `at` on the surface.
    #[must_use]
    pub fn aim_point(&self, at: Vec3, forward: Vec3, reach: f32, skip: Option<u32>) -> Vec3 {
        if let Some((origin, direction)) = self.aim
            && let Some((p, _)) = self.ray_hit(origin, direction, skip)
        {
            return p;
        }
        let flat = Vec2::new(forward.x, forward.z).normalize_or(Vec2::NEG_Y);
        let (x, z) = (at.x + flat.x * reach, at.z + flat.y * reach);
        Vec3::new(x, self.top_at(x, z), z)
    }

    /// The orb forming in front of the caster, if any.
    #[must_use]
    pub fn forming(&self) -> Option<&Orb> {
        self.orbs.iter().find(|o| o.state == State::Forming)
    }

    /// Starts a Water Orb: takes back the nearest hovering orb within
    /// [`REGRAB_REACH`] of the caster at `at`, or forms a new one in front.
    pub fn begin_orb(&mut self, at: Vec3, forward: Vec3) -> String {
        if self.forming().is_some() {
            return "Water Orb: already forming".into();
        }
        let near = self
            .orbs
            .iter()
            .enumerate()
            .filter(|(_, o)| o.hovering())
            .map(|(i, o)| (i, o.center.distance(at) - o.radius))
            .filter(|(_, d)| *d < REGRAB_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = near {
            let orb = &mut self.orbs[i];
            orb.state = State::Forming;
            orb.wobble = 1.0;
            return format!(
                "Water Orb: you take the {:.1} m orb back in hand",
                orb.radius * 2.0
            );
        }
        while self.orbs.len() >= MAX_ORBS {
            let Some(oldest) = self.orbs.iter().position(Orb::hovering) else {
                break;
            };
            let orb = self.orbs[oldest].clone();
            self.burst(oldest, Vec3::new(orb.center.x, orb.center.y, orb.center.z));
        }
        if self.orbs.len() >= MAX_ORBS {
            return "Water Orb: too many orbs in the air".into();
        }
        let id = self.next_orb;
        self.next_orb += 1;
        let center = in_hand(at, forward, MIN_RADIUS);
        self.orbs.push(Orb::new(id, center, self.time));
        "Water Orb: water gathers in front of you".into()
    }

    /// Lets go of the forming orb: throws it along the aim, or with `hold`
    /// sets it hovering where it is.
    pub fn release_orb(&mut self, hold: bool, at: Vec3, forward: Vec3) -> Option<String> {
        let i = self.orbs.iter().position(|o| o.state == State::Forming)?;
        let now = self.time;
        if hold {
            let orb = &mut self.orbs[i];
            orb.state = State::Hovering {
                until: now + HOVER_TIME,
            };
            orb.rest = orb.center;
            orb.velocity = Vec3::ZERO;
            let size = orb.radius * 2.0;
            return Some(self.say(format!("Water Orb: a {size:.1} m orb hovers for a minute")));
        }
        let id = self.orbs[i].id;
        let mut target = self.aim_point(at, forward, THROW_REACH, Some(id));
        let orb = &self.orbs[i];
        // A target inside or just past the orb's own skin goes on ahead.
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::NEG_Z);
        if Vec2::new(target.x - orb.center.x, target.z - orb.center.z).length() < orb.radius + 2.0 {
            let x = orb.center.x + flat.x * (orb.radius + 6.0);
            let z = orb.center.z + flat.z * (orb.radius + 6.0);
            target = Vec3::new(x, self.top_at(x, z), z);
        }
        let (velocity, _) = throw(orb.center, target, orb.radius);
        let orb = &mut self.orbs[i];
        orb.state = State::Flying { target };
        orb.velocity = velocity;
        orb.flown = 0.0;
        orb.wobble = 1.0;
        // What it holds flies with it.
        let floats = orb.floats.clone();
        let dummies = orb.dummies.clone();
        let size = orb.radius * 2.0;
        for id in floats {
            if let Some(body) = self.floats.world.bodies_mut().get_mut(id.0 as usize) {
                body.vel = velocity.as_dvec3();
            }
        }
        for d in dummies {
            if let Some(target) = self.targets.get_mut(d) {
                target.vel = velocity;
            }
        }
        Some(self.say(format!("Water Orb: you hurl the {size:.1} m orb")))
    }

    /// Advances the orbs `dt` seconds: the forming one grows and follows
    /// the caster, thrown ones fly and burst, hovering ones bob and in time
    /// fall, and every orb takes in what it touches.
    pub(crate) fn tick_orbs(&mut self, dt: f32) {
        let now = self.time;
        self.spills.retain(|s| now - s.start < SPILL_TIME);
        let (feet, forward) = self.caster;
        let mut bursts = Vec::new();
        for i in 0..self.orbs.len() {
            let before = self.orbs[i].center;
            match self.orbs[i].state {
                State::Forming => {
                    let (draw, source) = self.water_near(self.orbs[i].center, self.orbs[i].radius);
                    let orb = &mut self.orbs[i];
                    orb.draw = draw;
                    orb.source = source;
                    let rate = if source == Source::Air {
                        GROW_FROM_AIR
                    } else {
                        GROW_FROM_WATER
                    };
                    let growing = orb.radius < MAX_RADIUS;
                    orb.radius = (orb.radius + rate * dt).min(MAX_RADIUS);
                    let want = in_hand(feet, forward, orb.radius);
                    orb.center += (want - orb.center) * (dt * 6.0).min(1.0);
                    let target = if growing { 1.0 } else { 0.0 };
                    orb.feed += (target - orb.feed) * (dt * 4.0).min(1.0);
                }
                State::Hovering { until } => {
                    let orb = &mut self.orbs[i];
                    orb.feed = (orb.feed - dt * 3.0).max(0.0);
                    let bob = 0.18 * ((now - orb.born) * 1.3 + orb.id as f32).sin();
                    orb.center = orb.rest + Vec3::Y * bob;
                    if now > until {
                        // It falls as its hold gives out.
                        orb.state = State::Flying {
                            target: Vec3::new(orb.center.x, -100.0, orb.center.z),
                        };
                        orb.velocity = Vec3::ZERO;
                    }
                }
                State::Flying { target } => {
                    let orb = &mut self.orbs[i];
                    orb.feed = 0.0;
                    orb.flown += dt;
                    orb.velocity.y -= GRAVITY * dt;
                    orb.center += orb.velocity * dt;
                    let c = orb.center;
                    let r = orb.radius;
                    let id = orb.id;
                    let flown = orb.flown;
                    let toward = (target - c).dot(orb.velocity);
                    let surface = self.top_at(c.x, c.z);
                    let mut hit = None;
                    if c.y - r * 0.7 <= surface {
                        hit = Some(Vec3::new(c.x, surface, c.z));
                    } else if toward < 0.0 && c.y < target.y + r {
                        hit = Some(c);
                    } else if flown > FLIGHT_LIMIT {
                        hit = Some(c);
                    }
                    // A dummy it isn't carrying stops it.
                    let carried = &self.orbs[i].dummies;
                    if hit.is_none()
                        && self.targets.iter().enumerate().any(|(k, t)| {
                            !carried.contains(&k) && t.dummy.center().distance(c) < r + 0.35
                        })
                    {
                        hit = Some(c);
                    }
                    // Another orb takes it in.
                    if hit.is_none()
                        && let Some(j) = self.orbs.iter().position(|o| {
                            o.id != id
                                && o.state != State::Forming
                                && o.center.distance(c) < o.radius + r
                        })
                    {
                        bursts.push((i, None, Some(j)));
                        continue;
                    }
                    if let Some(at) = hit {
                        bursts.push((i, Some(at), None));
                    }
                }
            }
            let orb = &mut self.orbs[i];
            if !matches!(orb.state, State::Flying { .. }) {
                orb.velocity = (orb.center - before) / dt.max(1e-4);
            }
            orb.charge = (orb.charge - dt / CHARGE_TIME).max(0.0);
            orb.wobble = (orb.wobble - dt * 0.6).max(0.0);
        }
        // Merges and bursts, last first so indices hold.
        bursts.sort_by_key(|b| std::cmp::Reverse(b.0));
        for (i, at, into) in bursts {
            match (at, into) {
                (_, Some(j)) => self.merge(i, j),
                (Some(at), None) => self.burst(i, at),
                _ => {}
            }
        }
        self.engulf();
        self.feed_fx(dt);
    }

    /// Orb `from` pours into orb `into`: their volumes add, up to
    /// [`MAX_RADIUS`], and so do their contents.
    fn merge(&mut self, from: usize, into: usize) {
        let orb = self.orbs.remove(from);
        let into = if into > from { into - 1 } else { into };
        let target = &mut self.orbs[into];
        let volume = target.radius.powi(3) + orb.radius.powi(3);
        target.radius = volume.cbrt().min(MAX_RADIUS);
        target.wobble = 1.0;
        target.floats.extend(orb.floats);
        target.dummies.extend(orb.dummies);
        self.fx.start(
            "water_splash",
            Spawn::at(orb.center).scaled((0.6 + 0.3 * orb.radius).min(2.5)),
        );
        self.say("Water Orb: the orbs pour together".into());
    }

    /// Orb `i` bursts at `at`: a splash as wide as its size, the ground
    /// wet, floating bodies and dummies thrown back, and on water a wave.
    pub(crate) fn burst(&mut self, i: usize, at: Vec3) {
        let orb = self.orbs.remove(i);
        let r = orb.radius;
        let reach = splash_radius(r);
        let p = Vec2::new(at.x, at.z);
        let on_water = self.surface_at(p).is_some();
        // The splash: the orb's water thrown up as blobs on their arcs, a
        // crown of spray at the center, and more around it.
        if self.spills.len() >= MAX_SPILLS {
            self.spills.remove(0);
        }
        self.spills.push(Spill {
            at,
            radius: r,
            velocity: orb.velocity,
            start: self.time,
            seed: orb.seed,
        });
        let scale = (0.9 + 0.6 * r).min(4.5);
        self.fx.start("water_splash", Spawn::at(at).scaled(scale));
        let ring = (r * 1.5).round().clamp(0.0, 8.0) as usize;
        for k in 0..ring {
            let a = k as f32 / ring as f32 * std::f32::consts::TAU + orb.id as f32;
            let q = Vec3::new(at.x + a.cos() * r * 0.8, 0.0, at.z + a.sin() * r * 0.8);
            let q = Vec3::new(q.x, self.top_at(q.x, q.z), q.z);
            self.fx.start(
                "water_splash",
                Spawn::at(q).scaled((0.6 + 0.25 * r).min(2.2)),
            );
        }
        if r > 1.5 {
            self.fx.start(
                "water_mist",
                Spawn::at(at + Vec3::Y * 0.5).scaled(0.6 + 0.2 * r),
            );
        }
        if on_water {
            // The wave it spills: a strong ring at the center and rings
            // around it.
            self.water.add_ripple(p, (0.03 + 0.016 * r).min(0.12));
            let rings = (2.0 + r).round() as usize;
            for k in 0..rings {
                let a = k as f32 / rings as f32 * std::f32::consts::TAU;
                self.water.add_ripple(
                    p + Vec2::new(a.cos(), a.sin()) * r * 0.7,
                    (0.02 + 0.008 * r).min(0.07),
                );
            }
        } else {
            // It soaks the ground it lands on.
            self.spells.wet = Some((p, 1.0));
            self.spells.wet_radius = (reach * 0.6).clamp(2.0, 12.0);
        }
        // What it carried spills out with its motion.
        for id in &orb.floats {
            if let Some(body) = self.floats.world.bodies_mut().get_mut(id.0 as usize) {
                body.vel = (orb.velocity * 0.4).as_dvec3();
            }
        }
        self.floats.blast(at, reach, 3.0 + 1.6 * r);
        let now = self.time;
        for (k, target) in self.targets.iter_mut().enumerate() {
            let d = target.dummy.center() - at;
            let near = 1.0 - d.length() / reach;
            if orb.dummies.contains(&k) {
                target.vel = orb.velocity * 0.3 + Vec3::Y * 2.0;
                continue;
            }
            if near <= 0.0 {
                continue;
            }
            let out = Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X);
            target.vel += (out * (2.0 + 1.2 * r) + Vec3::Y * (1.0 + 0.4 * r)) * near;
            target.dummy.hit_at = now;
            target.touched = now;
            self.floaters.push(crate::targets::floater(
                target.dummy.top(),
                target.dummy.pos,
                "Soaked",
                [0.55, 0.8, 1.0],
                now,
            ));
        }
        let size = r * 2.0;
        let line = if on_water {
            format!("Water Orb: the {size:.1} m orb crashes into the water")
        } else {
            format!("Water Orb: the {size:.1} m orb bursts in a {reach:.0} m splash")
        };
        self.say(line);
    }

    /// Takes into each forming or hovering orb the floating bodies and the
    /// dummies inside it, and lets go of what has left it.
    fn engulf(&mut self) {
        let taken: Vec<BodyId> = self
            .orbs
            .iter()
            .flat_map(|o| o.floats.iter().copied())
            .collect();
        let held: Vec<usize> = self
            .orbs
            .iter()
            .flat_map(|o| o.dummies.iter().copied())
            .collect();
        for orb in &mut self.orbs {
            let open = !matches!(orb.state, State::Flying { .. });
            // Let go of what has slipped well out of it.
            let (c, r) = (orb.center, orb.radius);
            let world = &self.floats.world;
            orb.floats.retain(|id| {
                world
                    .bodies()
                    .get(id.0 as usize)
                    .is_some_and(|b| b.pos.as_vec3().distance(c) < r + 1.5)
            });
            if !open {
                continue;
            }
            for f in &self.floats.floats {
                if taken.contains(&f.id) || orb.floats.contains(&f.id) {
                    continue;
                }
                let size = f.kind.half().max_element() as f32;
                let p = world.bodies()[f.id.0 as usize].pos.as_vec3();
                if orb.radius > size * 1.3 && orb.contains(p, size * 0.3) {
                    orb.floats.push(f.id);
                }
            }
            for (k, target) in self.targets.iter().enumerate() {
                if held.contains(&k) || orb.dummies.contains(&k) {
                    continue;
                }
                let tall = crate::targets::HEIGHT * target.dummy.kind.scale();
                if orb.radius > tall * 0.62 && orb.contains(target.dummy.center(), tall * 0.15) {
                    orb.dummies.push(k);
                }
            }
        }
        // Each body's hold for the physics step.
        self.floats.holds = self
            .orbs
            .iter()
            .flat_map(|o| o.floats.iter().map(move |id| (*id, o.hold())))
            .collect();
    }

    /// The nearest open water to an orb at `center` of `radius`, within
    /// [`DRAW_REACH`] of its bottom, and which body it is.
    fn water_near(&self, center: Vec3, radius: f32) -> (Option<Vec3>, Source) {
        let at = Vec2::new(center.x, center.z);
        let body = |p: Vec2| {
            if terrain::fresh_water(p).is_some() {
                Source::River
            } else {
                Source::Sea
            }
        };
        let mut best: Option<(f32, Vec3, Source)> = None;
        let mut look = |p: Vec2| {
            if let Some(s) = self.surface_at(p) {
                let q = Vec3::new(p.x, s.height, p.y);
                let d = q.distance(center - Vec3::Y * radius);
                if d < DRAW_REACH && best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, q, body(p)));
                }
            }
        };
        look(at);
        for ring in [3.0, 6.0, 10.0, 14.0] {
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                look(at + Vec2::new(a.cos(), a.sin()) * ring);
            }
        }
        match best {
            Some((_, q, source)) => (Some(q), source),
            None => (None, Source::Air),
        }
    }

    /// The forming orb's streams stir the water where they rise; condensing
    /// air mists around it.
    fn feed_fx(&mut self, dt: f32) {
        self.feed_wait -= dt;
        if self.feed_wait > 0.0 {
            return;
        }
        self.feed_wait = 0.14;
        let Some(orb) = self.forming().cloned() else {
            return;
        };
        if orb.feed < 0.3 {
            return;
        }
        match orb.draw {
            Some(q) => {
                let jitter = Vec2::new(self.random() - 0.5, self.random() - 0.5) * 1.6;
                let p = Vec2::new(q.x, q.z) + jitter;
                self.water.add_ripple(p, 0.012 + 0.003 * orb.radius);
                self.fx.start(
                    "water_wade",
                    Spawn::at(Vec3::new(p.x, q.y, p.y))
                        .scaled(0.7 + 0.1 * orb.radius)
                        .moving(Vec3::Y * 3.0),
                );
            }
            None => {
                let a = self.random() * std::f32::consts::TAU;
                let out = Vec3::new(a.cos(), self.random() - 0.5, a.sin()) * orb.radius * 1.6;
                self.fx.start(
                    "water_mist",
                    Spawn::at(orb.center + out).scaled(0.3 + 0.08 * orb.radius),
                );
            }
        }
    }

    /// The orbs' water, droplets, and streams, and the water burst orbs
    /// threw up, as water-pass vertices.
    pub(crate) fn orb_liquid(&self, out: &mut Vec<WaterVertex>) {
        let t = self.time;
        for spill in &self.spills {
            spill.draw(out, t, &|x, z| self.top_at(x, z));
        }
        for orb in &self.orbs {
            let detail = if orb.radius > 2.0 { (40, 28) } else { (28, 20) };
            sphere(out, orb.center, orb.radius, orb.wobble, orb.charge, detail);
            droplets(out, orb, t);
            if let Some(q) = orb.draw
                && orb.feed > 0.02
            {
                streams(out, orb, q, t);
            }
        }
    }

    /// The forming orb's size ring and diameter, the throw's landing ring,
    /// and each struck orb's arcs and glow, seen from `eye`.
    pub(crate) fn orb_marks(&self, mesh: &mut Mesh, eye: Vec3) {
        let t = self.time;
        for orb in &self.orbs {
            if orb.state == State::Forming {
                // The size ring at its waist, and the faint largest ring.
                let k = orb.radius / MAX_RADIUS;
                let bright = [0.35 + 0.6 * k, 0.8 + 0.2 * k, 1.2];
                circle(
                    &mut mesh.lines,
                    orb.center,
                    orb.radius * 1.04,
                    64,
                    bright,
                    0.0,
                );
                if orb.radius < MAX_RADIUS - 0.05 {
                    dashed(
                        &mut mesh.lines,
                        orb.center,
                        MAX_RADIUS,
                        72,
                        [0.25, 0.45, 0.65],
                        t * 0.2,
                    );
                }
                let label = format!("{:.1} m", orb.radius * 2.0);
                verse_zone_everglade::zones::everglade::floaters::text(
                    mesh,
                    eye,
                    orb.center + Vec3::Y * (orb.radius + 0.5),
                    &label,
                    0.32 + 0.04 * orb.radius,
                    [0.7, 0.9, 1.0],
                );
                // Where it will land if thrown now.
                let (feet, forward) = self.caster;
                let aim = self.aim_point(feet, forward, THROW_REACH, Some(orb.id));
                let reach = splash_radius(orb.radius) * 0.5;
                let level = |x: f32, z: f32| self.top_at(x, z) + 0.08;
                let segments = 48;
                let mut last = None;
                for s in 0..=segments {
                    let a = s as f32 / segments as f32 * std::f32::consts::TAU + t * 0.5;
                    let x = aim.x + a.cos() * reach;
                    let z = aim.z + a.sin() * reach;
                    let p = Vec3::new(x, level(x, z), z);
                    if let Some(prev) = last
                        && s % 4 != 0
                    {
                        line(&mut mesh.lines, prev, p, [0.45, 0.85, 1.1]);
                    }
                    last = Some(p);
                }
            }
            if orb.charge > 0.0 {
                arcs(mesh, orb, self, t, eye);
            }
        }
    }
}

/// How long a burst's blobs fly, s, and the most bursts drawn at once.
pub const SPILL_TIME: f32 = 1.8;
const MAX_SPILLS: usize = 4;

/// The water a burst orb throws up: blobs on ballistic arcs, out and up
/// from where it burst, as far and as high as the orb was big, and a jet
/// up the middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spill {
    pub at: Vec3,
    pub radius: f32,
    /// The orb's velocity as it burst, which the water keeps a little of.
    pub velocity: Vec3,
    pub start: f32,
    seed: u32,
}

impl Spill {
    fn draw(&self, out: &mut Vec<WaterVertex>, now: f32, top: &dyn Fn(f32, f32) -> f32) {
        let age = now - self.start;
        if !(0.0..SPILL_TIME).contains(&age) {
            return;
        }
        let r = self.radius;
        let count = (20.0 + 6.0 * r).round() as usize;
        let carry = Vec3::new(self.velocity.x, 0.0, self.velocity.z) * 0.15;
        let unit = |k: usize, salt: u32| {
            let mut h =
                self.seed ^ (k as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
            h ^= h >> 15;
            h = h.wrapping_mul(0x2C1B_3C6D);
            h ^= h >> 12;
            (h >> 8) as f32 / 16_777_216.0
        };
        let fade = 1.0 - age / SPILL_TIME;
        for k in 0..count {
            let jet = k < 5 && r > 1.2;
            let a = k as f32 / count as f32 * std::f32::consts::TAU + unit(k, 1) * 0.4;
            let out_dir = Vec3::new(a.cos(), 0.0, a.sin());
            let (out_speed, up_speed) = if jet {
                (
                    0.4 + 0.6 * unit(k, 2),
                    (4.0 + 1.5 * r) * (0.75 + 0.35 * unit(k, 3)),
                )
            } else {
                (
                    (2.0 + 1.25 * r) * (0.45 + 0.55 * unit(k, 2)),
                    (2.5 + 1.2 * r) * (0.5 + 0.6 * unit(k, 3)),
                )
            };
            let from = self.at + out_dir * r * 0.45 * unit(k, 4);
            let v = out_dir * out_speed + Vec3::Y * up_speed + carry;
            let p = from + v * age - Vec3::Y * (0.5 * 9.81 * age * age);
            // Gone once it falls back below the surface it rose from.
            if age > 0.2 && p.y < top(p.x, p.z) - 0.1 {
                continue;
            }
            let size = (0.1 + 0.07 * r) * (0.5 + unit(k, 5)) * (0.35 + 0.65 * fade);
            let stretch = v - Vec3::Y * 9.81 * age;
            let wobble = (0.5 + 0.5 * (stretch.length() / 12.0).min(1.0)).min(0.99);
            sphere(out, p, size, wobble, 0.0, (8, 6));
        }
    }
}

/// A sphere of water at `center`, `detail` segments around and rings down.
fn sphere(
    out: &mut Vec<WaterVertex>,
    center: Vec3,
    radius: f32,
    wobble: f32,
    charge: f32,
    detail: (usize, usize),
) {
    let (around, down) = detail;
    let point = |j: usize, i: usize| {
        let theta = std::f32::consts::PI * j as f32 / down as f32;
        let phi = std::f32::consts::TAU * i as f32 / around as f32;
        Vec3::new(
            theta.sin() * phi.cos(),
            theta.cos(),
            theta.sin() * phi.sin(),
        ) * radius
    };
    for j in 0..down {
        for i in 0..around {
            let quad = [
                point(j, i),
                point(j, i + 1),
                point(j + 1, i + 1),
                point(j + 1, i),
            ];
            for k in [0, 1, 2, 0, 2, 3] {
                out.push(WaterVertex::orb(center, quad[k], wobble, charge));
            }
        }
    }
}

/// Droplets circling an orb, and, while it condenses from the air, more
/// spiraling in to it.
fn droplets(out: &mut Vec<WaterVertex>, orb: &Orb, t: f32) {
    let r = orb.radius;
    let count = (8.0 + 2.5 * r).round() as usize;
    for k in 0..count {
        let s = k as f32 / count as f32;
        let seed = (orb.seed as f32 * 0.001 + k as f32 * 7.31).sin() * 0.5 + 0.5;
        let speed = 0.9 + 0.8 * seed;
        let a = s * std::f32::consts::TAU + t * speed / (0.5 + 0.15 * r);
        let tilt = (seed - 0.5) * 1.2 + (t * 0.7 + k as f32).sin() * 0.15;
        let ring = r * (1.18 + 0.12 * (t * 1.7 + k as f32 * 2.1).sin());
        let at = orb.center + Vec3::new(a.cos() * ring, tilt * r * 0.6, a.sin() * ring);
        let size = (0.05 + 0.025 * r) * (0.7 + 0.6 * seed);
        sphere(out, at, size, 0.4, orb.charge, (8, 6));
    }
    if orb.state == State::Forming && orb.source == Source::Air && orb.feed > 0.05 {
        for k in 0..14 {
            let s = k as f32 / 14.0;
            let life = (t * 0.6 + s * 3.7).fract();
            let a = s * 19.0 + life * 3.0;
            let reach = r * (1.1 + 2.2 * (1.0 - life));
            let y = ((k as f32 * 1.9).sin()) * r * 0.8 * (1.0 - life);
            let at = orb.center + Vec3::new(a.cos() * reach, y, a.sin() * reach);
            sphere(out, at, 0.06 + 0.02 * r * life, 0.3, 0.0, (6, 4));
        }
    }
}

/// The streams a forming orb draws up from the water at `q`: three pulsing
/// columns rising out of the water, arching over, and pouring into the
/// orb's side toward the water.
fn streams(out: &mut Vec<WaterVertex>, orb: &Orb, q: Vec3, t: f32) {
    let r = orb.radius;
    let thick = (0.2 + 0.06 * r) * orb.feed;
    let toward = Vec3::new(q.x - orb.center.x, 0.0, q.z - orb.center.z).normalize_or(Vec3::Z);
    let side = Vec3::new(-toward.z, 0.0, toward.x);
    for k in 0..3 {
        let spread = k as f32 - 1.0;
        let foot = q + side * spread * 1.4 - Vec3::Y * 0.3;
        let entry = orb.center + (toward * 0.7 + side * spread * 0.35) * r + Vec3::Y * r * 0.55;
        let high = entry.y.max(foot.y) + 1.5 + 0.4 * r;
        let c1 = foot + Vec3::Y * (high - foot.y) * 1.1;
        let c2 = entry + toward * r * 0.9 + Vec3::Y * (high - entry.y + r * 0.5);
        let steps = 22;
        let mut points = Vec::with_capacity(steps + 1);
        let mut radii = Vec::with_capacity(steps + 1);
        for s in 0..=steps {
            let u = s as f32 / steps as f32;
            let v = 1.0 - u;
            let mut p =
                foot * v * v * v + c1 * 3.0 * v * v * u + c2 * 3.0 * v * u * u + entry * u * u * u;
            let sway = (t * 2.3 + u * 6.0 + k as f32 * 2.0).sin() * 0.45 * u * v;
            p += side * sway;
            points.push(p);
            // Pulses of water run up it, thickest where it leaves the water.
            let pulse = 0.8 + 0.25 * (u * 14.0 - t * 9.0 + k as f32).sin();
            radii.push(thick * pulse * (1.0 + 0.8 * v * v));
        }
        tube(out, &points, &radii, 10, 0.3);
    }
}

/// A tube of water along `points` with `radii`, `sides` around.
fn tube(out: &mut Vec<WaterVertex>, points: &[Vec3], radii: &[f32], sides: usize, wobble: f32) {
    if points.len() < 2 {
        return;
    }
    let mut frames = Vec::with_capacity(points.len());
    let first = (points[1] - points[0]).normalize_or(Vec3::Y);
    let mut u = first.any_orthonormal_vector();
    for k in 0..points.len() {
        let tangent = (points[(k + 1).min(points.len() - 1)] - points[k.saturating_sub(1)])
            .normalize_or(first);
        u = (u - tangent * u.dot(tangent)).normalize_or(tangent.any_orthonormal_vector());
        frames.push((u, tangent.cross(u)));
    }
    let ring = |k: usize, i: usize| {
        let a = std::f32::consts::TAU * i as f32 / sides as f32;
        let (u, w) = frames[k];
        (u * a.cos() + w * a.sin()) * radii[k].max(0.005)
    };
    for k in 0..points.len() - 1 {
        for i in 0..sides {
            let quad = [
                (k, ring(k, i)),
                (k, ring(k, i + 1)),
                (k + 1, ring(k + 1, i + 1)),
                (k + 1, ring(k + 1, i)),
            ];
            for n in [0, 1, 2, 0, 2, 3] {
                let (at, offset) = quad[n];
                out.push(WaterVertex::orb(points[at], offset, wobble, 0.0));
            }
        }
    }
}

/// Lightning's arcs crawling over a struck orb's skin, through its water,
/// and to everything inside it, redrawn many times a second, with a glow
/// at its heart.
fn arcs(mesh: &mut Mesh, orb: &Orb, lab: &WaterLab, t: f32, eye: Vec3) {
    let c = orb.charge;
    let flick = (t * 30.0) as u32;
    let mut seed = orb.seed ^ flick.wrapping_mul(0x85EB_CA6B);
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / 16_777_216.0
    };
    let dir = |unit: &mut dyn FnMut() -> f32| {
        let z = unit() * 2.0 - 1.0;
        let a = unit() * std::f32::consts::TAU;
        let s = (1.0 - z * z).sqrt();
        Vec3::new(s * a.cos(), z, s * a.sin())
    };
    let white = [1.6, 1.75, 2.0];
    let blue = [0.6, 0.85, 1.6];
    let r = orb.radius;
    // Over the skin.
    let skin = ((6.0 + 3.0 * r) * c).round() as usize;
    for _ in 0..skin {
        let a = dir(&mut unit);
        let mut b = (a + dir(&mut unit) * 0.9).normalize_or(a);
        if b.dot(a) > 0.995 {
            b = (a + Vec3::X * 0.3).normalize();
        }
        let steps = 8;
        let mut last = orb.center + a * r * 1.01;
        for s in 1..=steps {
            let u = s as f32 / steps as f32;
            let d = a.lerp(b, u).normalize_or(a);
            let jag = (dir(&mut unit)) * r * 0.06;
            let p = orb.center
                + (d * r * 1.01 + jag * (u * (1.0 - u) * 4.0)).normalize_or(d) * r * 1.01;
            line(
                &mut mesh.lines,
                last,
                p,
                if s % 3 == 0 { blue } else { white },
            );
            last = p;
        }
    }
    // Through the water, from skin to skin.
    let through = ((2.0 + r) * c).round() as usize;
    for _ in 0..through {
        let a = orb.center + dir(&mut unit) * r;
        let b = orb.center + dir(&mut unit) * r;
        jagged(&mut mesh.lines, a, b, r * 0.12, &mut unit, white);
    }
    // To everything inside.
    let mut inside: Vec<Vec3> = orb
        .floats
        .iter()
        .filter_map(|id| lab.floats.world.bodies().get(id.0 as usize))
        .map(|b| b.pos.as_vec3())
        .collect();
    inside.extend(
        orb.dummies
            .iter()
            .filter_map(|k| lab.targets.get(*k))
            .map(|t| t.dummy.center()),
    );
    for p in inside {
        for _ in 0..2 {
            let from = orb.center + dir(&mut unit) * r;
            jagged(&mut mesh.lines, from, p, 0.25, &mut unit, white);
        }
        glow_blob(
            &mut mesh.glow,
            p,
            0.6 + 0.4 * c,
            [0.55, 0.75, 1.0],
            25.0 * c,
            eye,
        );
    }
    // The heart's glow, and a flash just after the strike.
    glow_blob(
        &mut mesh.glow,
        orb.center,
        r * 0.8,
        [0.45, 0.65, 1.0],
        1.5 * c,
        eye,
    );
    let flash = ((c - 0.8) / 0.2).clamp(0.0, 1.0);
    if flash > 0.0 {
        glow_blob(
            &mut mesh.glow,
            orb.center,
            r * 1.1,
            [0.7, 0.85, 1.0],
            4.0 * flash,
            eye,
        );
    }
}

/// A jagged line from `a` to `b`, bent up to `wide` m off straight.
fn jagged(
    out: &mut Vec<Vertex>,
    a: Vec3,
    b: Vec3,
    wide: f32,
    unit: &mut dyn FnMut() -> f32,
    color: [f32; 3],
) {
    let steps = 7;
    let along = (b - a).normalize_or(Vec3::Y);
    let side = along.any_orthonormal_vector();
    let other = along.cross(side);
    let mut last = a;
    for s in 1..=steps {
        let u = s as f32 / steps as f32;
        let taper = (u * (1.0 - u) * 4.0).sqrt();
        let x = unit() * 2.0 - 1.0;
        let y = unit() * 2.0 - 1.0;
        let p = a.lerp(b, u) + (side * x + other * y) * wide * taper;
        line(out, last, p, color);
        last = p;
    }
}

/// One colored segment.
pub(crate) fn line(out: &mut Vec<Vertex>, a: Vec3, b: Vec3, color: [f32; 3]) {
    for p in [a, b] {
        out.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

/// A level circle of `radius` around `center`.
fn circle(
    out: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    segments: usize,
    color: [f32; 3],
    turn: f32,
) {
    let at = |s: usize| {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU + turn;
        center + Vec3::new(a.cos(), 0.0, a.sin()) * radius
    };
    for s in 0..segments {
        line(out, at(s), at(s + 1), color);
    }
}

/// [`circle`], every other segment drawn.
fn dashed(
    out: &mut Vec<Vertex>,
    center: Vec3,
    radius: f32,
    segments: usize,
    color: [f32; 3],
    turn: f32,
) {
    let at = |s: usize| {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU + turn;
        center + Vec3::new(a.cos(), 0.0, a.sin()) * radius
    };
    for s in (0..segments).step_by(2) {
        line(out, at(s), at(s + 1), color);
    }
}

/// A round glow of half size `half` at `at`, facing `eye`, of `color` at
/// `luminance`.
pub(crate) fn glow_blob(
    out: &mut Vec<GlowVertex>,
    at: Vec3,
    half: f32,
    color: [f32; 3],
    luminance: f32,
    eye: Vec3,
) {
    // A glow brushing the camera would fill the view; it fades out instead.
    let near = ((eye.distance(at) - half * 0.5 - 0.5) / 2.0).clamp(0.0, 1.0);
    if near <= 0.0 || luminance <= 0.0 {
        return;
    }
    let toward = (eye - at).normalize_or(Vec3::Z);
    let right = Vec3::Y.cross(toward).normalize_or(Vec3::X);
    let up = toward.cross(right);
    let radiance = color.map(|c| c * luminance * near);
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        out.push(GlowVertex {
            pos: (at + right * half * x + up * half * y).to_array(),
            radiance,
            uv: [x, y],
        });
    }
}
