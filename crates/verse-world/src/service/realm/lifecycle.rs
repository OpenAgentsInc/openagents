//! Lease-scoped ownership recovery and durable resident retirement.
use super::*;
use registry::{Record, account_key, credential_key};
impl Realm {
    fn publish_lifecycle(&mut self, instances: &[u64]) -> Result<(), String> {
        self.lifecycle_commit = true;
        let result = self.publish(instances);
        self.lifecycle_commit = false;
        result
    }

    pub(super) fn owned(&self, character: u64, principal: [u8; 32]) -> Result<Character, String> {
        let saved = self.character(character)?;
        let account = self
            .account_for_key(principal)?
            .ok_or("Realm account is missing")?;
        if saved.account != account.id {
            return Err("Realm character belongs to another account".into());
        }
        Ok(saved)
    }
    /// Removes a resident after saving its resources and private receipt namespace.
    /// A network adapter must derive the principal from its verified connection.
    pub fn logout(
        &mut self,
        lease: &Lease,
        principal: [u8; 32],
        character: u64,
        now: u64,
    ) -> Result<(), String> {
        self.check(lease, now)?;
        let mut saved = self.owned(character, principal)?;
        let (instance, actor) = match saved.residence {
            Residence::Resident { instance, actor } if instance == lease.instance => {
                (instance, actor)
            }
            Residence::Dormant { instance, .. } if instance == lease.instance => return Ok(()),
            _ => return Err("Realm logout instance is incompatible".into()),
        };
        let original = &self.games[&instance];
        let snapshot = disk::store_snapshot(self, &original.checkpoint()?)?;
        let content = original.content().ok_or("Realm content is missing")?;
        let mut candidate = original.fork();
        if candidate
            .chamber
            .grants
            .contains_key(&super::super::Principal(principal))
        {
            candidate.revoke(principal)?;
        }
        candidate.chamber.game.take_resident_player(actor)?;
        let (book, _) = candidate.chamber.rewards.take_book(actor)?;
        if book.character != character {
            return Err("Realm logout receipt identity differs".into());
        }
        candidate
            .chamber
            .owners
            .remove(&super::super::Principal(principal));
        super::super::save::decode_with_history(
            &candidate.checkpoint()?,
            content,
            instance,
            Some(self.history.clone()),
        )?;
        saved.residence = Residence::Dormant {
            instance,
            actor,
            content,
            snapshot,
        };
        let root = registry::put(
            self,
            self.manifest.registry_root,
            vec![Record::Character(saved)],
        )?;
        self.games.insert(instance, candidate);
        self.manifest.characters.remove(&character);
        self.manifest.registry_root = root;
        self.publish_lifecycle(&[instance])
    }
    /// Restores an explicitly selected owned character at a host-checked spawn.
    pub fn resume(
        &mut self,
        lease: &Lease,
        principal: [u8; 32],
        character: u64,
        spawn: [f32; 3],
        now: u64,
    ) -> Result<LifeId, String> {
        self.check(lease, now)?;
        if spawn.iter().any(|v| !v.is_finite() || v.abs() > 10_000.) {
            return Err("Realm resume spawn is invalid".into());
        }
        let mut saved = self.owned(character, principal)?;
        let (instance, actor, content, snapshot) = match saved.residence {
            Residence::Resident { instance, actor } if instance == lease.instance => {
                let g = &self.games[&instance];
                if g.game().player_spawn(actor) != Some(spawn.into()) {
                    return Err("Realm resume retry differs from placement".into());
                }
                return Ok(g
                    .game()
                    .player_admission(actor)
                    .ok_or("Resident life is missing")?
                    .actor());
            }
            Residence::Dormant {
                instance,
                actor,
                content,
                snapshot,
            } => (instance, actor, content, snapshot),
            _ => return Err("Realm character is resident in another instance".into()),
        };
        let slot = &self.manifest.instances[&lease.instance];
        if slot.phase != Phase::Open
            || self.manifest.characters.len() >= CHARACTERS
            || self.games[&lease.instance].chamber.owners.len() >= slot.capacity as usize
            || self
                .manifest
                .characters
                .values()
                .any(|p| p.principal == principal)
        {
            return Err("Realm resume capacity or active account is incompatible".into());
        }
        let bytes = disk::read(
            &self
                .root
                .join("snapshots")
                .join(format!("{}.json", disk::hex(&snapshot))),
            super::super::save::MAX_BYTES,
        )?;
        if disk::digest(&bytes) != snapshot {
            return Err("Dormant character snapshot digest mismatch".into());
        }
        let mut archive = super::super::save::decode_with_history(
            &bytes,
            content,
            instance,
            Some(self.history.clone()),
        )?;
        let original = &self.games[&lease.instance];
        let catalogs = |g: &Gateway| {
            serde_json::to_vec(&(
                &g.chamber.items,
                &g.chamber.outfits,
                &g.chamber.equipment,
                &g.chamber.progression,
            ))
            .map_err(|_| "Cannot compare character catalogs")
        };
        if catalogs(&archive)? != catalogs(original)? {
            return Err("Dormant character catalogs require migration".into());
        }
        let portable = archive.chamber.game.take_resident_player(actor)?;
        let (book, state) = archive.chamber.rewards.take_book(actor)?;
        if book.character != character {
            return Err("Dormant character receipt identity differs".into());
        }
        let mut candidate = original.fork();
        let identity = super::super::Principal(principal);
        if matches!(
            candidate.chamber.grants.get(&identity),
            Some(super::super::Rights::Spectator)
        ) {
            candidate.chamber.grants.remove(&identity);
        }
        candidate.account_observers.remove(&identity);
        let life = candidate.enroll_player(principal, spawn.into())?;
        candidate.chamber.game.put_transfer_player(life, portable)?;
        candidate
            .chamber
            .rewards
            .put_book(life.actor, book, state)?;
        super::super::save::decode_with_history(
            &candidate.checkpoint()?,
            slot.content,
            lease.instance,
            Some(self.history.clone()),
        )?;
        saved.residence = Residence::Resident {
            instance: lease.instance,
            actor: life.actor,
        };
        let root = registry::put(
            self,
            self.manifest.registry_root,
            vec![Record::Character(saved)],
        )?;
        self.games.insert(lease.instance, candidate);
        self.manifest.characters.insert(
            character,
            Placement {
                principal,
                instance: lease.instance,
                actor: life.actor,
            },
        );
        self.manifest.registry_root = root;
        self.publish_lifecycle(&[lease.instance])?;
        Ok(life)
    }
    /// Recovers an account through trusted operator authority, never a client body.
    /// Every resident world must have a current lease; old sessions are retired.
    pub fn recover_account(
        &mut self,
        leases: &[Lease],
        account: u64,
        expected_epoch: u64,
        new_key: [u8; 32],
        now: u64,
    ) -> Result<Account, String> {
        self.clock(now)?;
        super::super::auth::valid_principal(new_key)?;
        let mut saved = self.account(account)?;
        if saved.epoch != expected_epoch || saved.key == new_key {
            return Err("Realm recovery epoch or key is incompatible".into());
        }
        if registry::get(self, self.manifest.registry_root, credential_key(new_key))?.is_some() {
            return Err("Realm recovery key is already recorded".into());
        }
        let instances: BTreeSet<_> = saved
            .characters
            .iter()
            .filter_map(|id| self.manifest.characters.get(id).map(|p| p.instance))
            .collect();
        for instance in &instances {
            let lease = leases
                .iter()
                .find(|l| l.instance == *instance)
                .ok_or("Realm recovery lacks a resident lease")?;
            self.check(lease, now)?;
        }
        // A dormant-only account still requires a lease from this realm.
        if leases.is_empty() {
            return Err("Realm recovery requires operator lease authority".into());
        }
        for lease in leases {
            self.check(lease, now)?;
        }
        let old_key = saved.key;
        let old_epoch = saved.epoch;
        saved.epoch = old_epoch
            .checked_add(1)
            .ok_or("Realm account epochs exhausted")?;
        saved.key = new_key;
        let mut candidates = BTreeMap::new();
        for instance in &instances {
            let mut candidate = self.games[instance].fork();
            let principal = super::super::Principal(old_key);
            let actor = *candidate
                .chamber
                .owners
                .get(&principal)
                .ok_or("Recovery resident owner is missing")?;
            if candidate.chamber.grants.contains_key(&principal) {
                candidate.revoke(old_key)?;
            }
            candidate.chamber.owners.remove(&principal);
            candidate
                .chamber
                .owners
                .insert(super::super::Principal(new_key), actor);
            candidate.chamber.grants.insert(
                super::super::Principal(new_key),
                super::super::Rights::Player(actor),
            );
            super::super::save::decode_with_history(
                &candidate.checkpoint()?,
                candidate.content().unwrap(),
                *instance,
                Some(self.history.clone()),
            )?;
            candidates.insert(*instance, candidate);
        }
        let root = registry::put(
            self,
            self.manifest.registry_root,
            vec![
                Record::Account(saved.clone()),
                Record::Credential {
                    key: old_key,
                    account,
                    epoch: old_epoch,
                    current: false,
                },
                Record::Credential {
                    key: new_key,
                    account,
                    epoch: saved.epoch,
                    current: true,
                },
            ],
        )?;
        self.games.extend(candidates);
        for id in &saved.characters {
            if let Some(p) = self.manifest.characters.get_mut(id) {
                p.principal = new_key;
            }
        }
        self.manifest.registry_root = root;
        self.publish_lifecycle(&instances.into_iter().collect::<Vec<_>>())?;
        // Read back the selected account so future callers use its new epoch.
        match registry::get(self, root, account_key(account))? {
            Some(Record::Account(a)) => Ok(a),
            _ => Err("Recovered account is missing".into()),
        }
    }
}

