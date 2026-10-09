//! Bounded JSON messages dispatched through a transport-retained connection.
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;

use super::auth::{Challenge, ConnectionId, Gateway};
use crate::{Command, Intent, events::Event, play::Ability, rules::Snapshot};

pub const VERSION: u16 = 34;
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Unsolicited opening message sent on the newly assigned transport connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub version: u16,
    pub challenge: Challenge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Life {
    pub instance: u64,
    pub actor: u64,
    pub generation: u64,
}
impl From<LifeId> for Life {
    fn from(life: LifeId) -> Self {
        Self {
            instance: life.instance,
            actor: life.actor,
            generation: life.generation,
        }
    }
}
impl From<Life> for LifeId {
    fn from(life: Life) -> Self {
        Self {
            instance: life.instance,
            actor: life.actor,
            generation: life.generation,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Jump {},
    Move {
        axes: [f32; 2],
        yaw: f32,
    },
    Cast {
        ability: Ability,
        target: Option<Life>,
        aim: [f32; 3],
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub actor: Life,
    pub epoch: u64,
    pub sequence: u64,
    pub tick: u64,
    pub intent: Action,
}
impl From<Command<Ability>> for Input {
    fn from(c: Command<Ability>) -> Self {
        let intent = match c.intent {
            Intent::Jump => Action::Jump {},
            Intent::Move { axes, yaw } => Action::Move { axes, yaw },
            Intent::Cast {
                ability,
                target,
                aim,
            } => Action::Cast {
                ability,
                target: target.map(Into::into),
                aim,
            },
        };
        Self {
            actor: c.actor.into(),
            epoch: c.epoch,
            sequence: c.sequence,
            tick: c.tick,
            intent,
        }
    }
}
impl From<Input> for Command<Ability> {
    fn from(c: Input) -> Self {
        let intent = match c.intent {
            Action::Jump {} => Intent::Jump,
            Action::Move { axes, yaw } => Intent::Move { axes, yaw },
            Action::Cast {
                ability,
                target,
                aim,
            } => Intent::Cast {
                ability,
                target: target.map(Into::into),
                aim,
            },
        };
        Self {
            actor: c.actor.into(),
            epoch: c.epoch,
            sequence: c.sequence,
            tick: c.tick,
            intent,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    Safety {},
    SafetyAction {
        realm: [u8; 32],
        operation: [u8; 16],
        action: super::safety::Action,
    },
    Services {
        character: u64,
    },
    ServiceAction {
        realm: [u8; 32],
        character: u64,
        operation: [u8; 16],
        action: super::game_services::Action,
    },
    Account {},
    SelectCharacter {
        character: u64,
    },
    Logout {
        life: Life,
        epoch: u64,
    },
    Authenticate {
        public_key: [u8; 32],
        signature: Vec<u8>,
    },
    Command {
        command: Input,
    },
    Social {
        input: crate::play::social::Input,
    },
    BeginMovementFrames {
        life: Life,
        epoch: u64,
    },
    MovementFrame {
        frame: crate::movement::frames::Frame,
    },
    MovementCredit {},
    Snapshot {},
    Replicate {
        ack: Option<super::replication::Baseline>,
    },
    Inventory {},
    UseItem {
        life: Life,
        epoch: u64,
        item: u64,
        operation: [u8; 16],
    },
    EquipGear {
        life: Life,
        epoch: u64,
        slot: super::equipment::Slot,
        item: u64,
        operation: [u8; 16],
    },
    EquipOutfit {
        life: Life,
        epoch: u64,
        outfit: u64,
        operation: [u8; 16],
    },
    QuestCycle {
        life: Life,
        epoch: u64,
        quest: u64,
        cycle: u64,
        action: super::progression::Action,
    },
    AcceptQuest {
        life: Life,
        epoch: u64,
        quest: u64,
        giver: Life,
    },
    ClaimQuest {
        life: Life,
        epoch: u64,
        quest: u64,
    },
    Events {
        after: u64,
        limit: u16,
    },
    Respawn {
        life: Life,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request_id: u64,
    pub body: Body,
}
impl State {
    pub fn validate_control(&self, instance: u64, control: &Option<Control>) -> Result<(), String> {
        self.validate(instance)?;
        if control
            .as_ref()
            .is_some_and(|c| c.credit_step < c.world_step)
        {
            return Err("Checkpoint credit precedes admitted body time".into());
        }
        if self.hud.as_ref().map(|h| h.life)
            != control
                .as_ref()
                .map(|c| verse_engine::core::LifeId::from(c.life))
        {
            return Err("Owned HUD does not match admitted control".into());
        }
        if self.collision.is_some() && control.is_none() {
            return Err("Collision snapshot has no admitted control".into());
        }
        if let Some(movement) = &self.movement {
            movement.validate()?;
            if self.collision.is_none() {
                return Err("Movement baseline has no collision snapshot".into());
            }
            let control = control
                .as_ref()
                .ok_or("Movement baseline has no admitted control")?;
            let actor = self
                .presentation
                .actors
                .iter()
                .find(|a| a.life == control.life)
                .ok_or("Movement baseline actor is missing")?;
            if self
                .scope
                .as_ref()
                .is_some_and(|s| s.center != actor.actor.position.to_array())
                || movement.life != control.life.into()
                || movement.world_step != control.world_step
                || movement.epoch != control.epoch
                || movement.applied_sequence > control.accepted_sequence
                || (movement.profile == crate::movement::Profile::Arrival
                    && movement.applied_sequence != control.accepted_sequence)
                || movement.character.feet.as_vec3() != actor.actor.position
                || self.hud.as_ref().is_none_or(|h| h.resources.hp <= 0)
            {
                return Err("Movement baseline does not match applied owned state".into());
            }
        }
        Ok(())
    }
    /// Produces renderer values only after complete remote state admission.
    pub fn combat_visuals(&self, instance: u64) -> Result<crate::visuals::Combat, String> {
        self.validate(instance)?;
        Ok(crate::visuals::Combat {
            time: self.presentation.time,
            flames: self.presentation.flames.clone(),
            projectiles: self.snapshot.projectiles.clone(),
            players: self
                .presentation
                .effects
                .iter()
                .map(|e| crate::visuals::Player {
                    position: e.position.into(),
                    shield: e.shield,
                    shield_until: e.shield_until,
                    light: e.light.map(Into::into),
                    areas: e.areas.clone(),
                })
                .collect(),
            hostile: self
                .presentation
                .hostile_casts
                .iter()
                .map(|c| crate::visuals::Hostile {
                    origin: c.origin.into(),
                    target: c.target.into(),
                    position: c.position.map(Into::into),
                    started: c.started,
                    release: c.release,
                    radius: c.radius,
                    boss: c.boss,
                })
                .collect(),
            impacts: self
                .presentation
                .impacts
                .iter()
                .map(|i| (i.position.into(), i.at, i.kind))
                .collect(),
        })
    }
    /// Admits the shared snapshot and its complete presentation life bindings.
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        if let Some(social) = &self.social {
            social.validate(instance)?;
            if self
                .presentation
                .actors
                .iter()
                .any(|a| a.actor.nameplate && !a.actor.friendly)
                || !self.snapshot.projectiles.is_empty()
            {
                return Err("Social snapshot contains combat actors or projectiles".into());
            }
        }
        if let Some(scope) = &self.scope {
            scope.validate()?;
        }
        if let Some(collision) = &self.collision {
            collision.validate(instance)?;
        }
        let mut sources = std::collections::BTreeSet::new();
        let mut lives = std::collections::BTreeSet::new();
        let snapshot_sources: std::collections::BTreeSet<_> =
            self.snapshot.actors.iter().map(|a| a.id).collect();
        if self.actors.len() != self.snapshot.actors.len()
            || self.actors.len() > 256
            || !self.snapshot.elapsed.is_finite()
            || self.snapshot.elapsed < 0.
            || self.actors.iter().any(|a| {
                a.life.instance != instance
                    || !sources.insert(a.source)
                    || !lives.insert(verse_engine::core::LifeId::from(a.life))
            })
            || self.snapshot.actors.iter().any(|a| {
                !sources.contains(&a.id)
                    || !a.pos.iter().all(|v| v.is_finite())
                    || !a.yaw.is_finite()
            })
            || sources != snapshot_sources
            || snapshot_sources.len() != self.snapshot.actors.len()
        {
            return Err("Invalid chamber snapshot life bindings".into());
        }
        let mut projectiles = std::collections::BTreeSet::new();
        if self.snapshot.projectiles.len() > 128
            || self.snapshot.projectiles.iter().any(|p| {
                !projectiles.insert(p.id)
                    || !sources.contains(&p.caster)
                    || !p
                        .pos
                        .iter()
                        .chain(&p.vel)
                        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            })
        {
            return Err("Invalid chamber projectile presentation".into());
        }
        if let Some(hud) = &self.hud {
            hud.validate(instance)?;
            let p = &self.snapshot.player;
            if (
                hud.resources.hp,
                hud.resources.max_hp,
                hud.resources.mana,
                hud.resources.max_mana,
            ) != (p.hp, p.max_hp, p.mana, p.max_mana)
                || hud
                    .casting
                    .as_ref()
                    .is_some_and(|c| !lives.contains(&c.target_life))
            {
                return Err("Owned HUD resources or cast target mismatch".into());
            }
            if hud.time != self.presentation.time
                || !self.presentation.actors.iter().any(|p| {
                    verse_engine::core::LifeId::from(p.life) == hud.life
                        && p.actor.model == "adventurer"
                })
            {
                return Err("Owned HUD life or clock mismatch".into());
            }
        }
        self.presentation.validate(instance, &self.actors)
    }
}

impl Request {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("Chamber request exceeds byte budget".into());
        }
        let r: Self = serde_json::from_slice(bytes).map_err(|_| "Malformed chamber request")?;
        if r.version != VERSION || r.request_id == 0 {
            return Err("Unsupported chamber version or request identity".into());
        }
        Ok(r)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    /// Physics time of the admitted response body.
    pub world_step: u64,
    /// Durable authority physics credit, independent of historical snapshot time.
    pub credit_step: u64,
    pub life: Life,
    pub epoch: u64,
    /// Highest admitted envelope, including subsequent gameplay refusals.
    pub accepted_sequence: u64,
    /// Actual applied movement from a completed durable fence. Admission alone
    /// never supplies this confirmation; it cannot exceed the envelope prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_movement: Option<crate::movement::Baseline>,
    /// Poses of loose props near the confirmed character at the same
    /// committed state, so the owner replays the confirmation against where
    /// they are now instead of its last scene snapshot (#10559).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic: Vec<ColliderPose>,
}
/// One collider's committed pose.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColliderPose {
    pub key: physics::queries::ColliderKey,
    pub pose: physics::queries::Pose,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBinding {
    pub source: u32,
    pub life: Life,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social: Option<crate::play::social::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<super::replication::Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision: Option<physics::queries::SceneSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<crate::movement::Baseline>,
    pub hud: Option<crate::hud::Own>,
    pub snapshot: Snapshot,
    pub presentation: super::presentation::Presentation,
    pub actors: Vec<ActorBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPage {
    pub events: Vec<Event>,
    pub next: u64,
    pub latest: u64,
    /// First retained serial; `None` when no authority events exist.
    pub oldest: Option<u64>,
    pub gap: bool,
}
/// Owned character state; clients cannot choose a different character to read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub life: Life,
    pub revision: u64,
    pub experience: u64,
    pub items: Vec<super::rewards::Entry>,
    pub quests: Vec<super::rewards::Entry>,
    pub level: super::progression::Level,
    pub quest_log: Vec<super::progression::Progress>,
    pub catalog: super::items::Catalog,
    pub outfits: super::outfits::Catalog,
    pub outfit: u64,
    pub equipment: super::equipment::Catalog,
    pub equipped: std::collections::BTreeMap<super::equipment::Slot, u64>,
}
impl Inventory {
    pub fn validate(&self, control: &Option<Control>) -> Result<(), String> {
        if control.as_ref().is_none_or(|c| c.life != self.life) {
            return Err("Inventory does not match admitted character life".into());
        }
        super::rewards::entries(&self.items)?;
        super::rewards::entries(&self.quests)?;
        self.level.validate(self.experience)?;
        self.equipment
            .validate_catalogs(&self.catalog, &self.outfits)?;
        self.equipment.limits(&super::rewards::Character {
            items: self.items.iter().map(|i| (i.id, i.count)).collect(),
            equipment: self.equipped.clone(),
            ..Default::default()
        })?;
        if self.outfit != 0 {
            self.outfits.outfit(self.outfit)?;
            if self.items.iter().all(|i| i.id != self.outfit) {
                return Err("Equipped outfit is not owned".into());
            }
        }
        super::progression::validate_progress(&self.quest_log)?;
        if self.quest_log.iter().any(|q| {
            q.giver_life
                .is_some_and(|g| g.instance != self.life.instance)
        }) {
            return Err("Quest giver belongs to a foreign instance".into());
        }
        if self.revision == 0
            && (self.experience != 0 || !self.items.is_empty() || !self.quests.is_empty())
        {
            return Err("Inventory has grants without a transaction revision".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Safety {
        view: super::safety::View,
    },
    SafetyApplied {
        receipt: super::safety::Receipt,
    },
    Services {
        view: super::game_services::View,
    },
    ServiceApplied {
        receipt: super::game_services::Receipt,
    },
    Account {
        account: super::accounts::Account,
    },
    CharacterSelected {
        character: u64,
    },
    LoggedOut {
        character: u64,
    },
    Accepted,
    GearEquipped {
        slot: super::equipment::Slot,
        item: u64,
        operation: [u8; 16],
        revision: u64,
    },
    OutfitEquipped {
        outfit: u64,
        operation: [u8; 16],
        revision: u64,
    },
    QuestAccepted {
        quest: u64,
        revision: u64,
    },
    QuestCycleChanged {
        quest: u64,
        cycle: u64,
        action: super::progression::Action,
        revision: u64,
    },
    QuestClaimed {
        quest: u64,
        revision: u64,
    },
    ItemUsed {
        item: u64,
        operation: [u8; 16],
        revision: u64,
    },
    Replicated {
        packet: super::replication::Packet,
    },
    Snapshot {
        state: State,
    },
    Events {
        page: EventPage,
    },
    Inventory {
        inventory: Inventory,
    },
    Refused {
        code: String,
        message: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u16,
    pub request_id: u64,
    pub instance: u64,
    pub tick: u64,
    pub control: Option<Control>,
    pub body: Reply,
}
impl Response {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(|_| "Cannot encode chamber response")?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("Chamber response exceeds byte budget".into());
        }
        Ok(bytes)
    }
    /// Reuses the exact packet bytes already encoded during delta selection.
    fn encode_replication(&self, packet: &[u8]) -> Result<Vec<u8>, String> {
        #[derive(Serialize)]
        struct Header<'a> {
            version: u16,
            request_id: u64,
            instance: u64,
            tick: u64,
            control: &'a Option<Control>,
        }
        let mut bytes = serde_json::to_vec(&Header {
            version: self.version,
            request_id: self.request_id,
            instance: self.instance,
            tick: self.tick,
            control: &self.control,
        })
        .map_err(|_| "Cannot encode chamber response")?;
        bytes.pop();
        bytes.extend_from_slice(b",\"body\":{\"type\":\"replicated\",\"packet\":");
        bytes.extend_from_slice(packet);
        bytes.extend_from_slice(b"}}");
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("Chamber response exceeds byte budget".into());
        }
        Ok(bytes)
    }
}
impl Gateway {
    /// Assigns the connection and encodes its public opening challenge.
    pub fn open_json(&mut self, now_ms: u64) -> Result<(ConnectionId, Vec<u8>), String> {
        let (id, challenge) = self.open(now_ms)?;
        match serde_json::to_vec(&Hello {
            version: VERSION,
            challenge,
        }) {
            Ok(bytes) => Ok((id, bytes)),
            Err(_) => {
                let _ = self.close(id);
                Err("Cannot encode chamber opening challenge".into())
            }
        }
    }
    /// Decodes one bounded message; identity comes only from the host connection.
    pub fn dispatch_json(
        &mut self,
        connection: ConnectionId,
        now_ms: u64,
        bytes: &[u8],
    ) -> Result<Vec<u8>, String> {
        let mut packet_bytes = None;
        let (request_id, body) = match Request::decode(bytes) {
            Ok(r) => {
                let body = match r.body {
                    Body::Replicate { ack } => {
                        self.replication_for(connection, ack)
                            .map(|(packet, encoded)| {
                                packet_bytes = Some(encoded);
                                Reply::Replicated { packet }
                            })
                    }
                    body => self.dispatch_body(connection, now_ms, body),
                };
                (r.request_id, body)
            }
            Err(message) => (0, Err(("protocol", message))),
        };
        let control = self.admission(connection).ok().map(|a| Control {
            credit_step: self.game().physics_steps,
            world_step: self.game().physics_steps,
            life: a.actor().into(),
            epoch: a.epoch(),
            accepted_sequence: a.accepted_sequence(),
            applied_movement: None,
            dynamic: Vec::new(),
        });
        let response = Response {
            version: VERSION,
            request_id,
            instance: self.game().player_life().instance,
            tick: self.game().authority_tick,
            control,
            body: body.unwrap_or_else(|(code, message)| Reply::Refused {
                code: code.into(),
                message,
            }),
        };
        match packet_bytes {
            Some(encoded) => response.encode_replication(&encoded),
            None => response.encode(),
        }
    }
    fn extract_shared(&self, id: ConnectionId) -> Result<State, (&'static str, String)> {
        let snapshot = self.snapshot(id).map_err(|e| ("authentication", e))?;
        let actors: Vec<_> = snapshot
            .actors
            .iter()
            .filter_map(|actor| {
                let life = self.game().projectile_caster_life(actor.id).or_else(|| {
                    self.game()
                        .ids
                        .iter()
                        .find(|(_, source)| **source == actor.id)
                        .and_then(|(id, _)| self.game().actor_life(*id))
                })?;
                Some(ActorBinding {
                    source: actor.id,
                    life: life.into(),
                })
            })
            .collect();
        let mut presentation = super::presentation::Presentation::extract(self.game(), &actors);
        for pose in &mut presentation.actors {
            if let Some(character) = self.character_rewards(pose.life.actor) {
                pose.equipment = character
                    .equipment
                    .values()
                    .map(|id| self.equipment().item(*id).cloned())
                    .collect::<Result<_, _>>()
                    .map_err(|e| ("equipment", e))?;
                if character.outfit != 0 {
                    pose.outfit_model = Some(
                        self.outfits()
                            .outfit(character.outfit)
                            .map_err(|e| ("outfit", e))?
                            .model
                            .clone(),
                    );
                }
            }
        }
        Ok(State {
            social: self.game().social_state().cloned(),
            scope: None,
            collision: Some(
                self.game()
                    .query_scene
                    .snapshot(self.game().player_life().instance)
                    .map_err(|e| ("collision", e))?,
            ),
            movement: self
                .admission(id)
                .ok()
                .map(|a| self.game().movement_baseline(a.actor()))
                .transpose()
                .map_err(|e| ("movement", e))?
                .flatten(),
            presentation,
            hud: self
                .admission(id)
                .ok()
                .map(|a| self.game().player_hud(a.actor()))
                .transpose()
                .map_err(|e| ("presentation", e))?,
            snapshot,
            actors,
        })
    }
    fn state_for(&mut self, id: ConnectionId) -> Result<State, (&'static str, String)> {
        self.check_view(id).map_err(|e| ("authentication", e))?;
        if self.view_cache.is_none() {
            let mut shared = self.extract_shared(id)?;
            shared.hud = None;
            shared.movement = None;
            shared.snapshot.player = crate::rules::Player {
                hp: 0,
                max_hp: 0,
                mana: 0,
                max_mana: 0,
            };
            shared.snapshot.abilities.clear();
            self.view_index = Some(super::replication::Index::new(&shared));
            self.view_cache = Some(shared);
        }
        let mut state = self.view_cache.as_ref().unwrap().clone();
        if let Ok(admission) = self.admission(id) {
            let private = self
                .game()
                .player_private_snapshot(admission.actor())
                .map_err(|e| ("presentation", e))?;
            state.snapshot.player = private.player;
            state.snapshot.abilities = private.abilities;
            state.hud = Some(
                self.game()
                    .player_hud(admission.actor())
                    .map_err(|e| ("presentation", e))?,
            );
            state.movement = self
                .game()
                .movement_baseline(admission.actor())
                .map_err(|e| ("movement", e))?;
        } else {
            state.collision = None;
        }
        Ok(state)
    }
    fn replication_for(
        &mut self,
        id: ConnectionId,
        ack: Option<super::replication::Baseline>,
    ) -> Result<(super::replication::Packet, Vec<u8>), (&'static str, String)> {
        let state = self.state_for(id)?;
        let control = self.admission(id).ok().map(|a| Control {
            credit_step: self.game().physics_steps,
            world_step: self.game().physics_steps,
            life: a.actor().into(),
            epoch: a.epoch(),
            accepted_sequence: a.accepted_sequence(),
            applied_movement: None,
            dynamic: Vec::new(),
        });
        let tick = self.game().authority_tick;
        let instance = self.game().player_life().instance;
        let sender = self.replication.entry(id).or_default();
        sender
            .project_encoded(
                state,
                &control,
                instance,
                tick,
                ack,
                self.view_index.as_ref().unwrap(),
            )
            .map_err(|e| ("replication", e))
    }
    fn dispatch_body(
        &mut self,
        id: ConnectionId,
        now: u64,
        body: Body,
    ) -> Result<Reply, (&'static str, String)> {
        match body {
            Body::Safety {}
            | Body::SafetyAction { .. }
            | Body::Services { .. }
            | Body::ServiceAction { .. }
            | Body::Account {}
            | Body::SelectCharacter { .. }
            | Body::Logout { .. } => Err((
                "realm_required",
                "Character lifecycle requires a realm".into(),
            )),
            Body::Authenticate {
                public_key,
                signature,
            } => {
                let Ok(signature) = <[u8; 64]>::try_from(signature) else {
                    // Consume a pending challenge even for a malformed proof length.
                    let _ = self.authenticate(id, now, public_key, [0; 64]);
                    return Err(("protocol", "Signature must contain exactly 64 bytes".into()));
                };
                self.authenticate(id, now, public_key, signature)
                    .map_err(|e| ("authentication", e))?;
                Ok(Reply::Accepted)
            }
            Body::Command { command } => {
                let admission = self.admission(id).map_err(|e| ("command", e))?;
                if admission.actor() == command.actor.into()
                    && admission.epoch() == command.epoch
                    && command.sequence > admission.accepted_sequence()
                    && self.game().authority_tick.saturating_sub(command.tick) > crate::COMMAND_AGE
                {
                    return Err((
                        "stale_tick",
                        "Command control snapshot expired before admission".into(),
                    ));
                }
                self.submit(id, command.into())
                    .map_err(|e| ("command", e))?;
                Ok(Reply::Accepted)
            }
            Body::Social { input } => {
                let admission = self.admission(id).map_err(|e| ("social", e))?;
                if admission.actor() == input.life
                    && admission.epoch() == input.epoch
                    && input.sequence > admission.accepted_sequence()
                    && self.game().authority_tick.saturating_sub(input.tick) > crate::COMMAND_AGE
                {
                    return Err((
                        "stale_tick",
                        "Social control snapshot expired before admission".into(),
                    ));
                }
                self.submit_social(id, input).map_err(|e| ("social", e))?;
                Ok(Reply::Accepted)
            }
            Body::BeginMovementFrames { life, epoch } => {
                self.begin_movement_frames(id, life.into(), epoch)
                    .map_err(|e| ("command", e))?;
                self.dispatch_body(id, now, Body::Snapshot {})
            }
            Body::MovementCredit {} => {
                self.admission(id).map_err(|e| ("movement_credit", e))?;
                Ok(Reply::Accepted)
            }
            Body::MovementFrame { frame } => {
                self.submit_movement_frame(id, frame)
                    .map_err(|e| ("command", e))?;
                Ok(Reply::Accepted)
            }
            Body::Respawn { life } => {
                self.respawn(id, life.into()).map_err(|e| ("command", e))?;
                Ok(Reply::Accepted)
            }
            Body::EquipGear {
                life,
                epoch,
                slot,
                item,
                operation,
            } => {
                let receipt = self
                    .equip_gear(id, life.into(), epoch, slot, item, operation)
                    .map_err(|e| ("equipment", e))?;
                Ok(Reply::GearEquipped {
                    slot,
                    item,
                    operation,
                    revision: receipt.revision,
                })
            }
            Body::EquipOutfit {
                life,
                epoch,
                outfit,
                operation,
            } => {
                let receipt = self
                    .equip_outfit(id, life.into(), epoch, outfit, operation)
                    .map_err(|e| ("outfit", e))?;
                Ok(Reply::OutfitEquipped {
                    outfit,
                    operation,
                    revision: receipt.revision,
                })
            }
            Body::UseItem {
                life,
                epoch,
                item,
                operation,
            } => {
                let receipt = self
                    .use_item(id, life.into(), epoch, item, operation)
                    .map_err(|e| ("item", e))?;
                Ok(Reply::ItemUsed {
                    item,
                    operation,
                    revision: receipt.revision,
                })
            }
            Body::QuestCycle {
                life,
                epoch,
                quest,
                cycle,
                action,
            } => {
                let receipt = self
                    .quest_cycle(id, life.into(), epoch, quest, cycle, action)
                    .map_err(|e| ("quest", e))?;
                Ok(Reply::QuestCycleChanged {
                    quest,
                    cycle,
                    action,
                    revision: receipt.revision,
                })
            }
            Body::AcceptQuest {
                life,
                epoch,
                quest,
                giver,
            } => {
                let receipt = self
                    .accept_quest(id, life.into(), epoch, quest, giver.into())
                    .map_err(|e| ("quest", e))?;
                Ok(Reply::QuestAccepted {
                    quest,
                    revision: receipt.revision,
                })
            }
            Body::ClaimQuest { life, epoch, quest } => {
                let receipt = self
                    .claim_quest(id, life.into(), epoch, quest)
                    .map_err(|e| ("quest", e))?;
                Ok(Reply::QuestClaimed {
                    quest,
                    revision: receipt.revision,
                })
            }
            Body::Snapshot {} => Ok(Reply::Snapshot {
                state: self.state_for(id)?,
            }),
            Body::Replicate { ack } => self
                .replication_for(id, ack)
                .map(|(packet, _)| Reply::Replicated { packet }),
            Body::Inventory {} => {
                let (life, revision, character) =
                    self.inventory(id).map_err(|e| ("authentication", e))?;
                let entries = |values: std::collections::BTreeMap<u64, u32>| {
                    values
                        .into_iter()
                        .map(|(id, count)| super::rewards::Entry { id, count })
                        .collect()
                };
                Ok(Reply::Inventory {
                    inventory: Inventory {
                        life: life.into(),
                        revision,
                        level: self
                            .progression()
                            .level(character.experience)
                            .map_err(|e| ("progression", e))?,
                        quest_log: self.quest_log(life.actor),
                        catalog: self.items().clone(),
                        outfits: self.outfits().clone(),
                        outfit: character.outfit,
                        equipment: self.equipment().clone(),
                        equipped: character.equipment.clone(),
                        experience: character.experience,
                        items: entries(character.items),
                        quests: entries(character.quests),
                    },
                })
            }
            Body::Events { after, limit } => {
                self.check_events(id).map_err(|e| ("authentication", e))?;
                if !(1..=64).contains(&limit) {
                    return Err(("cursor", "Event page limit must be between 1 and 64".into()));
                }
                let events = &self.game().events;
                let latest = events.last().map_or(0, |e| e.serial);
                if after > latest {
                    return Err(("cursor", "Event cursor is ahead of the authority".into()));
                }
                let oldest = events.first().map(|e| e.serial);
                let gap = oldest.is_some_and(|first| after.saturating_add(1) < first);
                let page: Vec<_> = events
                    .iter()
                    .filter(|e| e.serial > after)
                    .take(usize::from(limit))
                    .cloned()
                    .collect();
                let next = page.last().map_or(after, |e| e.serial);
                Ok(Reply::Events {
                    page: EventPage {
                        events: page,
                        next,
                        latest,
                        oldest,
                        gap,
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chamber;
    use super::*;
    use crate::play::Game;
    use glam::Vec3;
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use verse_engine::director::Scene;
    fn key(n: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn public(k: &Keypair) -> [u8; 32] {
        k.x_only_public_key().0.serialize()
    }
    fn gateway() -> Gateway {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut g = Game::combat_in(scene, false, 110).unwrap();
        g.time = g.scene.cut_at;
        g.tick(0., [0.; 2]).unwrap();
        Gateway::new(Chamber::new(g).unwrap()).unwrap()
    }
    fn send(g: &mut Gateway, id: ConnectionId, request_id: u64, body: Body) -> Response {
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id,
            body,
        })
        .unwrap();
        serde_json::from_slice(&g.dispatch_json(id, 0, &bytes).unwrap()).unwrap()
    }
    fn join(g: &mut Gateway, k: &Keypair) -> ConnectionId {
        let (id, bytes) = g.open_json(0).unwrap();
        let hello: Hello = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(hello.version, VERSION);
        let c = hello.challenge;
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&c.signing_digest(public(k)), k)
            .to_byte_array()
            .to_vec();
        assert!(matches!(
            send(
                g,
                id,
                1,
                Body::Authenticate {
                    public_key: public(k),
                    signature
                }
            )
            .body,
            Reply::Accepted
        ));
        id
    }
    #[test]
    fn preencoded_replication_preserves_full_delta_bytes_digests_and_response_limits() {
        let mut g = gateway();
        let player = key(130);
        g.enroll_primary(public(&player)).unwrap();
        let id = join(&mut g, &player);
        let mut receiver = super::super::replication::Receiver::default();
        let mut full = 0;
        let mut delta = 0;
        for request_id in 2..=5 {
            let request = serde_json::to_vec(&Request {
                version: VERSION,
                request_id,
                body: Body::Replicate {
                    ack: receiver.ack(),
                },
            })
            .unwrap();
            let encoded = g.dispatch_json(id, 0, &request).unwrap();
            let response: Response = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(encoded, response.encode().unwrap());
            let Reply::Replicated { ref packet } = response.body else {
                panic!("Expected replication")
            };
            match packet {
                super::super::replication::Packet::Full { .. } => full += 1,
                super::super::replication::Packet::Delta { .. } => delta += 1,
            }
            receiver
                .admit(packet, response.instance, response.tick, &response.control)
                .unwrap();
            assert!(receiver.ack().is_some());
            g.tick(1. / 30.).unwrap();
        }
        assert!(full > 0 && delta > 0);
        let response = Response {
            version: VERSION,
            request_id: 6,
            instance: 110,
            tick: 0,
            control: None,
            body: Reply::Accepted,
        };
        assert!(
            response
                .encode_replication(&vec![b' '; MAX_RESPONSE_BYTES])
                .is_err()
        );
    }

    #[test]
    fn movement_baselines_wait_for_applied_inputs_and_match_owned_control() {
        let mut g = gateway();
        let player = key(131);
        let other = key(132);
        let spectator = key(133);
        g.enroll_primary(public(&player)).unwrap();
        g.enroll_player(public(&other), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&spectator)).unwrap();
        let player = join(&mut g, &player);
        let other = join(&mut g, &other);
        let spectator = join(&mut g, &spectator);
        g.tick(1. / 30.).unwrap();
        for id in [player, other] {
            let initial = send(&mut g, id, 2, Body::Snapshot {});
            let Reply::Snapshot { state } = initial.body else {
                panic!("Expected snapshot");
            };
            state.validate_control(110, &initial.control).unwrap();
            let baseline = state.movement.unwrap();
            let geometry = state.collision.as_ref().unwrap();
            let reconstructed = geometry.compile(110).unwrap();
            assert_eq!(
                serde_json::to_vec(geometry).unwrap(),
                serde_json::to_vec(&g.game().query_scene.snapshot(110).unwrap()).unwrap()
            );
            let mut filter = physics::queries::Filter::blocking(110);
            filter.ignore = Some(physics::queries::Life {
                instance: 110,
                entity: baseline.life.actor,
                generation: baseline.life.generation,
            });
            let mut original = baseline.character;
            let mut rebuilt = baseline.character;
            crate::movement::advance(
                &mut original,
                &g.game().query_scene,
                filter,
                glam::DVec3::X * 6.4008,
                true,
                4,
                1. / 120.,
            )
            .unwrap();
            crate::movement::advance(
                &mut rebuilt,
                &reconstructed,
                filter,
                glam::DVec3::X * 6.4008,
                true,
                4,
                1. / 120.,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_vec(&original).unwrap(),
                serde_json::to_vec(&rebuilt).unwrap()
            );
            let mut missing = state.clone();
            missing.collision = None;
            assert!(missing.validate_control(110, &initial.control).is_err());
            let mut foreign = state.clone();
            foreign.collision.as_mut().unwrap().instance = 111;
            assert!(foreign.validate_control(110, &initial.control).is_err());
            assert_eq!(baseline.applied_sequence, 0);
            let command = g
                .admission(id)
                .unwrap()
                .command(
                    g.game().authority_tick,
                    crate::Intent::Move {
                        axes: [1., 0.],
                        yaw: 0.,
                    },
                )
                .unwrap();
            g.submit(id, command).unwrap();
            let command = g
                .admission(id)
                .unwrap()
                .command(g.game().authority_tick, crate::Intent::Jump)
                .unwrap();
            g.submit(id, command).unwrap();
            let pending = send(&mut g, id, 3, Body::Snapshot {});
            assert_eq!(pending.control.as_ref().unwrap().accepted_sequence, 2);
            let Reply::Snapshot { state } = pending.body else {
                panic!("Expected snapshot");
            };
            assert!(state.movement.is_none());
            g.tick(1. / 30.).unwrap();
            let applied = send(&mut g, id, 4, Body::Snapshot {});
            let Reply::Snapshot { state } = applied.body else {
                panic!("Expected snapshot");
            };
            state.validate_control(110, &applied.control).unwrap();
            let moved = state.movement.unwrap();
            assert_eq!(moved.applied_sequence, 2);
            assert!(moved.character.feet.x > baseline.character.feet.x);
            assert!(moved.character.feet.y > baseline.character.feet.y);
            for forgery in 0..3 {
                let mut forged = state.clone();
                let motor = forged.movement.as_mut().unwrap();
                match forgery {
                    0 => motor.epoch += 1,
                    1 => motor.applied_sequence += 1,
                    _ => motor.character.feet.x += 1.,
                };
                assert!(forged.validate_control(110, &applied.control).is_err());
            }
        }
        let observer = send(&mut g, spectator, 2, Body::Snapshot {});
        let Reply::Snapshot { state } = observer.body else {
            panic!("Expected snapshot");
        };
        assert!(state.movement.is_none());
        assert!(state.collision.is_none());
        state.validate_control(110, &observer.control).unwrap();
        let life = g.admission(player).unwrap().actor();
        let mut stale = life;
        stale.generation += 1;
        assert!(g.game().movement_baseline(stale).is_err());
    }
    #[test]
    fn signed_json_players_and_spectator_share_state_and_control_acknowledgments() {
        let mut g = gateway();
        let a = key(11);
        let b = key(12);
        let s = key(13);
        g.enroll_primary(public(&a)).unwrap();
        g.enroll_player(public(&b), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&s)).unwrap();
        let a = join(&mut g, &a);
        let b = join(&mut g, &b);
        let s = join(&mut g, &s);
        let command = g
            .admission(a)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        assert!(matches!(
            send(
                &mut g,
                b,
                2,
                Body::Command {
                    command: command.clone().into()
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(
                &mut g,
                s,
                2,
                Body::Command {
                    command: command.clone().into()
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let r = send(
            &mut g,
            a,
            3,
            Body::Command {
                command: command.clone().into(),
            },
        );
        assert!(matches!(r.body, Reply::Accepted));
        assert_eq!(r.control.unwrap().accepted_sequence, command.sequence);
        let duplicate = send(
            &mut g,
            a,
            4,
            Body::Command {
                command: command.into(),
            },
        );
        assert!(matches!(duplicate.body, Reply::Refused { .. }));
        assert_eq!(duplicate.control.unwrap().accepted_sequence, 1);
        g.tick(1. / 30.).unwrap();
        let Reply::Snapshot { state: player } = send(&mut g, a, 5, Body::Snapshot {}).body else {
            panic!()
        };
        let observer = send(&mut g, s, 5, Body::Snapshot {});
        assert!(observer.control.is_none());
        let Reply::Snapshot { state: spectator } = observer.body else {
            panic!()
        };
        assert_eq!(
            serde_json::to_vec(&player.snapshot.actors).unwrap(),
            serde_json::to_vec(&spectator.snapshot.actors).unwrap()
        );
        assert_eq!(player.actors.len(), player.snapshot.actors.len());
        assert!(player.actors.iter().all(|a| a.life.instance == 110));
        assert_eq!(
            player
                .actors
                .iter()
                .find(|a| a.source == 0)
                .unwrap()
                .life
                .actor,
            g.game().player_life().actor
        );
    }
    #[test]
    fn presentation_keeps_independent_movement_and_spell_visuals_on_one_tick() {
        let mut g = gateway();
        let ka = key(18);
        let kb = key(19);
        g.enroll_primary(public(&ka)).unwrap();
        g.enroll_player(public(&kb), Vec3::new(3., 0., -22.))
            .unwrap();
        let a = join(&mut g, &ka);
        let b = join(&mut g, &kb);
        for (id, ability) in [
            (a, Ability::Shield),
            (b, Ability::Shield),
            (a, Ability::Light),
        ] {
            let command = g
                .admission(id)
                .unwrap()
                .command(
                    g.game().authority_tick,
                    Intent::Cast {
                        ability,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            assert!(matches!(
                send(
                    &mut g,
                    id,
                    2,
                    Body::Command {
                        command: command.into()
                    }
                )
                .body,
                Reply::Accepted
            ));
        }
        let own = g.admission(b).unwrap().actor();
        let position = g.game().actor_position(own.actor).unwrap();
        let target = g
            .game()
            .frame()
            .actors
            .into_iter()
            .filter(|p| {
                p.actor.nameplate
                    && g.game()
                        .attack_clear(position + Vec3::Y * 1.4, p.actor.position + Vec3::Y * 1.1)
            })
            .min_by(|x, y| {
                x.actor
                    .position
                    .distance_squared(position)
                    .total_cmp(&y.actor.position.distance_squared(position))
            })
            .unwrap();
        let direction = Vec3::new(
            target.actor.position.x - position.x,
            0.,
            target.actor.position.z - position.z,
        )
        .normalize();
        let command = g
            .admission(b)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Web,
                    target: target.life,
                    aim: direction.to_array(),
                },
            )
            .unwrap();
        assert!(matches!(
            send(
                &mut g,
                b,
                3,
                Body::Command {
                    command: command.into()
                }
            )
            .body,
            Reply::Accepted
        ));
        for _ in 0..36 {
            g.tick(1. / 30.).unwrap();
        }
        for (id, axes) in [(a, [0., 1.]), (b, [0., -1.])] {
            let command = g
                .admission(id)
                .unwrap()
                .command(g.game().authority_tick, Intent::Move { axes, yaw: 0. })
                .unwrap();
            assert!(matches!(
                send(
                    &mut g,
                    id,
                    4,
                    Body::Command {
                        command: command.into()
                    }
                )
                .body,
                Reply::Accepted
            ));
        }
        g.tick(1. / 30.).unwrap();
        let Reply::Snapshot { state } = send(&mut g, a, 5, Body::Snapshot {}).body else {
            panic!()
        };
        state.presentation.validate(110, &state.actors).unwrap();
        assert_eq!(state.presentation.time, g.game().time);
        assert_eq!(
            state
                .presentation
                .effects
                .iter()
                .filter(|e| e.shield > 0)
                .count(),
            2
        );
        assert_eq!(
            state
                .presentation
                .effects
                .iter()
                .filter(|e| e.light.is_some())
                .count(),
            1
        );
        assert!(
            state
                .presentation
                .effects
                .iter()
                .find(|e| e.life.actor == own.actor)
                .unwrap()
                .areas
                .iter()
                .any(|e| e.kind == crate::utilities::Utility::Web)
        );
        let primary = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == g.game().player_life().actor)
            .unwrap();
        let extra = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == own.actor)
            .unwrap();
        assert_eq!(primary.animation, verse_engine::motion::State::Run.into());
        assert_eq!(
            extra.animation,
            verse_engine::motion::State::Backpedal.into()
        );
        assert!(primary.animation_time > 0. && extra.animation_time > 0.);
        let mut invalid = state.presentation.clone();
        invalid.actors[0].life.generation += 1;
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation.clone();
        invalid.effects.push(invalid.effects[0].clone());
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation.clone();
        invalid.actors[0].actor.position.x = f32::NAN;
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation;
        invalid.effects[0].areas.resize(
            129,
            crate::utilities::Area {
                kind: crate::utilities::Utility::Web,
                position: Vec3::ZERO,
                until: 20.,
            },
        );
        assert!(invalid.validate(110, &state.actors).is_err());
        let primary = g.admission(a).unwrap().actor();
        g.chamber.game.hostile_hit_player(primary, 1000).unwrap();
        g.view_cache = None; // This fixture bypasses the host mutation API.
        let Reply::Snapshot { state: dead } = send(&mut g, a, 6, Body::Snapshot {}).body else {
            panic!()
        };
        let corpse = dead
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == primary.actor)
            .unwrap();
        assert_eq!(corpse.health, 0);
        assert_eq!(corpse.animation, verse_engine::motion::State::Death.into());
    }

    #[test]
    fn admitted_remote_combat_visuals_match_local_extraction() {
        let mut g = gateway();
        let k = key(24);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        g.chamber.game.activate(Ability::Shield).unwrap();
        g.chamber.game.activate(Ability::Light).unwrap();
        let time = g.game().time;
        g.chamber.game.impacts.push((Vec3::Y, time, 1));
        let frame = g.game().frame();
        let caster = frame
            .actors
            .iter()
            .find(|p| p.actor.model != "adventurer")
            .unwrap();
        let target = frame
            .actors
            .iter()
            .find(|p| p.actor.model == "adventurer")
            .unwrap();
        g.chamber
            .game
            .encounter
            .as_mut()
            .unwrap()
            .casts
            .push(crate::combat::EnemyCast {
                actor: caster.actor.id,
                life: caster.life.unwrap(),
                target_life: target.life.unwrap(),
                position: Some(caster.actor.position),
                origin: caster.actor.position,
                target: target.actor.position,
                started: time,
                release: time + 1.,
                impact: time + 2.,
                damage: 8,
                radius: 1.6,
                boss: false,
            });
        let local = crate::visuals::Combat::extract(g.game());
        let r = send(&mut g, id, 2, Body::Snapshot {});
        let Reply::Snapshot { mut state } = r.body else {
            panic!("Expected snapshot")
        };
        let remote = state.combat_visuals(110).unwrap();
        assert_eq!(local.time, remote.time);
        assert_eq!(
            format!("{:?}", local.players),
            format!("{:?}", remote.players)
        );
        assert_eq!(
            format!("{:?}", local.hostile),
            format!("{:?}", remote.hostile)
        );
        assert_eq!(
            format!("{:?}", local.impacts),
            format!("{:?}", remote.impacts)
        );
        assert_eq!(
            format!("{:?}", local.projectiles),
            format!("{:?}", remote.projectiles)
        );
        let caster = state.actors[0].source;
        let projectile = crate::rules::Projectile {
            id: 1,
            caster,
            kind: crate::rules::ProjectileKind::Fireball,
            pos: [0.; 3],
            vel: [1., 0., 0.],
        };
        state.snapshot.projectiles.push(projectile.clone());
        assert!(state.combat_visuals(110).is_ok());
        for case in 0..4 {
            let mut bad = state.clone();
            match case {
                0 => bad.snapshot.projectiles[0].pos[0] = f32::NAN,
                1 => bad.snapshot.projectiles[0].caster = u32::MAX,
                2 => bad.snapshot.projectiles.push(projectile.clone()),
                _ => bad.snapshot.projectiles = vec![projectile.clone(); 129],
            }
            assert!(bad.combat_visuals(110).is_err());
        }
    }
    #[test]
    fn owned_snapshots_keep_authority_time_after_the_cinematic_ends() {
        for elapsed in [0., 1., 500.] {
            let mut g = gateway();
            let player = key(28);
            let time = g.game().scene.duration + elapsed;
            g.chamber.game.time = time;
            assert_eq!(g.game().scene.frame(time).time, g.game().scene.duration);
            g.enroll_primary(public(&player)).unwrap();
            let id = join(&mut g, &player);
            let response = send(&mut g, id, 2, Body::Snapshot {});
            let Reply::Snapshot { state } = response.body else {
                panic!("Expected snapshot");
            };
            state.validate_control(110, &response.control).unwrap();
            assert_eq!(state.presentation.time, time);
            assert_eq!(state.hud.unwrap().time, time);
        }
    }
    #[test]
    fn owned_hud_is_scoped_to_each_authenticated_life_and_spectators_have_none() {
        let mut g = gateway();
        let ka = key(25);
        let kb = key(26);
        let ks = key(27);
        g.enroll_primary(public(&ka)).unwrap();
        g.enroll_player(public(&kb), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&ks)).unwrap();
        let a = join(&mut g, &ka);
        let b = join(&mut g, &kb);
        let spectator = join(&mut g, &ks);
        let command = g
            .admission(a)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        g.submit(a, command).unwrap();
        let ra = send(&mut g, a, 2, Body::Snapshot {});
        let rb = send(&mut g, b, 2, Body::Snapshot {});
        let rs = send(&mut g, spectator, 2, Body::Snapshot {});
        let Reply::Snapshot { state: sa } = &ra.body else {
            panic!("Expected snapshot")
        };
        let Reply::Snapshot { state: sb } = &rb.body else {
            panic!("Expected snapshot")
        };
        let Reply::Snapshot { state: ss } = &rs.body else {
            panic!("Expected snapshot")
        };
        sa.validate_control(110, &ra.control).unwrap();
        sb.validate_control(110, &rb.control).unwrap();
        ss.validate_control(110, &rs.control).unwrap();
        let ha = sa.hud.as_ref().unwrap();
        let hb = sb.hud.as_ref().unwrap();
        assert_ne!(ha.life, hb.life);
        assert!(ss.hud.is_none());
        assert_eq!(ha.resources.mana, 19);
        assert_eq!(hb.resources.mana, 20);
        assert!(
            !ha.slots
                .iter()
                .find(|s| s.ability == Ability::Shield)
                .unwrap()
                .ready
        );
        assert!(
            hb.slots
                .iter()
                .find(|s| s.ability == Ability::Shield)
                .unwrap()
                .ready
        );
        assert!(sa.validate_control(110, &rb.control).is_err());
        for case in 0..6 {
            let mut bad = sa.clone();
            let h = bad.hud.as_mut().unwrap();
            match case {
                0 => h.life.generation += 1,
                1 => h.resources.hp = h.resources.max_hp + 1,
                2 => h.slots[0].remaining = f32::NAN,
                3 => h.slots.swap(0, 1),
                4 => h.resources.mana = 18,
                _ => h.time += 1.,
            }
            assert!(bad.validate_control(110, &ra.control).is_err());
        }
        assert!(
            g.game()
                .player_hud(verse_engine::core::LifeId {
                    generation: ha.life.generation + 1,
                    ..ha.life
                })
                .is_err()
        );
        let cast = g
            .admission(b)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Fireball,
                    target: Some(g.game().actor_life(1).unwrap()),
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        g.submit(b, cast).unwrap();
        let hud = g.game().player_hud(hb.life).unwrap();
        hud.validate(110).unwrap();
        assert!(hud.casting.is_some());
        assert!(hud.slots.iter().all(|s| !s.ready));
    }
    #[test]
    fn inventory_refuses_foreign_lives_unbounded_counts_and_client_selected_characters() {
        let life = Life {
            instance: 110,
            actor: 14,
            generation: 0,
        };
        let control = Some(Control {
            credit_step: 0,
            world_step: 0,
            life,
            epoch: 1,
            accepted_sequence: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        });
        let inventory = Inventory {
            life,
            revision: 1,
            experience: 45,
            items: vec![super::super::rewards::Entry { id: 1, count: 1 }],
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
        inventory.validate(&control).unwrap();
        for case in 0..5 {
            let mut bad = inventory.clone();
            match case {
                0 => bad.life.actor += 1,
                1 => bad.revision = 0,
                2 => bad.items[0].count = 1_000_001,
                3 => bad.items[0].count = 0,
                _ => bad.items.push(bad.items[0].clone()),
            }
            assert!(bad.validate(&control).is_err());
        }
        let mut long_lived = inventory.clone();
        long_lived.revision = 4097;
        long_lived.validate(&control).unwrap();
        assert!(inventory.validate(&None).is_err());
        let mut request = serde_json::to_value(Request {
            version: VERSION,
            request_id: 1,
            body: Body::Inventory {},
        })
        .unwrap();
        request["body"]["actor"] = 14.into();
        assert!(Request::decode(&serde_json::to_vec(&request).unwrap()).is_err());
    }
    #[test]
    fn strict_request_budget_versions_and_nested_fields_are_enforced() {
        let mut g = gateway();
        let (id, _) = g.open(0).unwrap();
        let reply = send(&mut g, id, 1, Body::Snapshot {});
        assert!(matches!(reply.body, Reply::Refused { .. }));
        for bytes in [
            serde_json::to_vec(&serde_json::json!({"version": VERSION + 1, "request_id": 1, "body": {"type": "snapshot"}})).unwrap(),
            serde_json::to_vec(&serde_json::json!({"version": VERSION, "request_id": 1, "controller": 1, "body": {"type": "snapshot"}})).unwrap(),
            serde_json::to_vec(&serde_json::json!({"version": VERSION, "request_id": 1, "body": {"type": "snapshot", "principal": "fake"}})).unwrap(),
            vec![b' '; MAX_REQUEST_BYTES + 1],
        ] {
            assert!(Request::decode(&bytes).is_err());
        }
        let k = key(14);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        let input: Input = g
            .admission(id)
            .unwrap()
            .command(g.game().authority_tick, Intent::Jump)
            .unwrap()
            .into();
        let mut value = serde_json::to_value(Request {
            version: VERSION,
            request_id: 3,
            body: Body::Command { command: input },
        })
        .unwrap();
        value["body"]["command"]["actor"]["forged"] = serde_json::json!(true);
        assert!(Request::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        value["body"]["command"]["actor"]
            .as_object_mut()
            .unwrap()
            .remove("forged");
        value["body"]["command"]["intent"]["controller"] = serde_json::json!(1);
        assert!(Request::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn oversized_responses_are_refused_instead_of_sent_without_a_bound() {
        let response = Response {
            version: VERSION,
            request_id: 1,
            instance: 110,
            tick: 0,
            control: None,
            body: Reply::Refused {
                code: "fixture".into(),
                message: "x".repeat(MAX_RESPONSE_BYTES),
            },
        };
        assert!(response.encode().is_err());
    }

    #[test]
    fn event_permissions_preserve_player_control_and_spectator_access() {
        let mut g = gateway();
        let owner = key(213);
        let observer = key(214);
        g.enroll_primary(public(&owner)).unwrap();
        g.enroll_spectator(public(&observer)).unwrap();
        let player = join(&mut g, &owner);
        let spectator = join(&mut g, &observer);
        let (anonymous, _) = g.open(0).unwrap();
        let events = || Body::Events {
            after: 0,
            limit: 64,
        };
        for id in [player, spectator] {
            assert!(matches!(
                send(&mut g, id, 2, events()).body,
                Reply::Events { .. }
            ));
        }
        assert!(
            matches!(send(&mut g, anonymous, 2, events()).body, Reply::Refused { code, .. } if code == "authentication")
        );
        let life = g.admission(player).unwrap().actor();
        g.chamber
            .game
            .handoff_player(life, crate::Controller(999))
            .unwrap();
        assert!(g.snapshot(player).is_err());
        assert!(
            matches!(send(&mut g, player, 3, events()).body, Reply::Refused { code, .. } if code == "authentication")
        );
        assert!(matches!(
            send(&mut g, spectator, 3, events()).body,
            Reply::Events { .. }
        ));
    }

    #[test]
    fn event_pages_report_retention_gaps_and_never_replay_past_cursor() {
        let mut g = gateway();
        let k = key(15);
        g.enroll_spectator(public(&k)).unwrap();
        let id = join(&mut g, &k);
        // Model the authority's retained suffix after bounded event eviction.
        let mut chamber = g.chamber.game.checkpoint().unwrap();
        let mut saved: serde_json::Value = serde_json::from_slice(&chamber).unwrap();
        let world = &mut saved["world"];
        world["events"] = serde_json::json!([
            {"instance":110,"serial":7,"tick":0,"time":0.,"actor":null,"kind":"CameraHandoff"},
            {"instance":110,"serial":8,"tick":0,"time":0.,"actor":null,"kind":"CameraHandoff"}
        ]);
        world["event_serial"] = serde_json::json!(8);
        chamber = serde_json::to_vec(&saved).unwrap();
        g.chamber.game = Game::restore(&chamber).unwrap();
        let Reply::Events { page } = send(&mut g, id, 2, Body::Events { after: 0, limit: 1 }).body
        else {
            panic!()
        };
        assert!(page.gap);
        assert_eq!(page.oldest, Some(7));
        assert_eq!(page.next, 7);
        assert_eq!(page.latest, 8);
        let Reply::Events { page } = send(
            &mut g,
            id,
            3,
            Body::Events {
                after: page.next,
                limit: 64,
            },
        )
        .body
        else {
            panic!()
        };
        assert!(!page.gap);
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.next, 8);
        assert!(matches!(
            send(
                &mut g,
                id,
                4,
                Body::Events {
                    after: 9,
                    limit: 64
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(&mut g, id, 5, Body::Events { after: 0, limit: 0 }).body,
            Reply::Refused { .. }
        ));
    }
    #[test]
    fn movement_credit_reads_preserve_admission_and_cannot_renew_interval_clocks() {
        let mut g = gateway();
        let owner = key(213);
        let observer = key(214);
        g.enroll_primary(public(&owner)).unwrap();
        g.enroll_spectator(public(&observer)).unwrap();
        let id = join(&mut g, &owner);
        let spectator = join(&mut g, &observer);
        assert!(matches!(
            send(&mut g, spectator, 2, Body::MovementCredit {}).body,
            Reply::Refused { .. }
        ));
        g.tick(1. / 30.).unwrap();
        let life = g.admission(id).unwrap().actor();
        let epoch = g.admission(id).unwrap().epoch();
        let entry = send(
            &mut g,
            id,
            2,
            Body::BeginMovementFrames {
                life: life.into(),
                epoch,
            },
        );
        assert!(matches!(entry.body, Reply::Snapshot { .. }));
        let epoch = g.admission(id).unwrap().epoch();
        let before = g.game().movement_baseline(life).unwrap().unwrap();
        let sequence = g.admission(id).unwrap().accepted_sequence();
        let credit = send(&mut g, id, 3, Body::MovementCredit {});
        assert!(matches!(credit.body, Reply::Accepted));
        assert_eq!(
            credit.control.as_ref().unwrap().credit_step,
            g.game().physics_steps
        );
        assert!(credit.encode().unwrap().len() < 4096);
        assert_eq!(g.admission(id).unwrap().accepted_sequence(), sequence);
        assert_eq!(g.game().movement_baseline(life).unwrap().unwrap(), before);
        // Repeated reads cannot keep an idle owned interval alive beyond its grace.
        for request in 4..=17 {
            g.tick(1. / 30.).unwrap();
            let credit = send(&mut g, id, request, Body::MovementCredit {});
            assert!(matches!(credit.body, Reply::Accepted));
            assert_eq!(credit.control.unwrap().credit_step, g.game().physics_steps);
        }
        assert!(g.admission(id).unwrap().epoch() > epoch);
        assert_eq!(g.game().movement_expiry.total, 1);
        assert_eq!(g.game().movement_expiry.bootstrap, 1);
    }

    #[test]
    fn owned_intervals_acknowledge_admission_before_body_time_and_refuse_foreign_controls() {
        use crate::movement::{
            Profile,
            frames::{Frame, Segment},
        };
        let mut g = gateway();
        let owner = key(211);
        let observer = key(212);
        g.enroll_primary(public(&owner)).unwrap();
        g.enroll_spectator(public(&observer)).unwrap();
        let id = join(&mut g, &owner);
        let spectator = join(&mut g, &observer);
        g.tick(1. / 30.).unwrap();
        let life = g.admission(id).unwrap().actor();
        let entry_epoch = g.admission(id).unwrap().epoch();
        assert!(matches!(
            send(
                &mut g,
                spectator,
                2,
                Body::BeginMovementFrames {
                    life: life.into(),
                    epoch: entry_epoch
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(
                &mut g,
                id,
                2,
                Body::BeginMovementFrames {
                    life: life.into(),
                    epoch: entry_epoch
                }
            )
            .body,
            Reply::Snapshot { .. }
        ));
        let a = g.admission(id).unwrap();
        let b = g.game().movement_baseline(life).unwrap().unwrap();
        let frame = Frame {
            life,
            epoch: a.epoch(),
            sequence: 1,
            tick: g.game().authority_tick,
            start: b.physics_step,
            steps: 4,
            segments: vec![Segment {
                offset: 0,
                axes: [1., 0.],
                yaw: 0.,
                until: b.physics_step + 60,
                jump: false,
            }],
        };
        assert!(matches!(
            send(
                &mut g,
                spectator,
                3,
                Body::MovementFrame {
                    frame: frame.clone()
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(
                &mut g,
                id,
                3,
                Body::MovementFrame {
                    frame: frame.clone()
                }
            )
            .body,
            Reply::Accepted
        ));
        let response = send(&mut g, id, 4, Body::Snapshot {});
        let Reply::Snapshot { state } = response.body else {
            panic!()
        };
        state.validate_control(110, &response.control).unwrap();
        assert_eq!(state.movement.unwrap().profile, Profile::Frames);
        assert_eq!(state.movement.unwrap().applied_sequence, 0);
        assert_eq!(response.control.unwrap().accepted_sequence, 1);
        g.tick(1. / 30.).unwrap();
        let response = send(&mut g, id, 5, Body::Snapshot {});
        let Reply::Snapshot { state } = response.body else {
            panic!()
        };
        state.validate_control(110, &response.control).unwrap();
        assert_eq!(state.movement.unwrap().physics_step, frame.end().unwrap());
        assert_eq!(state.movement.unwrap().applied_sequence, 1);
    }
    #[test]
    fn expired_interval_refusal_exposes_the_new_epoch_and_allows_fresh_entry() {
        use crate::movement::{
            Profile,
            frames::{Frame, Segment},
        };
        fn proposal(g: &Gateway, id: ConnectionId) -> Frame {
            let admission = g.admission(id).unwrap();
            let life = admission.actor();
            let baseline = g.game().movement_baseline(life).unwrap().unwrap();
            Frame {
                life,
                epoch: admission.epoch(),
                sequence: admission.accepted_sequence() + 1,
                tick: g.game().authority_tick,
                start: baseline.physics_step,
                steps: 4,
                segments: vec![Segment {
                    offset: 0,
                    axes: [1., 0.],
                    yaw: 0.,
                    until: baseline.physics_step + 60,
                    jump: false,
                }],
            }
        }
        let mut g = gateway();
        let owner = key(225);
        g.enroll_primary(public(&owner)).unwrap();
        let id = join(&mut g, &owner);
        g.tick(1. / 30.).unwrap();
        let life = g.admission(id).unwrap().actor();
        let epoch = g.admission(id).unwrap().epoch();
        assert!(matches!(
            send(
                &mut g,
                id,
                2,
                Body::BeginMovementFrames {
                    life: life.into(),
                    epoch,
                }
            )
            .body,
            Reply::Snapshot { .. }
        ));
        for _ in 0..9 {
            g.tick(1. / 30.).unwrap();
        }
        let first = proposal(&g, id);
        assert!(matches!(
            send(&mut g, id, 3, Body::MovementFrame { frame: first }).body,
            Reply::Accepted
        ));
        g.tick(1. / 30.).unwrap();
        let delayed = proposal(&g, id);
        let epoch = delayed.epoch;
        let response = send(
            &mut g,
            id,
            4,
            Body::MovementFrame {
                frame: delayed.clone(),
            },
        );
        assert!(matches!(response.body, Reply::Refused {message, ..}
            if message.contains("Movement interval clock expired")));
        let control = response.control.unwrap();
        assert_eq!(control.epoch, epoch + 1);
        assert_eq!(control.accepted_sequence, 0);
        let response = send(&mut g, id, 5, Body::Snapshot {});
        let Reply::Snapshot { state } = response.body else {
            panic!("Missing recovery snapshot");
        };
        state.validate_control(110, &response.control).unwrap();
        assert_eq!(state.movement.unwrap().profile, Profile::Arrival);
        let response = send(&mut g, id, 6, Body::MovementFrame { frame: delayed });
        assert!(matches!(response.body, Reply::Refused { .. }));
        assert_eq!(response.control.unwrap().epoch, control.epoch);
        assert!(matches!(
            send(
                &mut g,
                id,
                7,
                Body::BeginMovementFrames {
                    life: life.into(),
                    epoch: control.epoch,
                }
            )
            .body,
            Reply::Snapshot { .. }
        ));
        let fresh = proposal(&g, id);
        assert!(matches!(
            send(&mut g, id, 8, Body::MovementFrame { frame: fresh }).body,
            Reply::Accepted
        ));
    }
    #[test]
    fn stale_tick_refusal_is_explicit_and_does_not_consume_or_apply_the_command() {
        let mut g = gateway();
        let k = key(224);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        let command = g
            .admission(id)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        for _ in 0..7 {
            g.tick(1. / 30.).unwrap();
        }
        let before = g.game().checkpoint().unwrap();
        let reply = send(
            &mut g,
            id,
            2,
            Body::Command {
                command: command.clone().into(),
            },
        );
        assert!(matches!(reply.body,Reply::Refused{code,..} if code=="stale_tick"));
        assert_eq!(reply.control.unwrap().accepted_sequence, 0);
        assert_eq!(g.game().checkpoint().unwrap(), before);
        let mut future = command;
        future.tick = g.game().authority_tick + 1;
        assert!(
            matches!(send(&mut g,id,3,Body::Command{command:future.into()}).body,Reply::Refused{code,..} if code=="command")
        );
        assert_eq!(g.game().checkpoint().unwrap(), before);
    }
    #[test]
    fn gameplay_refusal_reports_consumed_sequence_for_client_reconciliation() {
        let mut g = gateway();
        let k = key(17);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        for expected in 1..=2 {
            let command = g
                .admission(id)
                .unwrap()
                .command(
                    g.game().authority_tick,
                    Intent::Cast {
                        ability: Ability::Shield,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            let response = send(
                &mut g,
                id,
                expected + 1,
                Body::Command {
                    command: command.into(),
                },
            );
            assert_eq!(response.control.unwrap().accepted_sequence, expected);
            if expected == 1 {
                assert!(matches!(response.body, Reply::Accepted));
            } else {
                assert!(matches!(response.body, Reply::Refused { .. }));
            }
        }
    }

    #[test]
    fn malformed_signature_consumes_challenge_and_respawn_is_owned() {
        let mut g = gateway();
        let k = key(16);
        g.enroll_primary(public(&k)).unwrap();
        let (id, c) = g.open(0).unwrap();
        assert!(matches!(
            send(
                &mut g,
                id,
                1,
                Body::Authenticate {
                    public_key: public(&k),
                    signature: vec![0; 63]
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&c.signing_digest(public(&k)), &k)
            .to_byte_array()
            .to_vec();
        assert!(matches!(
            send(
                &mut g,
                id,
                2,
                Body::Authenticate {
                    public_key: public(&k),
                    signature
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let id = join(&mut g, &k);
        let life = g.admission(id).unwrap().actor();
        g.chamber.game.hostile_hit_player(life, 1000).unwrap();
        let response = send(&mut g, id, 3, Body::Respawn { life: life.into() });
        assert!(matches!(response.body, Reply::Accepted));
        assert_eq!(
            response.control.unwrap().life.generation,
            life.generation + 1
        );
        assert!(matches!(
            send(&mut g, id, 4, Body::Respawn { life: life.into() }).body,
            Reply::Refused { .. }
        ));
    }
}
