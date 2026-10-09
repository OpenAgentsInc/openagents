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
pub(crate) const MAX_SAFE: u64 = 9_007_199_254_740_991;
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
    /// Images the device sent first with `artifact.put`, by digest. The
    /// host binds the verified bytes it holds for this device to the task;
    /// a host that holds none of them refuses. Absent means none, so a task
    /// without images keeps its bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<crate::media::ImageRef>,
    /// The coding engine the person asked for (#10081): the typed `engine`
    /// of the chat's NIP-CJ `run_coder` offer, never read from text. The
    /// host puts it first only among the routes its owner's policy admits,
    /// and says plainly when it can't; it never adds a route, a model, or a
    /// limit. Absent means no preference, so a task without one keeps its
    /// bytes. A host that predates it rejects the field, so a device sends
    /// it only to a host whose presence advertises [`TASK_ENGINE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<nostr::cj_conversation::Engine>,
}

/// The presence capability a host advertises when its `task.create`
/// accepts [`TaskCreate::engine`] (#10081).
pub const TASK_ENGINE: &str = "task-engine";

/// The presence capability a host advertises while a device's coding reply
/// starts Coder there at once (#10101): its owner's `coder.start` setting is
/// `at_once` and its auto-start policy is on, so a `task.create` a device
/// sends for a chat's typed `run_coder` offer runs without anyone tapping
/// **Run Coder**. Its absence means "ask": a host that predates it, a
/// `coder.start: ask_first` owner, or a host whose created tasks wait inert.
/// It is a presentation hint, never authority: the host still checks the
/// device's grant and `operate` right, and its policy still decides whether
/// and how the task runs.
pub const CODER_START_AT_ONCE: &str = "coder-start-at-once";

/// The prefix of a presence capability that names one coding agent on the
/// host and its state (#10119): `engine-<state>-<engine>`, such as
/// `engine-ready-codex` or `engine-not_enabled-devin`. A host lists every
/// coding agent installed or signed in there, the ones its owner's settings
/// allow first, so a paired device can name them in its chat. A reader that
/// predates it ignores the flags as unknown capabilities. The flags carry a
/// closed state and the engine's word only, never an account, a token, or a
/// usage figure, and grant nothing.
pub const ENGINE_FLAG: &str = "engine-";

/// The states an [`ENGINE_FLAG`] carries: ready, not signed in, at its
/// usage limit, or installed but not allowed by the owner's settings.
pub const ENGINE_STATES: [&str; 4] = ["ready", "not_signed_in", "limited", "not_enabled"];

/// The most [`ENGINE_FLAG`] capabilities one presence carries or a reader
/// takes.
pub const MAX_ENGINE_FLAGS: usize = 8;

