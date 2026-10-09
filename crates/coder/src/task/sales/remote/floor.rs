//! Owner-only supervision projections for the remote adapter: Paul, the crew,
//! certification, Chicago-day expense holds, the floor report, the outbox,
//! untrusted replies, meeting proposals, and the private Agora board.
//!
//! Every projection is recomputed from the canonical books on each call and
//! carries no contact, message body, reply payload, or credential except the
//! one exact outbox subject an owner reviews before deciding it. Unknown
//! expense and delivery stay unknown; nothing here authorizes an effect.

use super::super::{
    Access, Result, Stage, Store, agents, earned, expenses, floor, meetings, outbox, replies, town,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FLOOR_SCHEMA: &str = "openagents.sales.remote-floor.v1";
pub const PROPOSAL_SCHEMA: &str = "openagents.sales.remote-outbox-proposal.v1";
pub const BOARD_SCHEMA: &str = "openagents.sales.remote-board.v1";
/// A private Agora board observation is current for three seconds.
pub const BOARD_TTL_SECONDS: u64 = 3;
pub const TIMEZONE: &str = "America/Chicago";
const ROWS_MAX: usize = 128;
const HIRING_BOOK_MAX: u64 = 192 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Floor {
    pub schema: String,
    pub observed_at: u64,
    /// The America/Chicago business day budgets and caps count against.
    pub business_day: u64,
    pub timezone: String,
    /// The fixed floor-wide daily model ceiling.
    pub ceiling_usd_millionths: u64,
    /// Paul's queue, or none when Paul is not configured for this owner.
    pub paul: Option<Paul>,
    pub crew: Vec<Member>,
    pub pending_hires: u64,
    pub certifications: Vec<Certification>,
    pub reservations: Vec<Reservation>,
    pub report: floor::Report,
    pub escalations: Vec<Escalation>,
    pub outbox: Outbox,
    pub replies: Vec<ReplyRow>,
    pub meetings: Vec<MeetingRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paul {
    pub idle: bool,
    pub model_available: bool,
    pub qualification_inferred: bool,
    pub external_effects: bool,
    pub rows: Vec<PaulRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaulRow {
    pub lead: String,
    pub revision: u64,
    pub stage: Stage,
    pub pending_drafts: u64,
    pub meetings_awaiting_owner: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub role: town::Role,
    pub lifecycle: town::Lifecycle,
    pub station: Option<String>,
    /// The activity word of the work under way; none when idle.
    pub activity: Option<String>,
    /// The kind of record that work cites (`assignment`, `draft`, ...).
    pub source_kind: Option<String>,
    pub queued: u64,
    pub idle: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certification {
    pub agent: String,
    pub version: u64,
    pub state: agents::CertState,
    pub measured_qualified: bool,
    pub expires_at: u64,
    pub outbound_authority: bool,
}

/// One model reservation. Its lead is omitted; amounts are model expense.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub id: String,
    pub agent: String,
    pub day: u64,
    pub status: expenses::Status,
    pub execution_unknown: bool,
    pub training: bool,
    pub maximum_usd_millionths: u64,
    /// The known list-price estimate; none while any settlement is unknown.
    pub estimated_usd_millionths: Option<u64>,
    pub billed_usd_millionths: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escalation {
    pub severity: floor::Severity,
    pub kind: String,
    pub at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outbox {
    pub revision: u64,
    pub controller_epoch: u64,
    pub paused: bool,
    pub cap: u32,
    pub live_messages_and_reservations: u32,
    pub fixture_messages_and_reservations: u32,
    pub rows: Vec<OutboxRow>,
    pub incidents: Vec<Incident>,
    /// Always false: a projection conveys no outbound authority.
    pub outbound_authority: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxRow {
    pub id: String,
    pub lead: String,
    pub phase: outbox::Phase,
    pub mode: outbox::Mode,
    pub kind: outbox::MessageKind,
    pub business_day: u64,
    pub expires_at: u64,
    pub subject_sha256: String,
    pub count_consumed: bool,
    pub decided: Option<bool>,
    /// The original subject is still held and readable for review.
    pub reviewable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incident {
    pub id: String,
    pub kind: outbox::IncidentKind,
    pub at: u64,
    pub resolved: bool,
}

/// An inbound reply's position. Its payload never leaves the owner host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyRow {
    pub id: String,
    pub lead: Option<String>,
    pub received_at: u64,
    pub safety: replies::Safety,
    pub owner_label: Option<replies::Label>,
    pub minimized: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingRow {
    pub id: String,
    pub lead: String,
    pub phase: meetings::Phase,
    pub start_at: u64,
    pub end_at: u64,
    pub owner_confirmation_needed: bool,
}

/// One outbox subject exactly as the owner would approve it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema: String,
    pub outbox_revision: u64,
    pub controller_epoch: u64,
    pub paused: bool,
    pub id: String,
    pub lead: String,
    pub phase: outbox::Phase,
    pub mode: outbox::Mode,
    pub kind: outbox::MessageKind,
    pub business_day: u64,
    pub expires_at: u64,
    pub subject_sha256: String,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<Attachment>,
    pub certification_reference: Option<String>,
    pub draft_reference: Option<String>,
    pub maximum_cost_microusd: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

/// The private Agora board: counts only, current for [`BOARD_TTL_SECONDS`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub schema: String,
    pub observed_at: u64,
    pub expires_at: u64,
    pub pipeline: [u64; 5],
    pub pending_drafts: u64,
    /// Qualified, in training or owner-marked, suspended.
    pub certifications: [u64; 3],
    pub practice_runs: u64,
    pub meeting_proposals: u64,
    pub outbox_live_proposals: u64,
    pub outbox_fixture_proposals: u64,
    pub outbox_unknown: u64,
    pub idle: bool,
    pub model_available: bool,
    /// The reviewed shared aggregate, `[earned sales, net USD floor]`, only
    /// while an approval is current. No bell event or record is exposed.
    pub shared: Option<[u64; 2]>,
}

fn stage_index(stage: Stage) -> usize {
    match stage {
        Stage::New => 0,
        Stage::Qualified => 1,
        Stage::Pilot => 2,
        Stage::Active => 3,
        Stage::Closed => 4,
    }
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn hiring_roster(path: Option<&Path>) -> Result<(Vec<town::Member>, Vec<town::PendingHire>)> {
    use crate::task::agent_hiring::{Book, SCHEMA};
    let book = match path {
        None => Book {
            schema: SCHEMA.into(),
            entries: Default::default(),
        },
        Some(path) => {
            use std::io::Read;
            let mut bytes = Vec::new();
            super::super::super::private_open(path, false, false)
                .map_err(|_| "sales hiring book is unavailable")?
                .take(HIRING_BOOK_MAX + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "sales hiring book is unavailable")?;
            if bytes.len() as u64 > HIRING_BOOK_MAX {
                return Err("sales hiring book exceeds its bound".into());
            }
            let book: Book =
                serde_json::from_slice(&bytes).map_err(|_| "sales hiring book is malformed")?;
            if book.schema != SCHEMA {
                return Err("sales hiring book has an unknown schema".into());
            }
            book
        }
    };
    Ok(town::roster_from_book(&book))
}

fn source_kind(source: &str) -> String {
    source.split(':').next().unwrap_or_default().to_owned()
}

impl Store {
    /// Paul's queue when Paul is configured for this owner.
    fn remote_paul(&mut self, owner: &Access) -> Option<super::super::paul::Pipeline> {
        self.read_paul_pipeline(owner).ok()
    }

    pub(super) fn remote_floor(&mut self, owner: &Access, hires: Option<&Path>) -> Result<Floor> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let paul = self.remote_paul(owner).map(|pipeline| Paul {
            idle: pipeline.idle,
            model_available: pipeline.model_available,
            qualification_inferred: pipeline.qualification_inferred,
            external_effects: pipeline.external_effects,
            rows: pipeline
                .rows
                .iter()
                .take(ROWS_MAX)
                .map(|row| PaulRow {
                    lead: row.original.lead.clone(),
                    revision: row.original.revision,
                    stage: row.recorded_stage,
                    pending_drafts: count(row.pending_drafts),
                    meetings_awaiting_owner: count(
                        row.meetings
                            .iter()
                            .filter(|m| m.owner_confirmation_needed && m.slot_current)
                            .count(),
                    ),
                })
                .collect(),
        });
        let (roster, pending) = hiring_roster(hires)?;
        let bodies = self.town_bodies(owner, &roster, &pending)?;
        let crew = bodies
            .bodies
            .iter()
            .map(|body| Member {
                name: body.name.clone(),
                role: body.role,
                lifecycle: body.lifecycle,
                station: match &body.placement {
                    town::Placement::Placed { station, .. } => Some(station.clone()),
                    town::Placement::Unplaced => None,
                },
                activity: body.current.as_ref().map(|w| w.activity.clone()),
                source_kind: body.current.as_ref().map(|w| source_kind(&w.source)),
                queued: count(body.queued.len()),
                idle: body.idle,
            })
            .collect();
        let view = self.sales_agent_owner_view(owner)?;
        let certifications = view
            .certificates
            .iter()
            .take(ROWS_MAX)
            .map(|record| Certification {
                agent: record.certification.agent.name.clone(),
                version: record.certification.version,
                state: record.certification.state,
                measured_qualified: record.measured_qualified,
                expires_at: record.certification.expires_at,
                outbound_authority: record.outbound_authority,
            })
            .collect();
        let day = agents::business_day(now)?;
        let mut reservations = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page = self.sales_model_reservations(owner, after.as_deref(), 100)?;
            let last = page.last().map(|r| r.id.clone());
            for r in page {
                if r.day + 1 < day && r.status == expenses::Status::Known {
                    continue;
                }
                let estimated = r
                    .settlements
                    .iter()
                    .map(|s| s.estimated_usd_millionths)
                    .try_fold(0u64, |sum, v| v.map(|v| sum.saturating_add(v)));
                let billed = r
                    .settlements
                    .iter()
                    .map(|s| s.billed_usd_millionths)
                    .try_fold(0u64, |sum, v| v.map(|v| sum.saturating_add(v)));
                let settled = !r.settlements.is_empty();
                reservations.push(Reservation {
                    id: r.id.clone(),
                    agent: r.native.name.clone(),
                    day: r.day,
                    status: r.status,
                    execution_unknown: r.execution_unknown,
                    training: r.training.is_some(),
                    maximum_usd_millionths: r.maximum_usd_millionths,
                    estimated_usd_millionths: estimated.filter(|_| settled),
                    billed_usd_millionths: billed.filter(|_| settled),
                });
            }
            match last {
                Some(id) if reservations.len() < ROWS_MAX => after = Some(id),
                _ => break,
            }
        }
        reservations.truncate(ROWS_MAX);
        let report = self.floor_report(owner)?;
        let escalations = self.floor_escalations(owner)?;
        let escalations = escalations
            .immediate
            .iter()
            .chain(&escalations.review)
            .take(ROWS_MAX)
            .map(|e| Escalation {
                severity: e.severity,
                kind: e.kind.clone(),
                at: e.at,
            })
            .collect();
        let projection = self.sales_outbox_projection(owner)?;
        let mut rows: Vec<OutboxRow> = projection
            .records
            .iter()
            .map(|r| OutboxRow {
                id: r.id.clone(),
                lead: r.lead.clone(),
                phase: r.phase,
                mode: r.mode,
                kind: r.kind,
                business_day: r.business_day,
                expires_at: r.expires_at,
                subject_sha256: r.subject_sha256.clone(),
                count_consumed: r.count_consumed,
                decided: r.decision.as_ref().map(|d| d.approved),
                reviewable: r.subject.is_some(),
            })
            .collect();
        rows.sort_by(|a, b| {
            b.business_day
                .cmp(&a.business_day)
                .then(b.expires_at.cmp(&a.expires_at))
                .then(a.id.cmp(&b.id))
        });
        rows.truncate(ROWS_MAX);
        let outbox = Outbox {
            revision: projection.revision,
            controller_epoch: projection.controller_epoch,
            paused: projection.paused,
            cap: projection.cap,
            live_messages_and_reservations: projection.live_messages_and_reservations,
            fixture_messages_and_reservations: projection.fixture_messages_and_reservations,
            rows,
            incidents: projection
                .incidents
                .values()
                .rev()
                .take(ROWS_MAX)
                .map(|i| Incident {
                    id: i.id.clone(),
                    kind: i.kind,
                    at: i.at,
                    resolved: i.resolved_at.is_some(),
                })
                .collect(),
            outbound_authority: false,
        };
        let mut replies: Vec<ReplyRow> = self
            .state
            .replies
            .records
            .values()
            .map(|r| ReplyRow {
                id: r.id.clone(),
                lead: r.lead.clone(),
                received_at: r.received_at,
                safety: r.safety,
                owner_label: r.owner_label,
                minimized: r.minimized_at.is_some() || r.payload.is_none(),
            })
            .collect();
        replies.sort_by(|a, b| b.received_at.cmp(&a.received_at).then(a.id.cmp(&b.id)));
        replies.truncate(ROWS_MAX);
        let meetings = self
            .state
            .meetings
            .meetings()
            .take(ROWS_MAX)
            .map(|m| MeetingRow {
                id: m.id.clone(),
                lead: m.lead.clone(),
                phase: m.phase,
                start_at: m.slot.start_at,
                end_at: m.slot.end_at,
                owner_confirmation_needed: m.phase == meetings::Phase::Pending
                    && m.owner_confirmation.is_none(),
            })
            .collect();
        Ok(Floor {
            schema: FLOOR_SCHEMA.into(),
            observed_at: now,
            business_day: day,
            timezone: TIMEZONE.into(),
            ceiling_usd_millionths: expenses::FLOOR_USD_MILLIONTHS,
            paul,
            crew,
            pending_hires: count(pending.len()),
            certifications,
            reservations,
            report,
            escalations,
            outbox,
            replies,
            meetings,
        })
    }

    /// One outbox subject for exact review; refused when it was minimized or
    /// its lead is not readable by this owner.
    pub(super) fn remote_proposal(&mut self, owner: &Access, id: &str) -> Result<Proposal> {
        self.refresh()?;
        self.admin(owner)?;
        let projection = self.sales_outbox_projection(owner)?;
        let record = projection
            .records
            .iter()
            .find(|r| r.id == id)
            .ok_or("outbox proposal is unavailable")?;
        let subject = record
            .subject
            .as_ref()
            .ok_or("outbox proposal subject is unavailable")?;
        if subject.sha256()? != record.subject_sha256 {
            return Err("outbox original proposal changed".into());
        }
        let message = &subject.proposal.message;
        Ok(Proposal {
            schema: PROPOSAL_SCHEMA.into(),
            outbox_revision: projection.revision,
            controller_epoch: projection.controller_epoch,
            paused: projection.paused,
            id: record.id.clone(),
            lead: record.lead.clone(),
            phase: record.phase,
            mode: record.mode,
            kind: record.kind,
            business_day: record.business_day,
            expires_at: record.expires_at,
            subject_sha256: record.subject_sha256.clone(),
            sender: match &message.sender {
                super::super::email::Sender::Human { principal } => format!("human:{principal}"),
                super::super::email::Sender::Agent { anchor, .. } => {
                    format!("agent:{}", anchor.name)
                }
            },
            recipient: message.recipient.clone(),
            subject: message.subject.clone(),
            body: message.body.clone(),
            attachments: subject
                .proposal
                .attachments
                .iter()
                .map(|a| Attachment {
                    filename: a.filename.clone(),
                    bytes: a.bytes,
                    sha256: a.sha256.clone(),
                })
                .collect(),
            certification_reference: subject.proposal.certification_reference.clone(),
            draft_reference: subject.proposal.draft_reference.clone(),
            maximum_cost_microusd: subject.proposal.maximum_cost_microusd,
        })
    }

    /// The private Agora board, counts only. It reads; it never rings a bell.
    pub(super) fn remote_board(&mut self, owner: &Access) -> Result<Board> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let pipeline = self.read_paul_pipeline(owner)?;
        let paul = self.sales_agent_anchor(owner, "paul")?;
        let practice = self.sales_roleplay_schedule(owner, &paul, None, 64)?;
        let view = self.sales_agent_owner_view(owner)?;
        let mut board = Board {
            schema: BOARD_SCHEMA.into(),
            observed_at: now,
            expires_at: now.saturating_add(BOARD_TTL_SECONDS),
            pipeline: [0; 5],
            pending_drafts: 0,
            certifications: [0; 3],
            practice_runs: count(practice.len()),
            meeting_proposals: 0,
            outbox_live_proposals: 0,
            outbox_fixture_proposals: 0,
            outbox_unknown: 0,
            idle: pipeline.idle,
            model_available: pipeline.model_available,
            shared: None,
        };
        let mut proposals = std::collections::BTreeSet::new();
        for row in &pipeline.rows {
            board.pipeline[stage_index(row.recorded_stage)] += 1;
            board.pending_drafts = board
                .pending_drafts
                .saturating_add(count(row.pending_drafts));
            for meeting in &row.meetings {
                if meeting.owner_confirmation_needed && meeting.slot_current {
                    proposals.insert(meeting.proposal_sha256.clone());
                }
            }
        }
        board.meeting_proposals = count(proposals.len());
        let outbox = self.sales_outbox_projection(owner)?;
        for record in &outbox.records {
            if record.phase == outbox::Phase::Unknown {
                board.outbox_unknown += 1;
            }
            if record.phase != outbox::Phase::Proposed
                || record.expires_at <= now
                || record.subject.is_none()
            {
                continue;
            }
            match record.mode {
                outbox::Mode::Live => board.outbox_live_proposals += 1,
                outbox::Mode::Fixture => board.outbox_fixture_proposals += 1,
            }
        }
        for record in &view.certificates {
            let index = match record.certification.state {
                agents::CertState::Qualified if record.measured_qualified => 0,
                agents::CertState::Suspended => 2,
                _ => 1,
            };
            board.certifications[index] += 1;
        }
        if let earned::Shared::Available { aggregate, .. } = self.shared_aggregate()? {
            board.shared = Some([
                aggregate.earned_sales,
                aggregate.net_usd_millionths_floor / 1_000_000,
            ]);
        }
        Ok(board)
    }
}
