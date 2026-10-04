//! Meteor Swarm presentation: each falling meteor as a blazing orb (a dark
//! core inside a fire shell with a hot center) trailing flame and smoke,
//! each detonation as a fireball that swells to the 40-foot Sphere with a
//! flash, a rising fire column, and smoke, scorch marks where they struck,
//! and the light all of it casts. The instance counts come from
//! `verse_world::spells::meteor_swarm`, whose budget test counts them.
use super::Instance;
use super::lighting::{Light, Lighting, MAX_LIGHTS};
use glam::{Mat4, Vec3};
use verse_world::meteor_swarm::{METEOR_RADIUS, RADIUS};
use verse_world::play::Game;
use verse_world::spells::meteor_swarm::{
    BLAST_COLUMN_INSTANCES, BLAST_GROW, BLAST_SHOW, BLAST_SMOKE_INSTANCES, MAX_SCORCHES,
    METEOR_SMOKE_INSTANCES, METEOR_TRAIL_INSTANCES, METEOR_TRAIL_SPACING,
};

fn particle(model: &str, center: Vec3, radius: f32, opacity: f32, time: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(radius)),
        animation: 0.into(),
        time,
        emission: Vec3::splat(opacity.clamp(0.0, 1.0)),
    }
}

/// A flat decal of `radius` meters on the ground at `center`.
fn decal(model: &str, center: Vec3, radius: f32, opacity: f32, time: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(center)
            * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
            * Mat4::from_scale(Vec3::new(radius, radius, 1.0)),
        animation: 0.into(),
        time,
        emission: Vec3::splat(opacity.clamp(0.0, 1.0)),
    }
}

/// Meteors, fireballs, and scorch marks.
pub fn instances(game: &Game) -> Vec<Instance> {
    let state = &game.spells.meteor_swarm;
    let time = game.time;
    let mut out = Vec::new();
    let r = METEOR_RADIUS as f32;
    for (pos, vel) in state.falling(&game.spells.world) {
        let (p, v) = (pos.as_vec3(), vel.as_vec3());
        let flicker = 1.0 + 0.08 * (time * 41.0 + p.x).sin();
        // The core, the fire shell around it, and the white-hot front.
        out.push(particle("effect-grease", p, r * 1.1, 1.0, time));
        out.push(particle("effect-fire", p, r * 2.6 * flicker, 1.0, time));
        out.push(particle(
            "effect-light",
            p + v.normalize_or_zero() * r * 0.5,
            r * 1.3,
            1.0,
            time,
        ));
        for n in 1..=METEOR_TRAIL_INSTANCES {
            let f = n as f32 / METEOR_TRAIL_INSTANCES as f32;
            out.push(particle(
                "effect-fire",
                p - v * METEOR_TRAIL_SPACING * n as f32,
                r * (2.2 - 1.6 * f),
                1.0 - 0.7 * f,
                time,
            ));
        }
        for n in 1..=METEOR_SMOKE_INSTANCES {
            let back = METEOR_TRAIL_SPACING * (METEOR_TRAIL_INSTANCES + 2 * n) as f32;
            out.push(particle(
                "particle-smoke",
                p - v * back,
                r * (1.6 + n as f32),
                0.55,
                time,
            ));
        }
    }
    let radius = RADIUS as f32;
    for blast in state.showing(time) {
        let age = time - blast.at;
        let c = blast.center.as_vec3();
        let swell = (age / BLAST_GROW).min(1.0);
        let fade = 1.0 - age / BLAST_SHOW;
        // The fireball fills the Sphere, then burns down.
        out.push(particle(
            "effect-impact",
            c + Vec3::Y * radius * 0.3 * swell,
            radius * (0.25 + 0.75 * swell.sqrt()),
            fade.powf(0.7),
            time,
        ));
        // The flash at the moment of impact.
        out.push(particle(
            "effect-light",
            c + Vec3::Y,
            6.0 * (1.0 - age / 0.35).max(0.0),
            1.0,
            time,
        ));
        for n in 0..BLAST_COLUMN_INSTANCES {
            let k = n as f32 / BLAST_COLUMN_INSTANCES as f32;
            out.push(particle(
                "effect-fire",
                c + Vec3::Y * (1.0 + age * (6.0 + 10.0 * k)),
                3.5 - 1.5 * k,
                fade,
                time,
            ));
        }
        for n in 0..BLAST_SMOKE_INSTANCES {
            let angle = n as f32 * 2.1 + blast.at;
            out.push(particle(
                "particle-smoke",
                c + Vec3::new(angle.cos() * 3.0, 1.5 + age * 5.0, angle.sin() * 3.0),
                2.5 + age * 4.0,
                (age / 0.3).min(1.0) * fade * 0.8,
                time,
            ));
        }
    }
    for blast in state.blasts.iter().rev().take(MAX_SCORCHES) {
        let c = blast.center.as_vec3();
        out.push(decal(
            "effect-grease",
            Vec3::new(c.x, c.y + 0.03, c.z),
            3.0,
            0.9,
            time,
        ));
    }
    out
}

/// Fire light from falling meteors and detonations.
pub fn lights(game: &Game, lighting: &mut Lighting) {
    let state = &game.spells.meteor_swarm;
    let time = game.time;
    let mut lights: Vec<Light> = state
        .showing(time)
        .map(|blast| {
            let age = time - blast.at;
            Light {
                position: blast.center.as_vec3() + Vec3::Y * 2.0,
                color: Vec3::new(1.0, 0.45, 0.08),
                intensity: 4_000.0 * (1.0 - age / BLAST_SHOW).powi(2),
                range: 2.5 * RADIUS as f32,
            }
        })
        .collect();
    lights.extend(
        state
            .falling(&game.spells.world)
            .into_iter()
            .map(|(pos, _)| Light {
                position: pos.as_vec3(),
                color: Vec3::new(1.0, 0.55, 0.12),
                intensity: 900.0,
                range: 30.0,
            }),
    );
    // Meteor Swarm's light outshines the small effects it replaces.
    let room = MAX_LIGHTS.saturating_sub(lights.len());
    lighting.lights.truncate(room);
    lighting.lights.extend(lights.into_iter().take(MAX_LIGHTS));
}
