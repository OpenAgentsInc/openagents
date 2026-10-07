//! One host-private sales pipeline. Human ownership, consent, retention, and
//! suppression are separate from agent identity and outbound authority.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub mod intake;

pub const SCHEMA: &str = "openagents.sales.pipeline.v1";
pub const COMMAND_SCHEMA: &str = "openagents.sales.pipeline-command.v1";
pub const LEAD_SCHEMA: &str = "openagents.sales.lead.v1";
pub const RECEIPT_SCHEMA: &str = "openagents.sales.receipt.v1";
const MAX_STATE: usize = 8 * 1024 * 1024;
const MAX_COMMAND: usize = 32 * 1024;
const MAX_LEADS: usize = 512;
const MAX_RECEIPTS: usize = 4096;
const MAX_PRINCIPALS: usize = 32;
const RETENTION_MAX: u64 = 366 * 24 * 60 * 60;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Writer,
    Reader,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Principal {
    role: Role,
    token_digest: String,
    active: bool,
}
/// A credential-derived capability. It is not serializable or caller-constructible.
pub struct Access {
    principal: String,
    token_digest: String,
}
impl Access {
    pub fn principal(&self) -> &str {
        &self.principal
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    New,
    Qualified,
    Pilot,
    Active,
    Closed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Unknown,
    Granted,
    Revoked,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permission {
    pub state: PermissionState,
    pub reference: String,
    pub recorded_at: u64,
    pub expires_at: u64,
    pub channels: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataBoundary {
    pub recipients: Vec<String>,
    pub permitted_use: String,
    pub retain_until: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NextAction {
    pub description: String,
    pub due_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomerDecision {
    pub decision: String,
    pub reference: String,
    pub at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Details {
    pub account: String,
    pub jurisdiction: String,
    pub permission: Permission,
    pub workflow: String,
    pub baseline_reference: String,
    pub data: DataBoundary,
    pub stage: Stage,
    pub next: Option<NextAction>,
    pub customer_decision: Option<CustomerDecision>,
    pub readers: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub contact: String,
    pub source: String,
    pub source_at: u64,
    pub details: Details,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub target: String,
    pub proposed_by: String,
    pub reference: String,
    pub proposed_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lead {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub contact: String,
    pub source: String,
    pub source_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub responsible_human: String,
    pub ownership_acceptance: String,
    pub details: Details,
    pub proposed_handoff: Option<Handoff>,
    /// Immutable public-intake provenance; manual records have none.
    #[serde(default)]
    pub intake: Option<intake::Provenance>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub schema: String,
    pub id: String,
    pub lead: Option<String>,
    pub expected_revision: u64,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Create {
        input: Input,
        ownership_acceptance: String,
    },
    Update {
        details: Details,
    },
    ProposeHandoff {
        target: String,
        reference: String,
    },
    AcceptHandoff {
        reference: String,
    },
    RejectHandoff {
        reference: String,
    },
    Suppress {
        reference: String,
    },
    Delete {
        reference: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Receipt {
    pub schema: String,
    pub command_digest: String,
    pub lead: String,
    pub revision: u64,
    pub sequence: u64,
    pub at: u64,
    pub outcome: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Recorded {
    actor: String,
    input_digest: String,
    receipt: Receipt,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Audit {
    pub sequence: u64,
    pub at: u64,
    pub actor: String,
    pub lead: String,
    pub operation: String,
    pub reference_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Suppression {
    at: u64,
    reference_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: String,
    owner: Option<String>,
    salt: String,
    sequence: u64,
    principals: BTreeMap<String, Principal>,
    leads: BTreeMap<String, Lead>,
    receipts: BTreeMap<String, Recorded>,
    suppressions: BTreeMap<String, Suppression>,
    audit: Vec<Audit>,
    #[serde(default)]
    intakes: BTreeMap<String, intake::Grant>,
    #[serde(default)]
    intake_submissions: BTreeMap<String, intake::RecordedSubmission>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            owner: None,
            salt: random_token(),
            sequence: 0,
            principals: BTreeMap::new(),
            leads: BTreeMap::new(),
            receipts: BTreeMap::new(),
            suppressions: BTreeMap::new(),
            audit: vec![],
            intakes: BTreeMap::new(),
            intake_submissions: BTreeMap::new(),
        }
    }
}
pub struct Store {
    dir: PathBuf,
    lock: File,
    state: State,
    clock: fn() -> u64,
    poisoned: bool,
}
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn random_token() -> String {
    let bytes = secp256k1::rand::random::<[u8; 32]>();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        Err("invalid sales identifier".into())
    } else {
        Ok(())
    }
}
fn text(s: &str, max: usize) -> Result<()> {
    if s.trim().is_empty() || s.len() > max || s.chars().any(|c| c.is_control() && c != '\n') {
        Err("invalid bounded sales text".into())
    } else {
        Ok(())
    }
}
fn contact(s: &str) -> Result<String> {
    text(s, 256)?;
    let normalized = s.to_ascii_lowercase();
    let (channel, address) = normalized
        .split_once(':')
        .ok_or("contact needs an explicit channel and stable address")?;
    id(channel)?;
    if s.chars().any(char::is_whitespace) || address.is_empty() {
        return Err("contact needs an explicit channel and stable address".into());
    }
    Ok(normalized)
}
fn token(s: &str) -> Result<()> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Err("invalid sales credential".into())
    } else {
        Ok(())
    }
}
fn validate(details: &Details, now: u64) -> Result<()> {
    for s in [
        &details.account,
        &details.jurisdiction,
        &details.workflow,
        &details.baseline_reference,
        &details.data.permitted_use,
    ] {
        text(s, 2048)?;
    }
    if details.data.retain_until <= now
        || details.data.retain_until > now.saturating_add(RETENTION_MAX)
        || details.data.recipients.len() > 16
        || details.readers.len() > 16
    {
        return Err("invalid sales retention or access bound".into());
    }
    for s in &details.data.recipients {
        text(s, 256)?;
    }
    for s in &details.readers {
        id(s)?;
    }
    let permission = &details.permission;
    text(&permission.reference, 256)?;
    if permission.recorded_at > now
        || permission.expires_at > details.data.retain_until
        || permission.channels.len() > 8
    {
        return Err("invalid permission evidence or expiry".into());
    }
    for channel in &permission.channels {
        id(channel)?;
    }
    if permission.state == PermissionState::Granted
        && (permission.expires_at <= now || permission.channels.is_empty())
    {
        return Err("granted permission is expired or has no channel".into());
    }
    if matches!(
        details.stage,
        Stage::Qualified | Stage::Pilot | Stage::Active
    ) && permission.state != PermissionState::Granted
    {
        return Err("qualified work requires current recorded permission".into());
    }
    match (&details.next, details.stage) {
        (Some(next), _) => {
            text(&next.description, 2048)?;
            if next.due_at > details.data.retain_until {
                return Err("next action exceeds retention".into());
            }
        }
        (None, Stage::Closed) => {}
        (None, _) => return Err("open lead requires a dated next action".into()),
    }
    if let Some(decision) = &details.customer_decision {
        text(&decision.decision, 256)?;
        text(&decision.reference, 256)?;
        if decision.at > now {
            return Err("customer decision is in the future".into());
        }
    }
    Ok(())
}
fn validate_contact_permission(address: &str, details: &Details) -> Result<()> {
    let normalized = contact(address)?;
    let channel = normalized.split_once(':').unwrap().0;
    if matches!(
        details.stage,
        Stage::Qualified | Stage::Pilot | Stage::Active
    ) && !details
        .permission
        .channels
        .iter()
        .any(|permitted| permitted == channel)
    {
        return Err("qualified contact channel is outside recorded permission".into());
    }
    Ok(())
}

impl Store {
    /// Uses the existing host-private directory, stable-lock, and atomic-file rules.
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with_clock(root, unix_now)
    }
    pub fn open_with_clock(root: &Path, clock: fn() -> u64) -> Result<Self> {
        super::prepare_directory(root).map_err(|e| e.to_string())?;
        let dir = root
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("sales");
        super::prepare_directory(&dir).map_err(|e| e.to_string())?;
        let lock = super::open_lock(&dir.join("sales.lock")).map_err(|e| e.to_string())?;
        super::take_lock(&lock, Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let path = dir.join("state.json");
        let state = if super::regular_or_absent(&path).map_err(|e| e.to_string())? {
            let mut bytes = Vec::new();
            super::private_open(&path, false, false)
                .map_err(|e| e.to_string())?
                .take((MAX_STATE + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_STATE {
                return Err("sales state exceeds bound".into());
            }
            serde_json::from_slice::<State>(&bytes).map_err(|_| "malformed sales state")?
        } else {
            State::default()
        };
        if state.schema != SCHEMA
            || state.leads.len() > MAX_LEADS
            || state.receipts.len() > MAX_RECEIPTS
            || state.principals.len() > MAX_PRINCIPALS
            || state.audit.len() > MAX_RECEIPTS
            || state.suppressions.len() > MAX_RECEIPTS
            || state.intakes.len() > MAX_PRINCIPALS
            || state.intake_submissions.len() > MAX_RECEIPTS - MAX_LEADS
        {
            return Err("unsupported or oversized sales state".into());
        }
        token(&state.salt)?;
        if state.leads.values().any(|lead| lead.schema != LEAD_SCHEMA)
            || state
                .receipts
                .values()
                .any(|r| r.receipt.schema != RECEIPT_SCHEMA)
        {
            return Err("unsupported sales record schema".into());
        }
        if let Some(owner) = &state.owner {
            id(owner)?;
            if !state
                .principals
                .get(owner)
                .is_some_and(|p| p.active && p.role == Role::Owner)
            {
                return Err("sales owner is unavailable".into());
            }
        }
        let mut store = Self {
            dir,
            lock,
            state,
            clock,
            poisoned: false,
        };
        store.refresh()?;
        Ok(store)
    }
    fn persist(&mut self, next: State) -> Result<()> {
        if self.poisoned {
            return Err("sales store needs recovery".into());
        }
        super::verify_same_file(&self.dir.join("sales.lock"), &self.lock)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_STATE {
            return Err("sales state exceeds bound".into());
        }
        if let Err(e) = super::replace_file(&self.dir, "state.json", &bytes) {
            self.poisoned = true;
            return Err(e.to_string());
        }
        self.state = next;
        Ok(())
    }
    fn suppression(state: &State, address: &str) -> Result<String> {
        Ok(digest(
            format!("{}:{}", state.salt, contact(address)?).as_bytes(),
        ))
    }
    fn remove(state: &mut State, lead: &str, now: u64, reference: &str) -> Result<()> {
        let found = state.leads.get(lead).ok_or("lead is unavailable")?;
        let key = Self::suppression(state, &found.contact)?;
        if !state.suppressions.contains_key(&key) && state.suppressions.len() >= MAX_RECEIPTS {
            return Err(
                "suppression bound reached; operator retention maintenance required".into(),
            );
        }
        state
            .suppressions
            .entry(key.clone())
            .or_insert(Suppression {
                at: now,
                reference_digest: digest(reference.as_bytes()),
            });
        // Suppression is contact-wide, including separate workflow records.
        // Keeping another active record would permit recontact after deletion.
        let salt = state.salt.clone();
        state.leads.retain(|_, record| {
            contact(&record.contact)
                .map(|address| digest(format!("{salt}:{address}").as_bytes()) != key)
                .unwrap_or(false)
        });
        Ok(())
    }
    fn refresh(&mut self) -> Result<()> {
        if self.poisoned {
            return Err("sales store needs recovery".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        let expired = next
            .leads
            .values()
            .filter(|l| l.details.data.retain_until <= now)
            .map(|l| l.id.clone())
            .collect::<Vec<_>>();
        for lead in &expired {
            if next.leads.contains_key(lead) {
                Self::remove(&mut next, lead, now, "retention expired")?;
            }
        }
        // Revoked/expired permission stops qualification and cancels proposed handoffs.
        let mut changed = !expired.is_empty();
        for lead in next.leads.values_mut() {
            if lead.details.permission.state == PermissionState::Granted
                && lead.details.permission.expires_at <= now
            {
                lead.details.permission.state = PermissionState::Revoked;
                lead.details.stage = Stage::Closed;
                lead.details.next = None;
                lead.proposed_handoff = None;
                lead.revision = lead
                    .revision
                    .checked_add(1)
                    .ok_or("lead revision overflow")?;
                lead.updated_at = now;
                changed = true;
            }
        }
        if changed {
            self.persist(next)?;
        }
        Ok(())
    }
    fn check(&self, access: &Access) -> Result<Role> {
        if self.poisoned {
            return Err("sales store needs recovery".into());
        }
        let p = self
            .state
            .principals
            .get(&access.principal)
            .filter(|p| p.active && p.token_digest == access.token_digest)
            .ok_or("sales access refused")?;
        Ok(p.role)
    }
    fn admin(&self, access: &Access) -> Result<()> {
        if self.check(access)? == Role::Owner {
            Ok(())
        } else {
            Err("sales owner access required".into())
        }
    }
    fn readable(&self, access: &Access, lead: &Lead) -> Result<()> {
        let role = self.check(access)?;
        if !Self::recipient(&lead.details, &access.principal) {
            return Err("human is outside the recorded data recipient boundary".into());
        }
        if role == Role::Owner
            || lead.responsible_human == access.principal
            || lead.details.readers.contains(&access.principal)
            || lead
                .proposed_handoff
                .as_ref()
                .is_some_and(|h| h.target == access.principal)
        {
            Ok(())
        } else {
            Err("sales record access refused".into())
        }
    }
    fn writable(&self, access: &Access, lead: &Lead) -> Result<()> {
        let role = self.check(access)?;
        if role == Role::Owner
            || (role == Role::Writer && lead.responsible_human == access.principal)
        {
            Ok(())
        } else {
            Err("sales record update refused".into())
        }
    }
    /// Local filesystem ownership permits initialization only. Later operations
    /// require credentials bound to the named human and current recorded rights.
    pub fn initialize(&mut self, owner: &str, credential: &Path) -> Result<()> {
        id(owner)?;
        if self.state.owner.is_some() {
            return Err("sales pipeline already initialized".into());
        }
        let secret = self.credential(credential)?;
        let mut next = self.state.clone();
        next.owner = Some(owner.into());
        next.principals.insert(
            owner.into(),
            Principal {
                role: Role::Owner,
                token_digest: digest(secret.as_bytes()),
                active: true,
            },
        );
        self.persist(next)
    }
    fn external_file(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let resolved = parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(path.file_name().ok_or("file name required")?);
        if resolved.starts_with(&self.dir) {
            return Err(
                "credential and export files must be outside the pipeline store directory".into(),
            );
        }
        Ok(())
    }
    fn credential(&self, path: &Path) -> Result<String> {
        self.external_file(path)?;
        if super::regular_or_absent(path).map_err(|e| e.to_string())? {
            return Self::read_credential(path);
        }
        let secret = random_token();
        let mut file = super::private_open(path, true, true).map_err(|e| e.to_string())?;
        file.write_all(secret.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|e| e.to_string())?;
        Ok(secret)
    }
    pub fn read_credential(path: &Path) -> Result<String> {
        let mut bytes = Vec::new();
        super::private_open(path, false, false)
            .map_err(|e| e.to_string())?
            .take(129)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 128 {
            return Err("sales credential exceeds bound".into());
        }
        let secret = std::str::from_utf8(&bytes)
            .map_err(|_| "invalid sales credential")?
            .trim()
            .to_string();
        token(&secret)?;
        Ok(secret)
    }
    pub fn authenticate(&mut self, secret: &str) -> Result<Access> {
        self.refresh()?;
        token(secret)?;
        let token_digest = digest(secret.as_bytes());
        let (principal, _) = self
            .state
            .principals
            .iter()
            .find(|(_, p)| p.active && p.token_digest == token_digest)
            .ok_or("sales access refused")?;
        Ok(Access {
            principal: principal.clone(),
            token_digest,
        })
    }
    pub fn issue(
        &mut self,
        access: &Access,
        human: &str,
        role: Role,
        credential: &Path,
    ) -> Result<()> {
        self.refresh()?;
        self.admin(access)?;
        id(human)?;
        if role == Role::Owner {
            return Err("cannot issue another pipeline owner".into());
        }
        let secret = self.credential(credential)?;
        let hash = digest(secret.as_bytes());
        if let Some(prior) = self.state.principals.get(human) {
            return if prior.token_digest == hash && prior.role == role && prior.active {
                Ok(())
            } else {
                Err("principal already exists; use a distinct principal credential".into())
            };
        }
        if self.state.principals.len() >= MAX_PRINCIPALS
            || self
                .state
                .principals
                .values()
                .any(|p| p.token_digest == hash)
        {
            return Err("principal bound or credential conflict".into());
        }
        let mut next = self.state.clone();
        next.principals.insert(
            human.into(),
            Principal {
                role,
                token_digest: hash,
                active: true,
            },
        );
        self.persist(next)
    }
    pub fn revoke(&mut self, access: &Access, human: &str) -> Result<()> {
        self.refresh()?;
        self.admin(access)?;
        if self.state.owner.as_deref() == Some(human) {
            return Err("pipeline owner cannot revoke itself".into());
        }
        let mut next = self.state.clone();
        next.principals
            .get_mut(human)
            .ok_or("unknown sales principal")?
            .active = false;
        for lead in next.leads.values_mut() {
            if lead
                .proposed_handoff
                .as_ref()
                .is_some_and(|h| h.target == human)
            {
                lead.proposed_handoff = None;
                lead.revision = lead
                    .revision
                    .checked_add(1)
                    .ok_or("lead revision overflow")?;
            }
        }
        self.persist(next)
    }
    pub fn show(&mut self, access: &Access, lead: &str) -> Result<Lead> {
        self.refresh()?;
        let found = self.state.leads.get(lead).ok_or("lead is unavailable")?;
        self.readable(access, found)?;
        Ok(found.clone())
    }
    pub fn list(
        &mut self,
        access: &Access,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Lead>> {
        self.refresh()?;
        self.check(access)?;
        if limit == 0 || limit > 100 {
            return Err("sales page bound is 1 to 100".into());
        }
        Ok(self
            .state
            .leads
            .values()
            .filter(|lead| {
                after.is_none_or(|a| lead.id.as_str() > a) && self.readable(access, lead).is_ok()
            })
            .take(limit)
            .cloned()
            .collect())
    }
    pub fn audit(&mut self, access: &Access, after: u64, limit: usize) -> Result<Vec<Audit>> {
        self.refresh()?;
        self.admin(access)?;
        if limit == 0 || limit > 100 {
            return Err("sales audit page bound is 1 to 100".into());
        }
        Ok(self
            .state
            .audit
            .iter()
            .filter(|a| a.sequence > after)
            .take(limit)
            .cloned()
            .collect())
    }
    pub fn is_suppressed(&mut self, access: &Access, address: &str) -> Result<bool> {
        self.refresh()?;
        if self.check(access)? == Role::Reader {
            return Err("suppression lookup requires sales write authority".into());
        }
        Ok(self
            .state
            .suppressions
            .contains_key(&Self::suppression(&self.state, address)?))
    }
    pub fn apply(&mut self, access: &Access, bytes: &[u8]) -> Result<Receipt> {
        self.refresh()?;
        let role = self.check(access)?;
        if bytes.len() > MAX_COMMAND {
            return Err("sales command exceeds bound".into());
        }
        let c: Command = serde_json::from_slice(bytes).map_err(|_| "malformed sales command")?;
        if c.schema != COMMAND_SCHEMA {
            return Err("unsupported sales command schema".into());
        }
        id(&c.id)?;
        let key = digest(c.id.as_bytes());
        let input_digest = digest(bytes);
        if let Some(prior) = self.state.receipts.get(&key) {
            if prior.actor == access.principal && prior.input_digest == input_digest {
                return Ok(prior.receipt.clone());
            }
            return Err("sales command idempotency conflict".into());
        }
        // Reserve one cleanup receipt for every possible live lead. Once the
        // ordinary history fills, no operation can add records or consume this
        // reserve except a deletion/suppression that removes a live record.
        let cleanup = matches!(
            c.operation,
            Operation::Delete { .. } | Operation::Suppress { .. }
        );
        let history_limit = if cleanup {
            MAX_RECEIPTS
        } else {
            MAX_RECEIPTS - MAX_LEADS
        };
        if self.state.receipts.len() >= history_limit || self.state.audit.len() >= history_limit {
            return Err(if cleanup {
                "sales cleanup history bound reached"
            } else {
                "sales ordinary history bound reached; deletion and suppression remain available"
            }
            .into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        let lead_id;
        let revision;
        let outcome;
        let reference;
        if let Operation::Create {
            input,
            ownership_acceptance,
        } = &c.operation
        {
            if role == Role::Reader {
                return Err("reader cannot create a lead".into());
            }
            if c.lead.is_some() || c.expected_revision != 0 || next.leads.len() >= MAX_LEADS {
                return Err("invalid lead creation or record bound".into());
            }
            contact(&input.contact)?;
            text(&input.source, 2048)?;
            text(ownership_acceptance, 256)?;
            if input.source_at > now {
                return Err("contact source is in the future".into());
            }
            validate(&input.details, now)?;
            validate_contact_permission(&input.contact, &input.details)?;
            self.validate_readers(&input.details, &access.principal)?;
            if next
                .suppressions
                .contains_key(&Self::suppression(&next, &input.contact)?)
            {
                return Err("contact is suppressed".into());
            }
            lead_id = format!("lead_{}", key);
            revision = 1;
            outcome = "created";
            reference = ownership_acceptance.clone();
            next.leads.insert(
                lead_id.clone(),
                Lead {
                    schema: LEAD_SCHEMA.into(),
                    id: lead_id.clone(),
                    revision,
                    contact: input.contact.clone(),
                    source: input.source.clone(),
                    source_at: input.source_at,
                    created_at: now,
                    updated_at: now,
                    responsible_human: access.principal.clone(),
                    ownership_acceptance: ownership_acceptance.clone(),
                    details: input.details.clone(),
                    proposed_handoff: None,
                    intake: None,
                },
            );
        } else {
            lead_id = c.lead.clone().ok_or("lead identity required")?;
            let found = next.leads.get(&lead_id).ok_or("lead is unavailable")?;
            if found.revision != c.expected_revision {
                return Err("sales revision conflict".into());
            }
            revision = found
                .revision
                .checked_add(1)
                .ok_or("lead revision overflow")?;
            match &c.operation {
                Operation::AcceptHandoff { reference: r } => {
                    text(r, 256)?;
                    if role == Role::Reader {
                        return Err("reader cannot accept accountable ownership".into());
                    }
                    let proposal = found
                        .proposed_handoff
                        .as_ref()
                        .ok_or("no proposed handoff")?;
                    if proposal.target != access.principal {
                        return Err("only the proposed human may accept".into());
                    }
                    let lead = next.leads.get_mut(&lead_id).unwrap();
                    lead.responsible_human = access.principal.clone();
                    lead.ownership_acceptance = r.clone();
                    lead.proposed_handoff = None;
                    outcome = "handoff_accepted";
                    reference = r.clone();
                }
                Operation::RejectHandoff { reference: r } => {
                    text(r, 256)?;
                    if !found
                        .proposed_handoff
                        .as_ref()
                        .is_some_and(|p| p.target == access.principal)
                    {
                        self.writable(access, found)?;
                    }
                    next.leads.get_mut(&lead_id).unwrap().proposed_handoff = None;
                    outcome = "handoff_rejected";
                    reference = r.clone();
                }
                Operation::Update { details } => {
                    self.writable(access, found)?;
                    validate(details, now)?;
                    validate_contact_permission(&found.contact, details)?;
                    self.validate_readers(details, &found.responsible_human)?;
                    let prior_permission = &found.details.permission;
                    let permission_expands = details.permission.state == PermissionState::Granted
                        && (prior_permission.state != PermissionState::Granted
                            || details.permission.expires_at > prior_permission.expires_at
                            || details
                                .permission
                                .channels
                                .iter()
                                .any(|channel| !prior_permission.channels.contains(channel)));
                    if permission_expands
                        && details.permission.reference == prior_permission.reference
                    {
                        return Err("renewed or expanded permission requires fresh evidence".into());
                    }
                    if details.data.recipients != found.details.data.recipients
                        || details.data.permitted_use != found.details.data.permitted_use
                        || details.data.retain_until > found.details.data.retain_until
                    {
                        self.admin(access)?;
                        if details.permission.reference == found.details.permission.reference {
                            return Err(
                                "changed data recipients, use, or extended retention require fresh permission evidence"
                                    .into(),
                            );
                        }
                    }
                    let lead = next.leads.get_mut(&lead_id).unwrap();
                    lead.details = details.clone();
                    lead.proposed_handoff = None;
                    outcome = "updated";
                    reference = "conditional update".into();
                }
                Operation::ProposeHandoff {
                    target,
                    reference: r,
                } => {
                    self.writable(access, found)?;
                    text(r, 256)?;
                    id(target)?;
                    if !Self::recipient(&found.details, target) {
                        return Err(
                            "handoff target is outside the recorded data recipient boundary".into(),
                        );
                    }
                    if target == &found.responsible_human
                        || !next
                            .principals
                            .get(target)
                            .is_some_and(|p| p.active && p.role != Role::Reader)
                    {
                        return Err("handoff target is unavailable".into());
                    }
                    next.leads.get_mut(&lead_id).unwrap().proposed_handoff = Some(Handoff {
                        target: target.clone(),
                        proposed_by: access.principal.clone(),
                        reference: r.clone(),
                        proposed_at: now,
                    });
                    outcome = "handoff_proposed";
                    reference = r.clone();
                }
                Operation::Suppress { reference: r } | Operation::Delete { reference: r } => {
                    self.writable(access, found)?;
                    text(r, 256)?;
                    Self::remove(&mut next, &lead_id, now, r)?;
                    outcome = if matches!(&c.operation, Operation::Delete { .. }) {
                        "deleted"
                    } else {
                        "suppressed"
                    };
                    reference = r.clone();
                }
                Operation::Create { .. } => unreachable!(),
            }
            if let Some(lead) = next.leads.get_mut(&lead_id) {
                lead.revision = revision;
                lead.updated_at = now;
            }
        }
        next.sequence = next
            .sequence
            .checked_add(1)
            .ok_or("sales sequence overflow")?;
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.into(),
            command_digest: key.clone(),
            lead: lead_id.clone(),
            revision,
            sequence: next.sequence,
            at: now,
            outcome: outcome.into(),
        };
        next.audit.push(Audit {
            sequence: next.sequence,
            at: now,
            actor: access.principal.clone(),
            lead: lead_id,
            operation: outcome.into(),
            reference_digest: digest(reference.as_bytes()),
        });
        next.receipts.insert(
            key,
            Recorded {
                actor: access.principal.clone(),
                input_digest,
                receipt: receipt.clone(),
            },
        );
        self.persist(next)?;
        Ok(receipt)
    }
    fn recipient(details: &Details, human: &str) -> bool {
        details.data.recipients.contains(&format!("human:{human}"))
    }
    fn validate_readers(&self, details: &Details, responsible: &str) -> Result<()> {
        if !Self::recipient(details, responsible)
            || !self
                .state
                .owner
                .as_ref()
                .is_some_and(|owner| Self::recipient(details, owner))
            || details
                .readers
                .iter()
                .any(|reader| !Self::recipient(details, reader))
        {
            return Err("record access must stay within the recorded data recipients".into());
        }
        if details
            .readers
            .iter()
            .any(|r| !self.state.principals.get(r).is_some_and(|p| p.active))
        {
            Err("unknown or revoked reader".into())
        } else {
            Ok(())
        }
    }
    pub fn export(&mut self, access: &Access, lead: &str, path: &Path) -> Result<String> {
        let record = self.show(access, lead)?;
        self.external_file(path)?;
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        let mut file = super::private_open(path, true, true).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|e| e.to_string())?;
        Ok(digest(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    fn now() -> u64 {
        1000
    }
    fn later() -> u64 {
        2001
    }
    fn expired_permission() -> u64 {
        1501
    }
    fn details() -> Details {
        Details {
            account: "synthetic-account".into(),
            jurisdiction: "synthetic jurisdiction record".into(),
            permission: Permission {
                state: PermissionState::Granted,
                reference: "synthetic-consent-v1".into(),
                recorded_at: 999,
                expires_at: 1500,
                channels: vec!["email".into()],
            },
            workflow: "one synthetic repository maintenance task".into(),
            baseline_reference: "private-baseline-reference".into(),
            data: DataBoundary {
                recipients: vec![
                    "human:operator".into(),
                    "human:writer-a".into(),
                    "human:writer-b".into(),
                ],
                permitted_use: "one agreed pilot; no marketing reuse".into(),
                retain_until: 2000,
            },
            stage: Stage::Qualified,
            next: Some(NextAction {
                description: "review pilot scope".into(),
                due_at: 1100,
            }),
            customer_decision: None,
            readers: vec![],
        }
    }
    fn command(id: &str, lead: Option<&str>, revision: u64, operation: Operation) -> Vec<u8> {
        serde_json::to_vec(&Command {
            schema: COMMAND_SCHEMA.into(),
            id: id.into(),
            lead: lead.map(Into::into),
            expected_revision: revision,
            operation,
        })
        .unwrap()
    }
    fn create(id: &str) -> Vec<u8> {
        command(
            id,
            None,
            0,
            Operation::Create {
                input: Input {
                    contact: "email:prospect@synthetic.invalid".into(),
                    source: "synthetic private introduction".into(),
                    source_at: 990,
                    details: details(),
                },
                ownership_acceptance: "operator accepted responsibility".into(),
            },
        )
    }
    fn fixture() -> (TempDir, Store, Access, PathBuf) {
        let dir = TempDir::new().unwrap();
        let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        let cred = dir.path().join("operator");
        store.initialize("operator", &cred).unwrap();
        let a = store
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        (dir, store, a, cred)
    }
    fn grant(dir: &TempDir, store: &mut Store, admin: &Access, name: &str, role: Role) -> Access {
        let path = dir.path().join(name);
        store.issue(admin, name, role, &path).unwrap();
        store
            .authenticate(&Store::read_credential(&path).unwrap())
            .unwrap()
    }
    #[test]
    fn restart_replay_preserves_ids_owner_permission_stage_next_and_audit() {
        let (dir, mut s, a, cred) = fixture();
        let bytes = create("create-once");
        let r = s.apply(&a, &bytes).unwrap();
        assert_eq!(s.apply(&a, &bytes).unwrap(), r);
        let lead = s.show(&a, &r.lead).unwrap();
        assert_eq!(lead.responsible_human, "operator");
        assert_eq!(lead.details.stage, Stage::Qualified);
        assert_eq!(lead.details.next.as_ref().unwrap().due_at, 1100);
        drop(s);
        let mut s = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        let a = s
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        assert_eq!(s.apply(&a, &bytes).unwrap(), r);
        let reopened = s.show(&a, &r.lead).unwrap();
        assert_eq!(
            serde_json::to_value(lead).unwrap(),
            serde_json::to_value(reopened).unwrap()
        );
        assert_eq!(s.audit(&a, 0, 100).unwrap().len(), 1);
        let mut c: Command = serde_json::from_slice(&bytes).unwrap();
        if let Operation::Create { input, .. } = &mut c.operation {
            input.source = "changed".into();
        }
        assert!(
            s.apply(&a, &serde_json::to_vec(&c).unwrap())
                .unwrap_err()
                .contains("idempotency")
        );
    }
    #[test]
    fn only_the_named_human_can_accept_handoff_and_previous_owner_loses_write() {
        let (dir, mut s, admin, _) = fixture();
        let a = grant(&dir, &mut s, &admin, "writer-a", Role::Writer);
        let b = grant(&dir, &mut s, &admin, "writer-b", Role::Writer);
        let receipt = s.apply(&a, &create("writer-created")).unwrap();
        let proposal = command(
            "propose",
            Some(&receipt.lead),
            1,
            Operation::ProposeHandoff {
                target: "writer-b".into(),
                reference: "private agreed handoff".into(),
            },
        );
        let p = s.apply(&a, &proposal).unwrap();
        assert_eq!(
            s.show(&b, &receipt.lead).unwrap().responsible_human,
            "writer-a"
        );
        let accept = command(
            "accept",
            Some(&receipt.lead),
            p.revision,
            Operation::AcceptHandoff {
                reference: "target accepted the handoff".into(),
            },
        );
        assert!(s.apply(&a, &accept).is_err());
        assert!(s.apply(&admin, &accept).is_err());
        let r = s.apply(&b, &accept).unwrap();
        assert_eq!(s.apply(&b, &accept).unwrap(), r);
        assert_eq!(s.show(&b, &r.lead).unwrap().responsible_human, "writer-b");
        assert!(s.show(&a, &r.lead).is_err());
        let update = command(
            "old-owner-update",
            Some(&r.lead),
            r.revision,
            Operation::Update { details: details() },
        );
        assert!(s.apply(&a, &update).is_err());
    }
    #[test]
    fn unauthorized_read_export_list_and_update_refuse() {
        let (dir, mut s, a, _) = fixture();
        let receipt = s.apply(&a, &create("private")).unwrap();
        let reader = grant(&dir, &mut s, &a, "reader", Role::Reader);
        let output = dir.path().join("unauthorized.json");
        assert!(s.show(&reader, &receipt.lead).is_err());
        assert!(s.export(&reader, &receipt.lead, &output).is_err());
        assert!(!output.exists());
        assert!(s.list(&reader, None, 100).unwrap().is_empty());
        assert!(s.apply(&reader, &create("reader-create")).is_err());
        assert!(s.audit(&reader, 0, 10).is_err());
        assert!(
            s.is_suppressed(&reader, "email:prospect@synthetic.invalid")
                .is_err()
        );
        let mut d = details();
        d.readers.push("reader".into());
        d.data.recipients.push("human:reader".into());
        d.permission.reference = "new-reader-consent".into();
        s.apply(
            &a,
            &command(
                "grant-read",
                Some(&receipt.lead),
                1,
                Operation::Update { details: d },
            ),
        )
        .unwrap();
        assert!(s.show(&reader, &receipt.lead).is_ok());
        assert!(
            s.apply(
                &reader,
                &command(
                    "reader-write",
                    Some(&receipt.lead),
                    2,
                    Operation::Update { details: details() }
                )
            )
            .is_err()
        );
        s.revoke(&a, "reader").unwrap();
        assert!(s.show(&reader, &receipt.lead).is_err());
    }
    #[test]
    fn deletion_retains_minimum_suppression_without_customer_material() {
        let (dir, mut s, a, cred) = fixture();
        let r = s.apply(&a, &create("deleted")).unwrap();
        let delete = command(
            "delete",
            Some(&r.lead),
            1,
            Operation::Delete {
                reference: "customer asked to stop and remove content".into(),
            },
        );
        s.apply(&a, &delete).unwrap();
        assert!(s.show(&a, &r.lead).is_err());
        assert!(
            s.is_suppressed(&a, "email:PROSPECT@synthetic.invalid")
                .unwrap()
        );
        assert!(
            s.apply(&a, &create("recontact"))
                .unwrap_err()
                .contains("suppressed")
        );
        drop(s);
        let bytes = std::fs::read(dir.path().join("host/sales/state.json")).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        for private in [
            "prospect@synthetic.invalid",
            "synthetic private introduction",
            "private-baseline-reference",
            "customer asked to stop",
        ] {
            assert!(!text.contains(private));
        }
        let mut s = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        let a = s
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        assert!(
            s.is_suppressed(&a, "email:prospect@synthetic.invalid")
                .unwrap()
        );
        assert!(s.apply(&a, &delete).is_ok());
    }
    #[test]
    fn suppression_removes_other_workflows_for_the_same_contact() {
        let (_dir, mut s, a, _) = fixture();
        let first = s.apply(&a, &create("first-workflow")).unwrap();
        let other = s.apply(&a, &create("other-workflow")).unwrap();
        assert_eq!(s.list(&a, None, 100).unwrap().len(), 2);
        s.apply(
            &a,
            &command(
                "stop-all",
                Some(&first.lead),
                1,
                Operation::Suppress {
                    reference: "contact asked to stop".into(),
                },
            ),
        )
        .unwrap();
        assert!(s.show(&a, &other.lead).is_err());
        assert!(s.list(&a, None, 100).unwrap().is_empty());
        assert!(s.apply(&a, &create("third-workflow")).is_err());
    }

    #[test]
    fn a_full_ordinary_history_cannot_block_privacy_cleanup() {
        let (_dir, mut s, a, _) = fixture();
        let r = s.apply(&a, &create("history")).unwrap();
        let recorded = s.state.receipts.values().next().unwrap().clone();
        while s.state.receipts.len() < MAX_RECEIPTS - MAX_LEADS {
            let key = digest(format!("synthetic-record-{}", s.state.receipts.len()).as_bytes());
            s.state.receipts.insert(key, recorded.clone());
        }
        assert!(
            s.apply(&a, &create("history-full"))
                .unwrap_err()
                .contains("ordinary history")
        );
        let deletion = command(
            "privacy-cleanup",
            Some(&r.lead),
            1,
            Operation::Delete {
                reference: "private deletion request".into(),
            },
        );
        let receipt = s.apply(&a, &deletion).unwrap();
        assert_eq!(s.apply(&a, &deletion).unwrap(), receipt);
        assert!(s.show(&a, &r.lead).is_err());
        assert!(
            s.is_suppressed(&a, "email:prospect@synthetic.invalid")
                .unwrap()
        );
    }

    #[test]
    fn retention_purges_content_on_restart_and_permission_expiry_stops_work() {
        let (dir, mut s, a, cred) = fixture();
        let r = s.apply(&a, &create("expire")).unwrap();
        drop(s);
        let mut s = Store::open_with_clock(&dir.path().join("host"), expired_permission).unwrap();
        let a = s
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        let lead = s.show(&a, &r.lead).unwrap();
        assert_eq!(lead.details.permission.state, PermissionState::Revoked);
        assert_eq!(lead.details.stage, Stage::Closed);
        assert!(lead.details.next.is_none());
        assert!(lead.revision > 1);
        drop(s);
        let mut s = Store::open_with_clock(&dir.path().join("host"), later).unwrap();
        let a = s
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        assert!(s.show(&a, &r.lead).is_err());
        assert!(
            s.is_suppressed(&a, "email:prospect@synthetic.invalid")
                .unwrap()
        );
        let text = std::fs::read_to_string(dir.path().join("host/sales/state.json")).unwrap();
        assert!(!text.contains("prospect@synthetic.invalid"));
    }
    #[test]
    fn stale_revision_and_changed_scope_do_not_reuse_a_proposed_handoff() {
        let (dir, mut s, a, _) = fixture();
        let b = grant(&dir, &mut s, &a, "writer-b", Role::Writer);
        let r = s.apply(&a, &create("scope")).unwrap();
        s.apply(
            &a,
            &command(
                "proposal",
                Some(&r.lead),
                1,
                Operation::ProposeHandoff {
                    target: "writer-b".into(),
                    reference: "proposal".into(),
                },
            ),
        )
        .unwrap();
        s.apply(
            &a,
            &command(
                "new-scope",
                Some(&r.lead),
                2,
                Operation::Update { details: details() },
            ),
        )
        .unwrap();
        assert!(
            s.apply(
                &b,
                &command(
                    "stale-accept",
                    Some(&r.lead),
                    2,
                    Operation::AcceptHandoff {
                        reference: "accept".into()
                    }
                )
            )
            .is_err()
        );
        assert!(
            s.apply(
                &b,
                &command(
                    "new-accept",
                    Some(&r.lead),
                    3,
                    Operation::AcceptHandoff {
                        reference: "accept".into()
                    }
                )
            )
            .is_err()
        );
        assert_eq!(s.show(&a, &r.lead).unwrap().responsible_human, "operator");
    }
    #[test]
    #[cfg(unix)]
    fn private_storage_export_and_credentials_never_use_public_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut s, a, cred) = fixture();
        let r = s.apply(&a, &create("export")).unwrap();
        let output = dir.path().join("export.json");
        s.export(&a, &r.lead, &output).unwrap();
        assert!(s.export(&a, &r.lead, &output).is_err());
        for path in [&output, &cred, &dir.path().join("host/sales/state.json")] {
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::set_permissions(&cred, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Store::read_credential(&cred).is_err());
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&output, &alias).unwrap();
        assert!(s.export(&a, &r.lead, &alias).is_err());
    }
    #[test]
    fn invalid_consent_retention_next_owner_and_bounds_refuse_atomically() {
        let (_dir, mut s, a, _) = fixture();
        let mut c: Command = serde_json::from_slice(&create("invalid")).unwrap();
        if let Operation::Create { input, .. } = &mut c.operation {
            input.details.next = None;
        }
        assert!(s.apply(&a, &serde_json::to_vec(&c).unwrap()).is_err());
        assert!(s.list(&a, None, 100).unwrap().is_empty());
        let mut c: Command = serde_json::from_slice(&create("invalid-permission")).unwrap();
        if let Operation::Create { input, .. } = &mut c.operation {
            input.details.permission.state = PermissionState::Unknown;
        }
        assert!(s.apply(&a, &serde_json::to_vec(&c).unwrap()).is_err());
        let mut c: Command = serde_json::from_slice(&create("invalid-channel")).unwrap();
        if let Operation::Create { input, .. } = &mut c.operation {
            input.contact = "phone:synthetic-number".into();
        }
        assert!(s.apply(&a, &serde_json::to_vec(&c).unwrap()).is_err());
        assert!(contact("email:").is_err());
        assert!(s.list(&a, None, 0).is_err());
        assert!(s.list(&a, None, 101).is_err());
        assert!(s.apply(&a, &vec![b'a'; MAX_COMMAND + 1]).is_err());
        assert!(s.revoke(&a, "operator").is_err());
    }
    #[test]
    fn recipients_bound_readers_exports_and_handoff_targets() {
        let (dir, mut s, a, _) = fixture();
        let reader = grant(&dir, &mut s, &a, "reader", Role::Reader);
        let outsider = grant(&dir, &mut s, &a, "outsider", Role::Writer);
        let r = s.apply(&a, &create("boundary")).unwrap();
        let mut d = details();
        d.readers.push("reader".into());
        assert!(
            s.apply(
                &a,
                &command(
                    "outside-reader",
                    Some(&r.lead),
                    1,
                    Operation::Update { details: d }
                )
            )
            .is_err()
        );
        assert!(s.show(&reader, &r.lead).is_err());
        assert!(
            s.export(&reader, &r.lead, &dir.path().join("outside.json"))
                .is_err()
        );
        assert!(
            s.apply(
                &a,
                &command(
                    "outside-handoff",
                    Some(&r.lead),
                    1,
                    Operation::ProposeHandoff {
                        target: "outsider".into(),
                        reference: "proposal".into()
                    }
                )
            )
            .is_err()
        );
        assert!(s.show(&outsider, &r.lead).is_err());
        let mut d = details();
        d.data.recipients.push("human:outsider".into());
        assert!(
            s.apply(
                &a,
                &command(
                    "same-evidence",
                    Some(&r.lead),
                    1,
                    Operation::Update { details: d }
                )
            )
            .is_err()
        );
        let writer = grant(&dir, &mut s, &a, "writer-a", Role::Writer);
        let w = s.apply(&writer, &create("writer-boundary")).unwrap();
        let mut d = details();
        d.data.recipients.push("human:outsider".into());
        d.permission.reference = "changed-boundary-permission".into();
        assert!(
            s.apply(
                &writer,
                &command(
                    "writer-widen",
                    Some(&w.lead),
                    1,
                    Operation::Update { details: d }
                )
            )
            .is_err()
        );
        let mut d = details();
        d.data.retain_until = 2500;
        assert!(
            s.apply(
                &writer,
                &command(
                    "writer-retain",
                    Some(&w.lead),
                    1,
                    Operation::Update { details: d.clone() }
                )
            )
            .is_err()
        );
        assert!(
            s.apply(
                &a,
                &command(
                    "owner-retain-old-evidence",
                    Some(&w.lead),
                    1,
                    Operation::Update { details: d.clone() }
                )
            )
            .is_err()
        );
        d.permission.reference = "fresh-retention-agreement".into();
        s.apply(
            &a,
            &command(
                "owner-retain",
                Some(&w.lead),
                1,
                Operation::Update { details: d },
            ),
        )
        .unwrap();
    }

    #[test]
    fn old_permission_evidence_cannot_extend_expiry_or_add_channels() {
        let (_dir, mut s, a, _) = fixture();
        let r = s.apply(&a, &create("permission-change")).unwrap();
        for name in ["extend", "channel"] {
            let mut d = details();
            if name == "extend" {
                d.permission.expires_at = 1700;
            } else {
                d.permission.channels.push("phone".into());
            }
            assert!(
                s.apply(
                    &a,
                    &command(name, Some(&r.lead), 1, Operation::Update { details: d })
                )
                .is_err()
            );
        }
        let mut d = details();
        d.permission.expires_at = 1700;
        d.permission.reference = "new-consent-duration".into();
        s.apply(
            &a,
            &command(
                "fresh-duration",
                Some(&r.lead),
                1,
                Operation::Update { details: d },
            ),
        )
        .unwrap();
        assert_eq!(
            s.show(&a, &r.lead).unwrap().details.permission.expires_at,
            1700
        );
    }

    #[test]
    fn credentials_and_exports_cannot_collide_with_store_or_temporary_files() {
        let dir = TempDir::new().unwrap();
        let mut s = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        assert!(s.initialize("operator", &s.dir.join("state.json")).is_err());
        assert!(s.state.owner.is_none());
        let cred = dir.path().join("owner");
        s.initialize("operator", &cred).unwrap();
        let a = s
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        assert!(
            s.issue(&a, "writer-a", Role::Writer, &s.dir.join(".state.json.tmp"))
                .is_err()
        );
        let r = s.apply(&a, &create("collision")).unwrap();
        assert!(
            s.export(&a, &r.lead, &s.dir.join(".state.json.tmp"))
                .is_err()
        );
        assert!(s.show(&a, &r.lead).is_ok());
    }

    #[test]
    fn revoked_target_cannot_accept_or_reauthenticate() {
        let (dir, mut s, a, _) = fixture();
        let b = grant(&dir, &mut s, &a, "writer-b", Role::Writer);
        let r = s.apply(&a, &create("revoke-target")).unwrap();
        s.apply(
            &a,
            &command(
                "proposal",
                Some(&r.lead),
                1,
                Operation::ProposeHandoff {
                    target: "writer-b".into(),
                    reference: "proposal".into(),
                },
            ),
        )
        .unwrap();
        s.revoke(&a, "writer-b").unwrap();
        assert!(s.show(&b, &r.lead).is_err());
        assert!(
            s.authenticate(&Store::read_credential(&dir.path().join("writer-b")).unwrap())
                .is_err()
        );
        assert!(s.show(&a, &r.lead).unwrap().proposed_handoff.is_none());
    }
}
