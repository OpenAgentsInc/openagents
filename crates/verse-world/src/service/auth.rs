//! Signed connection admission for a hosted chamber.
//!
//! The network adapter retains `ConnectionId` on its authenticated transport.
//! Client payloads must never select this ID. This adapter does not provide TLS,
//! a network listener, durable storage, or a Nostr authentication protocol.
use std::collections::BTreeMap;

use glam::Vec3;
use secp256k1::{Secp256k1, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use verse_engine::core::LifeId;

use super::{Chamber, Principal, Session};
use crate::{
    Admission, Command,
    play::{Ability, Game},
    rules::Snapshot,
};

const CAPACITY: usize = 128;
const LIFETIME_MS: u64 = 30_000;

/// Host-assigned transport identity, never a client-selected request field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ConnectionId {
    server: [u8; 32],
    serial: u64,
}

/// Public one-use challenge. Sign its digest with the enrolled identity key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    content: Option<[u8; 32]>,
    server: [u8; 32],
    instance: u64,
    connection: u64,
    nonce: [u8; 32],
    expires_ms: u64,
}
impl Challenge {
    pub fn content(&self) -> Option<[u8; 32]> {
        self.content
    }
    pub fn instance(&self) -> u64 {
        self.instance
    }
    /// SHA-256 over a versioned domain and fixed-width identity/context fields.
    pub fn signing_digest(&self, public_key: [u8; 32]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"verse.chamber.connection.v2\0");
        h.update([self.content.is_some() as u8]);
        if let Some(content) = self.content {
            h.update(content);
        }
        h.update(self.server);
        h.update(self.instance.to_be_bytes());
        h.update(self.connection.to_be_bytes());
        h.update(self.nonce);
        h.update(self.expires_ms.to_be_bytes());
        h.update(public_key);
        h.finalize().into()
    }
}

#[derive(Clone, Copy)]
struct Binding {
    principal: Principal,
    session: Session,
}

/// Public admission policy: any authenticated key the chamber does not
/// know joins as a player on a spawn ring until the cap is reached.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Guests {
    /// Most guest players alive in the chamber at once (1..=63).
    pub cap: u32,
    /// Center of the spawn ring.
    pub ring: [f32; 3],
    /// Radius of the spawn ring in meters (0.5..=100).
    pub radius: f32,
}
impl Guests {
    pub fn validate(&self) -> Result<(), String> {
        let ring = Vec3::from(self.ring);
        if !(1..=63).contains(&self.cap)
            || !ring.is_finite()
            || ring.abs().max_element() > 10_000.
            || !self.radius.is_finite()
            || !(0.5..=100.).contains(&self.radius)
        {
            return Err("Invalid chamber guest admission policy".into());
        }
        Ok(())
    }
    /// The `index`th spawn on the ring; indexes past the cap keep circling
    /// with a half-step offset so a blocked spawn finds a free neighbor.
    pub fn spawn(&self, index: usize) -> Vec3 {
        let cap = self.cap as usize;
        let turn = std::f32::consts::TAU / cap as f32;
        let angle = (index % cap) as f32 * turn + (index / cap) as f32 * turn * 0.5;
        Vec3::from(self.ring) + Vec3::new(angle.cos(), 0., angle.sin()) * self.radius
    }
}

pub(super) struct Proof {
    connection: ConnectionId,
    principal: Principal,
}
impl Proof {
    pub(super) fn principal(&self) -> Principal {
        self.principal
    }
}

