//! Reviewed offline transitions with retained backups and exclusive writer ownership.
use super::{Committed, Store, digest, journal, regular_or_absent};
use crate::{
    play::Game,
    service::{
        Chamber, Principal, Rights,
        auth::Gateway,
        host::{Config, Role, public_key},
        save,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const META_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub content: [u8; 32],
    pub rules: String,
    pub character_schema: u16,
    pub save_version: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentChange {
    pub key: [u8; 32],
    pub retained_actor: Option<u64>,
    pub before_spawn: Option<[f32; 3]>,
    pub after_spawn: Option<[f32; 3]>,
    pub before_actor: Option<u64>,
    pub after_actor: Option<u64>,
    pub before_enrolled: bool,
    pub after_enrolled: bool,
}
/// Review this document before passing it to `Store::apply_migration`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub version: u16,
    pub source: Identity,
    pub target: Identity,
    pub source_revision: u64,
    pub source_hash: [u8; 32],
    pub target_hash: [u8; 32],
    pub source_config: [u8; 32],
    pub target_config: [u8; 32],
    pub characters: usize,
    pub reward_revision: u64,
    pub enrollments: Vec<EnrollmentChange>,
    /// World dynamics and encounters restart; persistent characters retain their records.
    pub restart_world_and_respawn: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub version: u16,
    pub id: String,
    pub rollback_of: Option<String>,
    pub review: Review,
    pub before_revision: u64,
    pub before_digest: [u8; 32],
    pub after_revision: u64,
    pub after_digest: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Seal {
    record: [u8; 32],
    applied: bool,
}
fn hashed<T: Serialize>(value: &T) -> Result<[u8; 32], String> {
    Ok(
        Sha256::digest(serde_json::to_vec(value).map_err(|_| "Cannot encode migration metadata")?)
            .into(),
    )
}
fn identity(state: &serde_json::Value) -> Result<Identity, String> {
    Ok(Identity {
        content: serde_json::from_value(state["content"].clone())
            .map_err(|_| "Missing migration content identity")?,
        rules: state["world"]["rules_revision"]
            .as_str()
            .ok_or("Missing migration rules revision")?
            .into(),
        character_schema: state["character_schema"]
            .as_u64()
            .unwrap_or(1)
            .try_into()
            .map_err(|_| "Invalid character schema")?,
        save_version: state["version"]
            .as_u64()
            .ok_or("Missing save version")?
            .try_into()
            .map_err(|_| "Invalid save version")?,
    })
}
fn retained(source: &Config, target: &Config, gateway: &Gateway) -> Result<(), String> {
    for old in &source.items.items {
        target.items.item(old.id)?;
    }
    for old in &source.outfits.outfits {
        target.outfits.outfit(old.id)?;
    }
    for old in &source.equipment.gear {
        if target.equipment.item(old.id)?.slot != old.slot {
            return Err("Migration cannot change an existing equipment slot".into());
        }
    }
    for old in &source.progression.quests {
        let new = target
            .progression
            .quests
            .iter()
            .find(|q| q.id == old.id)
            .ok_or("Migration must retain existing quest IDs")?;
        for (life, _, _) in gateway.game().controlled_effects() {
            let Some(character) = gateway.character_rewards(life.actor) else {
                continue;
            };
            let active = !character.claimed_quests.contains(&old.id)
                && ((old.giver.is_none()
                    && character.quests.get(&old.objective).copied().unwrap_or(0) > 0)
                    || character.accepted_quests.contains_key(&old.id));
            if active
                && (old.objective != new.objective
                    || new.goal > old.goal
                    || old.giver != new.giver
                    || old.prerequisites != new.prerequisites)
            {
                return Err("Active quest semantics require an explicit progress adapter".into());
            }
        }
    }
    Ok(())
}
impl Store {
    fn migration_source(&self) -> Result<Gateway, String> {
        if self.poisoned || self.recovered.is_none() {
            return Err(
                "Migration requires a freshly opened offline store with pending recovery".into(),
            );
        }
        save::decode_with_history(
            &journal::contract(
                self.state
                    .as_ref()
                    .ok_or("Migration requires a committed source")?,
            )?,
            self.content,
            self.instance,
            Some(self.history.clone()),
        )
    }
    fn candidate(
        &self,
        source: &Config,
        target: &Config,
        target_game: Game,
        target_content: [u8; 32],
    ) -> Result<(Review, String), String> {
        source.validate()?;
        target.validate()?;
        let configured_root = source
            .state_dir
            .as_ref()
            .ok_or("Migration requires a configured durable storage directory")?
            .canonicalize()
            .map_err(|_| "Cannot resolve configured migration storage")?;
        let opened_root = self
            .root
            .canonicalize()
            .map_err(|_| "Cannot resolve opened migration storage")?;
        if source.state_dir != target.state_dir
            || configured_root != opened_root
            || source.instance != target.instance
            || source.instance != self.instance
            || target_content == [0; 32]
        {
            return Err("Migration must retain the storage directory and instance".into());
        }
        let old = self.migration_source()?;
        source.validate_recovered(&old)?;
        retained(source, target, &old)?;
        let mut spawns = BTreeMap::new();
        for enrollment in &target.enrollments {
            let key = Principal(public_key(&enrollment.public_key)?);
            if let (Some(actor), Role::Player { spawn }) =
                (old.chamber.owners.get(&key), &enrollment.role)
            {
                spawns.insert(*actor, (*spawn).into());
            }
        }
        let game = target_game.migrate_content(old.game(), &spawns)?;
        let mut chamber = Chamber::new(game)?;
        chamber.owners = old.chamber.owners.clone();
        chamber.rewards = old.chamber.rewards.clone();
        chamber.rewards.attach(self.history.clone())?;
        let mut gateway = Gateway::new(chamber)?;
        for enrollment in &target.enrollments {
            let key = public_key(&enrollment.public_key)?;
            match enrollment.role {
                Role::Primary {} => gateway.enroll_primary(key)?,
                Role::Player { spawn } => {
                    gateway.enroll_player(key, spawn.into())?;
                }
                Role::Spectator {} => gateway.enroll_spectator(key)?,
            }
        }
        gateway = gateway
            .with_rewards(target.rewards.clone())?
            .with_progression(target.progression.clone())?
            .with_items(target.items.clone())?
            .with_outfits(target.outfits.clone())?
            .with_equipment(target.equipment.clone())?
            .with_content(target_content)?;
        let lives: Vec<_> = gateway
            .game()
            .controlled_effects()
            .map(|(life, _, _)| life)
            .collect();
        for life in &lives {
            let character = gateway
                .character_rewards(life.actor)
                .cloned()
                .unwrap_or_default();
            let (hp, mana) = gateway.chamber.resource_limits(life.actor, &character)?;
            gateway
                .chamber
                .game
                .equipment_limits(life.actor, hp, mana)?;
            let resources = gateway.game().player_snapshot(*life)?.player;
            if resources.hp < hp || resources.mana < mana {
                gateway.chamber.game.recover_player_resources(
                    life.actor,
                    hp as u32,
                    mana as u32,
                )?;
            }
        }
        target.validate_recovered(&gateway)?;
        let checkpoint = String::from_utf8(gateway.checkpoint()?)
            .map_err(|_| "Cannot encode migration candidate")?;
        // Validation uses the original receipt index instead of replaying it under new definitions.
        save::decode_with_history(
            checkpoint.as_bytes(),
            target_content,
            self.instance,
            Some(self.history.clone()),
        )?;
        let state = journal::expand(checkpoint.as_bytes())?;
        let keys: std::collections::BTreeSet<_> = old
            .chamber
            .grants
            .keys()
            .chain(gateway.chamber.grants.keys())
            .copied()
            .collect();
        let enrollments = keys
            .into_iter()
            .filter_map(|key| {
                let before = old.chamber.grants.get(&key);
                let after = gateway.chamber.grants.get(&key);
                let actor = |right: Option<&Rights>| match right {
                    Some(Rights::Player(actor)) => Some(*actor),
                    _ => None,
                };
                let before_spawn = actor(before)
                    .and_then(|id| old.game().player_spawn(id))
                    .map(|v| v.to_array());
                let after_spawn = actor(after)
                    .and_then(|id| gateway.game().player_spawn(id))
                    .map(|v| v.to_array());
                (actor(before) != actor(after)
                    || before.is_some() != after.is_some()
                    || before_spawn != after_spawn)
                    .then_some(EnrollmentChange {
                        key: key.0,
                        retained_actor: gateway.chamber.owners.get(&key).copied(),
                        before_spawn,
                        after_spawn,
                        before_actor: actor(before),
                        after_actor: actor(after),
                        before_enrolled: before.is_some(),
                        after_enrolled: after.is_some(),
                    })
            })
            .collect();
        let review = Review {
            version: 1,
            source: identity(self.state.as_ref().unwrap())?,
            target: identity(&state)?,
            source_revision: self.revision,
            source_hash: self.last_hash.ok_or("Missing source digest")?,
            target_hash: journal::hash(&state)?,
            source_config: hashed(source)?,
            target_config: hashed(target)?,
            characters: lives.len(),
            reward_revision: gateway.chamber.rewards.revision(),
            enrollments,
            restart_world_and_respawn: true,
        };
        Ok((review, checkpoint))
    }
    /// Validates retained source assets using the same scene check as normal startup.
    pub fn validate_migration_source(
        &self,
        source: &Config,
        prepared: &Game,
    ) -> Result<(), String> {
        source.validate_recovered_scene(&self.migration_source()?, prepared)
    }
    /// Builds and validates a candidate without publishing it or changing the source snapshot.
    pub fn plan_migration(
        &self,
        source: &Config,
        target: &Config,
        target_game: Game,
        target_content: [u8; 32],
    ) -> Result<Review, String> {
        self.candidate(source, target, target_game, target_content)
            .map(|(review, _)| review)
    }
    /// Applies only the exact source, configuration, and candidate pinned by the reviewed plan.
    pub fn apply_migration(
        &mut self,
        source: &Config,
        target: &Config,
        target_game: Game,
        target_content: [u8; 32],
        reviewed: &Review,
    ) -> Result<Record, String> {
        let (review, checkpoint) = self.candidate(source, target, target_game, target_content)?;
        if &review != reviewed {
            return Err("Reviewed migration no longer matches the source or candidate".into());
        }
        self.transition(review, checkpoint, None)
    }
    /// Rollback is refused after any later commit, including a host startup commit.
    pub fn rollback_migration(&mut self, id: &str) -> Result<Record, String> {
        self.migration_source()?;
        let (record, before, after) = load_record(&self.root, id)?;
        if record.rollback_of.is_some()
            || !sealed(&self.root.join("migrations").join(id), &record)?
            || self.revision != after.revision
            || self.last_hash
                != Some(journal::hash(&journal::expand(
                    after.checkpoint.as_bytes(),
                )?)?)
        {
            return Err("Rollback requires the unchanged applied migration; later commits cannot be discarded".into());
        }
        let mut review = record.review;
        std::mem::swap(&mut review.source, &mut review.target);
        std::mem::swap(&mut review.source_config, &mut review.target_config);
        review.restart_world_and_respawn = false;
        let original = save::decode_with_history(
            before.checkpoint.as_bytes(),
            review.target.content,
            self.instance,
            Some(self.history.clone()),
        )?;
        review.characters = original.game().controlled_effects().count();
        review.source_revision = self.revision;
        review.source_hash = self.last_hash.unwrap();
        review.target_hash = journal::hash(&journal::expand(before.checkpoint.as_bytes())?)?;
        for change in &mut review.enrollments {
            std::mem::swap(&mut change.before_spawn, &mut change.after_spawn);
            std::mem::swap(&mut change.before_actor, &mut change.after_actor);
            std::mem::swap(&mut change.before_enrolled, &mut change.after_enrolled);
        }
        self.transition(review, before.checkpoint, Some(id.into()))
    }
    fn transition(
        &mut self,
        review: Review,
        checkpoint: String,
        rollback_of: Option<String>,
    ) -> Result<Record, String> {
        let before_checkpoint = String::from_utf8(journal::contract(
            self.state.as_ref().ok_or("Missing migration source")?,
        )?)
        .map_err(|_| "Cannot encode migration source")?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Chamber commit revisions exhausted")?;
        let before = Committed {
            version: 1,
            revision: self.revision,
            digest: digest(self.revision, &before_checkpoint),
            checkpoint: before_checkpoint,
        };
        let after = Committed {
            version: 1,
            revision,
            digest: digest(revision, &checkpoint),
            checkpoint,
        };
        let state = journal::expand(after.checkpoint.as_bytes())?;
        let content = identity(&state)?.content;
        let gateway = save::decode_with_history(
            after.checkpoint.as_bytes(),
            content,
            self.instance,
            Some(self.history.clone()),
        )?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Migration clock precedes epoch")?
            .as_nanos();
        let id = hashed(&(nonce, &review, &rollback_of))?
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        let record = Record {
            version: 1,
            id: id.clone(),
            rollback_of,
            review,
            before_revision: before.revision,
            before_digest: before.digest,
            after_revision: after.revision,
            after_digest: after.digest,
        };
        let root = self.root.clone();
        let dir = root.join("migrations").join(&id);
        directory(&root.join("migrations"))?;
        std::fs::create_dir(&dir).map_err(|_| "Cannot create migration archive")?;
        secure_directory(&dir)?;
        write_new(&dir.join("before.json"), &before, super::FILE_BYTES)?;
        write_new(&dir.join("after.json"), &after, super::FILE_BYTES)?;
        write_new(&dir.join("record.json"), &record, META_BYTES)?;
        sync(&dir)?;
        sync(&root.join("migrations"))?;
        self.history.synchronize()?;
        write_atomic(&root.join("migration.pending"), &id, META_BYTES)?;
        sync(&root)?;
        let outcome = (|| {
            #[cfg(test)]
            self.boundary("migration_prepared");
            self.snapshot(after.revision, after.checkpoint.clone())?;
            #[cfg(test)]
            self.boundary("migration_snapshot");
            self.journal
                .set_len(0)
                .and_then(|_| self.journal.sync_all())
                .map_err(|_| "Cannot clear migration journal")?;
            #[cfg(test)]
            self.boundary("migration_journal");
            write_atomic(
                &dir.join("seal.json"),
                &Seal {
                    record: hashed(&record)?,
                    applied: true,
                },
                META_BYTES,
            )?;
            sync(&dir)?;
            #[cfg(test)]
            self.boundary("migration_sealed");
            std::fs::remove_file(root.join("migration.pending"))
                .map_err(|_| "Cannot clear migration marker")?;
            sync(&root)
        })();
        if let Err(error) = outcome {
            self.poisoned = true;
            // The retained marker lets the next locked open choose the sealed target or source.
            return Err(error);
        }
        self.content = content;
        self.revision = revision;
        self.last_hash = Some(journal::hash(&state)?);
        self.state = Some(state);
        self.recovered = Some(gateway);
        self.owner = self.recovered.as_ref().map(Gateway::server_identity);
        self.records = 0;
        Ok(record)
    }
}
fn valid_id(id: &str) -> Result<(), String> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("Invalid migration archive identity".into());
    }
    Ok(())
}
fn read<T: serde::de::DeserializeOwned>(path: &Path, limit: usize) -> Result<T, String> {
    regular_or_absent(path)?;
    let mut bytes = vec![];
    File::open(path)
        .map_err(|_| "Cannot open migration record")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read migration record")?;
    if bytes.len() > limit {
        return Err("Migration record exceeds its byte budget".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid migration record".into())
}
fn write_new<T: Serialize>(path: &Path, value: &T, limit: usize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Cannot encode migration record")?;
    if bytes.len() > limit {
        return Err("Migration record exceeds its byte budget".into());
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "Cannot create migration record")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot sync migration record".into())
}
fn write_atomic<T: Serialize>(path: &Path, value: &T, limit: usize) -> Result<(), String> {
    let staged = path.with_extension("staged");
    regular_or_absent(path)?;
    regular_or_absent(&staged)?;
    if staged.exists() {
        std::fs::remove_file(&staged)
            .map_err(|_| "Cannot discard interrupted migration metadata")?;
    }
    write_new(&staged, value, limit)?;
    std::fs::rename(&staged, path).map_err(|_| "Cannot publish migration metadata")?;
    sync(
        path.parent()
            .ok_or("Missing migration metadata directory")?,
    )
}
fn sync(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync migration directory".into())
}
fn secure_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot secure migration directory")?;
    }
    Ok(())
}
fn directory(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err("Migration archive must be a directory".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(|_| "Cannot create migration directory")?;
            secure_directory(path)?;
            sync(path.parent().ok_or("Missing migration parent")?)
        }
        Err(_) => Err("Cannot inspect migration directory".into()),
    }
}
pub(super) fn load_record(root: &Path, id: &str) -> Result<(Record, Committed, Committed), String> {
    valid_id(id)?;
    let base = root.join("migrations");
    let dir = base.join(id);
    for path in [&base, &dir] {
        if !std::fs::symlink_metadata(path)
            .map_err(|_| "Missing migration archive")?
            .is_dir()
        {
            return Err("Invalid migration archive directory".into());
        }
    }
    let record: Record = read(&dir.join("record.json"), META_BYTES)?;
    let before: Committed = read(&dir.join("before.json"), super::FILE_BYTES)?;
    let after: Committed = read(&dir.join("after.json"), super::FILE_BYTES)?;
    if record.version != 1
        || record.id != id
        || record.before_revision != before.revision
        || record.after_revision != after.revision
        || record.before_digest != before.digest
        || record.after_digest != after.digest
        || after.revision
            != before
                .revision
                .checked_add(1)
                .ok_or("Migration revisions exhausted")?
    {
        return Err("Migration record identity is invalid".into());
    }
    for saved in [&before, &after] {
        if saved.version != 1
            || saved.revision == 0
            || saved.checkpoint.len() > save::MAX_BYTES
            || saved.digest != digest(saved.revision, &saved.checkpoint)
        {
            return Err("Migration backup checksum is invalid".into());
        }
    }
    if record.review.version != 1
        || record.review.source_revision != before.revision
        || journal::hash(&journal::expand(before.checkpoint.as_bytes())?)?
            != record.review.source_hash
        || journal::hash(&journal::expand(after.checkpoint.as_bytes())?)?
            != record.review.target_hash
        || identity(&journal::expand(before.checkpoint.as_bytes())?)? != record.review.source
        || identity(&journal::expand(after.checkpoint.as_bytes())?)? != record.review.target
    {
        return Err("Migration backup context is invalid".into());
    }
    Ok((record, before, after))
}
pub(super) fn sealed(dir: &Path, record: &Record) -> Result<bool, String> {
    let path = dir.join("seal.json");
    regular_or_absent(&path)?;
    if !path.exists() {
        return Ok(false);
    }
    let seal: Seal = read(&path, META_BYTES)?;
    if seal.record != hashed(record)? {
        return Err("Migration seal does not match its record".into());
    }
    Ok(seal.applied)
}
/// Runs after the writer lock is held, before selecting a content identity.
pub(super) fn recover_pending(root: &Path) -> Result<(), String> {
    let pending = root.join("migration.pending");
    regular_or_absent(&pending)?;
    if !pending.exists() {
        return Ok(());
    }
    let id: String = read(&pending, META_BYTES)?;
    let (record, before, after) = load_record(root, &id)?;
    let dir = root.join("migrations").join(&id);
    let applied = sealed(&dir, &record)?;
    let current: Committed = read(&root.join("chamber.json"), super::FILE_BYTES)?;
    if current.version != 1
        || current.digest != digest(current.revision, &current.checkpoint)
        || current.revision > after.revision
        || (current.revision == after.revision && current.digest != after.digest)
        || (current.revision <= before.revision
            && identity(&journal::expand(current.checkpoint.as_bytes())?)?.content
                != record.review.source.content)
    {
        return Err("Interrupted migration has an unrelated active snapshot".into());
    }
    let saved = if applied { after } else { before };
    let next = root.join("next.json");
    regular_or_absent(&next)?;
    if next.exists() {
        std::fs::remove_file(&next).map_err(|_| "Cannot discard interrupted migration snapshot")?;
    }
    regular_or_absent(&root.join("chamber.json"))?;
    write_new(&next, &saved, super::FILE_BYTES)?;
    std::fs::rename(next, root.join("chamber.json"))
        .map_err(|_| "Cannot recover migration snapshot")?;
    sync(root)?;
    let log = root.join("journal.jsonl");
    regular_or_absent(&log)?;
    OpenOptions::new()
        .write(true)
        .open(&log)
        .and_then(|f| {
            f.set_len(0)?;
            f.sync_all()
        })
        .map_err(|_| "Cannot recover migration journal")?;
    if !applied {
        write_atomic(
            &dir.join("seal.json"),
            &Seal {
                record: hashed(&record)?,
                applied: false,
            },
            META_BYTES,
        )?;
        sync(&dir)?;
    }
    std::fs::remove_file(pending).map_err(|_| "Cannot clear recovered migration marker")?;
    sync(root)
}
#[cfg(test)]
mod tests;
