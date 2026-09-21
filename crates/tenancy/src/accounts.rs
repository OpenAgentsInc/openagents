//! The account store: workspaces, memberships, and who may change them.
//!
//! The registry binds a tenant to its doors; this module binds people to
//! the tenant they act for. An *account* is a stable identity that an
//! external principal — an `oak_` key id, a relay's `nostr:<hex>` pubkey —
//! resolves to. A *workspace* is what accounts belong to: a personal
//! workspace holds exactly its owner, and an organization workspace holds
//! a membership per account. A *membership* carries the role every
//! mutation is authorized against, and a revocation epoch a session or
//! key authorization can compare to find out whether it still stands.
//!
//! # The store
//!
//! `accounts.json` sits beside `registry.json` and follows the same
//! discipline: a schema tag, a sequence, a `supersedes` chain, and a
//! SHA-256 digest over every field but itself, so a file that cannot
//! recompute its own digest is refused rather than read partially. Every
//! sealed revision is archived under `accounts-history/<digest>.json`, so
//! which membership authorized an earlier call can always be explained.
//! Writers serialize on an `accounts.lock` file — a mutation re-reads the
//! store inside the lock, so the second of two competing writers sees the
//! first's result rather than overwriting it.
//!
//! # What this is not
//!
//! This is the membership core, not an onboarding service. There is no
//! sign-in, no password, and no recovery flow here — account and
//! workspace creation are operator calls, an invitation token is a
//! credential the inviter delivers out of band, and the store keeps only
//! the token's digest. The HTTP adapter that will sit in front of these
//! calls is specified in
//! `docs/decision-models/workspace-membership.md`, not implemented here.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The file the account store lives in, beside `registry.json`.
const ACCOUNTS: &str = "accounts.json";

/// The directory past revisions are archived in, one file per digest.
const HISTORY: &str = "accounts-history";

/// The lock serializing writers, held only for a mutation's duration.
const LOCKFILE: &str = "accounts.lock";

/// The schema tag the store carries.
pub const ACCOUNTS_SCHEMA: &str = "openagents.tenancy.accounts.v1";

/// The invitation token's wire prefix, so a pasted token announces its
/// shape: `inv_<id>.<secret>`.
const INVITE_PREFIX: &str = "inv";

/// How many times a writer retries the lock before reporting it held.
const LOCK_RETRIES: u32 = 100;

/// A member's role. The declaration order is the rank: member, then
/// admin, then owner — `actor.role > target.role` is the seniority check
/// the removal rules are built on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Belongs to the workspace and may leave it. Nothing more.
    Member,
    /// Members plus invitations: an admin invites at any role below
    /// owner, revokes pending invitations, and removes members.
    Admin,
    /// Admins plus the workspace: an owner sets roles, moves seats and
    /// billing, and transfers ownership. Exactly one active member holds
    /// it at a time.
    Owner,
}

/// The kind of workspace — what membership rules apply.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceKind {
    /// One account's own workspace. It holds exactly its owner forever:
    /// invitations, role changes, and transfers are refused outright.
    Personal,
    /// A shared workspace with invitations, roles, and seats.
    Organization,
}

/// Whether a membership stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemberStatus {
    /// The member belongs and may act.
    Active,
    /// The member was removed. The record stays — revoked is a state an
    /// authorization can name, not an absence.
    Revoked,
}

/// An account: a stable identity external principals resolve to.
///
/// An account carries credential *references* — `key:<id>` for an `oak_`
/// key, `nostr:<hex>` for a relay principal — never a secret. The secrets
/// themselves are `keys.rs`'s business; a digest is the most any document
/// in this crate ever holds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Account {
    /// The account's id — `acct_<hex>`, assigned at creation and stable.
    pub id: String,
    /// The account's display label.
    pub label: String,
    /// The external principals that resolve to this account.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principals: Vec<String>,
    /// When the account was created, as RFC 3339 in UTC.
    pub created: String,
}

/// The join between an account and a workspace: the record every
/// authorization and mutation check reads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Membership {
    /// The account the membership belongs to.
    pub account: String,
    /// The member's role.
    pub role: Role,
    /// `active` or `revoked`.
    pub status: MemberStatus,
    /// The workspace's `members_epoch` when this membership last changed
    /// — granted, re-roled, or revoked. A session minted against one
    /// epoch knows it is stale the moment this moves.
    pub epoch: u64,
    /// When the membership was granted, as RFC 3339 in UTC.
    pub granted: String,
    /// When the membership was revoked, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<String>,
}

/// A workspace: a stable id, a kind, and the members it holds.
///
/// The `id` never changes — a rename, a seat change, and every membership
/// write leave it standing. `tenant` is the registry tenant the
/// workspace's quota and billing bind to; membership changes never touch
/// it, and only an owner may rebind it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    /// The workspace's id — `ws_<hex>`, assigned at creation and stable.
    pub id: String,
    /// `personal` or `organization`.
    pub kind: WorkspaceKind,
    /// The workspace's display name.
    pub name: String,
    /// The manifest tenant this workspace's quota and billing bind to.
    /// A reference, not a secret — the registry resolves what it means.
    pub tenant: String,
    /// How many members the workspace may hold, when it is bounded. A
    /// pending invitation holds a seat until it is accepted, expired, or
    /// revoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seats: Option<u32>,
    /// Bumped on every membership change in this workspace. A caller
    /// that cached an authorization under one epoch compares this field
    /// to learn whether its view moved.
    pub members_epoch: u64,
    /// Account id to membership. Revoked memberships stay in the map.
    #[serde(default)]
    pub members: BTreeMap<String, Membership>,
    /// When the workspace was created, as RFC 3339 in UTC.
    pub created: String,
}

/// Where an invitation stands. Expiry is derived from `expires_unix`
/// rather than stored — a pending invitation past its deadline is dead
/// whether or not anything marked it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InviteStatus {
    /// Issued, not yet answered.
    Pending,
    /// The token was presented once and consumed — an invitation is
    /// single-use, and a second presentation is a replay, not a join.
    Accepted,
    /// An owner or admin withdrew it.
    Revoked,
}

/// An invitation: a single-use, expiring bearer token's record.
///
/// The store keeps the secret's SHA-256 digest, exactly as `keys.rs`
/// keeps a key's. The token itself is returned once, to the inviter, and
/// after that exists only where the inviter delivered it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Invitation {
    /// The invitation's id — the token's middle segment.
    pub id: String,
    /// The workspace the invitation joins.
    pub workspace: String,
    /// SHA-256 of the secret, hex.
    pub digest: String,
    /// The role acceptance grants — `admin` or `member`, never `owner`.
    /// Ownership moves only through [`Accounts::transfer_ownership`].
    pub role: Role,
    /// The account that issued the invitation.
    pub invited_by: String,
    /// When the invitation was issued, as RFC 3339 in UTC.
    pub created: String,
    /// Unix seconds after which the invitation refuses acceptance.
    pub expires_unix: u64,
    /// `pending`, `accepted`, or `revoked`.
    pub status: InviteStatus,
    /// The account that consumed the invitation, when one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_by: Option<String>,
}

/// The store document: every account, workspace, and invitation, sealed.
///
/// `digest` covers every field but itself — canonicalized key-sorted JSON
/// over SHA-256, the same construction [`crate::manifest::Manifest`]
/// uses. `supersedes` names the revision this one replaced, so the store
/// is a chain rather than a file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Store {
    /// The schema tag.
    pub v: String,
    /// The revision number. Genesis is 0; each committed mutation adds
    /// one.
    pub sequence: u64,
    /// The digest of the revision this one replaced, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// Account id to account.
    #[serde(default)]
    pub accounts: BTreeMap<String, Account>,
    /// Workspace id to workspace.
    #[serde(default)]
    pub workspaces: BTreeMap<String, Workspace>,
    /// Invitation id to invitation.
    #[serde(default)]
    pub invitations: BTreeMap<String, Invitation>,
    /// The digest over every field above.
    pub digest: String,
}

