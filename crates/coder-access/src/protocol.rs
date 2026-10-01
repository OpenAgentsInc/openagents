//! Closed NIP-HOST artifacts inside original signed private `3188` envelopes.
//! Parsing establishes shape. Only the host's current records admit an operation.
use crate::{Code, Error, Result, Right, Rights, fail};
use coder_connect::{RelayPolicy, pairing::Invitation};
use nostr::{contracts, domain::Event};
use secp256k1::{SecretKey, rand::RngCore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub use coder_connect::protocol::{identity, pubkey, random_id};

/// Host invitations use the observer's binary layout under their own prefix,
/// so a history-observer invitation can never be redeemed as host access.
pub const INVITATION_PREFIX: &str = "coder-host:";
pub const GRANT: &str = "openagents.host-grant.v1";
pub const ACCESS: &str = "openagents.host-access.v1";
pub const REQUEST: &str = "openagents.host-request.v1";
pub const REPLY: &str = "openagents.host-reply.v1";
pub const ENROLLMENT: &str = "openagents.host-enrollment-request.v1";
pub const MAX_GRANT_LIFETIME: u64 = 30 * 24 * 60 * 60;
pub const MAX_REQUEST_LIFETIME: u64 = 60;
pub const INVITATION_LIFETIME: u64 = coder_connect::pairing::LIFETIME;
pub const ENROLLMENT_LIFETIME: u64 = 300;
pub const MAX_CODE_ATTEMPTS: u32 = 5;
const MAX_SAFE: u64 = 9_007_199_254_740_991;
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A parsed host invitation. It holds a temporary capability: never log it.
pub struct HostInvitation(pub(crate) Invitation);
impl HostInvitation {
    pub fn parse(code: &str, now: u64, policy: RelayPolicy) -> Result<Self> {
        Invitation::parse_prefixed(INVITATION_PREFIX, code, now, policy)
            .map(Self)
            .map_err(|error| {
                let mut mapped = Error::from(error);
                mapped.message = "host invitation is malformed, expired, or not allowed".into();
                mapped
            })
    }
    /// The same invitation from another carriage, such as an
    /// `openagents-connect:` code, which carries the host key, ID,
    /// capability, and times but not the relay: the device supplies the
    /// relay the host named for it. Checked exactly as [`Self::parse`]
    /// checks a `coder-host:` string.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        host: &str,
        id: &str,
        capability: &str,
        relay: &str,
        issued_at: u64,
        expires_at: u64,
        now: u64,
        policy: RelayPolicy,
    ) -> Result<Self> {
        let bytes32 = |hex: &str| -> Result<Vec<u8>> {
            identity(hex).map_err(Error::from)?;
            Ok((0..32)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or_default())
                .collect())
        };
        public(host)?;
        let relay_len = u16::try_from(relay.len())
            .map_err(|_| Error::new(Code::Bounds, "relay exceeds its bound"))?;
        let mut bytes = vec![1];
        bytes.extend(bytes32(host)?);
        bytes.extend(bytes32(id)?);
        bytes.extend(bytes32(capability)?);
        bytes.extend(issued_at.to_be_bytes());
        bytes.extend(expires_at.to_be_bytes());
        bytes.extend(relay_len.to_be_bytes());
        bytes.extend(relay.as_bytes());
        let code = format!("{INVITATION_PREFIX}{}", base64url(&bytes));
        Self::parse(&code, now, policy)
    }
    pub fn host(&self) -> &str {
        &self.0.host
    }
    pub fn id(&self) -> &str {
        &self.0.id
    }
    pub fn relay(&self) -> &str {
        &self.0.relay
    }
    pub fn expires_at(&self) -> u64 {
        self.0.expires_at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    Invitation,
    Approval,
}
/// How the host admitted a grant and which principal authorized it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub kind: OriginKind,
    pub id: String,
    pub issuer: String,
}

/// A host-signed grant. It is not a bearer credential: every operation also
/// needs the device's own signature and the host's current record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub v: String,
    pub requires: Vec<String>,
    pub grant: String,
    pub host: String,
    pub owner: String,
    pub device: String,
    pub relay: String,
    pub rights: Rights,
    pub epoch: u64,
    pub origin: Origin,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl Grant {
    pub fn validate(&self, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, GRANT, &self.requires)?;
        identity(&self.grant).map_err(Error::from)?;
        identity(&self.origin.id).map_err(Error::from)?;
        for key in [&self.host, &self.owner, &self.device, &self.origin.issuer] {
            public(key)?;
        }
        distinct(&self.host, &self.owner, &self.device)?;
        policy.validate(&self.relay).map_err(Error::from)?;
        if self.epoch > MAX_SAFE {
            return fail(
                Code::Malformed,
                "grant epoch exceeds the safe integer range",
            );
        }
        window(self.issued_at, self.expires_at, MAX_GRANT_LIFETIME)
    }
}

