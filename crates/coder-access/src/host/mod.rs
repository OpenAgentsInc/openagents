//! The host is the only issuer of access. Every operation checks the current
//! grant record under one private store lock before any effect or disclosure.
use crate::protocol::*;
use crate::{Code, Error, Result, Right, Rights, fail};
use coder_connect::{RelayPolicy, store::Store};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod enroll;
mod keys;
mod nearby;
mod serve;
pub use enroll::{EnrollmentStatus, IssuedInvitation, PendingEnrollment};
pub use keys::{KeySource, MemoryKeys};
pub use nearby::is_connect_code_rights;
pub use serve::{serve, serve_once};

const STORE: &str = "access";
const STORE_VERSION: &str = "coder-access.store.v1";
const MAX_GRANTS: usize = 128;
const MAX_INVITATIONS: usize = 64;
const MAX_ENROLLMENTS: usize = 32;
const MAX_REPLIES: usize = 1024;
/// What a retained local owner request records in place of a signed
/// event id, which is always 64 hex characters.
const LOCAL_REQUEST: &str = "local";
const MAX_EPOCHS: usize = 1024;
/// How far a device's clock may trail the host's: the same bound readers
/// allow for an `issued_at` ahead of them.
const CLOCK_SKEW: u64 = coder_connect::protocol::CLOCK_SKEW;
/// Revocation tombstones outlive grant expiry by the request window.
const SKEW: u64 = MAX_REQUEST_LIFETIME;
/// A channel admission refreshes last-seen at most this often, so an open
/// channel does not rewrite the store on every recheck.
pub const SEEN_RESOLUTION: u64 = 60;

fn task_answer(
    answer: std::result::Result<Outcome, Code>,
    operation: &Operation,
) -> Result<Outcome> {
    match answer {
        Ok(outcome) if outcome.validate().is_ok() && outcome.answers(operation) => Ok(outcome),
        Ok(_) => fail(Code::Unavailable, "the task owner's read answer is invalid"),
        Err(code) => fail(code, "the task owner refused the canonical read"),
    }
}

/// One principal whose current Operate standing was checked under the access lock.
#[derive(Clone, Debug)]
pub struct OperatePrincipal {
    pub device: String,
    pub grant: String,
    pub epoch: u64,
    pub expires_at: u64,
}

/// Supplies effects and disclosures from other profiles after admission.
/// `request` is the idempotency key for every effect.
pub trait Dispatch: Send {
    fn cloud(
        &mut self,
        _request: &str,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _op: &Operation,
    ) -> std::result::Result<Outcome, Code> {
        Err(Code::Unsupported)
    }
    fn cloud_admit_recovery(
        &mut self,
        _device: &str,
        _admission: &crate::cloud::Admission,
    ) -> std::result::Result<(), Code> {
        Err(Code::Unsupported)
    }
    fn project_list(
        &mut self,
        _device: &str,
        _workspace: &str,
    ) -> std::result::Result<crate::project::List, Code> {
        Err(Code::Unsupported)
    }
    fn project_read(
        &mut self,
        _device: &str,
        _query: &crate::project::Query,
    ) -> std::result::Result<crate::project::Page, Code> {
        Err(Code::Unsupported)
    }
    fn project_original(
        &mut self,
        _device: &str,
        _query: &crate::project::OriginalQuery,
    ) -> std::result::Result<crate::project::Chunk, Code> {
        Err(Code::Unsupported)
    }
    /// Supply current standing while this synchronous dispatch holds the access lock.
    /// An owner must not reopen that lock from a deferred-principal callback.
    fn operate_snapshot(&mut self, _owner: &str, _principals: Vec<OperatePrincipal>) {}
    /// Canonical task reads after current Observe admission; the owner also
    /// checks its admitted workspace and evidence disclosure policy.
    fn task_list(
        &mut self,
        _device: &str,
        _query: &crate::task_read::ListQuery,
    ) -> std::result::Result<crate::task_read::List, Code> {
        Err(Code::Unsupported)
    }
    fn task_read(
        &mut self,
        _device: &str,
        _query: &crate::task_read::PageQuery,
    ) -> std::result::Result<crate::task_read::Page, Code> {
        Err(Code::Unsupported)
    }
    fn task_original(
        &mut self,
        _device: &str,
        _query: &crate::task_read::OriginalQuery,
    ) -> std::result::Result<crate::task_read::OriginalChunk, Code> {
        Err(Code::Unsupported)
    }
    fn dispatch(
        &mut self,
        request: &str,
        device: &str,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code>;
    /// Dispatch with the admitted principal's grant and epoch, which a
    /// deferred effect rechecks before it runs. `grant` is `None` for the
    /// owner. The default ignores it.
    fn dispatch_as(
        &mut self,
        request: &str,
        device: &str,
        _grant: Option<(&str, u64)>,
        op: &Operation,
    ) -> std::result::Result<Receipt, Code> {
        self.dispatch(request, device, op)
    }
    /// The workspace labels `task.create` accepts, for `workspace.list`.
    /// Sorted and distinct. A host without a task owner has none to offer.
    fn workspaces(&mut self) -> std::result::Result<Vec<String>, Code> {
        Err(Code::Unavailable)
    }
    /// List or edit a task's queued messages for `device`, admitted under
    /// `grant` (`None` for the owner). A host without a task owner has no
    /// queue to offer.
    fn queue(
        &mut self,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _task: &str,
        _edit: &crate::protocol::QueueEdit,
    ) -> std::result::Result<crate::protocol::TaskQueue, Code> {
        Err(Code::Unavailable)
    }
    /// Apply a request-keyed edit at the native task and queue snapshot.
    fn queue_at_revision(
        &mut self,
        _request: &str,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _task: &str,
        _revision: u64,
        _edit: &crate::protocol::QueueEdit,
        _queue_digest: Option<&str>,
    ) -> std::result::Result<(crate::protocol::TaskQueue, String), Code> {
        Err(Code::Unavailable)
    }
    /// The host's spend requests. A host without them has none to offer.
    fn spends(&mut self) -> Option<&mut dyn Spends> {
        None
    }
    /// The host's asks for the owner's wallet (`openagents wallet link`). A
    /// host without them has none to offer.
    fn links(&mut self) -> Option<&mut dyn Links> {
        None
    }
    /// A single-use `coder-pair:` invitation to the host's read-only Coder
    /// chats for `device`, which holds `observe`, and when the chat grant it
    /// carries ends (`chats.invite`). A host that serves no chats has none
    /// to offer.
    fn chats(&mut self, _device: &str, _now: u64) -> std::result::Result<(String, u64), Code> {
        Err(Code::Unavailable)
    }
    /// The owner's private Verse placements for `device`, which holds
    /// `observe` and named its Verse `world_key` (`verse.private`): the
    /// placements file, or `None` when the owner has none. A host without
    /// a Verse home has none to offer.
    fn verse_private(
        &mut self,
        _device: &str,
        _world_key: &str,
        _now: u64,
    ) -> std::result::Result<Option<String>, Code> {
        Err(Code::Unsupported)
    }
    /// The host's chat threads for `device`, which holds `observe`
    /// (`thread.list`): newest first, archived ones left out. A host that
    /// keeps no threads has none to offer.
    fn threads(
        &mut self,
        _device: &str,
    ) -> std::result::Result<Vec<crate::thread::ThreadRow>, Code> {
        Err(Code::Unavailable)
    }
    /// One page of `thread` for `device`, which holds `observe`
    /// (`thread.read`). `before` names the first turn not to include.
    fn thread(
        &mut self,
        _device: &str,
        _thread: &str,
        _before: Option<u64>,
    ) -> std::result::Result<crate::thread::ThreadPage, Code> {
        Err(Code::Unavailable)
    }
    /// What `task` changed, for `device`, which holds `observe`
    /// (`task.review`). A host without a task owner, or a task with no
    /// worktree of its own, has none to offer.
    fn review(
        &mut self,
        _device: &str,
        _task: &str,
    ) -> std::result::Result<crate::review::TaskReview, Code> {
        Err(Code::Unsupported)
    }
    /// Publish the change of `task` that `device`, which holds `operate`,
    /// reviewed at `base`, `head_commit`, and `head` (`task.publish`),
    /// admitted under `grant` (`None` for the owner). The task owner keys
    /// the operation by the task and those revisions, so a retry is the
    /// same operation. A refusal the owner decided, such as a stale head,
    /// is a publication in state `refused`, not an error.
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        _request: &str,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _task: &str,
        _base: &str,
        _head_commit: &str,
        _head: &str,
    ) -> std::result::Result<crate::review::Publication, Code> {
        Err(Code::Unsupported)
    }
    /// Keep one chunk of an image for `device`, which holds `operate`
    /// (`artifact.put`), and answer what the host holds of that image. A
    /// host without a task owner keeps no images.
    fn put_artifact(
        &mut self,
        _device: &str,
        _put: &crate::media::ArtifactPut,
    ) -> std::result::Result<crate::media::ArtifactState, Code> {
        Err(Code::Unsupported)
    }
    /// Answer a `computer` request for `device`, which holds `terminal`:
    /// a screenshot, the open apps, or a file chunk (`crate::computer`). A
    /// host that does not serve its computer answers `unsupported`.
    fn computer(
        &mut self,
        _device: &str,
        _request: &crate::computer::Request,
    ) -> std::result::Result<crate::computer::Answer, Code> {
        Err(Code::Unsupported)
    }
    /// Answer a `background.*` operation for `device`, which holds the
    /// right it requires. A host without background rules has none.
    fn background(
        &mut self,
        _device: &str,
        _op: &crate::protocol::Operation,
    ) -> std::result::Result<serde_json::Value, Code> {
        Err(Code::Unsupported)
    }
    /// Answer a `studio.agent.*` operation for `device`, which holds the
    /// right it requires, under the grant and epoch `grant` names (none
    /// for the owner). `request` is the NIP-HOST request ID, which keys an
    /// ask so a retry asks once. The answer is one of
    /// [`crate::agent`]'s answers as JSON. A host without workshop agents
    /// has none.
    fn agent(
        &mut self,
        _request: &str,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _op: &crate::protocol::Operation,
    ) -> std::result::Result<serde_json::Value, Code> {
        Err(Code::Unsupported)
    }
    /// The Agent Studio now, for `device`, which holds `observe`
    /// (`studio.snapshot`). A host without a studio has none to offer.
    fn studio_snapshot(
        &mut self,
        _device: &str,
    ) -> std::result::Result<crate::studio::Snapshot, Code> {
        Err(Code::Unsupported)
    }
    /// What changed in the studio since `since` in `stream`, for `device`,
    /// which holds `observe` (`studio.update`). A stream that no longer
    /// holds that point refuses as `stale`.
    fn studio_update(
        &mut self,
        _device: &str,
        _stream: &str,
        _since: u64,
    ) -> std::result::Result<crate::studio::Update, Code> {
        Err(Code::Unsupported)
    }
    /// A studio task's review, for `device`, which holds `observe`
    /// (`studio.review.open`). The default reads it as `task.review` does.
    fn studio_review(
        &mut self,
        device: &str,
        task: &str,
    ) -> std::result::Result<crate::review::TaskReview, Code> {
        self.review(device, task)
    }
    /// Decide a studio task's merge at the reviewed revisions, for
    /// `device`, which holds `review`, admitted under `grant` (`None` for
    /// the owner). `request` is the idempotency key. A worktree whose
    /// revisions moved since the review refuses as `stale`.
    fn studio_merge(
        &mut self,
        _request: &str,
        _device: &str,
        _grant: Option<(&str, u64)>,
        _decision: &crate::studio::MergeDecision,
    ) -> std::result::Result<crate::studio::Merged, Code> {
        Err(Code::Unsupported)
    }
}
/// Where the host keeps agent spend requests (phase 1 agent spending). The
/// host has checked the sender's `operate` right, and for `spend.list` that
/// the grant is the sender's own and names this host, before it calls here.
pub trait Spends: Send {
    /// Take `grant` as `device`'s current spend grant for this host, and
    /// list the requests awaiting `device`'s answer.
    fn list(
        &mut self,
        device: &str,
        grant: &crate::spend::Grant,
        now: u64,
    ) -> std::result::Result<Vec<crate::spend::Entry>, Code>;
    /// Record `device`'s receipt; returns the recorded one.
    fn settle(
        &mut self,
        device: &str,
        receipt: &crate::spend::Receipt,
        now: u64,
    ) -> std::result::Result<crate::spend::Receipt, Code>;
}

