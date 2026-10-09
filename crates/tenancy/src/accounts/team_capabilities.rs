//! Exact team release and input grants in the native account history.
//! Evaluation references describe a release; they never authorize execution.

use super::{Accounts, Lock, MemberRef, Role, Store, save, unix_now};
use crate::private_fs;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::File;

/// What a team writer holds to notice the account state changing under
/// it: the open file, or the database revision it read.
pub(super) enum StateHandle {
    File(File),
    #[cfg(feature = "postgres")]
    Revision(i64),
}
use std::io::Read;

pub const SCHEMA: &str = "openagents.team-capability.v1";
pub const READ: &str = "team-capabilities.read";
pub const REVIEW: &str = "team-capabilities.review";
pub const ENABLE: &str = "team-capabilities.enable";
pub const USE: &str = "team-capabilities.use";
const MAX_GRANTS: usize = 1024;
const MAX_HISTORY: usize = 128;
const MAX_EFFECTS: usize = 4096;

/// Current native source pins. The selected adapter must reconstruct these
/// from signed releases, verified artifact bytes, and scoped evaluation owners.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub source: String,
    pub catalog: String,
    pub package: String,
    pub publisher: String,
    pub release: String,
    pub manifest: String,
    pub version: String,
    pub component: String,
    pub program: String,
    pub operation: String,
    pub wasm: String,
    pub evaluations: Vec<String>,
    pub data_requirements: Vec<String>,
    pub source_recipients: Vec<String>,
}
impl Release {
    fn validate(&self) -> Result<(), String> {
        if self.source.is_empty()
            || self.source.len() > 8192
            || self.version.is_empty()
            || self.version.len() > 128
            || self.component.is_empty()
            || self.operation.is_empty()
            || self.component.len() > 128
            || self.operation.len() > 128
            || !hex(&self.publisher)
            || !hex(&self.release)
            || !self.package.starts_with(&format!("{}:", self.publisher))
            || self.package.len() > 200
            || self.evaluations.is_empty()
            || self.evaluations.len() > 32
            || self.evaluations.iter().any(|v| !hex(v))
            || [&self.catalog, &self.manifest, &self.program, &self.wasm]
                .iter()
                .any(|d| !digest_ref(d))
            || [&self.data_requirements, &self.source_recipients]
                .iter()
                .any(|items| {
                    items.len() > 32 || items.iter().any(|s| s.is_empty() || s.len() > 2048)
                })
        {
            return Err("Invalid exact team release pins.".into());
        }
        Ok(())
    }
}

/// A native adapter re-reads its explicit source on every new admission.
/// Implementations run inside the operator, not as caller-supplied remote proofs.
pub trait Sources {
    fn current(&mut self, previous: &[Value]) -> Result<Verified, String>;
    /// Validated signed observations survive refused source reads. The native
    /// adapter supplies these; callers cannot submit them as authority.
    fn knowledge(&self) -> Vec<Value> {
        Vec::new()
    }
}
pub struct Verified {
    pub release: Release,
    /// Full signed discovery knowledge prevents rollback on later source reads.
    pub evidence: Vec<Value>,
}
impl Verified {
    fn validate(&self) -> Result<(), String> {
        self.release.validate()?;
        if self.evidence.is_empty()
            || self.evidence.len() > 256
            || serde_json::to_vec(&self.evidence)
                .map_err(|e| e.to_string())?
                .len()
                > 4 * 1024 * 1024
        {
            return Err("Invalid retained team source evidence.".into());
        }
        Ok(())
    }
}