/// The device's saved access record: its grant and the original host-signed
/// envelope. Verification offline cannot establish that the host has not revoked it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub v: String,
    pub requires: Vec<String>,
    pub grant: Grant,
    pub authorization: Event,
}
impl Access {
    /// Accept a host-signed grant envelope for this device from a pinned host.
    pub fn from_authorization(
        authorization: Event,
        secret: &SecretKey,
        host: &str,
        now: u64,
        policy: RelayPolicy,
    ) -> Result<Self> {
        let grant: Grant = open(&authorization, secret, host, &pubkey(secret), GRANT)?;
        let access = Self {
            v: ACCESS.into(),
            requires: vec![],
            grant,
            authorization,
        };
        access.verify(secret, now, policy)?;
        Ok(access)
    }
    /// Accept a renewal of this access: a new grant envelope the host sent
    /// unasked, for the same host, owner, device, relay, rights, epoch, and
    /// origin, issued no earlier than the current grant and expiring later.
    /// Store the result in place of `self` only on success; anything else
    /// leaves the current access as it is.
    pub fn renewed(
        &self,
        authorization: Event,
        secret: &SecretKey,
        now: u64,
        policy: RelayPolicy,
    ) -> Result<Self> {
        let next = Self::from_authorization(authorization, secret, &self.grant.host, now, policy)?;
        let (old, new) = (&self.grant, &next.grant);
        if new.grant == old.grant
            || new.owner != old.owner
            || new.relay != old.relay
            || new.rights != old.rights
            || new.epoch != old.epoch
            || new.origin != old.origin
            || new.issued_at < old.issued_at
            || new.expires_at <= old.expires_at
        {
            return fail(Code::Forbidden, "a renewal must keep the grant's terms");
        }
        Ok(next)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        decode(bytes)
    }
    pub fn verify(&self, secret: &SecretKey, now: u64, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, ACCESS, &self.requires)?;
        self.grant.validate(policy)?;
        if self.grant.device != pubkey(secret) {
            return fail(Code::Forbidden, "access belongs to another device key");
        }
        let signed: Grant = open(
            &self.authorization,
            secret,
            &self.grant.host,
            &self.grant.device,
            GRANT,
        )?;
        if encoded(&signed)? != encoded(&self.grant)?
            || self.authorization.tag_values("h").collect::<Vec<_>>() != [self.grant.grant.as_str()]
        {
            return fail(Code::Forbidden, "access and original signed grant differ");
        }
        fresh(self.grant.issued_at, self.grant.expires_at, now)
    }
}

/// A headless host's request, addressed to its owner and current administrators.
/// It never carries the short code; the approver reads that from the host's screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub v: String,
    pub requires: Vec<String>,
    pub enrollment: String,
    pub host: String,
    pub owner: String,
    pub relay: String,
    pub rights: Rights,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl Enrollment {
    pub fn validate(&self, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, ENROLLMENT, &self.requires)?;
        identity(&self.enrollment).map_err(Error::from)?;
        public(&self.host)?;
        public(&self.owner)?;
        if self.host == self.owner {
            return fail(Code::Forbidden, "host and owner keys must differ");
        }
        policy.validate(&self.relay).map_err(Error::from)?;
        window(self.issued_at, self.expires_at, ENROLLMENT_LIFETIME)
    }
    /// The exact artifact digest an approval must name.
    pub fn digest(&self) -> Result<String> {
        Ok(contracts::digest_bytes(&encoded(self)?))
    }
}

/// Generate an eight-character short code from 40 random bits.
pub fn short_code() -> String {
    let mut bytes = [0_u8; 5];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    let bits = bytes
        .iter()
        .fold(0_u64, |acc, b| (acc << 8) | u64::from(*b));
    let text: String = (0..8)
        .rev()
        .map(|i| CODE_ALPHABET[((bits >> (i * 5)) & 31) as usize] as char)
        .collect();
    format!("{}-{}", &text[..4], &text[4..])
}
/// Normalize a typed code: case, separators, and the ambiguous letters O, I, and L.
pub fn normalize_code(code: &str) -> Result<String> {
    if code.len() > 32 {
        return fail(Code::WrongCode, "short code exceeds its bound");
    }
    let normalized: String = code
        .chars()
        .filter(|c| *c != '-' && *c != ' ')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    if normalized.len() != 8 || !normalized.bytes().all(|b| CODE_ALPHABET.contains(&b)) {
        return fail(Code::WrongCode, "short code has the wrong shape");
    }
    Ok(normalized)
}
/// Bind the code to one enrollment so a digest cannot be reused across requests.
pub fn code_digest(enrollment: &str, code: &str) -> Result<String> {
    let normalized = normalize_code(code)?;
    Ok(contracts::digest_bytes(
        format!("openagents.host-enrollment-code.v1:{enrollment}:{normalized}").as_bytes(),
    ))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCreate {
    pub title: String,
    pub prompt: String,
    /// A host-scoped workspace label. The host resolves it; it is never a path.
    pub workspace: String,
}

/// What a durable task command asks for. The device picks it from task
/// state and its rights, never from the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandAction {
    /// Continue an ended task with a new turn.
    Send,
    /// Continue the task with a new turn after the current one ends.
    Queue,
    /// Steer the current turn, under the engine's stated semantics.
    Steer,
    /// Stop the current turn.
    Interrupt,
    /// Answer the engine's pending approval request or question.
    Answer,
}

