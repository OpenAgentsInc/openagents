//! A bounded resident remote adapter over the private pipeline, beside its
//! canonical owner on the sales-owner host.
//!
//! A web site never opens this store. The owner provisions one binding per
//! browser actor, workspace, and membership epoch; each binding names one
//! existing sales principal credential (issued through [`Store::issue`]) and
//! may only narrow it: its effect allowlist is the most a remote caller can
//! do, and the principal's recorded role still applies. Account, host, Studio,
//! world, or billing membership grants nothing here.
//!
//! Every call reopens the store and rereads the principal credential, so
//! revocation, rotation, and record revisions are rechecked on each read and
//! effect. An effect's exact retry identity and bytes are journaled before
//! dispatch. An exact retry returns the original receipt, different bytes
//! conflict, and [`Op::Reconcile`] settles an effect whose reply was lost.
//! Refusals are fixed codes; no contact, record body, or credential enters an
//! error, a list summary, or the journal's file name.

use super::{Lead, PermissionState, Receipt, Result, Role, Stage, Store, digest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const CONFIG_SCHEMA: &str = "openagents.sales.remote-bindings.v1";
pub const REQUEST_SCHEMA: &str = "openagents.sales.remote-request.v1";
pub const RESPONSE_SCHEMA: &str = "openagents.sales.remote-response.v1";
const JOURNAL_SCHEMA: &str = "openagents.sales.remote-journal.v1";
/// One command plus its envelope.
pub const BODY_MAX: usize = super::MAX_COMMAND + 8 * 1024;
const CONFIG_MAX: u64 = 64 * 1024;
const JOURNAL_MAX: usize = 256;
const JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const BINDINGS_MAX: usize = 64;

/// The effects a binding may admit. Operations outside this set (service
/// sales, acquisition, partners, funnel journeys) need their own reviewed
/// adapter and are refused here.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Create,
    Update,
    ProposeHandoff,
    AcceptHandoff,
    RejectHandoff,
    Suppress,
    Delete,
}

impl Effect {
    fn of(operation: &super::Operation) -> Option<Self> {
        use super::Operation as O;
        Some(match operation {
            O::Create { .. } => Self::Create,
            O::Update { .. } => Self::Update,
            O::ProposeHandoff { .. } => Self::ProposeHandoff,
            O::AcceptHandoff { .. } => Self::AcceptHandoff,
            O::RejectHandoff { .. } => Self::RejectHandoff,
            O::Suppress { .. } => Self::Suppress,
            O::Delete { .. } => Self::Delete,
            _ => return None,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    /// The existing host task root holding the private pipeline.
    root: PathBuf,
    /// A private directory for retry journals, outside the pipeline directory.
    journal: PathBuf,
    bindings: Vec<Binding>,
}

/// One owner-provisioned browser actor/workspace binding.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    /// The sales principal this binding acts as; its credential must match.
    principal: String,
    credential: PathBuf,
    /// Lowercase hex SHA-256 of the site's bearer for this binding only.
    client_digest: String,
    /// The narrowing allowlist. Empty means observation only.
    #[serde(default)]
    effects: Vec<Effect>,
}

impl Binding {
    fn journal_name(&self) -> String {
        format!(
            "{}.json",
            digest(
                json!({"schema":JOURNAL_SCHEMA,"binding":self.id,"account":self.account,
                "workspace":self.workspace,"members_epoch":self.members_epoch,
                "principal":self.principal})
                .to_string()
                .as_bytes()
            )
        )
    }
}

/// The browser actor the site attests; it must equal the binding exactly.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub actor: Actor,
    pub op: Op,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// The bound principal, its current role, and the admitted effects.
    Standing,
    /// Summaries only: no contact, source, or record body.
    List { after: Option<String>, limit: usize },
    /// One record visible to the bound principal.
    Show { lead: String },
    /// One exact command; its `id` must equal `request`.
    Apply { request: String, command: String },
    /// Settle a journaled request whose reply was lost.
    Reconcile { request: String, digest: String },
}

/// A record's pipeline position without contact or private text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub id: String,
    pub revision: u64,
    pub stage: Stage,
    pub responsible_human: String,
    pub permission: PermissionState,
    pub next_due_at: Option<u64>,
    pub updated_at: u64,
    pub handoff_pending: bool,
}

