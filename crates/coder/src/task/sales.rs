//! One host-private sales pipeline. Human ownership, consent, retention, and
//! suppression are separate from agent identity and outbound authority.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub mod agents;
pub mod claims;
pub mod earned;
pub mod email;
pub mod expenses;
pub mod floor;
pub mod intake;
pub mod jurisdictions;
pub mod meetings;
pub mod offboarding;
pub mod outbox;
pub mod partners;
pub mod paul;
pub mod privacy;
pub mod qualification;
pub mod referrals;
pub mod remote;
pub mod replies;
pub mod roles;
pub mod town;
pub mod training;
pub mod voice;

pub const SCHEMA: &str = "openagents.sales.pipeline.v1";
pub const COMMAND_SCHEMA: &str = "openagents.sales.pipeline-command.v1";
pub const LEAD_SCHEMA: &str = "openagents.sales.lead.v1";
pub const RECEIPT_SCHEMA: &str = "openagents.sales.receipt.v1";
const MAX_STATE: usize = 8 * 1024 * 1024;
const MAX_COMMAND: usize = 32 * 1024;
const MAX_LEADS: usize = 512;
const MAX_RECEIPTS: usize = 4096;
const FUNNEL_CLEANUP_BYTES: usize = 2048;
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
    /// Positive evidence for a reviewed non-US scope; the US baseline has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<jurisdictions::Evidence>,
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
    /// Canonical account source, separately checked by the pipeline owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquisition: Option<referrals::Introduction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub service_sales: BTreeMap<String, receipts::service_sale::Sale>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub partner_assignments: BTreeMap<String, partners::Assignment>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub funnel_journeys: BTreeMap<String, receipts::sales_funnel::Journey>,
    /// Verified offboarding per service sale, keyed by sale id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub offboarding: BTreeMap<String, offboarding::Record>,
    /// Private assigned-agent records; suppression erases them with this lead.
    #[serde(default)]
    pub agent_records: agents::LeadRecords,
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
    RecordServiceSale {
        admission: receipts::service_sale::Admission,
    },
    RecordAcquisition {
        accounts_directory: String,
    },
    ReconcileServicePayment {
        sale: String,
        payment: receipts::service_sale::PaymentInput,
    },
    ReconcileServiceFulfillment {
        sale: String,
        fulfillment: receipts::service_sale::FulfillmentInput,
    },
    /// The owner's checked cleanup report for one sale (#11013).
    RecordOffboarding {
        sale: String,
        report: offboarding::Report,
    },
    ProposePartner {
        proposal: partners::Proposal,
    },
    AdvancePartner {
        assignment: String,
        action: partners::Action,
    },
    RecordFunnelJourney {
        admission: receipts::sales_funnel::Admission,
    },
    RecordFunnelEvent {
        journey: String,
        event: receipts::sales_funnel::EventInput,
    },
    RecordConversionFailure {
        journey: String,
        failure: receipts::sales_funnel::FailureInput,
    },
    RevokeFunnelConsent {
        journey: String,
        reference: String,
    },
}