/// A durable task command (`task.command`): the device mints `command`
/// once and replays it unchanged, however many NIP-HOST requests carry it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCommand {
    /// The device-minted command ID: 64 lowercase hexadecimal characters.
    pub command: String,
    /// The host-issued task ID.
    pub task: String,
    pub action: CommandAction,
    /// The task revision the device last read.
    pub based_on: u64,
    /// The message, or for `interrupt` a single-line reason.
    pub text: String,
    /// For `steer` only: the caller chooses the engine's emulated steering.
    pub emulate: bool,
    /// When the device minted the command, in Unix seconds.
    pub issued_at: u64,
}

impl TaskCommand {
    /// Check identities, bounds, and that `emulate` goes with `steer` only.
    ///
    /// # Errors
    /// `malformed` for a bad identity or flag, `bounds` for text.
    pub fn validate(&self) -> Result<()> {
        identity(&self.command).map_err(Error::from)?;
        identity(&self.task).map_err(Error::from)?;
        safe(self.based_on)?;
        safe(self.issued_at)?;
        if self.emulate && self.action != CommandAction::Steer {
            return fail(Code::Malformed, "only a steer chooses emulation");
        }
        match self.action {
            CommandAction::Interrupt => text(&self.text, 512)?,
            _ => {
                if self.text.trim().is_empty() || self.text.len() > 16 * 1024 {
                    return fail(Code::Bounds, "command text exceeds its bound");
                }
            }
        }
        Ok(())
    }
}

/// The most held messages a `queue` outcome lists, and a reorder names.
pub const MAX_QUEUE: usize = 64;

/// One change to a task's queued messages (`task.queue`). Changes need the
/// device's current edit lease.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueueEdit {
    /// Read the queue.
    List {},
    /// Take or renew the edit lease. While a device holds it, queued
    /// messages wait even when the turn ends.
    Lease {},
    /// Give the lease up.
    Release {},
    /// Replace the text of this device's own held message.
    Edit { command: String, text: String },
    /// Remove this device's own held message.
    Remove { command: String },
    /// Put the queued messages in this exact order.
    Reorder { commands: Vec<String> },
    /// Send this device's own held message now, as the engine's emulated
    /// steering: stop the turn and continue with it, ahead of the queue.
    SendNow { command: String },
}

impl QueueEdit {
    fn validate(&self) -> Result<()> {
        match self {
            Self::List {} | Self::Lease {} | Self::Release {} => Ok(()),
            Self::Remove { command } | Self::SendNow { command } => {
                identity(command).map_err(Error::from)
            }
            Self::Edit { command, text } => {
                identity(command).map_err(Error::from)?;
                if text.trim().is_empty() || text.len() > 16 * 1024 {
                    return fail(Code::Bounds, "queued message exceeds its bound");
                }
                Ok(())
            }
            Self::Reorder { commands } => {
                if commands.len() > MAX_QUEUE {
                    return fail(Code::Bounds, "too many queued messages");
                }
                for (index, command) in commands.iter().enumerate() {
                    identity(command).map_err(Error::from)?;
                    if commands[..index].contains(command) {
                        return fail(Code::Malformed, "a reorder names a message twice");
                    }
                }
                Ok(())
            }
        }
    }
}

/// A device's edit lease on a task's queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueLease {
    pub device: String,
    /// Host time it lapses unless renewed.
    pub expires_at: u64,
}

/// One held message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueItem {
    /// The command ID its device minted.
    pub command: String,
    /// The device that sent it.
    pub device: String,
    /// Its text, only for the device that sent it; null for another's.
    pub text: Option<String>,
    /// It runs before queued messages: an emulated steer waiting for its
    /// stop, or a message sent now.
    pub priority: bool,
}

/// A task's held messages, in the order they run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskQueue {
    pub task: String,
    /// The task's revision.
    pub revision: u64,
    pub lease: Option<QueueLease>,
    pub items: Vec<QueueItem>,
}