/// Authenticates enrolled keys and derives dispatch authority from the transport.
pub struct Gateway {
    pub(super) chamber: Chamber,
    guests: Option<Guests>,
    /// Player enrollments the configuration placed; guests count above them.
    pub(super) configured_players: usize,
    pub(super) account_observers: std::collections::BTreeSet<Principal>,
    content: Option<[u8; 32]>,
    server: [u8; 32],
    next_connection: u64,
    last_now: u64,
    pending: BTreeMap<ConnectionId, Challenge>,
    bindings: BTreeMap<ConnectionId, Binding>,
    pub(super) view_index: Option<super::replication::Index>,
    pub(super) view_cache: Option<super::wire::State>,
    pub(super) replication_totals: super::replication::Stats,
    pub(super) replication: BTreeMap<ConnectionId, super::replication::Sender>,
}
impl Gateway {
    /// Isolates a host transaction while retaining unrelated transport authority.
    /// Baselines resynchronize without restarting their revision sequence.
    pub(super) fn fork(&self) -> Self {
        Self {
            chamber: self.chamber.clone(),
            guests: self.guests.clone(),
            configured_players: self.configured_players,
            account_observers: self.account_observers.clone(),
            content: self.content,
            server: self.server,
            next_connection: self.next_connection,
            last_now: self.last_now,
            pending: self.pending.clone(),
            bindings: self.bindings.clone(),
            view_index: None,
            view_cache: None,
            replication_totals: self.replication_totals.clone(),
            replication: self
                .replication
                .iter()
                .map(|(id, sender)| (*id, sender.fork()))
                .collect(),
        }
    }

