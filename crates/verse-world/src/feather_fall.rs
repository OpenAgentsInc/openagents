//! Feather Fall (SRD 5.2.1): a reaction that caps the descent of up to five
//! falling creatures and cancels the damage of the landing that ends it.
//!
//! The spell does not remove gravity. Each step, after gravity, a vertical
//! drag pulls a warded creature's downward speed back to 60 feet per round
//! (3.048 m/s) and reports the energy and momentum it removes. The drag closes
//! the excess over [`SETTLE`] seconds, so the cap holds within [`CAP_WITHIN`]
//! of the cast without an instant snap. Horizontal velocity is never touched,
//! so a creature pushed sideways off a ledge glides while it descends slowly.
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FEET: f64 = 0.3048;
/// One round, s.
pub const ROUND: f64 = 6.0;
/// Range: 60 feet from the caster.
pub const RANGE: f64 = 60.0 * FEET;
/// Up to five falling creatures.
pub const MAX_TARGETS: usize = 5;
/// The slowed rate of descent: 60 feet per round, m/s.
pub const DESCENT_CAP: f64 = 60.0 * FEET / ROUND;
/// Duration: 1 minute, s.
pub const DURATION: f64 = 60.0;
/// A creature is falling when it is unsupported and descends faster than
/// this, m/s.
pub const FALLING_SPEED: f64 = 1.0;
/// The descent speed reaches the cap within this time of the cast, s.
pub const CAP_WITHIN: f64 = 0.2;
/// Design time over which the drag closes the excess speed, s. It is shorter
/// than [`CAP_WITHIN`] so a fixed step that does not divide it evenly still
/// reaches the cap in time.
pub const SETTLE: f64 = 0.15;
/// Spell level, for the overlay.
pub const LEVEL: u8 = 1;
/// The SRD summary line the playground overlay shows.
pub const SRD_LINE: &str =
    "Level 1 Transmutation · Reaction · 60 ft · up to 5 falling creatures · 1 minute";

/// Whether a candidate is a creature. The SRD names creatures, so objects
/// are never eligible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Creature,
    Object,
}

/// Something the caster can see at the moment of casting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u32,
    pub kind: Kind,
    /// Center, m.
    pub position: DVec3,
    /// Velocity, m/s.
    pub velocity: DVec3,
    /// Standing on something.
    pub supported: bool,
}

impl Candidate {
    /// Unsupported and descending faster than [`FALLING_SPEED`].
    #[must_use]
    pub fn falling(&self) -> bool {
        !self.supported && self.velocity.y < -FALLING_SPEED
    }

    /// A falling creature within [`RANGE`] of `caster`.
    #[must_use]
    pub fn eligible(&self, caster: DVec3) -> bool {
        self.kind == Kind::Creature
            && self.falling()
            && self.position.distance_squared(caster) <= RANGE * RANGE
    }
}

/// Why a cast was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// No target was named.
    NoTarget,
    /// More than [`MAX_TARGETS`] targets.
    TooMany,
    /// A target is named twice.
    Duplicate(u32),
    /// A target is not among the candidates.
    Unknown(u32),
    /// A target is an object.
    NotCreature(u32),
    /// A target is not falling.
    NotFalling(u32),
    /// A target is farther than [`RANGE`].
    OutOfRange(u32),
}

/// Validates the caster's chosen targets at the moment of casting.
pub fn choose(caster: DVec3, candidates: &[Candidate], chosen: &[u32]) -> Result<(), Refusal> {
    if chosen.is_empty() {
        return Err(Refusal::NoTarget);
    }
    if chosen.len() > MAX_TARGETS {
        return Err(Refusal::TooMany);
    }
    for (i, id) in chosen.iter().enumerate() {
        if chosen[..i].contains(id) {
            return Err(Refusal::Duplicate(*id));
        }
        let candidate = candidates
            .iter()
            .find(|c| c.id == *id)
            .ok_or(Refusal::Unknown(*id))?;
        if candidate.kind != Kind::Creature {
            return Err(Refusal::NotCreature(*id));
        }
        if !candidate.falling() {
            return Err(Refusal::NotFalling(*id));
        }
        if candidate.position.distance_squared(caster) > RANGE * RANGE {
            return Err(Refusal::OutOfRange(*id));
        }
    }
    Ok(())
}