/// The reviewer explicitly accepts one member's bytes and local output recipient.
/// The selected adapter has no network or ambient workspace capability.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub id: String,
    pub workspace: String,
    pub member: String,
    pub release: Release,
    pub input: String,
    pub input_bytes: u64,
    pub recipient: String,
    pub purpose: String,
    pub expires_at: u64,
}
impl Request {
    pub fn digest(&self) -> String {
        digest(self)
    }
    fn validate(&self) -> Result<(), String> {
        self.release.validate()?;
        if self.schema != SCHEMA
            || !identifier(&self.id)
            || self.workspace.is_empty()
            || self.member.is_empty()
            || !digest_ref(&self.input)
            || self.input_bytes > 64 * 1024
            || self.recipient != format!("local-member:{}", self.member)
            || self.purpose.is_empty()
            || self.purpose.len() > 2048
        {
            return Err("Invalid exact team input grant.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub request: Request,
    pub revision: u64,
    pub supersedes: Option<String>,
    pub reviewer: String,
    pub membership_epoch: u64,
    /// The recipient membership when the reviewer grants this exact input.
    pub member_epoch: u64,
    pub active: bool,
    pub reviewed_at: u64,
    pub evidence: Vec<Value>,
    pub digest: String,
}
impl Revision {
    fn computed(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a revision serializes");
        value.as_object_mut().unwrap().remove("digest");
        digest(&value)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Install,
    Enable,
    Use,
}
impl Action {
    fn scope(&self) -> &str {
        if *self == Self::Use { USE } else { ENABLE }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    pub fingerprint: String,
    pub workspace: String,
    pub member: String,
    pub grant: String,
    pub action: Action,
    pub admitted_at: u64,
    pub account_revision: String,
    pub state: String,
    /// Receipt metadata contains digests, never approved input or guest output.
    pub receipt: Option<Value>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub grants: BTreeMap<String, Vec<Revision>>,
    pub effects: BTreeMap<String, Effect>,
    pub knowledge: BTreeMap<String, Vec<Value>>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.grants.is_empty() && self.effects.is_empty() && self.knowledge.is_empty()
    }
    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.grants.len() > MAX_GRANTS
            || self.effects.len() > MAX_EFFECTS
            || self.knowledge.len() > MAX_GRANTS
        {
            return Err("Team capability book limit exceeded.".into());
        }
        for (key, history) in &self.grants {
            if history.is_empty() || history.len() > MAX_HISTORY {
                return Err("Invalid team grant history.".into());
            }
            let mut previous: Option<&Revision> = None;
            for r in history {
                r.request.validate()?;
                if *key != grant_key(&r.request.workspace, &r.request.id)
                    || r.digest != r.computed()
                    || r.revision != previous.map_or(1, |p| p.revision + 1)
                    || r.supersedes.as_deref() != previous.map(|p| p.digest.as_str())
                    || !store.accounts.contains_key(&r.request.member)
                    || !store.accounts.contains_key(&r.reviewer)
                    || !store.workspaces.contains_key(&r.request.workspace)
                    || r.membership_epoch == 0
                    || r.member_epoch == 0
                    || store
                        .workspaces
                        .get(&r.request.workspace)
                        .is_some_and(|ws| {
                            r.membership_epoch > ws.members_epoch
                                || r.member_epoch > ws.members_epoch
                        })
                    || r.evidence.is_empty()
                    || r.evidence.len() > 256
                    || previous
                        .is_some_and(|p| p.request != r.request || p.member_epoch != r.member_epoch)
                {
                    return Err("Invalid team grant lineage.".into());
                }
                previous = Some(r);
            }
        }
        for (key, evidence) in &self.knowledge {
            if !digest_ref(key)
                || evidence.is_empty()
                || evidence.len() > 256
                || serde_json::to_vec(evidence)
                    .map_err(|e| e.to_string())?
                    .len()
                    > 4 * 1024 * 1024
            {
                return Err("Invalid retained team source knowledge.".into());
            }
        }
        for (id, e) in &self.effects {
            let original = self.grants.values().flatten().find(|r| r.digest == e.grant);
            if original
                .is_none_or(|r| r.request.workspace != e.workspace || r.request.member != e.member)
            {
                return Err("Team effect has no original member grant.".into());
            }
            if *id != grant_key(&e.workspace, &e.id)
                || !identifier(&e.id)
                || !digest_ref(&e.fingerprint)
                || !digest_ref(&e.grant)
                || !digest_ref(&e.account_revision)
                || !store.accounts.contains_key(&e.member)
                || !store.workspaces.contains_key(&e.workspace)
                || !matches!(e.state.as_str(), "unknown" | "complete")
                || (e.state == "complete") != e.receipt.is_some()
                || e.receipt
                    .as_ref()
                    .is_some_and(|r| serde_json::to_vec(r).map_or(true, |v| v.len() > 32 * 1024))
            {
                return Err("Invalid team effect history.".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ResultOfUse {
    pub effect: Effect,
    /// Present on the first successful dispatch only. Replay emits the receipt.
    pub output: Option<Value>,
    pub replayed: bool,
}
/// Current credential and membership rights; a read never grants an effect.
#[derive(Debug, Serialize)]
pub struct Permissions {
    pub account: String,
    pub member_epoch: u64,
    pub review: bool,
    pub enable: bool,
    pub use_capability: bool,
    /// Native action scopes remain separate from current effect admission.
    pub policy_blocks_new_effects: bool,
}
/// Native custody and fresh authority checks around the actual effect.
/// Completion checks custody only: expiry cannot relabel admitted work.
pub struct EffectFence<'a> {
    custody: &'a dyn Fn() -> Result<(), String>,
    authority: &'a dyn Fn() -> Result<(), String>,
    expires_at: u64,
}
impl EffectFence<'_> {
    pub fn before_effect(&self) -> Result<(), String> {
        (self.custody)()?;
        (self.authority)()?;
        if self.expires_at <= unix_now() {
            return Err("Team grant expired before the effect.".into());
        }
        Ok(())
    }
    pub fn after_effect(&self) -> Result<(), String> {
        (self.custody)()
    }
}
pub struct Completed {
    pub receipt: Value,
    pub output: Option<Value>,
}

fn hex(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest_ref(v: &str) -> bool {
    v.strip_prefix("sha256:").is_some_and(hex)
}
fn identifier(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn knowledge_key(workspace: &str, package: &str) -> String {
    digest(&json!({"workspace":workspace,"package":package}))
}
fn grant_key(workspace: &str, id: &str) -> String {
    format!("{workspace}/{id}")
}
pub fn digest<T: Serialize>(value: &T) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        Sha256::digest(
            super::canonicalize(&serde_json::to_value(value).expect("a value serializes"))
                .as_bytes()
        )
    )
}
pub fn bytes_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

impl Accounts {
    fn team_actor(
        &self,
        store: &Store,
        workspace: &str,
        token: &str,
        action: &str,
    ) -> Result<MemberRef, String> {
        let registry = crate::Registry::open(&self.dir).map_err(|e| e.to_string())?;
        let key = crate::keys::authenticate(&self.dir, registry.manifest(), token)
            .map_err(|e| e.to_string())?;
        if key
            .scopes
            .as_ref()
            .is_some_and(|s| !s.permits_action(action))
        {
            return Err("Credential lacks the selected team action.".into());
        }
        let ws = store
            .workspaces
            .get(workspace)
            .ok_or("Unknown team workspace.")?;
        if ws.tenant != key.tenant {
            return Err("Credential belongs to another workspace tenant.".into());
        }
        Self::principal_in_store(store, workspace, &format!("key:{}", key.key_id))
            .map_err(|e| e.to_string())
    }
    pub(super) fn team_state(&self) -> Result<(Store, StateHandle), String> {
        #[cfg(feature = "postgres")]
        if let Some(database) = crate::db::bound(&self.dir) {
            let (revision, _) = crate::db::docs::load(&database, &crate::db::docs::ACCOUNTS)
                .map_err(|e| e.to_string())?
                .ok_or("Native team account state is unavailable.")?;
            let store = super::load(&self.dir).map_err(|e| e.to_string())?;
            return Ok((store, StateHandle::Revision(revision)));
        }
        let mut file = private_fs::flags(
            std::fs::OpenOptions::new().read(true),
            private_fs::O_NOFOLLOW | private_fs::O_NONBLOCK,
        )
        .and_then(|options| options.open(self.dir.join(super::ACCOUNTS)))
        .map_err(|_| "Native team account state is unavailable or contains a symlink.")?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() || private_fs::nlink(&meta) != 1 || meta.len() > 16 * 1024 * 1024 {
            return Err("Native team account state is unsafe or exceeds 16 MiB.".into());
        }
        let mut text = String::new();
        file.by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 16 * 1024 * 1024 {
            return Err("Native team account state exceeds 16 MiB.".into());
        }
        let store = Store::parse(&text, "team account state")?;
        Ok((store, StateHandle::File(file)))
    }
    pub(super) fn team_check_state(&self, held: &StateHandle, digest: &str) -> Result<(), String> {
        let (current, file) = self.team_state()?;
        let same = match (held, &file) {
            (StateHandle::File(held), StateHandle::File(file)) => {
                let original = held.metadata().map_err(|e| e.to_string())?;
                let observed = file.metadata().map_err(|e| e.to_string())?;
                private_fs::same_file(&original, &observed)
            }
            #[cfg(feature = "postgres")]
            (StateHandle::Revision(held), StateHandle::Revision(now)) => held == now,
            #[allow(unreachable_patterns)]
            _ => false,
        };
        if !same || current.digest != digest {
            return Err("Account state custody changed outside its held writer.".into());
        }
        Ok(())
    }
    pub(super) fn team_commit(
        &self,
        lock: &Lock,
        held: &mut StateHandle,
        store: &mut Store,
    ) -> Result<(), String> {
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(held, &store.digest)?;
        store.supersedes = Some(store.digest.clone());
        store.sequence = store
            .sequence
            .checked_add(1)
            .ok_or("Account revision overflow.")?;
        store.seal();
        store.validate("team capabilities")?;
        if serde_json::to_string_pretty(store)
            .map_err(|e| e.to_string())?
            .len()
            >= 16 * 1024 * 1024
        {
            return Err("Team account checkpoint exceeds its 16 MiB reader bound.".into());
        }
        save(&self.dir, store).map_err(|e| e.to_string())?;
        let (written, file) = self.team_state()?;
        if written.digest != store.digest {
            return Err("Committed team account state changed.".into());
        }
        *held = file;
        lock.check().map_err(|e| e.to_string())
    }
    fn team_keep_knowledge(
        store: &mut Store,
        key: &str,
        evidence: Vec<Value>,
    ) -> Result<(), String> {
        let mut retained = BTreeMap::new();
        for event in store
            .team_capabilities
            .knowledge
            .get(key)
            .into_iter()
            .flatten()
            .chain(evidence.iter())
        {
            retained
                .entry(digest(event))
                .or_insert_with(|| event.clone());
        }
        let retained: Vec<Value> = retained.into_values().collect();
        if retained.is_empty()
            || retained.len() > 256
            || serde_json::to_vec(&retained)
                .map_err(|e| e.to_string())?
                .len()
                > 4 * 1024 * 1024
        {
            return Err("Retained team source history exceeds its bound.".into());
        }
        store
            .team_capabilities
            .knowledge
            .insert(key.into(), retained);
        Ok(())
    }
    fn team_retain_observation(
        &self,
        lock: &Lock,
        held: &mut StateHandle,
        store: &mut Store,
        key: &str,
        source: &dyn Sources,
    ) -> Result<(), String> {
        let evidence = source.knowledge();
        if evidence.is_empty() {
            return Ok(());
        }
        if evidence.len() > 256
            || serde_json::to_vec(&evidence)
                .map_err(|e| e.to_string())?
                .len()
                > 4 * 1024 * 1024
        {
            return Err("Refused team source evidence exceeds its bound.".into());
        }
        Self::team_keep_knowledge(store, key, evidence)?;
        self.team_commit(lock, held, store)
    }
    /// Member-scoped visibility. No installation, input read, or guest execution.
    pub fn team_list(&self, workspace: &str, token: &str) -> Result<Vec<Revision>, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, READ)?;
        self.team_check_state(&held, &store.digest)?;
        lock.check().map_err(|e| e.to_string())?;
        Ok(store
            .team_capabilities
            .grants
            .values()
            .filter_map(|h| h.last())
            .filter(|r| {
                r.request.workspace == workspace
                    && (actor.role >= Role::Admin || r.request.member == actor.account)
            })
            .cloned()
            .collect())
    }
    /// Inspect action rights without creating a grant or checkpoint.
    pub fn team_permissions(&self, workspace: &str, token: &str) -> Result<Permissions, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, READ)?;
        let permits = |action| {
            self.team_actor(&store, workspace, token, action)
                .is_ok_and(|current| current == actor)
        };
        let permissions = Permissions {
            account: actor.account.clone(),
            member_epoch: actor.epoch,
            review: actor.role >= Role::Admin && permits(REVIEW),
            enable: permits(ENABLE),
            use_capability: permits(USE),
            policy_blocks_new_effects: store.team_policies.current(workspace).is_some(),
        };
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(&held, &store.digest)?;
        if self.team_actor(&store, workspace, token, READ)? != actor {
            return Err("Team observation authority changed while inspecting rights.".into());
        }
        Ok(permissions)
    }
    /// Shared release cards exclude every member's input, purpose, source paths,
    /// and full retained evidence. This read grants no enablement or execution.
    pub fn team_cards(&self, workspace: &str, token: &str) -> Result<Vec<Value>, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, READ)?;
        self.team_check_state(&held, &store.digest)?;
        lock.check().map_err(|e| e.to_string())?;
        Ok(store.team_capabilities.grants.values().filter_map(|h|h.last())
            .filter(|r|r.request.workspace==workspace).map(|r|json!({
                "package":r.request.release.package,"publisher":r.request.release.publisher,
                "release":r.request.release.release,"manifest":r.request.release.manifest,
                "version":r.request.release.version,"component":r.request.release.component,
                "operation":r.request.release.operation,"evaluations":r.request.release.evaluations,
                "state":if !r.active {"withdrawn"}else if r.request.expires_at<=unix_now(){"expired"}else if store.team_policies.current(workspace).is_some(){"policy_blocked"}else{"reviewed_release"},
                "own_input_grant":r.request.member==actor.account,
                "execution_authorized":false
            })).collect())
    }
    /// An owner or admin reviews exact source pins and independently granted data.
    /// Reconstruct a selected public source with the workspace's retained
    /// signed knowledge. Observation grants no installation or execution.
    pub fn team_source(
        &self,
        workspace: &str,
        token: &str,
        package: &str,
        sources: &mut dyn Sources,
    ) -> Result<Verified, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, READ)?;
        let verified = sources.current(
            store
                .team_capabilities
                .knowledge
                .get(&knowledge_key(workspace, package))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        )?;
        verified.validate()?;
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(&held, &store.digest)?;
        if self.team_actor(&store, workspace, token, READ)? != actor {
            return Err("Team observation authority changed during source verification.".into());
        }
        if verified.release.package != package {
            return Err("Selected team source belongs to another package.".into());
        }
        Ok(verified)
    }
    pub fn team_grant(
        &self,
        token: &str,
        request: Request,
        approved: &str,
        sources: &mut dyn Sources,
    ) -> Result<Revision, String> {
        request.validate()?;
        if approved != request.digest() {
            return Err("Approval does not match the exact team request.".into());
        }
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let actor = self.team_actor(&store, &request.workspace, token, REVIEW)?;
        if actor.role < Role::Admin {
            return Err("Team grants require a current owner or admin.".into());
        }
        let member_epoch = super::active_member(
            store.workspaces.get(&request.workspace).unwrap(),
            &request.member,
        )
        .map_err(|e| e.to_string())?
        .epoch;
        let key = grant_key(&request.workspace, &request.id);
        if let Some(old) = store
            .team_capabilities
            .grants
            .get(&key)
            .and_then(|h| h.last())
        {
            if old.request == request && old.active && old.member_epoch == member_epoch {
                return Ok(old.clone());
            }
            return Err("A team grant ID is immutable; review a new ID for changed scope.".into());
        }
        if store.team_policies.current(&request.workspace).is_some() {
            return Err("New local team plugin approvals are not qualified under this workspace team policy; inspect the original grants instead.".into());
        }
        let knowledge = knowledge_key(&request.workspace, &request.release.package);
        let verified = match sources.current(
            store
                .team_capabilities
                .knowledge
                .get(&knowledge)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        ) {
            Ok(value) => value,
            Err(error) => {
                self.team_actor(&store, &request.workspace, token, REVIEW)?;
                self.team_retain_observation(&lock, &mut held, &mut store, &knowledge, sources)?;
                return Err(error);
            }
        };
        verified.validate()?;
        lock.check().map_err(|e| e.to_string())?;
        if self.team_actor(&store, &request.workspace, token, REVIEW)? != actor {
            return Err("Team reviewer authority changed.".into());
        }
        let now = unix_now();
        if verified.release != request.release
            || request.expires_at <= now
            || request.expires_at - now > 30 * 24 * 60 * 60
        {
            return Err("Team source changed or the grant expiry exceeds 30 days.".into());
        }
        let mut revision = Revision {
            request,
            revision: 1,
            supersedes: None,
            reviewer: actor.account,
            membership_epoch: actor.epoch,
            member_epoch,
            active: true,
            reviewed_at: now,
            evidence: verified.evidence,
            digest: String::new(),
        };
        revision.digest = revision.computed();
        Self::team_keep_knowledge(&mut store, &knowledge, revision.evidence.clone())?;
        store
            .team_capabilities
            .grants
            .insert(key, vec![revision.clone()]);
        self.team_commit(&lock, &mut held, &mut store)?;
        Ok(revision)
    }
    pub fn team_revoke(
        &self,
        workspace: &str,
        token: &str,
        id: &str,
        approved: &str,
    ) -> Result<Revision, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, REVIEW)?;
        if actor.role < Role::Admin {
            return Err("Team revocation requires a current owner or admin.".into());
        }
        let history = store
            .team_capabilities
            .grants
            .get_mut(&grant_key(workspace, id))
            .ok_or("Unknown team grant.")?;
        let old = history.last().unwrap();
        if !old.active && old.supersedes.as_deref() == Some(approved) {
            return Ok(old.clone());
        }
        if old.digest != approved {
            return Err("Revocation approval does not match the current grant.".into());
        }
        if !old.active {
            return Ok(old.clone());
        }
        let mut r = old.clone();
        r.revision += 1;
        r.supersedes = Some(old.digest.clone());
        r.active = false;
        r.reviewer = actor.account;
        r.membership_epoch = actor.epoch;
        r.reviewed_at = unix_now();
        r.digest = r.computed();
        history.push(r.clone());
        self.team_commit(&lock, &mut held, &mut store)?;
        Ok(r)
    }
    /// Admit and checkpoint one native operation before effects. An interrupted
    /// operation stays unknown; it is never redispatched from a retry.
    pub fn team_apply(
        &self,
        workspace: &str,
        token: &str,
        grant: &str,
        id: &str,
        action: Action,
        input: Option<(&[u8], &str)>,
        sources: &mut dyn Sources,
        dispatch: impl FnOnce(&Revision, &EffectFence<'_>) -> Result<Completed, String>,
    ) -> Result<ResultOfUse, String> {
        if !identifier(id) {
            return Err("Invalid team operation ID.".into());
        }
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let actor = self.team_actor(&store, workspace, token, action.scope())?;
        let revision = store
            .team_capabilities
            .grants
            .get(&grant_key(workspace, grant))
            .and_then(|h| h.last())
            .ok_or("Unknown team grant.")?
            .clone();
        if revision.request.member != actor.account {
            return Err("This exact team grant belongs to another member.".into());
        }
        if revision.member_epoch != actor.epoch {
            return Err("Team recipient membership changed; review a new exact grant.".into());
        }
        if !revision.active || revision.request.expires_at <= unix_now() {
            return Err("Team grant is withdrawn or expired.".into());
        }
        if action == Action::Use {
            let (bytes, approved) = input.ok_or("Use requires separately approved input bytes.")?;
            if bytes_digest(bytes) != revision.request.input
                || approved != revision.request.input
                || bytes.len() as u64 != revision.request.input_bytes
            {
                return Err(
                    "Input bytes differ from the separately approved team data grant.".into(),
                );
            }
        } else if input.is_some() {
            return Err("Install and enable accept no customer input.".into());
        }
        let fingerprint =
            digest(&json!({"grant":revision.digest,"member":actor.account,"action":action}));
        let key = grant_key(workspace, id);
        if let Some(old) = store.team_capabilities.effects.get(&key) {
            if old.fingerprint != fingerprint {
                return Err("Team operation retry differs from its original admission.".into());
            }
            if old.state != "complete" {
                return Err("Team operation outcome is unknown; inspect native state and use a new explicitly reviewed operation.".into());
            }
            return Ok(ResultOfUse {
                effect: old.clone(),
                output: None,
                replayed: true,
            });
        }
        if store.team_policies.current(workspace).is_some() {
            return Err("Local team plugin install, enable, and use are not qualified under this workspace team policy; inspect original receipts without another effect.".into());
        }
        let knowledge = knowledge_key(workspace, &revision.request.release.package);
        let verified = match sources.current(
            store
                .team_capabilities
                .knowledge
                .get(&knowledge)
                .unwrap_or(&revision.evidence),
        ) {
            Ok(value) => value,
            Err(error) => {
                self.team_actor(&store, workspace, token, action.scope())?;
                self.team_retain_observation(&lock, &mut held, &mut store, &knowledge, sources)?;
                return Err(error);
            }
        };
        verified.validate()?;
        lock.check().map_err(|e| e.to_string())?;
        if verified.release != revision.request.release {
            return Err(
                "Current team release or source scope changed; a new review is required.".into(),
            );
        }
        if self.team_actor(&store, workspace, token, action.scope())? != actor {
            return Err("Team member authority changed before dispatch.".into());
        }
        if revision.request.expires_at <= unix_now() {
            return Err("Team grant expired during source verification.".into());
        }
        // A separately enabled native team policy can narrow this admission;
        // it cannot widen the release, data, recipient, or member grant.
        let effect = Effect {
            id: id.into(),
            fingerprint,
            workspace: workspace.into(),
            member: actor.account.clone(),
            grant: revision.digest.clone(),
            action: action.clone(),
            admitted_at: unix_now(),
            account_revision: store.digest.clone(),
            state: "unknown".into(),
            receipt: None,
        };
        store.team_capabilities.effects.insert(key.clone(), effect);
        Self::team_keep_knowledge(&mut store, &knowledge, verified.evidence)?;
        self.team_commit(&lock, &mut held, &mut store)?;
        lock.check().map_err(|e| e.to_string())?;
        self.team_actor(&store, workspace, token, action.scope())?;
        let checkpoint = store.digest.clone();
        let custody = || {
            lock.check().map_err(|e| e.to_string())?;
            self.team_check_state(&held, &checkpoint)
        };
        let authority = || {
            self.team_actor(&store, workspace, token, action.scope())?;
            Ok(())
        };
        let fence = EffectFence {
            custody: &custody,
            authority: &authority,
            expires_at: revision.request.expires_at,
        };
        fence.before_effect()?;
        let completed = dispatch(&revision, &fence)?;
        lock.check().map_err(|e| e.to_string())?;
        if serde_json::to_vec(&completed.receipt)
            .map_err(|e| e.to_string())?
            .len()
            > 32 * 1024
        {
            return Err("Team receipt exceeds its bound.".into());
        }
        let effect = store.team_capabilities.effects.get_mut(&key).unwrap();
        effect.state = "complete".into();
        effect.receipt = Some(completed.receipt);
        let effect = effect.clone();
        self.team_commit(&lock, &mut held, &mut store)?;
        Ok(ResultOfUse {
            effect,
            output: completed.output,
            replayed: false,
        })
    }
}