/// Whether `engine` is a coding agent's word: 1 to 16 lowercase ASCII
/// letters, digits, `-`, or `_`.
fn engine_word(engine: &str) -> bool {
    (1..=16).contains(&engine.len())
        && engine
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// The presence capability for `engine` in `state` (#10119), or `None` for
/// a word or state outside [`ENGINE_FLAG`]'s bounds.
#[must_use]
pub fn engine_flag(engine: &str, state: &str) -> Option<String> {
    (engine_word(engine) && ENGINE_STATES.contains(&state))
        .then(|| format!("{ENGINE_FLAG}{state}-{engine}"))
}

/// The coding agents a host's presence names (#10119), as `(engine,
/// state)` in the order it lists them: at most [`MAX_ENGINE_FLAGS`], each
/// engine once, and a flag with a state this reader does not know left out.
#[must_use]
pub fn engine_flags<'a>(
    capabilities: impl IntoIterator<Item = &'a str>,
) -> Vec<(String, &'static str)> {
    let mut engines: Vec<(String, &'static str)> = Vec::new();
    for capability in capabilities {
        let Some(rest) = capability.strip_prefix(ENGINE_FLAG) else {
            continue;
        };
        let Some((state, engine)) = ENGINE_STATES.iter().find_map(|state| {
            rest.strip_prefix(state)
                .and_then(|rest| rest.strip_prefix('-'))
                .map(|engine| (*state, engine))
        }) else {
            continue;
        };
        if engine_word(engine) && !engines.iter().any(|(known, _)| known == engine) {
            engines.push((engine.to_owned(), state));
        }
        if engines.len() == MAX_ENGINE_FLAGS {
            break;
        }
    }
    engines
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
    #[serde(rename = "cloud.projects")]
    CloudProjects { workspace: String },
    #[serde(rename = "project.list")]
    ProjectList { workspace: String },
    #[serde(rename = "project.read")]
    ProjectRead { query: crate::project::Query },
    #[serde(rename = "project.original")]
    ProjectOriginal {
        query: crate::project::OriginalQuery,
    },
    #[serde(rename = "cloud.catalog")]
    CloudCatalog { query: crate::cloud::CatalogQuery },
    #[serde(rename = "cloud.list")]
    CloudList { query: crate::cloud::ListQuery },
    #[serde(rename = "cloud.read")]
    CloudRead { query: crate::cloud::ReadQuery },
    #[serde(rename = "cloud.original")]
    CloudOriginal { query: crate::cloud::OriginalQuery },
    #[serde(rename = "cloud.submit")]
    CloudSubmit { intent: crate::cloud::Submit },
    #[serde(rename = "cloud.continue")]
    CloudContinue { intent: crate::cloud::Continue },
    #[serde(rename = "cloud.cancel")]
    CloudCancel { intent: crate::cloud::Cancel },
    #[serde(rename = "cloud.follow")]
    CloudFollow { intent: crate::cloud::Follow },
    /// Release the user's own Claude credential for one turn of their job
    /// (BYO-05). Neither the request nor its reply is retained.
    #[serde(rename = "cloud.release")]
    CloudRelease { intent: crate::cloud::Release },
    #[serde(rename = "environment.read")]
    EnvironmentRead { query: crate::environment::Query },
    #[serde(rename = "environment.evidence")]
    EnvironmentEvidence {
        query: crate::environment::EvidenceQuery,
    },
    #[serde(rename = "environment.promote")]
    EnvironmentPromote { intent: crate::environment::Promote },
    #[serde(rename = "environment.select")]
    EnvironmentSelect { intent: crate::environment::Select },
    #[serde(rename = "environment.steer")]
    EnvironmentSteer { intent: crate::environment::Steer },
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
    /// Read the retained result of an original effect without dispatching it.
    #[serde(rename = "request.operation")]
    RequestOperation {
        request: String,
        request_event: String,
    },
    /// Read canonical tasks under an explicitly admitted host workspace label.
    #[serde(rename = "task.list")]
    ListTasks { query: crate::task_read::ListQuery },
    /// Read one pinned canonical task and intact original ATIF steps.
    #[serde(rename = "task.read")]
    ReadTask { query: crate::task_read::PageQuery },
    /// Read a pinned original task, trace, manifest, or retained artifact chunk.
    #[serde(rename = "task.original")]
    ReadTaskOriginal {
        query: crate::task_read::OriginalQuery,
    },
    #[serde(rename = "terminal.open")]
    OpenTerminal { cols: u16, rows: u16 },
    #[serde(rename = "task.terminal.open")]
    OpenTaskTerminal { task: String, cols: u16, rows: u16 },
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
    /// Apply a command only at the exact task revision it names.
    #[serde(rename = "task.command.at_revision")]
    CommandTaskAtRevision { command: TaskCommand, revision: u64 },
    /// List or edit a task's queued messages.
    #[serde(rename = "task.queue")]
    QueueTask { task: String, edit: QueueEdit },
    /// Read a queue or apply one request-keyed edit at its exact snapshot.
    #[serde(rename = "task.queue.at_revision")]
    QueueTaskAtRevision {
        task: String,
        revision: u64,
        edit: QueueEdit,
        queue_digest: Option<String>,
    },
    /// List the spend requests this host holds for the sender, and give it
    /// the sender's current spend grant for this host (phase 1 agent
    /// spending: `crate::spend`).
    #[serde(rename = "spend.list")]
    ListSpends { grant: Box<crate::spend::Grant> },
    /// Record the sender's receipt for one of those requests.
    #[serde(rename = "spend.settle")]
    SettleSpend { receipt: Box<crate::spend::Receipt> },
    /// List the computer's open asks for the owner's wallet
    /// (`openagents wallet link`, [`crate::wallet_link`]).
    #[serde(rename = "wallet.link.list")]
    ListWalletLinks {},
    /// Answer one ask: the wallet seed sealed to its key, or `None` when the
    /// owner declined.
    #[serde(rename = "wallet.link.answer")]
    AnswerWalletLink {
        id: String,
        sealed: Option<crate::wallet_link::Sealed>,
    },
    /// Ask for a single-use `coder-pair:` invitation to the host's
    /// read-only Coder chats, so the sender reads the tasks it starts. A
    /// device asks after pairing by any path and again before the chat
    /// grant ends.
    #[serde(rename = "chats.invite")]
    InviteChats {},
    /// Ask for the owner's private Verse placements
    /// (`docs/verse/private-assets.md`), naming the sender's Verse world
    /// key, which the host notes so the owner can grant it a reader. A
    /// read: it grants nothing, and only the asset's manifest decides who
    /// may load a pack.
    #[serde(rename = "verse.private")]
    VersePrivate { world_key: String },
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
    /// One chunk of an image for a task this device is about to create
    /// (`crate::media`). Idempotent: the host keeps the bytes for this
    /// device only and answers what it holds.
    #[serde(rename = "artifact.put")]
    PutArtifact { artifact: crate::media::ArtifactPut },
    /// The computer itself (`crate::computer`): a screenshot, its open
    /// apps, or a file chunk read or written. Needs `terminal`. Not
    /// retained: a read changes nothing and a write chunk is idempotent
    /// where the host keeps it.
    #[serde(rename = "computer")]
    Computer { computer: crate::computer::Request },
    /// The host's background rules (the disk cleanup monitor), each with
    /// its state and last result. A read.
    #[serde(rename = "background.list")]
    ListBackground {},
    /// One background rule's definition, version, and digest. A read.
    #[serde(rename = "background.show")]
    ShowBackground { rule: String },
    /// The background audit log, newest last, for one rule or all, from
    /// `since` (seconds since the epoch). A read.
    #[serde(rename = "background.log")]
    LogBackground {
        rule: Option<String>,
        since: Option<u64>,
    },
    /// Run a background rule now. The run happens on the host's runner;
    /// its result reaches the log and `background.list`. Asking twice runs
    /// it twice, which frees nothing more once its goal is met.
    #[serde(rename = "background.run")]
    RunBackground { rule: String },
    /// Pause a background rule (until a time, or until resumed), or resume
    /// it. Repeating it changes nothing more.
    #[serde(rename = "background.pause")]
    PauseBackground {
        rule: String,
        until: Option<u64>,
        resume: bool,
    },
    /// The Agent Studio now, in full ([`crate::studio::Snapshot`]). A
    /// read.
    #[serde(rename = "studio.snapshot")]
    StudioSnapshot {},
    /// What changed in the studio since `since` in `stream`
    /// ([`crate::studio::Update`]). A read. A host that no longer holds
    /// that point refuses as `stale`, and the client reads a snapshot.
    #[serde(rename = "studio.update")]
    StudioUpdate { stream: String, since: u64 },
    /// Start a goal on an admitted repository: the lead seat (the first
    /// lead when `lead` is null) plans it. `workspace` is a host label,
    /// never a path.
    #[serde(rename = "studio.goal.submit")]
    SubmitGoal {
        text: String,
        workspace: String,
        lead: Option<String>,
    },
    /// Message one seat, or every seat when `seat` is null. A running task
    /// reads it through the steer path; otherwise its next briefing does.
    #[serde(rename = "studio.seat.message")]
    MessageSeat { seat: Option<String>, text: String },
    /// Pause a seat: it keeps its task and takes no new one.
    #[serde(rename = "studio.seat.pause")]
    PauseSeat { seat: String },
    /// Resume a paused seat.
    #[serde(rename = "studio.seat.resume")]
    ResumeSeat { seat: String },
    /// Stop a seat: cancel its active task, return that task to the board
    /// as planned, and pause the seat.
    #[serde(rename = "studio.seat.stop")]
    StopSeat { seat: String },
    /// Give a planned task to another seat.
    #[serde(rename = "studio.task.reassign")]
    ReassignTask { task: String, seat: String },
    /// Cancel a planned or running studio task.
    #[serde(rename = "studio.task.cancel")]
    CancelStudioTask { task: String },
    /// Plan a failed or cancelled studio task again, under a new task
    /// identity.
    #[serde(rename = "studio.task.retry")]
    RetryTask { task: String },
    /// Move a planned task ahead of its goal's other planned tasks.
    #[serde(rename = "studio.task.prioritize")]
    PrioritizeTask { task: String },
    /// Answer an open studio decision: a task's question or approval,
    /// through the existing `answer` command keyed by `command`; or a
    /// goal's plan decision, with a plan. `based_on` is the decision's
    /// own `based_on`; another refuses as `stale`.
    #[serde(rename = "studio.decision.answer")]
    AnswerDecision {
        decision: String,
        based_on: u64,
        text: String,
        command: String,
        issued_at: u64,
    },
    /// Approve a waiting studio task's step and keep a standing rule for
    /// its seat: **Always allow for this seat**. `rule` is the exact text
    /// the approval offered; the host records the rule only when its own
    /// text for the step still matches, and refuses otherwise as `stale`.
    /// The approval itself takes the `studio.decision.answer` path under
    /// `command`. The host applies the rule to later matching steps of
    /// that seat; it never widens a grant.
    #[serde(rename = "studio.decision.always")]
    AllowAlways {
        decision: String,
        based_on: u64,
        rule: String,
        command: String,
        issued_at: u64,
    },
    /// Read a studio task's review: files, counts, diff, and the three
    /// revisions a merge decision binds to. A read.
    #[serde(rename = "studio.review.open")]
    OpenReview { task: String },
    /// **Merge**, **Request changes**, or **Reject** a studio task at the
    /// reviewed revisions. A worktree that changed since refuses as
    /// `stale`.
    #[serde(rename = "studio.merge.decide")]
    DecideMerge {
        decision: Box<crate::studio::MergeDecision>,
    },
    /// The host's workshop agents ([`crate::agent::Agents`]). A read.
    #[serde(rename = "studio.agent.list")]
    ListAgents {},
    /// A request for a workshop agent. The host checks it, journals it,
    /// and runs it; with `typist`, the asking device's pane types the
    /// agent's commands and reports each with `studio.agent.ran`.
    #[serde(rename = "studio.agent.ask")]
    AskAgent {
        agent: String,
        text: String,
        workspace: Option<String>,
        context: String,
        mode: crate::agent::Mode,
        typist: bool,
        /// The computer a task-mode request must run on: `local`, a
        /// connected computer's name, or absent for the policy's
        /// placement (#10930).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        computer: Option<String>,
    },
    /// CONFIRM or REJECT the agent's waiting proposal `step`.
    #[serde(rename = "studio.agent.answer")]
    AnswerAgent {
        agent: String,
        step: u64,
        confirm: bool,
    },
    /// How the command of step `step` ended in the device's pane.
    #[serde(rename = "studio.agent.ran")]
    AgentRan {
        agent: String,
        step: u64,
        ran: crate::agent::Ran,
    },
    /// Stop the agent: its standing jobs go off, its panes are released
    /// with `Ctrl+C`, its tasks are cancelled, and it starts nothing new.
    #[serde(rename = "studio.agent.stop")]
    StopAgent { agent: String, reason: String },
    /// The agent's memory ([`crate::agent::Memory`]). A read.
    #[serde(rename = "studio.agent.memory.list")]
    ListAgentMemory { agent: String, after: Option<u64> },
    /// Add a note, forget an entry, or accept or reject a preference.
    #[serde(rename = "studio.agent.memory.edit")]
    EditAgentMemory {
        agent: String,
        edit: crate::agent::MemoryEdit,
    },
    /// The agent's standing jobs ([`crate::agent::Jobs`]). A read.
    #[serde(rename = "studio.agent.jobs.list")]
    ListAgentJobs { agent: String },
    /// Pause, resume, or delete a standing job.
    #[serde(rename = "studio.agent.jobs.edit")]
    EditAgentJobs {
        agent: String,
        edit: crate::agent::JobEdit,
    },
    /// The agent's journal after entry `after` ([`crate::agent::Journal`]).
    /// A read.
    #[serde(rename = "studio.agent.log")]
    AgentLog { agent: String, after: Option<u64> },
    /// The Git checkouts the host offers a new agent as her workspace
    /// ([`crate::agent::Places`]). A read.
    #[serde(rename = "studio.agent.workspaces")]
    ListAgentWorkspaces {},
    /// Make the agent, working in the checkout `workspace`, with a key of
    /// her own that the host attests with the owner key it holds. Only
    /// the owner's own key may send it; a granted device may not.
    #[serde(rename = "studio.agent.new")]
    NewAgent { agent: String, workspace: String },
    /// Owner-only crew creation on the shared identity and runtime.
    #[serde(rename = "studio.agent.crew.new")]
    NewCrewAgent {
        agent: String,
        workspace: String,
        job_role: crate::crew::JobRole,
    },
    /// Owner-only cohort observation; no admission, resume, or send approval.
    #[serde(rename = "studio.agent.crew.status")]
    CrewStatus {},
    /// Owner-only cohort lifecycle and pending-subject revocation.
    #[serde(rename = "studio.agent.crew.control")]
    ControlCrew { control: crate::crew::Control },
    /// A durable proposal to hire or retire a sales member. Recording it
    /// creates nothing; only [`Self::DecideHire`] from the owner's key acts.
    #[serde(rename = "studio.agent.crew.hire.propose")]
    ProposeHire { proposal: crate::crew::HireProposal },
    /// Owner-only, single-use decision on one exact proposal digest. A
    /// confirmed hire is made in `workspace` through the shared crew path.
    #[serde(rename = "studio.agent.crew.hire.decide")]
    DecideHire {
        decision: crate::crew::HireDecision,
        workspace: Option<String>,
    },
    /// Owner-only read of every proposal and decision.
    #[serde(rename = "studio.agent.crew.hire.list")]
    ListHires {},
    /// Owner-only machine charter edit. It grants no tools or access rights.
    #[serde(rename = "studio.agent.charter.set")]
    SetAgentCharter {
        agent: String,
        job_role: crate::crew::JobRole,
        expected: u64,
        drafting: bool,
        purpose: String,
    },
    /// Owner-recorded evidence recommendation; never an action approval.
    #[serde(rename = "studio.agent.verdict.record")]
    RecordAgentVerdict {
        agent: String,
        verdict: crate::crew::VerdictInput,
    },
    #[serde(rename = "studio.agent.verdict.list")]
    ListAgentVerdicts { agent: String },
    /// Retire the agent: stop her, delete her key from the host's key
    /// store, and keep her journal and engrams, which the owner key still
    /// reads. With relay sync on and the owner key at the host, the owner
    /// asks her relays to archive her key (NIP-IA). Only the owner's own
    /// key may send it.
    #[serde(rename = "studio.agent.retire")]
    RetireAgent { agent: String },
    /// Rotate the agent's key with the owner key the host holds: a new
    /// key, every engram encrypted again under it, and the owner's
    /// lineage record. Only the owner's own key may send it.
    #[serde(rename = "studio.agent.rotate")]
    RotateAgent { agent: String, reason: String },
}
impl Operation {
    /// A read with no effect, whose reply the host does not retain: an
    /// exact retry reads again. Its answer changes as the thread does.
    #[must_use]
    pub fn reads_only(&self) -> bool {
        matches!(
            self,
            Self::RequestOperation { .. }
                | Self::CloudProjects { .. }
                | Self::ProjectList { .. }
                | Self::ProjectRead { .. }
                | Self::ProjectOriginal { .. }
                | Self::CloudCatalog { .. }
                | Self::CloudList { .. }
                | Self::CloudRead { .. }
                | Self::CloudOriginal { .. }
                | Self::EnvironmentRead { .. }
                | Self::EnvironmentEvidence { .. }
                | Self::QueueTaskAtRevision {
                    edit: QueueEdit::List {},
                    ..
                }
                | Self::ListThreads {}
                | Self::ListTasks { .. }
                | Self::ReadTask { .. }
                | Self::ReadTaskOriginal { .. }
                | Self::ReadThread { .. }
                | Self::ReviewTask { .. }
                | Self::VersePrivate { .. }
                | Self::StudioSnapshot {}
                | Self::StudioUpdate { .. }
                | Self::OpenReview { .. }
                | Self::ListAgents {}
                | Self::ListAgentMemory { .. }
                | Self::ListAgentJobs { .. }
                | Self::ListAgentVerdicts { .. }
                | Self::CrewStatus {}
                | Self::ListHires {}
                | Self::AgentLog { .. }
                | Self::ListAgentWorkspaces {}
        )
    }

    /// A `studio.agent.*` operation, answered by [`Outcome::Agent`]. Only
    /// the replies of [`Self::agent_effect`] are retained; the agent host
    /// also keys an ask by its ID and exact content in a durable ledger, so
    /// a retry asks once even after a restart.
    #[must_use]
    pub fn agent(&self) -> bool {
        matches!(
            self,
            Self::ListAgents {}
                | Self::AskAgent { .. }
                | Self::AnswerAgent { .. }
                | Self::AgentRan { .. }
                | Self::StopAgent { .. }
                | Self::ListAgentMemory { .. }
                | Self::EditAgentMemory { .. }
                | Self::ListAgentJobs { .. }
                | Self::EditAgentJobs { .. }
                | Self::ListAgentVerdicts { .. }
                | Self::AgentLog { .. }
                | Self::ListAgentWorkspaces {}
                | Self::NewCrewAgent { .. }
                | Self::SetAgentCharter { .. }
                | Self::CrewStatus {}
                | Self::ControlCrew { .. }
                | Self::ProposeHire { .. }
                | Self::DecideHire { .. }
                | Self::ProposeHire { .. }
                | Self::DecideHire { .. }
                | Self::ListHires {}
                | Self::RecordAgentVerdict { .. }
                | Self::NewAgent { .. }
                | Self::RetireAgent { .. }
                | Self::RotateAgent { .. }
        )
    }

    /// A `studio.agent.*` operation only the owner's own key sends, never
    /// a granted device: making, retiring, or rotating an agent.
    #[must_use]
    pub fn owner_agent(&self) -> bool {
        matches!(
            self,
            Self::NewAgent { .. }
                | Self::NewCrewAgent { .. }
                | Self::SetAgentCharter { .. }
                | Self::CrewStatus {}
                | Self::ControlCrew { .. }
                | Self::ProposeHire { .. }
                | Self::DecideHire { .. }
                | Self::ProposeHire { .. }
                | Self::DecideHire { .. }
                | Self::ListHires {}
                | Self::RecordAgentVerdict { .. }
                | Self::RetireAgent { .. }
                | Self::RotateAgent { .. }
        )
    }

    /// A `studio.*` intent the host hands its task owner, answered by a
    /// `dispatched` receipt.
    #[must_use]
    pub fn studio_intent(&self) -> bool {
        matches!(
            self,
            Self::SubmitGoal { .. }
                | Self::MessageSeat { .. }
                | Self::PauseSeat { .. }
                | Self::ResumeSeat { .. }
                | Self::StopSeat { .. }
                | Self::ReassignTask { .. }
                | Self::CancelStudioTask { .. }
                | Self::RetryTask { .. }
                | Self::PrioritizeTask { .. }
                | Self::AnswerDecision { .. }
                | Self::AllowAlways { .. }
        )
    }
    /// An operation the host's same-user control socket takes for its
    /// owner: the task operations, and the Agent Studio's reads, intents,
    /// reviews, and merge decisions, so a view on the host's own computer
    /// reaches the studio without a device grant.
    #[must_use]
    pub fn local_task(&self) -> bool {
        matches!(
            self,
            Self::CreateTask { .. }
                | Self::CloudProjects { .. }
                | Self::ProjectList { .. }
                | Self::ProjectRead { .. }
                | Self::ProjectOriginal { .. }
                | Self::CloudCatalog { .. }
                | Self::CloudList { .. }
                | Self::CloudRead { .. }
                | Self::CloudOriginal { .. }
                | Self::CloudSubmit { .. }
                | Self::CloudContinue { .. }
                | Self::CloudCancel { .. }
                | Self::CloudFollow { .. }
                | Self::EnvironmentRead { .. }
                | Self::EnvironmentEvidence { .. }
                | Self::EnvironmentPromote { .. }
                | Self::EnvironmentSelect { .. }
                | Self::EnvironmentSteer { .. }
                | Self::ListTasks { .. }
                | Self::ReadTask { .. }
                | Self::ReadTaskOriginal { .. }
                | Self::RequestOperation { .. }
                | Self::SteerTask { .. }
                | Self::CancelTask { .. }
                | Self::ArchiveTask { .. }
                | Self::CommandTask { .. }
                | Self::CommandTaskAtRevision { .. }
                | Self::QueueTaskAtRevision { .. }
                | Self::QueueTask { .. }
                | Self::ListWorkspaces {}
                | Self::StudioSnapshot {}
                | Self::StudioUpdate { .. }
                | Self::OpenReview { .. }
                | Self::DecideMerge { .. }
        ) || self.studio_intent()
            || self.agent()
    }

    /// Whether the host retains this operation's reply for an exact retry.
    /// A read is not retained, and neither is an image chunk: a chunk is
    /// idempotent where the host keeps it, and an image's many chunks would
    /// otherwise fill the reply store.
    #[must_use]
    pub fn retains_reply(&self) -> bool {
        !self.reads_only()
            && !matches!(
                self,
                Self::PutArtifact { .. } | Self::CloudRelease { .. } | Self::Computer { .. }
            )
            && !self.background()
            && (!self.agent() || self.agent_effect())
    }

    /// A `studio.agent.*` request from a device that can start or end work:
    /// ask, answer a proposal, or stop. Its signed reply is retained, so a
    /// lost reply is recovered exactly and a reused request identity with
    /// other bytes conflicts (#10955).
    #[must_use]
    pub fn agent_effect(&self) -> bool {
        matches!(
            self,
            Self::AskAgent { .. } | Self::AnswerAgent { .. } | Self::StopAgent { .. }
        )
    }

    /// A `background.*` operation. None of their replies is retained: the
    /// reads change nothing, and a run or pause repeated is harmless.
    #[must_use]
    pub fn background(&self) -> bool {
        matches!(
            self,
            Self::ListBackground {}
                | Self::ShowBackground { .. }
                | Self::LogBackground { .. }
                | Self::RunBackground { .. }
                | Self::PauseBackground { .. }
        )
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::CloudProjects { .. } => "cloud.projects",
            Self::ProjectList { .. } => "project.list",
            Self::ProjectRead { .. } => "project.read",
            Self::ProjectOriginal { .. } => "project.original",
            Self::CloudCatalog { .. } => "cloud.catalog",
            Self::CloudList { .. } => "cloud.list",
            Self::CloudRead { .. } => "cloud.read",
            Self::CloudOriginal { .. } => "cloud.original",
            Self::CloudSubmit { .. } => "cloud.submit",
            Self::CloudContinue { .. } => "cloud.continue",
            Self::CloudCancel { .. } => "cloud.cancel",
            Self::CloudFollow { .. } => "cloud.follow",
            Self::CloudRelease { .. } => "cloud.release",
            Self::EnvironmentRead { .. } => "environment.read",
            Self::EnvironmentEvidence { .. } => "environment.evidence",
            Self::EnvironmentPromote { .. } => "environment.promote",
            Self::EnvironmentSelect { .. } => "environment.select",
            Self::EnvironmentSteer { .. } => "environment.steer",
            Self::Redeem { .. } => "enroll.redeem",
            Self::Approve { .. } => "enroll.approve",
            Self::Deny { .. } => "enroll.deny",
            Self::Invite { .. } => "invite.create",
            Self::CancelInvite { .. } => "invite.cancel",
            Self::ListDevices {} => "device.list",
            Self::Revoke { .. } => "device.revoke",
            Self::CreateTask { .. } => "task.create",
            Self::RequestOperation { .. } => "request.operation",
            Self::ListTasks { .. } => "task.list",
            Self::ReadTask { .. } => "task.read",
            Self::ReadTaskOriginal { .. } => "task.original",
            Self::OpenTerminal { .. } => "terminal.open",
            Self::OpenTaskTerminal { .. } => "task.terminal.open",
            Self::SteerTask { .. } => "task.steer",
            Self::CancelTask { .. } => "task.cancel",
            Self::ArchiveTask { .. } => "task.archive",
            Self::ListWorkspaces {} => "workspace.list",
            Self::CommandTask { .. } => "task.command",
            Self::CommandTaskAtRevision { .. } => "task.command.at_revision",
            Self::QueueTaskAtRevision { .. } => "task.queue.at_revision",
            Self::QueueTask { .. } => "task.queue",
            Self::ListSpends { .. } => "spend.list",
            Self::SettleSpend { .. } => "spend.settle",
            Self::ListWalletLinks {} => "wallet.link.list",
            Self::AnswerWalletLink { .. } => "wallet.link.answer",
            Self::InviteChats {} => "chats.invite",
            Self::VersePrivate { .. } => "verse.private",
            Self::ListThreads {} => "thread.list",
            Self::ReadThread { .. } => "thread.read",
            Self::SendThread { .. } => "thread.send",
            Self::StopThread { .. } => "thread.stop",
            Self::RunThread { .. } => "thread.run",
            Self::ReviewTask { .. } => "task.review",
            Self::PublishTask { .. } => "task.publish",
            Self::PutArtifact { .. } => "artifact.put",
            Self::Computer { .. } => "computer",
            Self::ListBackground {} => "background.list",
            Self::ShowBackground { .. } => "background.show",
            Self::LogBackground { .. } => "background.log",
            Self::RunBackground { .. } => "background.run",
            Self::PauseBackground { .. } => "background.pause",
            Self::StudioSnapshot {} => "studio.snapshot",
            Self::StudioUpdate { .. } => "studio.update",
            Self::SubmitGoal { .. } => "studio.goal.submit",
            Self::MessageSeat { .. } => "studio.seat.message",
            Self::PauseSeat { .. } => "studio.seat.pause",
            Self::ResumeSeat { .. } => "studio.seat.resume",
            Self::StopSeat { .. } => "studio.seat.stop",
            Self::ReassignTask { .. } => "studio.task.reassign",
            Self::CancelStudioTask { .. } => "studio.task.cancel",
            Self::RetryTask { .. } => "studio.task.retry",
            Self::PrioritizeTask { .. } => "studio.task.prioritize",
            Self::AnswerDecision { .. } => "studio.decision.answer",
            Self::AllowAlways { .. } => "studio.decision.always",
            Self::OpenReview { .. } => "studio.review.open",
            Self::DecideMerge { .. } => "studio.merge.decide",
            Self::ListAgents {} => "studio.agent.list",
            Self::AskAgent { .. } => "studio.agent.ask",
            Self::AnswerAgent { .. } => "studio.agent.answer",
            Self::AgentRan { .. } => "studio.agent.ran",
            Self::StopAgent { .. } => "studio.agent.stop",
            Self::ListAgentMemory { .. } => "studio.agent.memory.list",
            Self::EditAgentMemory { .. } => "studio.agent.memory.edit",
            Self::ListAgentJobs { .. } => "studio.agent.jobs.list",
            Self::EditAgentJobs { .. } => "studio.agent.jobs.edit",
            Self::AgentLog { .. } => "studio.agent.log",
            Self::ListAgentWorkspaces {} => "studio.agent.workspaces",
            Self::NewCrewAgent { .. } => "studio.agent.crew.new",
            Self::SetAgentCharter { .. } => "studio.agent.charter.set",
            Self::CrewStatus {} => "studio.agent.crew.status",
            Self::ControlCrew { .. } => "studio.agent.crew.control",
            Self::ProposeHire { .. } => "studio.agent.crew.hire.propose",
            Self::DecideHire { .. } => "studio.agent.crew.hire.decide",
            Self::ListHires {} => "studio.agent.crew.hire.list",
            Self::RecordAgentVerdict { .. } => "studio.agent.verdict.record",
            Self::ListAgentVerdicts { .. } => "studio.agent.verdict.list",
            Self::NewAgent { .. } => "studio.agent.new",
            Self::RetireAgent { .. } => "studio.agent.retire",
            Self::RotateAgent { .. } => "studio.agent.rotate",
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
            | Self::CloudProjects { .. }
            | Self::ProjectList { .. }
            | Self::ProjectRead { .. }
            | Self::ProjectOriginal { .. }
            | Self::CloudCatalog { .. }
            | Self::CloudList { .. }
            | Self::CloudRead { .. }
            | Self::CloudOriginal { .. }
            | Self::EnvironmentRead { .. }
            | Self::EnvironmentEvidence { .. }
            | Self::RequestOperation { .. }
            | Self::ListTasks { .. }
            | Self::ReadTask { .. }
            | Self::ReadTaskOriginal { .. }
            | Self::VersePrivate { .. }
            | Self::ListThreads {}
            | Self::ReadThread { .. }
            | Self::ReviewTask { .. }
            | Self::ListBackground {}
            | Self::ShowBackground { .. }
            | Self::LogBackground { .. }
            | Self::StudioSnapshot {}
            | Self::StudioUpdate { .. }
            | Self::OpenReview { .. }
            | Self::ListAgents {}
            | Self::ListAgentMemory { .. }
            | Self::ListAgentJobs { .. }
            | Self::ListAgentVerdicts { .. }
            | Self::CrewStatus {}
            | Self::ListHires {}
            | Self::AgentLog { .. }
            | Self::ListAgentWorkspaces {} => Some(Right::Observe),
            Self::CreateTask { .. }
            | Self::CloudSubmit { .. }
            | Self::CloudContinue { .. }
            | Self::CloudCancel { .. }
            | Self::CloudFollow { .. }
            | Self::CloudRelease { .. }
            | Self::EnvironmentPromote { .. }
            | Self::EnvironmentSelect { .. }
            | Self::EnvironmentSteer { .. }
            | Self::SteerTask { .. }
            | Self::CancelTask { .. }
            | Self::ArchiveTask { .. }
            | Self::ListWorkspaces {}
            | Self::CommandTask { .. }
            | Self::CommandTaskAtRevision { .. }
            | Self::QueueTaskAtRevision { .. }
            | Self::QueueTask { .. }
            | Self::ListSpends { .. }
            | Self::SettleSpend { .. }
            | Self::ListWalletLinks {}
            | Self::AnswerWalletLink { .. }
            | Self::SendThread { .. }
            | Self::StopThread { .. }
            | Self::RunThread { .. }
            | Self::PublishTask { .. }
            | Self::PutArtifact { .. }
            | Self::RunBackground { .. }
            | Self::PauseBackground { .. }
            | Self::SubmitGoal { .. }
            | Self::MessageSeat { .. }
            | Self::PauseSeat { .. }
            | Self::ResumeSeat { .. }
            | Self::StopSeat { .. }
            | Self::ReassignTask { .. }
            | Self::CancelStudioTask { .. }
            | Self::RetryTask { .. }
            | Self::PrioritizeTask { .. }
            | Self::AnswerDecision { .. }
            | Self::AllowAlways { .. }
            | Self::AskAgent { .. }
            | Self::AnswerAgent { .. }
            | Self::AgentRan { .. }
            | Self::StopAgent { .. }
            | Self::EditAgentMemory { .. }
            | Self::EditAgentJobs { .. }
            | Self::NewCrewAgent { .. }
            | Self::SetAgentCharter { .. }
            | Self::ControlCrew { .. }
            | Self::ProposeHire { .. }
            | Self::DecideHire { .. }
            | Self::RecordAgentVerdict { .. }
            | Self::NewAgent { .. }
            | Self::RetireAgent { .. }
            | Self::RotateAgent { .. } => Some(Right::Operate),
            Self::OpenTerminal { .. } | Self::OpenTaskTerminal { .. } | Self::Computer { .. } => {
                Some(Right::Terminal)
            }
            Self::DecideMerge { .. } => Some(Right::Review),
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::CloudProjects { workspace } => crate::cloud::alias(workspace)?,
            Self::ProjectList { workspace } => crate::cloud::alias(workspace)?,
            Self::ProjectRead { query } => query.validate()?,
            Self::ProjectOriginal { query } => query.validate()?,
            Self::CloudCatalog { query } => query.validate()?,
            Self::CloudList { query } => query.validate()?,
            Self::CloudRead { query } => query.validate()?,
            Self::CloudOriginal { query } => query.validate()?,
            Self::CloudSubmit { intent } => intent.validate()?,
            Self::CloudContinue { intent } => intent.validate()?,
            Self::CloudCancel { intent } => intent.validate()?,
            Self::CloudFollow { intent } => intent.validate()?,
            Self::CloudRelease { intent } => intent.validate()?,
            Self::EnvironmentRead { query } => query.validate()?,
            Self::EnvironmentEvidence { query } => query.validate()?,
            Self::EnvironmentPromote { intent } => intent.validate()?,
            Self::EnvironmentSelect { intent } => intent.validate()?,
            Self::EnvironmentSteer { intent } => intent.validate()?,
            Self::RequestOperation {
                request,
                request_event,
            } => {
                identity(request).map_err(Error::from)?;
                identity(request_event).map_err(Error::from)?;
            }
            Self::ListTasks { query } => query.validate()?,
            Self::ReadTask { query } => query.validate()?,
            Self::ReadTaskOriginal { query } => query.validate()?,
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
            Self::VersePrivate { world_key } => public(world_key)?,
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
                crate::media::validate_all(&task.images)?;
            }
            Self::PutArtifact { artifact } => artifact.validate()?,
            Self::Computer { computer } => computer.validate()?,
            Self::ListBackground {} => {}
            Self::ShowBackground { rule }
            | Self::RunBackground { rule }
            | Self::PauseBackground { rule, .. } => background_rule(rule)?,
            Self::LogBackground { rule, since } => {
                if let Some(rule) = rule {
                    background_rule(rule)?;
                }
                if let Some(since) = since {
                    safe(*since)?;
                }
            }
            Self::OpenTaskTerminal { task, cols, rows } => {
                crate::studio::id(task)?;
                if !(1..=1000).contains(cols) || !(1..=1000).contains(rows) {
                    return fail(Code::Bounds, "terminal size exceeds its bound");
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
            Self::CommandTaskAtRevision { command, revision } => {
                command.validate()?;
                safe(*revision)?;
                if command.based_on != *revision {
                    return fail(Code::Malformed, "command and exact revision differ");
                }
            }
            Self::QueueTaskAtRevision {
                task,
                revision,
                edit,
                queue_digest,
            } => {
                identity(task).map_err(Error::from)?;
                safe(*revision)?;
                edit.validate()?;
                match (edit, queue_digest) {
                    (QueueEdit::List {}, None) => {}
                    (_, Some(value)) => digest(value)?,
                    _ => return fail(Code::Malformed, "queue edits require an exact digest"),
                }
            }
            Self::QueueTask { task, edit } => {
                identity(task).map_err(Error::from)?;
                edit.validate()?;
            }
            Self::ListSpends { grant } => grant.validate()?,
            Self::SettleSpend { receipt } => receipt.validate()?,
            Self::ListWalletLinks {} => {}
            Self::AnswerWalletLink { id, sealed } => {
                crate::wallet_link::id(id)?;
                if let Some(sealed) = sealed {
                    sealed.validate()?;
                }
            }
            Self::StudioSnapshot {} => {}
            Self::StudioUpdate { stream, since } => {
                crate::studio::stream_id(stream)?;
                safe(*since)?;
            }
            Self::SubmitGoal {
                text: goal,
                workspace,
                lead,
            } => {
                crate::studio::text(goal, crate::studio::MAX_SUBMIT)?;
                text(workspace, 128)?;
                if let Some(lead) = lead {
                    crate::studio::seat_name(lead)?;
                }
            }
            Self::MessageSeat { seat, text } => {
                if let Some(seat) = seat {
                    crate::studio::seat_name(seat)?;
                }
                crate::studio::text(text, crate::studio::MAX_MESSAGE)?;
            }
            Self::PauseSeat { seat } | Self::ResumeSeat { seat } | Self::StopSeat { seat } => {
                crate::studio::seat_name(seat)?;
            }
            Self::ReassignTask { task, seat } => {
                crate::studio::id(task)?;
                crate::studio::seat_name(seat)?;
            }
            Self::CancelStudioTask { task }
            | Self::RetryTask { task }
            | Self::PrioritizeTask { task }
            | Self::OpenReview { task } => crate::studio::id(task)?,
            Self::AnswerDecision {
                decision,
                based_on,
                text,
                command,
                issued_at,
            } => {
                crate::studio::id(decision)?;
                safe(*based_on)?;
                crate::studio::text(text, crate::studio::MAX_ANSWER)?;
                identity(command).map_err(Error::from)?;
                safe(*issued_at)?;
            }
            Self::AllowAlways {
                decision,
                based_on,
                rule,
                command,
                issued_at,
            } => {
                crate::studio::id(decision)?;
                safe(*based_on)?;
                crate::studio::text(rule, crate::studio::MAX_RULE_TEXT)?;
                identity(command).map_err(Error::from)?;
                safe(*issued_at)?;
            }
            Self::DecideMerge { decision } => decision.validate()?,
            Self::ListAgents {} | Self::ListAgentWorkspaces {} => {}
            Self::NewCrewAgent {
                agent, workspace, ..
            }
            | Self::NewAgent { agent, workspace } => {
                crate::agent::name(agent)?;
                crate::agent::workspace_path(workspace)?;
            }
            Self::SetAgentCharter {
                agent,
                expected,
                purpose,
                ..
            } => {
                crate::agent::name(agent)?;
                safe(*expected)?;
                crate::crew::Charter {
                    schema: crate::crew::CHARTER_SCHEMA.into(),
                    revision: 1,
                    drafting: false,
                    purpose: purpose.clone(),
                }
                .validate()?;
            }
            Self::ControlCrew { control } => control.validate()?,
            Self::ProposeHire { proposal } => proposal.validate()?,
            Self::DecideHire {
                decision,
                workspace,
            } => {
                decision.validate()?;
                if let Some(workspace) = workspace {
                    crate::agent::workspace_path(workspace)?;
                }
            }
            Self::ListHires {} => {}
            Self::CrewStatus {} => {}
            Self::RecordAgentVerdict { agent, verdict } => {
                crate::agent::name(agent)?;
                verdict.validate()?;
            }
            Self::ListAgentVerdicts { agent } | Self::RetireAgent { agent } => {
                crate::agent::name(agent)?
            }
            Self::RotateAgent { agent, reason } => {
                crate::agent::name(agent)?;
                text(reason, 256)?;
            }
            Self::AskAgent {
                agent,
                text: request,
                workspace,
                context,
                computer,
                ..
            } => {
                crate::agent::name(agent)?;
                crate::agent::request_text(request)?;
                if let Some(workspace) = workspace {
                    text(workspace, 128)?;
                }
                if let Some(computer) = computer {
                    text(computer, 128)?;
                }
                crate::agent::context(context)?;
            }
            Self::AnswerAgent { agent, step, .. } => {
                crate::agent::name(agent)?;
                safe(*step)?;
            }
            Self::AgentRan { agent, step, ran } => {
                crate::agent::name(agent)?;
                safe(*step)?;
                ran.validate()?;
            }
            Self::StopAgent { agent, reason } => {
                crate::agent::name(agent)?;
                text(reason, 512)?;
            }
            Self::ListAgentMemory { agent, after } | Self::AgentLog { agent, after } => {
                crate::agent::name(agent)?;
                if let Some(after) = after {
                    safe(*after)?;
                }
            }
            Self::EditAgentMemory { agent, edit } => {
                crate::agent::name(agent)?;
                edit.validate()?;
            }
            Self::ListAgentJobs { agent } => crate::agent::name(agent)?,
            Self::EditAgentJobs { agent, edit } => {
                crate::agent::name(agent)?;
                edit.validate()?;
            }
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
    CloudProjects {
        projects: crate::cloud::Projects,
    },
    ProjectList {
        projects: crate::project::List,
    },
    ProjectRead {
        project: Box<crate::project::Page>,
    },
    ProjectOriginal {
        chunk: crate::project::Chunk,
    },
    CloudCatalog {
        catalog: crate::cloud::Catalog,
    },
    CloudList {
        jobs: crate::cloud::List,
    },
    CloudRead {
        job: Box<crate::cloud::Job>,
    },
    CloudOriginal {
        chunk: crate::cloud::OriginalChunk,
    },
    CloudAccepted {
        accepted: crate::cloud::Accepted,
    },
    CloudReleased {
        released: crate::cloud::Released,
    },
    EnvironmentRead {
        view: Box<crate::environment::View>,
    },
    EnvironmentEvidence {
        page: Box<crate::environment::EvidencePage>,
    },
    EnvironmentAccepted {
        accepted: crate::environment::Accepted,
    },
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
    /// A known original result, or an explicit unknown without redispatch.
    RequestOperation {
        request: String,
        request_event: String,
        result: Option<Box<ReplyResult>>,
    },
    /// The workspace labels a device may name in `task.create`, sorted and
    /// distinct. A label names a host-side root; it is never a path.
    Workspaces {
        workspaces: Vec<String>,
    },
    Tasks {
        tasks: Box<crate::task_read::List>,
    },
    Task {
        task: Box<crate::task_read::Page>,
    },
    TaskOriginal {
        original: Box<crate::task_read::OriginalChunk>,
    },
    /// A task's held messages after a `task.queue` operation.
    Queue {
        queue: TaskQueue,
    },
    /// The exact queue snapshot after a request-keyed edit or current read.
    QueueAtRevision {
        queue: TaskQueue,
        revision: u64,
        queue_digest: String,
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
    /// The open asks for the owner's wallet (`wallet.link.list`), oldest
    /// first, at most [`crate::wallet_link::MAX_LISTED`].
    WalletLinks {
        links: Vec<crate::wallet_link::Ask>,
    },
    /// The host recorded the answer to this ask (`wallet.link.answer`).
    WalletLinkAnswered {
        id: String,
    },
    /// The owner's private Verse placements (`verse.private`): the
    /// `openagents.verse.private-placements.v1` file as the host holds it,
    /// at most [`MAX_VERSE_PLACEMENTS`] bytes, or `None` when it holds none.
    VersePrivate {
        placements: Option<String>,
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
    /// What the host holds of an image after `artifact.put`.
    Artifact {
        artifact: crate::media::ArtifactState,
    },
    /// The answer to a `computer` request.
    Computer {
        computer: crate::computer::Answer,
    },
    /// A `background.*` answer: the `background` crate's JSON (rules and
    /// their state, one rule, log records, or an acknowledgement), at most
    /// [`MAX_BACKGROUND_BYTES`]. It is carried as JSON so this crate does
    /// not depend on the host's rule types.
    Background {
        background: Box<serde_json::Value>,
    },
    /// The Agent Studio in full (`studio.snapshot`).
    Studio {
        snapshot: Box<crate::studio::Snapshot>,
    },
    /// What changed in the studio (`studio.update`).
    StudioUpdate {
        update: Box<crate::studio::Update>,
    },
    /// What the host did with a merge decision (`studio.merge.decide`).
    Merged {
        merged: Box<crate::studio::Merged>,
    },
    /// A `studio.agent.*` answer: one of [`crate::agent`]'s answers as
    /// JSON, at most [`crate::agent::MAX_AGENT_BYTES`].
    Agent {
        agent: Box<serde_json::Value>,
    },
}

/// The largest `background` outcome.
pub const MAX_BACKGROUND_BYTES: usize = 256 * 1024;

/// A background rule ID: 1 to 64 lowercase letters, digits, and dashes.
fn background_rule(rule: &str) -> Result<()> {
    if rule.is_empty()
        || rule.len() > 64
        || !rule
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return fail(Code::Malformed, "not a background rule ID");
    }
    Ok(())
}

/// The longest `coder-pair:` invitation a `chats` outcome carries.
pub const MAX_CHAT_INVITATION: usize = 4096;
/// The longest private Verse placements file a host answers with.
pub const MAX_VERSE_PLACEMENTS: usize = 64 * 1024;

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
        if matches!(
            self,
            Self::ProjectList { .. }
                | Self::ProjectRead { .. }
                | Self::ProjectOriginal { .. }
                | Self::CloudProjects { .. }
                | Self::CloudCatalog { .. }
                | Self::CloudList { .. }
                | Self::CloudRead { .. }
                | Self::CloudOriginal { .. }
                | Self::CloudAccepted { .. }
                | Self::EnvironmentRead { .. }
                | Self::EnvironmentEvidence { .. }
                | Self::EnvironmentAccepted { .. }
        ) {
            crate::task_read::bounded(self, crate::cloud::MAX_REPLY_BYTES)?;
        }
        match self {
            Self::CloudProjects { projects } => projects.validate()?,
            Self::ProjectList { projects } => projects.validate()?,
            Self::ProjectRead { project } => project.validate()?,
            Self::ProjectOriginal { chunk } => chunk.validate()?,
            Self::CloudCatalog { catalog } => catalog.validate()?,
            Self::CloudList { jobs } => jobs.validate()?,
            Self::CloudRead { job } => job.validate()?,
            Self::CloudOriginal { chunk } => chunk.validate()?,
            Self::CloudAccepted { accepted } => accepted.validate()?,
            Self::CloudReleased { released } => released.validate()?,
            Self::EnvironmentRead { view } => view.validate()?,
            Self::EnvironmentEvidence { page } => page.validate()?,
            Self::EnvironmentAccepted { accepted } => accepted.validate()?,
            Self::Tasks { tasks } => tasks.validate()?,
            Self::Task { task } => task.validate()?,
            Self::TaskOriginal { original } => original.validate()?,
            Self::Computer { computer } => computer.validate()?,
            _ => {}
        }
        if matches!(
            self,
            Self::Tasks { .. } | Self::Task { .. } | Self::TaskOriginal { .. }
        ) {
            crate::task_read::bounded(self, crate::task_read::MAX_REPLY_BYTES)?;
        }
        if let Self::RequestOperation {
            request,
            request_event,
            result,
        } = self
        {
            identity(request).map_err(Error::from)?;
            identity(request_event).map_err(Error::from)?;
            if let Some(result) = result {
                match result.as_ref() {
                    ReplyResult::Ok { outcome }
                        if matches!(
                            outcome,
                            Self::Dispatched { .. }
                                | Self::CloudAccepted { .. }
                                | Self::EnvironmentAccepted { .. }
                                | Self::QueueAtRevision { .. }
                                | Self::Published { .. }
                                | Self::Merged { .. }
                                | Self::Agent { .. }
                        ) =>
                    {
                        outcome.validate()?
                    }
                    ReplyResult::Refused { code, missing } => {
                        if (*code == Code::MissingRight) != missing.is_some() {
                            return fail(
                                Code::Malformed,
                                "recovery refusal right differs from its code",
                            );
                        }
                    }
                    _ => {
                        return fail(
                            Code::Malformed,
                            "recovery result is not a supported task effect",
                        );
                    }
                }
            }
            crate::task_read::bounded(self, crate::task_read::MAX_REPLY_BYTES + 2048)?;
        }
        if let Self::QueueAtRevision {
            queue,
            revision,
            queue_digest,
        } = self
        {
            safe(*revision)?;
            if queue.revision != *revision {
                return fail(Code::Malformed, "queue and exact revision differ");
            }
            digest(queue_digest)?;
            crate::task_read::bounded(self, crate::task_read::MAX_REPLY_BYTES)?;
        }
        if let Self::Queue { queue } | Self::QueueAtRevision { queue, .. } = self {
            identity(&queue.task).map_err(Error::from)?;
            safe(queue.revision)?;
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
                safe(lease.expires_at)?;
            }
        }
        if let Self::WalletLinks { links } = self {
            if links.len() > crate::wallet_link::MAX_LISTED {
                return fail(Code::Bounds, "too many wallet link asks");
            }
            for ask in links {
                ask.validate()?;
            }
        }
        if let Self::WalletLinkAnswered { id } = self {
            crate::wallet_link::id(id)?;
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
        if let Self::VersePrivate {
            placements: Some(placements),
        } = self
            && placements.len() > MAX_VERSE_PLACEMENTS
        {
            return fail(Code::Bounds, "private placements exceed their bound");
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
        if let Self::Studio { snapshot } = self {
            snapshot.validate()?;
        }
        if let Self::StudioUpdate { update } = self {
            update.validate()?;
        }
        if let Self::Merged { merged } = self {
            merged.validate()?;
        }
        if let Self::Background { .. } = self
            && serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > MAX_BACKGROUND_BYTES)
        {
            return fail(Code::Bounds, "background answer exceeds its bound");
        }
        if let Self::Agent { .. } = self
            && serde_json::to_vec(self)
                .map_or(true, |bytes| bytes.len() > crate::agent::MAX_AGENT_BYTES)
        {
            return fail(Code::Bounds, "agent answer exceeds its bound");
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
            (Operation::CloudProjects { workspace }, Self::CloudProjects { projects }) => {
                *workspace == projects.workspace
            }
            (Operation::ProjectList { workspace }, Self::ProjectList { projects }) => {
                projects.workspace == *workspace
            }
            (Operation::ProjectRead { query }, Self::ProjectRead { project }) => {
                project.answers(query)
            }
            (Operation::ProjectOriginal { query }, Self::ProjectOriginal { chunk }) => {
                chunk.answers(query)
            }
            (Operation::CloudCatalog { query }, Self::CloudCatalog { catalog }) => {
                catalog.answers(query)
            }
            (Operation::CloudList { query }, Self::CloudList { jobs }) => jobs.answers(query),
            (Operation::CloudRead { query }, Self::CloudRead { job }) => job.answers(query),
            (Operation::CloudOriginal { query }, Self::CloudOriginal { chunk }) => {
                chunk.answers(query)
            }
            (Operation::CloudRelease { intent }, Self::CloudReleased { released }) => {
                released.answers(intent)
            }
            (Operation::CloudSubmit { intent }, Self::CloudAccepted { accepted }) => {
                accepted.action == "submit"
                    && accepted.scope.workspace == intent.workspace
                    && accepted.scope.project == intent.project
                    && accepted.scope.profile == intent.profile
                    && accepted.scope.profile_revision == intent.profile_revision
                    && accepted.scope.source_digest == intent.source_digest
            }
            (Operation::CloudContinue { intent }, Self::CloudAccepted { accepted }) => {
                accepted.action == "continue"
                    && accepted.scope.workspace == intent.scope.workspace
                    && accepted.scope.project == intent.scope.project
                    && accepted.scope.job == intent.scope.job
                    && accepted.scope.profile == intent.scope.profile
                    && accepted.scope.profile_revision == intent.scope.profile_revision
                    && accepted.scope.source_digest == intent.scope.source_digest
                    && accepted.scope.attempt == intent.scope.attempt.saturating_add(1)
            }
            (Operation::CloudCancel { intent }, Self::CloudAccepted { accepted }) => {
                accepted.action == "cancel"
                    && accepted.scope.workspace == intent.scope.workspace
                    && accepted.scope.project == intent.scope.project
                    && accepted.scope.job == intent.scope.job
                    && accepted.scope.profile == intent.scope.profile
                    && accepted.scope.profile_revision == intent.scope.profile_revision
                    && accepted.scope.source_digest == intent.scope.source_digest
                    && accepted.scope.attempt == intent.scope.attempt
            }
            (Operation::EnvironmentRead { query }, Self::EnvironmentRead { view }) => {
                view.answers(query)
            }
            (Operation::EnvironmentEvidence { query }, Self::EnvironmentEvidence { page }) => {
                page.answers(query)
            }
            (
                op @ (Operation::EnvironmentPromote { .. }
                | Operation::EnvironmentSelect { .. }
                | Operation::EnvironmentSteer { .. }),
                Self::EnvironmentAccepted { accepted },
            ) => accepted.answers(op),
            (Operation::CloudFollow { intent }, Self::CloudAccepted { accepted }) => {
                accepted.action == "follow"
                    && accepted.scope.workspace == intent.scope.workspace
                    && accepted.scope.project == intent.scope.project
                    && accepted.scope.job == intent.scope.job
                    && accepted.scope.profile == intent.scope.profile
                    && accepted.scope.profile_revision == intent.scope.profile_revision
                    && accepted.scope.source_digest == intent.scope.source_digest
                    && accepted.scope.attempt == intent.scope.attempt
            }
            (
                Operation::RequestOperation {
                    request,
                    request_event,
                },
                Self::RequestOperation {
                    request: answered,
                    request_event: event,
                    ..
                },
            ) => request == answered && request_event == event,
            (Operation::ListTasks { query }, Self::Tasks { tasks }) => tasks.answers(query),
            (Operation::ReadTask { query }, Self::Task { task }) => task.answers(query),
            (Operation::ReadTaskOriginal { query }, Self::TaskOriginal { original }) => {
                original.answers(query)
            }
            (Operation::Redeem { .. } | Operation::Approve { .. }, Self::Granted { .. })
            | (Operation::Deny { .. }, Self::Denied {})
            | (Operation::Invite { .. }, Self::Invitation { .. })
            | (Operation::CancelInvite { .. }, Self::Cancelled { .. })
            | (Operation::ListDevices {}, Self::Devices { .. })
            | (Operation::Revoke { .. }, Self::Revoked { .. })
            | (Operation::ListWorkspaces {}, Self::Workspaces { .. }) => true,
            (Operation::CommandTaskAtRevision { command, .. }, Self::Dispatched { receipt }) => {
                receipt.operation == op.name() && receipt.reference == command.task
            }
            (Operation::QueueTask { task, .. }, Self::Queue { queue }) => queue.task == *task,
            (
                Operation::QueueTaskAtRevision {
                    task,
                    revision: expected,
                    edit,
                    ..
                },
                Self::QueueAtRevision {
                    queue, revision, ..
                },
            ) => {
                queue.task == *task
                    && queue.revision == *revision
                    && if matches!(edit, QueueEdit::List {}) {
                        *revision == *expected
                    } else {
                        *revision >= *expected
                    }
            }
            (Operation::ListWalletLinks {}, Self::WalletLinks { .. }) => true,
            (Operation::AnswerWalletLink { id, .. }, Self::WalletLinkAnswered { id: answered }) => {
                id == answered
            }
            (Operation::ListSpends { .. }, Self::Spends { .. })
            | (Operation::InviteChats {}, Self::Chats { .. })
            | (Operation::VersePrivate { .. }, Self::VersePrivate { .. })
            | (Operation::ListThreads {}, Self::Threads { .. }) => true,
            (Operation::ReadThread { thread, .. }, Self::Thread { thread: page }) => {
                page.thread == *thread
            }
            (
                Operation::ReviewTask { task } | Operation::OpenReview { task },
                Self::Review { review },
            ) => review.task == *task,
            (Operation::StudioSnapshot {}, Self::Studio { .. }) => true,
            (Operation::StudioUpdate { stream, since }, Self::StudioUpdate { update }) => {
                update.stream == *stream && update.from == *since
            }
            (Operation::DecideMerge { decision }, Self::Merged { merged }) => {
                merged.answers(decision)
            }
            (op, Self::Dispatched { receipt }) if op.studio_intent() => {
                receipt.operation == op.name()
            }
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
            (op, Self::Background { .. }) if op.background() => true,
            (op, Self::Agent { .. }) if op.agent() => true,
            (Operation::PutArtifact { artifact }, Self::Artifact { artifact: state }) => {
                state.digest == artifact.digest && state.received <= artifact.size
            }
            (Operation::Computer { computer }, Self::Computer { computer: answer }) => {
                answer.answers(computer)
            }
            (Operation::SettleSpend { receipt }, Self::Settled { receipt: recorded }) => {
                recorded.request == receipt.request && recorded.grant == receipt.grant
            }
            (
                Operation::CreateTask { .. }
                | Operation::OpenTerminal { .. }
                | Operation::OpenTaskTerminal { .. }
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
// Only the relay clients seal; a browser build has none.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
// Only the relay clients read it; a browser build has none.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