impl Summary {
    fn of(lead: &Lead) -> Self {
        Self {
            id: lead.id.clone(),
            revision: lead.revision,
            stage: lead.details.stage,
            responsible_human: lead.responsible_human.clone(),
            permission: lead.details.permission.state,
            next_due_at: lead.details.next.as_ref().map(|n| n.due_at),
            updated_at: lead.updated_at,
            handoff_pending: lead.proposed_handoff.is_some(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standing {
    pub binding: String,
    pub principal: String,
    pub role: Role,
    pub effects: Vec<Effect>,
}

/// The answer to a reconcile request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Settled {
    /// The effect is recorded with this receipt.
    Recorded { receipt: Receipt },
    /// This adapter never journaled the request, so it never dispatched it.
    Absent,
}

/// A fixed refusal code. None carries record content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    AccessDenied,
    InvalidRequest,
    /// The request identity was used with different bytes.
    Conflict,
    /// The record revision moved; review the current record.
    Stale,
    /// The owner refused the effect; nothing was applied.
    Refused,
    /// Too many unsettled requests; reconcile them first.
    Busy,
    /// The outcome may be unknown; retry the same request.
    Unavailable,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AccessDenied => "access_denied",
            Self::InvalidRequest => "invalid_request",
            Self::Conflict => "conflict",
            Self::Stale => "stale",
            Self::Refused => "refused",
            Self::Busy => "busy",
            Self::Unavailable => "unavailable",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "access_denied" => Self::AccessDenied,
            "invalid_request" => Self::InvalidRequest,
            "conflict" => Self::Conflict,
            "stale" => Self::Stale,
            "refused" => Self::Refused,
            "busy" => Self::Busy,
            _ => Self::Unavailable,
        }
    }
    pub fn status(self) -> u16 {
        match self {
            Self::AccessDenied => 403,
            Self::InvalidRequest => 400,
            Self::Conflict | Self::Stale => 409,
            Self::Refused => 422,
            Self::Busy => 429,
            Self::Unavailable => 503,
        }
    }
    /// Whether the owner definitely applied nothing for this answer.
    pub fn definitive(self) -> bool {
        !matches!(self, Self::Busy | Self::Unavailable)
    }
}

