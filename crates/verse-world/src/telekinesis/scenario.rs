//! The Telekinesis playground recording.
//!
//! 1. Lift a crate, plow it through a row of eight standing planks, then
//!    let go mid-swing so it flies into a crate stack.
//! 2. Build a four-crate tower, one application per crate.
//! 3. A dummy that makes its save (forced roll) does not move.
//! 4. A dummy that fails (forced roll) is lifted 20 feet, hangs for one
//!    round, then falls and takes 2d6.
//! 5. Lift a 1,000 kg stone block: size, not weight, limits the grip.
//! 6. Push a crate out to the edge of range with the whole 30-foot budget,
//!    then step back: the grip releases beyond 60 feet.
//! 7. The throw again at 0.25×.
//!
//! The wizard acts only through admitted commands: facing, casting, the
//! movement keys (which steer the hand while it has path left, and walk
//! once it has none), and jumping (which lets go).
use super::{NAME, SRD_LINE};
use crate::play::{Ability, Game};
use crate::playground::{Cue, Hall, Scenario, Shot, Step, creature};
use crate::spells::{PropKind, PropSpec};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};

const DUMMY_SAVES: u64 = 102;
const DUMMY_FAILS: u64 = 101;
/// Video frames per second; one movement cue moves the hand this far at
/// full input, m.
const STEP: f32 = super::HAND_SPEED as f32 / crate::playground::FPS as f32;
/// Where things stand, relative to the caster's spawn.
const THROWN: Vec3 = Vec3::new(-2.5, 0.3, 0.);
const PLANK_START: f32 = 4.0;
const PLANK_GAP: f32 = 0.5;
const STACK_REACH: f32 = 11.3;
/// The tower stands 5 m ahead of the caster at yaw π; its crates wait 5 m
/// out at these yaws.
const TOWER_REACH: f32 = 5.;
const TOWER_YAWS: [f32; 4] = [PI + 0.35, PI - 0.35, PI + 0.7, PI - 0.7];
const DUMMY_FAILS_AT: Vec3 = Vec3::new(-2.5, 0., -3.5);
const DUMMY_SAVES_AT: Vec3 = Vec3::new(0., 0., -3.5);
/// The failing dummy rises a little over 20 feet, so the fall from the arc's
/// peak counts two whole tens of feet.
const LIFT: f32 = 6.2;
const STONE: Vec3 = Vec3::new(2.5, 0.375, 0.);
const RANGE_YAW: f32 = 2.24;
const RANGE_REACH: f32 = 9.6;

fn forward(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0., -yaw.cos())
}

fn yaw_to(offset: Vec3) -> f32 {
    (-offset.x).atan2(-offset.z)
}

fn tower_crate(layer: usize) -> Vec3 {
    forward(TOWER_YAWS[layer]) * TOWER_REACH + Vec3::Y * 0.3
}

fn tower_site() -> Vec3 {
    forward(PI) * TOWER_REACH
}

fn range_crate() -> Vec3 {
    forward(RANGE_YAW) * RANGE_REACH + Vec3::Y * 0.3
}

fn stack() -> Vec<(String, Vec3)> {
    let x = -STACK_REACH;
    let mut crates = vec![];
    for (row, count) in [(0, 3), (1, 2), (2, 1)] {
        for i in 0..count {
            let z = (i as f32 - (count - 1) as f32 * 0.5) * 0.61;
            let y = 0.3 + 0.602 * row as f32;
            crates.push((
                format!("Stack crate {}-{}", row + 1, i + 1),
                Vec3::new(x, y, z),
            ));
        }
    }
    crates
}

/// Builds the wizard's commands tick by tick.
struct Script {
    cues: Vec<Cue>,
    tick: u32,
}

impl Script {
    fn at(tick: u32) -> f32 {
        tick as f32 / crate::playground::FPS as f32
    }
    fn push(&mut self, step: Step) {
        self.cues.push(Cue {
            at: Self::at(self.tick),
            step,
        });
        self.tick += 1;
    }
    fn wait(&mut self, ticks: u32) {
        self.tick += ticks;
    }
    fn face(&mut self, yaw: f32) {
        self.push(Step::Face(yaw));
    }
    fn cast(&mut self) {
        self.push(Step::Cast(Ability::Spell(0)));
    }
    /// Holds one movement axis long enough to move the hand `distance`:
    /// axis 0 raises (positive) or lowers it, axis 1 pushes it out or in.
    fn steer(&mut self, axis: usize, distance: f32) {
        let mut left = distance.abs();
        while left > 1e-4 {
            let amount = (left / STEP).min(1.);
            let mut axes = [0.; 2];
            axes[axis] = amount * distance.signum();
            self.push(Step::Move(axes));
            left -= amount * STEP;
        }
    }
    fn hold(&mut self, axes: [f32; 2], ticks: u32) {
        for _ in 0..ticks {
            self.push(Step::Move(axes));
        }
    }
}

