//! Skurdo, the Prussian: what he chooses each step.
//!
//! He plays by the same rules and the same commands as the player. He
//! sees a blow only once it starts, and answers it after a human reaction
//! time drawn from his own dice ([`Brain::reaction`]): a raised shield
//! turned toward the side it comes from, a parry timed to its arrival, or a
//! step back out of reach. A feint that changes the blow after he has read
//! it costs him a fresh reaction. He keeps just outside the knight's reach,
//! circles, steps in to strike at whatever the knight leaves open, feints
//! now and then, backs off to get his breath when tired, and presses when
//! the knight is tired.

use glam::Vec2;
use verse_world::spells::dice::Dice;

use crate::rules::{
    Action, BODY_GAP, Command, Dir, Fight, Fighter, PARRY_CLOSE, PARRY_OPEN, Side, Steer, TIRED,
    timing,
};

/// The quickest and slowest he reacts to a blow he sees start, s.
pub const REACTION_MIN: f32 = 0.16;
pub const REACTION_MAX: f32 = 0.42;

/// How he means to meet a blow he has read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    Shield,
    /// Parry when the blow is due: fight time, s.
    Parry {
        at: f32,
    },
    Back,
    /// Strike first, into a slow windup.
    Counter,
}

/// His intentions.
#[derive(Clone, Debug)]
pub struct Brain {
    dice: Dice,
    /// The player's blow he is answering: its direction, when he took it
    /// in, and his answer.
    pub answering: Option<(Dir, f32, Answer)>,
    /// When he may start his next blow, s.
    pub next_attack: f32,
    /// A feint he has planned for his windup: the direction he switches to
    /// and at what part of the windup.
    pub feint: Option<(Dir, f32)>,
    /// Which way he circles (1 right, -1 left) and until when.
    pub circle: (f32, f32),
    /// He holds his shield up until this time.
    pub shield_until: f32,
    /// The last time he decided anything slow.
    pub thought: f32,
}

impl Brain {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            dice: Dice::new(seed),
            answering: None,
            next_attack: 1.8,
            feint: None,
            circle: (1.0, 2.0),
            shield_until: 0.0,
            thought: -1.0,
        }
    }

    /// A uniform number in [0, 1) from his dice.
    fn unit(&mut self) -> f32 {
        (self.dice.roll(10_000) - 1) as f32 / 10_000.0
    }

    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    /// One reaction time, s: the sum of three dice keeps most near the
    /// middle, between [`REACTION_MIN`] and [`REACTION_MAX`].
    pub fn reaction(&mut self) -> f32 {
        let three = self.dice.sum(3, 10) as f32; // 3 to 30
        REACTION_MIN + (three - 3.0) / 27.0 * (REACTION_MAX - REACTION_MIN)
    }
}

/// How far apart the two stand, m.
fn gap(a: &Fighter, b: &Fighter) -> f32 {
    Vec2::new(a.pos.x - b.pos.x, a.pos.z - b.pos.z).length()
}

/// The distance a blow in `dir` reaches from, m, center to center.
#[must_use]
pub fn measure(dir: Dir) -> f32 {
    match dir {
        Dir::Thrust => 1.75,
        Dir::Overhead => 1.6,
        Dir::Left | Dir::Right => 1.6,
    }
}

/// When the player's blow in `dir`, wound up for `dur` from `since`, will
/// arrive: the middle of its release.
fn due(f: &Fighter, dir: Dir) -> f32 {
    let t = timing(dir, f.kit.weapon());
    match f.action {
        Action::Windup { dur, .. } => f.since + dur + t.release * 0.45,
        Action::Release { .. } => f.since + t.release * 0.45,
        _ => f.since,
    }
}