impl Store {
    /// Fill in `digest` over the store's other fields.
    fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a store serializes");
        value
            .as_object_mut()
            .expect("a store is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Read and validate a store's text.
    fn parse(text: &str, name: &str) -> Result<Self, String> {
        let store: Self =
            serde_json::from_str(text).map_err(|error| format!("{name}: {error}"))?;
        store.validate(name)?;
        Ok(store)
    }

    /// The checks a store must pass before anything reads it.
    ///
    /// This is the fail-closed boundary: a schema tag this crate does not
    /// know, a digest that does not recompute, a membership filed under
    /// the wrong account or pointing at an account that does not exist, a
    /// workspace without exactly one active owner, a personal workspace
    /// with more than its owner, a seat count below the membership, an
    /// invitation pointing at a missing workspace or granting ownership —
    /// any of them refuses the whole store rather than serving the part
    /// that parsed.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.v != ACCOUNTS_SCHEMA {
            return Err(format!("{name}: schema `{}` is not `{ACCOUNTS_SCHEMA}`", self.v));
        }
        if self.digest != self.compute_digest() {
            return Err(format!(
                "{name}: the store's digest does not recompute over its contents"
            ));
        }
        let mut principals: BTreeMap<&str, &str> = BTreeMap::new();
        for account in self.accounts.values() {
            if account.id.is_empty() || account.label.is_empty() {
                return Err(format!("{name}: an account carries an empty id or label"));
            }
            for principal in &account.principals {
                if let Some(other) = principals.insert(principal, &account.id) {
                    return Err(format!(
                        "{name}: principal `{principal}` resolves to both `{other}` and \
                         `{}` — a principal names one account",
                        account.id
                    ));
                }
            }
        }
        for (id, workspace) in &self.workspaces {
            if workspace.name.is_empty() {
                return Err(format!("{name}: workspace `{id}` carries an empty name"));
            }
            if workspace.tenant.is_empty() {
                return Err(format!(
                    "{name}: workspace `{id}` carries no tenant binding"
                ));
            }
            let mut owners = 0_u64;
            let mut active = 0_u64;
            for (account, membership) in &workspace.members {
                if membership.account != *account {
                    return Err(format!(
                        "{name}: workspace `{id}` files a membership for `{}` under \
                         `{account}`",
                        membership.account
                    ));
                }
                if !self.accounts.contains_key(account) {
                    return Err(format!(
                        "{name}: workspace `{id}` lists `{account}`, which is not an \
                         account this store holds"
                    ));
                }
                if membership.status == MemberStatus::Active {
                    active += 1;
                    if membership.role == Role::Owner {
                        owners += 1;
                    }
                }
            }
            if owners != 1 {
                return Err(format!(
                    "{name}: workspace `{id}` has {owners} active owners — a workspace \
                     stands under exactly one"
                ));
            }
            if workspace.kind == WorkspaceKind::Personal && active != 1 {
                return Err(format!(
                    "{name}: personal workspace `{id}` holds {active} active members — \
                     a personal workspace holds its owner and no one else"
                ));
            }
            if workspace.seats.is_some_and(|seats| (seats as u64) < active) {
                return Err(format!(
                    "{name}: workspace `{id}` seats {} below its {active} active members",
                    workspace.seats.unwrap_or_default()
                ));
            }
        }
        for invitation in self.invitations.values() {
            if !self.workspaces.contains_key(&invitation.workspace) {
                return Err(format!(
                    "{name}: invitation `{}` joins `{}`, which is not a workspace this \
                     store holds",
                    invitation.id, invitation.workspace
                ));
            }
            if !self.accounts.contains_key(&invitation.invited_by) {
                return Err(format!(
                    "{name}: invitation `{}` was issued by `{}`, which is not an account \
                     this store holds",
                    invitation.id, invitation.invited_by
                ));
            }
            if invitation.role == Role::Owner {
                return Err(format!(
                    "{name}: invitation `{}` grants owner — ownership moves only by \
                     transfer",
                    invitation.id
                ));
            }
            if invitation.digest.len() != 64
                || !invitation.digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "{name}: invitation `{}` carries a digest that is not 64 hex \
                     characters",
                    invitation.id
                ));
            }
        }
        Ok(())
    }
}

/// What a successful authorization names — the snapshot a session or key
/// authorization keeps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberRef {
    /// The workspace the member was authorized in.
    pub workspace: String,
    /// The account authorized.
    pub account: String,
    /// The member's role at authorization.
    pub role: Role,
    /// The membership's own epoch — it moves when this membership is
    /// granted, re-roled, or revoked. A session minted against one epoch
    /// is stale the moment this differs.
    pub epoch: u64,
    /// The workspace's membership epoch — it moves on any membership
    /// change in the workspace, including changes to other members.
    pub members_epoch: u64,
}

/// What `invite` hands back: the record and, once, the token.
#[derive(Debug)]
pub struct Invited {
    /// The stored record — safe to log and keep.
    pub invitation: Invitation,
    /// The full token, `inv_<id>.<secret>`. This is the only place the
    /// secret exists outside the invitee's hands; the store cannot
    /// reproduce it.
    pub token: String,
}

/// Why an account-store read or write failed at the storage layer.
#[derive(Debug)]
pub enum Trouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// The store document did not parse or failed its own checks — the
    /// store fails closed rather than serving a partial read.
    Invalid(String),
    /// Another writer held the lock past the retry bound.
    Locked(String),
    /// A history lookup named a digest no archived revision carries.
    UnknownRevision(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Locked(path) => write!(
                f,
                "another writer holds {path}. The store takes one writer at a time: \
                 wait for it to finish, or remove the lock file if no writer is running"
            ),
            Self::UnknownRevision(digest) => {
                write!(f, "no archived revision carries digest `{digest}`")
            }
        }
    }
}

