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

use super::claims::{Decision, RegisterEntry};
use super::{
    Audit, CustomerDecision, DataBoundary, Lead, PermissionState, Receipt, Result, Role, Stage,
    Store, digest, outbox,
};
use receipts::sales_funnel::Journey;
use receipts::service_sale::Sale;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const CONFIG_SCHEMA: &str = "openagents.sales.remote-bindings.v1";
pub const REQUEST_SCHEMA: &str = "openagents.sales.remote-request.v1";
pub const RESPONSE_SCHEMA: &str = "openagents.sales.remote-response.v1";
/// The receipt an outbox decision or stop answers with; its revision is the
/// outbox book's revision after the command.
pub const OUTBOX_RECEIPT_SCHEMA: &str = "openagents.sales.remote-outbox-receipt.v1";
const JOURNAL_SCHEMA: &str = "openagents.sales.remote-journal.v1";
/// One command plus its envelope.
pub const BODY_MAX: usize = super::MAX_COMMAND + 8 * 1024;
const CONFIG_MAX: u64 = 64 * 1024;
const JOURNAL_MAX: usize = 256;
const JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const BINDINGS_MAX: usize = 64;
const AUDIT_MAX: usize = 200;
const HISTORY_MAX: usize = 300;
const WEEKLY_MAX: u64 = 1024 * 1024;

#[path = "remote/floor.rs"]
mod floor;
pub use floor::{
    Attachment, Certification, Escalation, Incident, MeetingRow, Member, Outbox, OutboxRow, Paul,
    PaulRow, ReplyRow, Reservation, TIMEZONE,
};
pub use floor::{
    BOARD_SCHEMA, BOARD_TTL_SECONDS, Board, FLOOR_SCHEMA, Floor, PROPOSAL_SCHEMA, Proposal,
};

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
    /// Decide one exact outbox subject at its original outbox revision.
    OutboxDecide,
    /// Stop outbound dispatch: pause the outbox controller (REV-62).
    OutboxStop,
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

    fn of_outbox(operation: &outbox::Operation) -> Option<Self> {
        match operation {
            outbox::Operation::Decide { .. } => Some(Self::OutboxDecide),
            outbox::Operation::Pause {
                incident: outbox::IncidentKind::OwnerStop,
                ..
            } => Some(Self::OutboxStop),
            _ => None,
        }
    }
}

/// Which canonical book a journaled command belongs to.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Ledger {
    #[default]
    Pipeline,
    Outbox,
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
    /// The private service evidence root. Delivery documents are reread here
    /// by their retained digest; none is configured means none is shown.
    #[serde(default)]
    evidence: Option<PathBuf>,
    /// The owner's explicit weekly review sources, rebuilt on each read.
    #[serde(default)]
    weekly: Option<WeeklySources>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeeklySources {
    /// The private weekly manifest.
    input: PathBuf,
    /// The private evidence root the manifest names.
    evidence_root: PathBuf,
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
    /// Admit owner-only floor supervision reads (floor, exact outbox
    /// subjects, private board). The principal must still be the owner.
    #[serde(default)]
    supervise: bool,
    /// A private crew hiring book naming confirmed hires, read only.
    #[serde(default)]
    hires: Option<PathBuf>,
    /// The sales mailbox key the owner's approval rechecks; without it an
    /// approval refuses and only rejection is possible.
    #[serde(default)]
    mailbox_key: Option<PathBuf>,
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
    /// Contact-free records with their retained service sales and consented
    /// journeys, each fenced by the store's retention and recipients.
    Records { after: Option<String>, limit: usize },
    /// One retained sale's delivery handoff, reread by its exact digest.
    Delivery { lead: String, sale: String },
    /// The owner audit scoped to one readable record; no other record's
    /// entries are returned.
    Audit { lead: String },
    /// The owner's claim register and review history.
    Claims,
    /// The owner's weekly review, rebuilt from current sources.
    Weekly,
    /// Partner assignments scoped to the bound principal (WEB-16): full
    /// accepted records only for their owner, accepted recipient, or accepted
    /// support human; a pending invitation only for its named recipient.
    Partners {
        after: Option<PartnerCursor>,
        limit: usize,
    },
    /// One scoped partner assignment, for a scoped export.
    Partner { lead: String, assignment: String },
    /// Arthur's partner brief or Vanna's attribution view, projected by the
    /// owner from current records. The sales owner principal only.
    Desk { desk: super::roles::Desk },
    /// The owner's earned-sale ledger over original settlements and
    /// reconciled delivery. Reading it rings no bell. Owner only.
    Earned,
    /// Owner-only floor supervision projection.
    Floor,
    /// One exact outbox subject for review.
    Proposal { proposal: String },
    /// The private Agora board, current for three seconds.
    Board,
    /// One exact outbox command (decide or stop); its `id` must equal `request`.
    Outbox { request: String, command: String },
}