/// Chooses Skurdo's steering and commands for this step.
pub fn think(fight: &mut Fight) {
    let now = fight.now;
    let me = fight.enemy().clone();
    let you = fight.player().clone();
    if !me.alive() || !you.alive() {
        fight.steer(Side::Enemy, Steer::default());
        return;
    }
    let distance = gap(&me, &you);
    let aggressive = you.tired() || you.hp < 35.0;
    let winded = me.stamina < TIRED + 8.0;
    // What he has read of your blow, once his reaction time has passed.
    let threat = you.threat(now);
    match (threat, me.read) {
        (Some((dir, _, _)), Some(read)) if read == dir => {
            let fresh = fight.brain.answering.is_none_or(|(d, ..)| d != dir);
            if fresh {
                let answer = choose_answer(fight, &me, &you, dir, distance, winded);
                fight.brain.answering = Some((dir, now, answer));
            }
        }
        (None, _) => fight.brain.answering = None,
        _ => {}
    }
    let mut shield = now < fight.brain.shield_until;
    let mut ahead;
    let mut side = 0.0;
    if let Some((_, _, answer)) = fight.brain.answering {
        match answer {
            Answer::Shield => {
                shield = true;
                fight.brain.shield_until = now + 0.45;
            }
            Answer::Parry { at } => {
                if now >= at {
                    let _ = fight.command(Side::Enemy, Command::Parry);
                    fight.brain.answering = None;
                }
            }
            Answer::Back => {
                let _ = fight.command(Side::Enemy, Command::Step(Vec2::new(0.0, -1.0)));
                fight.brain.answering = None;
            }
            Answer::Counter => {
                let _ = fight.command(Side::Enemy, Command::Attack(Dir::Thrust));
                fight.brain.answering = None;
            }
        }
    }
    // Spacing: out of reach in guard, closer when pressing or ready to
    // strike, well back when winded.
    let free = matches!(me.action, Action::Guard)
        || matches!(me.action, Action::Recover { dur, .. } if me.elapsed(now) > dur * 0.75);
    let riposte = now < me.riposte_until;
    let enough = me.stamina >= if aggressive { 20.0 } else { 30.0 };
    let ready =
        free && (riposte || (now >= fight.brain.next_attack && enough && (!winded || aggressive)));
    let want = if winded {
        3.6
    } else if ready {
        1.7
    } else if aggressive {
        1.95
    } else {
        2.3
    };
    ahead = ((distance - want) * 1.6).clamp(-1.0, 1.0);
    if now > fight.brain.circle.1 {
        // Circling, he drifts back toward the middle of the field.
        let toward_middle = me.right().dot(-me.pos);
        let dir = if me.pos.length() > 6.0 {
            toward_middle.signum()
        } else if fight.brain.chance(0.5) {
            1.0
        } else {
            -1.0
        };
        let until = now + 1.2 + fight.brain.unit() * 1.8;
        fight.brain.circle = (dir, until);
    }
    if (distance - want).abs() < 0.6 {
        side = fight.brain.circle.0 * 0.55;
    }
    // His own blow: step in through the windup, switch it if he planned a
    // feint.
    if let Action::Windup { dir, dur, feints } = me.action {
        ahead = if distance > measure(dir) - 0.15 {
            1.0
        } else {
            0.0
        };
        if let Some((to, at)) = fight.brain.feint
            && feints == 0
            && me.elapsed(now) / dur >= at
        {
            let _ = fight.command(Side::Enemy, Command::Attack(to));
            fight.brain.feint = None;
        }
    }
    // He strikes once he has closed in; a riposte is taken at once.
    let player_busy = matches!(you.action, Action::Release { .. });
    if ready && !player_busy && fight.brain.answering.is_none() && distance < 2.0 {
        let dir = choose_attack(fight, &me, &you, riposte, distance);
        if dir == Some(Dir::Thrust) && you.guarding() && distance < 1.45 && fight.brain.chance(0.3)
        {
            let _ = fight.command(Side::Enemy, Command::Bash);
        } else if let Some(dir) = dir
            && fight.command(Side::Enemy, Command::Attack(dir)).is_ok()
        {
            let feint = !riposte && fight.brain.chance(if you.guarding() { 0.3 } else { 0.15 });
            fight.brain.feint = feint.then(|| {
                let to = match dir {
                    Dir::Right => Dir::Left,
                    Dir::Left => Dir::Overhead,
                    Dir::Overhead => Dir::Left,
                    Dir::Thrust => Dir::Right,
                };
                (to, 0.4 + fight.brain.unit() * 0.25)
            });
            let pause = if aggressive { 0.55 } else { 1.0 };
            fight.brain.next_attack = now + pause * (0.7 + fight.brain.unit() * 1.1);
        }
    }
    // A shield held between his own blows, to rest it when nothing comes.
    if matches!(me.action, Action::Windup { .. } | Action::Release { .. }) {
        shield = false;
    }
    if winded && fight.brain.answering.is_none() && distance > 2.6 {
        shield = false;
    }
    if distance < BODY_GAP + 0.25 {
        ahead = ahead.min(-0.5);
    }
    fight.steer(
        Side::Enemy,
        Steer {
            ahead,
            side,
            shield,
        },
    );
}