/// An HTTP-shaped answer for any transport.
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    fn ok(result: Value) -> Self {
        Self {
            status: 200,
            body: json!({"schema":RESPONSE_SCHEMA,"result":result}),
        }
    }
    fn refuse(code: Code) -> Self {
        Self {
            status: code.status(),
            body: json!({"schema":RESPONSE_SCHEMA,"error":code.as_str()}),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    digest: String,
    /// Exact bytes while unsettled, held beside the owner so reconciliation
    /// never needs the site. Cleared once a receipt is recorded.
    command: String,
    at: u64,
    receipt: Option<Receipt>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: String,
    entries: BTreeMap<String, Entry>,
}

/// The resident adapter. One call runs at a time; each reopens the store.
pub struct Service {
    config: PathBuf,
    clock: fn() -> u64,
    turn: Mutex<()>,
}

impl Service {
    pub fn open(config: &Path) -> Result<Self> {
        Self::open_with_clock(config, super::unix_now)
    }

    pub fn open_with_clock(config: &Path, clock: fn() -> u64) -> Result<Self> {
        load(config)?;
        Ok(Self {
            config: config.into(),
            clock,
            turn: Mutex::new(()),
        })
    }

    /// Answer one call. `bearer` is the site's per-binding credential.
    pub fn call(&self, binding: &str, bearer: &str, body: &[u8]) -> Reply {
        let _turn = match self.turn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match self.answer(binding, bearer, body) {
            Ok(value) => Reply::ok(value),
            Err(code) => Reply::refuse(code),
        }
    }

    fn answer(&self, binding: &str, bearer: &str, body: &[u8]) -> std::result::Result<Value, Code> {
        let config = load(&self.config).map_err(|_| Code::Unavailable)?;
        let presented = digest(bearer.as_bytes());
        let binding = config
            .bindings
            .iter()
            .find(|b| b.id == binding && same(&b.client_digest, &presented))
            .ok_or(Code::AccessDenied)?;
        if body.len() > BODY_MAX {
            return Err(Code::InvalidRequest);
        }
        let request: Request = serde_json::from_slice(body).map_err(|_| Code::InvalidRequest)?;
        if request.schema != REQUEST_SCHEMA {
            return Err(Code::InvalidRequest);
        }
        if request.actor
            != (Actor {
                account: binding.account.clone(),
                workspace: binding.workspace.clone(),
                members_epoch: binding.members_epoch,
            })
        {
            return Err(Code::AccessDenied);
        }
        let mut store =
            Store::open_with_clock(&config.root, self.clock).map_err(|_| Code::Unavailable)?;
        let secret = Store::read_credential(&binding.credential).map_err(|_| Code::AccessDenied)?;
        let access = store
            .authenticate(&secret)
            .map_err(|_| Code::AccessDenied)?;
        if access.principal() != binding.principal {
            return Err(Code::AccessDenied);
        }
        let role = store.check(&access).map_err(|_| Code::AccessDenied)?;
        match request.op {
            Op::Standing => Ok(json!(Standing {
                binding: binding.id.clone(),
                principal: binding.principal.clone(),
                role,
                effects: if role == Role::Reader {
                    vec![]
                } else {
                    binding.effects.clone()
                },
            })),
            Op::List { after, limit } => {
                if after.as_deref().is_some_and(|a| !record_id(a)) {
                    return Err(Code::InvalidRequest);
                }
                let leads = store
                    .list(&access, after.as_deref(), limit)
                    .map_err(|_| Code::InvalidRequest)?;
                Ok(json!(leads.iter().map(Summary::of).collect::<Vec<_>>()))
            }
            Op::Show { lead } => {
                if !record_id(&lead) {
                    return Err(Code::InvalidRequest);
                }
                // Absent and refused records answer alike.
                let lead = store.show(&access, &lead).map_err(|_| Code::AccessDenied)?;
                serde_json::to_value(lead).map_err(|_| Code::Unavailable)
            }
            Op::Apply { request, command } => {
                let receipt =
                    self.apply(&config, binding, &mut store, &access, &request, command)?;
                Ok(json!(receipt))
            }
            Op::Reconcile { request, digest } => {
                super::id(&request).map_err(|_| Code::InvalidRequest)?;
                let mut journal = Journal::load(&config.journal, binding)?;
                let Some(entry) = journal.entries.get(&request).cloned() else {
                    return Ok(json!(Settled::Absent));
                };
                if entry.digest != digest {
                    return Err(Code::Conflict);
                }
                if let Some(receipt) = entry.receipt {
                    return Ok(json!(Settled::Recorded { receipt }));
                }
                // The binding may have narrowed since this was journaled.
                let parsed: super::Command =
                    serde_json::from_str(&entry.command).map_err(|_| Code::Unavailable)?;
                if !Effect::of(&parsed.operation).is_some_and(|e| binding.effects.contains(&e)) {
                    return Err(Code::AccessDenied);
                }
                let receipt = dispatch(
                    &config,
                    binding,
                    &mut journal,
                    &mut store,
                    &access,
                    &request,
                    &entry.command,
                )?;
                Ok(json!(Settled::Recorded { receipt }))
            }
        }
    }

    fn apply(
        &self,
        config: &Config,
        binding: &Binding,
        store: &mut Store,
        access: &super::Access,
        request: &str,
        command: String,
    ) -> std::result::Result<Receipt, Code> {
        super::id(request).map_err(|_| Code::InvalidRequest)?;
        if command.len() > super::MAX_COMMAND {
            return Err(Code::InvalidRequest);
        }
        let parsed: super::Command =
            serde_json::from_str(&command).map_err(|_| Code::InvalidRequest)?;
        if parsed.id != request {
            return Err(Code::InvalidRequest);
        }
        let effect = Effect::of(&parsed.operation).ok_or(Code::AccessDenied)?;
        if !binding.effects.contains(&effect) {
            return Err(Code::AccessDenied);
        }
        let exact = digest(command.as_bytes());
        let mut journal = Journal::load(&config.journal, binding)?;
        match journal.entries.get(request) {
            Some(entry) if entry.digest != exact => return Err(Code::Conflict),
            Some(Entry {
                receipt: Some(receipt),
                ..
            }) => {
                // Credentials were rechecked above; the original stands.
                return Ok(receipt.clone());
            }
            Some(_) => {}
            None => {
                if journal.entries.len() >= JOURNAL_MAX {
                    let oldest = journal
                        .entries
                        .iter()
                        .filter(|(_, e)| e.receipt.is_some())
                        .min_by_key(|(_, e)| e.at)
                        .map(|(k, _)| k.clone())
                        .ok_or(Code::Busy)?;
                    journal.entries.remove(&oldest);
                }
                journal.entries.insert(
                    request.into(),
                    Entry {
                        digest: exact,
                        command: command.clone(),
                        at: (self.clock)(),
                        receipt: None,
                    },
                );
                // The exact retry identity is durable before dispatch.
                journal.save(&config.journal, binding)?;
            }
        }
        dispatch(
            config,
            binding,
            &mut journal,
            store,
            access,
            request,
            &command,
        )
    }
}

/// Dispatch a journaled command once and settle its entry.
fn dispatch(
    config: &Config,
    binding: &Binding,
    journal: &mut Journal,
    store: &mut Store,
    access: &super::Access,
    request: &str,
    command: &str,
) -> std::result::Result<Receipt, Code> {
    let result = store.apply(access, command.as_bytes());
    let receipt = match result {
        Ok(receipt) => receipt,
        Err(message) => {
            // The store's own command identity decides whether anything was
            // applied; a poisoned store leaves the outcome unknown.
            if store.poisoned {
                return Err(Code::Unavailable);
            }
            let code = if message.contains("idempotency conflict") {
                Code::Conflict
            } else if message.contains("revision conflict") {
                Code::Stale
            } else {
                Code::Refused
            };
            journal.entries.remove(request);
            journal.save(&config.journal, binding)?;
            return Err(code);
        }
    };
    if let Some(entry) = journal.entries.get_mut(request) {
        entry.receipt = Some(receipt.clone());
        // Settled: the digest keeps the identity; the bytes (which may name
        // a contact later suppressed or deleted) are not retained.
        entry.command.clear();
    }
    journal.save(&config.journal, binding)?;
    Ok(receipt)
}

impl Journal {
    fn load(root: &Path, binding: &Binding) -> std::result::Result<Self, Code> {
        let path = root.join(binding.journal_name());
        if !super::super::regular_or_absent(&path).map_err(|_| Code::Unavailable)? {
            return Ok(Self {
                schema: JOURNAL_SCHEMA.into(),
                entries: BTreeMap::new(),
            });
        }
        let mut bytes = Vec::new();
        super::super::private_open(&path, false, false)
            .map_err(|_| Code::Unavailable)?
            .take(JOURNAL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Code::Unavailable)?;
        if bytes.len() as u64 > JOURNAL_BYTES {
            return Err(Code::Unavailable);
        }
        let journal: Self = serde_json::from_slice(&bytes).map_err(|_| Code::Unavailable)?;
        if journal.schema != JOURNAL_SCHEMA || journal.entries.len() > JOURNAL_MAX {
            return Err(Code::Unavailable);
        }
        Ok(journal)
    }

    fn save(&self, root: &Path, binding: &Binding) -> std::result::Result<(), Code> {
        let bytes = serde_json::to_vec(self).map_err(|_| Code::Unavailable)?;
        super::super::replace_file(root, &binding.journal_name(), &bytes)
            .map_err(|_| Code::Unavailable)
    }
}

fn load(path: &Path) -> Result<Config> {
    let mut bytes = Vec::new();
    super::super::private_open(path, false, false)
        .map_err(|_| "sales remote configuration must be a private regular file")?
        .take(CONFIG_MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "sales remote configuration is unavailable")?;
    if bytes.len() as u64 > CONFIG_MAX {
        return Err("sales remote configuration exceeds its bound".into());
    }
    let config: Config =
        serde_json::from_slice(&bytes).map_err(|_| "malformed sales remote configuration")?;
    if config.schema != CONFIG_SCHEMA || config.bindings.len() > BINDINGS_MAX {
        return Err("unsupported sales remote configuration".into());
    }
    super::super::prepare_directory(&config.journal)
        .map_err(|_| "sales remote journal must be a private directory")?;
    let journal = config
        .journal
        .canonicalize()
        .map_err(|_| "sales remote journal is unavailable")?;
    let root = config
        .root
        .canonicalize()
        .map_err(|_| "sales pipeline root is unavailable")?;
    if journal.starts_with(&root) {
        return Err("sales remote journal must be outside the host task root".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for b in &config.bindings {
        for value in [&b.id, &b.account, &b.workspace, &b.principal] {
            super::id(value).map_err(|_| "invalid sales remote binding")?;
        }
        if !seen.insert(b.id.clone())
            || b.client_digest.len() != 64
            || !b
                .client_digest
                .bytes()
                .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
            || b.members_epoch > 9_007_199_254_740_991
            || b.effects.len() > 7
        {
            return Err("invalid sales remote binding".into());
        }
        if b.credential.starts_with(&config.root) || b.credential.starts_with(&root) {
            return Err("sales credentials must be outside the host task root".into());
        }
    }
    Ok(config)
}

/// A record identifier: `lead_` and a digest, or another bounded id.
pub fn record_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Compare equal-length digests without an early exit.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

#[cfg(test)]
#[path = "remote/tests.rs"]
mod tests;
