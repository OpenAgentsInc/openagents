//! Hosted chamber rights after a trusted transport verifies a principal.
//!
//! Principals and session handles are adapter values, not bearer credentials.
//! Never deserialize a principal from an untrusted command and call it verified.
#[cfg(feature = "service-auth")]
pub mod accounts;
#[cfg(feature = "service-auth")]
pub mod auth;
#[cfg(feature = "service-client")]
pub mod client;
#[cfg(feature = "service-client")]
pub mod client_runtime;
pub mod equipment;
#[cfg(feature = "service-auth")]
pub mod event_cursor;
pub mod game_services;
#[cfg(feature = "service-net")]
pub mod host;
pub mod items;
#[cfg(feature = "service-net")]
pub mod net;
#[cfg(feature = "service-net")]
pub mod operator;
pub mod outfits;
#[cfg(feature = "service-net")]
pub mod persistence;
#[cfg(feature = "service-auth")]
pub mod presentation;
pub mod progression;
#[cfg(feature = "service-reach")]
pub mod reach;
#[cfg(feature = "reach-client")]
pub mod reach_client;
#[cfg(feature = "service-net")]
pub mod realm;
#[cfg(feature = "service-auth")]
pub mod replica;
#[cfg(feature = "service-auth")]
pub mod replication;
pub mod rewards;
#[cfg(feature = "service-auth")]
pub mod save;
#[cfg(feature = "service-client")]
pub mod transport;
#[cfg(feature = "service-auth")]
pub mod view;
#[cfg(feature = "service-auth")]
pub mod wire;
#[cfg(feature = "service-client")]
pub mod worker;

use std::collections::BTreeMap;

use glam::Vec3;
use verse_engine::core::LifeId;

use crate::{
    Admission, Command, Controller,
    play::{Ability, Game},
    rules::Snapshot,
};

/// Identity supplied by a trusted authentication adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Principal(pub [u8; 32]);

/// Connection binding retained by the trusted adapter, not a secret token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Session {
    instance: u64,
    serial: u64,
}

#[derive(Clone, Copy)]
enum Rights {
    Player(u64),
    Spectator,
}

#[derive(Clone, Copy)]
struct Connection {
    principal: Principal,
    rights: Rights,
    controller: Controller,
}

/// Owns one authoritative game and its explicitly enrolled principals.
///
/// Enrollment, ticking, reset, and revocation are host operations. Network
/// dispatch uses only the principal verified for that connection and its handle.
#[derive(Clone)]
pub struct Chamber {
    game: Game,
    grants: BTreeMap<Principal, Rights>,
    owners: BTreeMap<Principal, u64>,
    connections: BTreeMap<u64, Connection>,
    next_session: u64,
    rewards: rewards::Ledger,
    reward_policy: Vec<rewards::Policy>,
    reward_cursor: u64,
    progression: progression::Config,
    items: items::Catalog,
    outfits: outfits::Catalog,
    equipment: equipment::Catalog,
}

impl Chamber {
    pub fn new(mut game: Game) -> Result<Self, String> {
        let lives: Vec<_> = game.controlled_effects().map(|(life, _, _)| life).collect();
        for life in lives {
            game.handoff_player(life, Controller(0))?;
        }
        Ok(Self {
            game,
            grants: BTreeMap::new(),
            owners: BTreeMap::new(),
            connections: BTreeMap::new(),
            next_session: 1,
            rewards: rewards::Ledger::default(),
            reward_policy: Vec::new(),
            reward_cursor: 0,
            progression: Default::default(),
            items: Default::default(),
            outfits: Default::default(),
            equipment: Default::default(),
        })
    }

    /// Read-only authority for host presentation and checkpoint extraction.
    pub fn game(&self) -> &Game {
        &self.game
    }

