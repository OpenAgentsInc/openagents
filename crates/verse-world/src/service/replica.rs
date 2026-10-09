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
    ended_generations: BTreeMap<u64, u64>,
    prop_generations: BTreeMap<u64, u64>,
    blocker_generations: BTreeMap<u64, u64>,
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
            ended_generations: BTreeMap::new(),
            prop_generations: BTreeMap::new(),
            blocker_generations: BTreeMap::new(),
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
        for life in state
            .actors
            .iter()
            .map(|a| a.life)
            .chain(state.presentation.corpses.iter().map(|p| p.life))
        {
            if generations
                .get(&life.actor)
                .is_some_and(|g| life.generation < *g)
            {
                return Err("Replica actor generation regressed".into());
            }
            generations.insert(life.actor, life.generation);
        }
        if generations.len() > 512 {
            return Err("Replica generation history budget exceeded".into());
        }
        let mut reset = false;
        if let Some(old) = &self.current {
            match (&old.state.social, &state.social) {
                (Some(before), Some(after))
                    if before.profile != after.profile || after.revision < before.revision =>
                {
                    return Err("Replica social profile changed or revision regressed".into());
                }
                (None, None) | (Some(_), Some(_)) => {}
                _ => return Err("Replica changed world rules without destination admission".into()),
            }
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
        let mut ended_generations = self.ended_generations.clone();
        for pose in state
            .presentation
            .actors
            .iter()
            .chain(&state.presentation.corpses)
        {
            if pose.health > 0
                && ended_generations
                    .get(&pose.life.actor)
                    .is_some_and(|g| pose.life.generation <= *g)
            {
                return Err("Replica ended actor life became alive again".into());
            }
            if pose.health == 0 {
                ended_generations
                    .entry(pose.life.actor)
                    .and_modify(|g| *g = (*g).max(pose.life.generation))
                    .or_insert(pose.life.generation);
            }
        }
        for corpse in &state.presentation.corpses {
            if state.scope.is_none()
                && self.generations.get(&corpse.life.actor) == Some(&corpse.life.generation)
                && self.current.as_ref().is_some_and(|c| {
                    !c.state
                        .presentation
                        .actors
                        .iter()
                        .chain(&c.state.presentation.corpses)
                        .any(|p| p.life == corpse.life)
                })
            {
                return Err("Replica corpse life is retired".into());
            }
        }
        if ended_generations.len() > 512 {
            return Err("Replica ended-life history budget exceeded".into());
        }
        let mut prop_generations = if reset {
            BTreeMap::new()
        } else {
            self.prop_generations.clone()
        };
        for prop in &state.presentation.props {
            if prop_generations.get(&prop.life.entity).is_some_and(|g| {
                prop.life.generation < *g
                    || (prop.life.generation == *g
                        && state.scope.is_none()
                        && !reset
                        && self.current.as_ref().is_some_and(|current| {
                            !current
                                .state
                                .presentation
                                .props
                                .iter()
                                .any(|p| p.life == prop.life)
                        }))
            }) {
                return Err("Replica prop generation regressed".into());
            }
            prop_generations.insert(prop.life.entity, prop.life.generation);
        }
        if prop_generations.len() > 512 {
            return Err("Replica prop generation history budget exceeded".into());
        }
        let mut blocker_generations = if reset {
            BTreeMap::new()
        } else {
            self.blocker_generations.clone()
        };
        for blocker in &state.presentation.blockers {
            if blocker_generations
                .get(&blocker.life.entity)
                .is_some_and(|g| {
                    blocker.life.generation < *g
                        || (blocker.life.generation == *g
                            && state.scope.is_none()
                            && !reset
                            && self.current.as_ref().is_some_and(|c| {
                                !c.state
                                    .presentation
                                    .blockers
                                    .iter()
                                    .any(|b| b.life == blocker.life)
                            }))
                })
            {
                return Err("Replica blocker generation is stale or retired".into());
            }
            blocker_generations.insert(blocker.life.entity, blocker.life.generation);
        }
        if blocker_generations.len() > 512 {
            return Err("Replica blocker history budget exceeded".into());
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
        self.ended_generations = ended_generations;
        self.prop_generations = prop_generations;
        self.blocker_generations = blocker_generations;
        Ok(())
    }
    pub fn tick(&self) -> Option<u64> {
        self.current.as_ref().map(|f| f.tick)
    }
    pub fn control(&self) -> Option<&Control> {
        self.current.as_ref().and_then(|f| f.control.as_ref())
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
        let props: BTreeMap<_, _> = previous
            .state
            .presentation
            .props
            .iter()
            .map(|p| (p.life.entity, p))
            .collect();
        for prop in &mut result.props {
            let Some(old) = props.get(&prop.life.entity) else {
                continue;
            };
            if prop.life != old.life
                || prop.kind != old.kind
                || prop.secured != old.secured
                || prop.dimensions != old.dimensions
                || prop.center.distance(old.center) > self.max_displacement
            {
                continue;
            }
            prop.center = old.center.lerp(prop.center, alpha);
            prop.rotation = old.rotation.slerp(prop.rotation, alpha);
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
                    social: None,
                    scope: None,
                    collision: None,
                    movement: None,
                    hud: None,
                    snapshot,
                    actors,
                    presentation,
                },
            },
        }
    }
    pub(in crate::service) fn attach_hud(r: &mut Response) {
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
                    credit_step: 0,
                    world_step: 0,
                    life,
                    epoch: 1,
                    accepted_sequence: 0,
                    applied_movement: None,
                    dynamic: Vec::new(),
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
            credit_step: 0,
            world_step: 0,
            life,
            epoch: 2,
            accepted_sequence: 4,
            applied_movement: None,
            dynamic: Vec::new(),
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
    fn prop() -> crate::visuals::Prop {
        crate::visuals::Prop {
            life: physics::queries::Life {
                instance: 130,
                entity: crate::spells::PROP_ENTITY_BASE,
                generation: 0,
            },
            kind: crate::spells::PropKind::Crate,
            secured: false,
            center: Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            dimensions: Vec3::ONE,
        }
    }
    #[test]
    fn prop_poses_interpolate_snap_new_lives_and_retire_removed_lives() {
        let mut first = response(1);
        state(&mut first).presentation.props = vec![prop()];
        let mut next = first.clone();
        next.tick = 2;
        state(&mut next).presentation.time += 1.;
        let p = &mut state(&mut next).presentation.props[0];
        p.center = Vec3::X * 2.;
        p.rotation = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut b = Buffer::new(130, 4.).unwrap();
        b.push(&first).unwrap();
        b.push(&next).unwrap();
        let sampled = b.sample(0.5).unwrap().unwrap();
        assert_eq!(sampled.props[0].center, Vec3::X);
        assert!(
            sampled.props[0]
                .rotation
                .dot(glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_4))
                .abs()
                > 0.99999
        );
        next.tick = 3;
        let p = &mut state(&mut next).presentation.props[0];
        p.life.generation = 1;
        p.center = Vec3::X * 3.;
        b.push(&next).unwrap();
        assert_eq!(b.sample(0.).unwrap().unwrap().props[0].center, Vec3::X * 3.);
        let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
        let mut stale = next.clone();
        stale.tick = 4;
        state(&mut stale).presentation.props[0].life.generation = 0;
        assert!(b.push(&stale).is_err());
        assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
        let mut removed = next.clone();
        removed.tick = 4;
        state(&mut removed).presentation.props.clear();
        b.push(&removed).unwrap();
        assert!(b.sample(0.).unwrap().unwrap().props.is_empty());
        next.tick = 5;
        assert!(b.push(&next).is_err());
        state(&mut next).presentation.props[0].life.generation = 2;
        b.push(&next).unwrap();
        let mut reset = next.clone();
        reset.tick = 6;
        let s = state(&mut reset);
        s.presentation.time = 0.;
        s.snapshot.elapsed = 0.;
        for p in &mut s.presentation.actors {
            p.life.generation += 1;
        }
        for a in &mut s.actors {
            a.life.generation += 1;
        }
        for e in &mut s.presentation.effects {
            e.life.generation += 1;
        }
        s.presentation.props[0].life.generation = 0;
        s.presentation.props[0].center = Vec3::X * 4.;
        b.push(&reset).unwrap();
        assert_eq!(b.sample(0.).unwrap().unwrap().props[0].center, Vec3::X * 4.);
    }
    #[test]
    fn malformed_prop_transforms_and_budgets_leave_the_replica_unchanged() {
        let mut first = response(1);
        state(&mut first).presentation.props = vec![prop()];
        let mut b = Buffer::new(130, 4.).unwrap();
        b.push(&first).unwrap();
        let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
        for case in 0..7 {
            let mut bad = first.clone();
            bad.tick = 2;
            let p = &mut state(&mut bad).presentation.props[0];
            match case {
                0 => p.life.instance += 1,
                1 => p.center.x = f32::NAN,
                2 => p.rotation = glam::Quat::from_xyzw(0., 0., 0., 0.),
                3 => p.dimensions.x = 0.,
                4 => p.life.entity = 0,
                5 => state(&mut bad).presentation.props.push(prop()),
                _ => {
                    state(&mut bad).presentation.props = vec![prop(); crate::spells::MAX_PROPS + 1]
                }
            }
            assert!(b.push(&bad).is_err());
            assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
        }
    }
    #[test]
    fn blocker_bounds_validate_atomically_and_retire_without_interpolation() {
        let blocker = crate::visuals::Blocker {
            life: physics::queries::Life {
                instance: 130,
                entity: 10000,
                generation: 0,
            },
            min: glam::DVec3::ZERO,
            max: glam::DVec3::ONE,
            table_proxy: true,
        };
        let mut first = response(1);
        state(&mut first).presentation.blockers = vec![blocker.clone()];
        let mut b = Buffer::new(130, 4.).unwrap();
        b.push(&first).unwrap();
        let saved = serde_json::to_vec(b.latest().unwrap()).unwrap();
        for case in 0..6 {
            let mut bad = first.clone();
            bad.tick = 2;
            let p = &mut state(&mut bad).presentation.blockers[0];
            match case {
                0 => p.life.instance += 1,
                1 => p.min.x = f64::NAN,
                2 => p.max = p.min,
                3 => p.table_proxy = false,
                4 => state(&mut bad).presentation.blockers.push(blocker.clone()),
                _ => state(&mut bad).presentation.blockers = vec![blocker.clone(); 257],
            }
            assert!(b.push(&bad).is_err());
            assert_eq!(saved, serde_json::to_vec(b.latest().unwrap()).unwrap());
        }
        let mut moved = first.clone();
        moved.tick = 2;
        state(&mut moved).presentation.blockers[0].max.x = 2.;
        b.push(&moved).unwrap();
        assert_eq!(b.sample(0.).unwrap().unwrap().blockers[0].max.x, 2.);
        let mut removed = moved.clone();
        removed.tick = 3;
        state(&mut removed).presentation.blockers.clear();
        b.push(&removed).unwrap();
        assert!(b.sample(0.).unwrap().unwrap().blockers.is_empty());
        moved.tick = 4;
        assert!(b.push(&moved).is_err());
        state(&mut moved).presentation.blockers[0].life.generation = 1;
        b.push(&moved).unwrap();
    }
}