impl std::error::Error for Trouble {}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Why an account, membership, or authorization operation was refused.
#[derive(Debug)]
pub enum Refusal {
    /// The store itself failed — not a membership answer.
    Store(Trouble),
    /// The named account is not one this store holds.
    UnknownAccount(String),
    /// The named workspace is not one this store holds.
    UnknownWorkspace(String),
    /// The principal resolves to no account.
    UnknownPrincipal(String),
    /// The principal is already bound to another account.
    PrincipalTaken { principal: String, account: String },
    /// The account holds no membership in the workspace — or holds only a
    /// revoked one where the operation needs an active record.
    NotMember { workspace: String, account: String },
    /// The membership exists and is revoked. Distinct from `NotMember`:
    /// the member was known and was removed.
    Revoked { workspace: String, account: String },
    /// The accepting account already belongs to the workspace.
    AlreadyMember { workspace: String, account: String },
    /// The acting member's role does not cover the action.
    Forbidden {
        workspace: String,
        account: String,
        action: &'static str,
    },
    /// The operation does not apply to a personal workspace.
    PersonalWorkspace(String),
    /// The workspace's seats are full — members plus live invitations.
    SeatLimit { workspace: String, seats: u32 },
    /// A seat count below the active membership would orphan members.
    SeatsBelowMembers {
        workspace: String,
        seats: u32,
        members: u64,
    },
    /// The operation would leave the workspace without an owner. The
    /// owner transfers first; the last owner never exits.
    LastOwner { workspace: String },
    /// An invitation cannot grant ownership.
    OwnerByInvitation { workspace: String },
    /// Ownership moves only through [`Accounts::transfer_ownership`],
    /// never through a role write.
    OwnershipByTransfer { workspace: String },
    /// The invitation token is not `inv_<id>.<secret>` shaped.
    MalformedInvitation,
    /// The token names no invitation the store holds.
    UnknownInvitation(String),
    /// The invitation exists and the secret does not match.
    WrongSecret(String),
    /// The invitation was already consumed or withdrawn — a second
    /// acceptance is a replay, not a join.
    InvitationClosed { id: String, status: InviteStatus },
    /// The invitation's deadline has passed.
    InvitationExpired(String),
    /// A required field arrived empty; the field name says which.
    EmptyField(&'static str),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(trouble) => write!(f, "{trouble}"),
            Self::UnknownAccount(account) => {
                write!(f, "`{account}` is not an account this store holds")
            }
            Self::UnknownWorkspace(workspace) => {
                write!(f, "`{workspace}` is not a workspace this store holds")
            }
            Self::UnknownPrincipal(principal) => {
                write!(f, "principal `{principal}` resolves to no account")
            }
            Self::PrincipalTaken { principal, account } => write!(
                f,
                "principal `{principal}` is already bound to account `{account}`"
            ),
            Self::NotMember { workspace, account } => write!(
                f,
                "account `{account}` holds no active membership in workspace \
                 `{workspace}`"
            ),
            Self::Revoked { workspace, account } => write!(
                f,
                "account `{account}`'s membership in workspace `{workspace}` is revoked"
            ),
            Self::AlreadyMember { workspace, account } => write!(
                f,
                "account `{account}` is already an active member of workspace \
                 `{workspace}`"
            ),
            Self::Forbidden {
                workspace,
                account,
                action,
            } => write!(
                f,
                "account `{account}`'s role in workspace `{workspace}` does not cover \
                 {action}"
            ),
            Self::PersonalWorkspace(workspace) => write!(
                f,
                "workspace `{workspace}` is personal — it holds its owner and no one \
                 else"
            ),
            Self::SeatLimit { workspace, seats } => write!(
                f,
                "workspace `{workspace}` is at its {seats}-seat limit — a seat frees \
                 when a member is removed or an invitation lapses"
            ),
            Self::SeatsBelowMembers {
                workspace,
                seats,
                members,
            } => write!(
                f,
                "workspace `{workspace}` holds {members} active members — seats cannot \
                 drop to {seats} below them"
            ),
            Self::LastOwner { workspace } => write!(
                f,
                "workspace `{workspace}` would be left without an owner — transfer \
                 ownership first"
            ),
            Self::OwnerByInvitation { workspace } => write!(
                f,
                "workspace `{workspace}` cannot grant ownership by invitation — \
                 ownership moves only by transfer"
            ),
            Self::OwnershipByTransfer { workspace } => write!(
                f,
                "workspace `{workspace}` grants ownership only through transfer"
            ),
            Self::MalformedInvitation => {
                write!(f, "the token is not an `{INVITE_PREFIX}_<id>.<secret>` invitation")
            }
            Self::UnknownInvitation(id) => {
                write!(f, "invitation `{id}` is not one this store issued")
            }
            Self::WrongSecret(id) => {
                write!(f, "invitation `{id}`'s secret does not match")
            }
            Self::InvitationClosed { id, status } => write!(
                f,
                "invitation `{id}` is already {} — an invitation is single-use",
                invite_status_name(*status)
            ),
            Self::InvitationExpired(id) => {
                write!(f, "invitation `{id}` has expired")
            }
            Self::EmptyField(field) => write!(f, "{field} must not be empty"),
        }
    }
}

impl std::error::Error for Refusal {}

/// An invitation status's wire label, for errors and reports.
fn invite_status_name(status: InviteStatus) -> &'static str {
    match status {
        InviteStatus::Pending => "pending",
        InviteStatus::Accepted => "accepted",
        InviteStatus::Revoked => "revoked",
    }
}

/// The exclusive lock one mutation holds while it re-reads, changes, and
/// writes the store.
///
/// Same shape as the quota ledger's: `create_new` makes the lock atomic,
/// the file's absence is the release, and a dropped guard removes it. The
/// lock is per mutation rather than per open handle — reads never take
/// it, and a writer waits a bounded time for the holder to finish before
/// reporting the store locked.
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn acquire(dir: &Path) -> Result<Self, Trouble> {
        let path = dir.join(LOCKFILE);
        for _ in 0..LOCK_RETRIES {
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                Ok(mut file) => {
                    writeln!(file, "pid {}", std::process::id()).ok();
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(Trouble::Io(error)),
            }
        }
        Err(Trouble::Locked(path.display().to_string()))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The digest the store keeps of an invitation secret.
fn digest_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Fresh random material for ids and secrets — 32 bytes of hex.
fn fresh() -> Result<String, Trouble> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| Trouble::Invalid(format!("no randomness available: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The current time as Unix seconds — the clock invitations expire
/// against.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// Parse an invitation token into its id and secret halves.
fn split_invite(token: &str) -> Option<(String, String)> {
    let body = token.strip_prefix(&format!("{INVITE_PREFIX}_"))?;
    let (id, secret) = body.split_once('.')?;
    if id.is_empty() || secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((id.to_string(), secret.to_string()))
}

/// Load the store from a directory, validating it end to end.
///
/// A missing file is an error, not an empty store: the account store is
/// created by [`Accounts::install`], and a directory that should hold one
/// and does not is incomplete storage — it fails closed.
fn load(dir: &Path) -> Result<Store, Trouble> {
    let path = dir.join(ACCOUNTS);
    let text = std::fs::read_to_string(&path)?;
    Store::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)
}

/// Write a sealed store: archive it by digest, then replace
/// `accounts.json` in one rename.
fn save(dir: &Path, store: &Store) -> Result<(), Trouble> {
    let history = dir.join(HISTORY);
    std::fs::create_dir_all(&history)?;
    let text = serde_json::to_string_pretty(store)
        .map_err(|error| Trouble::Invalid(error.to_string()))?;
    let archived = history.join(format!("{}.json", store.digest));
    if !archived.exists() {
        std::fs::write(&archived, format!("{text}\n"))?;
    }
    let staged = dir.join(format!(".{ACCOUNTS}.tmp"));
    std::fs::write(&staged, format!("{text}\n"))?;
    std::fs::rename(&staged, dir.join(ACCOUNTS))?;
    Ok(())
}

/// The membership an actor must hold to act, or the refusal naming why
/// not. A revoked membership is a distinct answer from none at all.
fn active_member<'a>(ws: &'a Workspace, account: &str) -> Result<&'a Membership, Refusal> {
    match ws.members.get(account) {
        Some(membership) if membership.status == MemberStatus::Active => Ok(membership),
        Some(_) => Err(Refusal::Revoked {
            workspace: ws.id.clone(),
            account: account.to_string(),
        }),
        None => Err(Refusal::NotMember {
            workspace: ws.id.clone(),
            account: account.to_string(),
        }),
    }
}

/// The count of active members in a workspace.
fn active_count(ws: &Workspace) -> u64 {
    ws.members
        .values()
        .filter(|membership| membership.status == MemberStatus::Active)
        .count() as u64
}

/// The account store handle: a directory the calls read and write.
///
/// The handle holds no cache. Every query re-reads and re-validates
/// `accounts.json`, so a revocation committed by any writer is visible to
/// the very next [`Accounts::authorize`] on any handle — the property
/// session and key authorization rely on. Every mutation takes the lock,
/// re-reads inside it, applies, re-seals, and writes, so two writers on
/// the same directory serialize rather than tear.
#[derive(Clone, Debug)]
pub struct Accounts {
    dir: PathBuf,
}