    /// Host-only grants; the host must commit its checkpoint before acknowledging.
    pub fn grant_reward(
        &mut self,
        transaction: rewards::Transaction,
    ) -> Result<rewards::Receipt, String> {
        if transaction.instance != self.game.player_life().instance
            || self.game.player_admission(transaction.actor).is_none()
        {
            return Err("Reward character or instance is foreign".into());
        }
        if progression::reserved(&transaction.source)
            || items::reserved(&transaction.source)
            || outfits::reserved(&transaction.source)
            || equipment::reserved(&transaction.source)
            || game_services::trade_source(&transaction.source)
            || transaction.acceptance.is_some()
            || transaction.equipment.is_some()
            || transaction.outfit.is_some()
            || !transaction.spent.is_empty()
        {
            return Err("Campaign claim source is reserved".into());
        }
        self.apply_progression(transaction)
    }
    fn restore_reward(
        &mut self,
        transaction: rewards::Transaction,
    ) -> Result<rewards::Receipt, String> {
        if transaction.instance != self.game.player_life().instance
            || self.game.player_admission(transaction.actor).is_none()
        {
            return Err("Saved reward character or instance is foreign".into());
        }
        if progression::cycle_source(&transaction.source) {
            self.progression
                .validate_cycle(&transaction, &self.rewards)?;
        } else if progression::acceptance_source(&transaction.source) {
            self.progression
                .validate_acceptance(&transaction, &self.rewards)?;
        } else if transaction.acceptance.is_some() {
            return Err("Saved acceptance source is not an admitted quest enrollment".into());
        } else if progression::reserved(&transaction.source) {
            self.progression
                .validate_claim(&transaction, &self.rewards)?;
        }
        if game_services::trade_source(&transaction.source) {
            game_services::validate_trade(&transaction)?;
        } else if items::reserved(&transaction.source) {
            self.items.validate_use(&transaction)?;
        } else if !transaction.spent.is_empty() {
            return Err("Saved debit source is not an admitted item use".into());
        }
        if outfits::reserved(&transaction.source) {
            self.outfits.validate_change(&transaction)?;
        } else if transaction.outfit.is_some() {
            return Err("Saved outfit source is not an admitted change".into());
        }
        if equipment::reserved(&transaction.source) {
            self.equipment.validate_change(&transaction)?;
        } else if transaction.equipment.is_some() {
            return Err("Saved equipment source is not an admitted change".into());
        }
        self.rewards.apply(transaction)
    }
    pub fn equip_gear(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        slot: equipment::Slot,
        item: u64,
        operation: [u8; 16],
    ) -> Result<rewards::Receipt, String> {
        let admission = self.admission(principal, session)?;
        if admission.actor() != life || admission.epoch() != epoch {
            return Err("Equipment life or control is stale or foreign".into());
        }
        let tx = equipment::transaction(life.instance, life.actor, slot, item, operation)?;
        self.equipment.validate_change(&tx)?;
        if self.rewards.contains(life.actor, tx.source)? {
            return self.rewards.apply(tx);
        }
        if self.game.player_snapshot(life)?.player.hp == 0 {
            return Err("Cannot change equipment while defeated".into());
        }
        let mut next = self.rewards.clone();
        let receipt = next.apply(tx)?;
        let (hp, mana) = self.resource_limits(life.actor, next.character(life.actor).unwrap())?;
        self.game.equipment_limits(life.actor, hp, mana)?;
        self.rewards = next;
        Ok(receipt)
    }
    pub fn equip_outfit(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        outfit: u64,
        operation: [u8; 16],
    ) -> Result<rewards::Receipt, String> {
        let admission = self.admission(principal, session)?;
        if admission.actor() != life || admission.epoch() != epoch {
            return Err("Outfit life or control is stale or foreign".into());
        }
        if outfit != 0 {
            self.outfits.outfit(outfit)?;
        }
        let tx = outfits::transaction(life.instance, life.actor, outfit, operation)?;
        self.rewards.apply(tx)
    }
    pub fn use_item(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        item: u64,
        operation: [u8; 16],
    ) -> Result<rewards::Receipt, String> {
        let admission = self.admission(principal, session)?;
        if admission.actor() != life || admission.epoch() != epoch {
            return Err("Item use life or control is stale or foreign".into());
        }
        let definition = self.items.item(item)?;
        let tx = items::transaction(life.instance, life.actor, item, operation)?;
        if self.rewards.contains(life.actor, tx.source)? {
            return self.rewards.apply(tx);
        }
        let mut next = self.rewards.clone();
        let receipt = next.apply(tx)?;
        self.game
            .recover_player_resources(life.actor, definition.health, definition.mana)?;
        self.rewards = next;
        Ok(receipt)
    }
    fn validate_givers(&self, config: &progression::Config) -> Result<(), String> {
        config.validate()?;
        for quest in &config.quests {
            if quest.giver.is_some_and(|actor| {
                self.game.player_admission(actor).is_some()
                    || !self.game.scene.actors.iter().any(|a| a.id == actor)
            }) {
                return Err("Quest giver must be an authored NPC".into());
            }
        }
        Ok(())
    }
    fn quest_interaction(&self, life: LifeId, giver: LifeId) -> Result<(), String> {
        if self.game.actor_life(giver.actor) != Some(giver)
            || self.game.player_admission(giver.actor).is_some()
            || life.instance != giver.instance
            || self.game.player_snapshot(life)?.player.hp == 0
        {
            return Err("Quest interaction life is stale, foreign, or defeated".into());
        }
        let source = self
            .game
            .ids
            .get(&giver.actor)
            .ok_or("Quest giver is not an NPC")?;
        if !self
            .game
            .snapshot()
            .actors
            .iter()
            .any(|a| a.id == *source && a.alive && a.hp > 0)
        {
            return Err("Quest giver is defeated or unavailable".into());
        }
        let player = self
            .game
            .actor_position(life.actor)
            .ok_or("Quest adventurer is unavailable")?;
        let npc = self
            .game
            .actor_position(giver.actor)
            .ok_or("Quest giver is unavailable")?;
        if player.distance_squared(npc) > 16.
            || !self
                .game
                .attack_clear(player + Vec3::Y * 1.4, npc + Vec3::Y * 1.4)
        {
            return Err("Approach the quest giver with a clear line of sight".into());
        }
        Ok(())
    }
    fn quest_log(&self, actor: u64) -> Vec<progression::Progress> {
        let instance = self.game.player_life().instance;
        let mut progress = self.progression.progress(actor, instance, &self.rewards);
        for quest in &mut progress {
            if let Some(giver) = quest.giver {
                quest.giver_life = self.game.actor_life(giver);
                quest.interactable = self
                    .game
                    .actor_life(actor)
                    .zip(quest.giver_life)
                    .is_some_and(|(life, npc)| self.quest_interaction(life, npc).is_ok());
            }
        }
        progress
    }
    pub fn accept_quest(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        quest: u64,
        giver: LifeId,
    ) -> Result<rewards::Receipt, String> {
        self.quest_cycle(
            principal,
            session,
            life,
            epoch,
            quest,
            0,
            progression::Action::Accept { giver },
        )
    }
    pub fn claim_quest(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        quest: u64,
    ) -> Result<rewards::Receipt, String> {
        self.quest_cycle(
            principal,
            session,
            life,
            epoch,
            quest,
            0,
            progression::Action::Claim,
        )
    }
    pub fn quest_cycle(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
        quest: u64,
        cycle: u64,
        action: progression::Action,
    ) -> Result<rewards::Receipt, String> {
        let admission = self.admission(principal, session)?;
        if admission.actor() != life || admission.epoch() != epoch {
            return Err("Quest life or control is stale or foreign".into());
        }
        let quest = self
            .progression
            .quests
            .iter()
            .find(|q| q.id == quest)
            .cloned()
            .ok_or("Quest is not defined")?;
        let baseline = quest.count(self.rewards.character(life.actor));
        let tx = match action {
            progression::Action::Accept { giver } => {
                if quest.giver != Some(giver.actor) || giver.instance != life.instance {
                    return Err("Quest giver does not match its authored definition".into());
                }
                quest.acceptance_at(life.instance, life.actor, baseline, cycle)
            }
            progression::Action::Claim => quest.transaction_at(life.instance, life.actor, cycle),
            progression::Action::Abandon | progression::Action::Reset => quest.cycle_transaction(
                life.instance,
                life.actor,
                cycle,
                baseline,
                if matches!(action, progression::Action::Abandon) {
                    progression::Transition::Abandon
                } else {
                    progression::Transition::Reset
                },
            ),
        };
        if let Some(receipt) = self.rewards.receipt(life.actor, tx.source)? {
            if receipt.transaction.acceptance.map(|a| a.transition)
                != tx.acceptance.map(|a| a.transition)
            {
                return Err("Quest cycle already binds a different transition".into());
            }
            return Ok(receipt);
        }
        match action {
            progression::Action::Accept { giver } => {
                self.quest_interaction(life, giver)?;
                self.progression.validate_acceptance(&tx, &self.rewards)?;
            }
            progression::Action::Claim => {
                self.progression.validate_claim(&tx, &self.rewards)?;
                if let Some(giver) = quest.giver {
                    self.quest_interaction(
                        life,
                        self.game
                            .actor_life(giver)
                            .ok_or("Quest giver is unavailable")?,
                    )?;
                }
            }
            _ => self.progression.validate_cycle(&tx, &self.rewards)?,
        }
        self.apply_progression(tx)
    }
    fn resource_limits(
        &self,
        actor: u64,
        character: &rewards::Character,
    ) -> Result<(i32, i32), String> {
        let life = self
            .game
            .actor_life(actor)
            .ok_or("Character is unavailable")?;
        let definition = &self
            .game
            .actor_state(life)
            .ok_or("Character class is unavailable")?
            .definition;
        let level = self.progression.level(character.experience)?.level;
        self.equipment
            .derived_limits(character, definition.health, definition.mana, level)
    }
    fn apply_progression(&mut self, tx: rewards::Transaction) -> Result<rewards::Receipt, String> {
        let actor = tx.actor;
        let mut next = self.rewards.clone();
        let receipt = next.apply(tx)?;
        let (hp, mana) = self.resource_limits(actor, next.character(actor).unwrap())?;
        self.game.equipment_limits(actor, hp, mana)?;
        self.rewards = next;
        Ok(receipt)
    }