/// Typed operations. Each names the one right it requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "enroll.redeem")]
    Redeem {
        invitation: String,
        capability: String,
    },
    #[serde(rename = "enroll.approve")]
    Approve {
        enrollment: String,
        request_digest: String,
        code: String,
        device: String,
        rights: Rights,
        grant_expires_at: u64,
    },
    #[serde(rename = "enroll.deny")]
    Deny {
        enrollment: String,
        request_digest: String,
    },
    #[serde(rename = "invite.create")]
    Invite {
        rights: Rights,
        grant_expires_at: u64,
    },
    #[serde(rename = "invite.cancel")]
    CancelInvite { invitation: String },
    #[serde(rename = "device.list")]
    ListDevices {},
    #[serde(rename = "device.revoke")]
    Revoke { device: String },
    #[serde(rename = "task.create")]
    CreateTask { task: TaskCreate },
    #[serde(rename = "terminal.open")]
    OpenTerminal { cols: u16, rows: u16 },
    /// Replace a task's instructions, as a CTRL steer does. `revision` is the
    /// task revision the device last read.
    #[serde(rename = "task.steer")]
    SteerTask {
        task: String,
        revision: u64,
        prompt: String,
    },
    /// Request a task's cancellation, as a CTRL cancel does.
    #[serde(rename = "task.cancel")]
    CancelTask {
        task: String,
        revision: u64,
        reason: String,
    },
    /// Take a finished or cancelled task off every device's lists. It
    /// deletes nothing; the host's owner can restore it.
    #[serde(rename = "task.archive")]
    ArchiveTask { task: String },
    /// List the workspace labels `task.create` accepts on this host.
    #[serde(rename = "workspace.list")]
    ListWorkspaces {},
    /// A durable task command: send, queue, steer, interrupt, or answer.
    #[serde(rename = "task.command")]
    CommandTask { command: TaskCommand },
    /// List or edit a task's queued messages.
    #[serde(rename = "task.queue")]
    QueueTask { task: String, edit: QueueEdit },
    /// List the spend requests this host holds for the sender, and give it
    /// the sender's current spend grant for this host (phase 1 agent
    /// spending: `crate::spend`).
    #[serde(rename = "spend.list")]
    ListSpends { grant: Box<crate::spend::Grant> },
    /// Record the sender's receipt for one of those requests.
    #[serde(rename = "spend.settle")]
    SettleSpend { receipt: Box<crate::spend::Receipt> },
    /// Ask for a single-use `coder-pair:` invitation to the host's
    /// read-only Coder chats, so the sender reads the tasks it starts. A
    /// device asks after pairing by any path and again before the chat
    /// grant ends.
    #[serde(rename = "chats.invite")]
    InviteChats {},
    /// List the host's chat threads, newest first, without archived ones.
    #[serde(rename = "thread.list")]
    ListThreads {},
    /// Read one page of a thread: its newest turns, or those before
    /// `before`, and the reply streaming into it.
    #[serde(rename = "thread.read")]
    ReadThread { thread: String, before: Option<u64> },
    /// Append a message to a thread through the host, which asks
    /// OpenAgents for the reply. `request` is a send ID the device mints
    /// once and replays unchanged: the host appends a message once per ID,
    /// and different text under the same ID refuses as `conflict`.
    #[serde(rename = "thread.send")]
    SendThread {
        thread: String,
        request: String,
        text: String,
    },
    /// Stop receiving the reply streaming into a thread, in answer to the
    /// message whose send ID is `request` (null for a message sent without
    /// one). What streamed is kept as a stopped reply; the hosted worker may
    /// still finish. A stop naming a message the thread is not answering
    /// changes nothing, so a repeated stop, or one after the reply ended,
    /// is harmless.
    #[serde(rename = "thread.stop")]
    StopThread {
        thread: String,
        request: Option<String>,
    },
    /// Start Coder for a thread through the host's handoff. When the thread
    /// already names a task, the receipt names that task and no second task
    /// starts. The device replays its request ID; the handoff's own key
    /// keeps a second request from starting another task.
    #[serde(rename = "thread.run")]
    RunThread { thread: String },
    /// Read what a Coder task changed: the exact base and head revisions,
    /// the changed-file counts, and the diff as far as it fits
    /// ([`crate::review::TaskReview`]). A read with no effect.
    #[serde(rename = "task.review")]
    ReviewTask { task: String },
    /// Publish the reviewed change: commit exactly the reviewed tree and
    /// push it as the repository's policy says (onto its branch, or to a
    /// branch of its own with a draft pull request). `base`, `head_commit`,
    /// and `head` are the revisions the device reviewed; the host refuses a
    /// head the worktree has moved past. A retry is the same operation.
    #[serde(rename = "task.publish")]
    PublishTask {
        task: String,
        base: String,
        head_commit: String,
        head: String,
    },
}
impl Operation {
    /// A read with no effect, whose reply the host does not retain: an
    /// exact retry reads again. Its answer changes as the thread does.
    #[must_use]
    pub fn reads_only(&self) -> bool {
        matches!(
            self,
            Self::ListThreads {} | Self::ReadThread { .. } | Self::ReviewTask { .. }
        )
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Redeem { .. } => "enroll.redeem",
            Self::Approve { .. } => "enroll.approve",
            Self::Deny { .. } => "enroll.deny",
            Self::Invite { .. } => "invite.create",
            Self::CancelInvite { .. } => "invite.cancel",
            Self::ListDevices {} => "device.list",
            Self::Revoke { .. } => "device.revoke",
            Self::CreateTask { .. } => "task.create",
            Self::OpenTerminal { .. } => "terminal.open",
            Self::SteerTask { .. } => "task.steer",
            Self::CancelTask { .. } => "task.cancel",
            Self::ArchiveTask { .. } => "task.archive",
            Self::ListWorkspaces {} => "workspace.list",
            Self::CommandTask { .. } => "task.command",
            Self::QueueTask { .. } => "task.queue",
            Self::ListSpends { .. } => "spend.list",
            Self::SettleSpend { .. } => "spend.settle",
            Self::InviteChats {} => "chats.invite",
            Self::ListThreads {} => "thread.list",
            Self::ReadThread { .. } => "thread.read",
            Self::SendThread { .. } => "thread.send",
            Self::StopThread { .. } => "thread.stop",
            Self::RunThread { .. } => "thread.run",
            Self::ReviewTask { .. } => "task.review",
            Self::PublishTask { .. } => "task.publish",
        }
    }
    /// The right this operation requires. Redemption uses the invitation's
    /// capability instead of a grant, so it requires none.
    pub fn required(&self) -> Option<Right> {
        match self {
            Self::Redeem { .. } => None,
            Self::Approve { .. }
            | Self::Deny { .. }
            | Self::Invite { .. }
            | Self::CancelInvite { .. }
            | Self::Revoke { .. } => Some(Right::AccessAdmin),
            Self::ListDevices {} => Some(Right::AccessRead),
            Self::InviteChats {}
            | Self::ListThreads {}
            | Self::ReadThread { .. }
            | Self::ReviewTask { .. } => Some(Right::Observe),
            Self::CreateTask { .. }
            | Self::SteerTask { .. }
            | Self::CancelTask { .. }
            | Self::ArchiveTask { .. }
            | Self::ListWorkspaces {}
            | Self::CommandTask { .. }
            | Self::QueueTask { .. }
            | Self::ListSpends { .. }
            | Self::SettleSpend { .. }
            | Self::SendThread { .. }
            | Self::StopThread { .. }
            | Self::RunThread { .. }
            | Self::PublishTask { .. } => Some(Right::Operate),
            Self::OpenTerminal { .. } => Some(Right::Terminal),
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Redeem {
                invitation,
                capability,
            } => {
                identity(invitation).map_err(Error::from)?;
                identity(capability).map_err(Error::from)?;
            }
            Self::Approve {
                enrollment,
                request_digest,
                code,
                device,
                grant_expires_at,
                ..
            } => {
                identity(enrollment).map_err(Error::from)?;
                digest(request_digest)?;
                if code.is_empty() || code.len() > 32 || !code.is_ascii() {
                    return fail(Code::Malformed, "approval code exceeds its bound");
                }
                public(device)?;
                safe(*grant_expires_at)?;
            }
            Self::Deny {
                enrollment,
                request_digest,
            } => {
                identity(enrollment).map_err(Error::from)?;
                digest(request_digest)?;
            }
            Self::Invite {
                grant_expires_at, ..
            } => safe(*grant_expires_at)?,
            Self::CancelInvite { invitation } => identity(invitation).map_err(Error::from)?,
            Self::ListDevices {}
            | Self::ListWorkspaces {}
            | Self::InviteChats {}
            | Self::ListThreads {} => {}
            Self::ReadThread { thread, before } => {
                crate::thread::id(thread)?;
                if let Some(before) = before {
                    safe(*before)?;
                }
            }
            Self::SendThread {
                thread,
                request,
                text,
            } => {
                crate::thread::id(thread)?;
                crate::thread::id(request)?;
                crate::thread::message(text)?;
            }
            Self::StopThread { thread, request } => {
                crate::thread::id(thread)?;
                if let Some(request) = request {
                    crate::thread::id(request)?;
                }
            }
            Self::RunThread { thread } => crate::thread::id(thread)?,
            Self::ReviewTask { task } => identity(task).map_err(Error::from)?,
            Self::PublishTask {
                task,
                base,
                head_commit,
                head,
            } => {
                identity(task).map_err(Error::from)?;
                crate::review::revision(base)?;
                crate::review::revision(head_commit)?;
                crate::review::revision(head)?;
            }
            Self::Revoke { device } => public(device)?,
            Self::CreateTask { task } => {
                text(&task.title, 200)?;
                text(&task.workspace, 128)?;
                if task.prompt.is_empty() || task.prompt.len() > 16 * 1024 {
                    return fail(Code::Bounds, "task prompt exceeds its bound");
                }
            }
            Self::OpenTerminal { cols, rows } => {
                if !(1..=1000).contains(cols) || !(1..=1000).contains(rows) {
                    return fail(Code::Bounds, "terminal size exceeds its bound");
                }
            }
            Self::SteerTask {
                task,
                revision,
                prompt,
            } => {
                identity(task).map_err(Error::from)?;
                safe(*revision)?;
                if prompt.trim().is_empty() || prompt.len() > 16 * 1024 {
                    return fail(Code::Bounds, "steering prompt exceeds its bound");
                }
            }
            Self::CancelTask {
                task,
                revision,
                reason,
            } => {
                identity(task).map_err(Error::from)?;
                safe(*revision)?;
                text(reason, 512)?;
            }
            Self::ArchiveTask { task } => identity(task).map_err(Error::from)?,
            Self::CommandTask { command } => command.validate()?,
            Self::QueueTask { task, edit } => {
                identity(task).map_err(Error::from)?;
                edit.validate()?;
            }
            Self::ListSpends { grant } => grant.validate()?,
            Self::SettleSpend { receipt } => receipt.validate()?,
        }
        Ok(())
    }
}