/// The whole script; [`LIVE`] is its length.
fn script() -> Vec<Cue> {
    let mut s = Script {
        cues: vec![],
        tick: 3,
    };
    // 1. The throw: lift, plow through the planks, rise, swing, let go.
    s.face(FRAC_PI_2);
    s.wait(2);
    s.cast();
    s.steer(0, 0.6);
    s.hold([0., 1.], 30);
    s.steer(0, 0.6);
    s.hold([0., 1.], 7);
    s.push(Step::Jump);
    // 2. The tower: one application per crate.
    s.tick = 80;
    for (layer, yaw) in TOWER_YAWS.iter().enumerate() {
        s.face(*yaw);
        s.cast();
        s.steer(0, 0.6 * layer as f32 + 0.15);
        s.face(PI);
        let chord = 2. * TOWER_REACH * ((yaw - PI).abs() * 0.5).sin();
        s.wait((chord / STEP).ceil() as u32 + 3);
        s.steer(0, -0.135);
        s.wait(9);
        s.push(Step::Jump);
    }
    // 3. A dummy that saves, then one that fails and is lifted 20 feet.
    s.wait(2);
    s.face(yaw_to(DUMMY_SAVES_AT));
    s.wait(2);
    s.cast();
    // Past the half-second cooldown.
    s.wait(14);
    s.face(yaw_to(DUMMY_FAILS_AT));
    s.wait(2);
    s.cast();
    s.steer(0, LIFT);
    // The hold runs one round from the cast; it then falls, lands, and
    // shows its falling damage before the next shot.
    s.tick += 180 - 31 + 60;
    // 4. The stone block.
    s.face(yaw_to(STONE));
    s.wait(2);
    s.cast();
    s.steer(0, 1.5);
    s.wait(22);
    // 5. Out to the edge of range, then a step back.
    s.face(RANGE_YAW);
    s.wait(2);
    s.cast();
    s.steer(0, 0.7);
    s.hold([0., 1.], 43);
    s.hold([0., -1.], 15);
    s.cues
}

/// Seconds of live action: the script's last cue is at tick 587.
const LIVE: f32 = 20.2;
/// The throw: the release at 1.67 s and the stack's collapse.
const REPLAY: (f32, f32) = (1.5, 2.5);

fn shot(at: f32, eye: Vec3, target: Vec3) -> Shot {
    Shot { at, eye, target }
}

pub fn scenario() -> Scenario {
    Scenario {
        key: "telekinesis",
        title: NAME,
        srd: SRD_LINE,
        seed: 452,
        live: LIVE,
        replay: REPLAY,
        setup: |scene, hall| {
            let caster = hall.spawn("caster")?;
            scene.actors.push(creature(
                DUMMY_FAILS,
                "Dummy A",
                "dummy",
                caster + DUMMY_FAILS_AT,
                0.,
                100,
            ));
            scene.actors.push(creature(
                DUMMY_SAVES,
                "Dummy B",
                "dummy",
                caster + DUMMY_SAVES_AT,
                0.,
                100,
            ));
            Ok(())
        },
        populate,
        script,
        camera: || {
            // World coordinates; the caster stands at (5.75, 0, 0) and the
            // stone wall fills x 10.0 to 10.6, so no eye sits in a solid.
            let throw = (Vec3::new(-0.5, 3.6, -9.), Vec3::new(-0.5, 1.4, 0.));
            let tower = (Vec3::new(1.25, 3.4, -3.), Vec3::new(6., 1.2, 4.2));
            // From the south, between the pillars: Dummy A (x 3.25) sits
            // right of center, and its 8 m peak and the floor both fit.
            let dummies = (Vec3::new(1.5, 4.5, -13.), Vec3::new(5.5, 4.2, -3.5));
            // Close on the stone block (8.25, 0.4, 0) as it rises.
            let stone = (Vec3::new(7.5, 2.6, -4.2), Vec3::new(8.25, 1.5, 0.));
            // Follow the range crate out to 60 ft, where it drops.
            let range_out = (Vec3::new(2., 3.4, 0.5), Vec3::new(-2.5, 1., 7.));
            let range_end = (Vec3::new(-3.5, 3., 5.5), Vec3::new(-7.5, 0.8, 10.5));
            [
                (0., throw),
                (2.6, throw),
                (3.0, tower),
                (7.4, tower),
                (7.8, dummies),
                (16., dummies),
                (16.3, stone),
                (17.3, stone),
                (17.6, range_out),
                (19.1, range_end),
                (LIVE, range_end),
            ]
            .into_iter()
            .map(|(at, (eye, target))| shot(at, eye, target))
            .collect()
        },
        replay_camera: (Vec3::new(-2.5, 2.6, -6.), Vec3::new(-4.5, 1.1, 0.)),
        check,
    }
}

