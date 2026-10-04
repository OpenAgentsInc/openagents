//! Bounded presentation interpolation over admitted authoritative snapshots.
use super::{
    presentation::Presentation,
    wire::{Control, Life, Reply, Response, State, VERSION},
};
use std::collections::BTreeMap;

struct Frame {
    tick: u64,
    control: Option<Control>,
    state: State,
}

/// Retains two snapshots and bounded actor-generation history; owns no rules.
pub struct Buffer {
    instance: u64,
    max_displacement: f32,
    previous: Option<Frame>,
    current: Option<Frame>,
    generations: BTreeMap<u64, u64>,
}
impl Buffer {
    pub fn new(instance: u64, max_displacement: f32) -> Result<Self, String> {
        if !max_displacement.is_finite() || !(0.01..=1000.).contains(&max_displacement) {
            return Err("Invalid replica displacement bound".into());
        }
        Ok(Self {
            instance,
            max_displacement,
            previous: None,
            current: None,
            generations: BTreeMap::new(),
        })
    }
    /// Replaces state only after validating every context, life, and budget.
    pub fn push(&mut self, response: &Response) -> Result<(), String> {
        if response.version != VERSION
            || response.instance != self.instance
            || response.request_id == 0
        {
            return Err("Replica snapshot context mismatch".into());
        }
        let Reply::Snapshot { state } = &response.body else {
            return Err("Replica requires an authoritative snapshot".into());
        };
        state.validate_control(self.instance, &response.control)?;
        if response.control.as_ref().is_some_and(|c| {
            c.life.instance != self.instance
                || !state
                    .presentation
                    .actors
                    .iter()
                    .any(|p| p.life == c.life && p.actor.model == "adventurer")
        }) {
            return Err("Replica control life is missing".into());
        }
        let mut generations = self.generations.clone();
        for actor in &state.actors {
            if generations
                .get(&actor.life.actor)
                .is_some_and(|g| actor.life.generation < *g)
            {
                return Err("Replica actor generation regressed".into());
            }
            generations.insert(actor.life.actor, actor.life.generation);
        }
        if generations.len() > 512 {
            return Err("Replica generation history budget exceeded".into());
        }
        let mut reset = false;
        if let Some(old) = &self.current {
            if response.tick < old.tick {
                return Err("Replica authority tick regressed".into());
            }
            if let (Some(before), Some(after)) = (&old.control, &response.control) {
                if before.life.actor == after.life.actor
                    && (after.epoch < before.epoch
                        || (after.epoch == before.epoch
                            && (after.accepted_sequence < before.accepted_sequence
                                || after.life.generation != before.life.generation)))
                {
                    return Err("Replica control fence regressed".into());
                }
            }
            reset = control_key(&response.control) != control_key(&old.control);
            if state.presentation.time < old.state.presentation.time {
                let older: BTreeMap<_, _> = old
                    .state
                    .actors
                    .iter()
                    .map(|a| (a.life.actor, a.life.generation))
                    .collect();
                let common: Vec<_> = state
                    .actors
                    .iter()
                    .filter(|a| older.contains_key(&a.life.actor))
                    .collect();
                if common.is_empty()
                    || common
                        .iter()
                        .any(|a| a.life.generation <= older[&a.life.actor])
                {
                    return Err("Replica scene clock regressed without a world life reset".into());
                }
                reset = true;
            }
        }
        let next = Frame {
            tick: response.tick,
            control: response.control.clone(),
            state: state.clone(),
        };
        if reset {
            self.previous = None;
        } else if self
            .current
            .as_ref()
            .is_some_and(|old| response.tick > old.tick)
        {
            self.previous = self.current.take();
        }
        self.current = Some(next);
        self.generations = generations;
        Ok(())
    }
    pub fn latest(&self) -> Option<&State> {
        self.current.as_ref().map(|f| &f.state)
    }
    /// Samples render poses only; resources, statuses, and effects use latest state.
    pub fn sample(&self, alpha: f32) -> Result<Option<Presentation>, String> {
        if !alpha.is_finite() || !(0. ..=1.).contains(&alpha) {
            return Err("Invalid replica interpolation fraction".into());
        }
        let Some(current) = &self.current else {
            return Ok(None);
        };
        let mut result = current.state.presentation.clone();
        let Some(previous) = &self.previous else {
            return Ok(Some(result));
        };
        result.time = previous.state.presentation.time
            + (result.time - previous.state.presentation.time) * alpha;
        let poses: BTreeMap<_, _> = previous
            .state
            .presentation
            .actors
            .iter()
            .map(|p| (p.life.actor, p))
            .collect();
        for actor in &mut result.actors {
            let Some(old) = poses.get(&actor.life.actor) else {
                continue;
            };
            if actor.life != old.life
                || actor.teleport_stamp != old.teleport_stamp
                || actor.actor.model != old.actor.model
                || actor.actor.scale != old.actor.scale
                || actor.visible != old.visible
                || (actor.health == 0) != (old.health == 0)
                || actor.animation != old.animation
                || actor.animation_time < old.animation_time
                || actor.actor.position.distance(old.actor.position) > self.max_displacement
            {
                continue;
            }
            actor.actor.position = old.actor.position.lerp(actor.actor.position, alpha);
            let yaw = (actor.actor.yaw - old.actor.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            actor.actor.yaw = old.actor.yaw + yaw * alpha;
            actor.animation_time =
                old.animation_time + (actor.animation_time - old.animation_time) * alpha;
        }
        let positions: BTreeMap<_, _> = result
            .actors
            .iter()
            .map(|p| (p.life.actor, p.actor.position.to_array()))
            .collect();
        for effect in &mut result.effects {
            if let Some(position) = positions.get(&effect.life.actor) {
                effect.position = *position;
            }
        }
        Ok(Some(result))
    }
}
fn control_key(control: &Option<Control>) -> Option<(Life, u64)> {
    control.as_ref().map(|c| (c.life, c.epoch))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        play::Game,
        service::{Chamber, auth::Gateway, presentation::Presentation, wire::ActorBinding},
    };
    use glam::Vec3;
    use verse_engine::director::Scene;
    pub(in crate::service) fn response(tick: u64) -> Response {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::combat_in(scene, false, 130).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        let game = Gateway::new(Chamber::new(game).unwrap()).unwrap();
        let snapshot = game.game().snapshot();
        let actors: Vec<_> = snapshot
            .actors
            .iter()
            .map(|a| {
                let life = game
                    .game()
                    .projectile_caster_life(a.id)
                    .or_else(|| {
                        game.game()
                            .ids
                            .iter()
                            .find(|(_, id)| **id == a.id)
                            .and_then(|(id, _)| game.game().actor_life(*id))
                    })
                    .unwrap();
                ActorBinding {
                    source: a.id,
                    life: life.into(),
                }
            })
            .collect();
        let presentation = Presentation::extract(game.game(), &actors);
        Response {
            version: VERSION,
            request_id: 1,
            instance: 130,
            tick,
            control: None,
            body: Reply::Snapshot {
                state: State {
                    hud: None,
                    snapshot,
                    actors,
                    presentation,
                },
            },
        }
    }
    fn attach_hud(r: &mut Response) {
        let life = r.control.as_ref().unwrap().life.into();
        let s = state(r);
        s.hud = Some(crate::hud::Own {
            life,
            time: s.presentation.time,
            resources: s.snapshot.player.clone(),
            casting: None,
            slots: crate::play::Ability::ALL
                .into_iter()
                .map(|ability| crate::hud::Slot {
                    ability,
                    ready: false,
                    remaining: 0.,
                    duration: 1.,
                })
                .collect(),
        });
    }
    fn state(r: &mut Response) -> &mut State {
        let Reply::Snapshot { state } = &mut r.body else {
            panic!()
        };
        state
    }
    #[test]
    fn positions_yaw_phases_and_effect_anchors_interpolate_without_changing_authority() {
        let first = response(1);
        let mut next = first.clone();
        next.tick = 2;
        let s = state(&mut next);
        s.presentation.time += 1.;
        for p in &mut s.presentation.actors {
            p.actor.position.x += 1.;
            p.animation_time += 1.;
        }
        let mut first = first;
        state(&mut first).presentation.actors[0].actor.yaw = 3.1;
        state(&mut next).presentation.actors[0].actor.yaw = -3.1;
        let mut buffer = Buffer::new(130, 2.).unwrap();
        buffer.push(&first).unwrap();
        buffer.push(&next).unwrap();
        let latest = serde_json::to_vec(buffer.latest().unwrap()).unwrap();
        let sample = buffer.sample(0.5).unwrap().unwrap();
        let Reply::Snapshot { state: original } = first.body else {
            panic!()
        };
        assert_eq!(
            sample.actors[0].actor.position.x,
            original.presentation.actors[0].actor.position.x + 0.5
        );
        assert!((sample.actors[0].actor.yaw.abs() - std::f32::consts::PI).abs() < 0.001);
        assert_eq!(
            sample.actors[0].animation_time,
            original.presentation.actors[0].animation_time + 0.5
        );
        let anchor = sample.effects[0].life;
        assert_eq!(
            sample.effects[0].position,
            sample
                .actors
                .iter()
                .find(|p| p.life == anchor)
                .unwrap()
                .actor
                .position
                .to_array()
        );
        assert_eq!(
            latest,
            serde_json::to_vec(buffer.latest().unwrap()).unwrap()
        );
    }
    #[test]
    fn short_teleports_animation_changes_death_and_control_changes_snap() {
        for fence in 0..4 {
            let first = response(1);
            let mut next = first.clone();
            next.tick = 2;
            let p = &mut state(&mut next).presentation.actors[0];
            p.actor.position.x += 0.25;
            p.animation_time += 1.;
            match fence {
                0 => p.teleport_stamp = Some(30.),
                1 => p.animation = verse_engine::motion::State::Cast.into(),
                2 => p.health = 0,
                _ => {}
            }
            if fence == 3 {
                let life = state(&mut next)
                    .presentation
                    .actors
                    .iter()
                    .find(|p| p.actor.model == "adventurer")
                    .unwrap()
                    .life;
                next.control = Some(Control {
                    life,
                    epoch: 1,
                    accepted_sequence: 0,
                });
                attach_hud(&mut next);
            }
            let target = state(&mut next).presentation.actors[0].actor.position;
            let mut b = Buffer::new(130, 2.).unwrap();
            b.push(&first).unwrap();
            b.push(&next).unwrap();
            assert_eq!(
                b.sample(0.).unwrap().unwrap().actors[0].actor.position,
                target
            );
        }
    }
    #[test]
    fn stale_lives_bad_ticks_and_malformed_state_leave_buffer_unchanged() {
        let first = response(5);
        let mut b = Buffer::new(130, 2.).unwrap();
        b.push(&first).unwrap();
        let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
        for invalid in 0..4 {
            let mut next = first.clone();
            match invalid {
                0 => next.tick = 4,
                1 => next.instance = 131,
                2 => state(&mut next).presentation.actors[0].actor.position = Vec3::splat(f32::NAN),
                _ => state(&mut next).presentation.time -= 1.,
            }
            assert!(b.push(&next).is_err());
            assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
        }
        assert!(b.sample(f32::NAN).is_err());
        assert!(Buffer::new(130, f32::INFINITY).is_err());
    }
    #[test]
    fn world_reset_advances_every_life_and_never_blends_old_corpses() {
        let first = response(1);
        let mut next = first.clone();
        next.tick = 2;
        let s = state(&mut next);
        s.presentation.time -= 1.;
        for a in &mut s.actors {
            a.life.generation += 1;
        }
        for p in &mut s.presentation.actors {
            p.life.generation += 1;
            p.actor.position.x += 0.5;
        }
        for e in &mut s.presentation.effects {
            e.life.generation += 1;
        }
        let target = s.presentation.actors[0].actor.position;
        let mut b = Buffer::new(130, 2.).unwrap();
        b.push(&first).unwrap();
        b.push(&next).unwrap();
        assert_eq!(
            b.sample(0.).unwrap().unwrap().actors[0].actor.position,
            target
        );
        let mut stale = first;
        stale.tick = 3;
        assert!(b.push(&stale).is_err());
    }
    #[test]
    fn control_regression_and_generation_capacity_are_atomic() {
        let mut first = response(1);
        let life = state(&mut first)
            .presentation
            .actors
            .iter()
            .find(|p| p.actor.model == "adventurer")
            .unwrap()
            .life;
        first.control = Some(Control {
            life,
            epoch: 2,
            accepted_sequence: 4,
        });
        attach_hud(&mut first);
        let mut b = Buffer::new(130, 2.).unwrap();
        b.push(&first).unwrap();
        let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
        let mut invalid = first.clone();
        invalid.tick = 2;
        invalid.control.as_mut().unwrap().epoch = 1;
        assert!(b.push(&invalid).is_err());
        assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
        let template = response(1);
        let mut b = Buffer::new(130, 2.).unwrap();
        for batch in 0..40 {
            let mut next = template.clone();
            next.tick = batch + 1;
            let s = state(&mut next);
            let offset = batch * 1000;
            for a in &mut s.actors {
                a.life.actor += offset;
            }
            for p in &mut s.presentation.actors {
                p.life.actor += offset;
                p.actor.id += offset;
            }
            for e in &mut s.presentation.effects {
                e.life.actor += offset;
            }
            if b.generations.len() + s.actors.len() > 512 {
                let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
                assert!(b.push(&next).is_err());
                assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
                assert!(b.generations.len() <= 512);
                return;
            }
            b.push(&next).unwrap();
        }
        panic!("Generation capacity was not exercised");
    }

    #[test]
    fn admitted_misty_step_stamp_survives_subsequent_casts_and_checkpoint() {
        let mut c = crate::utilities::Controls::default();
        let mut sim = crate::rules::Simulation::chamber([0.; 3], &[([0., 0., -2.], 15)])
            .unwrap()
            .0;
        let p = c
            .cast(
                &mut sim,
                crate::utilities::Utility::MistyStep,
                0.,
                Vec3::ZERO,
                Vec3::X,
                None,
            )
            .unwrap();
        let stamp = c.teleport_stamp().unwrap();
        c.cast(
            &mut sim,
            crate::utilities::Utility::Shield,
            0.,
            p,
            Vec3::X,
            None,
        )
        .unwrap();
        let restored: crate::utilities::Controls =
            serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();
        assert_eq!(restored.teleport_stamp(), Some(stamp));
    }
}