#[path = "sales/funnel.rs"]
pub mod funnel;
#[path = "sales/service.rs"]
pub mod service;
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
    #[serde(default)]
    claims: claims::State,
    #[serde(default)]
    agents: agents::Book,
    #[serde(default)]
    privacy: privacy::Book,
    #[serde(default)]
    email: email::Book,
    #[serde(default)]
    expenses: expenses::Book,
    #[serde(default)]
    outbox: outbox::Book,
    #[serde(default)]
    replies: replies::Book,
    #[serde(default)]
    training: training::Book,
    #[serde(default)]
    meetings: meetings::Book,
    #[serde(default, skip_serializing_if = "voice::Book::is_empty")]
    voice: voice::Book,
    #[serde(default)]
    qualification: qualification::Book,
    #[serde(default)]
    paul: paul::Book,
    #[serde(default)]
    roles: roles::Book,
    #[serde(default)]
    earned: earned::Book,
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
            claims: claims::State::default(),
            agents: agents::Book::default(),
            privacy: privacy::Book::default(),
            email: email::Book::default(),
            expenses: expenses::Book::default(),
            outbox: outbox::Book::default(),
            replies: replies::Book::default(),
            training: training::Book::default(),
            meetings: meetings::Book::default(),
            voice: voice::Book::default(),
            qualification: qualification::Book::default(),
            paul: paul::Book::default(),
            roles: roles::Book::default(),
            earned: earned::Book::default(),
        }
    }
}
pub struct Store {
    dir: PathBuf,
    lock: File,
    root_directory: File,
    sales_directory: File,
    native_keys: std::sync::Arc<dyn super::agent_key::KeyStore>,
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
        secret_screen::Screen::shapes()
            .check(s)
            .map_err(|_| "sales text refuses credential material".into())
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
    if let Some(evidence) = &details.scope {
        evidence.check()?;
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
        let mut state = if super::regular_or_absent(&path).map_err(|e| e.to_string())? {
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
        state.claims.check()?;
        state.agents.check(&state.leads)?;
        state.privacy.check()?;
        state.email.check()?;
        state.expenses.check()?;
        state.outbox.check()?;
        state.replies.check()?;
        state.training.check()?;
        state.meetings.check()?;
        state.voice.check()?;
        state.qualification.check()?;
        state.qualification.check_certificates(&state.agents)?;
        state.paul.check()?;
        state.roles.check()?;
        privacy::remember_retained(&mut state)?;
        if state.leads.values().any(|lead| lead.schema != LEAD_SCHEMA)
            || state
                .receipts
                .values()
                .any(|r| r.receipt.schema != RECEIPT_SCHEMA)
        {
            return Err("unsupported sales record schema".into());
        }
        for lead in state.leads.values() {
            if let Some(introduction) = &lead.acquisition {
                introduction.validate(&lead.details.account)?;
            }
            if lead.partner_assignments.len() > partners::MAX_ASSIGNMENTS {
                return Err("Private partner assignment count exceeds its bound.".into());
            }
            for (id, assignment) in &lead.partner_assignments {
                assignment.validate()?;
                if id != &assignment.proposal.id || assignment.pipeline_lead != lead.id {
                    return Err("Private partner assignment ownership disagrees.".into());
                }
            }
            if lead.funnel_journeys.len() > funnel::MAX_JOURNEYS {
                return Err("private funnel journey count exceeds bound".into());
            }
            for (id, journey) in &lead.funnel_journeys {
                journey.validate()?;
                if id != &journey.admission.id || journey.pipeline_lead != lead.id {
                    return Err("private funnel journey ownership disagrees".into());
                }
            }
            if lead.service_sales.len() > service::MAX_SALES {
                return Err("private service sale count exceeds bound".into());
            }
            for (id, sale) in &lead.service_sales {
                sale.validate()?;
                if id != &sale.admission.id || sale.pipeline_lead != lead.id {
                    return Err("private service sale ownership disagrees".into());
                }
            }
            for (id, record) in &lead.offboarding {
                record.validate()?;
                if lead
                    .service_sales
                    .get(id)
                    .is_none_or(|s| s.admission.sources.handoff.sha256 != record.handoff_sha256)
                {
                    return Err("private offboarding record ownership disagrees".into());
                }
            }
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
            root_directory: agents::native::directory(
                dir.parent().ok_or("host root is unavailable")?,
            )?,
            sales_directory: agents::native::directory(&dir)?,
            native_keys: super::agent_key::installed(),
            dir,
            lock,
            state,
            clock,
            poisoned: false,
        };
        store.refresh()?;
        let mut next = store.state.clone();
        if next.outbox.recover() {
            store.persist(next)?;
        }
        Ok(store)
    }
    fn persist(&mut self, mut next: State) -> Result<()> {
        if self.poisoned {
            return Err("sales store needs recovery".into());
        }
        self.sales_custody()?;
        super::verify_same_file(&self.dir.join("sales.lock"), &self.lock)
            .map_err(|e| e.to_string())?;
        if next.privacy.deleted.len() > self.state.privacy.deleted.len() {
            let root = self.dir.parent().ok_or("host root is unavailable")?;
            let stores = super::agent::Store::all(root);
            next.privacy.native_cleanup_truncated |= stores.len() > 64;
            for store in stores.into_iter().take(64) {
                next.privacy
                    .agent_cleanup
                    .insert(store.name().into(), false);
            }
        }
        privacy::retain_credentials(&mut next)?;
        let bytes = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
        let reserved = Self::funnel_count(&next)
            .saturating_add(next.agents.cleanup_count(&next.leads))
            .saturating_mul(FUNNEL_CLEANUP_BYTES);
        if bytes.len() > MAX_STATE.saturating_sub(reserved) {
            return Err("sales state exceeds bound".into());
        }
        if let Err(e) = super::replace_file(&self.dir, "state.json", &bytes) {
            self.poisoned = true;
            return Err(e.to_string());
        }
        self.state = next;
        Ok(())
    }
    fn funnel_count(state: &State) -> usize {
        state
            .leads
            .values()
            .map(|lead| lead.funnel_journeys.len())
            .sum()
    }
    fn ordinary_history_limit(&self, additional_journeys: usize) -> usize {
        (MAX_RECEIPTS - MAX_LEADS)
            .saturating_sub(Self::funnel_count(&self.state).saturating_add(additional_journeys))
            .saturating_sub(self.state.agents.cleanup_count(&self.state.leads))
    }
    fn suppression(state: &State, address: &str) -> Result<String> {
        Ok(digest(
            format!("{}:{}", state.salt, contact(address)?).as_bytes(),
        ))
    }
    fn remove(state: &mut State, lead: &str, now: u64, reference: &str) -> Result<()> {
        privacy::remove(state, lead, now, reference)?;
        claims::helpers::retire_lead(state, lead);
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
        let removed = state
            .leads
            .values()
            .filter(|record| {
                privacy::suppressed(state, record).unwrap_or(true)
                    || contact(&record.contact)
                        .map(|address| digest(format!("{salt}:{address}").as_bytes()) == key)
                        .unwrap_or(true)
            })
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        for id in &removed {
            state.meetings.retire(id);
            privacy::remove(state, id, now, reference)?;
        }
        state.leads.retain(|_, record| {
            !removed.contains(&record.id)
                && contact(&record.contact)
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
        let contact_history = outbox::remember_contact_history(&mut next)?;
        let outbox_expired = next.outbox.expire(now);
        let replies_expired = next.replies.expire(now);
        let expired = next
            .leads
            .values()
            .filter(|l| l.details.data.retain_until <= now || privacy::inactive(&next, l, now))
            .map(|l| l.id.clone())
            .collect::<Vec<_>>();
        for lead in &expired {
            if next.leads.contains_key(lead) {
                Self::remove(&mut next, lead, now, "retention expired")?;
            }
        }
        // Revoked/expired permission stops qualification and cancels proposed handoffs.
        let mut changed =
            !expired.is_empty() || contact_history || outbox_expired || replies_expired;
        changed |= next.expenses.recover(&self.dir, now)?;
        changed |= next.training.recover(&self.dir)?;
        changed |= next.qualification.recover(&self.dir)?;
        changed |= next.meetings.prune(now);
        let retired_sales = next
            .leads
            .values()
            .flat_map(|lead| lead.service_sales.values())
            .filter(|sale| sale.retain_until <= now)
            .cloned()
            .collect::<Vec<_>>();
        for sale in &retired_sales {
            privacy::retire_service(&mut next, sale)?;
        }
        for lead in next.leads.values_mut() {
            let unavailable = lead.details.permission.state != PermissionState::Granted
                || lead.details.permission.expires_at <= now;
            let mut partner_changed = false;
            for assignment in lead.partner_assignments.values_mut() {
                let scope_changed = assignment.account != lead.details.account
                    || assignment.permission_reference != lead.details.permission.reference
                    || assignment.data.permitted_use != lead.details.data.permitted_use
                    || assignment
                        .data
                        .recipients
                        .iter()
                        .any(|r| !lead.details.data.recipients.contains(r));
                partner_changed |= assignment.retire(now, unavailable || scope_changed);
            }
            if partner_changed {
                lead.revision = lead
                    .revision
                    .checked_add(1)
                    .ok_or("Lead revision overflow.")?;
                lead.updated_at = now;
                changed = true;
            }
            let before = lead.funnel_journeys.len();
            lead.funnel_journeys
                .retain(|_, journey| journey.retain_until > now);
            changed |= before != lead.funnel_journeys.len();
            let before = lead.service_sales.len();
            lead.service_sales.retain(|_, sale| sale.retain_until > now);
            changed |= before != lead.service_sales.len();
            let sales = &lead.service_sales;
            let before = lead.offboarding.len();
            lead.offboarding.retain(|sale, _| sales.contains_key(sale));
            changed |= before != lead.offboarding.len();
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
        self.cleanup_sales_copies()?;
        Ok(())
    }
    fn check(&self, access: &Access) -> Result<Role> {
        self.sales_custody()?;
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
    fn sales_custody(&self) -> Result<()> {
        agents::native::same_directory(
            self.dir.parent().ok_or("host root is unavailable")?,
            &self.root_directory,
        )?;
        agents::native::same_directory(&self.dir, &self.sales_directory)?;
        super::verify_same_file(&self.dir.join("sales.lock"), &self.lock)
            .map_err(|_| "sales store custody changed".into())
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
            let now = (self.clock)();
            for assignment in lead.partner_assignments.values_mut() {
                if assignment.proposal.recipient_human == human
                    || assignment
                        .handoff
                        .as_ref()
                        .is_some_and(|h| h.target == human)
                {
                    if assignment.retire(now, true) {
                        lead.revision = lead
                            .revision
                            .checked_add(1)
                            .ok_or("Lead revision overflow.")?;
                    }
                }
            }
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
        Ok(self.visible_lead(access, found))
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
            .map(|lead| self.visible_lead(access, lead))
            .collect())
    }
    fn visible_lead(&self, access: &Access, lead: &Lead) -> Lead {
        let mut visible = lead.clone();
        if self.check(access).ok() != Some(Role::Owner) {
            visible.agent_records = agents::LeadRecords::default();
        }
        visible.funnel_journeys.retain(|_, journey| {
            (self.clock)() < journey.retain_until && journey.recipient(access.principal())
        });
        visible.service_sales.retain(|_, sale| {
            (self.clock)() < sale.retain_until
                && sale
                    .admitted_recipients
                    .contains(&format!("human:{}", access.principal()))
        });
        let sales = &visible.service_sales;
        visible
            .offboarding
            .retain(|sale, _| sales.contains_key(sale));
        visible.partner_assignments.retain(|_, assignment| {
            (self.clock)() < assignment.data.retain_until
                && assignment
                    .data
                    .recipients
                    .contains(&format!("human:{}", access.principal()))
                && self.partner_visible(access, assignment)
        });
        visible
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
        let legacy = self
            .state
            .suppressions
            .contains_key(&Self::suppression(&self.state, address)?);
        Ok(legacy || privacy::address_suppressed(&self.state, address)?)
    }
    pub fn apply(&mut self, access: &Access, bytes: &[u8]) -> Result<Receipt> {
        self.apply_with_evidence_root(access, bytes, None)
    }
    pub fn apply_with_evidence_root(
        &mut self,
        access: &Access,
        bytes: &[u8],
        evidence_root: Option<&Path>,
    ) -> Result<Receipt> {
        self.refresh()?;
        let role = self.check(access)?;
        if bytes.len() > MAX_COMMAND {
            return Err("sales command exceeds bound".into());
        }
        privacy::check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "sales command is not UTF-8")?,
        )?;
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
        // Reserve contact cleanup plus one retry-safe withdrawal per current
        // journey. Enrollment and intake cannot consume those privacy slots.
        let cleanup = matches!(
            c.operation,
            Operation::Delete { .. } | Operation::Suppress { .. }
        );
        let history_limit = if cleanup {
            MAX_RECEIPTS
        } else if matches!(c.operation, Operation::RevokeFunnelConsent { .. }) {
            MAX_RECEIPTS - self.state.leads.len()
        } else {
            self.ordinary_history_limit(usize::from(matches!(
                c.operation,
                Operation::RecordFunnelJourney { .. }
            )))
        };
        if self.state.receipts.len() >= history_limit || self.state.audit.len() >= history_limit {
            return Err(if cleanup {
                "sales cleanup history bound reached"
            } else {
                "sales ordinary history bound reached; privacy cleanup remains available"
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
            privacy::check_identity(&next, &input.contact, &input.details.account)?;
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
                    acquisition: None,
                    service_sales: BTreeMap::new(),
                    offboarding: BTreeMap::new(),
                    partner_assignments: BTreeMap::new(),
                    funnel_journeys: BTreeMap::new(),
                    agent_records: agents::LeadRecords::default(),
                },
            );
            let added = next.leads.get(&lead_id).unwrap().clone();
            privacy::remember(&mut next, &added)?;
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
                Operation::RecordAcquisition { accounts_directory } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let introduction =
                        self.admit_acquisition(access, found, accounts_directory, now)?;
                    reference = introduction.accounts_revision.clone();
                    next.leads.get_mut(&lead_id).unwrap().acquisition = Some(introduction);
                    outcome = "acquisition_recorded";
                }
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
                    if found
                        .acquisition
                        .as_ref()
                        .is_some_and(|source| source.source.account != details.account)
                    {
                        return Err(
                            "account changes cannot rewrite an assisted introduction".into()
                        );
                    }
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
                    if details.permission.state == PermissionState::Revoked {
                        lead.funnel_journeys.clear();
                    }
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
                Operation::RecordFunnelJourney { admission } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let journey = self.admit_funnel(
                        access,
                        found,
                        admission,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .funnel_journeys
                        .insert(admission.id.clone(), journey);
                    outcome = "funnel_journey_recorded";
                    reference = admission.consent.evidence.sha256.clone();
                }
                Operation::RecordFunnelEvent { journey, event } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let recorded = self.record_funnel_event(
                        access,
                        found,
                        journey,
                        event,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .funnel_journeys
                        .get_mut(journey)
                        .unwrap()
                        .events
                        .push(recorded);
                    outcome = "funnel_event_recorded";
                    reference = event.id.clone();
                }
                Operation::RecordConversionFailure { journey, failure } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let recorded = self.record_conversion_failure(
                        access,
                        found,
                        journey,
                        failure,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .funnel_journeys
                        .get_mut(journey)
                        .unwrap()
                        .failures
                        .push(recorded);
                    outcome = "conversion_failure_recorded";
                    reference = failure.evidence.sha256.clone();
                }
                Operation::RevokeFunnelConsent {
                    journey,
                    reference: r,
                } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    text(r, 256)?;
                    if next
                        .leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .funnel_journeys
                        .remove(journey)
                        .is_none()
                    {
                        return Err("funnel journey is unavailable".into());
                    }
                    outcome = "funnel_consent_revoked";
                    reference = r.clone();
                }
                Operation::RecordServiceSale { admission } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let recorded = self.admit_service(
                        access,
                        found,
                        admission,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .service_sales
                        .insert(admission.id.clone(), recorded);
                    outcome = "service_sale_recorded";
                    reference = admission.sources.agreement.sha256.clone();
                }
                Operation::ReconcileServicePayment { sale, payment } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let verified = self.reconcile_service(
                        access,
                        found,
                        sale,
                        payment,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .service_sales
                        .get_mut(sale)
                        .unwrap()
                        .payments
                        .push(verified);
                    outcome = "service_payment_reconciled";
                    reference = payment.evidence.sha256.clone();
                }
                Operation::ReconcileServiceFulfillment { sale, fulfillment } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let verified = self.reconcile_fulfillment(
                        access,
                        found,
                        sale,
                        fulfillment,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .service_sales
                        .get_mut(sale)
                        .unwrap()
                        .fulfillment_reconciliations
                        .push(verified);
                    outcome = "service_fulfillment_reconciled";
                    reference = fulfillment.bill.sha256.clone();
                }
                Operation::RecordOffboarding { sale, report } => {
                    self.admin(access)?;
                    self.readable(access, found)?;
                    let record = self.record_offboarding(
                        access,
                        found,
                        sale,
                        report,
                        &input_digest,
                        evidence_root,
                        now,
                    )?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .offboarding
                        .insert(sale.clone(), record);
                    outcome = "offboarding_recorded";
                    reference = report.handoff_sha256.clone();
                }
                Operation::ProposePartner { proposal } => {
                    let assignment =
                        self.propose_partner(access, found, proposal, evidence_root, now)?;
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .partner_assignments
                        .insert(proposal.id.clone(), assignment);
                    outcome = "partner_proposed";
                    reference = proposal.approval.sha256.clone();
                }
                Operation::AdvancePartner { assignment, action } => {
                    let updated = self.advance_partner(
                        access,
                        found,
                        assignment,
                        action,
                        evidence_root,
                        now,
                    )?;
                    reference = updated.events.last().unwrap().evidence.sha256.clone();
                    next.leads
                        .get_mut(&lead_id)
                        .unwrap()
                        .partner_assignments
                        .insert(assignment.clone(), updated);
                    outcome = "partner_advanced";
                }
                Operation::Create { .. } => unreachable!(),
            }
            if let Some(lead) = next.leads.get_mut(&lead_id) {
                lead.revision = revision;
                lead.updated_at = now;
            }
        }
        if let Some(lead) = next.leads.get(&lead_id).cloned() {
            privacy::remember(&mut next, &lead)?;
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
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        self.write_sales_copy(access, &[lead.into()], path, &bytes)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    mod referral_tests {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/task/sales/referrals/tests.rs"
        ));
    }
    mod funnel_tests {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/task/sales/funnel_tests.rs"
        ));
    }
    pub(crate) mod service_fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../receipts/tests/support/service_sale.rs"
        ));
    }
    use super::*;
    use tempfile::TempDir;
    pub(crate) fn now() -> u64 {
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
            scope: None,
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
    pub(crate) fn command(id: &str, lead: Option<&str>, revision: u64, operation: Operation) -> Vec<u8> {
        serde_json::to_vec(&Command {
            schema: COMMAND_SCHEMA.into(),
            id: id.into(),
            lead: lead.map(Into::into),
            expected_revision: revision,
            operation,
        })
        .unwrap()
    }
    pub(crate) fn create(id: &str) -> Vec<u8> {
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
    pub(crate) fn fixture() -> (TempDir, Store, Access, PathBuf) {
        let dir = TempDir::new().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        let cred = dir.path().join("operator");
        store.initialize("operator", &cred).unwrap();
        let a = store
            .authenticate(&Store::read_credential(&cred).unwrap())
            .unwrap();
        (dir, store, a, cred)
    }
    pub(crate) fn grant(dir: &TempDir, store: &mut Store, admin: &Access, name: &str, role: Role) -> Access {
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
    pub(crate) fn service_setup() -> (
        TempDir,
        Store,
        Access,
        String,
        receipts::service_sale::Admission,
    ) {
        use service_fixture::{Comparison, retain};
        let (dir, mut store, owner, _) = fixture();
        let lead = store.apply(&owner, &create("service-lead")).unwrap().lead;
        let root = dir.path().join("evidence");
        std::fs::create_dir(&root).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let comparison = Comparison {
            manifest: retain(
                &root,
                "comparison.json",
                b"synthetic manifest checked by Gym when reported",
            ),
            report: retain(
                &root,
                "comparison-report.json",
                b"synthetic report checked by Gym when reported",
            ),
            candidate: retain(
                &root,
                "candidate.patch",
                b"synthetic exact accepted candidate",
            ),
            check: retain(&root, "independent-check", b"synthetic independent check"),
            decision: retain(
                &root,
                "buyer-decision",
                b"synthetic buyer accepted exact result",
            ),
            frozen_checks: vec![retain(
                &root,
                "frozen-command",
                b"synthetic frozen check command",
            )],
        };
        let admission = service_fixture::admission(
            &root,
            now(),
            &lead,
            "synthetic-account",
            "offer-v1",
            comparison,
        );
        (dir, store, owner, lead, admission)
    }
    pub(crate) fn service_apply(
        store: &mut Store,
        access: &Access,
        root: &Path,
        lead: &str,
        id: &str,
        operation: Operation,
    ) -> Receipt {
        let revision = store.show(access, lead).unwrap().revision;
        store
            .apply_with_evidence_root(
                access,
                &command(id, Some(lead), revision, operation),
                Some(root),
            )
            .unwrap()
    }
    fn service_payment(
        root: &Path,
        name: &str,
        disposition: receipts::service_sale::Disposition,
        refund: Option<u64>,
    ) -> receipts::service_sale::PaymentInput {
        use receipts::service_sale::{Disposition, PaymentInput};
        PaymentInput {
            disposition,
            external_reference: if disposition == Disposition::Unknown {
                None
            } else {
                Some("synthetic-external-payment".into())
            },
            paid_minor: if matches!(disposition, Disposition::Paid | Disposition::Reversed) {
                Some(25000)
            } else {
                None
            },
            reversed_minor: refund,
            evidence: service_fixture::retain(root, name, name.as_bytes()),
        }
    }
    #[test]
    fn service_owner_verified_history_replays_once_after_restart_and_exports_privately() {
        use receipts::service_sale::Disposition;
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut store, owner, lead, admission) = service_setup();
        let root = dir.path().join("evidence");
        let bytes = command(
            "admit-sale",
            Some(&lead),
            1,
            Operation::RecordServiceSale { admission },
        );
        let first = store
            .apply_with_evidence_root(&owner, &bytes, Some(&root))
            .unwrap();
        assert_eq!(
            store
                .apply_with_evidence_root(&owner, &bytes, None)
                .unwrap(),
            first
        );
        for (id, disposition, refund) in [
            ("unknown", Disposition::Unknown, None),
            ("paid", Disposition::Paid, None),
            ("refund", Disposition::Reversed, Some(101)),
            ("restored", Disposition::Paid, Some(51)),
            ("dispute", Disposition::Disputed, None),
        ] {
            let payment = service_payment(&root, id, disposition, refund);
            service_apply(
                &mut store,
                &owner,
                &root,
                &lead,
                id,
                Operation::ReconcileServicePayment {
                    sale: "synthetic-sale".into(),
                    payment,
                },
            );
        }
        let sale = store.service_show(&owner, &lead, "synthetic-sale").unwrap();
        let summary = sale.summary().unwrap();
        assert_eq!(
            (
                summary.paid_minor,
                summary.refunded_minor,
                summary.refund_reversals_minor
            ),
            (25000, 101, 50)
        );
        assert!(summary.unresolved);
        assert_eq!(sale.payments.len(), 5);
        let output = dir.path().join("private-service-export.json");
        let hash = store
            .service_export(&owner, &lead, "synthetic-sale", &output)
            .unwrap();
        let exported = std::fs::read(&output).unwrap();
        assert_eq!(hash, digest(&exported));
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let text = String::from_utf8(exported).unwrap();
        assert!(!text.contains("prospect@"));
        assert!(!text.contains("private-baseline-reference"));
        assert!(
            store
                .service_export(&owner, &lead, "synthetic-sale", &output)
                .is_err()
        );
        drop(store);
        let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
        let owner = store
            .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
            .unwrap();
        assert_eq!(
            store
                .apply_with_evidence_root(&owner, &bytes, None)
                .unwrap(),
            first
        );
        assert_eq!(
            store.service_show(&owner, &lead, "synthetic-sale").unwrap(),
            sale
        );
        let mut changed: Command = serde_json::from_slice(&bytes).unwrap();
        if let Operation::RecordServiceSale { admission } = &mut changed.operation {
            admission.invoice.amount_minor += 1;
        }
        assert!(
            store
                .apply_with_evidence_root(
                    &owner,
                    &serde_json::to_vec(&changed).unwrap(),
                    Some(&root)
                )
                .unwrap_err()
                .contains("idempotency")
        );
    }
    #[test]
    fn service_old_scope_stays_pinned_and_new_readers_cannot_receive_historical_invoices() {
        let (dir, mut store, owner, lead, admission) = service_setup();
        let root = dir.path().join("evidence");
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "admit",
            Operation::RecordServiceSale { admission },
        );
        let reader = grant(&dir, &mut store, &owner, "new-reader", Role::Reader);
        let mut updated = details();
        updated.account = "different-current-account".into();
        updated.readers.push("new-reader".into());
        updated.data.recipients.push("human:new-reader".into());
        updated.permission.reference = "new reader and account consent".into();
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "update",
            Operation::Update { details: updated },
        );
        assert_eq!(
            store
                .service_show(&owner, &lead, "synthetic-sale")
                .unwrap()
                .account,
            "synthetic-account"
        );
        assert!(store.show(&reader, &lead).unwrap().service_sales.is_empty());
        assert!(
            store.list(&reader, None, 10).unwrap()[0]
                .service_sales
                .is_empty()
        );
        assert!(
            store
                .service_show(&reader, &lead, "synthetic-sale")
                .is_err()
        );
        let output = dir.path().join("reader-lead.json");
        store.export(&reader, &lead, &output).unwrap();
        assert!(
            !String::from_utf8(std::fs::read(output).unwrap())
                .unwrap()
                .contains("synthetic-invoice")
        );
        store.revoke(&owner, "new-reader").unwrap();
        assert!(store.show(&reader, &lead).is_err());
    }
    #[test]
    fn service_write_authority_source_linkage_partial_claim_and_duplicate_invoice_refuse() {
        use receipts::service_sale::Disposition;
        let (dir, mut store, owner, lead, admission) = service_setup();
        let root = dir.path().join("evidence");
        let writer = grant(&dir, &mut store, &owner, "writer-a", Role::Writer);
        let bytes = command(
            "writer-sale",
            Some(&lead),
            1,
            Operation::RecordServiceSale {
                admission: admission.clone(),
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&writer, &bytes, Some(&root))
                .is_err()
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, None)
                .is_err()
        );
        let mut bad = admission.clone();
        bad.invoice.amount_minor += 1;
        let bytes = command(
            "bad-price",
            Some(&lead),
            1,
            Operation::RecordServiceSale { admission: bad },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .is_err()
        );
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "admit",
            Operation::RecordServiceSale {
                admission: admission.clone(),
            },
        );
        let mut partial = service_payment(&root, "partial", Disposition::Paid, None);
        partial.paid_minor = Some(1);
        let bytes = command(
            "partial",
            Some(&lead),
            2,
            Operation::ReconcileServicePayment {
                sale: admission.id.clone(),
                payment: partial,
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .is_err()
        );
        let mut duplicate = admission;
        duplicate.id = "other-sale".into();
        let bytes = command(
            "duplicate",
            Some(&lead),
            2,
            Operation::RecordServiceSale {
                admission: duplicate,
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .unwrap_err()
                .contains("duplicate")
        );
        std::fs::write(root.join("service-support-ack"), b"changed acknowledgment").unwrap();
        let payment = service_payment(&root, "paid", Disposition::Paid, None);
        let bytes = command(
            "changed-support",
            Some(&lead),
            2,
            Operation::ReconcileServicePayment {
                sale: "synthetic-sale".into(),
                payment,
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .is_err()
        );
        assert!(
            store
                .service_show(&owner, &lead, "synthetic-sale")
                .unwrap()
                .payments
                .is_empty()
        );
    }
    #[test]
    fn service_original_retention_expires_even_after_lead_extension_and_delete_purges() {
        let (dir, mut store, owner, lead, admission) = service_setup();
        let root = dir.path().join("evidence");
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "admit",
            Operation::RecordServiceSale { admission },
        );
        let mut updated = details();
        updated.data.retain_until = 2500;
        updated.permission.reference = "new retention consent".into();
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "extend",
            Operation::Update { details: updated },
        );
        drop(store);
        let mut store = Store::open_with_clock(&dir.path().join("host"), later).unwrap();
        let owner = store
            .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
            .unwrap();
        assert!(store.show(&owner, &lead).unwrap().service_sales.is_empty());
        assert!(
            !String::from_utf8(std::fs::read(dir.path().join("host/sales/state.json")).unwrap())
                .unwrap()
                .contains("synthetic-sale")
        );
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "delete",
            Operation::Delete {
                reference: "synthetic deletion request".into(),
            },
        );
        assert!(store.show(&owner, &lead).is_err());
    }
    #[test]
    fn service_fulfillment_reconciliation_preserves_price_and_requires_the_separate_payment_trigger()
     {
        use receipts::service_sale::Disposition;
        let (dir, mut store, owner, lead, mut admission) = service_setup();
        let root = dir.path().join("evidence");
        admission.fulfillment = Some(service_fixture::fulfillment(&root, now(), &admission));
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "admit",
            Operation::RecordServiceSale { admission },
        );
        let fulfillment = service_fixture::fulfillment_input(&root, now(), true);
        let bytes = command(
            "premature-fulfillment",
            Some(&lead),
            2,
            Operation::ReconcileServiceFulfillment {
                sale: "synthetic-sale".into(),
                fulfillment: fulfillment.clone(),
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .unwrap_err()
                .contains("trigger")
        );
        let payment = service_payment(&root, "collected", Disposition::Paid, None);
        service_apply(
            &mut store,
            &owner,
            &root,
            &lead,
            "collected",
            Operation::ReconcileServicePayment {
                sale: "synthetic-sale".into(),
                payment,
            },
        );
        let bytes = command(
            "paid-fulfillment",
            Some(&lead),
            3,
            Operation::ReconcileServiceFulfillment {
                sale: "synthetic-sale".into(),
                fulfillment,
            },
        );
        let receipt = store
            .apply_with_evidence_root(&owner, &bytes, Some(&root))
            .unwrap();
        assert_eq!(
            store
                .apply_with_evidence_root(&owner, &bytes, None)
                .unwrap(),
            receipt
        );
        let sale = store.service_show(&owner, &lead, "synthetic-sale").unwrap();
        assert_eq!(sale.fulfillment_reconciliations.len(), 1);
        assert_eq!(
            sale.effective_fulfillment().unwrap().unwrap().amount_minor,
            5000
        );
        assert!(
            sale.admission
                .fulfillment
                .as_ref()
                .unwrap()
                .payment
                .is_none()
        );
        let mut bill: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("fulfillment-bill.json")).unwrap())
                .unwrap();
        bill["amount_minor"] = serde_json::json!(6000);
        let bill = service_fixture::doc(&root, "different-fulfillment-bill.json", bill);
        let input = receipts::service_sale::FulfillmentInput {
            bill,
            payment: None,
        };
        let bytes = command(
            "change-fulfillment-price",
            Some(&lead),
            4,
            Operation::ReconcileServiceFulfillment {
                sale: "synthetic-sale".into(),
                fulfillment: input,
            },
        );
        assert!(
            store
                .apply_with_evidence_root(&owner, &bytes, Some(&root))
                .is_err()
        );
    }
}