/// One record's commercial projection without contact, source text, or the
/// assigned-agent records.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordView {
    pub summary: Summary,
    pub account: String,
    pub workflow: String,
    pub source_at: u64,
    /// Whether public intake created this record.
    pub intake: bool,
    /// Whether a canonical acquisition source is recorded.
    pub acquisition: bool,
    pub customer_decision: Option<CustomerDecision>,
    pub data: DataBoundary,
    pub services: Vec<Sale>,
    pub journeys: Vec<Journey>,
    /// Verified offboarding for the shown sales that have any recorded.
    #[serde(default)]
    pub offboarding: Vec<super::offboarding::View>,
}

impl RecordView {
    fn of(lead: &Lead, now: u64) -> Self {
        Self {
            offboarding: lead
                .offboarding
                .iter()
                .filter(|(sale, _)| lead.service_sales.contains_key(*sale))
                .map(|(sale, record)| record.view(sale, now))
                .collect(),
            summary: Summary::of(lead),
            account: lead.details.account.clone(),
            workflow: lead.details.workflow.clone(),
            source_at: lead.source_at,
            intake: lead.intake.is_some(),
            acquisition: lead.acquisition.is_some(),
            customer_decision: lead.details.customer_decision.clone(),
            data: lead.details.data.clone(),
            services: lead.service_sales.values().cloned().collect(),
            journeys: lead.funnel_journeys.values().cloned().collect(),
        }
    }
}

/// A delivery handoff's operating fields. Paths, contact references, and
/// document bodies stay with the owner.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub id: String,
    pub version: String,
    pub delivered_at: Option<u64>,
    pub dependencies: Vec<Dependency>,
    pub known_limits: Vec<String>,
    pub retained_artifacts: Vec<Retained>,
    pub support: Support,
    pub cleanup_plan: Vec<Cleanup>,
    pub reuse_default: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version_or_digest: String,
    pub scope: String,
    pub readiness: String,
    pub unavailable_reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Retained {
    pub id: String,
    pub controller: String,
    pub retain_until: Option<u64>,
    pub purpose: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Support {
    pub responsible_human: String,
    pub business_hours: String,
    pub response_boundary: String,
    pub included_work: String,
    pub out_of_scope_route: String,
    pub ends_at: Option<u64>,
}

/// One planned offboarding item. The handoff plans it; only a separately
/// verified cleanup report can show it done.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cleanup {
    pub id: String,
    pub class: String,
    pub responsible_human: String,
    pub due_at: Option<u64>,
}