impl Accounts {
    /// Create the store's genesis revision in a directory.
    ///
    /// The directory must not already hold one — a store begins, it does
    /// not appear mid-chain.
    pub fn install(dir: &Path) -> Result<Self, Trouble> {
        if dir.join(ACCOUNTS).exists() {
            return Err(Trouble::Invalid(format!(
                "{} already holds an account store; open it rather than reinstalling",
                dir.display()
            )));
        }
        let mut store = Store {
            v: ACCOUNTS_SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            accounts: BTreeMap::new(),
            workspaces: BTreeMap::new(),
            invitations: BTreeMap::new(),
            digest: String::new(),
        };
        store.seal();
        store
            .validate(&dir.join(ACCOUNTS).display().to_string())
            .map_err(Trouble::Invalid)?;
        save(dir, &store)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Open the store in a directory, validating it end to end. A
    /// missing or corrupt store is refused, not guessed at.
    pub fn open(dir: &Path) -> Result<Self, Trouble> {
        load(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Read the current store. This is the fresh read every query makes.
    pub fn store(&self) -> Result<Store, Trouble> {
        load(&self.dir)
    }

    /// Read an archived revision by digest — the lookup that explains
    /// which membership authorized an earlier call.
    pub fn revision(dir: &Path, digest: &str) -> Result<Store, Trouble> {
        let path = dir.join(HISTORY).join(format!("{digest}.json"));
        if !path.exists() {
            return Err(Trouble::UnknownRevision(digest.to_string()));
        }
        let text = std::fs::read_to_string(&path)?;
        Store::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)
    }

    /// Whether an account may act in a workspace, and under which role.
    ///
    /// This is the read session and key authorization make: a fresh load,
    /// then the membership's status and epochs. A revoked member gets a
    /// distinct `Revoked` answer, and the returned [`MemberRef`]'s epochs
    /// let a caller holding an earlier authorization see that it moved.
    pub fn authorize(&self, workspace: &str, account: &str) -> Result<MemberRef, Refusal> {
        let store = load(&self.dir).map_err(Refusal::Store)?;
        let ws = store
            .workspaces
            .get(workspace)
            .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
        let membership = active_member(ws, account)?;
        Ok(MemberRef {
            workspace: ws.id.clone(),
            account: account.to_string(),
            role: membership.role,
            epoch: membership.epoch,
            members_epoch: ws.members_epoch,
        })
    }

    /// Resolve an external principal to its account, then authorize.
    ///
    /// This is the key-authorization half of the call: `authenticate`
    /// names a key id, `key:<id>` is the principal convention this lookup
    /// reads, and the answer is the same [`MemberRef`] an account-name
    /// authorization returns.
    pub fn authorize_principal(
        &self,
        workspace: &str,
        principal: &str,
    ) -> Result<MemberRef, Refusal> {
        let store = load(&self.dir).map_err(Refusal::Store)?;
        let account = store
            .accounts
            .values()
            .find(|account| account.principals.iter().any(|p| p == principal))
            .map(|account| account.id.clone())
            .ok_or_else(|| Refusal::UnknownPrincipal(principal.to_string()))?;
        let ws = store
            .workspaces
            .get(workspace)
            .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
        let membership = active_member(ws, &account)?;
        Ok(MemberRef {
            workspace: ws.id.clone(),
            account,
            role: membership.role,
            epoch: membership.epoch,
            members_epoch: ws.members_epoch,
        })
    }

    /// The account a principal resolves to, when one does.
    pub fn account_of_principal(&self, principal: &str) -> Result<Option<String>, Trouble> {
        Ok(load(&self.dir)?.accounts.values()
            .find(|account| account.principals.iter().any(|p| p == principal))
            .map(|account| account.id.clone()))
    }

    /// A workspace by id, when the store holds it.
    pub fn workspace(&self, workspace: &str) -> Result<Option<Workspace>, Trouble> {
        Ok(load(&self.dir)?.workspaces.get(workspace).cloned())
    }

    /// A membership by workspace and account — including a revoked one,
    /// which is the record a `Revoked` authorization was read from.
    pub fn membership(
        &self,
        workspace: &str,
        account: &str,
    ) -> Result<Option<Membership>, Trouble> {
        Ok(load(&self.dir)?
            .workspaces
            .get(workspace)
            .and_then(|ws| ws.members.get(account))
            .cloned())
    }

    /// Create an account. Provisioning is an operator path — there is no
    /// self-serve sign-up here — so the call takes no actor.
    pub fn create_account(&self, label: &str, principals: &[String]) -> Result<Account, Refusal> {
        if label.is_empty() {
            return Err(Refusal::EmptyField("label"));
        }
        self.mutate(|store, _| {
            for principal in principals {
                if let Some(other) = store
                    .accounts
                    .values()
                    .find(|account| account.principals.iter().any(|p| p == principal))
                {
                    return Err(Refusal::PrincipalTaken {
                        principal: principal.clone(),
                        account: other.id.clone(),
                    });
                }
            }
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]),
                label: label.to_string(),
                principals: principals.to_vec(),
                created: crate::registry::now_utc(),
            };
            store.accounts.insert(account.id.clone(), account.clone());
            Ok(account)
        })
    }

    /// Replace an account's principal set — the operator path that binds
    /// a freshly issued `oak_` key id to the account it belongs to.
    pub fn update_principals(
        &self,
        account: &str,
        principals: &[String],
    ) -> Result<Account, Refusal> {
        self.mutate(|store, _| {
            if !store.accounts.contains_key(account) {
                return Err(Refusal::UnknownAccount(account.to_string()));
            }
            for principal in principals {
                if let Some(other) = store.accounts.values().find(|candidate| {
                    candidate.id != account
                        && candidate.principals.iter().any(|p| p == principal)
                }) {
                    return Err(Refusal::PrincipalTaken {
                        principal: principal.clone(),
                        account: other.id.clone(),
                    });
                }
            }
            let record = store.accounts.get_mut(account).unwrap();
            record.principals = principals.to_vec();
            Ok(record.clone())
        })
    }

    /// Create a workspace owned by an account.
    ///
    /// The account becomes the workspace's owner and only member. The
    /// `tenant` argument is the manifest tenant the workspace's quota and
    /// billing bind to — this crate records the binding; resolving what
    /// the tenant may reach is the registry's business.
    pub fn create_workspace(
        &self,
        owner: &str,
        name: &str,
        kind: WorkspaceKind,
        tenant: &str,
        seats: Option<u32>,
    ) -> Result<Workspace, Refusal> {
        if name.is_empty() {
            return Err(Refusal::EmptyField("name"));
        }
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        self.mutate(|store, _| {
            if !store.accounts.contains_key(owner) {
                return Err(Refusal::UnknownAccount(owner.to_string()));
            }
            let id = format!("ws_{}", &fresh().map_err(Refusal::Store)?[..16]);
            let granted = crate::registry::now_utc();
            let mut members = BTreeMap::new();
            members.insert(
                owner.to_string(),
                Membership {
                    account: owner.to_string(),
                    role: Role::Owner,
                    status: MemberStatus::Active,
                    epoch: 1,
                    granted: granted.clone(),
                    revoked: None,
                },
            );
            let workspace = Workspace {
                id: id.clone(),
                kind,
                name: name.to_string(),
                tenant: tenant.to_string(),
                seats,
                members_epoch: 1,
                members,
                created: granted,
            };
            store.workspaces.insert(id, workspace.clone());
            Ok(workspace)
        })
    }

    /// Issue an invitation to an organization workspace.
    ///
    /// The acting member must be an active owner or admin, and the grant
    /// is `admin` or `member` — ownership moves only through transfer. A
    /// live invitation holds a seat: when members plus unexpired pending
    /// invitations reach `seats`, the invitation is refused rather than
    /// over-selling the workspace.
    pub fn invite(
        &self,
        actor: &str,
        workspace: &str,
        role: Role,
        ttl_secs: u64,
    ) -> Result<Invited, Refusal> {
        self.mutate(|store, now| {
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if ws.kind == WorkspaceKind::Personal {
                return Err(Refusal::PersonalWorkspace(ws.id.clone()));
            }
            if member.role < Role::Admin {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "invite",
                });
            }
            if role == Role::Owner {
                return Err(Refusal::OwnerByInvitation {
                    workspace: ws.id.clone(),
                });
            }
            if let Some(seats) = ws.seats {
                let pending = store
                    .invitations
                    .values()
                    .filter(|invitation| {
                        invitation.workspace == ws.id
                            && invitation.status == InviteStatus::Pending
                            && invitation.expires_unix > now
                    })
                    .count() as u64;
                if active_count(ws) + pending >= seats as u64 {
                    return Err(Refusal::SeatLimit {
                        workspace: ws.id.clone(),
                        seats,
                    });
                }
            }
            let id = format!("{INVITE_PREFIX}_{}", &fresh().map_err(Refusal::Store)?[..16]);
            let secret = fresh().map_err(Refusal::Store)?;
            let invitation = Invitation {
                id: id.clone(),
                workspace: ws.id.clone(),
                digest: digest_secret(&secret),
                role,
                invited_by: actor.to_string(),
                created: crate::registry::now_utc(),
                expires_unix: now + ttl_secs,
                status: InviteStatus::Pending,
                accepted_by: None,
            };
            store.invitations.insert(id.clone(), invitation.clone());
            Ok(Invited {
                invitation,
                token: format!("{INVITE_PREFIX}_{id}.{secret}"),
            })
        })
    }

    /// Withdraw a pending invitation. The record stays — a revoked
    /// invitation is a state, not an absence.
    pub fn revoke_invitation(
        &self,
        actor: &str,
        workspace: &str,
        invitation: &str,
    ) -> Result<Invitation, Refusal> {
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if member.role < Role::Admin {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "revoke-invitation",
                });
            }
            let record = store
                .invitations
                .get_mut(invitation)
                .ok_or_else(|| Refusal::UnknownInvitation(invitation.to_string()))?;
            if record.workspace != ws.id {
                return Err(Refusal::UnknownInvitation(invitation.to_string()));
            }
            if record.status != InviteStatus::Pending {
                return Err(Refusal::InvitationClosed {
                    id: record.id.clone(),
                    status: record.status,
                });
            }
            record.status = InviteStatus::Revoked;
            Ok(record.clone())
        })
    }

    /// Accept an invitation. The token is the authority — any account
    /// holding it joins, which is why the token is a secret the inviter
    /// delivers out of band.
    ///
    /// Acceptance is single-use: a consumed or withdrawn invitation
    /// answers `InvitationClosed`, an expired one `InvitationExpired`,
    /// and a seat that filled between issue and acceptance refuses with
    /// `SeatLimit`. Accepting into a workspace the account already
    /// belongs to is refused; re-joining after a revocation re-activates
    /// the membership under a fresh epoch.
    pub fn accept(&self, account: &str, token: &str) -> Result<Membership, Refusal> {
        let (id, secret) = split_invite(token).ok_or(Refusal::MalformedInvitation)?;
        self.mutate(|store, now| {
            if !store.accounts.contains_key(account) {
                return Err(Refusal::UnknownAccount(account.to_string()));
            }
            let invitation = store
                .invitations
                .get(&id)
                .ok_or_else(|| Refusal::UnknownInvitation(id.clone()))?;
            if invitation.digest != digest_secret(&secret) {
                return Err(Refusal::WrongSecret(id));
            }
            if invitation.status != InviteStatus::Pending {
                return Err(Refusal::InvitationClosed {
                    id: invitation.id.clone(),
                    status: invitation.status,
                });
            }
            if invitation.expires_unix <= now {
                return Err(Refusal::InvitationExpired(id));
            }
            let role = invitation.role;
            let ws = store
                .workspaces
                .get_mut(&invitation.workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(invitation.workspace.clone()))?;
            if let Some(membership) = ws.members.get(account)
                && membership.status == MemberStatus::Active
            {
                return Err(Refusal::AlreadyMember {
                    workspace: ws.id.clone(),
                    account: account.to_string(),
                });
            }
            if let Some(seats) = ws.seats
                && active_count(ws) >= seats as u64
            {
                return Err(Refusal::SeatLimit {
                    workspace: ws.id.clone(),
                    seats,
                });
            }
            ws.members_epoch += 1;
            let membership = Membership {
                account: account.to_string(),
                role,
                status: MemberStatus::Active,
                epoch: ws.members_epoch,
                granted: crate::registry::now_utc(),
                revoked: None,
            };
            ws.members.insert(account.to_string(), membership.clone());
            let record = store.invitations.get_mut(&id).unwrap();
            record.status = InviteStatus::Accepted;
            record.accepted_by = Some(account.to_string());
            Ok(membership)
        })
    }

    /// Change a member's role between `admin` and `member`.
    ///
    /// Only the owner sets roles, and ownership never passes this way —
    /// granting `owner` is refused with `OwnershipByTransfer`, and
    /// writing a role onto the owner is `LastOwner`. Both move through
    /// [`Accounts::transfer_ownership`] instead.
    pub fn set_role(
        &self,
        actor: &str,
        workspace: &str,
        target: &str,
        role: Role,
    ) -> Result<Membership, Refusal> {
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if member.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "set-role",
                });
            }
            if role == Role::Owner {
                return Err(Refusal::OwnershipByTransfer {
                    workspace: ws.id.clone(),
                });
            }
            if actor == target {
                return Err(Refusal::LastOwner {
                    workspace: ws.id.clone(),
                });
            }
            let target_member = active_member(ws, target)?;
            if target_member.role == Role::Owner {
                return Err(Refusal::LastOwner {
                    workspace: ws.id.clone(),
                });
            }
            ws.members_epoch += 1;
            let record = ws.members.get_mut(target).unwrap();
            record.role = role;
            record.epoch = ws.members_epoch;
            Ok(record.clone())
        })
    }

    /// Remove a member — or let a member leave, when actor and target
    /// are the same account.
    ///
    /// The seniority rule is rank: an owner removes any other member, an
    /// admin removes members only, and a member removes no one but
    /// itself. The owner never exits — removing or abandoning the last
    /// owner is `LastOwner`, and ownership moves through transfer first.
    /// The record stays with `status: revoked` and a fresh epoch, so the
    /// next authorization names the revocation.
    pub fn remove_member(
        &self,
        actor: &str,
        workspace: &str,
        target: &str,
    ) -> Result<Membership, Refusal> {
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let actor_role = active_member(ws, actor)?.role;
            let target_role = match ws.members.get(target) {
                Some(membership) if membership.status == MemberStatus::Active => {
                    membership.role
                }
                Some(_) => {
                    return Err(Refusal::Revoked {
                        workspace: ws.id.clone(),
                        account: target.to_string(),
                    })
                }
                None => {
                    return Err(Refusal::NotMember {
                        workspace: ws.id.clone(),
                        account: target.to_string(),
                    })
                }
            };
            let permitted = if actor == target {
                target_role != Role::Owner
            } else {
                match actor_role {
                    Role::Owner => true,
                    Role::Admin => target_role == Role::Member,
                    Role::Member => false,
                }
            };
            if !permitted {
                return Err(if target_role == Role::Owner {
                    Refusal::LastOwner {
                        workspace: ws.id.clone(),
                    }
                } else {
                    Refusal::Forbidden {
                        workspace: ws.id.clone(),
                        account: actor.to_string(),
                        action: "remove-member",
                    }
                });
            }
            ws.members_epoch += 1;
            let record = ws.members.get_mut(target).unwrap();
            record.status = MemberStatus::Revoked;
            record.epoch = ws.members_epoch;
            record.revoked = Some(crate::registry::now_utc());
            Ok(record.clone())
        })
    }

    /// Hand ownership to another member, atomically.
    ///
    /// The acting owner becomes an admin and the target — an active
    /// member of any other role — becomes the owner in one committed
    /// revision, so the workspace never passes through a state with zero
    /// or two owners. This is the only path that moves ownership.
    pub fn transfer_ownership(
        &self,
        actor: &str,
        workspace: &str,
        target: &str,
    ) -> Result<(), Refusal> {
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if ws.kind == WorkspaceKind::Personal {
                return Err(Refusal::PersonalWorkspace(ws.id.clone()));
            }
            if member.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "transfer-ownership",
                });
            }
            if actor == target {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "transfer-ownership",
                });
            }
            active_member(ws, target)?;
            ws.members_epoch += 1;
            let epoch = ws.members_epoch;
            ws.members.get_mut(actor).unwrap().role = Role::Admin;
            ws.members.get_mut(actor).unwrap().epoch = epoch;
            ws.members.get_mut(target).unwrap().role = Role::Owner;
            ws.members.get_mut(target).unwrap().epoch = epoch;
            Ok(())
        })
    }

    /// Set the workspace's seat bound — or lift it with `None`.
    ///
    /// Seats bound members plus live invitations. Setting a bound below
    /// the active membership is refused rather than orphaning members
    /// the next validation would refuse to read.
    pub fn set_seats(
        &self,
        actor: &str,
        workspace: &str,
        seats: Option<u32>,
    ) -> Result<Workspace, Refusal> {
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if ws.kind == WorkspaceKind::Personal {
                return Err(Refusal::PersonalWorkspace(ws.id.clone()));
            }
            if member.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "set-seats",
                });
            }
            let active = active_count(ws);
            if seats.is_some_and(|seats| (seats as u64) < active) {
                return Err(Refusal::SeatsBelowMembers {
                    workspace: ws.id.clone(),
                    seats: seats.unwrap_or_default(),
                    members: active,
                });
            }
            ws.seats = seats;
            Ok(ws.clone())
        })
    }

    /// Rename a workspace. The id — and every membership, epoch, and
    /// binding under it — stands unchanged.
    pub fn rename(&self, actor: &str, workspace: &str, name: &str) -> Result<Workspace, Refusal> {
        if name.is_empty() {
            return Err(Refusal::EmptyField("name"));
        }
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if member.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "rename",
                });
            }
            ws.name = name.to_string();
            Ok(ws.clone())
        })
    }

    /// Rebind the workspace's tenant — the registry tenant its quota and
    /// billing resolve against. Owner only; the workspace id and every
    /// membership stand unchanged.
    pub fn rebind_tenant(
        &self,
        actor: &str,
        workspace: &str,
        tenant: &str,
    ) -> Result<Workspace, Refusal> {
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        self.mutate(|store, _| {
            let ws = store
                .workspaces
                .get_mut(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.to_string()))?;
            let member = active_member(ws, actor)?;
            if member.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: ws.id.clone(),
                    account: actor.to_string(),
                    action: "rebind-tenant",
                });
            }
            ws.tenant = tenant.to_string();
            Ok(ws.clone())
        })
    }

    /// The serialization point every mutation passes through.
    ///
    /// The lock goes on first, the store is re-read inside it — so the
    /// second of two competing writers decides against the first's
    /// committed result — and a successful change is sequenced, chained
    /// to the digest it supersedes, sealed, re-validated, archived, and
    /// renamed into place before the lock drops. A failed closure writes
    /// nothing.
    fn mutate<T>(
        &self,
        f: impl FnOnce(&mut Store, u64) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let _lock = Lock::acquire(&self.dir).map_err(Refusal::Store)?;
        let mut store = load(&self.dir).map_err(Refusal::Store)?;
        let supersedes = store.digest.clone();
        let out = f(&mut store, unix_now())?;
        store.sequence += 1;
        store.supersedes = Some(supersedes);
        store.seal();
        store
            .validate(&self.dir.join(ACCOUNTS).display().to_string())
            .map_err(|message| Refusal::Store(Trouble::Invalid(message)))?;
        save(&self.dir, &store).map_err(Refusal::Store)?;
        Ok(out)
    }
}