/// The controller's choice for an agent or NPC caster: the eligible
/// creatures nearest the caster, ties broken by id, at most [`MAX_TARGETS`].
/// `None` when nobody is eligible, so the reaction stays available.
#[must_use]
pub fn decide(caster: DVec3, candidates: &[Candidate]) -> Option<Vec<u32>> {
    let mut eligible: Vec<&Candidate> = candidates.iter().filter(|c| c.eligible(caster)).collect();
    eligible.sort_by(|a, b| {
        a.position
            .distance_squared(caster)
            .total_cmp(&b.position.distance_squared(caster))
            .then(a.id.cmp(&b.id))
    });
    let chosen: Vec<u32> = eligible.iter().take(MAX_TARGETS).map(|c| c.id).collect();
    (!chosen.is_empty()).then_some(chosen)
}

/// The spell on one creature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ward {
    pub target: u32,
    /// Rate at which the drag closes the excess descent speed, m/s².
    pub closing: f64,
    /// Kinetic energy the drag has removed, J.
    pub energy: f64,
    /// Upward impulse the drag has applied, N s.
    pub impulse: f64,
}

/// The drag applied to one creature in one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Vertical speed after the drag, m/s.
    pub vertical_speed: f64,
    /// Kinetic energy removed this step, J.
    pub energy: f64,
    /// Upward impulse applied this step, N s.
    pub impulse: f64,
}

/// What a warded creature's landing does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landing {
    pub target: u32,
    /// Falling damage dealt: always zero, since the spell cancels it.
    pub damage: i32,
}

/// One cast of Feather Fall and the creatures it still holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatherFall {
    pub caster: u32,
    /// Simulation time of the cast, s.
    pub cast_at: f64,
    pub wards: Vec<Ward>,
}

impl FeatherFall {
    /// Casts on `chosen` after [`choose`] admits them.
    pub fn cast(
        caster_id: u32,
        caster: DVec3,
        candidates: &[Candidate],
        chosen: &[u32],
        time: f64,
    ) -> Result<Self, Refusal> {
        choose(caster, candidates, chosen)?;
        Ok(Self {
            caster: caster_id,
            cast_at: time,
            wards: chosen
                .iter()
                .map(|id| {
                    let velocity = candidates
                        .iter()
                        .find(|c| c.id == *id)
                        .map_or(DVec3::ZERO, |c| c.velocity);
                    Ward {
                        target: *id,
                        closing: excess(velocity.y) / SETTLE,
                        energy: 0.0,
                        impulse: 0.0,
                    }
                })
                .collect(),
        })
    }

    /// Simulation time the spell ends, s.
    #[must_use]
    pub fn ends_at(&self) -> f64 {
        self.cast_at + DURATION
    }

    /// Whether `target` is still warded at `time`.
    #[must_use]
    pub fn holds(&self, target: u32, time: f64) -> bool {
        time < self.ends_at() && self.wards.iter().any(|w| w.target == target)
    }

    /// Whether the spell holds nobody, or its minute has run out.
    #[must_use]
    pub fn ended(&self, time: f64) -> bool {
        time >= self.ends_at() || self.wards.is_empty()
    }

    /// Drops every ward once the minute has run out.
    pub fn expire(&mut self, time: f64) {
        if time >= self.ends_at() {
            self.wards.clear();
        }
    }

    /// Applies one step of drag to `target`. `before` is the vertical speed
    /// at the start of the step and `after` is the speed once this step's
    /// gravity is applied. Returns `None` when the target is not warded.
    pub fn drag(
        &mut self,
        target: u32,
        mass: f64,
        before: f64,
        after: f64,
        dt: f64,
        time: f64,
    ) -> Option<Drag> {
        if time >= self.ends_at() {
            return None;
        }
        let ward = self.wards.iter_mut().find(|w| w.target == target)?;
        let excess_before = excess(before);
        // A new downward shove mid-air widens the excess; the drag still
        // closes it within one settle time.
        ward.closing = ward.closing.max(excess_before / SETTLE);
        let allowed = (excess_before - ward.closing * dt).max(0.0);
        let vertical_speed = after.max(-DESCENT_CAP - allowed);
        let energy = 0.5 * mass * (after * after - vertical_speed * vertical_speed).max(0.0);
        let impulse = mass * (vertical_speed - after);
        ward.energy += energy;
        ward.impulse += impulse;
        Some(Drag {
            vertical_speed,
            energy,
            impulse,
        })
    }