impl Handoff {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let doc: Value = serde_json::from_slice(bytes).ok()?;
        if doc["schema"] != "openagents.sales.delivery-handoff.v1" {
            return None;
        }
        let list = |field: &str| -> Vec<Value> {
            doc[field]
                .as_array()
                .map(|a| a.iter().take(32).cloned().collect())
                .unwrap_or_default()
        };
        let support = &doc["support"];
        Some(Self {
            id: text(&doc["id"]),
            version: text(&doc["version"]),
            delivered_at: doc["delivered_at"].as_u64(),
            dependencies: list("dependencies")
                .iter()
                .map(|d| Dependency {
                    id: text(&d["id"]),
                    version_or_digest: text(&d["version_or_digest"]),
                    scope: text(&d["scope"]),
                    readiness: text(&d["readiness"]),
                    unavailable_reason: text(&d["unavailable_reason"]),
                })
                .collect(),
            known_limits: list("known_limits").iter().map(text).collect(),
            retained_artifacts: list("retained_artifacts")
                .iter()
                .map(|r| Retained {
                    id: text(&r["id"]),
                    controller: text(&r["controller"]),
                    retain_until: r["retain_until"].as_u64(),
                    purpose: text(&r["purpose"]),
                })
                .collect(),
            support: Support {
                responsible_human: text(&support["responsible_human"]),
                business_hours: text(&support["business_hours"]),
                response_boundary: text(&support["response_boundary"]),
                included_work: text(&support["included_work"]),
                out_of_scope_route: text(&support["out_of_scope_route"]),
                ends_at: support["ends_at"].as_u64(),
            },
            cleanup_plan: list("cleanup_plan")
                .iter()
                .map(|c| Cleanup {
                    id: text(&c["id"]),
                    class: text(&c["class"]),
                    responsible_human: text(&c["responsible_human"]),
                    due_at: c["due_at"].as_u64(),
                })
                .collect(),
            reuse_default: text(&doc["reuse_default"]),
        })
    }
}