/// Canonical JSON: keys sorted, whitespace gone — the same
/// canonicalization the manifest digest uses, so two writers digest the
/// same store to the same bytes regardless of map order.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> (tempfile::TempDir, Accounts) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        (dir, accounts)
    }

    fn account(accounts: &Accounts, label: &str) -> Account {
        accounts.create_account(label, &[]).unwrap()
    }

    fn org(accounts: &Accounts, owner: &Account, seats: Option<u32>) -> Workspace {
        accounts
            .create_workspace(
                &owner.id,
                "engineering",
                WorkspaceKind::Organization,
                "acme",
                seats,
            )
            .unwrap()
    }

    #[test]
    fn two_workspaces_hold_independent_memberships_and_tenants() {
        let (_dir, accounts) = installed();
        let alice = account(&accounts, "alice");
        let bob = account(&accounts, "bob");
        let eng = org(&accounts, &alice, None);
        let ops = accounts
            .create_workspace(
                &alice.id,
                "operations",
                WorkspaceKind::Organization,
                "globex",
                None,
            )
            .unwrap();
        assert_ne!(eng.id, ops.id);

        // Bob joins eng only. Each workspace answers for itself.
        let invite = accounts
            .invite(&alice.id, &eng.id, Role::Member, 3600)
            .unwrap();
        accounts.accept(&bob.id, &invite.token).unwrap();
        assert_eq!(
            accounts.authorize(&eng.id, &bob.id).unwrap().role,
            Role::Member
        );
        assert!(matches!(
            accounts.authorize(&ops.id, &bob.id),
            Err(Refusal::NotMember { .. })
        ));

        // A rename moves no id, membership, or tenant binding.
        accounts.rename(&alice.id, &eng.id, "platform").unwrap();
        let renamed = accounts.workspace(&eng.id).unwrap().unwrap();
        assert_eq!(renamed.id, eng.id);
        assert_eq!(renamed.tenant, "acme");
        assert_eq!(ops.tenant, "globex");
        assert_eq!(
            accounts.authorize(&eng.id, &bob.id).unwrap().role,
            Role::Member
        );
    }

    #[test]
    fn the_permission_matrix_refuses_escalation() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let admin = account(&accounts, "admin");
        let member = account(&accounts, "member");
        let ws = org(&accounts, &owner, None);

        let invite = accounts
            .invite(&owner.id, &ws.id, Role::Admin, 3600)
            .unwrap();
        accounts.accept(&admin.id, &invite.token).unwrap();
        let invite = accounts
            .invite(&owner.id, &ws.id, Role::Member, 3600)
            .unwrap();
        accounts.accept(&member.id, &invite.token).unwrap();

        // A member cannot invite, re-role, remove another, or touch the
        // workspace's seats, name, or tenant.
        assert!(matches!(
            accounts.invite(&member.id, &ws.id, Role::Member, 3600),
            Err(Refusal::Forbidden { action: "invite", .. })
        ));
        assert!(matches!(
            accounts.set_role(&member.id, &ws.id, &admin.id, Role::Member),
            Err(Refusal::Forbidden { action: "set-role", .. })
        ));
        assert!(matches!(
            accounts.remove_member(&member.id, &ws.id, &admin.id),
            Err(Refusal::Forbidden { action: "remove-member", .. })
        ));
        assert!(matches!(
            accounts.set_seats(&member.id, &ws.id, Some(10)),
            Err(Refusal::Forbidden { action: "set-seats", .. })
        ));

        // An admin invites and removes members, but cannot re-role,
        // remove a peer admin or the owner, transfer, or set seats.
        assert!(accounts
            .invite(&admin.id, &ws.id, Role::Member, 3600)
            .is_ok());
        assert!(matches!(
            accounts.set_role(&admin.id, &ws.id, &member.id, Role::Admin),
            Err(Refusal::Forbidden { action: "set-role", .. })
        ));
        assert!(matches!(
            accounts.remove_member(&admin.id, &ws.id, &owner.id),
            Err(Refusal::LastOwner { .. })
        ));
        assert!(matches!(
            accounts.transfer_ownership(&admin.id, &ws.id, &member.id),
            Err(Refusal::Forbidden { action: "transfer-ownership", .. })
        ));
        assert!(matches!(
            accounts.set_seats(&admin.id, &ws.id, Some(10)),
            Err(Refusal::Forbidden { action: "set-seats", .. })
        ));

        // Nobody writes ownership: not by invitation, not by role write.
        assert!(matches!(
            accounts.invite(&owner.id, &ws.id, Role::Owner, 3600),
            Err(Refusal::OwnerByInvitation { .. })
        ));
        assert!(matches!(
            accounts.set_role(&owner.id, &ws.id, &member.id, Role::Owner),
            Err(Refusal::OwnershipByTransfer { .. })
        ));
    }

    #[test]
    fn invitations_expire_replay_and_die_once() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let joiner = account(&accounts, "joiner");
        let ws = org(&accounts, &owner, None);

        // Malformed and unknown tokens are distinct refusals.
        assert!(matches!(
            accounts.accept(&joiner.id, "not-a-token"),
            Err(Refusal::MalformedInvitation)
        ));
        assert!(matches!(
            accounts.accept(&joiner.id, &format!("{INVITE_PREFIX}_nosuch.{}", "0".repeat(64))),
            Err(Refusal::UnknownInvitation(_))
        ));

        // An expired invitation refuses.
        let dead = accounts.invite(&owner.id, &ws.id, Role::Member, 0).unwrap();
        assert!(matches!(
            accounts.accept(&joiner.id, &dead.token),
            Err(Refusal::InvitationExpired(_))
        ));

        // A wrong secret is not a second chance.
        let invite = accounts.invite(&owner.id, &ws.id, Role::Admin, 3600).unwrap();
        let id = &invite.invitation.id;
        let wrong = format!("{INVITE_PREFIX}_{id}.{}", "f".repeat(64));
        assert!(matches!(
            accounts.accept(&joiner.id, &wrong),
            Err(Refusal::WrongSecret(_))
        ));

        // Acceptance consumes the token; a replay is closed, not joined —
        // for the account that already holds it and for anyone else.
        accounts.accept(&joiner.id, &invite.token).unwrap();
        assert!(matches!(
            accounts.accept(&joiner.id, &invite.token),
            Err(Refusal::InvitationClosed {
                status: InviteStatus::Accepted,
                ..
            })
        ));
        let other = account(&accounts, "other");
        assert!(matches!(
            accounts.accept(&other.id, &invite.token),
            Err(Refusal::InvitationClosed {
                status: InviteStatus::Accepted,
                ..
            })
        ));

        // A fresh invitation for an existing member is a different
        // refusal: the account already belongs.
        let again = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        assert!(matches!(
            accounts.accept(&joiner.id, &again.token),
            Err(Refusal::AlreadyMember { .. })
        ));
        accounts
            .revoke_invitation(&owner.id, &ws.id, &again.invitation.id)
            .unwrap();

        // A revoked invitation refuses whoever presents it.
        let invite = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        accounts
            .revoke_invitation(&owner.id, &ws.id, &invite.invitation.id)
            .unwrap();
        assert!(matches!(
            accounts.accept(&other.id, &invite.token),
            Err(Refusal::InvitationClosed {
                status: InviteStatus::Revoked,
                ..
            })
        ));

        // The store on disk carries the digest, never the secret.
        let live = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        let secret = live.token.split_once('.').unwrap().1;
        let text = std::fs::read_to_string(_dir.path().join(ACCOUNTS)).unwrap();
        assert!(!text.contains(secret), "{text}");
        assert!(text.contains(&live.invitation.digest));
    }

    #[test]
    fn seats_bound_members_and_live_invitations() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let ws = org(&accounts, &owner, Some(2));

        // Owner fills one seat; one live invitation holds the other.
        let first = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        assert!(matches!(
            accounts.invite(&owner.id, &ws.id, Role::Member, 3600),
            Err(Refusal::SeatLimit { seats: 2, .. })
        ));

        // Accepting fills the seat; the next invitation is refused too.
        let joiner = account(&accounts, "joiner");
        accounts.accept(&joiner.id, &first.token).unwrap();
        assert!(matches!(
            accounts.invite(&owner.id, &ws.id, Role::Member, 3600),
            Err(Refusal::SeatLimit { seats: 2, .. })
        ));

        // Seats cannot drop below the membership they would orphan.
        assert!(matches!(
            accounts.set_seats(&owner.id, &ws.id, Some(1)),
            Err(Refusal::SeatsBelowMembers {
                seats: 1,
                members: 2,
                ..
            })
        ));

        // Raising the bound frees invitations again.
        accounts.set_seats(&owner.id, &ws.id, Some(3)).unwrap();
        let held = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();

        // Removing a member does not free a seat a live invitation still
        // holds — the seat frees when the invitation lapses.
        accounts.set_seats(&owner.id, &ws.id, Some(2)).unwrap();
        accounts.remove_member(&owner.id, &ws.id, &joiner.id).unwrap();
        assert!(matches!(
            accounts.invite(&owner.id, &ws.id, Role::Member, 3600),
            Err(Refusal::SeatLimit { seats: 2, .. })
        ));
        accounts
            .revoke_invitation(&owner.id, &ws.id, &held.invitation.id)
            .unwrap();
        assert!(accounts
            .invite(&owner.id, &ws.id, Role::Member, 3600)
            .is_ok());
    }

    #[test]
    fn owner_transfer_and_last_owner_protection() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let next = account(&accounts, "next");
        let ws = org(&accounts, &owner, None);

        // The owner cannot leave or be removed — the workspace would
        // stand without an owner.
        assert!(matches!(
            accounts.remove_member(&owner.id, &ws.id, &owner.id),
            Err(Refusal::LastOwner { .. })
        ));

        // A non-member cannot receive ownership.
        assert!(matches!(
            accounts.transfer_ownership(&owner.id, &ws.id, &next.id),
            Err(Refusal::NotMember { .. })
        ));

        let invite = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        accounts.accept(&next.id, &invite.token).unwrap();

        // Transfer is atomic: the target owns, the old owner administers.
        accounts.transfer_ownership(&owner.id, &ws.id, &next.id).unwrap();
        assert_eq!(
            accounts.authorize(&ws.id, &next.id).unwrap().role,
            Role::Owner
        );
        assert_eq!(
            accounts.authorize(&ws.id, &owner.id).unwrap().role,
            Role::Admin
        );

        // The new owner may now remove the old owner — the workspace
        // keeps exactly one owner throughout.
        accounts.remove_member(&next.id, &ws.id, &owner.id).unwrap();
        assert!(matches!(
            accounts.authorize(&ws.id, &owner.id),
            Err(Refusal::Revoked { .. })
        ));
        assert_eq!(
            accounts.authorize(&ws.id, &next.id).unwrap().role,
            Role::Owner
        );
    }

    #[test]
    fn revocation_is_named_immediately_and_the_epoch_moves() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let member = account(&accounts, "member");
        let ws = org(&accounts, &owner, None);

        let invite = accounts.invite(&owner.id, &ws.id, Role::Member, 3600).unwrap();
        let membership = accounts.accept(&member.id, &invite.token).unwrap();
        let before = accounts.authorize(&ws.id, &member.id).unwrap();
        assert_eq!(before.epoch, membership.epoch);

        // Revocation is visible to the very next authorization, on any
        // handle — a second handle sees the same committed state.
        accounts.remove_member(&owner.id, &ws.id, &member.id).unwrap();
        let other = Accounts::open(_dir.path()).unwrap();
        assert!(matches!(
            other.authorize(&ws.id, &member.id),
            Err(Refusal::Revoked { .. })
        ));
        assert!(matches!(
            other.authorize(&ws.id, &member.id),
            Err(Refusal::Revoked { .. })
        ));
        let after = other.membership(&ws.id, &member.id).unwrap().unwrap();
        assert!(after.epoch > before.epoch);
        assert_eq!(after.status, MemberStatus::Revoked);

        // A revoked member cannot act — the membership answer precedes
        // the role check.
        assert!(matches!(
            other.invite(&member.id, &ws.id, Role::Member, 3600),
            Err(Refusal::Revoked { .. })
        ));
    }

    #[test]
    fn competing_writers_serialize_on_the_committed_store() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let ws = org(&accounts, &owner, Some(2));
        let dir = _dir.path().to_path_buf();

        // Two writers race for the workspace's one free seat. The store
        // serializes them: exactly one invitation commits, and the loser
        // decides against the winner's written state, not a stale read.
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let dir = dir.clone();
                let owner = owner.id.clone();
                let ws = ws.id.clone();
                std::thread::spawn(move || {
                    Accounts::open(&dir)
                        .unwrap()
                        .invite(&owner, &ws, Role::Member, 3600)
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        let issued = results.iter().filter(|result| result.is_ok()).count();
        let refused = results
            .iter()
            .filter(|result| matches!(result, Err(Refusal::SeatLimit { .. })))
            .count();
        assert_eq!(issued, 1, "{results:?}");
        assert_eq!(refused, 1, "{results:?}");

        // The committed chain is intact: genesis, the account, the
        // workspace, and the one winning invitation — each revision
        // supersedes the last and the digest recomputes.
        let store = accounts.store().unwrap();
        assert_eq!(store.sequence, 3);
        let archived = Accounts::revision(_dir.path(), &store.digest).unwrap();
        assert_eq!(archived.digest, store.digest);
    }

    #[test]
    fn a_personal_workspace_holds_its_owner_alone() {
        let (_dir, accounts) = installed();
        let alice = account(&accounts, "alice");
        let ws = accounts
            .create_workspace(
                &alice.id,
                "alice",
                WorkspaceKind::Personal,
                "alice-tenant",
                None,
            )
            .unwrap();
        assert!(matches!(
            accounts.invite(&alice.id, &ws.id, Role::Member, 3600),
            Err(Refusal::PersonalWorkspace(_))
        ));
        assert!(matches!(
            accounts.transfer_ownership(&alice.id, &ws.id, &alice.id),
            Err(Refusal::PersonalWorkspace(_))
        ));
        assert!(matches!(
            accounts.remove_member(&alice.id, &ws.id, &alice.id),
            Err(Refusal::LastOwner { .. })
        ));
    }

    #[test]
    fn principals_resolve_and_stay_unique() {
        let (_dir, accounts) = installed();
        let alice = accounts
            .create_account("alice", &["key:abc123".to_string()])
            .unwrap();
        assert!(matches!(
            accounts.create_account("mallory", &["key:abc123".to_string()]),
            Err(Refusal::PrincipalTaken { .. })
        ));
        assert_eq!(
            accounts.account_of_principal("key:abc123").unwrap(),
            Some(alice.id.clone())
        );
        accounts
            .update_principals(&alice.id, &["nostr:deadbeef".to_string()])
            .unwrap();
        assert_eq!(accounts.account_of_principal("key:abc123").unwrap(), None);
        assert_eq!(
            accounts
                .account_of_principal("nostr:deadbeef")
                .unwrap()
                .as_deref(),
            Some(alice.id.as_str())
        );
    }

    #[test]
    fn corrupt_and_incomplete_storage_fails_closed() {
        let dir = tempfile::tempdir().unwrap();

        // Missing entirely: an opened store is not an empty one.
        assert!(matches!(
            Accounts::open(dir.path()),
            Err(Trouble::Io(_))
        ));

        let accounts = Accounts::install(dir.path()).unwrap();
        let owner = account(&accounts, "owner");
        org(&accounts, &owner, None);

        // A digest that does not recompute refuses the whole store.
        let path = dir.path().join(ACCOUNTS);
        let text = std::fs::read_to_string(&path).unwrap();
        let tampered = text.replace("engineering", "engin33ring");
        std::fs::write(&path, tampered).unwrap();
        assert!(matches!(
            Accounts::open(dir.path()),
            Err(Trouble::Invalid(_))
        ));
        assert!(matches!(
            accounts.authorize("anything", "anyone"),
            Err(Refusal::Store(Trouble::Invalid(_)))
        ));

        // A torn write fails at parse, not partially.
        std::fs::write(&path, "{\"v\":\"openagents.tenancy.accounts.v1\",\"seq").unwrap();
        assert!(matches!(
            Accounts::open(dir.path()),
            Err(Trouble::Invalid(_))
        ));

        // A schema this crate does not know is refused.
        let mut store = Store {
            v: "openagents.tenancy.accounts.v0".to_string(),
            sequence: 0,
            supersedes: None,
            accounts: BTreeMap::new(),
            workspaces: BTreeMap::new(),
            invitations: BTreeMap::new(),
            digest: String::new(),
        };
        store.seal();
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&store).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            Accounts::open(dir.path()),
            Err(Trouble::Invalid(_))
        ));
    }

    #[test]
    fn owner_and_member_exit_rules() {
        let (_dir, accounts) = installed();
        let owner = account(&accounts, "owner");
        let admin = account(&accounts, "admin");
        let member = account(&accounts, "member");
        let ws = org(&accounts, &owner, None);
        for (who, role) in [(&admin, Role::Admin), (&member, Role::Member)] {
            let invite = accounts.invite(&owner.id, &ws.id, role, 3600).unwrap();
            accounts.accept(&who.id, &invite.token).unwrap();
        }

        // An admin cannot remove a peer admin — only members.
        let second_admin = account(&accounts, "second-admin");
        let invite = accounts.invite(&owner.id, &ws.id, Role::Admin, 3600).unwrap();
        accounts.accept(&second_admin.id, &invite.token).unwrap();
        assert!(matches!(
            accounts.remove_member(&admin.id, &ws.id, &second_admin.id),
            Err(Refusal::Forbidden { action: "remove-member", .. })
        ));

        // A member may leave; an admin may remove a member.
        accounts.remove_member(&member.id, &ws.id, &member.id).unwrap();
        assert!(matches!(
            accounts.authorize(&ws.id, &member.id),
            Err(Refusal::Revoked { .. })
        ));
    }
}
