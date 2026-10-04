//! Read-only remote scene projection for native renderers.
use super::{
    event_cursor::{Delivery, Gap},
    replica::Buffer,
    wire::{Reply, Response},
};
use crate::{
    events::{Event, Kind},
    rules::ProjectileKind,
};
use glam::Vec3;
use verse_engine::director::{Action, ActorFrame, Cue, Frame, Projectile};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov: f32,
}
impl Camera {
    fn validate(self) -> Result<(), String> {
        if !self.eye.is_finite()
            || !self.target.is_finite()
            || self.eye.abs().max_element() > 1_000_000.
            || self.target.abs().max_element() > 1_000_000.
            || self.eye.distance_squared(self.target) < 0.0001
            || !self.fov.is_finite()
            || !(1.0..=179.0).contains(&self.fov)
        {
            return Err("Invalid remote view camera".into());
        }
        Ok(())
    }
}
/// A single admitted presentation sample for all native world render consumers.
pub struct SceneSample {
    pub frame: Frame,
    pub presentation: super::presentation::Presentation,
    pub combat: crate::visuals::Combat,
}
/// Owns presentation history only. Initialize progress from the worker's cursor.
pub struct View {
    instance: u64,
    target: Option<verse_engine::core::LifeId>,
    replica: Buffer,
    events: Vec<Event>,
    after: u64,
    reset_tick: u64,
    event_tick: u64,
    handoff: Option<f32>,
    gap: Option<Gap>,
}
impl View {
    pub fn new(instance: u64, displacement: f32, after: u64) -> Result<Self, String> {
        Ok(Self {
            instance,
            target: None,
            replica: Buffer::new(instance, displacement)?,
            events: Vec::new(),
            after,
            reset_tick: 0,
            event_tick: 0,
            handoff: None,
            gap: None,
        })
    }
    fn targetable(&self, life: verse_engine::core::LifeId) -> bool {
        !self
            .events
            .iter()
            .any(|e| e.actor == Some(life) && matches!(e.kind, Kind::Death))
            && self.replica.latest().is_some_and(|s| {
                s.presentation.actors.iter().any(|p| {
                    verse_engine::core::LifeId::from(p.life) == life
                        && p.visible
                        && p.health > 0
                        && p.actor.nameplate
                        && p.actor.model != "adventurer"
                })
            })
    }
    pub fn target(&self) -> Option<verse_engine::core::LifeId> {
        self.target
    }
    /// Retains selection locally; casting still passes through host command admission.
    pub fn select_target(
        &mut self,
        life: Option<verse_engine::core::LifeId>,
    ) -> Result<(), String> {
        if life.is_some_and(|l| l.instance != self.instance || !self.targetable(l)) {
            return Err("Remote target life is unavailable".into());
        }
        self.target = life;
        Ok(())
    }
    pub fn cycle_target(&mut self) -> Option<verse_engine::core::LifeId> {
        let mut lives: Vec<verse_engine::core::LifeId> = self
            .replica
            .latest()
            .into_iter()
            .flat_map(|s| &s.presentation.actors)
            .filter(|p| self.targetable(p.life.into()))
            .map(|p| p.life.into())
            .collect();
        lives.sort_by_key(|life| life.actor);
        self.target = if lives.is_empty() {
            None
        } else {
            Some(
                lives[self
                    .target
                    .and_then(|life| lives.iter().position(|l| *l == life))
                    .map_or(0, |i| (i + 1) % lives.len())],
            )
        };
        self.target
    }
    pub fn replica(&self) -> &Buffer {
        &self.replica
    }
    pub fn camera_handoff(&self) -> bool {
        self.handoff.is_some_and(|at| {
            self.replica
                .latest()
                .is_some_and(|s| s.presentation.time >= at)
        })
    }
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    /// Projects actual committed damage only onto the matching sampled actor life.
    pub fn damage_numbers(&self, alpha: f32) -> Result<Vec<crate::play::DamageNumber>, String> {
        let Some(presentation) = self.replica.sample(alpha)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .events
            .iter()
            .filter_map(|event| {
                let Kind::Damage { amount, incoming } = event.kind else {
                    return None;
                };
                if !(0.0..1.35).contains(&(presentation.time - event.time)) {
                    return None;
                }
                let life = event.actor?;
                let pose = presentation
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == life)?;
                Some(crate::play::DamageNumber {
                    actor: life.actor,
                    amount,
                    at: event.time,
                    position: pose.actor.position,
                    incoming,
                    serial: event.serial,
                })
            })
            .collect())
    }
    pub fn last_gap(&self) -> Option<Gap> {
        self.gap
    }
    pub fn push_snapshot(&mut self, response: &Response) -> Result<(), String> {
        let reset = match (&response.body, self.replica.latest()) {
            (Reply::Snapshot { state }, Some(old)) => {
                state.presentation.time < old.presentation.time
            }
            _ => false,
        };
        self.replica.push(response)?;
        if self.target.is_some_and(|life| !self.targetable(life)) {
            self.target = None;
        }
        if reset {
            self.events.clear();
            self.handoff = None;
            self.reset_tick = response.tick;
        }
        Ok(())
    }
    /// Admits a bounded worker delivery atomically; duplicates never replay cues.
    pub fn push_events(&mut self, delivery: &Delivery) -> Result<(), String> {
        if delivery.events.len() > 64 {
            return Err("Remote view event budget exceeded".into());
        }
        let mut after = self.after;
        let mut new_gap = None;
        if let Some(gap) = delivery.gap {
            if gap.first == 0 || gap.last < gap.first {
                return Err("Remote view retention gap mismatch".into());
            }
            if gap.last > after {
                if gap.first != after.saturating_add(1) || delivery.events.is_empty() {
                    return Err("Remote view retention gap mismatch".into());
                }
                after = gap.last;
                new_gap = Some(gap);
            }
        }
        let mut serial = 0;
        let mut previous_tick = 0;
        let mut event_tick = self.event_tick;
        for event in &delivery.events {
            if event.instance != self.instance
                || event.serial == 0
                || event.serial <= serial
                || event.tick < previous_tick
                || event.actor.is_some_and(|a| a.instance != self.instance)
                || !event.time.is_finite()
                || event.time < 0.
            {
                return Err("Invalid remote view event identity".into());
            }
            match &event.kind {
                Kind::Dialogue { text } if text.len() > 4096 => {
                    return Err("Remote dialogue exceeds byte budget".into());
                }
                Kind::Damage { amount, .. }
                    if event.actor.is_none() || !(1..=1_000_000).contains(amount) =>
                {
                    return Err("Invalid remote damage event".into());
                }
                Kind::Death | Kind::Respawn if event.actor.is_none() => {
                    return Err("Remote lifecycle event has no life".into());
                }
                _ => {}
            }
            serial = event.serial;
            previous_tick = event.tick;
            if event.serial > after {
                if event.tick < event_tick {
                    return Err("Remote event tick regressed".into());
                }
                event_tick = event.tick;
                if event.serial
                    != after
                        .checked_add(1)
                        .ok_or("Remote event serial exhausted")?
                {
                    return Err("Remote view event continuity mismatch".into());
                }
                after = event.serial;
            }
        }
        if new_gap.is_some() {
            self.events.clear();
            self.gap = new_gap;
        }
        for event in delivery
            .events
            .iter()
            .filter(|e| e.serial > self.after && e.tick >= self.reset_tick)
        {
            if matches!(event.kind, Kind::CameraHandoff) {
                self.handoff = Some(event.time);
            }
            if matches!(event.kind, Kind::Death) && event.actor == self.target {
                self.target = None;
            }
            self.events.push(event.clone());
        }
        if self.events.len() > 128 {
            self.events.drain(..self.events.len() - 128);
        }
        self.after = after;
        self.event_tick = event_tick;
        Ok(())
    }
    pub fn frame(&self, alpha: f32, camera: Camera) -> Result<Option<Frame>, String> {
        Ok(self.scene_sample(alpha, camera)?.map(|sample| sample.frame))
    }
    pub fn scene_sample(&self, alpha: f32, camera: Camera) -> Result<Option<SceneSample>, String> {
        camera.validate()?;
        let Some(presentation) = self.replica.sample(alpha)? else {
            return Ok(None);
        };
        let mut combat = self
            .replica
            .latest()
            .unwrap()
            .combat_visuals(self.instance)?;
        combat.time = presentation.time;
        for (player, effect) in combat.players.iter_mut().zip(&presentation.effects) {
            player.position = effect.position.into();
        }
        let actors: Vec<_> = presentation
            .actors
            .iter()
            .cloned()
            .map(|p| ActorFrame {
                actor: p.actor,
                life: Some(p.life.into()),
                animation: p.animation,
                animation_time: p.animation_time,
                visible: p.visible,
                health: p.health,
            })
            .collect();
        let yell = self.events.iter().rev().find_map(|event| {
            let Kind::Dialogue { text } = &event.kind else {
                return None;
            };
            if !(0.0..5.0).contains(&(presentation.time - event.time)) {
                return None;
            }
            let life = event.actor?;
            let actor = actors
                .iter()
                .find(|a| a.life == Some(life) && a.visible && a.health > 0)?;
            Some(Cue {
                at: event.time,
                actor: life.actor,
                action: Action::Yell {
                    text: text.clone(),
                    animation: actor.animation,
                },
            })
        });
        let projectiles = self
            .replica
            .latest()
            .unwrap()
            .snapshot
            .projectiles
            .iter()
            .filter(|p| p.kind == ProjectileKind::Bow)
            .map(|p| Projectile {
                position: p.pos.into(),
                direction: Vec3::from(p.vel).normalize_or(Vec3::NEG_Z),
            })
            .collect();
        let frame = Frame {
            time: presentation.time,
            actors,
            eye: camera.eye,
            target: camera.target,
            fov: camera.fov,
            yell,
            projectiles,
            shots: Vec::new(),
        };
        Ok(Some(SceneSample {
            frame,
            presentation,
            combat,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{replica::tests::response, wire::State};
    fn state(r: &mut Response) -> &mut State {
        let Reply::Snapshot { state } = &mut r.body else {
            panic!("Expected snapshot")
        };
        state
    }
    fn camera() -> Camera {
        Camera {
            eye: Vec3::new(0., 3., -5.),
            target: Vec3::Y,
            fov: 60.,
        }
    }
    fn dialogue(serial: u64, time: f32, life: verse_engine::core::LifeId) -> Event {
        Event {
            instance: 130,
            serial,
            tick: 10,
            time,
            actor: Some(life),
            kind: Kind::Dialogue {
                text: "Our master Claude has been ensouled!".into(),
            },
        }
    }
    #[test]
    fn scene_sample_keeps_interpolated_body_and_effect_anchors_together() {
        let mut view = View::new(130, 10., 0).unwrap();
        let first = response(1);
        view.push_snapshot(&first).unwrap();
        let mut next = first.clone();
        next.tick = 2;
        let s = state(&mut next);
        s.presentation.time += 0.1;
        s.snapshot.elapsed = s.presentation.time;
        for actor in &mut s.presentation.actors {
            actor.actor.position.x += 1.;
        }
        for effect in &mut s.presentation.effects {
            effect.position[0] += 1.;
        }
        view.push_snapshot(&next).unwrap();
        let sampled = view.scene_sample(0.5, camera()).unwrap().unwrap();
        assert_eq!(sampled.frame.time, sampled.combat.time);
        assert_eq!(sampled.frame.time, sampled.presentation.time);
        for (player, effect) in sampled
            .combat
            .players
            .iter()
            .zip(&sampled.presentation.effects)
        {
            assert_eq!(player.position, Vec3::from(effect.position));
        }
        for (actor, pose) in sampled
            .frame
            .actors
            .iter()
            .zip(&sampled.presentation.actors)
        {
            assert_eq!(actor.actor.position, pose.actor.position);
            assert_eq!(actor.life, Some(pose.life.into()));
        }
        assert!(view.scene_sample(f32::NAN, camera()).is_err());
    }
    #[test]
    fn admitted_poses_bow_flights_and_programmatic_dialogue_project_into_native_frames() {
        let mut r = response(10);
        let s = state(&mut r);
        let time = s.presentation.time;
        let life = s.presentation.actors[0].life.into();
        s.snapshot.projectiles.push(crate::rules::Projectile {
            id: 1,
            caster: s.actors[0].source,
            kind: ProjectileKind::Bow,
            pos: [1., 2., 3.],
            vel: [4., 0., 0.],
        });
        let mut view = View::new(130, 5., 0).unwrap();
        view.push_snapshot(&r).unwrap();
        view.push_events(&Delivery {
            events: vec![
                dialogue(1, time, life),
                Event {
                    instance: 130,
                    serial: 2,
                    tick: 10,
                    time: time + 1.,
                    actor: None,
                    kind: Kind::CameraHandoff,
                },
            ],
            gap: None,
        })
        .unwrap();
        let frame = view.frame(1., camera()).unwrap().unwrap();
        assert_eq!(frame.projectiles.len(), 1);
        assert_eq!(frame.projectiles[0].direction, Vec3::X);
        assert_eq!(frame.yell.unwrap().actor, life.actor);
        assert!(frame.actors.iter().all(|a| a.life.is_some()));
        assert!(!view.camera_handoff());
        r.tick = 11;
        state(&mut r).presentation.time += 2.;
        state(&mut r).snapshot.projectiles[0].vel = [0.; 3];
        view.push_snapshot(&r).unwrap();
        assert!(view.camera_handoff());
        assert_eq!(
            view.frame(1., camera()).unwrap().unwrap().projectiles[0].direction,
            Vec3::NEG_Z
        );
        for bad in [
            Camera {
                eye: Vec3::splat(f32::NAN),
                ..camera()
            },
            Camera {
                target: camera().eye,
                ..camera()
            },
            Camera {
                fov: 180.,
                ..camera()
            },
        ] {
            assert!(view.frame(1., bad).is_err());
        }
    }
    #[test]
    fn event_duplicates_gaps_and_malformed_batches_are_atomic() {
        let mut r = response(10);
        let time = state(&mut r).presentation.time;
        let life = state(&mut r).presentation.actors[0].life.into();
        let mut view = View::new(130, 5., 0).unwrap();
        view.push_snapshot(&r).unwrap();
        let first = Delivery {
            events: vec![dialogue(1, time, life)],
            gap: None,
        };
        view.push_events(&first).unwrap();
        view.push_events(&first).unwrap();
        assert_eq!(view.events.len(), 1);
        let gap = Delivery {
            events: vec![dialogue(5, time, life)],
            gap: Some(Gap { first: 2, last: 4 }),
        };
        let mut bad = gap.clone();
        bad.events[0].actor.as_mut().unwrap().instance += 1;
        assert!(view.push_events(&bad).is_err());
        assert_eq!(view.after, 1);
        assert!(view.last_gap().is_none());
        view.push_events(&gap).unwrap();
        view.push_events(&gap).unwrap();
        assert_eq!(view.after, 5);
        assert_eq!(view.events.len(), 1);
        assert_eq!(view.last_gap(), gap.gap);
        for case in 0..5 {
            let mut bad = Delivery {
                events: vec![dialogue(6, time, life)],
                gap: None,
            };
            match case {
                0 => bad.events[0].serial = 7,
                1 => bad.events[0].time = f32::NAN,
                2 => bad.events = vec![dialogue(6, time, life); 65],
                3 => {
                    bad.events[0].kind = Kind::Damage {
                        amount: -1,
                        incoming: true,
                    }
                }
                _ => bad.events[0].tick = 9,
            }
            assert!(view.push_events(&bad).is_err());
            assert_eq!(view.after, 5);
        }
    }
    #[test]
    fn respawn_and_world_reset_fence_old_dialogue_and_camera_history() {
        let mut r = response(10);
        let time = state(&mut r).presentation.time;
        let old = state(&mut r).presentation.actors[0].life;
        let mut view = View::new(130, 5., 0).unwrap();
        view.push_snapshot(&r).unwrap();
        view.push_events(&Delivery {
            events: vec![dialogue(1, time, old.into())],
            gap: None,
        })
        .unwrap();
        r.tick = 11;
        let s = state(&mut r);
        s.presentation.actors[0].life.generation += 1;
        for binding in &mut s.actors {
            if binding.life == old {
                binding.life.generation += 1;
            }
        }
        for effect in &mut s.presentation.effects {
            if effect.life == old {
                effect.life.generation += 1;
            }
        }
        view.push_snapshot(&r).unwrap();
        assert!(view.frame(1., camera()).unwrap().unwrap().yell.is_none());
        r.tick = 12;
        let s = state(&mut r);
        s.presentation.time = 0.;
        s.snapshot.elapsed = 0.;
        for p in &mut s.presentation.actors {
            p.life.generation += 1;
        }
        for b in &mut s.actors {
            b.life.generation += 1;
        }
        for e in &mut s.presentation.effects {
            e.life.generation += 1;
        }
        let fresh = s.presentation.actors[0].life;
        view.push_snapshot(&r).unwrap();
        view.push_events(&Delivery {
            events: vec![Event {
                instance: 130,
                serial: 2,
                tick: 11,
                time,
                actor: None,
                kind: Kind::CameraHandoff,
            }],
            gap: None,
        })
        .unwrap();
        assert!(!view.camera_handoff());
        assert!(view.events.is_empty());
        let mut event = dialogue(3, 0., fresh.into());
        event.tick = 12;
        view.push_events(&Delivery {
            events: vec![event],
            gap: None,
        })
        .unwrap();
        assert!(view.frame(1., camera()).unwrap().unwrap().yell.is_some());
    }
    #[cfg(feature = "service-net")]
    #[tokio::test]
    async fn tls_worker_updates_drive_read_only_native_frames() {
        use crate::service::{
            client::Client,
            event_cursor::Cursor,
            net::tests::{key, start},
            worker::{self, Update},
        };
        use std::time::Duration;
        use tokio::{sync::oneshot, time::timeout};
        let keys = [key(91), key(92), key(93)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let client = Client::connect(
            address,
            rustls::pki_types::ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[2],
        )
        .await
        .unwrap();
        let (_input, inputs, updates, mut output) = worker::channels();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(worker::run(
            client,
            Cursor::new(120),
            Duration::from_millis(33),
            inputs,
            updates,
            stopped,
        ));
        let mut view = View::new(120, 5., 0).unwrap();
        let mut received_events = false;
        for _ in 0..4 {
            match timeout(Duration::from_secs(2), output.recv())
                .await
                .unwrap()
                .unwrap()
            {
                Update::Snapshot(r) => view.push_snapshot(&r).unwrap(),
                Update::Events { delivery, .. } => {
                    view.push_events(&delivery).unwrap();
                    received_events = true;
                }
                Update::Outcome(_) => panic!("Spectator issued no command"),
            }
            if received_events {
                break;
            }
        }
        let frame = view.frame(1., camera()).unwrap().unwrap();
        assert!(received_events && frame.actors.len() > 2);
        assert!(frame.actors.iter().all(|a| a.life.unwrap().instance == 120));
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
    #[test]
    fn actual_damage_numbers_expire_and_never_attach_to_a_respawned_life() {
        let mut r = response(10);
        let time = state(&mut r).presentation.time;
        let life = state(&mut r).presentation.actors[0].life;
        let mut view = View::new(130, 5., 0).unwrap();
        view.push_snapshot(&r).unwrap();
        let mut a = dialogue(1, time, life.into());
        a.kind = Kind::Damage {
            amount: 45,
            incoming: false,
        };
        let mut b = dialogue(2, time, life.into());
        b.kind = Kind::Damage {
            amount: 20,
            incoming: true,
        };
        let delivery = Delivery {
            events: vec![a, b],
            gap: None,
        };
        view.push_events(&delivery).unwrap();
        view.push_events(&delivery).unwrap();
        let numbers = view.damage_numbers(1.).unwrap();
        assert_eq!(numbers.len(), 2);
        assert_eq!(numbers[0].amount, 45);
        assert!(!numbers[0].incoming);
        assert_eq!(numbers[1].amount, 20);
        assert!(numbers[1].incoming);
        r.tick = 11;
        state(&mut r).presentation.actors[0].health = 0;
        view.push_snapshot(&r).unwrap();
        assert_eq!(view.damage_numbers(1.).unwrap().len(), 2);
        r.tick = 12;
        let s = state(&mut r);
        s.presentation.actors[0].life.generation += 1;
        for b in &mut s.actors {
            if b.life == life {
                b.life.generation += 1;
            }
        }
        for e in &mut s.presentation.effects {
            if e.life == life {
                e.life.generation += 1;
            }
        }
        view.push_snapshot(&r).unwrap();
        assert!(view.damage_numbers(1.).unwrap().is_empty());
        let fresh = state(&mut r).presentation.actors[0].life;
        let mut e = dialogue(3, time, fresh.into());
        e.tick = 12;
        e.kind = Kind::Damage {
            amount: 17,
            incoming: true,
        };
        view.push_events(&Delivery {
            events: vec![e],
            gap: None,
        })
        .unwrap();
        assert_eq!(view.damage_numbers(1.).unwrap()[0].amount, 17);
        r.tick = 13;
        state(&mut r).presentation.time = time + 1.4;
        view.push_snapshot(&r).unwrap();
        assert!(view.damage_numbers(1.).unwrap().is_empty());
    }
    #[test]
    fn exact_life_selection_cycles_stably_and_clears_on_committed_death_or_respawn() {
        let mut r = response(10);
        let mut view = View::new(130, 5., 0).unwrap();
        assert!(view.cycle_target().is_none());
        view.push_snapshot(&r).unwrap();
        let first = view.cycle_target().unwrap();
        let second = view.cycle_target().unwrap();
        assert!(second.actor > first.actor);
        view.select_target(Some(first)).unwrap();
        let player = state(&mut r)
            .presentation
            .actors
            .iter()
            .find(|p| p.actor.model == "adventurer")
            .unwrap()
            .life
            .into();
        for bad in [
            player,
            verse_engine::core::LifeId {
                instance: 131,
                ..first
            },
            verse_engine::core::LifeId {
                generation: first.generation + 1,
                ..first
            },
        ] {
            assert!(view.select_target(Some(bad)).is_err());
            assert_eq!(view.target(), Some(first));
        }
        let mut invalid = r.clone();
        invalid.instance = 131;
        assert!(view.push_snapshot(&invalid).is_err());
        assert_eq!(view.target(), Some(first));
        let time = state(&mut r).presentation.time;
        let death = Event {
            instance: 130,
            serial: 1,
            tick: 10,
            time,
            actor: Some(first),
            kind: Kind::Death,
        };
        view.push_events(&Delivery {
            events: vec![death],
            gap: None,
        })
        .unwrap();
        assert!(view.target().is_none());
        assert_ne!(view.cycle_target(), Some(first));
        assert!(view.select_target(Some(first)).is_err());
        view.select_target(Some(second)).unwrap();
        r.tick = 11;
        let s = state(&mut r);
        for p in &mut s.presentation.actors {
            if p.life.actor == second.actor {
                p.life.generation += 1;
            }
        }
        for b in &mut s.actors {
            if b.life.actor == second.actor {
                b.life.generation += 1;
            }
        }
        view.push_snapshot(&r).unwrap();
        assert!(view.target().is_none());
        assert!(view.select_target(Some(second)).is_err());
        let fresh = verse_engine::core::LifeId {
            generation: second.generation + 1,
            ..second
        };
        view.select_target(Some(fresh)).unwrap();
        r.tick = 12;
        state(&mut r)
            .presentation
            .actors
            .iter_mut()
            .find(|p| p.life.actor == fresh.actor)
            .unwrap()
            .visible = false;
        view.push_snapshot(&r).unwrap();
        assert!(view.target().is_none());
        view.select_target(None).unwrap();
    }
}