/// Where the host keeps asks for the owner's wallet
/// ([`crate::wallet_link`]). The host has checked the sender's `operate`
/// right before it calls here.
pub trait Links: Send {
    /// The open asks, oldest first.
    fn list(
        &mut self,
        device: &str,
        now: u64,
    ) -> std::result::Result<Vec<crate::wallet_link::Ask>, Code>;
    /// Record `device`'s answer to ask `id`: the sealed seed, or `None` when
    /// the owner declined. An ask that is unknown, expired, or answered is
    /// refused.
    fn answer(
        &mut self,
        device: &str,
        id: &str,
        sealed: Option<&crate::wallet_link::Sealed>,
        now: u64,
    ) -> std::result::Result<(), Code>;
}

/// The default dispatcher: task and terminal effects are not connected.
pub struct Unconnected;
impl Dispatch for Unconnected {
    fn dispatch(&mut self, _: &str, _: &str, _: &Operation) -> std::result::Result<Receipt, Code> {
        Err(Code::Unavailable)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRecord {
    grant: Grant,
    authorization: Event,
    revoked_at: Option<u64>,
    /// Host time of the last authenticated request or direct channel from
    /// the grant's device under this grant. Older stores lack it.
    #[serde(default)]
    seen_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    request_event: String,
    signer: String,
    expires_at: u64,
    /// `None` while a dispatched effect is uncertain.
    reply: Option<Event>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recovery: Option<Recovery>,
}

/// Retain authority metadata for task-effect recovery without storing its prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recovery {
    grant: Option<String>,
    epoch: Option<u64>,
    required: Right,
    until: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cloud: Option<crate::cloud::Admission>,
}

impl Recovery {
    fn for_request(request: &Request) -> Option<Self> {
        let supported = matches!(
            &request.op,
            Operation::CreateTask { .. }
                | Operation::SteerTask { .. }
                | Operation::CancelTask { .. }
                | Operation::CommandTaskAtRevision { .. }
                | Operation::PublishTask { .. }
                | Operation::CloudSubmit { .. }
                | Operation::CloudContinue { .. }
                | Operation::CloudCancel { .. }
                | Operation::CloudFollow { .. }
                | Operation::EnvironmentPromote { .. }
                | Operation::EnvironmentSelect { .. }
                | Operation::EnvironmentSteer { .. }
        ) || matches!(&request.op, Operation::QueueTaskAtRevision { edit, .. } if !matches!(edit, QueueEdit::List {}))
            || request.op.agent_effect()
            || request.op.studio_intent()
            || matches!(&request.op, Operation::DecideMerge { .. });
        let required = request.op.required().unwrap_or(Right::Operate);
        supported.then(|| Self {
            grant: request.grant.clone(),
            epoch: request.epoch,
            required,
            until: request.expires_at.saturating_add(48 * 60 * 60),
            cloud: crate::cloud::Admission::for_operation(&request.op),
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    v: String,
    host: String,
    owner: String,
    epochs: BTreeMap<String, u64>,
    grants: BTreeMap<String, GrantRecord>,
    invitations: BTreeMap<String, enroll::InvitationRecord>,
    enrollments: BTreeMap<String, enroll::EnrollmentRecord>,
    replies: BTreeMap<String, Retained>,
}
impl Book {
    fn epoch(&self, device: &str) -> u64 {
        self.epochs.get(device).copied().unwrap_or(0)
    }
    fn prune(&mut self, now: u64) {
        self.grants
            .retain(|_, g| g.grant.expires_at.saturating_add(SKEW) > now);
        self.replies.retain(|_, r| {
            r.recovery
                .as_ref()
                .map_or(r.expires_at, |recovery| recovery.until)
                > now
        });
        // A consumed invitation's grant outlives it, so pruning keeps references valid.
        self.invitations
            .retain(|_, i| i.expires_at.saturating_add(SKEW) > now);
        self.enrollments
            .retain(|_, e| e.expires_at.saturating_add(SKEW) > now);
    }
    /// Whether a delegating issuer still holds its rights at this host.
    fn issuer_current(&self, issuer: &str, grant: Option<&str>, rights: &Rights, now: u64) -> bool {
        if issuer == self.host || issuer == self.owner {
            return true;
        }
        grant.and_then(|id| self.grants.get(id)).is_some_and(|r| {
            r.grant.device == issuer
                && r.revoked_at.is_none()
                && r.grant.expires_at > now
                && r.grant.epoch == self.epoch(issuer)
                && r.grant.rights.contains(Right::AccessAdmin)
                && rights.first_missing(&r.grant.rights).is_none()
        })
    }
}

/// The admitted signer of one request.
struct Principal {
    key: String,
    rights: Rights,
    grant: Option<String>,
    expires_at: u64,
}

pub struct Host {
    directory: PathBuf,
    policy: RelayPolicy,
    /// `None` keeps the key in `host.key` in the store directory.
    keys: Option<keys::Keys>,
}
impl Host {
    pub fn new(directory: impl Into<PathBuf>, policy: RelayPolicy) -> Self {
        Self {
            directory: directory.into(),
            policy,
            keys: None,
        }
    }
    /// A host whose secret key lives in `source`, such as the OS keychain,
    /// rather than in a file. The book and its lock stay in `directory`.
    pub fn with_keys(
        directory: impl Into<PathBuf>,
        policy: RelayPolicy,
        source: std::sync::Arc<dyn KeySource>,
    ) -> Self {
        Self {
            directory: directory.into(),
            policy,
            keys: Some(keys::Keys::new(source)),
        }
    }
    /// The store directory: the book, its lock, and, without a key source,
    /// the host key.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// The host key from the key source, or from the store's key file.
    fn read_key(&self, store: &Store, create: bool) -> Result<SecretKey> {
        match &self.keys {
            Some(keys) => keys.key(create),
            None => Ok(store.key(create)?),
        }
    }
    /// Establish the owner locally. An invitation or relay event cannot do this.
    /// Reinitializing with the same owner is a no-op; another owner refuses.
    pub fn init(&self, owner: &str) -> Result<String> {
        public(owner)?;
        ensure_parent(&self.directory)?;
        let mut store = Store::open_named(&self.directory, STORE, true)?;
        // A key is created only for a store without a book, so an empty key
        // source can never give an existing book a second identity.
        let fresh = store.load::<serde_json::Value>()?.is_none();
        let secret = self.read_key(&store, fresh)?;
        let host = pubkey(&secret);
        if host == owner {
            return fail(Code::Forbidden, "owner and host keys must differ");
        }
        match store.load::<serde_json::Value>()? {
            Some(_) => {
                let book = self.book(&store, &secret)?;
                if book.owner != owner {
                    return fail(
                        Code::Conflict,
                        "this host already has another owner; create a new store to change it",
                    );
                }
            }
            None => store.save(&Book {
                v: STORE_VERSION.into(),
                host: host.clone(),
                owner: owner.into(),
                epochs: BTreeMap::new(),
                grants: BTreeMap::new(),
                invitations: BTreeMap::new(),
                enrollments: BTreeMap::new(),
                replies: BTreeMap::new(),
            })?,
        }
        Ok(host)
    }
    pub fn public_key(&self) -> Result<String> {
        Ok(pubkey(&self.key()?))
    }
    pub(crate) fn key(&self) -> Result<SecretKey> {
        let store = Store::open_named(&self.directory, STORE, false)?;
        self.read_key(&store, false)
    }
    /// The host's signing key. A resident host signs presence, hints,
    /// channel proofs, terminal results, and summaries with this one key, so
    /// every record a device reads names the same host identity.
    pub fn signing_key(&self) -> Result<SecretKey> {
        self.key()
    }
    /// The private state file. A resident host watches its identity to
    /// reload grants that another local process changed.
    pub fn state_path(&self) -> PathBuf {
        self.directory.join(format!("{STORE}.json"))
    }
    pub fn policy(&self) -> RelayPolicy {
        self.policy
    }
    pub fn owner(&self) -> Result<String> {
        let (_, _, book) = self.open()?;
        Ok(book.owner)
    }
    fn open(&self) -> Result<(Store, SecretKey, Book)> {
        let store = Store::open_named(&self.directory, STORE, false)?;
        let secret = self.read_key(&store, false)?;
        let book = self.book(&store, &secret)?;
        Ok((store, secret, book))
    }
    /// Make `owner` this host's owner, for a host whose owner another
    /// program created, such as a desktop app, when the person imports the
    /// owner key they use on their other computers. Grants name the owner,
    /// so this is refused with `conflict` while any device holds a current
    /// grant; the person removes those devices first. Revoked and expired
    /// grants, invitations, enrollment requests, and retained replies leave
    /// the book, and epochs stay, so no old grant can come back. The same
    /// owner again is a no-op.
    pub fn reown(&self, owner: &str, now: u64) -> Result<()> {
        public(owner)?;
        let (mut store, _, mut book) = self.open()?;
        if book.owner == owner {
            return Ok(());
        }
        if owner == book.host {
            return fail(Code::Forbidden, "owner and host keys must differ");
        }
        if devices(&book, now)
            .iter()
            .any(|d| d.state == DeviceState::Active)
        {
            return fail(
                Code::Conflict,
                "remove every device before changing the owner",
            );
        }
        book.owner = owner.into();
        book.grants.clear();
        book.invitations.clear();
        book.enrollments.clear();
        book.replies.clear();
        store.save(&book)?;
        Ok(())
    }
    /// Every enrolled grant with its current state.
    pub fn devices(&self, now: u64) -> Result<Vec<DeviceEntry>> {
        let (_, _, book) = self.open()?;
        Ok(devices(&book, now))
    }
    /// Revoke every grant a device holds and advance its epoch. Persisted
    /// before return; later operations and cached retries refuse.
    pub fn revoke(&self, device: &str, now: u64) -> Result<(u64, Vec<String>)> {
        public(device)?;
        let (mut store, _, mut book) = self.open()?;
        let result = revoke(&mut book, device, now)?;
        store.save(&book)?;
        Ok(result)
    }
    /// Record that the host admitted `device` under `grant` at `now`, for
    /// example when a direct channel opens. A request admitted through
    /// [`Host::handle`] records itself. Writes at most once per
    /// [`SEEN_RESOLUTION`] seconds per grant; an unknown, revoked, or foreign
    /// grant records nothing.
    pub fn touch(&self, device: &str, grant: &str, now: u64) -> Result<()> {
        let (mut store, _, mut book) = self.open()?;
        let Some(record) = book.grants.get_mut(grant) else {
            return Ok(());
        };
        if record.grant.device != device
            || record.revoked_at.is_some()
            || record
                .seen_at
                .is_some_and(|at| at.saturating_add(SEEN_RESOLUTION) > now)
        {
            return Ok(());
        }
        record.seen_at = Some(now);
        store.save(&book)?;
        Ok(())
    }

    /// Renew `device`'s grant `grant` at `epoch` when a quarter or less of
    /// its lifetime remains, so a paired device never has to pair again
    /// while it keeps connecting. The new grant has a new ID and the same
    /// device, relay, rights, epoch, origin, and lifetime (at most 30 days),
    /// issued now. The renewed grant stays admitted until its own expiry,
    /// which is at most a quarter of its lifetime away, so a request already
    /// signed under it, or a device that does not read renewals yet, is not
    /// cut off; revoking the device revokes both.
    ///
    /// Returns the new grant envelope, or `None` when nothing is due: the
    /// grant is not the device's newest current grant, is revoked, expired,
    /// or at an old epoch, has more than a quarter of its life left, or was
    /// delegated by a device rather than issued by the host or its owner.
    ///
    /// # Errors
    /// Refuses a store that cannot be read or saved.
    pub fn renew(&self, device: &str, grant: &str, epoch: u64, now: u64) -> Result<Option<Event>> {
        public(device)?;
        let (mut store, secret, mut book) = self.open()?;
        let Some(record) = book.grants.get(grant).cloned() else {
            return Ok(None);
        };
        let current = |r: &GrantRecord| {
            r.revoked_at.is_none()
                && r.grant.expires_at > now
                && r.grant.epoch == book.epoch(&r.grant.device)
        };
        let lifetime = record
            .grant
            .expires_at
            .saturating_sub(record.grant.issued_at)
            .min(MAX_GRANT_LIFETIME);
        let newest = !book.grants.values().any(|r| {
            r.grant.device == device
                && r.grant.grant != grant
                && current(r)
                && r.grant.expires_at >= record.grant.expires_at
        });
        let due = record
            .grant
            .expires_at
            .saturating_sub(now)
            .saturating_mul(4)
            <= lifetime;
        let issued_here =
            record.grant.origin.issuer == book.host || record.grant.origin.issuer == book.owner;
        if record.grant.device != device
            || record.grant.epoch != epoch
            || !current(&record)
            || !newest
            || !due
            || !issued_here
            || lifetime == 0
        {
            return Ok(None);
        }
        book.prune(now);
        if book.grants.len() >= MAX_GRANTS && !evict_one_dead(&mut book, now) {
            return Ok(None);
        }
        let renewed = Grant {
            v: GRANT.into(),
            requires: vec![],
            grant: random_id(),
            host: book.host.clone(),
            owner: book.owner.clone(),
            device: device.into(),
            relay: record.grant.relay.clone(),
            rights: record.grant.rights.clone(),
            epoch,
            origin: record.grant.origin.clone(),
            issued_at: now,
            expires_at: now.saturating_add(lifetime),
        };
        renewed.validate(RelayPolicy::LoopbackTest)?;
        let authorization = seal(
            &renewed,
            GRANT,
            &secret,
            device,
            &renewed.grant,
            now,
            renewed.expires_at,
        )?;
        book.grants.insert(
            renewed.grant.clone(),
            GrantRecord {
                grant: renewed,
                authorization: authorization.clone(),
                revoked_at: None,
                seen_at: Some(now),
            },
        );
        store.save(&book)?;
        Ok(Some(authorization))
    }

    /// Issue grants to `devices` in one commit, as if each had redeemed an
    /// invitation from the host, so a test can fill the book quickly.
    #[cfg(test)]
    pub(crate) fn issue_for_test(
        &self,
        devices: &[String],
        relay: &str,
        rights: &Rights,
        now: u64,
        expires_at: u64,
    ) -> Result<()> {
        let (mut store, secret, mut book) = self.open()?;
        for device in devices {
            let origin = Origin {
                kind: OriginKind::Invitation,
                id: random_id(),
                issuer: book.host.clone(),
            };
            issue(
                &mut book,
                &secret,
                device,
                relay,
                rights.clone(),
                origin,
                now,
                expires_at,
            )?;
        }
        Ok(store.save(&book)?)
    }

    pub fn handle(
        &self,
        event: &Event,
        relay: &str,
        now: u64,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.handle_with_clock(event, relay, || Ok(now), dispatch)
    }
    pub fn handle_current(
        &self,
        event: &Event,
        relay: &str,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.handle_with_clock(event, relay, crate::unix_time, dispatch)
    }
    /// An `Err` means no signed reply exists: the request was unreadable,
    /// unauthenticated, stale, or its persistence was uncertain.
    pub fn handle_with_clock(
        &self,
        event: &Event,
        relay: &str,
        clock: impl FnMut() -> Result<u64>,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        self.handle_inner(event, Some(relay), clock, dispatch)
    }
    /// Admit one `enroll.redeem` that arrived on a transport other than a
    /// relay, such as the iroh enroll ALPN. The relay binding is the
    /// invitation's own: the request must name the relay the invitation
    /// names, although it did not travel over it. Every other redemption
    /// check is unchanged. Any other operation, and an invitation this host
    /// does not retain, gets no signed reply.
    pub fn handle_redemption(
        &self,
        event: &Event,
        clock: impl FnMut() -> Result<u64>,
    ) -> Result<Event> {
        self.handle_inner(event, None, clock, &mut Unconnected)
    }
    /// The relay a retained invitation names, or `None` for an invitation
    /// this host does not retain. A cancelled, consumed, or expired
    /// invitation is still named, so its redemption earns its refusal.
    pub fn invitation_relay(&self, id: &str) -> Result<Option<String>> {
        let (_, _, book) = self.open()?;
        Ok(book.invitations.get(id).map(|i| i.relay().to_owned()))
    }
    /// `relay` is the relay the request arrived on, or `None` for a
    /// redemption on another transport, whose relay is its invitation's.
    fn handle_inner(
        &self,
        event: &Event,
        relay: Option<&str>,
        mut clock: impl FnMut() -> Result<u64>,
        dispatch: &mut dyn Dispatch,
    ) -> Result<Event> {
        if let Some(relay) = relay {
            self.policy.validate(relay).map_err(Error::from)?;
        }
        let (mut store, secret, mut book) = self.open()?;
        let host = book.host.clone();
        if event.pubkey == host {
            return fail(Code::Forbidden, "the host does not answer itself");
        }
        let request: Request = open(event, &secret, &event.pubkey, &host, REQUEST)?;
        request.validate(self.policy)?;
        let relay = match (relay, &request.op) {
            (Some(relay), _) => relay.to_owned(),
            (None, Operation::Redeem { invitation, .. }) => book
                .invitations
                .get(invitation)
                .map(|i| i.relay().to_owned())
                .ok_or_else(|| Error::new(Code::Forbidden, "invitation is not admitted"))?,
            (None, _) => return fail(Code::Forbidden, "only a redemption is admitted here"),
        };
        let relay = relay.as_str();
        let now = clock()?;
        fresh(request.issued_at, request.expires_at, now)?;
        if request.host != host
            || request.relay != relay
            || event.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()]
        {
            return fail(Code::Forbidden, "request host, relay, or mailbox differs");
        }
        book.prune(now);
        let signer = event.pubkey.clone();
        let retained = book.replies.get(&request.request).cloned();
        if retained
            .as_ref()
            .is_some_and(|r| r.request_event != event.id || r.signer != signer)
        {
            let refused = refused(Error::new(Code::Conflict, "request identity reused"));
            return self.seal_reply(&secret, event, &request, refused, clock()?);
        }
        let (result, retain) = if let Operation::Redeem { .. } = &request.op {
            match self.redeem(
                &mut book,
                &secret,
                &request,
                &signer,
                now,
                retained.as_ref(),
            )? {
                Step::Retained(reply) => return Ok(reply),
                Step::Reply(result, retain) => (result, retain),
            }
        } else {
            match principal(&book, &request, &signer, now) {
                Err(error) => (refused(error), false),
                Ok(p) => {
                    // The host observed the device now. The reply's save,
                    // below, commits it with everything else.
                    if let Some(record) = p.grant.as_ref().and_then(|id| book.grants.get_mut(id)) {
                        record.seen_at = Some(now);
                    }
                    if let Some(right) = request.op.required().filter(|r| !p.rights.contains(*r)) {
                        (refused(Error::missing(right)), true)
                    } else if let Some(Retained {
                        reply: Some(reply), ..
                    }) = &retained
                    {
                        if let Some(admission) = crate::cloud::Admission::for_operation(&request.op)
                        {
                            if let Err(code) = dispatch.cloud_admit_recovery(&p.key, &admission) {
                                return self.seal_reply(
                                    &secret,
                                    event,
                                    &request,
                                    refused(Error::new(
                                        code,
                                        "the operator cloud admission no longer holds",
                                    )),
                                    clock()?,
                                );
                            }
                        }
                        // Authority was rechecked above; the retained bytes are current.
                        return Ok(reply.clone());
                    } else if !request.op.retains_reply() {
                        // A read changes nothing, so its reply is not
                        // retained: a thread polled while its reply streams
                        // would otherwise fill the store with pages.
                        let result = match self.execute(
                            &mut store,
                            &mut book,
                            &secret,
                            &request,
                            (&event.id, &event.pubkey),
                            &p,
                            now,
                            dispatch,
                        )? {
                            Ok(outcome) => ReplyResult::Ok { outcome },
                            Err(error) => refused(error),
                        };
                        (result, false)
                    } else {
                        let result = match self.execute(
                            &mut store,
                            &mut book,
                            &secret,
                            &request,
                            (&event.id, &event.pubkey),
                            &p,
                            now,
                            dispatch,
                        )? {
                            Ok(outcome) => ReplyResult::Ok { outcome },
                            Err(error) => refused(error),
                        };
                        (result, true)
                    }
                }
            }
        };
        // A slow operation must not extend the request's freshness window.
        let reply_time = clock()?;
        fresh(request.issued_at, request.expires_at, reply_time)?;
        if matches!(
            request.op,
            Operation::ListTasks { .. }
                | Operation::ReadTask { .. }
                | Operation::ReadTaskOriginal { .. }
        ) && matches!(result, ReplyResult::Ok { .. })
        {
            let current = principal(&book, &request, &signer, reply_time)?;
            if !current.rights.contains(Right::Observe) {
                return Err(Error::missing(Right::Observe));
            }
        }
        let reply = self.seal_reply(&secret, event, &request, result, reply_time)?;
        if retain {
            if book.replies.len() >= MAX_REPLIES && !book.replies.contains_key(&request.request) {
                return fail(Code::Bounds, "retained reply limit reached");
            }
            book.replies.insert(
                request.request.clone(),
                Retained {
                    request_event: event.id.clone(),
                    signer,
                    expires_at: request.expires_at,
                    reply: Some(reply.clone()),
                    recovery: Recovery::for_request(&request),
                },
            );
            // Consumption, grant, and the exact reply commit together. No reply
            // escapes a failed save; a retry reopens the last committed book.
            store.save(&book)?;
        }
        Ok(reply)
    }

    /// Run one task or Agent Studio operation ([`Operation::local_task`])
    /// for this host's owner, asked over the host's same-user control
    /// socket rather than signed with the owner key.
    ///
    /// The control socket is reachable only by the account the host runs
    /// as, which already holds the host's store and can make any key its
    /// owner (`reown`), so it carries the owner's authority. A host whose
    /// owner key lives elsewhere, such as on the person's other computer,
    /// still takes local task requests this way. The admitted intent is
    /// retained before the effect, as for a signed request, so a retry of
    /// `request` dispatches the same idempotency key; a signed request may
    /// not reuse it.
    ///
    /// # Errors
    /// Refuses an operation other than the task broker's, a malformed
    /// request identity or operation, a reused identity, or a store that
    /// cannot be read or saved. A dispatcher refusal is the inner error.
    pub fn handle_local_owner(
        &self,
        request: &str,
        op: &Operation,
        now: u64,
        dispatch: &mut dyn Dispatch,
    ) -> Result<std::result::Result<Outcome, Error>> {
        if !op.local_task() {
            return fail(Code::Forbidden, "only task operations are taken locally");
        }
        identity(request).map_err(Error::from)?;
        op.validate()?;
        let (mut store, secret, mut book) = self.open()?;
        book.prune(now);
        let owner = book.owner.clone();
        if book
            .replies
            .get(request)
            .is_some_and(|r| r.request_event != LOCAL_REQUEST || r.signer != owner)
        {
            return fail(Code::Conflict, "request identity reused");
        }
        let local = Request {
            v: REQUEST.into(),
            requires: Vec::new(),
            request: request.into(),
            host: book.host.clone(),
            grant: None,
            epoch: None,
            relay: String::new(),
            issued_at: now,
            expires_at: now.saturating_add(MAX_REQUEST_LIFETIME),
            op: op.clone(),
        };
        let principal = Principal {
            key: owner.clone(),
            rights: Rights::all(),
            grant: None,
            expires_at: u64::MAX,
        };
        self.execute(
            &mut store,
            &mut book,
            &secret,
            &local,
            (LOCAL_REQUEST, &owner),
            &principal,
            now,
            dispatch,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute(
        &self,
        store: &mut Store,
        book: &mut Book,
        secret: &SecretKey,
        request: &Request,
        origin: (&str, &str),
        p: &Principal,
        now: u64,
        dispatch: &mut dyn Dispatch,
    ) -> Result<std::result::Result<Outcome, Error>> {
        let standing = book
            .grants
            .values()
            .filter(|record| {
                record.revoked_at.is_none()
                    && record.grant.expires_at > now
                    && record.grant.epoch == book.epoch(&record.grant.device)
                    && record.grant.rights.contains(Right::Operate)
            })
            .map(|record| OperatePrincipal {
                device: record.grant.device.clone(),
                grant: record.grant.grant.clone(),
                epoch: record.grant.epoch,
                expires_at: record.grant.expires_at,
            })
            .collect();
        dispatch.operate_snapshot(&book.owner, standing);
        Ok(match &request.op {
            Operation::Redeem { .. } => Err(Error::new(Code::Malformed, "redeem has no grant")),
            Operation::Approve { .. } | Operation::Deny { .. } => {
                enroll::decide(book, secret, request, p, now)?
            }
            Operation::Invite {
                rights,
                grant_expires_at,
            } => enroll::remote_invite(book, request, p, rights, *grant_expires_at, now),
            Operation::CancelInvite { invitation } => enroll::cancel(book, invitation),
            Operation::ListDevices {} => Ok(Outcome::Devices {
                devices: devices(book, now),
            }),
            Operation::Revoke { device } => {
                revoke(book, device, now).map(|(epoch, grants)| Outcome::Revoked {
                    device: device.clone(),
                    epoch,
                    grants,
                })
            }
            Operation::ListWorkspaces {} => match dispatch.workspaces() {
                Ok(mut workspaces) => {
                    workspaces.sort();
                    workspaces.dedup();
                    let outcome = Outcome::Workspaces { workspaces };
                    outcome.validate().map(|()| outcome)
                }
                Err(code) => Err(Error::new(code, "the host lists no workspaces")),
            },
            Operation::RequestOperation {
                request: original,
                request_event,
            } => {
                let result = if let Some(record) = book.replies.get(original) {
                    if record.signer != p.key || record.request_event != *request_event {
                        return Ok(Err(Error::new(
                            Code::Forbidden,
                            "the original request belongs to another identity",
                        )));
                    }
                    let recovery = record
                        .recovery
                        .as_ref()
                        .filter(|metadata| metadata.until > now)
                        .ok_or_else(|| {
                            Error::new(
                                Code::Unavailable,
                                "the original effect is outside recovery retention",
                            )
                        })?;
                    if recovery.grant != request.grant || recovery.epoch != request.epoch {
                        return Ok(Err(Error::new(
                            Code::Stale,
                            "recovery must use the original grant and epoch",
                        )));
                    }
                    if !p.rights.contains(recovery.required) {
                        return Ok(Err(Error::missing(recovery.required)));
                    }
                    if let Some(admission) = &recovery.cloud {
                        if let Err(code) = dispatch.cloud_admit_recovery(&p.key, admission) {
                            return Ok(Err(Error::new(
                                code,
                                "the operator cloud admission no longer holds",
                            )));
                        }
                    }
                    match &record.reply {
                        Some(event) => {
                            let reply: Reply =
                                open(event, secret, &book.host, &record.signer, REPLY)?;
                            if reply.request != *original
                                || reply.request_event != *request_event
                                || reply.host != book.host
                            {
                                return fail(
                                    Code::Malformed,
                                    "the retained effect result differs from its original request",
                                );
                            }
                            Some(Box::new(reply.result))
                        }
                        None => None,
                    }
                } else {
                    None
                };
                let outcome = Outcome::RequestOperation {
                    request: original.clone(),
                    request_event: request_event.clone(),
                    result,
                };
                outcome.validate()?;
                Ok(outcome)
            }
            Operation::ListTasks { query } => task_answer(
                dispatch
                    .task_list(&p.key, query)
                    .map(|tasks| Outcome::Tasks {
                        tasks: Box::new(tasks),
                    }),
                &request.op,
            ),
            Operation::ProjectList { workspace } => task_answer(
                dispatch
                    .project_list(&p.key, workspace)
                    .map(|projects| Outcome::ProjectList { projects }),
                &request.op,
            ),
            Operation::ProjectRead { query } => task_answer(
                dispatch
                    .project_read(&p.key, query)
                    .map(|project| Outcome::ProjectRead {
                        project: Box::new(project),
                    }),
                &request.op,
            ),
            Operation::ProjectOriginal { query } => task_answer(
                dispatch
                    .project_original(&p.key, query)
                    .map(|chunk| Outcome::ProjectOriginal { chunk }),
                &request.op,
            ),
            op @ (Operation::CloudProjects { .. }
            | Operation::CloudCatalog { .. }
            | Operation::CloudList { .. }
            | Operation::CloudRead { .. }
            | Operation::CloudOriginal { .. }
            | Operation::CloudSubmit { .. }
            | Operation::CloudContinue { .. }
            | Operation::CloudCancel { .. }
            | Operation::CloudFollow { .. }
            | Operation::CloudRelease { .. }
            | Operation::EnvironmentRead { .. }
            | Operation::EnvironmentEvidence { .. }
            | Operation::EnvironmentPromote { .. }
            | Operation::EnvironmentSelect { .. }
            | Operation::EnvironmentSteer { .. }) => {
                // A released credential is held only in the resident's
                // memory: nothing about it is retained here (BYO-05).
                if op.retains_reply() {
                    if book.replies.len() >= MAX_REPLIES {
                        return fail(Code::Bounds, "retained reply limit reached");
                    }
                    book.replies.insert(
                        request.request.clone(),
                        Retained {
                            request_event: origin.0.to_owned(),
                            signer: origin.1.to_owned(),
                            expires_at: request.expires_at,
                            reply: None,
                            recovery: Recovery::for_request(request),
                        },
                    );
                    store.save(book)?;
                }
                task_answer(
                    dispatch
                        .cloud(
                            &request.request,
                            &p.key,
                            p.grant.as_deref().zip(request.epoch),
                            op,
                        )
                        .and_then(|outcome| {
                            if let Outcome::EnvironmentAccepted { accepted } = &outcome {
                                if accepted.request != request.request {
                                    return Err(Code::Malformed);
                                }
                            }
                            if let Outcome::CloudAccepted { accepted } = &outcome {
                                if accepted.request != request.request
                                    || (matches!(op, Operation::CloudSubmit { .. })
                                        && accepted.scope.job != request.request)
                                {
                                    return Err(Code::Malformed);
                                }
                            }
                            Ok(outcome)
                        }),
                    op,
                )
            }
            Operation::ReadTask { query } => task_answer(
                dispatch.task_read(&p.key, query).map(|task| Outcome::Task {
                    task: Box::new(task),
                }),
                &request.op,
            ),
            Operation::ReadTaskOriginal { query } => task_answer(
                dispatch
                    .task_original(&p.key, query)
                    .map(|original| Outcome::TaskOriginal {
                        original: Box::new(original),
                    }),
                &request.op,
            ),
            // Queue edits are idempotent: an exact retry after an uncertain
            // save sets the same text, order, or lease again.
            Operation::QueueTask { task, edit } => {
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.queue(&p.key, grant, task, edit) {
                    Ok(queue) => {
                        let outcome = Outcome::Queue { queue };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(
                                Code::Unavailable,
                                "the task owner's queue is invalid",
                            )),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the task owner refused the queue edit")),
                }
            }
            Operation::QueueTaskAtRevision {
                task,
                revision,
                edit,
                queue_digest,
            } => {
                if !matches!(edit, QueueEdit::List {}) {
                    if book.replies.len() >= MAX_REPLIES
                        && !book.replies.contains_key(&request.request)
                    {
                        return fail(Code::Bounds, "retained reply limit reached");
                    }
                    book.replies
                        .entry(request.request.clone())
                        .or_insert_with(|| Retained {
                            request_event: origin.0.to_owned(),
                            signer: origin.1.to_owned(),
                            expires_at: request.expires_at,
                            reply: None,
                            recovery: Recovery::for_request(request),
                        });
                    store.save(book)?;
                }
                let grant = p.grant.as_deref().zip(request.epoch);
                task_answer(
                    dispatch
                        .queue_at_revision(
                            &request.request,
                            &p.key,
                            grant,
                            task,
                            *revision,
                            edit,
                            queue_digest.as_deref(),
                        )
                        .map(|(queue, queue_digest)| Outcome::QueueAtRevision {
                            revision: queue.revision,
                            queue,
                            queue_digest,
                        }),
                    &request.op,
                )
            }
            Operation::ListSpends { grant } => {
                if grant.issuer != p.key || grant.grantee != book.host {
                    Err(Error::new(
                        Code::Forbidden,
                        "a spend grant must be the sender's own and name this host",
                    ))
                } else {
                    match dispatch.spends().map(|s| s.list(&p.key, grant, now)) {
                        Some(Ok(spends)) => {
                            let outcome = Outcome::Spends { spends };
                            outcome.validate().map(|()| outcome)
                        }
                        Some(Err(code)) => Err(Error::new(code, "the host refused the spend list")),
                        None => Err(Error::new(Code::Unavailable, "the host holds no spends")),
                    }
                }
            }
            Operation::InviteChats {} => match dispatch.chats(&p.key, now) {
                Ok((invitation, expires_at)) => {
                    let outcome = Outcome::Chats {
                        invitation,
                        expires_at,
                    };
                    match outcome.validate() {
                        Ok(()) => Ok(outcome),
                        Err(_) => Err(Error::new(
                            Code::Unavailable,
                            "the chat invitation is invalid",
                        )),
                    }
                }
                Err(code) => Err(Error::new(code, "the host serves no chats")),
            },
            Operation::VersePrivate { world_key } => {
                match dispatch.verse_private(&p.key, world_key, now) {
                    Ok(placements) => {
                        let outcome = Outcome::VersePrivate { placements };
                        match outcome.validate() {
                            Ok(()) => Ok(outcome),
                            Err(_) => Err(Error::new(
                                Code::Unavailable,
                                "the private placements exceed their bound",
                            )),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the host has no private placements")),
                }
            }
            Operation::ListThreads {} => match dispatch.threads(&p.key) {
                Ok(threads) => {
                    let outcome = Outcome::Threads { threads };
                    match outcome.validate() {
                        Ok(()) => Ok(outcome),
                        Err(_) => Err(Error::new(Code::Unavailable, "the thread list is invalid")),
                    }
                }
                Err(code) => Err(Error::new(code, "the host keeps no threads")),
            },
            Operation::ReadThread { thread, before } => {
                match dispatch.thread(&p.key, thread, *before) {
                    Ok(page) => {
                        let outcome = Outcome::Thread {
                            thread: Box::new(page),
                        };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(Code::Unavailable, "the thread page is invalid")),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the host could not read the thread")),
                }
            }
            Operation::StudioSnapshot {} => match dispatch.studio_snapshot(&p.key) {
                Ok(snapshot) => {
                    let outcome = Outcome::Studio {
                        snapshot: Box::new(snapshot),
                    };
                    match outcome.validate() {
                        Ok(()) => Ok(outcome),
                        Err(_) => Err(Error::new(
                            Code::Unavailable,
                            "the studio snapshot is invalid",
                        )),
                    }
                }
                Err(code) => Err(Error::new(code, "the host could not read its studio")),
            },
            Operation::StudioUpdate { stream, since } => {
                match dispatch.studio_update(&p.key, stream, *since) {
                    Ok(update) => {
                        let outcome = Outcome::StudioUpdate {
                            update: Box::new(update),
                        };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(
                                Code::Unavailable,
                                "the studio update is invalid",
                            )),
                        }
                    }
                    Err(code) => Err(Error::new(
                        code,
                        "the host holds no studio update from that point",
                    )),
                }
            }
            Operation::OpenReview { task } => match dispatch.studio_review(&p.key, task) {
                Ok(review) => {
                    let outcome = Outcome::Review {
                        review: Box::new(review),
                    };
                    match outcome.validate() {
                        Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                        _ => Err(Error::new(Code::Unavailable, "the task review is invalid")),
                    }
                }
                Err(code) => Err(Error::new(code, "the host could not review the task")),
            },
            Operation::DecideMerge { decision } => {
                // Record the admitted intent before the effect; the task
                // owner keys a merge by its review and a request for
                // changes by its command ID, so a retry repeats nothing.
                if book.replies.len() >= MAX_REPLIES {
                    return fail(Code::Bounds, "retained reply limit reached");
                }
                book.replies.insert(
                    request.request.clone(),
                    Retained {
                        request_event: origin.0.to_owned(),
                        signer: origin.1.to_owned(),
                        expires_at: request.expires_at,
                        reply: None,
                        recovery: Recovery::for_request(request),
                    },
                );
                store.save(book)?;
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.studio_merge(&request.request, &p.key, grant, decision) {
                    Ok(merged) => {
                        let outcome = Outcome::Merged {
                            merged: Box::new(merged),
                        };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(Code::Unavailable, "the merge record is invalid")),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the host refused the merge decision")),
                }
            }
            Operation::ReviewTask { task } => match dispatch.review(&p.key, task) {
                Ok(review) => {
                    let outcome = Outcome::Review {
                        review: Box::new(review),
                    };
                    match outcome.validate() {
                        Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                        _ => Err(Error::new(Code::Unavailable, "the task review is invalid")),
                    }
                }
                Err(code) => Err(Error::new(code, "the host could not review the task")),
            },
            Operation::PublishTask {
                task,
                base,
                head_commit,
                head,
            } => {
                // Record the admitted intent before the effect, as for any
                // dispatched operation; the task owner's own record keys
                // the publication by its review identity.
                if book.replies.len() >= MAX_REPLIES {
                    return fail(Code::Bounds, "retained reply limit reached");
                }
                book.replies.insert(
                    request.request.clone(),
                    Retained {
                        request_event: origin.0.to_owned(),
                        signer: origin.1.to_owned(),
                        expires_at: request.expires_at,
                        reply: None,
                        recovery: Recovery::for_request(request),
                    },
                );
                store.save(book)?;
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.publish(
                    &request.request,
                    &p.key,
                    grant,
                    task,
                    base,
                    head_commit,
                    head,
                ) {
                    Ok(publication) => {
                        let outcome = Outcome::Published {
                            publication: Box::new(publication),
                        };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(
                                Code::Unavailable,
                                "the publication record is invalid",
                            )),
                        }
                    }
                    Err(code) => Err(Error::new(code, "the host refused the publication")),
                }
            }
            op @ (Operation::ListBackground {}
            | Operation::ShowBackground { .. }
            | Operation::LogBackground { .. }
            | Operation::RunBackground { .. }
            | Operation::PauseBackground { .. }) => match dispatch.background(&p.key, op) {
                Ok(value) => {
                    let outcome = Outcome::Background {
                        background: Box::new(value),
                    };
                    outcome.validate().map(|()| outcome)
                }
                Err(code) => Err(Error::new(code, "the host has no background rules")),
            },
            op @ (Operation::ListAgents {}
            | Operation::AskAgent { .. }
            | Operation::AnswerAgent { .. }
            | Operation::AgentRan { .. }
            | Operation::StopAgent { .. }
            | Operation::ListAgentMemory { .. }
            | Operation::EditAgentMemory { .. }
            | Operation::ListAgentJobs { .. }
            | Operation::EditAgentJobs { .. }
            | Operation::AgentLog { .. }
            | Operation::ListAgentWorkspaces {}
            | Operation::NewCrewAgent { .. }
            | Operation::CrewStatus {}
            | Operation::ControlCrew { .. }
            | Operation::ProposeHire { .. }
            | Operation::DecideHire { .. }
            | Operation::ListHires {}
            | Operation::SetAgentCharter { .. }
            | Operation::RecordAgentVerdict { .. }
            | Operation::ListAgentVerdicts { .. }
            | Operation::NewAgent { .. }
            | Operation::RetireAgent { .. }
            | Operation::RotateAgent { .. }) => {
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.agent(&request.request, &p.key, grant, op) {
                    Ok(value) => {
                        let outcome = Outcome::Agent {
                            agent: Box::new(value),
                        };
                        outcome.validate().map(|()| outcome)
                    }
                    Err(code) => Err(Error::new(code, "the host refused the agent operation")),
                }
            }
            Operation::PutArtifact { artifact } => match dispatch.put_artifact(&p.key, artifact) {
                Ok(state) => {
                    let outcome = Outcome::Artifact { artifact: state };
                    match outcome.validate() {
                        Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                        _ => Err(Error::new(
                            Code::Unavailable,
                            "the host's image state is invalid",
                        )),
                    }
                }
                Err(code) => Err(Error::new(code, "the host did not keep the image")),
            },
            Operation::Computer { computer } => match dispatch.computer(&p.key, computer) {
                Ok(answer) => {
                    let outcome = Outcome::Computer { computer: answer };
                    match outcome.validate() {
                        Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                        _ => Err(Error::new(
                            Code::Unavailable,
                            "the host's computer answer is invalid",
                        )),
                    }
                }
                Err(code) => Err(Error::new(code, "the host refused the computer request")),
            },
            Operation::ListWalletLinks {} => match dispatch.links().map(|l| l.list(&p.key, now)) {
                Some(Ok(mut links)) => {
                    links.truncate(crate::wallet_link::MAX_LISTED);
                    let outcome = Outcome::WalletLinks { links };
                    outcome.validate().map(|()| outcome)
                }
                Some(Err(code)) => Err(Error::new(code, "the host refused the wallet link list")),
                None => Err(Error::new(
                    Code::Unavailable,
                    "the host holds no wallet links",
                )),
            },
            Operation::AnswerWalletLink { id, sealed } => {
                match dispatch
                    .links()
                    .map(|l| l.answer(&p.key, id, sealed.as_ref(), now))
                {
                    Some(Ok(())) => Ok(Outcome::WalletLinkAnswered { id: id.clone() }),
                    Some(Err(code)) => Err(Error::new(code, "the host refused the answer")),
                    None => Err(Error::new(
                        Code::Unavailable,
                        "the host holds no wallet links",
                    )),
                }
            }
            Operation::SettleSpend { receipt } => {
                match dispatch.spends().map(|s| s.settle(&p.key, receipt, now)) {
                    Some(Ok(recorded)) => {
                        let outcome = Outcome::Settled {
                            receipt: Box::new(recorded),
                        };
                        match outcome.validate() {
                            Ok(()) if outcome.answers(&request.op) => Ok(outcome),
                            _ => Err(Error::new(
                                Code::Unavailable,
                                "the recorded receipt is invalid",
                            )),
                        }
                    }
                    Some(Err(code)) => Err(Error::new(code, "the host refused the receipt")),
                    None => Err(Error::new(Code::Unavailable, "the host holds no spends")),
                }
            }
            Operation::CreateTask { .. }
            | Operation::OpenTerminal { .. }
            | Operation::OpenTaskTerminal { .. }
            | Operation::SteerTask { .. }
            | Operation::CancelTask { .. }
            | Operation::ArchiveTask { .. }
            | Operation::CommandTask { .. }
            | Operation::CommandTaskAtRevision { .. }
            | Operation::SendThread { .. }
            | Operation::StopThread { .. }
            | Operation::RunThread { .. }
            | Operation::SubmitGoal { .. }
            | Operation::MessageSeat { .. }
            | Operation::PauseSeat { .. }
            | Operation::ResumeSeat { .. }
            | Operation::StopSeat { .. }
            | Operation::ReassignTask { .. }
            | Operation::CancelStudioTask { .. }
            | Operation::RetryTask { .. }
            | Operation::PrioritizeTask { .. }
            | Operation::AnswerDecision { .. }
            | Operation::AllowAlways { .. } => {
                // Record the admitted intent before the effect. A crash after
                // dispatch replays the same idempotency key, never a new one.
                if book.replies.len() >= MAX_REPLIES {
                    return fail(Code::Bounds, "retained reply limit reached");
                }
                book.replies.insert(
                    request.request.clone(),
                    Retained {
                        request_event: origin.0.to_owned(),
                        signer: origin.1.to_owned(),
                        expires_at: request.expires_at,
                        reply: None,
                        recovery: Recovery::for_request(request),
                    },
                );
                store.save(book)?;
                let grant = p.grant.as_deref().zip(request.epoch);
                match dispatch.dispatch_as(&request.request, &p.key, grant, &request.op) {
                    Ok(receipt)
                        if receipt.operation == request.op.name()
                            && !receipt.reference.is_empty()
                            && receipt.reference.len() <= 128
                            && (Outcome::Dispatched {
                                receipt: receipt.clone(),
                            })
                            .answers(&request.op) =>
                    {
                        Ok(Outcome::Dispatched { receipt })
                    }
                    Ok(_) => Err(Error::new(
                        Code::Unavailable,
                        "dispatcher receipt is invalid",
                    )),
                    Err(code) => Err(Error::new(code, "dispatcher refused the operation")),
                }
            }
        })
    }

    fn seal_reply(
        &self,
        secret: &SecretKey,
        event: &Event,
        request: &Request,
        result: ReplyResult,
        now: u64,
    ) -> Result<Event> {
        let reply = Reply {
            v: REPLY.into(),
            requires: vec![],
            request: request.request.clone(),
            request_event: event.id.clone(),
            host: pubkey(secret),
            issued_at: now,
            expires_at: request.expires_at,
            result,
        };
        seal(
            &reply,
            REPLY,
            secret,
            &event.pubkey,
            &request.request,
            now,
            request.expires_at,
        )
    }

    fn book(&self, store: &Store, secret: &SecretKey) -> Result<Book> {
        let book: Book = store.load()?.ok_or_else(|| {
            Error::new(Code::Unavailable, "initialize the host with `init` first")
        })?;
        let host = pubkey(secret);
        if book.v != STORE_VERSION
            || book.host != host
            || book.grants.len() > MAX_GRANTS
            || book.invitations.len() > MAX_INVITATIONS
            || book.enrollments.len() > MAX_ENROLLMENTS
            || book.replies.len() > MAX_REPLIES
            || book.epochs.len() > MAX_EPOCHS
        {
            return fail(
                Code::Malformed,
                "host access store identity or bounds differ",
            );
        }
        public(&book.owner)?;
        for (id, record) in &book.grants {
            record.grant.validate(self.policy)?;
            let signed: Grant = open(
                &record.authorization,
                secret,
                &host,
                &record.grant.device,
                GRANT,
            )?;
            if id != &record.grant.grant
                || encoded(&signed)? != encoded(&record.grant)?
                || record.grant.owner != book.owner
                || record.grant.host != host
            {
                return fail(
                    Code::Malformed,
                    "retained grant differs from its signed bytes",
                );
            }
        }
        for (id, retained) in &book.replies {
            if let Some(recovery) = &retained.recovery {
                if let Some(admission) = &recovery.cloud {
                    admission.validate()?;
                }
                if !matches!(recovery.required, Right::Operate | Right::Review)
                    || recovery.until != retained.expires_at.saturating_add(48 * 60 * 60)
                    || recovery.grant.is_some() != recovery.epoch.is_some()
                {
                    return fail(
                        Code::Malformed,
                        "retained recovery authority or lifetime differs",
                    );
                }
                if let Some(grant) = &recovery.grant {
                    identity(grant).map_err(Error::from)?;
                }
                if let Some(epoch) = recovery.epoch {
                    if epoch > MAX_SAFE {
                        return fail(Code::Malformed, "retained recovery epoch exceeds its bound");
                    }
                }
            }
            if let Some(reply) = &retained.reply
                && (reply.pubkey != host
                    || reply.tag_values("h").collect::<Vec<_>>() != [id.as_str()]
                    || nostr::private_artifact::admit(reply).is_err())
            {
                return fail(
                    Code::Malformed,
                    "retained reply differs from its signed bytes",
                );
            }
        }
        enroll::validate(&book, self.policy)?;
        Ok(book)
    }
}

/// A new reply and whether to retain it, or the exact retained reply bytes.
enum Step {
    Reply(ReplyResult, bool),
    Retained(Event),
}

fn refused(error: Error) -> ReplyResult {
    ReplyResult::Refused {
        code: error.code,
        missing: error.missing,
    }
}

fn principal(
    book: &Book,
    request: &Request,
    signer: &str,
    now: u64,
) -> std::result::Result<Principal, Error> {
    let Some(id) = &request.grant else {
        if signer == book.owner {
            return Ok(Principal {
                key: signer.into(),
                rights: Rights::all(),
                grant: None,
                expires_at: u64::MAX,
            });
        }
        return Err(Error::new(Code::Forbidden, "this key holds no grant"));
    };
    let record = book
        .grants
        .get(id)
        .ok_or_else(|| Error::new(Code::Forbidden, "grant is not admitted at this host"))?;
    if record.grant.device != signer {
        // A copied grant is not a bearer credential.
        return Err(Error::new(
            Code::Forbidden,
            "grant belongs to another device",
        ));
    }
    if record.revoked_at.is_some() {
        return Err(Error::new(Code::Revoked, "grant is revoked"));
    }
    if record.grant.expires_at <= now {
        return Err(Error::new(Code::Expired, "grant has expired"));
    }
    if request.epoch != Some(record.grant.epoch) || record.grant.epoch != book.epoch(signer) {
        return Err(Error::new(Code::Stale, "grant epoch is not current"));
    }
    // A device clock up to the skew behind the host's may date a request
    // just before the grant it holds was issued.
    if request.expires_at > record.grant.expires_at
        || request.issued_at.saturating_add(CLOCK_SKEW) < record.grant.issued_at
    {
        return Err(Error::new(Code::Forbidden, "request is outside its grant"));
    }
    Ok(Principal {
        key: signer.into(),
        rights: record.grant.rights.clone(),
        grant: Some(id.clone()),
        expires_at: record.grant.expires_at,
    })
}

fn devices(book: &Book, now: u64) -> Vec<DeviceEntry> {
    book.grants
        .values()
        .map(|r| DeviceEntry {
            device: r.grant.device.clone(),
            grant: r.grant.grant.clone(),
            rights: r.grant.rights.clone(),
            epoch: r.grant.epoch,
            origin: r.grant.origin.kind,
            issued_at: r.grant.issued_at,
            expires_at: r.grant.expires_at,
            last_seen: r.seen_at,
            state: if r.revoked_at.is_some() {
                DeviceState::Revoked
            } else if r.grant.expires_at <= now || r.grant.epoch != book.epoch(&r.grant.device) {
                DeviceState::Expired
            } else {
                DeviceState::Active
            },
        })
        .collect()
}

fn revoke(
    book: &mut Book,
    device: &str,
    now: u64,
) -> std::result::Result<(u64, Vec<String>), Error> {
    let grants: Vec<String> = book
        .grants
        .values()
        .filter(|r| r.grant.device == device)
        .map(|r| r.grant.grant.clone())
        .collect();
    if grants.is_empty() {
        return Err(Error::new(
            Code::Forbidden,
            "this device holds no retained grant",
        ));
    }
    if !book.epochs.contains_key(device) && book.epochs.len() >= MAX_EPOCHS {
        return Err(Error::new(
            Code::Bounds,
            "revocation epoch retention limit reached",
        ));
    }
    for id in &grants {
        book.grants
            .get_mut(id)
            .expect("listed grant")
            .revoked_at
            .get_or_insert(now);
    }
    let epoch = book.epoch(device) + 1;
    book.epochs.insert(device.into(), epoch);
    // Cached replies for this device can no longer be served.
    book.replies.retain(|_, r| r.signer != device);
    Ok((epoch, grants))
}

/// Issue and seal a new grant. The caller commits it with its reply.
#[allow(clippy::too_many_arguments)]
fn issue(
    book: &mut Book,
    secret: &SecretKey,
    device: &str,
    relay: &str,
    rights: Rights,
    origin: Origin,
    now: u64,
    expires_at: u64,
) -> Result<Event> {
    book.prune(now);
    // Only a grant live until now hands on its delegations; a grant that
    // was already revoked keeps its invitations refused.
    let live = |r: &GrantRecord| {
        r.revoked_at.is_none()
            && r.grant.expires_at > now
            && r.grant.epoch == book.epoch(&r.grant.device)
    };
    let handed_on: Vec<String> = book
        .grants
        .values()
        .filter(|r| r.grant.device == device && live(r))
        .map(|r| r.grant.grant.clone())
        .collect();
    make_room(book, device, now)?;
    let grant = Grant {
        v: GRANT.into(),
        requires: vec![],
        grant: random_id(),
        host: book.host.clone(),
        owner: book.owner.clone(),
        device: device.into(),
        relay: relay.into(),
        rights,
        epoch: book.epoch(device),
        origin,
        issued_at: now,
        expires_at,
    };
    // The caller validated the relay under the host's policy; this checks shape.
    grant.validate(RelayPolicy::LoopbackTest)?;
    let authorization = seal(&grant, GRANT, secret, device, &grant.grant, now, expires_at)?;
    book.grants.insert(
        grant.grant.clone(),
        GrantRecord {
            grant,
            authorization: authorization.clone(),
            revoked_at: None,
            seen_at: None,
        },
    );
    reparent(book, device, &handed_on);
    Ok(authorization)
}

/// Grant IDs that a retained invitation or approved enrollment names. They
/// stay in the book, so the record's retry answers `revoked`, until the
/// record leaves it.
fn named_grants(book: &Book) -> std::collections::BTreeSet<String> {
    book.invitations
        .values()
        .filter_map(|i| i.grant.clone())
        .chain(book.enrollments.values().filter_map(|e| e.approved_grant()))
        .collect()
}

/// Make room for a new grant to `device`, whose earlier grants the new
/// grant supersedes: one device key holds one grant. A superseded grant is revoked in the same commit as the
/// new grant and leaves the book once no retained record names it. The
/// device's epoch does not advance, so the new grant is current and grants
/// that other devices hold, including ones this device delegated, are
/// untouched. While the book is full, the grant that stopped being live
/// longest ago (revoked, expired, or at an old epoch) leaves it, so a dead
/// grant never blocks a new one. A request under a grant that left the book
/// is refused and reads nothing.
///
/// Nothing changes unless the new grant fits.
///
/// # Errors
/// Refuses with `Bounds` when every retained grant is live or named.
fn make_room(book: &mut Book, device: &str, now: u64) -> Result<()> {
    let named = named_grants(book);
    let superseded: Vec<String> = book
        .grants
        .values()
        .filter(|r| r.grant.device == device)
        .map(|r| r.grant.grant.clone())
        .collect();
    let dead = |id: &str, r: &GrantRecord| {
        superseded.iter().any(|s| s == id)
            || r.revoked_at.is_some()
            || r.grant.expires_at <= now
            || r.grant.epoch != book.epoch(&r.grant.device)
    };
    let mut evictable: Vec<(u64, String)> = book
        .grants
        .iter()
        .filter(|(id, r)| dead(id, r) && !named.contains(*id))
        .map(|(id, r)| {
            let died = if superseded.contains(id) {
                r.revoked_at.unwrap_or(now)
            } else {
                r.revoked_at.unwrap_or(r.grant.expires_at).min(now)
            };
            (died, id.clone())
        })
        .collect();
    let departing = superseded.iter().filter(|id| !named.contains(*id)).count();
    let kept = book.grants.len() - departing;
    let removable = evictable.len() - departing;
    if kept.saturating_sub(removable) >= MAX_GRANTS {
        return fail(Code::Bounds, "grant retention limit reached");
    }
    for id in &superseded {
        if named.contains(id) {
            let record = book.grants.get_mut(id).expect("listed grant");
            record.revoked_at.get_or_insert(now);
        } else {
            book.grants.remove(id);
        }
    }
    evictable.retain(|(_, id)| book.grants.contains_key(id));
    evictable.sort();
    let mut evictable = evictable.into_iter();
    while book.grants.len() >= MAX_GRANTS {
        let (_, id) = evictable.next().expect("counted above");
        book.grants.remove(&id);
    }
    Ok(())
}

/// Remove the grant that stopped being live longest ago and that no
/// retained record names. Returns whether one left.
fn evict_one_dead(book: &mut Book, now: u64) -> bool {
    let named = named_grants(book);
    let oldest = book
        .grants
        .iter()
        .filter(|(id, r)| {
            !named.contains(*id)
                && (r.revoked_at.is_some()
                    || r.grant.expires_at <= now
                    || r.grant.epoch != book.epoch(&r.grant.device))
        })
        .map(|(id, r)| {
            (
                r.revoked_at.unwrap_or(r.grant.expires_at).min(now),
                id.clone(),
            )
        })
        .min();
    oldest.is_some_and(|(_, id)| book.grants.remove(&id).is_some())
}

/// Move `device`'s unredeemed delegated invitations from a grant that was
/// live until the new grant superseded it to the new one, only where the new grant alone could have issued them:
/// it holds `access_admin`, every invited right, and outlives the invited
/// grant. Any other such invitation stays on its superseded grant and is
/// refused as `revoked` at redemption.
fn reparent(book: &mut Book, device: &str, handed_on: &[String]) {
    if handed_on.is_empty() {
        return;
    }
    let Some(current) = book
        .grants
        .values()
        .find(|r| r.grant.device == device && r.revoked_at.is_none())
        .map(|r| r.grant.clone())
    else {
        return;
    };
    for invitation in book.invitations.values_mut() {
        invitation.reparent(device, handed_on, &current);
    }
}

pub(crate) fn same_digest(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.as_bytes()
            .iter()
            .zip(b.as_bytes())
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// Create the private store's parent directory with owner-only permissions.
pub fn ensure_parent(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    fn create(parent: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
    }
    #[cfg(windows)]
    fn create(parent: &Path) -> std::io::Result<()> {
        private_fs::create_dir_all(parent)
    }
    if let Some(parent) = directory.parent() {
        create(parent).map_err(|_| {
            Error::new(
                Code::Unavailable,
                "cannot create the store's parent directory",
            )
        })?;
    }
    Ok(())
}