impl Realm {
    pub(super) fn lifecycle_request(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        now: u64,
        body: &Body,
    ) -> Option<Result<super::super::wire::Reply, String>> {
        use super::super::wire::Reply;
        match body {
            Body::Account {} => Some((|| {
                let principal = self.games[&lease.instance].principal(connection)?.0;
                Ok(Reply::Account {
                    account: self
                        .account_for_key(principal)?
                        .ok_or("Principal has no realm account")?,
                })
            })()),
            Body::Authenticate {
                public_key,
                signature,
            } => Some((|| {
                let signature = <[u8; 64]>::try_from(signature.as_slice()).unwrap_or([0; 64]);
                let proof = self.games.get_mut(&lease.instance).unwrap().verify(
                    connection,
                    now,
                    *public_key,
                    signature,
                )?;
                let registered = self.account_for_key(*public_key)?.is_some();
                if !registered
                    && !self.games[&lease.instance]
                        .chamber
                        .grants
                        .contains_key(&proof.principal())
                    && let Some(guests) = self.games[&lease.instance].guests().cloned()
                {
                    let taken = self.games[&lease.instance].guest_count();
                    if taken >= guests.cap as usize {
                        return Err("Chamber guest capacity exceeded".into());
                    }
                    let mut admitted = false;
                    let mut last = String::new();
                    for attempt in 0..guests.cap as usize * 2 {
                        match self.admit(
                            lease,
                            *public_key,
                            guests.spawn(taken + attempt).to_array(),
                            now,
                        ) {
                            Ok(_) => {
                                admitted = true;
                                break;
                            }
                            Err(error) if self.poisoned => return Err(error),
                            Err(error) => last = error,
                        }
                    }
                    if !admitted {
                        return Err(format!("Chamber guest admission refused: {last}"));
                    }
                }
                let gateway = self.games.get_mut(&lease.instance).unwrap();
                if registered && !gateway.chamber.grants.contains_key(&proof.principal()) {
                    gateway.enroll_spectator(*public_key)?;
                    gateway.account_observers.insert(proof.principal());
                }
                gateway.bind_verified(proof)?;
                Ok(Reply::Accepted)
            })()),
            Body::SelectCharacter { character } => Some((|| {
                let gateway = &self.games[&lease.instance];
                let principal = gateway.principal(connection)?.0;
                if gateway.admission(connection).is_ok() {
                    return Err("Logout the active character before selection".into());
                }
                let spawn = gateway.game().account_spawn()?.to_array();
                self.resume(lease, principal, *character, spawn, now)?;
                self.games
                    .get_mut(&lease.instance)
                    .unwrap()
                    .refresh_binding(connection)?;
                Ok(Reply::CharacterSelected {
                    character: *character,
                })
            })()),
            Body::Logout { life, epoch } => Some((|| {
                let gateway = &self.games[&lease.instance];
                let principal = gateway.principal(connection)?.0;
                let admission = gateway.admission(connection)?;
                if admission.actor() != (*life).into() || admission.epoch() != *epoch {
                    return Err("Logout control is stale or foreign".into());
                }
                let character = self
                    .manifest
                    .characters
                    .iter()
                    .find(|(_, p)| {
                        p.principal == principal
                            && p.instance == lease.instance
                            && p.actor == life.actor
                    })
                    .map(|(id, _)| *id)
                    .ok_or("Logout character is missing")?;
                self.logout(lease, principal, character, now)?;
                Ok(Reply::LoggedOut { character })
            })()),
            _ => None,
        }
    }
}
