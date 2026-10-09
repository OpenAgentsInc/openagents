//! Host-owned independent instances with durable placement and authority fences.
use super::{
    auth::{ConnectionId, Gateway},
    rewards::history::History,
    wire::{Body, Request},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    net::SocketAddr,
    path::{Path, PathBuf},
};
use verse_engine::core::LifeId;
mod disk;
mod lifecycle;
pub mod net;
mod registry;
mod safety;
mod services;
mod transfer;
pub use registry::{Account, Character, Residence};
pub use transfer::Transfer;
const INSTANCES: usize = 32;
const CHARACTERS: usize = 2048;
const LEASE_MS: u64 = 30_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Open,
    Draining,
    Stopped,
}
/// A trusted adapter retains this capability; network bodies cannot select it.
#[derive(Clone, Debug)]
pub struct Lease {
    realm: [u8; 32],
    run: [u8; 32],
    instance: u64,
    epoch: u64,
    owner: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub realm: [u8; 32],
    pub instance: u64,
    pub epoch: u64,
    pub owner: [u8; 32],
    pub endpoint: SocketAddr,
    pub expires_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    principal: [u8; 32],
    instance: u64,
    actor: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    content: [u8; 32],
    snapshot: [u8; 32],
    phase: Phase,
    capacity: u16,
    epoch: u64,
    owner: Option<[u8; 32]>,
    endpoint: SocketAddr,
    expires_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u16,
    id: [u8; 32],
    revision: u64,
    last_now_ms: u64,
    next_character: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registry_root: registry::Root,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_account: u64,
    transfer_root: Option<[u8; 32]>,
    transfers: u64,
    instances: BTreeMap<u64, Slot>,
    characters: BTreeMap<u64, Placement>,
}
fn is_zero(value: &u64) -> bool {
    *value == 0
}
/// One coordinator holds the durable writer lock and all mutable game authorities.
pub struct Realm {
    root: PathBuf,
    _lock: File,
    run: [u8; 32],
    manifest: Manifest,
    games: BTreeMap<u64, Gateway>,
    history: History,
    poisoned: bool,
    dirty: BTreeSet<u64>,
    transfer_commit: bool,
    lifecycle_commit: bool,
    services_commit: bool,
    safety_commit: bool,
}
impl Realm {
    pub fn open(root: &Path) -> Result<Self, String> {
        disk::open(root)
    }
    fn clock(&mut self, now: u64) -> Result<(), String> {
        if self.poisoned {
            return Err("Realm requires recovery after an uncertain commit".into());
        }
        if now < self.manifest.last_now_ms {
            return Err("Realm host clock regressed".into());
        }
        self.manifest.last_now_ms = now;
        Ok(())
    }
    fn check(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.clock(now)?;
        let slot = self
            .manifest
            .instances
            .get(&lease.instance)
            .ok_or("Realm instance is missing")?;
        if lease.realm != self.manifest.id
            || lease.run != self.run
            || lease.epoch != slot.epoch
            || Some(lease.owner) != slot.owner
            || slot.expires_ms <= now
            || slot.phase == Phase::Stopped
        {
            return Err("Realm authority lease is stale or unavailable".into());
        }
        Ok(())
    }
    fn publish(&mut self, instances: &[u64]) -> Result<(), String> {
        let result = disk::publish(self, instances);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// Admits a prepared instance without assigning a running authority.
    pub fn create(
        &mut self,
        mut gateway: Gateway,
        endpoint: SocketAddr,
        capacity: u16,
        now: u64,
    ) -> Result<(), String> {
        self.clock(now)?;
        let instance = gateway.game().player_life().instance;
        let content = gateway
            .content()
            .ok_or("Realm instance requires bound content")?;
        if self.manifest.instances.len() >= INSTANCES
            || self.manifest.instances.contains_key(&instance)
            || !(1..=64).contains(&capacity)
            || endpoint.port() == 0
            || gateway.chamber.owners.len() > capacity as usize
            || self.manifest.characters.len() + gateway.chamber.owners.len() > CHARACTERS
            || gateway.chamber.owners.keys().any(|p| {
                self.manifest
                    .characters
                    .values()
                    .any(|c| c.principal == p.0)
            })
        {
            return Err("Realm instance capacity, identity, or ownership is incompatible".into());
        }
        self.manifest
            .next_character
            .checked_add(gateway.chamber.owners.len() as u64)
            .ok_or("Realm character identities exhausted")?;
        gateway.close_all()?;
        gateway.chamber.rewards.attach(self.history.clone())?;
        let assignments: Vec<_> = gateway
            .chamber
            .owners
            .values()
            .enumerate()
            .map(|(index, actor)| (*actor, self.manifest.next_character + index as u64))
            .collect();
        gateway
            .chamber
            .rewards
            .activate_books(&assignments, instance)?;
        let mut root = self.manifest.registry_root;
        let mut next_account = self.manifest.next_account;
        for (index, (principal, actor)) in gateway.chamber.owners.iter().enumerate() {
            if self.account_for_key(principal.0)?.is_some() {
                return Err("Realm account already exists".into());
            }
            root = registry::put(
                self,
                root,
                registry::initial(
                    next_account,
                    self.manifest.next_character + index as u64,
                    &Placement {
                        principal: principal.0,
                        instance,
                        actor: *actor,
                    },
                ),
            )?;
            next_account = next_account
                .checked_add(1)
                .ok_or("Realm account identities exhausted")?;
        }
        self.manifest.registry_root = root;
        self.manifest.next_account = next_account;
        for (principal, actor) in &gateway.chamber.owners {
            let id = self.manifest.next_character;
            self.manifest.next_character = id
                .checked_add(1)
                .ok_or("Realm character identities exhausted")?;
            self.manifest.characters.insert(
                id,
                Placement {
                    principal: principal.0,
                    instance,
                    actor: *actor,
                },
            );
        }
        self.manifest.instances.insert(
            instance,
            Slot {
                content,
                snapshot: [0; 32],
                phase: Phase::Open,
                capacity,
                epoch: 0,
                owner: None,
                endpoint,
                expires_ms: 0,
            },
        );
        self.games.insert(instance, gateway);
        self.publish(&[instance])
    }
    /// Admits one owned character through the leased instance's collision checks.
    pub fn admit(
        &mut self,
        lease: &Lease,
        principal: [u8; 32],
        spawn: [f32; 3],
        now: u64,
    ) -> Result<(u64, LifeId), String> {
        self.admit_character(lease, principal, spawn, now, false)
    }
    /// Creates another owned character after the account logs out its resident.
    pub fn create_character(
        &mut self,
        lease: &Lease,
        principal: [u8; 32],
        spawn: [f32; 3],
        now: u64,
    ) -> Result<(u64, LifeId), String> {
        self.admit_character(lease, principal, spawn, now, true)
    }
    fn admit_character(
        &mut self,
        lease: &Lease,
        principal: [u8; 32],
        spawn: [f32; 3],
        now: u64,
        additional: bool,
    ) -> Result<(u64, LifeId), String> {
        self.check(lease, now)?;
        if self.manifest.instances[&lease.instance].phase != Phase::Open {
            return Err("Realm instance is draining".into());
        }
        if let Some((id, c)) = self
            .manifest
            .characters
            .iter()
            .find(|(_, c)| c.principal == principal)
        {
            if additional
                || c.instance != lease.instance
                || self.games[&c.instance].game().player_spawn(c.actor)
                    != Some(glam::Vec3::from(spawn))
            {
                return Err("Realm character admission retry differs from its placement".into());
            }
            return Ok((
                *id,
                self.games[&c.instance]
                    .game()
                    .player_admission(c.actor)
                    .unwrap()
                    .actor(),
            ));
        }
        let slot = &self.manifest.instances[&lease.instance];
        if self.manifest.characters.len() >= CHARACTERS
            || self.games[&lease.instance].chamber.owners.len() >= slot.capacity as usize
        {
            return Err("Realm character admission capacity exceeded".into());
        }
        let existing = self.account_for_key(principal)?;
        if existing
            .as_ref()
            .is_some_and(|a| !additional || a.characters.len() >= 8)
        {
            return Err("Realm account requires selection or exceeds its character budget".into());
        }
        if additional && existing.is_none() {
            return Err("Additional character requires an existing account".into());
        }
        let id = self.manifest.next_character;
        let account = existing
            .as_ref()
            .map_or(self.manifest.next_account, |a| a.id);
        let next_account = if existing.is_some() {
            self.manifest.next_account
        } else {
            account
                .checked_add(1)
                .ok_or("Realm account identities exhausted")?
        };
        let next = id
            .checked_add(1)
            .ok_or("Realm character identities exhausted")?;
        let original = &self.games[&lease.instance];
        let mut candidate = original.fork();
        let identity = super::Principal(principal);
        if existing.is_some()
            && matches!(
                candidate.chamber.grants.get(&identity),
                Some(super::Rights::Spectator)
            )
        {
            candidate.chamber.grants.remove(&identity);
            candidate.account_observers.remove(&identity);
        }
        let life = candidate.enroll_player(principal, glam::Vec3::from(spawn))?;
        candidate
            .chamber
            .rewards
            .register_book(life.actor, id, life.instance)?;
        super::save::decode_with_history(
            &candidate.checkpoint()?,
            slot.content,
            lease.instance,
            Some(self.history.clone()),
        )?;
        let placement = Placement {
            principal,
            instance: lease.instance,
            actor: life.actor,
        };
        let records = if let Some(mut owner) = existing {
            owner.characters.push(id);
            vec![
                registry::Record::Account(owner),
                registry::Record::Character(Character {
                    id,
                    account,
                    residence: Residence::Resident {
                        instance: lease.instance,
                        actor: life.actor,
                    },
                }),
            ]
        } else {
            registry::initial(account, id, &placement)
        };
        let root = registry::put(self, self.manifest.registry_root, records)?;
        self.games.insert(lease.instance, candidate);
        self.manifest.registry_root = root;
        self.manifest.next_account = next_account;
        self.manifest.characters.insert(
            id,
            Placement {
                principal,
                instance: lease.instance,
                actor: life.actor,
            },
        );
        self.manifest.next_character = next;
        self.publish(&[lease.instance])?;
        Ok((id, life))
    }
    /// Leases never renew implicitly; a stopped or expired holder cannot advance the world.
    pub fn acquire(&mut self, instance: u64, owner: [u8; 32], now: u64) -> Result<Lease, String> {
        self.clock(now)?;
        let slot = self
            .manifest
            .instances
            .get_mut(&instance)
            .ok_or("Realm instance is missing")?;
        if owner == [0; 32]
            || slot.phase == Phase::Stopped
            || slot.owner.is_some() && slot.expires_ms > now
        {
            return Err("Realm instance has a live authority or is stopped".into());
        }
        let epoch = slot
            .epoch
            .checked_add(1)
            .ok_or("Realm authority epochs exhausted")?;
        let expires = now
            .checked_add(LEASE_MS)
            .ok_or("Realm lease clock exhausted")?;
        self.games.get_mut(&instance).unwrap().close_all()?;
        slot.epoch = epoch;
        slot.owner = Some(owner);
        slot.expires_ms = expires;
        self.publish(&[instance])?;
        Ok(Lease {
            realm: self.manifest.id,
            run: self.run,
            instance,
            epoch,
            owner,
        })
    }
    /// Stops authority while preserving the instance's open or draining phase.
    pub fn release(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.check(lease, now)?;
        let epoch = self.manifest.instances[&lease.instance]
            .epoch
            .checked_add(1)
            .ok_or("Realm authority epochs exhausted")?;
        self.games.get_mut(&lease.instance).unwrap().close_all()?;
        let slot = self.manifest.instances.get_mut(&lease.instance).unwrap();
        slot.owner = None;
        slot.expires_ms = 0;
        slot.epoch = epoch;
        self.publish(&[lease.instance])
    }
    pub fn renew(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.check(lease, now)?;
        self.manifest
            .instances
            .get_mut(&lease.instance)
            .unwrap()
            .expires_ms = now
            .checked_add(LEASE_MS)
            .ok_or("Realm lease clock exhausted")?;
        self.publish(&[])
    }
    pub fn set_endpoint(
        &mut self,
        lease: &Lease,
        endpoint: SocketAddr,
        now: u64,
    ) -> Result<(), String> {
        self.check(lease, now)?;
        if endpoint.port() == 0 {
            return Err("Realm endpoint requires a nonzero port".into());
        }
        self.manifest
            .instances
            .get_mut(&lease.instance)
            .unwrap()
            .endpoint = endpoint;
        self.publish(&[])
    }
    pub fn route(&mut self, character: u64, now: u64) -> Result<Route, String> {
        self.clock(now)?;
        let placement = self
            .manifest
            .characters
            .get(&character)
            .ok_or("Realm character is missing")?;
        let slot = &self.manifest.instances[&placement.instance];
        if slot.phase != Phase::Open || slot.expires_ms <= now {
            return Err("Realm character has no reachable admitted authority".into());
        }
        Ok(Route {
            realm: self.manifest.id,
            instance: placement.instance,
            epoch: slot.epoch,
            owner: slot.owner.ok_or("Realm authority is missing")?,
            endpoint: slot.endpoint,
            expires_ms: slot.expires_ms,
        })
    }
    pub fn characters(&self) -> impl Iterator<Item = (u64, [u8; 32], LifeId)> + '_ {
        self.manifest.characters.iter().filter_map(|(id, c)| {
            self.games[&c.instance]
                .game()
                .player_admission(c.actor)
                .map(|a| (*id, c.principal, a.actor()))
        })
    }
    pub fn phase(&self, instance: u64) -> Option<Phase> {
        self.manifest.instances.get(&instance).map(|s| s.phase)
    }
    pub fn drain(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.check(lease, now)?;
        self.manifest
            .instances
            .get_mut(&lease.instance)
            .unwrap()
            .phase = Phase::Draining;
        self.publish(&[lease.instance])
    }
    pub fn stop(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.check(lease, now)?;
        self.games.get_mut(&lease.instance).unwrap().close_all()?;
        let slot = self.manifest.instances.get_mut(&lease.instance).unwrap();
        slot.phase = Phase::Stopped;
        slot.owner = None;
        slot.expires_ms = 0;
        slot.epoch = slot
            .epoch
            .checked_add(1)
            .ok_or("Realm authority epochs exhausted")?;
        self.publish(&[lease.instance])
    }
    /// Restarts the last sealed checkpoint with fresh connection and lease fences.
    pub fn restart(&mut self, instance: u64, now: u64) -> Result<(), String> {
        self.clock(now)?;
        let slot = self
            .manifest
            .instances
            .get(&instance)
            .ok_or("Realm instance is missing")?;
        if slot.owner.is_some() && slot.expires_ms > now {
            return Err("Cannot restart a live realm authority".into());
        }
        let game = disk::recover_game(self, instance)?;
        self.games.insert(instance, game);
        let slot = self.manifest.instances.get_mut(&instance).unwrap();
        slot.phase = Phase::Open;
        slot.owner = None;
        slot.expires_ms = 0;
        slot.epoch = slot
            .epoch
            .checked_add(1)
            .ok_or("Realm authority epochs exhausted")?;
        self.publish(&[instance])
    }
    /// Existing sessions can drain; new transports require an open instance.
    pub fn open_connection(
        &mut self,
        lease: &Lease,
        now: u64,
    ) -> Result<(ConnectionId, Vec<u8>), String> {
        self.check(lease, now)?;
        if self.phase(lease.instance) != Some(Phase::Open) {
            return Err("Realm instance is draining".into());
        }
        self.games.get_mut(&lease.instance).unwrap().open_json(now)
    }
    pub fn dispatch(
        &mut self,
        lease: &Lease,
        id: ConnectionId,
        now: u64,
        bytes: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.check(lease, now)?;
        let request = Request::decode(bytes)?;
        if matches!(request.body, Body::Authenticate { .. })
            && self.phase(lease.instance) != Some(Phase::Open)
        {
            return Err("Realm instance is draining".into());
        }
        let mutating = !matches!(
            request.body,
            Body::Snapshot {}
                | Body::Replicate { .. }
                | Body::Inventory {}
                | Body::Services { .. }
                | Body::Safety {}
                | Body::Account {}
                | Body::Events { .. }
        );
        let service = self
            .services_request(lease, id, now, &request.body)
            .or_else(|| self.safety_request(lease, id, now, &request.body));
        let service_handled = service.is_some();
        let response = if let Some(result) =
            service.or_else(|| self.lifecycle_request(lease, id, now, &request.body))
        {
            if self.poisoned {
                return Err("Realm requires recovery after an uncertain commit".into());
            }
            let gateway = &self.games[&lease.instance];
            super::wire::Response {
                version: super::wire::VERSION,
                request_id: request.request_id,
                instance: lease.instance,
                tick: gateway.game().authority_tick,
                control: gateway.admission(id).ok().map(|a| super::wire::Control {
                    credit_step: gateway.game().physics_steps,
                    world_step: gateway.game().physics_steps,
                    life: a.actor().into(),
                    epoch: a.epoch(),
                    accepted_sequence: a.accepted_sequence(),
                    applied_movement: None,
                    dynamic: Vec::new(),
                }),
                body: result.unwrap_or_else(|message| super::wire::Reply::Refused {
                    code: if service_handled {
                        "game_services"
                    } else {
                        "character_lifecycle"
                    }
                    .into(),
                    message,
                }),
            }
            .encode()?
        } else {
            self.games
                .get_mut(&lease.instance)
                .unwrap()
                .dispatch_json(id, now, bytes)?
        };
        if mutating && !service_handled || self.dirty.contains(&lease.instance) {
            self.publish(&[lease.instance])?;
        }
        Ok(response)
    }
    pub fn close_connection(
        &mut self,
        lease: &Lease,
        id: ConnectionId,
        now: u64,
    ) -> Result<(), String> {
        self.check(lease, now)?;
        self.games.get_mut(&lease.instance).unwrap().close(id)?;
        self.publish(&[lease.instance])
    }
    pub fn tick(&mut self, lease: &Lease, now: u64, dt: f32) -> Result<(), String> {
        self.check(lease, now)?;
        self.dirty.insert(lease.instance);
        let result = self.games.get_mut(&lease.instance).unwrap().tick(dt);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// A trusted host submits an owned reward; clients cannot choose its amounts.
    pub fn grant_reward(
        &mut self,
        lease: &Lease,
        character: u64,
        transaction: super::rewards::Transaction,
        now: u64,
    ) -> Result<super::rewards::Receipt, String> {
        self.check(lease, now)?;
        let c = self
            .manifest
            .characters
            .get(&character)
            .ok_or("Realm character is missing")?;
        if c.instance != lease.instance
            || transaction.instance != c.instance
            || transaction.actor != c.actor
        {
            return Err("Realm reward placement is stale or foreign".into());
        }
        let receipt = self
            .games
            .get_mut(&lease.instance)
            .unwrap()
            .grant_reward(transaction)?;
        self.publish(&[lease.instance])?;
        Ok(receipt)
    }
    /// A local host may publish public seat poses under its current instance lease.
    pub fn publish_social_studio(
        &mut self,
        lease: &Lease,
        actors: Vec<crate::play::social::SeatActor>,
        now: u64,
    ) -> Result<(), String> {
        self.check(lease, now)?;
        self.games
            .get_mut(&lease.instance)
            .unwrap()
            .publish_social_studio(actors)?;
        self.publish(&[lease.instance])
    }
    pub fn checkpoint(&mut self, lease: &Lease, now: u64) -> Result<(), String> {
        self.check(lease, now)?;
        self.publish(&[lease.instance])
    }
}

#[cfg(test)]
mod tests;