    pub fn new(chamber: Chamber) -> Result<Self, String> {
        let mut server = [0; 32];
        getrandom::fill(&mut server)
            .map_err(|_| "Cannot generate chamber authentication entropy")?;
        Ok(Self {
            chamber,
            guests: None,
            configured_players: 0,
            account_observers: Default::default(),
            content: None,
            server,
            next_connection: 1,
            last_now: 0,
            pending: BTreeMap::new(),
            bindings: BTreeMap::new(),
            replication: BTreeMap::new(),
            replication_totals: Default::default(),
            view_cache: None,
            view_index: None,
        })
    }
    /// Admits unknown keys as guest players, above `configured_players`
    /// configured player enrollments.
    pub fn with_guests(
        mut self,
        guests: Option<Guests>,
        configured_players: usize,
    ) -> Result<Self, String> {
        if let Some(guests) = &guests {
            guests.validate()?;
        }
        self.guests = guests;
        self.configured_players = configured_players;
        Ok(self)
    }
    pub fn guests(&self) -> Option<&Guests> {
        self.guests.as_ref()
    }
    /// Guest players enrolled so far: owned adventurers beyond the primary
    /// and the configured player enrollments.
    pub fn guest_count(&self) -> usize {
        let primary = self.game().player_life().actor;
        self.chamber
            .owners
            .values()
            .filter(|actor| **actor != primary)
            .count()
            .saturating_sub(self.configured_players)
    }
    /// Enrolls an authenticated unknown key on the guest spawn ring.
    fn admit_guest(&mut self, principal: super::Principal) -> Result<(), String> {
        let Some(guests) = self.guests.clone() else {
            return Err("Principal is not enrolled".into());
        };
        let taken = self.guest_count();
        if taken >= guests.cap as usize {
            return Err("Chamber guest capacity exceeded".into());
        }
        let mut last = String::new();
        for attempt in 0..guests.cap as usize * 2 {
            match self
                .chamber
                .enroll_player(principal, guests.spawn(taken + attempt))
            {
                Ok(_) => return Ok(()),
                Err(error) => last = error,
            }
        }
        Err(format!("Chamber guest spawn ring is blocked: {last}"))
    }
    /// Fixes content identity before any opening challenge has been issued.
    pub fn with_content(mut self, content: [u8; 32]) -> Result<Self, String> {
        if self.next_connection != 1 || self.content.is_some() {
            return Err("Chamber content identity is already bound".into());
        }
        self.content = Some(content);
        Ok(self)
    }
    /// Read-only host authority; clients use admitted snapshots instead.
    pub fn game(&self) -> &Game {
        self.chamber.game()
    }
    pub fn content(&self) -> Option<[u8; 32]> {
        self.content
    }
    /// Binds durable storage to this live host authority.
    pub(super) fn server_identity(&self) -> [u8; 32] {
        self.server
    }
    pub(super) fn committed_controls(&self) -> BTreeMap<ConnectionId, super::wire::Control> {
        self.bindings
            .keys()
            .filter_map(|id| {
                self.admission(*id).ok().map(|admission| {
                    (
                        *id,
                        super::wire::Control {
                            credit_step: self.game().physics_steps,
                            world_step: self.game().physics_steps,
                            life: admission.actor().into(),
                            epoch: admission.epoch(),
                            accepted_sequence: admission.accepted_sequence(),
                            applied_movement: None,
                            dynamic: Vec::new(),
                        },
                    )
                })
            })
            .collect()
    }
    pub(super) fn committed_connections(&self) -> std::collections::BTreeSet<ConnectionId> {
        self.bindings.keys().copied().collect()
    }
    /// Saves the world and grants without connection challenges or sessions.
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        super::save::encode(self)
    }
    /// Restores enrolled characters with parked controls and fresh authentication.
    pub fn restore(bytes: &[u8], content: [u8; 32], instance: u64) -> Result<Self, String> {
        super::save::decode(bytes, content, instance)
    }
    /// Publishes a trusted host projection; no wire request grants this operation.
    pub fn publish_social_studio(
        &mut self,
        actors: Vec<crate::play::social::SeatActor>,
    ) -> Result<(), String> {
        self.chamber.game.set_social_studio(actors)?;
        self.view_cache = None;
        Ok(())
    }
    pub fn enroll_primary(&mut self, key: [u8; 32]) -> Result<(), String> {
        self.view_cache = None;
        self.chamber.enroll_primary(valid_principal(key)?)
    }
    pub fn enroll_player(&mut self, key: [u8; 32], spawn: Vec3) -> Result<LifeId, String> {
        self.view_cache = None;
        self.chamber.enroll_player(valid_principal(key)?, spawn)
    }
    pub fn enroll_spectator(&mut self, key: [u8; 32]) -> Result<(), String> {
        self.view_cache = None;
        self.chamber.enroll_spectator(valid_principal(key)?)
    }
    /// Enrolls a key its transport already admitted as a spectator, unless the
    /// chamber knows it. Returns whether it enrolled the key.
    pub fn admit_spectator(&mut self, key: [u8; 32]) -> Result<bool, String> {
        let principal = valid_principal(key)?;
        if self.chamber.grants.contains_key(&principal) {
            return Ok(false);
        }
        self.chamber.enroll_spectator(principal).map(|()| true)
    }
    /// Trusted host operation, excluded from client request payloads.
    pub fn grant_reward(
        &mut self,
        transaction: super::rewards::Transaction,
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        self.chamber.grant_reward(transaction)
    }
    pub fn character_rewards(&self, actor: u64) -> Option<&super::rewards::Character> {
        self.chamber.character_rewards(actor)
    }
    /// Fixes the cooperative reward policy before issuing connection challenges.
    pub fn with_rewards(mut self, policies: Vec<super::rewards::Policy>) -> Result<Self, String> {
        if self.next_connection != 1 || !self.chamber.reward_policy.is_empty() {
            return Err("Combat reward policy is already bound".into());
        }
        self.chamber.configure_rewards(policies)?;
        Ok(self)
    }
    pub fn reward_policy(&self) -> &[super::rewards::Policy] {
        &self.chamber.reward_policy
    }
    pub fn inventory(
        &self,
        id: ConnectionId,
    ) -> Result<(LifeId, u64, super::rewards::Character), String> {
        let b = self.binding(id)?;
        self.chamber.inventory(b.principal, b.session)
    }
    pub fn with_progression(mut self, config: super::progression::Config) -> Result<Self, String> {
        if self.next_connection != 1
            || self.chamber.progression != super::progression::Config::default()
        {
            return Err("Campaign progression is already bound".into());
        }
        self.chamber.validate_givers(&config)?;
        self.chamber.progression = config;
        Ok(self)
    }
    pub fn progression(&self) -> &super::progression::Config {
        &self.chamber.progression
    }
    pub fn quest_log(&self, actor: u64) -> Vec<super::progression::Progress> {
        self.chamber.quest_log(actor)
    }
    pub fn accept_quest(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        quest: u64,
        giver: LifeId,
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .accept_quest(b.principal, b.session, life, epoch, quest, giver)
    }
    pub fn quest_cycle(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        quest: u64,
        cycle: u64,
        action: super::progression::Action,
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .quest_cycle(b.principal, b.session, life, epoch, quest, cycle, action)
    }
    pub fn claim_quest(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        quest: u64,
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .claim_quest(b.principal, b.session, life, epoch, quest)
    }
    pub fn with_items(mut self, catalog: super::items::Catalog) -> Result<Self, String> {
        if self.next_connection != 1 || self.chamber.items != super::items::Catalog::default() {
            return Err("Item catalog is already bound".into());
        }
        self.chamber
            .equipment
            .validate_catalogs(&catalog, &self.chamber.outfits)?;
        self.chamber.items = catalog;
        Ok(self)
    }
    pub fn items(&self) -> &super::items::Catalog {
        &self.chamber.items
    }
    pub fn use_item(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        item: u64,
        operation: [u8; 16],
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .use_item(b.principal, b.session, life, epoch, item, operation)
    }
    pub fn with_outfits(mut self, catalog: super::outfits::Catalog) -> Result<Self, String> {
        if self.next_connection != 1 || self.chamber.outfits != super::outfits::Catalog::default() {
            return Err("Outfit catalog is already bound".into());
        }
        self.chamber
            .equipment
            .validate_catalogs(&self.chamber.items, &catalog)?;
        self.chamber.outfits = catalog;
        Ok(self)
    }
    pub fn with_equipment(mut self, catalog: super::equipment::Catalog) -> Result<Self, String> {
        if self.next_connection != 1
            || self.chamber.equipment != super::equipment::Catalog::default()
        {
            return Err("Equipment catalog is already bound".into());
        }
        catalog.validate_catalogs(&self.chamber.items, &self.chamber.outfits)?;
        self.chamber.equipment = catalog;
        Ok(self)
    }
    pub fn equipment(&self) -> &super::equipment::Catalog {
        &self.chamber.equipment
    }
    pub fn equip_gear(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        slot: super::equipment::Slot,
        item: u64,
        operation: [u8; 16],
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .equip_gear(b.principal, b.session, life, epoch, slot, item, operation)
    }
    pub fn outfits(&self) -> &super::outfits::Catalog {
        &self.chamber.outfits
    }
    pub fn equip_outfit(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
        outfit: u64,
        operation: [u8; 16],
    ) -> Result<super::rewards::Receipt, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .equip_outfit(b.principal, b.session, life, epoch, outfit, operation)
    }
    fn clock(&mut self, now_ms: u64) -> Result<(), String> {
        if now_ms < self.last_now {
            return Err("Authentication clock moved backward".into());
        }
        self.last_now = now_ms;
        Ok(())
    }
    /// Opens a connection using monotonic milliseconds supplied by the host.
    pub fn open(&mut self, now_ms: u64) -> Result<(ConnectionId, Challenge), String> {
        self.view_cache = None;
        self.clock(now_ms)?;
        self.pending.retain(|_, c| now_ms < c.expires_ms);
        if self.pending.len() + self.bindings.len() >= CAPACITY {
            return Err("Chamber connection budget exceeded".into());
        }
        let expires_ms = now_ms
            .checked_add(LIFETIME_MS)
            .ok_or("Authentication deadline exhausted")?;
        let next = self
            .next_connection
            .checked_add(1)
            .ok_or("Connection identities exhausted")?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce)
            .map_err(|_| "Cannot generate chamber authentication entropy")?;
        let id = ConnectionId {
            server: self.server,
            serial: self.next_connection,
        };
        let challenge = Challenge {
            content: self.content,
            server: self.server,
            instance: self.game().player_life().instance,
            connection: id.serial,
            nonce,
            expires_ms,
        };
        self.next_connection = next;
        self.pending.insert(id, challenge.clone());
        Ok((id, challenge))
    }
    /// Consumes the challenge on every verification attempt, including refusal.
    pub fn authenticate(
        &mut self,
        connection: ConnectionId,
        now_ms: u64,
        public_key: [u8; 32],
        signature: [u8; 64],
    ) -> Result<(), String> {
        let proof = self.verify(connection, now_ms, public_key, signature)?;
        if !self.chamber.grants.contains_key(&proof.principal()) {
            self.admit_guest(proof.principal())?;
        }
        self.bind_verified(proof)
    }
    pub(super) fn verify(
        &mut self,
        connection: ConnectionId,
        now_ms: u64,
        public_key: [u8; 32],
        signature: [u8; 64],
    ) -> Result<Proof, String> {
        self.view_cache = None;
        self.clock(now_ms)?;
        let challenge = self
            .pending
            .remove(&connection)
            .ok_or("Connection challenge is unavailable")?;
        if now_ms >= challenge.expires_ms {
            return Err("Connection challenge expired".into());
        }
        let principal = valid_principal(public_key)?;
        let key = XOnlyPublicKey::from_byte_array(public_key)
            .map_err(|_| "Invalid identity public key")?;
        Secp256k1::verification_only()
            .verify_schnorr(
                &Signature::from_byte_array(signature),
                &challenge.signing_digest(public_key),
                &key,
            )
            .map_err(|_| "Connection signature refused")?;
        Ok(Proof {
            connection,
            principal,
        })
    }
    pub(super) fn bind_verified(&mut self, proof: Proof) -> Result<(), String> {
        self.bind_principal(proof.connection, proof.principal)
    }
    pub(super) fn principal(&self, connection: ConnectionId) -> Result<Principal, String> {
        self.check_view(connection)?;
        Ok(self.binding(connection)?.principal)
    }
    pub(super) fn refresh_binding(&mut self, connection: ConnectionId) -> Result<(), String> {
        let principal = self.principal(connection)?;
        self.bind_principal(connection, principal)
    }
    fn bind_principal(
        &mut self,
        connection: ConnectionId,
        principal: Principal,
    ) -> Result<(), String> {
        let session = self.chamber.connect(principal)?;
        self.bindings.retain(|_, b| b.principal != principal);
        self.purge_replication();
        self.bindings
            .insert(connection, Binding { principal, session });
        Ok(())
    }
    fn binding(&self, id: ConnectionId) -> Result<Binding, String> {
        self.bindings
            .get(&id)
            .copied()
            .ok_or_else(|| "Connection is not authenticated".into())
    }
    pub(super) fn check_view(&self, id: ConnectionId) -> Result<(), String> {
        let b = self.binding(id)?;
        self.chamber.connection(b.principal, b.session)?;
        Ok(())
    }
    pub(super) fn check_events(&self, id: ConnectionId) -> Result<(), String> {
        let b = self.binding(id)?;
        if matches!(
            self.chamber.connection(b.principal, b.session)?.rights,
            super::Rights::Player(_)
        ) {
            self.chamber.admission(b.principal, b.session)?;
        }
        Ok(())
    }
    pub fn admission(&self, id: ConnectionId) -> Result<Admission, String> {
        let b = self.binding(id)?;
        self.chamber.admission(b.principal, b.session)
    }
    pub fn submit(&mut self, id: ConnectionId, command: Command<Ability>) -> Result<(), String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber.submit(b.principal, b.session, command)
    }
    pub fn submit_social(
        &mut self,
        id: ConnectionId,
        input: crate::play::social::Input,
    ) -> Result<(), String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber.submit_social(b.principal, b.session, input)
    }
    pub fn begin_movement_frames(
        &mut self,
        id: ConnectionId,
        life: LifeId,
        epoch: u64,
    ) -> Result<(), String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .begin_movement_frames(b.principal, b.session, life, epoch)
    }
    pub fn submit_movement_frame(
        &mut self,
        id: ConnectionId,
        frame: crate::movement::frames::Frame,
    ) -> Result<(), String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber
            .submit_movement_frame(b.principal, b.session, frame)
    }
    pub fn snapshot(&self, id: ConnectionId) -> Result<Snapshot, String> {
        let b = self.binding(id)?;
        self.chamber.snapshot(b.principal, b.session)
    }
    pub fn respawn(&mut self, id: ConnectionId, life: LifeId) -> Result<LifeId, String> {
        self.view_cache = None;
        let b = self.binding(id)?;
        self.chamber.respawn(b.principal, b.session, life)
    }
    pub fn close(&mut self, id: ConnectionId) -> Result<(), String> {
        self.view_cache = None;
        self.retire_replication(id);
        if let Some(b) = self.bindings.get(&id).copied() {
            self.chamber.disconnect(b.principal, b.session)?;
            self.bindings.remove(&id);
            if self.account_observers.remove(&b.principal) {
                self.chamber.grants.remove(&b.principal);
            }
            Ok(())
        } else if self.pending.remove(&id).is_some() {
            Ok(())
        } else {
            Err("Connection is already closed".into())
        }
    }
    pub fn revoke(&mut self, key: [u8; 32]) -> Result<(), String> {
        self.view_cache = None;
        let principal = valid_principal(key)?;
        self.chamber.revoke(principal)?;
        self.account_observers.remove(&principal);
        self.bindings.retain(|_, b| b.principal != principal);
        self.purge_replication();
        Ok(())
    }
    /// Parks admitted controllers and clears challenges when the host stops.
    pub fn close_all(&mut self) -> Result<(), String> {
        self.view_cache = None;
        let ids: Vec<_> = self.bindings.keys().copied().collect();
        for id in ids {
            self.close(id)?;
        }
        self.pending.clear();
        Ok(())
    }
    #[cfg(feature = "service-net")]
    pub(super) fn authenticated(&self, id: ConnectionId) -> bool {
        self.bindings.contains_key(&id)
    }
    pub fn tick(&mut self, dt: f32) -> Result<(), String> {
        self.view_cache = None;
        self.chamber.tick(dt)
    }
    pub fn reset(&mut self) -> Result<(), String> {
        self.view_cache = None;
        self.chamber.reset()
    }
}
pub(super) fn valid_principal(key: [u8; 32]) -> Result<Principal, String> {
    XOnlyPublicKey::from_byte_array(key).map_err(|_| "Invalid identity public key")?;
    Ok(Principal(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Intent;
    use secp256k1::{Keypair, SecretKey};
    use verse_engine::director::Scene;
    fn key(n: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn public(key: &Keypair) -> [u8; 32] {
        key.x_only_public_key().0.serialize()
    }
    fn sign(c: &Challenge, key: &Keypair) -> [u8; 64] {
        Secp256k1::new()
            .sign_schnorr_no_aux_rand(&c.signing_digest(public(key)), key)
            .to_byte_array()
    }
    fn gateway(instance: u64) -> Gateway {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::combat_in(scene, false, instance).unwrap();
        game.time = game.scene.cut_at;
        game.tick(0., [0.; 2]).unwrap();
        Gateway::new(Chamber::new(game).unwrap()).unwrap()
    }
    fn join(g: &mut Gateway, k: &Keypair, now: u64) -> ConnectionId {
        let (id, challenge) = g.open(now).unwrap();
        g.authenticate(id, now, public(k), sign(&challenge, k))
            .unwrap();
        id
    }
    #[test]
    fn challenge_signature_binds_immutable_content_identity() {
        let key = key(1);
        let mut g = gateway(180).with_content([9; 32]).unwrap();
        g.enroll_primary(public(&key)).unwrap();
        let (id, challenge) = g.open(0).unwrap();
        assert_eq!(challenge.content(), Some([9; 32]));
        let mut forged = challenge.clone();
        forged.content = Some([8; 32]);
        assert_ne!(
            challenge.signing_digest(public(&key)),
            forged.signing_digest(public(&key))
        );
        assert!(
            g.authenticate(id, 0, public(&key), sign(&forged, &key))
                .is_err()
        );
        let (id, challenge) = g.open(1).unwrap();
        g.authenticate(id, 1, public(&key), sign(&challenge, &key))
            .unwrap();
        assert!(g.with_content([8; 32]).is_err());
    }
    #[test]
    fn signed_players_and_spectator_dispatch_without_request_principals() {
        let mut g = gateway(100);
        let a = key(1);
        let b = key(2);
        let s = key(3);
        g.enroll_primary(public(&a)).unwrap();
        g.enroll_player(public(&b), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&s)).unwrap();
        let a = join(&mut g, &a, 0);
        let b = join(&mut g, &b, 0);
        let s = join(&mut g, &s, 0);
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
        assert!(g.submit(b, command.clone()).is_err());
        assert!(g.submit(s, command.clone()).is_err());
        g.submit(a, command).unwrap();
        g.tick(1. / 30.).unwrap();
        assert_eq!(
            serde_json::to_vec(&g.snapshot(a).unwrap().actors).unwrap(),
            serde_json::to_vec(&g.snapshot(s).unwrap().actors).unwrap()
        );
        assert!(g.admission(s).is_err());
    }
    #[test]
    fn challenges_are_single_use_expiring_and_bound_to_key_connection_server_instance() {
        let mut g = gateway(101);
        let k = key(4);
        g.enroll_primary(public(&k)).unwrap();
        let (id, c) = g.open(10).unwrap();
        let signature = sign(&c, &k);
        g.authenticate(id, 10, public(&k), signature).unwrap();
        assert!(g.authenticate(id, 10, public(&k), signature).is_err());
        let (expired, c) = g.open(20).unwrap();
        assert!(
            g.authenticate(expired, 30_020, public(&k), sign(&c, &k))
                .is_err()
        );
        let (x, cx) = g.open(30_020).unwrap();
        let (y, _) = g.open(30_020).unwrap();
        assert!(
            g.authenticate(y, 30_020, public(&k), sign(&cx, &k))
                .is_err()
        );
        assert!(
            g.authenticate(y, 30_020, public(&k), sign(&cx, &k))
                .is_err()
        );
        let mut other = gateway(101);
        other.enroll_primary(public(&k)).unwrap();
        let (other_id, _) = other.open(30_020).unwrap();
        assert!(
            other
                .authenticate(other_id, 30_020, public(&k), sign(&cx, &k))
                .is_err()
        );
        let mut changed = cx.clone();
        changed.instance += 1;
        assert!(
            g.authenticate(x, 30_020, public(&k), sign(&changed, &k))
                .is_err()
        );
        let (bad, c) = g.open(30_020).unwrap();
        let wrong = key(5);
        assert!(
            g.authenticate(bad, 30_020, public(&k), sign(&c, &wrong))
                .is_err()
        );
        assert!(
            g.authenticate(bad, 30_020, public(&k), sign(&c, &k))
                .is_err()
        );
        assert!(g.open(1).is_err());
    }
    #[test]
    fn reconnect_close_and_revocation_remove_dispatch_authority() {
        let mut g = gateway(102);
        let k = key(6);
        g.enroll_primary(public(&k)).unwrap();
        let old = join(&mut g, &k, 0);
        let queued = g
            .admission(old)
            .unwrap()
            .command(g.game().authority_tick, Intent::Jump)
            .unwrap();
        let new = join(&mut g, &k, 1);
        assert!(g.snapshot(old).is_err());
        assert!(g.submit(new, queued).is_err());
        g.close(new).unwrap();
        assert!(g.snapshot(new).is_err());
        let new = join(&mut g, &k, 2);
        g.revoke(public(&k)).unwrap();
        assert!(g.snapshot(new).is_err());
        let (id, c) = g.open(3).unwrap();
        assert!(g.authenticate(id, 3, public(&k), sign(&c, &k)).is_err());
        assert!(g.snapshot(id).is_err());
    }
    #[test]
    fn authenticated_handles_cannot_cross_server_lifetimes() {
        let k = key(8);
        let mut first = gateway(104);
        let mut replacement = gateway(104);
        first.enroll_primary(public(&k)).unwrap();
        replacement.enroll_primary(public(&k)).unwrap();
        let old = join(&mut first, &k, 0);
        let new = join(&mut replacement, &k, 0);
        assert_ne!(old, new);
        assert!(replacement.snapshot(old).is_err());
        assert!(first.snapshot(new).is_err());
        assert!(replacement.close(old).is_err());
        assert!(replacement.snapshot(new).is_ok());
    }

    #[test]
    fn pending_budget_expiry_invalid_keys_and_unenrolled_signatures() {
        let mut g = gateway(103);
        assert!(g.enroll_primary([0xff; 32]).is_err());
        let k = key(7);
        let (id, c) = g.open(0).unwrap();
        assert!(g.authenticate(id, 0, public(&k), sign(&c, &k)).is_err());
        assert!(g.snapshot(id).is_err());
        for _ in 0..CAPACITY {
            g.open(0).unwrap();
        }
        assert!(g.open(0).is_err());
        assert!(g.open(LIFETIME_MS).is_ok());
        let (id, _) = g.open(LIFETIME_MS).unwrap();
        g.close(id).unwrap();
        assert!(g.close(id).is_err());
    }
}