fn choose_answer(
    fight: &mut Fight,
    me: &Fighter,
    you: &Fighter,
    dir: Dir,
    distance: f32,
    winded: bool,
) -> Answer {
    let now = fight.now;
    let left = due(you, dir) - now;
    // Out of reach already: a step back takes him further.
    if distance > measure(dir) + 0.5 {
        return Answer::Shield;
    }
    if winded && me.stamina >= crate::rules::STEP_COST && fight.brain.chance(0.5) {
        return Answer::Back;
    }
    // Into a slow overhead, a quick thrust lands first.
    let quick = timing(Dir::Thrust, me.kit.weapon());
    if dir == Dir::Overhead
        && left > quick.windup + quick.release * 0.5 + 0.05
        && distance < measure(Dir::Thrust)
        && me.stamina > 30.0
        && fight.brain.chance(0.25)
    {
        return Answer::Counter;
    }
    let roll = fight.brain.unit();
    if roll < 0.27 && me.stamina >= crate::rules::PARRY_COST + 5.0 {
        // Aim for the middle of the parry's window, with a human error.
        let error = (fight.brain.unit() - 0.5) * 0.14;
        let at = due(you, dir) - (PARRY_OPEN + PARRY_CLOSE) * 0.5 + error;
        return Answer::Parry { at };
    }
    if roll > 0.93 && me.stamina >= crate::rules::STEP_COST {
        return Answer::Back;
    }
    Answer::Shield
}

fn choose_attack(
    fight: &mut Fight,
    me: &Fighter,
    you: &Fighter,
    riposte: bool,
    distance: f32,
) -> Option<Dir> {
    let open = matches!(
        you.action,
        Action::Recover { .. } | Action::Stagger { .. } | Action::Step { .. }
    );
    if riposte || open {
        return Some(if distance > 1.7 {
            Dir::Thrust
        } else if fight.brain.chance(0.5) {
            Dir::Right
        } else {
            Dir::Left
        });
    }
    let _ = me;
    let r = fight.brain.unit();
    Some(if you.guarding() {
        // Strike where the shield is not.
        if you.cover > 0.35 {
            if r < 0.55 {
                Dir::Left
            } else if r < 0.85 {
                Dir::Overhead
            } else {
                Dir::Thrust
            }
        } else if you.cover < -0.3 {
            if r < 0.6 { Dir::Right } else { Dir::Overhead }
        } else if r < 0.4 {
            Dir::Left
        } else if r < 0.7 {
            Dir::Overhead
        } else {
            Dir::Right
        }
    } else if r < 0.3 {
        Dir::Thrust
    } else if r < 0.55 {
        Dir::Overhead
    } else if r < 0.8 {
        Dir::Right
    } else {
        Dir::Left
    })
}