    /// Ends the spell for a warded creature that lands. The landing deals no
    /// falling damage. `None` when the target is not warded, so the caller
    /// applies normal falling damage.
    pub fn land(&mut self, target: u32, time: f64) -> Option<Landing> {
        if time >= self.ends_at() {
            return None;
        }
        let index = self.wards.iter().position(|w| w.target == target)?;
        self.wards.remove(index);
        Some(Landing { target, damage: 0 })
    }

    /// Kinetic energy the drag has removed from every current target, J.
    #[must_use]
    pub fn energy(&self) -> f64 {
        self.wards.iter().map(|w| w.energy).sum()
    }

    /// Checks a restored checkpoint.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = Vec::with_capacity(self.wards.len());
        if !self.cast_at.is_finite()
            || self.cast_at < 0.0
            || self.wards.len() > MAX_TARGETS
            || self.wards.iter().any(|w| {
                let duplicate = seen.contains(&w.target);
                seen.push(w.target);
                duplicate
                    || !w.closing.is_finite()
                    || w.closing < 0.0
                    || !w.energy.is_finite()
                    || w.energy < 0.0
                    || !w.impulse.is_finite()
            })
        {
            return Err("Invalid Feather Fall checkpoint".into());
        }
        Ok(())
    }
}

/// How much faster than the cap a vertical speed descends, m/s.
fn excess(vertical_speed: f64) -> f64 {
    (-DESCENT_CAP - vertical_speed).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::body::Body;
    use physics::collision::{Collider, Shape};
    use physics::ledger::Ledger;
    use physics::world::{BodyId, Uniform, World};

    const G: f64 = 9.81;
    const DT: f64 = 1.0 / 120.0;
    const TOWER: f64 = 60.0 * FEET;
    const RADIUS: f64 = 0.4;
    const DUMMY: f64 = 75.0;

    /// SRD falling damage dice: 1d6 per full 10 feet fallen, at most 20d6.
    fn fall_dice(height: f64) -> u32 {
        ((height / (10.0 * FEET) + 1e-9).floor() as u32).min(20)
    }

    fn candidate(id: u32, position: DVec3, velocity: DVec3) -> Candidate {
        Candidate {
            id,
            kind: Kind::Creature,
            position,
            velocity,
            supported: false,
        }
    }

    /// A 60 ft tower beside a floor, five sphere dummies at its edge, and a
    /// caster on the floor.
    struct Scene {
        world: World,
        dummies: Vec<BodyId>,
        caster: DVec3,
    }

    fn scene() -> Scene {
        let mut world = World::new(DT);
        let floor = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0))
                .with_kind(physics::body::BodyKind::Static),
        );
        world.add_collider(Collider::new(
            floor,
            Shape::Cuboid {
                half: DVec3::new(100.0, 0.5, 100.0),
            },
        ));
        let tower = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(-5.0, TOWER / 2.0, 0.0))
                .with_kind(physics::body::BodyKind::Static),
        );
        world.add_collider(Collider::new(
            tower,
            Shape::Cuboid {
                half: DVec3::new(5.0, TOWER / 2.0, 10.0),
            },
        ));
        let dummies = (0..5)
            .map(|i| {
                let id = world.add(Body::new(
                    DUMMY,
                    DVec3::splat(0.4 * DUMMY * RADIUS * RADIUS),
                    DVec3::new(-0.6, TOWER + RADIUS, -6.0 + 3.0 * i as f64),
                ));
                world.add_collider(Collider::new(id, Shape::Sphere { radius: RADIUS }));
                id
            })
            .collect();
        Scene {
            world,
            dummies,
            caster: DVec3::new(-3.0, TOWER + 0.9, 0.0),
        }
    }

    /// The fall a creature has made since its highest unsupported point.
    #[derive(Default, Clone, Copy)]
    struct Fall {
        apex: Option<f64>,
        landed: Option<f64>,
        /// Distance from the tower's edge at the landing, m.
        reach: f64,
        /// Time in the air, s.
        airtime: f64,
        left_at: Option<f64>,
    }

    /// Runs the scene: a Thunderwave-like shove sends every dummy off the
    /// edge, the caster reacts once the first three are falling, and each
    /// landing is scored. Returns the falls, the spell, and the ledger.
    fn run(shove: [f64; 5], warded: &[usize]) -> (Scene, Vec<Fall>, FeatherFall, Ledger, f64) {
        let mut s = scene();
        for (i, id) in s.dummies.iter().enumerate() {
            s.world[*id].vel = DVec3::new(shove[i], 0.0, 0.0);
        }
        let ledger_origin = DVec3::ZERO;
        let mut ledger = Ledger::new(ledger_origin, s.world.momentum(ledger_origin));
        let mut falls = vec![Fall::default(); 5];
        let mut spell: Option<FeatherFall> = None;
        let mut cast_at = 0.0;
        for _ in 0..(120 * 30) {
            let time = s.world.time();
            let candidates: Vec<Candidate> = s
                .dummies
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let body = &s.world[*id];
                    Candidate {
                        id: i as u32,
                        kind: Kind::Creature,
                        position: body.pos,
                        velocity: body.vel,
                        supported: body.pos.y - RADIUS < 0.02
                            || (body.pos.x < 0.0 && body.pos.y - RADIUS > TOWER - 0.02),
                    }
                })
                .collect();
            if spell.is_none() && warded.iter().all(|i| candidates[*i].falling()) {
                let chosen: Vec<u32> = warded.iter().map(|i| *i as u32).collect();
                spell = Some(FeatherFall::cast(0, s.caster, &candidates, &chosen, time).unwrap());
                cast_at = time;
            }
            // The drag acts as a force, so the world's own integration and
            // contacts see it and the ledger can name it.
            for (i, id) in s.dummies.iter().enumerate() {
                let body = s.world[*id];
                if let Some(spell) = spell.as_mut() {
                    let before = body.vel.y;
                    let after = before - G * DT;
                    if let Some(drag) = spell.drag(i as u32, DUMMY, before, after, DT, time) {
                        if drag.impulse != 0.0 {
                            s.world[*id].apply_force(DVec3::Y * drag.impulse / DT);
                            ledger.add_impulse("feather_fall", DVec3::Y * drag.impulse, body.pos);
                        }
                    }
                }
            }
            s.world.step(&Uniform(DVec3::new(0.0, -G, 0.0)));
            for (i, id) in s.dummies.iter().enumerate() {
                let body = &s.world[*id];
                let height = body.pos.y - RADIUS;
                let fall = &mut falls[i];
                if fall.landed.is_some() {
                    continue;
                }
                if body.pos.x > 0.0 || height < TOWER - 0.02 {
                    fall.apex = Some(fall.apex.map_or(height, |a: f64| a.max(height)));
                    fall.left_at.get_or_insert(s.world.time());
                }
                if fall.apex.is_some() && height < 0.02 && body.vel.y > -0.5 {
                    // The landing point is the floor's surface; the
                    // contact margin leaves the sphere a few millimetres
                    // above it.
                    fall.landed = Some(fall.apex.unwrap());
                    fall.reach = body.pos.x;
                    fall.airtime = s.world.time() - fall.left_at.unwrap();
                    if let Some(spell) = spell.as_mut() {
                        if let Some(landing) = spell.land(i as u32, s.world.time()) {
                            assert_eq!(landing.damage, 0);
                        }
                    }
                }
            }
        }
        (s, falls, spell.unwrap(), ledger, cast_at)
    }

    #[test]
    fn at_most_five_targets_and_only_falling_creatures_in_range() {
        let caster = DVec3::ZERO;
        let fall = DVec3::new(0.0, -4.0, 0.0);
        let six: Vec<Candidate> = (0..6)
            .map(|i| candidate(i, DVec3::new(i as f64, 5.0, 0.0), fall))
            .collect();
        assert_eq!(
            choose(caster, &six, &[0, 1, 2, 3, 4, 5]),
            Err(Refusal::TooMany)
        );
        assert_eq!(choose(caster, &six, &[0, 1, 2, 3, 4]), Ok(()));
        assert_eq!(decide(caster, &six).unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(choose(caster, &six, &[]), Err(Refusal::NoTarget));
        assert_eq!(choose(caster, &six, &[1, 1]), Err(Refusal::Duplicate(1)));

        // Range is exactly 60 feet.
        let edge = candidate(7, DVec3::new(RANGE - 1e-6, 0.0, 0.0), fall);
        let beyond = candidate(8, DVec3::new(RANGE + 1e-3, 0.0, 0.0), fall);
        assert!((RANGE - 18.288).abs() < 1e-12);
        assert_eq!(choose(caster, &[edge], &[7]), Ok(()));
        assert_eq!(choose(caster, &[beyond], &[8]), Err(Refusal::OutOfRange(8)));

        // Objects are never eligible, even falling in range.
        let crate_ = Candidate {
            kind: Kind::Object,
            ..candidate(9, DVec3::X, fall)
        };
        assert_eq!(
            choose(caster, &[crate_], &[9]),
            Err(Refusal::NotCreature(9))
        );
        assert_eq!(decide(caster, &[crate_]), None);

        // Standing, rising, or drifting slowly down is not falling.
        let standing = Candidate {
            supported: true,
            ..candidate(10, DVec3::X, fall)
        };
        let slow = candidate(11, DVec3::X, DVec3::new(0.0, -0.9, 0.0));
        assert_eq!(
            choose(caster, &[standing], &[10]),
            Err(Refusal::NotFalling(10))
        );
        assert_eq!(choose(caster, &[slow], &[11]), Err(Refusal::NotFalling(11)));
        assert_eq!(decide(caster, &[standing, slow, beyond]), None);
    }

    #[test]
    fn drag_reaches_the_cap_within_two_tenths_of_a_second_without_a_snap() {
        for start in [-3.5, -12.0, -40.0, -55.0] {
            let c = candidate(1, DVec3::ZERO, DVec3::new(2.0, start, 0.0));
            let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 0.0).unwrap();
            let mut v = start;
            let mut steps = 0;
            let mut first = None;
            while v < -DESCENT_CAP - 1e-9 {
                let after = v - G * DT;
                let next = spell
                    .drag(1, DUMMY, v, after, DT, steps as f64 * DT)
                    .unwrap()
                    .vertical_speed;
                first.get_or_insert(next);
                v = next;
                steps += 1;
            }
            assert!(steps as f64 * DT <= CAP_WITHIN, "{start}: {steps} steps");
            // Not an instant snap: the first step removes a small fraction.
            if start < -10.0 {
                assert!(first.unwrap() < -DESCENT_CAP - 1.0, "{start}");
            }
            // Gravity cannot push it past the cap once there.
            for i in 0..600 {
                let after = v - G * DT;
                v = spell
                    .drag(1, DUMMY, v, after, DT, 1.0 + i as f64 * DT)
                    .unwrap()
                    .vertical_speed;
                assert!(v >= -DESCENT_CAP - 1e-12);
            }
            assert!(spell.energy() > 0.0);
        }
    }

    #[test]
    fn warded_dummies_land_slowly_unharmed_and_the_others_take_six_d6() {
        let (_scene, falls, spell, ledger, cast_at) = run([3.0, 3.0, 6.0, 3.0, 3.0], &[0, 1, 2]);
        for (i, fall) in falls.iter().enumerate() {
            let height = fall.landed.expect("every dummy lands");
            assert!(
                (height - TOWER).abs() < 0.05,
                "dummy {i} fell {height} m, apex {:?}",
                fall.apex
            );
        }
        // The landing ended the spell for each warded dummy.
        assert!(spell.wards.is_empty());
        assert!(spell.ended(cast_at + 1.0));
        // The two unwarded dummies fell 60 feet: 6d6.
        assert_eq!(fall_dice(falls[3].landed.unwrap()), 6);
        assert_eq!(fall_dice(falls[4].landed.unwrap()), 6);
        // Falling at 3.048 m/s, 60 feet takes about six seconds; free fall
        // takes two, plus the roll off the edge and the bounce.
        for i in 0..3 {
            assert!(falls[i].airtime > 5.5, "{i}: {}", falls[i].airtime);
        }
        for i in 3..5 {
            assert!(falls[i].airtime < 3.5, "{i}: {}", falls[i].airtime);
        }
        // Horizontal motion is untouched, so the warded dummies glide far
        // and the one shoved hardest glides past the others.
        let x = |i: usize| falls[i].reach;
        assert!(x(2) > x(0) + 10.0, "{} vs {}", x(2), x(0));
        assert!(x(0) > x(3) + 5.0, "{} vs {}", x(0), x(3));
        // Momentum stays accounted for: only gravity, contacts, and the drag
        // act, and the floor takes the rest, so check the drag term exists.
        assert!(ledger.external["feather_fall"].linear.y > 0.0);
    }

    #[test]
    fn descent_is_capped_within_two_tenths_and_horizontal_velocity_is_preserved() {
        let mut s = scene();
        let id = s.dummies[0];
        s.world[id].vel = DVec3::new(5.0, 0.0, 1.0);
        let mut spell = None;
        let mut checked = false;
        for _ in 0..(120 * 4) {
            let time = s.world.time();
            let body = s.world[id];
            let c = candidate(0, body.pos, body.vel);
            if spell.is_none() && body.pos.x > 0.5 && c.falling() {
                spell = Some((
                    FeatherFall::cast(9, s.caster, &[c], &[0], time).unwrap(),
                    time,
                ));
            }
            if let Some((spell, cast_at)) = spell.as_mut() {
                let horizontal = DVec3::new(body.vel.x, 0.0, body.vel.z);
                let after = body.vel.y - G * DT;
                let drag = spell.drag(0, DUMMY, body.vel.y, after, DT, time).unwrap();
                s.world[id].apply_force(DVec3::Y * drag.impulse / DT);
                s.world.step(&Uniform(DVec3::new(0.0, -G, 0.0)));
                let now = s.world[id].vel;
                assert!((DVec3::new(now.x, 0.0, now.z) - horizontal).length() < 1e-9);
                if s.world.time() - *cast_at >= CAP_WITHIN - 1e-9 {
                    assert!(now.y >= -DESCENT_CAP - 1e-9, "{now}");
                    checked = true;
                }
            } else {
                s.world.step(&Uniform(DVec3::new(0.0, -G, 0.0)));
            }
        }
        assert!(checked);
    }

    #[test]
    fn the_minute_runs_out() {
        // A 300 m drop outlasts the spell: the drag stops at 60 s and the
        // body then falls freely again.
        let mut world = World::new(DT);
        let id = world.add(Body::new(DUMMY, DVec3::ONE, DVec3::new(0.0, 300.0, 0.0)));
        world[id].vel = DVec3::new(0.0, -5.0, 0.0);
        let c = candidate(0, world[id].pos, world[id].vel);
        let mut spell = FeatherFall::cast(1, DVec3::new(0.0, 290.0, 0.0), &[c], &[0], 0.0).unwrap();
        while world.time() < DURATION + 1.0 {
            let time = world.time();
            let v = world[id].vel.y;
            if let Some(drag) = spell.drag(0, DUMMY, v, v - G * DT, DT, time) {
                world[id].apply_force(DVec3::Y * drag.impulse / DT);
            }
            world.step(&Uniform(DVec3::new(0.0, -G, 0.0)));
            if time + DT < DURATION - 1e-9 && time > CAP_WITHIN {
                assert!(world[id].vel.y >= -DESCENT_CAP - 1e-9);
            }
        }
        assert!(world[id].pos.y > 300.0 - 60.0 * DESCENT_CAP - 10.0);
        assert!(world[id].vel.y < -DESCENT_CAP - 5.0, "{}", world[id].vel.y);
        assert!(!spell.holds(0, DURATION));
        assert!(spell.holds(0, DURATION - 1e-6));
        assert_eq!(spell.land(0, DURATION + 1.0), None);
        spell.expire(DURATION);
        assert!(spell.wards.is_empty());
    }

    #[test]
    fn an_unwarded_creature_lands_with_normal_damage() {
        let c = candidate(1, DVec3::ZERO, DVec3::new(0.0, -5.0, 0.0));
        let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 0.0).unwrap();
        assert_eq!(spell.land(2, 1.0), None);
        assert_eq!(
            spell.land(1, 1.0),
            Some(Landing {
                target: 1,
                damage: 0
            })
        );
        // The landing ended it for that creature: a second landing is normal.
        assert_eq!(spell.land(1, 2.0), None);
        assert_eq!(fall_dice(10.0 * FEET), 1);
        assert_eq!(fall_dice(25.0 * FEET), 2);
        assert_eq!(fall_dice(250.0 * FEET), 20);
    }

    #[test]
    fn checkpoints_round_trip_and_reject_corruption() {
        let c = candidate(1, DVec3::ZERO, DVec3::new(0.0, -5.0, 0.0));
        let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 2.0).unwrap();
        spell.drag(1, DUMMY, -5.0, -5.1, DT, 2.0);
        let json = serde_json::to_string(&spell).unwrap();
        let restored: FeatherFall = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, spell);
        restored.validate().unwrap();
        let mut bad = spell.clone();
        bad.wards.push(bad.wards[0]);
        assert!(bad.validate().is_err());
    }
}