    pub fn character_rewards(&self, actor: u64) -> Option<&rewards::Character> {
        self.rewards.character(actor)
    }

    fn configure_rewards(&mut self, policies: Vec<rewards::Policy>) -> Result<(), String> {
        rewards::Policy::validate(&policies)?;
        for policy in &policies {
            if self.game.actor_life(policy.target).is_none()
                || self.game.player_admission(policy.target).is_some()
            {
                return Err("Combat reward target must be an authored NPC".into());
            }
        }
        self.reward_cursor = if policies.is_empty() {
            0
        } else {
            self.game.events.last().map_or(0, |e| e.serial)
        };
        self.reward_policy = policies;
        Ok(())
    }
    fn process_rewards(&mut self) -> Result<(), String> {
        if self.reward_policy.is_empty() {
            return Ok(());
        }
        let latest = self.game.events.last().map_or(0, |e| e.serial);
        if self.reward_cursor > latest
            || self
                .game
                .events
                .first()
                .is_some_and(|e| self.reward_cursor.saturating_add(1) < e.serial)
        {
            return Err("Combat reward cursor lost authoritative events".into());
        }
        if latest == self.reward_cursor {
            return Ok(());
        }
        let mut transactions = Vec::new();
        for event in self
            .game
            .events
            .iter()
            .filter(|e| e.serial > self.reward_cursor)
        {
            if !matches!(event.kind, crate::events::Kind::Death) {
                continue;
            }
            let Some(life) = event.actor else {
                continue;
            };
            if let Some(policy) = self.reward_policy.iter().find(|p| p.target == life.actor) {
                for (principal, rights) in &self.grants {
                    if let Rights::Player(actor) = rights {
                        let connected = self.connections.values().any(|c| {
                            c.principal == *principal
                                && matches!(c.rights, Rights::Player(a) if a == *actor)
                        });
                        let eligible = match policy.participation {
                            rewards::Participation::EnrolledResidents => true,
                            rewards::Participation::Connected => connected,
                            rewards::Participation::ConnectedWithin { radius } => {
                                connected
                                    && self
                                        .game
                                        .actor_position(*actor)
                                        .zip(self.game.actor_position(life.actor))
                                        .is_some_and(|(a, b)| {
                                            a.distance_squared(b) <= f32::from(radius).powi(2)
                                        })
                            }
                        };
                        if eligible {
                            transactions.push(policy.transaction(*actor, life));
                        }
                    }
                }
            }
        }
        let actors = transactions
            .iter()
            .map(|tx| tx.actor)
            .collect::<std::collections::BTreeSet<_>>();
        let mut next = self.rewards.clone();
        next.batch(transactions)?;
        let limits = actors
            .into_iter()
            .map(|actor| {
                self.resource_limits(actor, next.character(actor).unwrap())
                    .map(|limits| (actor, limits))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (actor, (hp, mana)) in limits {
            self.game.equipment_limits(actor, hp, mana)?;
        }
        self.rewards = next;
        self.reward_cursor = latest;
        Ok(())
    }
    pub fn inventory(
        &self,
        principal: Principal,
        session: Session,
    ) -> Result<(LifeId, u64, rewards::Character), String> {
        let (_, life) = self.player(principal, session)?;
        Ok((
            life,
            self.rewards.character_revision(life.actor),
            self.rewards
                .character(life.actor)
                .cloned()
                .unwrap_or_default(),
        ))
    }

    fn room_for_grant(&self, principal: Principal) -> Result<(), String> {
        if self.grants.contains_key(&principal) {
            return Err("Principal already enrolled".into());
        }
        if self.grants.len() >= 128 {
            return Err("Chamber principal budget exceeded".into());
        }
        Ok(())
    }

    /// Gives an enrolled principal the existing primary adventurer.
    pub fn enroll_primary(&mut self, principal: Principal) -> Result<(), String> {
        if self
            .game
            .player_admission(self.game.player_actor())
            .is_none()
        {
            return Err("Primary scene character is absent".into());
        }
        self.room_for_grant(principal)?;
        let actor = self.game.player_life().actor;
        if self
            .owners
            .iter()
            .any(|(owner, owned)| *owned == actor && *owner != principal)
            || self
                .owners
                .get(&principal)
                .is_some_and(|owned| *owned != actor)
        {
            return Err("Primary adventurer already owned".into());
        }
        self.grants.insert(principal, Rights::Player(actor));
        self.owners.insert(principal, actor);
        Ok(())
    }

    /// Adds an adventurer at a spawn selected and collision-checked by the host.
    pub fn enroll_player(&mut self, principal: Principal, spawn: Vec3) -> Result<LifeId, String> {
        self.room_for_grant(principal)?;
        if let Some(actor) = self.owners.get(&principal) {
            if *actor == self.game.player_life().actor
                || self.game.player_spawn(*actor) != Some(spawn)
            {
                return Err("Owned adventurer role or spawn is incompatible".into());
            }
            let life = self
                .game
                .player_admission(*actor)
                .ok_or("Owned adventurer is missing")?
                .actor();
            self.grants.insert(principal, Rights::Player(*actor));
            return Ok(life);
        }
        let life = self.game.add_player(Controller(0), spawn)?;
        self.grants.insert(principal, Rights::Player(life.actor));
        self.owners.insert(principal, life.actor);
        Ok(life)
    }

    pub fn enroll_spectator(&mut self, principal: Principal) -> Result<(), String> {
        self.room_for_grant(principal)?;
        self.grants.insert(principal, Rights::Spectator);
        Ok(())
    }

    /// Replaces the principal's previous connection and fences queued commands.
    pub fn connect(&mut self, principal: Principal) -> Result<Session, String> {
        let rights = *self
            .grants
            .get(&principal)
            .ok_or("Principal is not enrolled")?;
        let serial = self.next_session;
        let next = serial
            .checked_add(1)
            .ok_or("Session identities exhausted")?;
        let controller = Controller(
            serial
                .checked_add(3)
                .ok_or("Controller identities exhausted")?,
        );
        if let Rights::Player(actor) = rights {
            let life = self
                .game
                .player_admission(actor)
                .ok_or("Owned adventurer is missing")?
                .actor();
            self.game.handoff_player(life, controller)?;
        }
        self.connections.retain(|_, c| c.principal != principal);
        self.connections.insert(
            serial,
            Connection {
                principal,
                rights,
                controller,
            },
        );
        self.next_session = next;
        Ok(Session {
            instance: self.game.player_life().instance,
            serial,
        })
    }

    fn connection(&self, principal: Principal, session: Session) -> Result<Connection, String> {
        if session.instance != self.game.player_life().instance {
            return Err("Foreign chamber session".into());
        }
        let c = *self
            .connections
            .get(&session.serial)
            .ok_or("Session is no longer admitted")?;
        if c.principal != principal {
            return Err("Session principal mismatch".into());
        }
        Ok(c)
    }

    fn player(
        &self,
        principal: Principal,
        session: Session,
    ) -> Result<(Controller, LifeId), String> {
        let c = self.connection(principal, session)?;
        let Rights::Player(actor) = c.rights else {
            return Err("Spectators cannot control adventurers".into());
        };
        let admission = self
            .game
            .player_admission(actor)
            .ok_or("Owned adventurer is missing")?;
        if admission.controller() != c.controller {
            return Err("Session control has been replaced".into());
        }
        Ok((c.controller, admission.actor()))
    }

    pub fn admission(&self, principal: Principal, session: Session) -> Result<Admission, String> {
        let (_, life) = self.player(principal, session)?;
        Ok(self.game.player_admission(life.actor).unwrap().clone())
    }

    /// Derives the controller from the admitted connection, never command data.
    pub fn submit(
        &mut self,
        principal: Principal,
        session: Session,
        command: Command<Ability>,
    ) -> Result<(), String> {
        let (controller, life) = self.player(principal, session)?;
        if command.actor != life {
            return Err("Command does not name the session's current adventurer".into());
        }
        self.game.submit(controller, command)
    }

    pub fn submit_social(
        &mut self,
        principal: Principal,
        session: Session,
        input: crate::play::social::Input,
    ) -> Result<(), String> {
        let (controller, life) = self.player(principal, session)?;
        if life != input.life {
            return Err("Social command does not name the current character".into());
        }
        self.game.submit_social(controller, input)
    }

    pub fn begin_movement_frames(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
        epoch: u64,
    ) -> Result<(), String> {
        let (controller, current) = self.player(principal, session)?;
        if life != current || self.game.player_admission(life.actor).unwrap().epoch() != epoch {
            return Err("Movement interval control is stale or foreign".into());
        }
        self.game.begin_movement_frames(controller, life)
    }
    pub fn submit_movement_frame(
        &mut self,
        principal: Principal,
        session: Session,
        frame: crate::movement::frames::Frame,
    ) -> Result<(), String> {
        let (controller, life) = self.player(principal, session)?;
        if frame.life != life {
            return Err("Movement interval does not name the session's character".into());
        }
        self.game.submit_movement_frame(controller, frame)
    }

    pub fn snapshot(&self, principal: Principal, session: Session) -> Result<Snapshot, String> {
        match self.connection(principal, session)?.rights {
            Rights::Spectator => Ok(self.game.snapshot()),
            Rights::Player(_) => {
                let (_, life) = self.player(principal, session)?;
                self.game.player_snapshot(life)
            }
        }
    }

    pub fn respawn(
        &mut self,
        principal: Principal,
        session: Session,
        life: LifeId,
    ) -> Result<LifeId, String> {
        let (controller, current) = self.player(principal, session)?;
        if life != current {
            return Err("Respawn life is stale or foreign".into());
        }
        self.game.respawn_controlled_player(controller, life)
    }

    /// Disconnect leaves the actor in the world and drops pending movement/jump.
    pub fn disconnect(&mut self, principal: Principal, session: Session) -> Result<(), String> {
        let c = self.connection(principal, session)?;
        if let Rights::Player(_) = c.rights {
            let (_, life) = self.player(principal, session)?;
            self.game.handoff_player(life, Controller(0))?;
        }
        self.connections.remove(&session.serial);
        Ok(())
    }

    pub fn revoke(&mut self, principal: Principal) -> Result<(), String> {
        let rights = *self
            .grants
            .get(&principal)
            .ok_or("Principal is not enrolled")?;
        if let Rights::Player(actor) = rights {
            let life = self
                .game
                .player_admission(actor)
                .ok_or("Owned adventurer is missing")?
                .actor();
            self.game.handoff_player(life, Controller(0))?;
        }
        self.connections.retain(|_, c| c.principal != principal);
        self.grants.remove(&principal);
        Ok(())
    }

    /// Advances the shared world once, independent of session count.
    pub fn tick(&mut self, dt: f32) -> Result<(), String> {
        self.game.tick(dt, [0.; 2])?;
        self.process_rewards()
    }

    /// Resets this instance and fences commands while retaining enrolled rights.
    pub fn reset(&mut self) -> Result<(), String> {
        self.game.restart_combat(false)?;
        let primary = self.game.player_life();
        let controller = self
            .connections
            .values()
            .find_map(|c| match c.rights {
                Rights::Player(a) if a == primary.actor => Some(c.controller),
                _ => None,
            })
            .unwrap_or(Controller(0));
        if self.game.player_admission(primary.actor).is_some() {
            self.game.handoff_player(primary, controller)?;
        }
        for actor in self
            .game
            .controlled_effects()
            .map(|(life, _, _)| life.actor)
            .collect::<Vec<_>>()
        {
            let character = self.rewards.character(actor).cloned().unwrap_or_default();
            let (hp, mana) = self.resource_limits(actor, &character)?;
            self.game.equipment_limits(actor, hp, mana)?;
            let definition = &self
                .game
                .actor_state(self.game.actor_life(actor).unwrap())
                .unwrap()
                .definition;
            let extra_health = (hp - definition.health) as u32;
            let extra_mana = (mana - definition.mana) as u32;
            if extra_health != 0 || extra_mana != 0 {
                self.game
                    .recover_player_resources(actor, extra_health, extra_mana)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Intent;
    use verse_engine::director::Scene;
    const A: Principal = Principal([1; 32]);
    const B: Principal = Principal([2; 32]);
    const S: Principal = Principal([3; 32]);

    fn chamber(instance: u64) -> Chamber {
        let scene =
            Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json")).unwrap();
        let mut game = Game::combat_in(scene, false, instance).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        game.encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)
            .unwrap();
        Chamber::new(game).unwrap()
    }
    fn movement(c: &Chamber, principal: Principal, session: Session) -> Command<Ability> {
        c.admission(principal, session)
            .unwrap()
            .command(
                c.game.authority_tick,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap()
    }

    #[test]
    fn cooperative_rewards_follow_npc_lives_without_connection_duplicates() {
        let mut c = chamber(99);
        c.configure_rewards(vec![rewards::Policy {
            participation: Default::default(),
            target: 2,
            experience: 45,
            items: vec![rewards::Entry { id: 1, count: 1 }],
            quests: vec![rewards::Entry { id: 1, count: 1 }],
        }])
        .unwrap();
        c.enroll_primary(A).unwrap();
        let extra = c.enroll_player(B, Vec3::new(3., 0., -22.)).unwrap();
        c.enroll_spectator(S).unwrap();
        let a = c.connect(A).unwrap();
        let b = c.connect(B).unwrap();
        let spectator = c.connect(S).unwrap();
        let primary = c.game.player_life().actor;
        let defeated = c.game.actor_life(2).unwrap();
        c.game.simulation.bow_impact(c.game.ids[&2], 1000).unwrap();
        c.tick(1. / 30.).unwrap();
        assert_eq!(c.inventory(A, a).unwrap().2.experience, 45);
        assert_eq!(c.inventory(B, b).unwrap().2.experience, 45);
        assert!(c.inventory(S, spectator).is_err());
        assert_eq!(c.rewards.revision(), 2);
        c.reward_cursor = 0;
        c.process_rewards().unwrap();
        assert_eq!(c.rewards.revision(), 2);
        c.disconnect(B, b).unwrap();
        c.game.time += 61.;
        c.tick(0.).unwrap();
        assert_eq!(c.game.actor_life(2).unwrap(), defeated.next().unwrap());
        c.game.simulation.bow_impact(c.game.ids[&2], 1000).unwrap();
        c.tick(1. / 30.).unwrap();
        assert_eq!(c.character_rewards(primary).unwrap().experience, 90);
        assert_eq!(c.character_rewards(extra.actor).unwrap().experience, 90);
        c.reset().unwrap();
        c.game.time = c.game.scene.cut_at;
        c.tick(0.).unwrap();
        c.game.simulation.bow_impact(c.game.ids[&2], 1000).unwrap();
        c.tick(1. / 30.).unwrap();
        assert_eq!(c.character_rewards(primary).unwrap().experience, 135);
        assert_eq!(c.character_rewards(extra.actor).unwrap().items[&1], 3);
    }
    #[test]
    fn two_players_and_spectator_share_one_clock_with_owned_commands() {
        let mut c = chamber(90);
        c.enroll_primary(A).unwrap();
        let extra = c.enroll_player(B, Vec3::new(3., 0., -22.)).unwrap();
        c.enroll_spectator(S).unwrap();
        let a = c.connect(A).unwrap();
        let b = c.connect(B).unwrap();
        let s = c.connect(S).unwrap();
        let ca = movement(&c, A, a);
        let cb = movement(&c, B, b);
        let before = c.game.checkpoint().unwrap();
        assert!(c.submit(B, a, ca.clone()).is_err());
        assert!(c.submit(B, b, ca.clone()).is_err());
        assert!(c.submit(S, s, ca.clone()).is_err());
        assert_eq!(before, c.game.checkpoint().unwrap());
        c.submit(A, a, ca).unwrap();
        c.submit(B, b, cb).unwrap();
        let start = c.game.actor_position(extra.actor).unwrap();
        let tick = c.game.authority_tick;
        let steps = c.game.physics_steps;
        c.tick(1. / 30.).unwrap();
        assert_eq!(c.game.authority_tick, tick + 1);
        assert_eq!(c.game.physics_steps, steps + 4);
        assert_ne!(start, c.game.actor_position(extra.actor).unwrap());
        assert_eq!(
            serde_json::to_vec(&c.snapshot(A, a).unwrap().actors).unwrap(),
            serde_json::to_vec(&c.snapshot(S, s).unwrap().actors).unwrap()
        );
        assert!(c.admission(S, s).is_err());
    }

    #[test]
    fn reconnect_disconnect_revocation_and_reset_fence_queued_input() {
        let mut c = chamber(91);
        c.enroll_primary(A).unwrap();
        let old = c.connect(A).unwrap();
        let command = movement(&c, A, old);
        c.submit(A, old, command.clone()).unwrap();
        let fresh = c.connect(A).unwrap();
        assert!(c.snapshot(A, old).is_err());
        assert!(c.submit(A, fresh, command).is_err());
        let pos = c.game.player;
        c.tick(1. / 30.).unwrap();
        assert_eq!(pos.x, c.game.player.x);
        let command = movement(&c, A, fresh);
        c.reset().unwrap();
        assert!(c.submit(A, fresh, command).is_err());
        assert_eq!(c.admission(A, fresh).unwrap().actor().instance, 91);
        c.disconnect(A, fresh).unwrap();
        assert!(c.snapshot(A, fresh).is_err());
        assert_eq!(c.game.admission.controller(), Controller(0));
        let reconnected = c.connect(A).unwrap();
        c.revoke(A).unwrap();
        assert!(c.snapshot(A, reconnected).is_err());
        assert!(c.connect(A).is_err());
    }

    #[test]
    fn session_casts_spend_only_owned_resources_and_respawn_only_owned_lives() {
        let mut c = chamber(94);
        c.enroll_primary(A).unwrap();
        let extra = c.enroll_player(B, Vec3::new(3., 0., -22.)).unwrap();
        let a = c.connect(A).unwrap();
        let b = c.connect(B).unwrap();
        for (principal, session) in [(A, a), (B, b)] {
            let command = c
                .admission(principal, session)
                .unwrap()
                .command(
                    c.game.authority_tick,
                    Intent::Cast {
                        ability: Ability::Shield,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            c.submit(principal, session, command).unwrap();
        }
        let primary = c.admission(A, a).unwrap().actor();
        assert_eq!(c.game.hostile_hit_player(primary, 45).unwrap(), (27, 18));
        assert_eq!(c.game.hostile_hit_player(extra, 45).unwrap(), (27, 18));
        assert_eq!(c.snapshot(A, a).unwrap().player.hp, 173);
        assert_eq!(c.snapshot(B, b).unwrap().player.hp, 173);
        c.game.hostile_hit_player(extra, 1000).unwrap();
        assert!(c.respawn(A, a, extra).is_err());
        let next = c.respawn(B, b, extra).unwrap();
        assert_eq!(next.generation, extra.generation + 1);
        assert_eq!(c.snapshot(B, b).unwrap().player.hp, 200);
        assert_eq!(c.snapshot(A, a).unwrap().player.hp, 173);
    }

    #[test]
    fn hosting_existing_players_clears_every_pending_controller_input() {
        let mut c = chamber(95);
        c.enroll_primary(A).unwrap();
        let life = c.enroll_player(B, Vec3::new(3., 0., -22.)).unwrap();
        let b = c.connect(B).unwrap();
        c.submit(B, b, movement(&c, B, b)).unwrap();
        let game = Game::restore(&c.game.checkpoint().unwrap()).unwrap();
        let mut hosted = Chamber::new(game).unwrap();
        let position = hosted.game.actor_position(life.actor).unwrap();
        hosted.tick(1. / 30.).unwrap();
        assert_eq!(
            position.x,
            hosted.game.actor_position(life.actor).unwrap().x
        );
        assert!(hosted.game.controlled_effects().all(|(life, _, _)| {
            hosted
                .game
                .player_admission(life.actor)
                .unwrap()
                .controller()
                == Controller(0)
        }));
        assert!(hosted.snapshot(B, b).is_err());
    }

    #[test]
    fn admission_budgets_foreign_handles_and_respawn_rights() {
        let mut c = chamber(92);
        c.enroll_primary(A).unwrap();
        assert!(c.enroll_primary(B).is_err());
        assert!(c.connect(B).is_err());
        let session = c.connect(A).unwrap();
        let mut other = chamber(93);
        other.enroll_primary(A).unwrap();
        other.connect(A).unwrap();
        assert!(other.snapshot(A, session).is_err());
        let life = c.admission(A, session).unwrap().actor();
        c.game.hostile_hit_player(life, 1000).unwrap();
        let next = c.respawn(A, session, life).unwrap();
        assert_eq!(next.generation, life.generation + 1);
        assert!(c.respawn(A, session, life).is_err());
        c.enroll_spectator(S).unwrap();
        let spectator = c.connect(S).unwrap();
        assert!(c.respawn(S, spectator, next).is_err());
        for n in 4..=129 {
            c.enroll_spectator(Principal([n; 32])).unwrap();
        }
        assert!(c.enroll_spectator(Principal([130; 32])).is_err());
        for _ in 0..140 {
            c.connect(A).unwrap();
        }
        assert_eq!(c.connections.len(), 2);
    }
}

#[cfg(test)]
mod participation_tests {
    use super::*;
    #[test]
    fn authored_participation_excludes_disconnected_and_distant_recipients() {
        use rewards::{Participation, Policy};
        for (participation, expected) in [
            (Participation::EnrolledResidents, [true, true, true]),
            (Participation::Connected, [true, true, false]),
            (
                Participation::ConnectedWithin { radius: 5 },
                [false, true, false],
            ),
        ] {
            let scene = verse_engine::director::Scene::from_json(include_bytes!(
                "../../../assets/verse/original/ritual.json"
            ))
            .unwrap();
            let mut game = Game::combat_in(scene, false, 1901).unwrap();
            game.time = game.scene.cut_at;
            game.tick(0., [0.; 2]).unwrap();
            let target = game.actor_position(2).unwrap();
            let mut chamber = Chamber::new(game).unwrap();
            let principals = [Principal([1; 32]), Principal([2; 32]), Principal([3; 32])];
            chamber.enroll_primary(principals[0]).unwrap();
            let near = chamber
                .enroll_player(principals[1], target + Vec3::X * 3.)
                .unwrap();
            let offline = chamber
                .enroll_player(principals[2], target - Vec3::X * 3.)
                .unwrap();
            for p in principals {
                chamber.connect(p).unwrap();
            }
            let connection = *chamber
                .connections
                .values()
                .find(|c| c.principal == principals[2])
                .unwrap();
            let session = Session {
                instance: 1901,
                serial: *chamber
                    .connections
                    .iter()
                    .find(|(_, c)| c.principal == principals[2])
                    .unwrap()
                    .0,
            };
            chamber.disconnect(connection.principal, session).unwrap();
            chamber
                .configure_rewards(vec![Policy {
                    participation,
                    target: 2,
                    experience: 7,
                    items: vec![],
                    quests: vec![],
                }])
                .unwrap();
            chamber
                .game
                .simulation
                .bow_impact(chamber.game.ids[&2], 1000)
                .unwrap();
            chamber.tick(1. / 30.).unwrap();
            let actors = [chamber.game.player_actor(), near.actor, offline.actor];
            for (actor, expected) in actors.into_iter().zip(expected) {
                assert_eq!(
                    chamber.character_rewards(actor).map_or(0, |c| c.experience),
                    if expected { 7 } else { 0 }
                );
            }
            chamber.reward_cursor = 0;
            chamber.process_rewards().unwrap();
            for (actor, expected) in actors.into_iter().zip(expected) {
                assert_eq!(
                    chamber.character_rewards(actor).map_or(0, |c| c.experience),
                    if expected { 7 } else { 0 }
                );
            }
        }
    }
}