/// A bounded single-line string, or empty when absent or not a string.
fn text(value: &Value) -> String {
    value
        .as_str()
        .map(|s| {
            s.chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// One retained sale's delivery document as the owner rereads it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub sale: String,
    pub handoff_sha256: String,
    pub handoff: Option<Handoff>,
    /// Why no handoff is shown: `not_configured` or `unreadable`.
    pub unavailable: Option<String>,
    /// The verified offboarding record, when the owner has recorded one.
    /// It is read from the owner's store, so it shows even when the
    /// handoff document itself is unavailable here.
    #[serde(default)]
    pub offboarding: Option<super::offboarding::View>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub register: Vec<RegisterEntry>,
    pub history: Vec<Decision>,
}

/// The owner's weekly review rebuilt from current, custody-checked sources.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weekly {
    pub manifest_digest: String,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub gaps: Vec<String>,
    pub limitations: Vec<String>,
    pub contribution_scope: String,
    pub commercial_activation_attested: bool,
    pub finance_included: bool,
    /// Cohort rows exactly as the review computes them.
    pub cohorts: Vec<Value>,
    /// Journey rows, including failed and unknown history.
    pub journeys: Vec<Value>,
}

/// An exclusive (lead, assignment) position in a partner listing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartnerCursor {
    pub lead: String,
    pub assignment: String,
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
    /// Floor supervision reads are admitted for this binding.
    pub supervise: bool,
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
    #[serde(default)]
    ledger: Ledger,
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
                supervise: binding.supervise && role == Role::Owner,
            })),
            Op::Floor => {
                supervising(binding, role)?;
                let floor = store
                    .remote_floor(&access, binding.hires.as_deref())
                    .map_err(|_| Code::Unavailable)?;
                serde_json::to_value(floor).map_err(|_| Code::Unavailable)
            }
            Op::Proposal { proposal } => {
                supervising(binding, role)?;
                super::id(&proposal).map_err(|_| Code::InvalidRequest)?;
                // Absent, minimized, and unreadable subjects answer alike.
                let proposal = store
                    .remote_proposal(&access, &proposal)
                    .map_err(|_| Code::AccessDenied)?;
                serde_json::to_value(proposal).map_err(|_| Code::Unavailable)
            }
            Op::Board => {
                supervising(binding, role)?;
                let board = store.remote_board(&access).map_err(|_| Code::Unavailable)?;
                serde_json::to_value(board).map_err(|_| Code::Unavailable)
            }
            Op::Outbox { request, command } => {
                if role != Role::Owner {
                    return Err(Code::AccessDenied);
                }
                let receipt =
                    self.outbox(&config, binding, &mut store, &access, &request, command)?;
                Ok(json!(receipt))
            }
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
            Op::Partners { after, limit } => {
                if after
                    .as_ref()
                    .is_some_and(|c| !record_id(&c.lead) || !record_id(&c.assignment))
                {
                    return Err(Code::InvalidRequest);
                }
                let cursor = after
                    .as_ref()
                    .map(|c| (c.lead.as_str(), c.assignment.as_str()));
                let views = store
                    .partner_list(&access, cursor, limit)
                    .map_err(|_| Code::InvalidRequest)?;
                serde_json::to_value(views).map_err(|_| Code::Unavailable)
            }
            Op::Partner { lead, assignment } => {
                if !record_id(&lead) || !record_id(&assignment) {
                    return Err(Code::InvalidRequest);
                }
                // Absent and refused assignments answer alike.
                let view = store
                    .partner_one(&access, &lead, &assignment)
                    .map_err(|_| Code::AccessDenied)?;
                serde_json::to_value(view).map_err(|_| Code::Unavailable)
            }
            Op::Desk { desk } => {
                if role != Role::Owner {
                    return Err(Code::AccessDenied);
                }
                match desk {
                    super::roles::Desk::Arthur => store
                        .partner_brief(&access)
                        .map_err(|_| Code::Refused)
                        .and_then(|v| serde_json::to_value(v).map_err(|_| Code::Unavailable)),
                    super::roles::Desk::Vanna => store
                        .attribution_view(&access)
                        .map_err(|_| Code::Refused)
                        .and_then(|v| serde_json::to_value(v).map_err(|_| Code::Unavailable)),
                }
            }
            Op::Earned => {
                if role != Role::Owner {
                    return Err(Code::AccessDenied);
                }
                let ledger = store.earned_ledger(&access).map_err(|_| Code::Refused)?;
                serde_json::to_value(ledger).map_err(|_| Code::Unavailable)
            }
            Op::Apply { request, command } => {
                let receipt =
                    self.apply(&config, binding, &mut store, &access, &request, command)?;
                Ok(json!(receipt))
            }
            Op::Records { after, limit } => {
                if after.as_deref().is_some_and(|a| !record_id(a)) {
                    return Err(Code::InvalidRequest);
                }
                let leads = store
                    .list(&access, after.as_deref(), limit)
                    .map_err(|_| Code::InvalidRequest)?;
                let now = (self.clock)();
                Ok(json!(
                    leads
                        .iter()
                        .map(|lead| RecordView::of(lead, now))
                        .collect::<Vec<_>>()
                ))
            }
            Op::Delivery { lead, sale } => {
                if !record_id(&lead) || super::id(&sale).is_err() {
                    return Err(Code::InvalidRequest);
                }
                // Retention and recipients fence the sale like any read.
                let sale = store
                    .service_show(&access, &lead, &sale)
                    .map_err(|_| Code::AccessDenied)?;
                let offboarding = store
                    .offboarding_show(&access, &lead, &sale.admission.id)
                    .map_err(|_| Code::AccessDenied)?;
                let handoff = &sale.admission.sources.handoff;
                let (document, unavailable) = match &config.evidence {
                    None => (None, Some("not_configured")),
                    Some(root) => match super::service::Reader::new(Some(root))
                        .and_then(|mut reader| reader.read(handoff))
                        .ok()
                        .and_then(|bytes| Handoff::parse(&bytes))
                    {
                        Some(document) => (Some(document), None),
                        None => (None, Some("unreadable")),
                    },
                };
                Ok(json!(Delivery {
                    sale: sale.admission.id.clone(),
                    handoff_sha256: handoff.sha256.clone(),
                    handoff: document,
                    unavailable: unavailable.map(String::from),
                    offboarding,
                }))
            }
            Op::Audit { lead } => {
                if !record_id(&lead) {
                    return Err(Code::InvalidRequest);
                }
                store.show(&access, &lead).map_err(|_| Code::AccessDenied)?;
                let mut scoped: Vec<Audit> = Vec::new();
                let mut after = 0;
                loop {
                    let page = store
                        .audit(&access, after, 100)
                        .map_err(|_| Code::AccessDenied)?;
                    let Some(last) = page.last() else { break };
                    after = last.sequence;
                    scoped.extend(page.into_iter().filter(|a| a.lead == lead));
                }
                let keep = scoped.len().saturating_sub(AUDIT_MAX);
                Ok(json!(scoped.split_off(keep)))
            }
            Op::Claims => {
                let register = store
                    .claim_register(&access)
                    .map_err(|_| Code::AccessDenied)?;
                let mut history = Vec::new();
                while history.len() < HISTORY_MAX {
                    let page = store
                        .claim_history(&access, history.len(), 100)
                        .map_err(|_| Code::AccessDenied)?;
                    if page.is_empty() {
                        break;
                    }
                    history.extend(page);
                }
                history.truncate(HISTORY_MAX);
                Ok(json!(Claims { register, history }))
            }
            Op::Weekly => {
                if role != Role::Owner {
                    return Err(Code::AccessDenied);
                }
                let Some(sources) = &config.weekly else {
                    return Ok(Value::Null);
                };
                let mut bytes = Vec::new();
                super::super::private_open(&sources.input, false, false)
                    .map_err(|_| Code::Unavailable)?
                    .take(WEEKLY_MAX + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Code::Unavailable)?;
                let report =
                    gym::sales_weekly::rebuild(&sources.evidence_root, &bytes, (self.clock)())
                        .map_err(|_| Code::Refused)?;
                if report.manifest.owner != access.principal() {
                    return Err(Code::AccessDenied);
                }
                // A snapshot older than current custody (a refund, deletion,
                // suppression, or expired retention) refuses the review.
                store
                    .authorize_funnel_snapshots(&access, &report.sources)
                    .map_err(|_| Code::Stale)?;
                Ok(json!(Weekly {
                    manifest_digest: report.manifest_digest.clone(),
                    period_start: report.manifest.period_start,
                    period_end: report.manifest.period_end,
                    generated_at: report.manifest.generated_at,
                    gaps: report.manifest.gaps.clone(),
                    limitations: report.limitations.clone(),
                    contribution_scope: report.contribution_scope.clone(),
                    commercial_activation_attested: report.commercial_activation_attested,
                    finance_included: report.finances.is_some(),
                    cohorts: report.cohorts.iter().map(|c| json!(c)).collect(),
                    journeys: report.journeys.iter().map(|j| json!(j)).collect(),
                }))
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
                let effect = match entry.ledger {
                    Ledger::Pipeline => {
                        let parsed: super::Command =
                            serde_json::from_str(&entry.command).map_err(|_| Code::Unavailable)?;
                        Effect::of(&parsed.operation)
                    }
                    Ledger::Outbox => {
                        if role != Role::Owner {
                            return Err(Code::AccessDenied);
                        }
                        let parsed: outbox::Command =
                            serde_json::from_str(&entry.command).map_err(|_| Code::Unavailable)?;
                        Effect::of_outbox(&parsed.operation)
                    }
                };
                if !effect.is_some_and(|e| binding.effects.contains(&e)) {
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
        self.admit(
            config,
            binding,
            store,
            access,
            request,
            command,
            Ledger::Pipeline,
        )
    }

    /// One exact owner outbox command: decide an exact subject or stop
    /// dispatch. Nothing else in the outbox book is reachable remotely.
    fn outbox(
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
        let parsed: outbox::Command =
            serde_json::from_str(&command).map_err(|_| Code::InvalidRequest)?;
        if parsed.id != request || parsed.schema != outbox::COMMAND_SCHEMA {
            return Err(Code::InvalidRequest);
        }
        let effect = Effect::of_outbox(&parsed.operation).ok_or(Code::AccessDenied)?;
        if !binding.effects.contains(&effect) {
            return Err(Code::AccessDenied);
        }
        self.admit(
            config,
            binding,
            store,
            access,
            request,
            command,
            Ledger::Outbox,
        )
    }

    /// Journal an exact command's retry identity, then dispatch it once.
    #[allow(clippy::too_many_arguments)]
    fn admit(
        &self,
        config: &Config,
        binding: &Binding,
        store: &mut Store,
        access: &super::Access,
        request: &str,
        command: String,
        ledger: Ledger,
    ) -> std::result::Result<Receipt, Code> {
        let exact = digest(command.as_bytes());
        let mut journal = Journal::load(&config.journal, binding)?;
        match journal.entries.get(request) {
            Some(entry) if entry.digest != exact || entry.ledger != ledger => {
                return Err(Code::Conflict);
            }
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
                        ledger,
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
    let ledger = journal
        .entries
        .get(request)
        .map_or(Ledger::Pipeline, |e| e.ledger);
    let result = match ledger {
        Ledger::Pipeline => store.apply(access, command.as_bytes()),
        Ledger::Outbox => dispatch_outbox(binding, store, access, command),
    };
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

/// A mailbox key source that is never present: an approval then refuses.
struct NoMailbox;

impl super::email::MailboxCredentials for NoMailbox {
    fn load(&self, _account: &str) -> Result<super::email::MailboxSecret> {
        Err("sales mailbox key is not bound to this remote binding".into())
    }
}

/// Apply one outbox command through the canonical owner book and phrase its
/// result as a receipt. The mailbox key stays on the owner host.
fn dispatch_outbox(
    binding: &Binding,
    store: &mut Store,
    access: &super::Access,
    command: &str,
) -> Result<Receipt> {
    let parsed: outbox::Command =
        serde_json::from_str(command).map_err(|_| "malformed outbox command")?;
    let keys: Box<dyn super::email::MailboxCredentials> = match &binding.mailbox_key {
        Some(path) => {
            let view = store.email_view(access)?;
            let current = view["current"]
                .as_str()
                .ok_or("email configuration unavailable")?;
            let account = view["configurations"]
                .as_array()
                .and_then(|rows| rows.iter().find(|r| r["sha256"].as_str() == Some(current)))
                .and_then(|r| r["config"]["credential_account"].as_str())
                .ok_or("email account unavailable")?;
            Box::new(super::email::FileAccount::new(account, path)?)
        }
        None => Box::new(NoMailbox),
    };
    let revision = store.apply_sales_outbox(access, command.as_bytes(), keys.as_ref())?;
    let (lead, outcome) = match &parsed.operation {
        outbox::Operation::Decide {
            proposal, approve, ..
        } => (
            proposal.clone(),
            if *approve {
                "outbox_approved"
            } else {
                "outbox_rejected"
            },
        ),
        _ => ("outbox".to_string(), "outbox_stopped"),
    };
    Ok(Receipt {
        schema: OUTBOX_RECEIPT_SCHEMA.into(),
        command_digest: digest(command.as_bytes()),
        lead,
        revision,
        sequence: 0,
        at: (store.clock)(),
        outcome: outcome.into(),
    })
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
            || b.effects.len() > 9
        {
            return Err("invalid sales remote binding".into());
        }
        for path in std::iter::once(&b.credential)
            .chain(b.mailbox_key.as_ref())
            .chain(b.hires.as_ref())
        {
            if !path.is_absolute() || path.starts_with(&config.root) || path.starts_with(&root) {
                return Err("sales credentials must be outside the host task root".into());
            }
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

/// Floor supervision reads need the binding's grant and the owner role.
fn supervising(binding: &Binding, role: Role) -> std::result::Result<(), Code> {
    if binding.supervise && role == Role::Owner {
        Ok(())
    } else {
        Err(Code::AccessDenied)
    }
}

#[cfg(test)]
#[path = "remote/tests.rs"]
mod tests;