/// A device or owner operation, addressed and encrypted to the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub host: String,
    pub grant: Option<String>,
    pub epoch: Option<u64>,
    pub relay: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub op: Operation,
}
impl Request {
    pub fn validate(&self, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, REQUEST, &self.requires)?;
        identity(&self.request).map_err(Error::from)?;
        public(&self.host)?;
        policy.validate(&self.relay).map_err(Error::from)?;
        match (&self.grant, self.epoch) {
            (Some(grant), Some(epoch)) => {
                identity(grant).map_err(Error::from)?;
                safe(epoch)?;
                if matches!(self.op, Operation::Redeem { .. }) {
                    return fail(Code::Malformed, "redemption must not name a grant");
                }
            }
            (None, None) => {}
            _ => return fail(Code::Malformed, "grant and epoch must appear together"),
        }
        window(self.issued_at, self.expires_at, MAX_REQUEST_LIFETIME)?;
        self.op.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Active,
    Revoked,
    Expired,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceEntry {
    pub device: String,
    pub grant: String,
    pub rights: Rights,
    pub epoch: u64,
    pub origin: OriginKind,
    pub issued_at: u64,
    pub expires_at: u64,
    pub state: DeviceState,
    /// Host time of the host's last authenticated request or direct channel
    /// from the device under this grant, or null when it has none. A device
    /// reads it as the host observed it; the device's own clock plays no part.
    #[serde(default)]
    pub last_seen: Option<u64>,
}
/// Host handling of a dispatched operation. It is not evidence that a task
/// finished or a terminal produced output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation: String,
    pub reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Granted {
        authorization: Box<Event>,
    },
    Denied {},
    Invitation {
        invitation: String,
        code: String,
        expires_at: u64,
    },
    Cancelled {
        invitation: String,
    },
    Devices {
        devices: Vec<DeviceEntry>,
    },
    Revoked {
        device: String,
        epoch: u64,
        grants: Vec<String>,
    },
    Dispatched {
        receipt: Receipt,
    },
    /// The workspace labels a device may name in `task.create`, sorted and
    /// distinct. A label names a host-side root; it is never a path.
    Workspaces {
        workspaces: Vec<String>,
    },
    /// A task's held messages after a `task.queue` operation.
    Queue {
        queue: TaskQueue,
    },
    /// The spend requests the host holds for the sender (`spend.list`), at
    /// most [`crate::spend::MAX_LISTED`].
    Spends {
        spends: Vec<crate::spend::Entry>,
    },
    /// The receipt the host recorded (`spend.settle`): the one sent, or an
    /// earlier final one for the same request.
    Settled {
        receipt: Box<crate::spend::Receipt>,
    },
    /// A single-use `coder-pair:` invitation to the host's read-only Coder
    /// chats (`chats.invite`), and when the chat grant it carries ends.
    Chats {
        invitation: String,
        expires_at: u64,
    },
    /// The host's chat threads (`thread.list`), newest first, at most
    /// [`crate::thread::MAX_THREADS`].
    Threads {
        threads: Vec<crate::thread::ThreadRow>,
    },
    /// One page of a thread (`thread.read`).
    Thread {
        thread: Box<crate::thread::ThreadPage>,
    },
    /// What a task changed (`task.review`).
    Review {
        review: Box<crate::review::TaskReview>,
    },
    /// A publication of a reviewed change (`task.publish`), including one
    /// that was refused or whose push is uncertain.
    Published {
        publication: Box<crate::review::Publication>,
    },
}