fn populate(game: &mut Game, hall: &Hall) -> Result<(), String> {
    let caster = hall.spawn("caster")?;
    let crate_spec = PropSpec::reference(PropKind::Crate);
    game.spawn_prop("Thrown crate", crate_spec.clone(), caster + THROWN, 0.)?;
    let mut plank = PropSpec::reference(PropKind::Crate);
    plank.dimensions = glam::DVec3::new(0.12, 1.2, 0.8);
    plank.mass = 6.;
    for i in 0..8 {
        let x = -(PLANK_START + PLANK_GAP * i as f32);
        game.spawn_prop(
            &format!("Plank {}", i + 1),
            plank.clone(),
            caster + Vec3::new(x, 0.6, 0.),
            0.,
        )?;
    }
    for (name, at) in stack() {
        game.spawn_prop(&name, crate_spec.clone(), caster + at, 0.)?;
    }
    for layer in 0..4 {
        game.spawn_prop(
            &format!("Tower crate {}", layer + 1),
            crate_spec.clone(),
            caster + tower_crate(layer),
            0.,
        )?;
    }
    game.spawn_prop(
        "Stone block",
        PropSpec::reference(PropKind::StoneBlock),
        caster + STONE,
        0.,
    )?;
    game.spawn_prop("Range crate", crate_spec, caster + range_crate(), 0.)?;
    game.spells.dice.force_save(DUMMY_FAILS, 3)?;
    game.spells.dice.force_save(DUMMY_SAVES, 18)?;
    Ok(())
}

fn check(game: &Game) -> Result<(), String> {
    let caster = crate::playground::hall()?.spawn("caster")?;
    let prop = |name: &str| {
        game.spells
            .props
            .iter()
            .position(|p| p.name == name)
            .ok_or(format!("No prop named {name}"))
    };
    let toppled = (1..=8)
        .filter_map(|i| prop(&format!("Plank {i}")).ok())
        .filter(|i| {
            (game.spells.world[game.spells.props[*i].body].orientation * glam::DVec3::Y).y < 0.7
        })
        .count();
    if toppled < 6 {
        return Err(format!("Only {toppled} of 8 planks fell"));
    }
    let scattered = stack()
        .iter()
        .filter(|(name, at)| {
            prop(name).is_ok_and(|i| {
                game.spells
                    .prop_center(i)
                    .distance((caster + *at).as_dvec3())
                    > 0.15
            })
        })
        .count();
    if scattered < 2 {
        return Err(format!("The throw moved only {scattered} stack crates"));
    }
    let site = (caster + tower_site()).as_dvec3();
    for layer in 0..4 {
        let at = game
            .spells
            .prop_center(prop(&format!("Tower crate {}", layer + 1))?);
        let flat = glam::DVec3::new(at.x - site.x, 0., at.z - site.z).length();
        let height = 0.3 + 0.6 * layer as f64;
        if flat > 0.15 || (at.y - height).abs() > 0.06 {
            return Err(format!("Tower crate {} rests at {at}", layer + 1));
        }
    }
    let logged = |needle: &str| game.spells.log.iter().any(|r| r.text.contains(needle));
    if !game
        .spells
        .log
        .iter()
        .any(|r| r.text.starts_with("Dummy A fell") && r.text.contains("2d6"))
    {
        return Err("Dummy A did not take 2d6 falling damage".into());
    }
    if !logged("Dummy A: hold expired") {
        return Err("Dummy A's hold never expired".into());
    }
    let b = game.actor_position(DUMMY_SAVES).ok_or("Dummy B is gone")?;
    if b.distance(caster + DUMMY_SAVES_AT) > 0.02 {
        return Err(format!("Dummy B moved to {b}"));
    }
    if !logged("Dummy B: STR save 18") {
        return Err("Dummy B never saved".into());
    }
    if !logged("Range crate: released (beyond 60 ft") {
        return Err("The range crate was never released at 60 feet".into());
    }
    if !logged("Stone block: gripped") {
        return Err("The stone block was never gripped".into());
    }
    let error = game.spells.ledger_error();
    if error.linear > crate::spells::LEDGER_TOLERANCE
        || error.angular > crate::spells::LEDGER_TOLERANCE
    {
        return Err(format!("Ledger residual {error:?}"));
    }
    Ok(())
}
