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
}
impl Operation {
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
            Self::CreateTask { .. } | Self::SteerTask { .. } | Self::CancelTask { .. } => {
                Some(Right::Operate)
            }
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
            Self::ListDevices {} => {}
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
}
impl Outcome {
    /// Whether this outcome is the one the operation can produce.
    pub fn answers(&self, op: &Operation) -> bool {
        match (op, self) {
            (Operation::Redeem { .. } | Operation::Approve { .. }, Self::Granted { .. })
            | (Operation::Deny { .. }, Self::Denied {})
            | (Operation::Invite { .. }, Self::Invitation { .. })
            | (Operation::CancelInvite { .. }, Self::Cancelled { .. })
            | (Operation::ListDevices {}, Self::Devices { .. })
            | (Operation::Revoke { .. }, Self::Revoked { .. }) => true,
            (
                Operation::CreateTask { .. }
                | Operation::OpenTerminal { .. }
                | Operation::SteerTask { .. }
                | Operation::CancelTask { .. },
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
