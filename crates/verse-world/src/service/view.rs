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
    /// Vertical field of view in degrees.
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
fn same_quest_rewards(
    current: &[super::progression::Progress],
    previous: &[super::progression::Progress],
) -> bool {
    current.len() == previous.len()
        && current.iter().zip(previous).all(|(current, previous)| {
            // Only giver life and interaction availability belong to the world tick.
            let mut compared = current.clone();
            compared.giver_life = previous.giver_life;
            compared.interactable = previous.interactable;
            compared == *previous
        })
}
fn snapshot_generation(state: Option<&super::wire::State>, actor: u64) -> Option<u64> {
    state
        .into_iter()
        .flat_map(|s| s.presentation.actors.iter().chain(&s.presentation.corpses))
        .filter(|p| p.life.actor == actor)
        .map(|p| p.life.generation)
        .max()
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiverInteraction {
    pub character: verse_engine::core::LifeId,
    pub giver: verse_engine::core::LifeId,
}

/// Owns presentation history only. Initialize progress from the worker's cursor.
pub struct View {
    instance: u64,
    target: Option<verse_engine::core::LifeId>,
    interaction: Option<GiverInteraction>,
    replica: Buffer,
    events: Vec<Event>,
    after: u64,
    reset_tick: u64,
    event_tick: u64,
    handoff: Option<f32>,
    gap: Option<Gap>,
    inventory: Option<(u64, super::wire::Inventory)>,
    giver_generations: std::collections::BTreeMap<u64, u64>,
}
impl View {
    pub fn instance(&self) -> u64 {
        self.instance
    }
    pub fn new(instance: u64, displacement: f32, after: u64) -> Result<Self, String> {
        Ok(Self {
            instance,
            target: None,
            interaction: None,
            replica: Buffer::new(instance, displacement)?,
            events: Vec::new(),
            after,
            reset_tick: 0,
            event_tick: 0,
            handoff: None,
            gap: None,
            inventory: None,
            giver_generations: Default::default(),
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
                        && !p.actor.friendly
                        && p.actor.model != "adventurer"
                })
            })
    }
    /// Projects owned quest discovery markers onto current visible friendly lives.
    pub fn quest_markers(
        &self,
    ) -> std::collections::BTreeMap<verse_engine::core::LifeId, super::progression::Marker> {
        let mut markers = std::collections::BTreeMap::new();
        let Some(inventory) = self.inventory() else {
            return markers;
        };
        let Some(snapshot) = self.replica.latest() else {
            return markers;
        };
        for quest in &inventory.quest_log {
            let (Some(life), Some(marker)) = (quest.giver_life, quest.marker()) else {
                continue;
            };
            if life.instance != self.instance
                || self
                    .events
                    .iter()
                    .any(|e| e.actor == Some(life) && matches!(e.kind, Kind::Death))
                || !snapshot.presentation.actors.iter().any(|p| {
                    verse_engine::core::LifeId::from(p.life) == life
                        && p.visible
                        && p.health > 0
                        && p.actor.friendly
                })
            {
                continue;
            }
            markers
                .entry(life)
                .and_modify(|old: &mut super::progression::Marker| *old = (*old).max(marker))
                .or_insert(marker);
        }
        markers
    }

    fn giver_reachable(&self, giver: verse_engine::core::LifeId) -> bool {
        let Some(inventory) = self.inventory() else {
            return false;
        };
        let Some(state) = self.replica.latest() else {
            return false;
        };
        let Some(hud) = state.hud.as_ref() else {
            return false;
        };
        if hud.resources.hp <= 0
            || !self.quest_markers().contains_key(&giver)
            || !inventory
                .quest_log
                .iter()
                .any(|q| q.giver_life == Some(giver) && q.interactable && q.marker().is_some())
        {
            return false;
        }
        let character = state
            .presentation
            .actors
            .iter()
            .find(|p| verse_engine::core::LifeId::from(p.life) == hud.life);
        let npc = state
            .presentation
            .actors
            .iter()
            .find(|p| verse_engine::core::LifeId::from(p.life) == giver);
        character
            .zip(npc)
            .is_some_and(|(a, b)| a.actor.position.distance(b.actor.position) <= 4.)
    }

    pub fn open_giver(&mut self, giver: verse_engine::core::LifeId) -> Result<(), String> {
        if !self.giver_reachable(giver) {
            return Err("Quest giver is not currently reachable".into());
        }
        self.interaction = Some(GiverInteraction {
            character: self.inventory().unwrap().life.into(),
            giver,
        });
        Ok(())
    }

    pub fn interaction(&self) -> Option<GiverInteraction> {
        let interaction = self.interaction?;
        (self.inventory()?.life == interaction.character.into()
            && self.giver_reachable(interaction.giver))
        .then_some(interaction)
    }

    pub fn interaction_quests(&self) -> Vec<&super::progression::Progress> {
        let Some(interaction) = self.interaction() else {
            return vec![];
        };
        self.inventory()
            .unwrap()
            .quest_log
            .iter()
            .filter(|q| q.giver_life == Some(interaction.giver) && q.marker().is_some())
            .collect()
    }

    pub fn close_giver(&mut self) {
        self.interaction = None;
    }

    fn retire_interaction(&mut self) {
        if self.interaction().is_none() {
            self.interaction = None;
        }
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
    /// Hides retained counters until their exact life matches the owned snapshot.
    pub fn inventory(&self) -> Option<&super::wire::Inventory> {
        let (_, inventory) = self.inventory.as_ref()?;
        (self.replica.latest()?.hud.as_ref()?.life == inventory.life.into()).then_some(inventory)
    }
    /// Admits inventory updates independently of rendering or gameplay rules.
    pub fn push_inventory(&mut self, response: &Response) -> Result<(), String> {
        let Reply::Inventory { inventory } = &response.body else {
            return Err("Remote inventory reply is missing".into());
        };
        inventory.validate(&response.control)?;
        let control = self
            .replica
            .control()
            .ok_or("Spectator cannot receive owned inventory")?;
        let received = response.control.as_ref().unwrap();
        if response.version != super::wire::VERSION
            || response.request_id == 0
            || response.instance != self.instance
            || inventory.life.instance != self.instance
            || inventory.life.actor != control.life.actor
            || response.tick < self.replica.tick().unwrap_or(0)
            || inventory.life.generation < control.life.generation
            || received.epoch < control.epoch
            || (received.epoch == control.epoch
                && (received.life != control.life
                    || received.accepted_sequence < control.accepted_sequence))
        {
            return Err("Remote inventory ownership or control fence is incompatible".into());
        }
        if let Some((tick, previous)) = &self.inventory {
            if response.tick < *tick
                || inventory.revision < previous.revision
                || inventory.life.generation < previous.life.generation
                || (inventory.revision == previous.revision
                    && (inventory.experience != previous.experience
                        || inventory.items != previous.items
                        || inventory.quests != previous.quests
                        || inventory.level != previous.level
                        || !same_quest_rewards(&inventory.quest_log, &previous.quest_log)
                        || inventory.catalog != previous.catalog
                        || inventory.outfits != previous.outfits
                        || inventory.outfit != previous.outfit
                        || inventory.equipment != previous.equipment
                        || inventory.equipped != previous.equipped))
            {
                return Err("Remote inventory revision or counters regressed".into());
            }
        }
        let mut generations = self.giver_generations.clone();
        let mut givers = std::collections::BTreeMap::new();
        for quest in &inventory.quest_log {
            if let Some(actor) = quest.giver {
                let state = (quest.giver_life, quest.interactable);
                if givers
                    .insert(actor, state)
                    .is_some_and(|previous| previous != state)
                {
                    return Err("Remote quests disagree on the current giver state".into());
                }
            }
        }
        for (_, (life, _)) in givers {
            if let Some(life) = life {
                let known = generations
                    .get(&life.actor)
                    .copied()
                    .into_iter()
                    .chain(snapshot_generation(self.replica.latest(), life.actor))
                    .max();
                if known.is_some_and(|generation| life.generation < generation) {
                    return Err("Remote quest giver generation regressed".into());
                }
                generations.insert(life.actor, life.generation);
            }
        }
        if generations.len() > 64 {
            return Err("Remote quest giver generation budget exceeded".into());
        }
        self.giver_generations = generations;
        self.inventory = Some((response.tick, inventory.clone()));
        self.retire_interaction();
        Ok(())
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
        for (actor, generation) in &mut self.giver_generations {
            if let Some(observed) = snapshot_generation(self.replica.latest(), *actor) {
                *generation = (*generation).max(observed);
            }
        }
        if self.target.is_some_and(|life| !self.targetable(life)) {
            self.target = None;
        }
        if reset {
            self.events.clear();
            self.handoff = None;
            self.reset_tick = response.tick;
            self.interaction = None;
        }
        self.retire_interaction();
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
                Kind::Ability { label }
                    if event.actor.is_none() || label.is_empty() || label.len() > 64 =>
                {
                    return Err("Invalid ability event".into());
                }
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
        self.retire_interaction();
        Ok(())
    }
    pub fn frame(&self, alpha: f32, camera: Camera) -> Result<Option<Frame>, String> {
        Ok(self.scene_sample(alpha, camera)?.map(|sample| sample.frame))
    }
    pub fn scene_sample(&self, alpha: f32, camera: Camera) -> Result<Option<SceneSample>, String> {
        self.scene_sample_predicted(alpha, camera, None)
    }
    /// Overrides only the current owned pose; resources and remote actors stay authoritative.
    pub fn scene_sample_predicted(
        &self,
        alpha: f32,
        camera: Camera,
        predicted: Option<crate::prediction::Pose>,
    ) -> Result<Option<SceneSample>, String> {
        camera.validate()?;
        let Some(mut presentation) = self.replica.sample(alpha)? else {
            return Ok(None);
        };
        let mut animation_owner = None;
        if let Some(predicted) = predicted {
            let control = self
                .replica
                .control()
                .ok_or("Predicted pose has no owned control")?;
            if predicted.life != control.life.into()
                || predicted.epoch != control.epoch
                || !predicted.position.is_finite()
                || predicted.position.abs().max_element() > 1_000_000.
                || !predicted.yaw.is_finite()
                || predicted
                    .axes
                    .iter()
                    .any(|a| !a.is_finite() || a.abs() > 1.)
                || !predicted.motion_time.is_finite()
                || predicted.motion_time < 0.
                || self
                    .replica
                    .latest()
                    .and_then(|s| s.hud.as_ref())
                    .is_none_or(|h| h.resources.hp <= 0)
            {
                return Err("Predicted pose does not match live owned control".into());
            }
            let actor = presentation
                .actors
                .iter_mut()
                .find(|a| a.life == control.life)
                .ok_or("Predicted presentation actor is missing")?;
            let delta = predicted.position - actor.actor.position;
            actor.actor.position = predicted.position;
            actor.actor.yaw = predicted.yaw;
            use verse_engine::motion::{Selection, State};
            if predicted.airborne && !actor.animation.casting() {
                actor.animation = State::Airborne.into();
                actor.animation_time = 0.2;
            } else if predicted.moving && predicted.axes.iter().any(|a| a.abs() > 0.00001) {
                actor.animation = if predicted.axes[0].abs() > predicted.axes[1].abs() {
                    if predicted.axes[0] < 0. {
                        State::StrafeLeft
                    } else {
                        State::StrafeRight
                    }
                } else if predicted.axes[1] < 0. {
                    State::Backpedal
                } else {
                    State::Run
                }
                .into();
                actor.animation_time = predicted.motion_time;
                animation_owner = Some((predicted.life, predicted.epoch));
            } else if predicted.axes == [0.; 2]
                && matches!(
                    actor.animation,
                    Selection::Named(
                        State::Run | State::Backpedal | State::StrafeLeft | State::StrafeRight
                    )
                )
            {
                actor.animation = State::CombatReady.into();
                actor.animation_time = presentation.time;
            }
            for effect in &mut presentation.effects {
                if effect.life == control.life {
                    effect.position = predicted.position.to_array();
                    effect.light = effect.light.map(|p| (Vec3::from(p) + delta).to_array());
                }
            }
        }
        let mut combat = self
            .replica
            .latest()
            .unwrap()
            .combat_visuals(self.instance)?;
        combat.time = presentation.time;
        for (player, effect) in combat.players.iter_mut().zip(&presentation.effects) {
            player.position = effect.position.into();
            player.light = effect.light.map(Into::into);
        }
        let actors: Vec<_> = presentation
            .actors
            .iter()
            .chain(&presentation.corpses)
            .cloned()
            .map(|p| ActorFrame {
                actor: p.actor,
                life: Some(p.life.into()),
                animation: p.animation,
                animation_time: p.animation_time,
                animation_epoch: animation_owner
                    .filter(|(life, _)| *life == p.life.into())
                    .map(|(_, epoch)| epoch),
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
            fov: camera.fov.to_radians(),
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
    #[cfg(feature = "service-net")]
    #[tokio::test]
    async fn authored_friendly_giver_enrolls_over_tls_and_fences_quest_chain() {
        use crate::service::{
            Chamber,
            auth::Gateway,
            client::Client,
            net,
            net::tests::{key, tls},
            progression::Config,
            wire::Body,
        };
        use rustls::pki_types::ServerName;
        use tokio::{net::TcpListener, sync::oneshot};
        let keys = [key(131), key(132)];
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual-quests.json"
        ))
        .unwrap();
        let config: Config = serde_json::from_slice(include_bytes!(
            "../../../../assets/verse/original/ritual-progression.json"
        ))
        .unwrap();
        let mut game = crate::play::Game::combat_in(scene, false, 120).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        game.encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)
            .unwrap();
        let mut gateway = Gateway::new(Chamber::new(game).unwrap())
            .unwrap()
            .with_progression(config)
            .unwrap()
            .with_rewards(
                serde_json::from_slice(include_bytes!(
                    "../../../../assets/verse/original/ritual-rewards.json"
                ))
                .unwrap(),
            )
            .unwrap();
        gateway
            .enroll_primary(keys[0].x_only_public_key().0.serialize())
            .unwrap();
        gateway
            .enroll_spectator(keys[1].x_only_public_key().0.serialize())
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve(listener, tls, gateway, async {
            let _ = stopping.await;
        }));
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let mut view = View::new(120, 12., 0).unwrap();
        view.push_snapshot(&client.request(Body::Snapshot {}).await.unwrap())
            .unwrap();
        view.push_inventory(&client.request(Body::Inventory {}).await.unwrap())
            .unwrap();
        let quest = &view.inventory().unwrap().quest_log[0];
        assert!(quest.interactable && quest.available && !quest.accepted);
        assert!(quest.dialogue_text().unwrap().contains("outer summoner"));
        let giver = quest.giver_life.unwrap();
        assert_eq!(giver.actor, 1_000_000);
        assert!(view.select_target(Some(giver)).is_err());
        view.open_giver(giver).unwrap();
        assert_eq!(view.interaction().unwrap().giver, giver);
        assert_eq!(view.interaction_quests().len(), 1);
        assert!(
            view.interaction_quests()[0]
                .dialogue_text()
                .unwrap()
                .contains("outer summoner")
        );
        let mut foreign = giver;
        foreign.instance += 1;
        assert!(view.open_giver(foreign).is_err());
        assert_eq!(view.interaction().unwrap().giver, giver);
        assert_eq!(
            view.quest_markers().get(&giver),
            Some(&crate::service::progression::Marker::Available)
        );
        let control = client.control().unwrap().clone();
        let accept = Body::AcceptQuest {
            life: control.life,
            epoch: control.epoch,
            quest: 101,
            giver: giver.into(),
        };
        let first = client.request(accept.clone()).await.unwrap();
        assert!(matches!(
            first.body,
            Reply::QuestAccepted {
                quest: 101,
                revision: 1
            }
        ));
        assert!(matches!(
            client.request(accept.clone()).await.unwrap().body,
            Reply::QuestAccepted {
                quest: 101,
                revision: 1
            }
        ));
        view.push_inventory(&client.request(Body::Inventory {}).await.unwrap())
            .unwrap();
        assert!(view.inventory().unwrap().quest_log[0].accepted);
        assert!(
            view.inventory().unwrap().quest_log[0]
                .dialogue_text()
                .unwrap()
                .contains("still anchors")
        );
        assert_eq!(
            view.quest_markers().get(&giver),
            Some(&crate::service::progression::Marker::Active)
        );
        assert!(!view.inventory().unwrap().quest_log[1].available);
        assert!(matches!(
            client
                .request(Body::AcceptQuest {
                    life: control.life,
                    epoch: control.epoch,
                    quest: 102,
                    giver: giver.into()
                })
                .await
                .unwrap()
                .body,
            Reply::Refused { .. }
        ));
        let mut observer = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[1],
        )
        .await
        .unwrap();
        assert!(matches!(
            observer.request(accept).await.unwrap().body,
            Reply::Refused { .. }
        ));
        view.inventory.as_mut().unwrap().1.quest_log[0]
            .giver_life
            .as_mut()
            .unwrap()
            .generation += 1;
        assert!(view.quest_markers().is_empty());
        assert!(view.interaction().is_none());
        view.inventory.as_mut().unwrap().1.quest_log[0].giver_life = Some(giver);
        view.close_giver();
        assert!(view.interaction().is_none());
        view.open_giver(giver).unwrap();
        view.inventory.as_mut().unwrap().1.quest_log[0].interactable = false;
        view.retire_interaction();
        view.inventory.as_mut().unwrap().1.quest_log[0].interactable = true;
        assert!(view.interaction().is_none());
        view.open_giver(giver).unwrap();
        view.events.push(Event {
            instance: 120,
            serial: 999,
            tick: 1,
            time: 1.,
            actor: Some(giver),
            kind: Kind::Death,
        });
        assert!(view.quest_markers().is_empty());
        assert!(view.interaction().is_none());
        // Use a fresh replica after the deliberately forged local projection checks.
        let mut view = View::new(120, 12., 0).unwrap();
        for (quest, target_actor, expected_xp) in [(101, 2, 75), (102, 3, 175)] {
            view.push_snapshot(&client.request(Body::Snapshot {}).await.unwrap())
                .unwrap();
            view.push_inventory(&client.request(Body::Inventory {}).await.unwrap())
                .unwrap();
            view.open_giver(giver).unwrap();
            if quest == 102 {
                assert!(matches!(
                    client.accept_quest(quest, giver).await.unwrap().body,
                    Reply::QuestAccepted { quest: 102, .. }
                ));
            }
            let target = view
                .replica()
                .latest()
                .unwrap()
                .presentation
                .actors
                .iter()
                .find(|a| a.life.actor == target_actor)
                .unwrap()
                .life
                .into();
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                for _ in 0..3 {
                    loop {
                        let state = client.snapshot().await.unwrap();
                        if state
                            .snapshot
                            .abilities
                            .iter()
                            .any(|a| a.id == crate::rules::Spell::MagicMissile && a.ready)
                        {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    let cast = client
                        .command(crate::Intent::Cast {
                            ability: crate::play::Ability::MagicMissile,
                            target: Some(target),
                            aim: [0., 0., 1.],
                        })
                        .await
                        .unwrap();
                    assert!(
                        matches!(cast.body, Reply::Accepted),
                        "Cast refused: {:?}",
                        cast
                    );
                    let until = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        view.push_snapshot(&client.request(Body::Snapshot {}).await.unwrap())
                            .unwrap();
                        view.push_inventory(&client.request(Body::Inventory {}).await.unwrap())
                            .unwrap();
                        if view.inventory().unwrap().quest_log.iter().any(|q| {
                            q.id == quest
                                && q.marker() == Some(crate::service::progression::Marker::TurnIn)
                        }) {
                            return;
                        }
                        if tokio::time::Instant::now() >= until {
                            break;
                        }
                    }
                }
                panic!("Authored quest objective was not defeated");
            })
            .await
            .unwrap();
            assert_eq!(
                view.quest_markers().get(&giver),
                Some(&crate::service::progression::Marker::TurnIn)
            );
            assert!(matches!(
                client.claim_quest(quest).await.unwrap().body,
                Reply::QuestClaimed { .. }
            ));
            assert!(matches!(
                client.claim_quest(quest).await.unwrap().body,
                Reply::QuestClaimed { .. }
            ));
            view.push_inventory(&client.request(Body::Inventory {}).await.unwrap())
                .unwrap();
            assert_eq!(view.inventory().unwrap().experience, expected_xp);
        }
        assert!(view.interaction().is_none());
        assert!(view.quest_markers().is_empty());
        if let Some(path) = std::env::var_os("VERSE_AUTHORED_QUEST_TLS_EVIDENCE") {
            std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"verse.authored-quest-tls.fixture.v1", "wire_version":crate::service::wire::VERSION,
                "transport":"authenticated loopback TLS", "scene":"ritual-quests.json",
                "quests":[101,102], "objectives":"actual admitted Magic Missile kills",
                "inventory":view.inventory().unwrap(), "duplicate_turn_in_xp":175,
                "spectator_acceptance_refused":true,
                "scope":"Authored network quest flow; not OS input, native scene capture, durable restart, or performance acceptance"
            })).unwrap()).unwrap();
        }
        drop(observer);
        drop(client);
        stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }

    #[cfg(feature = "service-net")]
    #[tokio::test]
    async fn tls_movement_updates_giver_availability_at_unchanged_reward_revision() {
        use crate::service::{
            client::Client,
            net,
            net::tests::{gateway_at, key, tls},
            progression::{Config, Quest},
            wire::Body,
        };
        use rustls::pki_types::ServerName;
        use tokio::{net::TcpListener, sync::oneshot};
        let keys = [key(121), key(122), key(123)];
        let g = gateway_at(&keys, Some(Vec3::new(-1., 0., -22.)))
            .with_progression(Config {
                version: 1,
                levels: vec![0],
                quests: vec![Quest {
                    repeatable: false,
                    dialogue: None,
                    giver: Some(2),
                    prerequisites: vec![],
                    id: 1,
                    name: "Disrupt the ritual".into(),
                    objective: 1,
                    goal: 2,
                    experience: 75,
                    items: vec![],
                }],
            })
            .unwrap();
        let start = g
            .game()
            .actor_position(g.game().player_life().actor)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (tls, connector) = tls();
        let (stop, stopping) = oneshot::channel();
        let server = tokio::spawn(net::serve(listener, tls, g, async {
            let _ = stopping.await;
        }));
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let mut view = View::new(120, 12., 0).unwrap();
        let snapshot = client.request(Body::Snapshot {}).await.unwrap();
        view.push_snapshot(&snapshot).unwrap();
        let before = client.request(Body::Inventory {}).await.unwrap();
        view.push_inventory(&before).unwrap();
        assert!(view.inventory().unwrap().quest_log[0].interactable);
        let revision = view.inventory().unwrap().revision;
        let outcome = client
            .command(crate::Intent::Cast {
                ability: crate::play::Ability::MistyStep,
                target: None,
                aim: [0., 0., 1.],
            })
            .await
            .unwrap();
        assert!(
            matches!(outcome.body, Reply::Accepted),
            "{:?}",
            outcome.body
        );
        let snapshot = client.request(Body::Snapshot {}).await.unwrap();
        view.push_snapshot(&snapshot).unwrap();
        let after = client.request(Body::Inventory {}).await.unwrap();
        let Reply::Inventory { inventory } = &after.body else {
            panic!("Expected inventory")
        };
        assert_eq!(inventory.revision, revision);
        assert!(!inventory.quest_log[0].interactable);
        view.push_inventory(&after).unwrap();
        assert!(!view.inventory().unwrap().quest_log[0].interactable);
        let end = view
            .replica()
            .latest()
            .unwrap()
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == inventory.life.actor)
            .unwrap()
            .actor
            .position;
        assert!(start.distance(end) > 4.);
        stop.send(()).unwrap();
        let exit = server.await.unwrap();
        assert!(exit.failure.is_none());
        println!(
            "VERSE_GIVER_VIEW {}",
            serde_json::json!({"schema":"verse.giver-view.fixture.v1",
            "before":before,"after":after,"start":start.to_array(),"end":end.to_array(),
            "same_ledger_revision":revision,"typed_movement":"Misty Step","view_admitted":true})
        );
    }
    #[test]
    fn quest_world_updates_keep_ledger_fields_and_generation_fences_atomic() {
        use super::super::{
            progression::{Level, Progress},
            wire::{Control, Inventory, Life},
        };
        let life = Life {
            instance: 130,
            actor: 14,
            generation: 0,
        };
        let mut snapshot = response(1);
        snapshot.control = Some(Control {
            credit_step: 0,
            world_step: 0,
            life,
            epoch: 1,
            accepted_sequence: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        });
        super::super::replica::tests::attach_hud(&mut snapshot);
        let mut view = View::new(130, 10., 0).unwrap();
        view.push_snapshot(&snapshot).unwrap();
        let giver = verse_engine::core::LifeId {
            instance: 130,
            actor: 2,
            generation: 0,
        };
        let data = Inventory {
            life,
            revision: 1,
            experience: 45,
            items: vec![],
            quests: vec![],
            level: Level {
                level: 1,
                start: 0,
                next: None,
            },
            quest_log: vec![Progress {
                cycle: 0,
                repeatable: false,
                dialogue: None,
                accepted: true,
                giver: Some(2),
                giver_life: Some(giver),
                interactable: false,
                available: true,
                id: 1,
                name: "Disrupt the ritual".into(),
                progress: 1,
                goal: 2,
                claimed: false,
                experience: 75,
                items: vec![],
            }],
            catalog: Default::default(),
            outfits: Default::default(),
            outfit: 0,
            equipment: Default::default(),
            equipped: Default::default(),
        };
        let mut reply = snapshot.clone();
        reply.request_id = 2;
        reply.body = Reply::Inventory { inventory: data };
        view.push_inventory(&reply).unwrap();
        reply.tick = 2;
        let Reply::Inventory { inventory } = &mut reply.body else {
            unreachable!()
        };
        inventory.quest_log[0].interactable = true;
        view.push_inventory(&reply).unwrap();
        assert!(view.inventory().unwrap().quest_log[0].interactable);
        for case in 0..10 {
            let mut bad = reply.clone();
            bad.tick = 3;
            let Reply::Inventory { inventory } = &mut bad.body else {
                unreachable!()
            };
            let quest = &mut inventory.quest_log[0];
            quest.giver_life = Some(verse_engine::core::LifeId {
                generation: 7,
                ..giver
            });
            match case {
                0 => quest.name = "Altered definition".into(),
                1 => quest.progress = 2,
                2 => {
                    quest.accepted = false;
                    quest.progress = 0;
                }
                3 => quest.available = false,
                4 => {
                    quest.claimed = true;
                    quest.progress = 2;
                }
                5 => quest.experience += 1,
                6 => quest.items = vec![super::super::rewards::Entry { id: 1, count: 1 }],
                7 => {
                    quest.giver = Some(3);
                    quest.giver_life.as_mut().unwrap().actor = 3;
                }
                8 => quest.id = 2,
                _ => quest.goal = 3,
            }
            assert!(view.push_inventory(&bad).is_err(), "case {case}");
            let Reply::Inventory { inventory } = &reply.body else {
                unreachable!()
            };
            assert_eq!(view.inventory(), Some(inventory));
        }
        for change_life in [true, false] {
            let mut mixed = reply.clone();
            mixed.tick = 3;
            let Reply::Inventory { inventory } = &mut mixed.body else {
                unreachable!()
            };
            inventory.revision = 2;
            let mut second = inventory.quest_log[0].clone();
            second.id = 2;
            if change_life {
                second.giver_life.as_mut().unwrap().generation = 7;
            } else {
                second.interactable = false;
            }
            inventory.quest_log.push(second);
            assert!(view.push_inventory(&mixed).is_err());
        }
        reply.tick = 3;
        let Reply::Inventory { inventory } = &mut reply.body else {
            unreachable!()
        };
        inventory.quest_log[0]
            .giver_life
            .as_mut()
            .unwrap()
            .generation = 1;
        view.push_inventory(&reply).unwrap();
        reply.tick = 4;
        let Reply::Inventory { inventory } = &mut reply.body else {
            unreachable!()
        };
        inventory.quest_log[0].giver_life = None;
        inventory.quest_log[0].interactable = false;
        view.push_inventory(&reply).unwrap();
        let retained = view.inventory().unwrap().clone();
        let mut stale = reply.clone();
        stale.tick = 5;
        let Reply::Inventory { inventory } = &mut stale.body else {
            unreachable!()
        };
        inventory.quest_log[0].giver_life = Some(giver);
        assert!(view.push_inventory(&stale).is_err());
        assert_eq!(view.inventory(), Some(&retained));
        let Reply::Inventory { inventory } = &mut stale.body else {
            unreachable!()
        };
        inventory.quest_log[0].giver_life = Some(verse_engine::core::LifeId {
            generation: 1,
            ..giver
        });
        view.push_inventory(&stale).unwrap();
        let mut next_snapshot = response(2);
        next_snapshot.control = snapshot.control;
        super::super::replica::tests::attach_hud(&mut next_snapshot);
        let mut value = serde_json::to_value(&next_snapshot).unwrap();
        fn advance_giver(value: &mut serde_json::Value) {
            if let Some(object) = value.as_object_mut() {
                if object.get("actor").and_then(|v| v.as_u64()) == Some(2)
                    && object.contains_key("generation")
                {
                    object.insert("generation".into(), 2.into());
                }
                for value in object.values_mut() {
                    advance_giver(value);
                }
            } else if let Some(array) = value.as_array_mut() {
                for value in array {
                    advance_giver(value);
                }
            }
        }
        advance_giver(&mut value);
        view.push_snapshot(&serde_json::from_value(value).unwrap())
            .unwrap();
        stale.tick = 6;
        assert!(view.push_inventory(&stale).is_err());
        let Reply::Inventory { inventory } = &mut stale.body else {
            unreachable!()
        };
        inventory.quest_log[0]
            .giver_life
            .as_mut()
            .unwrap()
            .generation = 2;
        view.push_inventory(&stale).unwrap();
    }
    #[test]
    fn inventory_admission_is_atomic_and_future_lives_wait_for_their_snapshot() {
        use super::super::{
            rewards::Entry,
            wire::{Control, Inventory, Life},
        };
        let life = Life {
            instance: 130,
            actor: 14,
            generation: 0,
        };
        let mut snapshot = response(1);
        snapshot.control = Some(Control {
            credit_step: 0,
            world_step: 0,
            life,
            epoch: 1,
            accepted_sequence: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        });
        super::super::replica::tests::attach_hud(&mut snapshot);
        let mut view = View::new(130, 10., 0).unwrap();
        view.push_snapshot(&snapshot).unwrap();
        let data = Inventory {
            life,
            revision: 1,
            experience: 45,
            items: vec![Entry { id: 1, count: 2 }],
            quests: vec![],
            level: super::super::progression::Level {
                level: 1,
                start: 0,
                next: None,
            },
            quest_log: vec![],
            catalog: Default::default(),
            outfits: Default::default(),
            outfit: 0,
            equipment: Default::default(),
            equipped: Default::default(),
        };
        let mut reply = snapshot.clone();
        reply.request_id = 2;
        reply.body = Reply::Inventory {
            inventory: data.clone(),
        };
        view.push_inventory(&reply).unwrap();
        assert_eq!(view.inventory(), Some(&data));
        for case in 0..7 {
            let mut bad = reply.clone();
            match case {
                0 => bad.instance = 131,
                1 => bad.tick = 0,
                2 => bad.request_id = 0,
                3 => bad.control.as_mut().unwrap().epoch = 0,
                4 => bad.control = None,
                5 => {
                    if let Reply::Inventory { inventory } = &mut bad.body {
                        inventory.experience += 1;
                    }
                }
                _ => {
                    if let Reply::Inventory { inventory } = &mut bad.body {
                        inventory.life.actor += 1;
                    }
                }
            }
            assert!(view.push_inventory(&bad).is_err());
            assert_eq!(view.inventory(), Some(&data));
        }
        let mut future = reply.clone();
        future.tick = 2;
        future.control.as_mut().unwrap().life.generation = 1;
        future.control.as_mut().unwrap().epoch = 2;
        if let Reply::Inventory { inventory } = &mut future.body {
            inventory.life.generation = 1;
        }
        view.push_inventory(&future).unwrap();
        assert!(view.inventory().is_none());
        let mut value = serde_json::to_value(&snapshot).unwrap();
        fn advance(value: &mut serde_json::Value) {
            if let Some(object) = value.as_object_mut() {
                if object.get("actor").and_then(|v| v.as_u64()) == Some(14)
                    && object.contains_key("generation")
                {
                    object.insert("generation".into(), 1.into());
                }
                for value in object.values_mut() {
                    advance(value);
                }
            } else if let Some(array) = value.as_array_mut() {
                for value in array {
                    advance(value);
                }
            }
        }
        advance(&mut value);
        value["tick"] = 2.into();
        value["control"]["epoch"] = 2.into();
        let next: Response = serde_json::from_value(value).unwrap();
        view.push_snapshot(&next).unwrap();
        assert_eq!(view.inventory().unwrap().life.generation, 1);
        assert_eq!(view.inventory().unwrap().experience, 45);
        assert!(view.push_inventory(&reply).is_err());
        let mut spectator = View::new(130, 10., 0).unwrap();
        spectator.push_snapshot(&response(1)).unwrap();
        assert!(spectator.push_inventory(&reply).is_err());
    }
    fn state(r: &mut Response) -> &mut State {
        let Reply::Snapshot { state } = &mut r.body else {
            panic!("Expected snapshot")
        };
        state
    }
    #[test]
    fn predicted_owned_pose_moves_attachments_without_mutating_remote_state() {
        use crate::service::wire::{Control, Life};
        let life = Life {
            instance: 130,
            actor: 14,
            generation: 0,
        };
        let mut snapshot = response(1);
        snapshot.control = Some(Control {
            credit_step: 0,
            world_step: 0,
            life,
            epoch: 1,
            accepted_sequence: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        });
        super::super::replica::tests::attach_hud(&mut snapshot);
        let mut view = View::new(130, 10., 0).unwrap();
        view.push_snapshot(&snapshot).unwrap();
        let original = view.scene_sample(1., camera()).unwrap().unwrap();
        let encoded = serde_json::to_vec(view.replica.latest().unwrap()).unwrap();
        let pose = crate::prediction::Pose {
            life: life.into(),
            epoch: 1,
            position: Vec3::new(2., 0., -22.),
            yaw: 1.,
            axes: [1., 0.],
            airborne: false,
            moving: true,
            motion_time: 0.1,
        };
        let predicted = view
            .scene_sample_predicted(1., camera(), Some(pose))
            .unwrap()
            .unwrap();
        for actor in &original.presentation.actors {
            let after = predicted
                .presentation
                .actors
                .iter()
                .find(|a| a.life == actor.life)
                .unwrap();
            if actor.life == life {
                assert_eq!(after.actor.position, pose.position);
                assert_eq!(after.actor.yaw, 1.);
                assert_eq!(
                    after.animation,
                    verse_engine::motion::State::StrafeRight.into()
                );
            } else {
                assert_eq!(
                    serde_json::to_vec(actor).unwrap(),
                    serde_json::to_vec(after).unwrap()
                );
            }
            assert_eq!(actor.health, after.health);
        }
        assert_eq!(
            serde_json::to_vec(view.replica.latest().unwrap()).unwrap(),
            encoded
        );
        assert!(
            original
                .frame
                .actors
                .iter()
                .all(|actor| actor.animation_epoch.is_none())
        );
        assert_eq!(
            predicted
                .frame
                .actors
                .iter()
                .find(|actor| actor.life == Some(life.into()))
                .unwrap()
                .animation_epoch,
            Some(pose.epoch)
        );
        assert!(
            predicted
                .frame
                .actors
                .iter()
                .filter(|actor| actor.life != Some(life.into()))
                .all(|actor| actor.animation_epoch.is_none())
        );
        assert_eq!(predicted.combat.players[0].position, pose.position);
        let mut foreign = pose;
        foreign.epoch += 1;
        assert!(
            view.scene_sample_predicted(1., camera(), Some(foreign))
                .is_err()
        );
        let mut invalid = pose;
        invalid.position.x = f32::NAN;
        assert!(
            view.scene_sample_predicted(1., camera(), Some(invalid))
                .is_err()
        );
        let mut spectator = View::new(130, 10., 0).unwrap();
        spectator.push_snapshot(&response(1)).unwrap();
        assert!(
            spectator
                .scene_sample_predicted(1., camera(), Some(pose))
                .is_err()
        );
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
    fn native_camera_degrees_project_into_a_radian_scene_frame() {
        let mut view = View::new(130, 10., 0).unwrap();
        view.push_snapshot(&response(1)).unwrap();
        let frame = view.frame(1., camera()).unwrap().unwrap();
        assert!((frame.fov - std::f32::consts::FRAC_PI_3).abs() < 0.00001);
        assert!(frame.view_projection(16. / 9.).is_finite());
    }
    #[test]
    fn retired_combat_actor_keeps_a_corpse_frame_and_requires_new_life_to_return() {
        let mut r = response(1);
        let s = state(&mut r);
        let index = s
            .presentation
            .actors
            .iter()
            .position(|p| p.actor.nameplate && p.actor.model != "adventurer")
            .unwrap();
        let mut corpse = s.presentation.actors.remove(index);
        let life = corpse.life;
        let source = s.actors.iter().find(|a| a.life == life).unwrap().source;
        s.actors.retain(|a| a.life != life);
        s.snapshot.actors.retain(|a| a.id != source);
        corpse.health = 0;
        corpse.actor.nameplate = false;
        corpse.animation = verse_engine::motion::State::Death.into();
        s.presentation.corpses.push(corpse.clone());
        let mut view = View::new(130, 10., 0).unwrap();
        view.push_snapshot(&r).unwrap();
        let frame = view.frame(1., camera()).unwrap().unwrap();
        let body = frame
            .actors
            .iter()
            .find(|a| a.life == Some(life.into()))
            .unwrap();
        assert_eq!(body.health, 0);
        assert!(!body.actor.nameplate);
        assert!(view.select_target(Some(life.into())).is_err());
        let mut expired = r.clone();
        expired.tick = 2;
        state(&mut expired).presentation.corpses.clear();
        view.push_snapshot(&expired).unwrap();
        let mut stale_corpse = r.clone();
        stale_corpse.tick = 3;
        assert!(view.push_snapshot(&stale_corpse).is_err());
        let mut revived = response(4);
        assert!(view.push_snapshot(&revived).is_err());
        let s = state(&mut revived);
        s.actors
            .iter_mut()
            .find(|a| a.life == life)
            .unwrap()
            .life
            .generation += 1;
        s.presentation
            .actors
            .iter_mut()
            .find(|p| p.life == life)
            .unwrap()
            .life
            .generation += 1;
        view.push_snapshot(&revived).unwrap();
        assert!(
            !view
                .frame(1., camera())
                .unwrap()
                .unwrap()
                .actors
                .iter()
                .any(|a| a.life == Some(life.into()))
        );
        assert!(view.push_snapshot(&r).is_err());
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
                Update::MovementSuperseded { .. }
                | Update::FrameBound { .. }
                | Update::CommandBound { .. }
                | Update::Outcome(_)
                | Update::Inventory(_)
                | Update::MovementCredit(_) => {
                    panic!("Spectator issued no private request")
                }
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
    fn friendly_givers_remain_visible_but_cannot_be_combat_targets() {
        let mut reply = response(10);
        let snapshot = state(&mut reply);
        let giver = snapshot
            .presentation
            .actors
            .iter_mut()
            .find(|p| p.actor.id == 2)
            .unwrap();
        giver.actor.friendly = true;
        let life: verse_engine::core::LifeId = giver.life.into();
        let source = snapshot
            .actors
            .iter()
            .find(|a| a.life == giver.life)
            .unwrap()
            .source;
        snapshot
            .snapshot
            .actors
            .iter_mut()
            .find(|a| a.id == source)
            .unwrap()
            .faction = "friendly".into();
        let mut view = View::new(130, 5., 0).unwrap();
        view.push_snapshot(&reply).unwrap();
        assert!(view.select_target(Some(life)).is_err());
        assert!(view.target().is_none());
        for _ in 0..32 {
            assert_ne!(view.cycle_target(), Some(life));
        }
        let frame = view.frame(1., camera()).unwrap().unwrap();
        assert!(
            frame
                .actors
                .iter()
                .any(|a| a.actor.id == life.actor && a.visible && a.health > 0 && a.actor.friendly)
        );
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