/// The longest `coder-pair:` invitation a `chats` outcome carries.
pub const MAX_CHAT_INVITATION: usize = 4096;

/// The most workspace labels a `workspaces` outcome carries.
pub const MAX_WORKSPACES: usize = 64;

impl Outcome {
    /// Check an outcome's own bounds. A `workspaces` list holds at most
    /// [`MAX_WORKSPACES`] sorted, distinct labels of 1 to 128 bytes without
    /// control characters.
    ///
    /// # Errors
    /// Refuses a list over its bounds, unsorted, or with a bad label.
    pub fn validate(&self) -> Result<()> {
        if let Self::Queue { queue } = self {
            identity(&queue.task).map_err(Error::from)?;
            if queue.items.len() > MAX_QUEUE {
                return fail(Code::Bounds, "too many queued messages");
            }
            for item in &queue.items {
                identity(&item.command).map_err(Error::from)?;
                public(&item.device)?;
                if item
                    .text
                    .as_ref()
                    .is_some_and(|text| text.len() > 16 * 1024)
                {
                    return fail(Code::Bounds, "queued message exceeds its bound");
                }
            }
            if let Some(lease) = &queue.lease {
                public(&lease.device)?;
            }
        }
        if let Self::Spends { spends } = self {
            if spends.len() > crate::spend::MAX_LISTED {
                return fail(Code::Bounds, "too many spend requests");
            }
            for entry in spends {
                entry.request.validate()?;
                if let Some(receipt) = &entry.receipt {
                    receipt.answers(&entry.request)?;
                }
            }
        }
        if let Self::Settled { receipt } = self {
            receipt.validate()?;
        }
        if let Self::Chats { invitation, .. } = self
            && (!invitation.starts_with(coder_connect::pairing::PREFIX)
                || invitation.len() > MAX_CHAT_INVITATION
                || !invitation.is_ascii())
        {
            return fail(Code::Malformed, "not a chat invitation");
        }
        if let Self::Threads { threads } = self {
            if threads.len() > crate::thread::MAX_THREADS {
                return fail(Code::Bounds, "too many threads");
            }
            for row in threads {
                row.validate()?;
            }
        }
        if let Self::Thread { thread } = self {
            thread.validate()?;
        }
        if matches!(self, Self::Threads { .. } | Self::Thread { .. })
            && serde_json::to_vec(self).map_or(true, |bytes| {
                bytes.len() > crate::thread::MAX_PAGE_BYTES + 1024
            })
        {
            return fail(Code::Bounds, "thread answer exceeds its bound");
        }
        if let Self::Review { review } = self {
            review.validate()?;
            if serde_json::to_vec(self).map_or(true, |bytes| {
                bytes.len() > crate::review::MAX_REVIEW_BYTES + 1024
            }) {
                return fail(Code::Bounds, "review exceeds its bound");
            }
        }
        if let Self::Published { publication } = self {
            publication.validate()?;
        }
        if let Self::Workspaces { workspaces } = self {
            if workspaces.len() > MAX_WORKSPACES {
                return fail(Code::Bounds, "too many workspaces");
            }
            for (index, label) in workspaces.iter().enumerate() {
                text(label, 128)?;
                if label.is_empty() || (index > 0 && workspaces[index - 1] >= *label) {
                    return fail(Code::Malformed, "workspaces must be sorted and distinct");
                }
            }
        }
        Ok(())
    }

