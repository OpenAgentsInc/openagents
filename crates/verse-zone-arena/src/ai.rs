//! The enemy robot's mind: close to fighting range, circle the player, fire
//! a plasma bolt on its cooldown, and wind up a ground slam when the player
//! comes near. A stun stops all of it and breaks a slam in progress. It
//! raises one emergency shield below half health.
//!
//! Every choice is a function of the fight's state, so the same seed and
//! the same player moves replay the same fight.

use glam::Vec3;

use crate::rules::{
    ARENA_RADIUS, BOLT_COOLDOWN, BOLT_RANGE, Fight, SLAM_COOLDOWN, SLAM_REACH, SLAM_WINDUP, Windup,
};
use verse_world::social::controller::{forward, wrap};

/// How the enemy walks, m/s: approaching, circling, and backing off.
pub const CHASE_SPEED: f32 = 4.4;
pub const CIRCLE_SPEED: f32 = 3.2;
/// Turn rate, rad/s.
pub const TURN_RATE: f32 = 3.0;
/// The distance band the enemy circles in, m.
pub const FAR: f32 = 13.0;
pub const NEAR: f32 = 7.0;
/// Within this the enemy walks in to slam when the slam is ready.
pub const CHARGE: f32 = 9.0;
/// How far off its facing the enemy still fires, rad.
pub const FIRE_CONE: f32 = 0.5;

/// One frame of the enemy's thinking and walking.
pub fn think(fight: &mut Fight, dt: f32) {
    let now = fight.now;
    if fight.enemy.stunned(now) || !fight.enemy.alive() {
        fight.enemy.speed = 0.0;
        return;
    }
    if !fight.brain.overdrive && fight.enemy.hp < fight.enemy.max_hp * 0.5 {
        fight.overdrive();
    }
    // A slam in progress stands still and lands when the wind-up ends.
    if let Some(windup) = fight.brain.windup {
        fight.enemy.speed = 0.0;
        face(fight, dt);
        if now - windup.started >= SLAM_WINDUP {
            fight.brain.windup = None;
            fight.land_slam();
        }
        return;
    }
    let distance = fight.distance();
    if distance <= SLAM_REACH - 0.5 && now >= fight.brain.slam_ready {
        fight.brain.windup = Some(Windup { started: now });
        fight.brain.slam_ready = now + SLAM_COOLDOWN;
        fight.enemy.speed = 0.0;
        return;
    }
    // Which way to circle flips every few seconds, chosen by the dice.
    if now >= fight.brain.circle_until {
        let roll = fight.dice_mut().roll(6);
        fight.brain.circle = if roll <= 3 { 1.0 } else { -1.0 };
        fight.brain.circle_until = now + 2.0 + roll as f32 * 0.4;
    }
    let to_player = fight.player.pos - fight.enemy.pos;
    let toward = Vec3::new(to_player.x, 0.0, to_player.z).normalize_or_zero();
    let around = Vec3::Y.cross(toward) * fight.brain.circle;
    let slam_ready = now >= fight.brain.slam_ready;
    let (wish, speed) = if slam_ready && distance < CHARGE {
        (toward, CHASE_SPEED)
    } else if distance > FAR {
        ((toward + around * 0.3).normalize_or_zero(), CHASE_SPEED)
    } else if distance < NEAR {
        ((-toward + around * 0.6).normalize_or_zero(), CIRCLE_SPEED)
    } else {
        (around, CIRCLE_SPEED)
    };
    // Near the ring's edge it turns back toward the middle.
    let flat = Vec3::new(fight.enemy.pos.x, 0.0, fight.enemy.pos.z);
    let wish = if flat.length() > ARENA_RADIUS - 3.0 {
        (wish - flat.normalize_or_zero()).normalize_or_zero()
    } else {
        wish
    };
    let step = wish * speed * dt;
    fight.enemy.pos += step;
    fight.enemy.keep_on_floor();
    fight.enemy.speed = speed;
    fight.enemy.stride += step.length();
    face(fight, dt);
    let facing = forward(fight.enemy.yaw);
    let off = facing.angle_between(toward);
    if now >= fight.brain.bolt_ready && distance <= BOLT_RANGE && off <= FIRE_CONE {
        fight.brain.bolt_ready = now + BOLT_COOLDOWN;
        fight.fire_bolt();
    }
}

/// Turns the enemy toward the player at its turn rate.
fn face(fight: &mut Fight, dt: f32) {
    let d = fight.player.pos - fight.enemy.pos;
    if d.x.hypot(d.z) < 1e-3 {
        return;
    }
    let want = d.x.atan2(d.z);
    let delta = wrap(want - fight.enemy.yaw);
    let most = TURN_RATE * dt;
    fight.enemy.yaw = wrap(fight.enemy.yaw + delta.clamp(-most, most));
}