    /// Whether this outcome is the one the operation can produce.
    pub fn answers(&self, op: &Operation) -> bool {
        match (op, self) {
            (Operation::Redeem { .. } | Operation::Approve { .. }, Self::Granted { .. })
            | (Operation::Deny { .. }, Self::Denied {})
            | (Operation::Invite { .. }, Self::Invitation { .. })
            | (Operation::CancelInvite { .. }, Self::Cancelled { .. })
            | (Operation::ListDevices {}, Self::Devices { .. })
            | (Operation::Revoke { .. }, Self::Revoked { .. })
            | (Operation::ListWorkspaces {}, Self::Workspaces { .. }) => true,
            (Operation::QueueTask { task, .. }, Self::Queue { queue }) => queue.task == *task,
            (Operation::ListSpends { .. }, Self::Spends { .. })
            | (Operation::InviteChats {}, Self::Chats { .. })
            | (Operation::ListThreads {}, Self::Threads { .. }) => true,
            (Operation::ReadThread { thread, .. }, Self::Thread { thread: page }) => {
                page.thread == *thread
            }
            (Operation::ReviewTask { task }, Self::Review { review }) => review.task == *task,
            (
                Operation::PublishTask {
                    task,
                    base,
                    head_commit,
                    head,
                },
                Self::Published { publication },
            ) => {
                publication.task == *task
                    && publication.base == *base
                    && publication.head_commit == *head_commit
                    && publication.head == *head
            }
            (Operation::SettleSpend { receipt }, Self::Settled { receipt: recorded }) => {
                recorded.request == receipt.request && recorded.grant == receipt.grant
            }
            (
                Operation::CreateTask { .. }
                | Operation::OpenTerminal { .. }
                | Operation::SteerTask { .. }
                | Operation::CancelTask { .. }
                | Operation::ArchiveTask { .. }
                | Operation::CommandTask { .. }
                | Operation::SendThread { .. }
                | Operation::StopThread { .. }
                | Operation::RunThread { .. },
                Self::Dispatched { receipt },
            ) => receipt.operation == op.name(),
            _ => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplyResult {
    Ok { outcome: Outcome },
    Refused { code: Code, missing: Option<Right> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub request_event: String,
    pub host: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub result: ReplyResult,
}

pub(crate) fn schema(actual: &str, expected: &str, requires: &[String]) -> Result<()> {
    if actual != expected || !requires.is_empty() {
        return fail(
            Code::Unsupported,
            "unsupported host access schema or feature",
        );
    }
    Ok(())
}
pub(crate) fn public(key: &str) -> Result<()> {
    coder_connect::protocol::public(key).map_err(Error::from)
}
pub(crate) fn distinct(host: &str, owner: &str, device: &str) -> Result<()> {
    if host == owner || host == device || owner == device {
        return fail(Code::Forbidden, "host, owner, and device keys must differ");
    }
    Ok(())
}
pub(crate) fn window(issued: u64, expires: u64, max: u64) -> Result<()> {
    coder_connect::protocol::window(issued, expires, max)
        .map_err(|_| Error::new(Code::Malformed, "invalid host access lifetime"))
}
/// Unpadded base64url (RFC 4648, section 5), the layout host invitations
/// use.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    out
}
pub(crate) fn fresh(issued: u64, expires: u64, now: u64) -> Result<()> {
    coder_connect::protocol::fresh(issued, expires, now)
        .map_err(|_| Error::new(Code::Expired, "host access artifact is not current"))
}
fn safe(value: u64) -> Result<()> {
    if value > MAX_SAFE {
        return fail(Code::Malformed, "integer exceeds the safe range");
    }
    Ok(())
}
fn digest(value: &str) -> Result<()> {
    match value.strip_prefix("sha256:") {
        Some(hex) if identity(hex).is_ok() => Ok(()),
        _ => fail(Code::Malformed, "digest must be sha256 lower-case hex"),
    }
}
fn text(value: &str, max: usize) -> Result<()> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return fail(Code::Bounds, "text field exceeds its bound");
    }
    Ok(())
}
pub(crate) fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    coder_connect::protocol::encoded(value).map_err(Error::from)
}
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    coder_connect::protocol::decode(bytes)
        .map_err(|_| Error::new(Code::Malformed, "invalid bounded host access JSON"))
}
pub(crate) fn seal(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    recipient: &str,
    mailbox: &str,
    issued: u64,
    expires: u64,
) -> Result<Event> {
    coder_connect::protocol::seal(value, schema, secret, recipient, mailbox, issued, expires)
        .map_err(Error::from)
}
pub(crate) fn open<T: DeserializeOwned>(
    event: &Event,
    secret: &SecretKey,
    signer: &str,
    recipient: &str,
    schema: &str,
) -> Result<T> {
    coder_connect::protocol::open(event, secret, signer, recipient, schema).map_err(|error| {
        let mut mapped = Error::from(error);
        mapped.message = "host access envelope signer, recipient, or schema differs".into();
        mapped
    })
}
/// Read an artifact's schema without trusting its content.
pub(crate) fn schema_of(event: &Event, secret: &SecretKey) -> Result<String> {
    if event.content.len() > 400 * 1024 {
        return fail(
            Code::Bounds,
            "encrypted host access event exceeds its bound",
        );
    }
    let opened = nostr::private_artifact::open(event, secret)
        .map_err(|_| Error::new(Code::Forbidden, "invalid encrypted host access artifact"))?;
    opened
        .artifact()
        .schema
        .clone()
        .ok_or_else(|| Error::new(Code::Malformed, "host access artifact has no schema"))
}
