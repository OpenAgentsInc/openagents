//! Exact owner decisions and single-use dispatch in the canonical sales book.
//! Unknown handoff consumes its reservation and never authorizes another attempt.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

pub mod batch;

pub const PROPOSAL_SCHEMA: &str = "openagents.sales.outbox-proposal.v1";
pub const SUBJECT_SCHEMA: &str = "openagents.sales.outbox-subject.v1";
pub const COMMAND_SCHEMA: &str = "openagents.sales.outbox-command.v1";
pub const PROJECTION_SCHEMA: &str = "openagents.sales.outbox-projection.v1";
const MAX_RECORDS: usize = 1024;
const MAX_ATTACHMENT: usize = 64 * 1024;
const MAX_ATTACHMENTS: usize = 4;
pub const QUALIFIED_AGENT_SUBJECT: &str = "Requested business information";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    OwnerPilot,
    FirstMessage,
    Reply,
    FollowUp,
    SalesPost,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Fixture,
    Live,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub filename: String,
    pub media_type: String,
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}
impl Attachment {
    fn check(&self) -> Result<()> {
        token(&self.sha256)?;
        if self.filename.is_empty()
            || self.filename.len() > 128
            || !self
                .filename
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || self.filename.starts_with('.')
            || self.media_type != "text/plain"
            || !self.path.is_absolute()
            || self.path.as_os_str().len() > 4096
            || self.bytes == 0
            || self.bytes > MAX_ATTACHMENT as u64
        {
            return Err(
                "outbox attachment requires an explicit bounded private text source".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema: String,
    pub id: String,
    pub kind: MessageKind,
    pub message: email::Message,
    pub attachments: Vec<Attachment>,
    pub certification_reference: Option<String>,
    pub draft_reference: Option<String>,
    pub follow_up_reference: Option<agents::Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_reference: Option<agents::Artifact>,
    pub model_reservation_reference: Option<String>,
    pub maximum_cost_microusd: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    pub schema: String,
    pub proposal: Proposal,
    pub mode: Mode,
    pub message_sha256: String,
    pub mime_sha256: String,
    pub scope_sha256: String,
    pub qualification_sha256: String,
    pub draft_qualification_sha256: Option<String>,
    pub budget_sha256: String,
    pub controller_epoch: u64,
    pub reserved_business_day: u64,
    pub reservation_id: String,
    pub created_at: u64,
    pub retain_until: u64,
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub contact_pins: std::collections::BTreeSet<String>,
}
impl Subject {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "outbox subject serialization failed")?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Proposed,
    Approved,
    Rejected,
    DispatchIntent,
    Accepted,
    Delivered,
    Failed,
    HardBounce,
    Unknown,
    Cancelled,
    Invalidated,
    OwnerReported,
}
impl Phase {
    fn outstanding(self) -> bool {
        matches!(self, Self::Proposed | Self::Approved)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub subject_sha256: String,
    pub owner: String,
    pub approved: bool,
    pub at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub lead: String,
    pub subject_sha256: String,
    pub mime_sha256: String,
    pub subject: Option<Subject>,
    pub mode: Mode,
    pub kind: MessageKind,
    pub actor: String,
    pub business_day: u64,
    pub expires_at: u64,
    pub maximum_cost_microusd: u64,
    #[serde(default)]
    pub model_reservation_reference: Option<String>,
    pub created_at: u64,
    pub retain_until: u64,
    pub decision: Option<Decision>,
    pub phase: Phase,
    pub count_consumed: bool,
    pub attempt: Option<String>,
    #[serde(default)]
    pub attempt_started_at: Option<u64>,
    #[serde(default)]
    pub observation_at: Option<u64>,
    pub observation: Option<email::smtp::Observation>,
    pub minimized_at: Option<u64>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub contact_pins: std::collections::BTreeSet<String>,
}
/// An owner statement about an original uncertain attempt, never provider evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    pub id: String,
    pub owner: String,
    pub proposal: String,
    pub subject_sha256: String,
    pub mime_sha256: String,
    pub attempt: String,
    pub reference_sha256: String,
    pub at: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Terminal,
    Phone,
    Lectern,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub surface: Surface,
    pub owner_binding_available: bool,
    pub reason: String,
}
/// Every renderer reads the same subject and submits the same exact command.
/// A projection conveys no approval or outbound authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub schema: String,
    pub revision: u64,
    pub controller_epoch: u64,
    pub paused: bool,
    pub cap: u32,
    pub business_day: u64,
    pub live_messages_and_reservations: u32,
    pub fixture_messages_and_reservations: u32,
    pub records: Vec<Record>,
    pub incidents: BTreeMap<String, Incident>,
    pub owner_reports: BTreeMap<String, OwnerReport>,
    pub reconciliations: BTreeMap<String, Reconciliation>,
    pub owner_reports_are_delivery_evidence: bool,
    pub reconciliations_are_delivery_evidence: bool,
    pub outbound_authority: bool,
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub batches: batch::Book,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncidentKind {
    Complaint,
    UnsupportedSentClaim,
    SuppressionBreach,
    AuthenticationFailure,
    HardBounce,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incident {
    pub id: String,
    pub kind: IncidentKind,
    pub reference_sha256: String,
    pub at: u64,
    pub resolved_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activation {
    pub config_sha256: String,
    pub policy_sha256: String,
    pub qualified_actors: BTreeMap<String, String>,
    pub reply_qualification_sha256: String,
    pub owner_review_sha256: String,
    pub expires_at: u64,
}
/// An attributable owner statement; it establishes no provider delivery or qualification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerReport {
    pub id: String,
    pub owner: String,
    pub mode: Mode,
    pub kind: MessageKind,
    pub sent_at: u64,
    pub original_subject_sha256: Option<String>,
    pub reference_sha256: String,
    pub counted_in_original_attempt: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    pub revision: u64,
    pub epoch: u64,
    pub records: BTreeMap<String, Record>,
    pub incidents: BTreeMap<String, Incident>,
    pub owner_reports: BTreeMap<String, OwnerReport>,
    #[serde(default)]
    pub reconciliations: BTreeMap<String, Reconciliation>,
    pub activation: Option<Activation>,
    pub reply_qualification: Option<String>,
    pub cap: u32,
    pub cap_started_at: u64,
    pub paused: bool,
    #[serde(default)]
    pub batches: batch::Book,
    commands: BTreeMap<String, (String, String, u64)>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.records.len() > MAX_RECORDS
            || self.incidents.len() > MAX_RECORDS
            || self.owner_reports.len() > MAX_RECORDS
            || self.reconciliations.len() > MAX_RECORDS
            || self.commands.len() > MAX_RECORDS * 4
            || !matches!(self.cap, 0 | 5 | 10 | 20)
        {
            return Err("outbox history or cap exceeds its bound".into());
        }
        self.batches.check()?;
        for (id, entry) in &self.reconciliations {
            super::id(id)?;
            super::id(&entry.owner)?;
            for sha in [
                &entry.subject_sha256,
                &entry.mime_sha256,
                &entry.attempt,
                &entry.reference_sha256,
            ] {
                token(sha)?;
            }
            let row = self
                .records
                .get(&entry.proposal)
                .ok_or("outbox reconciliation original attempt is unavailable")?;
            if entry.id != *id
                || !row.count_consumed
                || row.subject_sha256 != entry.subject_sha256
                || row.mime_sha256 != entry.mime_sha256
                || row.attempt.as_ref() != Some(&entry.attempt)
            {
                return Err("outbox reconciliation original identity changed".into());
            }
        }
        for (id, report) in &self.owner_reports {
            super::id(id)?;
            token(&report.reference_sha256)?;
            if report.id != *id {
                return Err("owner report identity changed".into());
            }
            if let Some(subject) = &report.original_subject_sha256 {
                token(subject)?;
            }
            if report.counted_in_original_attempt
                && !self.records.values().any(|r| {
                    report.original_subject_sha256.as_ref() == Some(&r.subject_sha256)
                        && r.count_consumed
                        && r.phase == Phase::OwnerReported
                })
            {
                return Err("owner report original count is unavailable".into());
            }
        }
        for (id, record) in &self.records {
            super::id(id)?;
            if record.contact_pins.len() > 32 {
                return Err("original outbox contact history exceeds its bound".into());
            }
            for pin in &record.contact_pins {
                token(pin)?;
            }
            token(&record.subject_sha256)?;
            token(&record.mime_sha256)?;
            token(
                record
                    .lead
                    .strip_prefix("lead_")
                    .ok_or("outbox original lead identity changed")?,
            )?;
            if record
                .observation_at
                .zip(record.attempt_started_at)
                .is_some_and(|(observed, started)| observed < started)
            {
                return Err("outbox native observation clock precedes its original attempt".into());
            }
            if let Some(attempt) = &record.attempt {
                token(attempt)?;
            }
            if let Some(subject) = &record.subject {
                if let Some(sha) = &subject.draft_qualification_sha256 {
                    token(sha)?;
                }
                for sha in [
                    &subject.message_sha256,
                    &subject.mime_sha256,
                    &subject.scope_sha256,
                    &subject.qualification_sha256,
                    &subject.budget_sha256,
                    &subject.reservation_id,
                ] {
                    token(sha)?;
                }
                if subject.schema != SUBJECT_SCHEMA
                    || subject.proposal.schema != PROPOSAL_SCHEMA
                    || subject.proposal.id != record.id
                    || subject.proposal.message.lead != record.lead
                    || subject.mode != record.mode
                    || !subject.contact_pins.is_empty()
                        && subject.contact_pins != record.contact_pins
                    || subject.proposal.kind != record.kind
                    || actor(&subject.proposal.message) != record.actor
                    || subject.mime_sha256 != record.mime_sha256
                    || subject.retain_until != record.retain_until
                    || subject.created_at != record.created_at
                    || subject.reserved_business_day != record.business_day
                    || subject.proposal.message.expires_at != record.expires_at
                    || subject.proposal.maximum_cost_microusd != record.maximum_cost_microusd
                    || subject.proposal.model_reservation_reference
                        != record.model_reservation_reference
                {
                    return Err("outbox original subject metadata changed".into());
                }
            } else if record.minimized_at.is_none() {
                return Err("outbox subject disappeared without minimization".into());
            }
            if let Some(observation) = &record.observation {
                token(&observation.message_sha256)?;
                token(&observation.reference_sha256)?;
                if observation.message_sha256 != record.mime_sha256 {
                    return Err("outbox observation digest changed".into());
                }
            }
            if id != &record.id
                || record
                    .subject
                    .as_ref()
                    .is_some_and(|s| s.sha256().ok().as_ref() != Some(&record.subject_sha256))
                || record.count_consumed && record.attempt.is_none()
                || matches!(
                    record.phase,
                    Phase::DispatchIntent | Phase::Accepted | Phase::Delivered | Phase::Unknown
                ) && !record.count_consumed
                || record
                    .decision
                    .as_ref()
                    .is_some_and(|d| d.subject_sha256 != record.subject_sha256)
            {
                return Err("outbox immutable identity or consumption disagrees".into());
            }
        }
        Ok(())
    }
    fn invalidate_pending(&mut self) {
        for record in self.records.values_mut().filter(|r| r.phase.outstanding()) {
            record.phase = Phase::Invalidated;
        }
    }
    pub(super) fn recover(&mut self) -> bool {
        let mut changed = false;
        for record in self
            .records
            .values_mut()
            .filter(|r| r.phase == Phase::DispatchIntent)
        {
            record.phase = Phase::Unknown;
            changed = true;
        }
        changed
    }
    pub(super) fn redact(&mut self, lead: &str, now: u64) {
        for record in self.records.values_mut().filter(|r| r.lead == lead) {
            record.subject = None;
            record.minimized_at = Some(now);
            if record.phase.outstanding() {
                record.phase = Phase::Invalidated;
            }
        }
    }
    pub(super) fn expire(&mut self, now: u64) -> bool {
        let mut changed = false;
        for record in self
            .records
            .values_mut()
            .filter(|r| r.subject.is_some() && r.retain_until <= now)
        {
            record.subject = None;
            record.minimized_at = Some(now);
            if record.phase.outstanding() {
                record.phase = Phase::Invalidated;
            }
            changed = true;
        }
        changed
    }
    pub(super) fn obligations(&self, lead: &str) -> Vec<privacy::Obligation> {
        self.records
            .values()
            .filter(|r| r.lead == lead && r.count_consumed)
            .map(|r| privacy::Obligation {
                kind: "outbox_attempt".into(),
                original_sha256: r.subject_sha256.clone(),
                origin_sha256: digest(r.id.as_bytes()),
                currency: None,
                currency_scale: None,
                amount_minor: None,
                paid_minor: None,
                refunded_minor: None,
                unknown: matches!(
                    r.phase,
                    Phase::Accepted | Phase::Unknown | Phase::DispatchIntent | Phase::OwnerReported
                ),
            })
            .collect()
    }
    fn counts(&self, day: u64, mode: Mode, now: u64) -> (u32, BTreeMap<String, u32>) {
        let mut total = 0;
        let mut actors = BTreeMap::new();
        for record in self.records.values().filter(|r| {
            r.business_day == day
                && r.mode == mode
                && (r.count_consumed || r.phase.outstanding() && r.expires_at > now)
        }) {
            total += 1;
            *actors.entry(record.actor.clone()).or_default() += 1;
        }
        for report in self.owner_reports.values().filter(|r| {
            r.mode == mode
                && !r.counted_in_original_attempt
                && agents::business_day(r.sent_at).ok() == Some(day)
        }) {
            total += 1;
            *actors.entry(format!("human:{}", report.owner)).or_default() += 1;
        }
        (total, actors)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub schema: String,
    pub id: String,
    pub expected_revision: u64,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Reconcile {
        proposal: String,
        subject_sha256: String,
        mime_sha256: String,
        attempt: String,
        reference_sha256: String,
    },
    Decide {
        proposal: String,
        subject_sha256: String,
        approve: bool,
    },
    OwnerSent {
        proposal: Option<String>,
        subject_sha256: Option<String>,
        message_kind: MessageKind,
        sent_at: u64,
        reference_sha256: String,
    },
    Pause {
        incident: IncidentKind,
        reference_sha256: String,
    },
    Restart {
        corrected_incidents: Vec<String>,
        correction_sha256: String,
    },
    Activate {
        activation: Activation,
    },
    RaiseCap {
        cap: u32,
        operating_week_sha256: String,
        owner_review_sha256: String,
    },
    GrantBatch {
        grant: batch::Grant,
    },
    RevokeBatch {
        grant: String,
        reference_sha256: String,
    },
    RaiseBatch {
        size: u32,
        owner_review_sha256: String,
    },
}
fn clean_week_elapsed(start: u64, now: u64) -> Result<bool> {
    let zone =
        jiff::tz::TimeZone::get("America/Chicago").map_err(|_| "business timezone unavailable")?;
    let timestamp = |at: u64| {
        jiff::Timestamp::from_second(i64::try_from(at).map_err(|_| "business timestamp invalid")?)
            .map_err(|_| "business timestamp invalid")
    };
    let target = timestamp(start)?
        .to_zoned(zone)
        .checked_add(jiff::Span::new().days(7))
        .map_err(|_| "operating week exceeds its date bound")?;
    Ok(timestamp(now)? >= target.timestamp())
}
fn actor(message: &email::Message) -> String {
    match &message.sender {
        email::Sender::Human { principal } => format!("human:{principal}"),
        email::Sender::Agent { anchor, .. } => anchor.pubkey.clone(),
    }
}
fn mode(config: &email::Config) -> Mode {
    if config.provider == email::Provider::Fixture {
        Mode::Fixture
    } else {
        Mode::Live
    }
}
fn attachment_bytes(store: &Store, lead: &Lead, attachment: &Attachment) -> Result<Vec<u8>> {
    attachment.check()?;
    privacy::check_outbound_copy(&store.state, lead, &attachment.filename)?;
    store.external_file(&attachment.path)?;
    let parent = attachment
        .path
        .parent()
        .ok_or("outbox attachment parent is unavailable")?
        .canonicalize()
        .map_err(|_| "outbox attachment parent is unavailable")?;
    let directory = agents::native::directory(&parent)?;
    let (_file, bytes) = privacy::private_bytes(&attachment.path, MAX_ATTACHMENT)?;
    if bytes.len() as u64 != attachment.bytes || digest(&bytes) != attachment.sha256 {
        return Err("outbox attachment source changed".into());
    }
    agents::native::same_directory(
        attachment
            .path
            .parent()
            .ok_or("outbox attachment parent is unavailable")?,
        &directory,
    )?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "outbox text attachment is not UTF-8")?;
    privacy::check_outbound_copy(&store.state, lead, text)?;
    Ok(bytes)
}
fn base64_lines(bytes: &[u8]) -> String {
    let encoded = STANDARD.encode(bytes);
    let mut out = String::new();
    for line in encoded.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(line).unwrap_or_default());
        out.push_str("\r\n");
    }
    out
}
pub(super) fn mime(
    store: &Store,
    prepared: &email::Prepared,
    proposal: &Proposal,
    created_at: u64,
) -> Result<Vec<u8>> {
    if proposal.attachments.len() > MAX_ATTACHMENTS {
        return Err("outbox attachment count exceeds its bound".into());
    }
    let lead = store
        .state
        .leads
        .get(&proposal.message.lead)
        .ok_or("outbox original lead is unavailable")?;
    let metadata =
        serde_json::to_string(proposal).map_err(|_| "outbox proposal serialization failed")?;
    privacy::check_outbound_copy(&store.state, lead, &metadata)?;
    privacy::check_outbound_copy(&store.state, lead, &prepared.rendered)?;
    let config = store.email_config(&proposal.message.config_sha256, (store.clock)())?;
    let domain = config
        .sender
        .split_once('@')
        .ok_or("outbox sender domain is unavailable")?
        .1;
    let mut encoded_subject = String::new();
    let mut chunk = String::new();
    for character in proposal.message.subject.chars() {
        if chunk.len() + character.len_utf8() > 36 {
            if !encoded_subject.is_empty() {
                encoded_subject.push_str("\r\n ");
            }
            encoded_subject.push_str(&format!(
                "=?UTF-8?B?{}?=",
                STANDARD.encode(chunk.as_bytes())
            ));
            chunk.clear();
        }
        chunk.push(character);
    }
    if !chunk.is_empty() {
        if !encoded_subject.is_empty() {
            encoded_subject.push_str("\r\n ");
        }
        encoded_subject.push_str(&format!(
            "=?UTF-8?B?{}?=",
            STANDARD.encode(chunk.as_bytes())
        ));
    }
    let date = jiff::Timestamp::from_second(
        i64::try_from(created_at).map_err(|_| "outbox date exceeds its bound")?,
    )
    .map_err(|_| "outbox date is invalid")?
    .to_zoned(jiff::tz::TimeZone::UTC)
    .strftime("%a, %d %b %Y %H:%M:%S +0000")
    .to_string();
    let body = prepared
        .rendered
        .split_once("\n\n")
        .ok_or("outbox prepared body is unavailable")?
        .1;
    let message_id =
        digest(&serde_json::to_vec(proposal).map_err(|_| "outbox message identity failed")?);
    let mut out = format!(
        "From: {}\r\nReply-To: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {}\r\nMessage-ID: <{}@{}>\r\nMIME-Version: 1.0\r\n",
        config.sender,
        config.reply_to,
        proposal.message.recipient,
        encoded_subject,
        date,
        message_id,
        domain
    );
    if proposal.attachments.is_empty() {
        out.push_str(
            "Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n",
        );
        out.push_str(&base64_lines(body.as_bytes()));
    } else {
        let boundary = format!("sales-{}", prepared.sha256);
        out.push_str(&format!("Content-Type: multipart/mixed; boundary=\"{boundary}\"\r\n\r\n--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
        out.push_str(&base64_lines(body.as_bytes()));
        for attachment in &proposal.attachments {
            let bytes = attachment_bytes(store, lead, attachment)?;
            out.push_str(&format!("--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n",attachment.filename));
            out.push_str(&base64_lines(&bytes));
        }
        out.push_str(&format!("--{boundary}--\r\n"));
    }
    email::smtp::validate_mime(out.as_bytes())?;
    Ok(out.into_bytes())
}
/// Reconstruct legacy minimal history only from retained canonical original sources.
pub(super) fn remember_contact_history(state: &mut State) -> Result<bool> {
    let mut pins = Vec::new();
    for (id, record) in &state.outbox.records {
        if !record.contact_pins.is_empty() {
            continue;
        }
        if let (Some(subject), Some(lead)) = (&record.subject, state.leads.get(&record.lead)) {
            let original = if subject.contact_pins.is_empty() {
                let address = format!("email:{}", subject.proposal.message.recipient);
                privacy::contact_history_pins(state, &address, &lead.details.account)?
            } else {
                subject.contact_pins.clone()
            };
            pins.push((id.clone(), original));
        }
    }
    let changed = !pins.is_empty();
    for (id, pins) in pins {
        state.outbox.records.get_mut(&id).unwrap().contact_pins = pins;
    }
    Ok(changed)
}
impl Store {
    pub(super) fn outbox_contact_records(&self, lead: &str, mode: Mode) -> Result<Vec<&Record>> {
        let lead = self
            .state
            .leads
            .get(lead)
            .ok_or("original contact history source unavailable")?;
        let pins =
            privacy::contact_history_pins(&self.state, &lead.contact, &lead.details.account)?;
        let mut matches = Vec::new();
        for record in self
            .state
            .outbox
            .records
            .values()
            .filter(|r| r.mode == mode && r.count_consumed)
        {
            if record.contact_pins.is_empty() {
                return Err("historical consumed contact identity is unavailable".into());
            }
            if !record.contact_pins.is_disjoint(&pins) {
                matches.push(record);
            }
        }
        Ok(matches)
    }
    fn outbox_message_kind(
        &self,
        owner: &Access,
        proposal: &Proposal,
        mode: Mode,
        excluding: Option<&str>,
    ) -> Result<()> {
        if mode == Mode::Fixture {
            return Ok(());
        }
        match proposal.kind {
            MessageKind::OwnerPilot | MessageKind::FirstMessage => {
                if proposal.kind == MessageKind::OwnerPilot
                    && !matches!(proposal.message.sender, email::Sender::Human { .. })
                {
                    return Err("owner SMTP pilot requires the current human owner".into());
                }
                if self
                    .outbox_contact_records(&proposal.message.lead, mode)?
                    .iter()
                    .any(|r| Some(r.id.as_str()) != excluding)
                {
                    return Err("original contact already has a consumed message; exact reply or follow-up authority required".into());
                }
                Ok(())
            }
            MessageKind::Reply => self.validate_sales_reply_response(owner, proposal),
            MessageKind::FollowUp => self.validate_sales_follow_up(proposal, mode),
            MessageKind::SalesPost => Err("public sales posting channel is unavailable".into()),
        }
    }
    pub(super) fn outbox_qualification(
        &mut self,
        access: &Access,
        proposal: &Proposal,
        mode: Mode,
    ) -> Result<(String, Option<String>)> {
        if let (Mode::Live, email::Sender::Agent { anchor, assignment }) =
            (mode, &proposal.message.sender)
        {
            if proposal.message.subject != QUALIFIED_AGENT_SUBJECT
                || !proposal.attachments.is_empty()
            {
                return Err(
                    "qualified agent email requires the neutral native subject and no attachments"
                        .into(),
                );
            }
            let snapshot = self.qualified_sales_draft(
                access,
                &proposal.message.lead,
                anchor,
                assignment,
                &proposal.message.policy_sha256,
                proposal
                    .certification_reference
                    .as_deref()
                    .ok_or("measured certification reference required")?,
                proposal
                    .draft_reference
                    .as_deref()
                    .ok_or("exact qualified draft reference required")?,
            )?;
            if snapshot.body_sha256 != digest(proposal.message.body.as_bytes())
                || proposal.model_reservation_reference.as_deref()
                    != Some(snapshot.original_expense_reference.as_str())
                || snapshot.qualification.expires_at < proposal.message.expires_at
            {
                return Err(
                    "qualified draft body, original content expense, or expiry changed".into(),
                );
            }
            return Ok((snapshot.qualification.sha256()?, Some(snapshot.sha256()?)));
        }
        match &proposal.message.sender {
            email::Sender::Human { principal } if principal == access.principal() => Ok((
                digest(format!("{mode:?}-human-owner:{principal}").as_bytes()),
                None,
            )),
            email::Sender::Agent { anchor, assignment } => {
                let lead = self
                    .state
                    .leads
                    .get(&proposal.message.lead)
                    .ok_or("outbox lead is unavailable")?;
                let grant = lead
                    .agent_records
                    .assignments
                    .get(assignment)
                    .ok_or("outbox assignment is unavailable")?;
                if !grant.active || grant.anchor != *anchor {
                    return Err("outbox current assignment is unavailable".into());
                }
                let reference = proposal
                    .certification_reference
                    .as_ref()
                    .ok_or("outbox certification reference is required")?;
                let cert = self
                    .state
                    .agents
                    .certificates
                    .get(reference)
                    .ok_or("outbox certification is unavailable")?;
                if cert.certification.agent != *anchor
                    || cert.certification.expires_at < proposal.message.expires_at
                    || cert.certification.state != agents::CertState::OwnerMarked
                    || cert.certification.playbook
                        != self
                            .email_policy(&proposal.message.policy_sha256, (self.clock)())?
                            .playbook
                {
                    return Err("outbox certification pins changed".into());
                }
                Ok((
                    digest(
                        &serde_json::to_vec(cert)
                            .map_err(|_| "outbox certification serialization failed")?,
                    ),
                    None,
                ))
            }
            _ => Err("outbox human authority is unavailable".into()),
        }
    }
    fn outbox_activation_actors(&mut self, owner: &Access, activation: &Activation) -> Result<()> {
        let human = format!("human:{}", owner.principal());
        let human_sha = digest(format!("Live-human-owner:{}", owner.principal()).as_bytes());
        for (actor, expected) in &activation.qualified_actors {
            if actor == &human && expected == &human_sha {
                continue;
            }
            let mut candidates = Vec::new();
            for lead in self.state.leads.values() {
                for (assignment, grant) in &lead.agent_records.assignments {
                    if !grant.active || &grant.anchor.pubkey != actor {
                        continue;
                    }
                    for (reference, cert) in &self.state.agents.certificates {
                        if cert.certification.agent == grant.anchor
                            && cert.certification.state == agents::CertState::Qualified
                            && cert.certification.expires_at >= activation.expires_at
                        {
                            if candidates.len() >= MAX_RECORDS {
                                return Err(
                                    "activation qualification search exceeds its bound".into()
                                );
                            }
                            candidates.push((
                                lead.id.clone(),
                                grant.anchor.clone(),
                                assignment.clone(),
                                reference.clone(),
                            ));
                        }
                    }
                }
            }
            let mut qualified = false;
            for (lead, anchor, assignment, reference) in candidates {
                if let Ok(snapshot) = self.qualified_sales_outbound(
                    owner,
                    &lead,
                    &anchor,
                    &assignment,
                    &activation.policy_sha256,
                    &reference,
                ) {
                    if snapshot.expires_at >= activation.expires_at
                        && snapshot.sha256()? == *expected
                    {
                        qualified = true;
                        break;
                    }
                }
            }
            if !qualified {
                return Err(
                    "activation actor has no exact current measured native qualification".into(),
                );
            }
        }
        Ok(())
    }
    pub(super) fn outbox_budget(&self, proposal: &Proposal, mode: Mode) -> Result<String> {
        if mode == Mode::Fixture {
            if proposal.maximum_cost_microusd != 0 || proposal.model_reservation_reference.is_some()
            {
                return Err(
                    "fixture outbox cannot assert a billable or native model reservation".into(),
                );
            }
            return Ok(digest(b"fixture-only, no model/provider charge"));
        }
        let Some(reference) = &proposal.model_reservation_reference else {
            if matches!(proposal.message.sender, email::Sender::Human { .. })
                && proposal.maximum_cost_microusd == 0
            {
                return Ok(digest(
                    b"owner-written SMTP, no model call or model reservation",
                ));
            }
            return Err("canonical sales cost reservation is unavailable".into());
        };
        let receipt = self
            .state
            .expenses
            .reservation(reference)
            .ok_or("canonical original sales cost receipt is unavailable")?;
        let settlement = receipt
            .settlements
            .last()
            .ok_or("sales model cost settlement is unavailable")?;
        let estimate = settlement
            .estimated_usd_millionths
            .ok_or("sales model cost estimate is unknown")?;
        if receipt.status != expenses::Status::Known
            || receipt.execution_unknown
            || receipt.training.is_some()
            || receipt.lead != proposal.message.lead
            || receipt.maximum_usd_millionths != proposal.maximum_cost_microusd
            || estimate.max(settlement.billed_usd_millionths.unwrap_or(0))
                > receipt.maximum_usd_millionths
        {
            return Err(
                "outbox model cost is unknown, exceeded, or has different original attribution"
                    .into(),
            );
        }
        if let email::Sender::Agent { anchor, assignment } = &proposal.message.sender {
            if &receipt.native != anchor || &receipt.assignment != assignment {
                return Err("outbox original model actor or assignment changed".into());
            }
        }
        Ok(digest(&serde_json::to_vec(receipt).map_err(
            |_| "outbox original cost receipt serialization failed",
        )?))
    }
    fn outbox_current(
        &mut self,
        access: &Access,
        subject: &Subject,
        keys: &dyn email::MailboxCredentials,
    ) -> Result<(email::Prepared, Vec<u8>)> {
        self.refresh()?;
        self.admin(access)?;
        if self.state.outbox.paused
            || subject.retain_until <= (self.clock)()
            || subject.controller_epoch != self.state.outbox.epoch
            || agents::business_day((self.clock)())? != subject.reserved_business_day
        {
            return Err(
                "outbox controller is paused, changed, or outside the reserved business day".into(),
            );
        }
        self.outbox_message_kind(
            access,
            &subject.proposal,
            subject.mode,
            Some(&subject.proposal.id),
        )?;
        let prepared = self.prepare_email(access, subject.proposal.message.clone(), keys)?;
        let config = self
            .email_config(&subject.proposal.message.config_sha256, (self.clock)())?
            .clone();
        let qualification = self.outbox_qualification(access, &subject.proposal, subject.mode)?;
        if mode(&config) != subject.mode
            || prepared.sha256 != subject.message_sha256
            || prepared.scope_sha256 != subject.scope_sha256
            || qualification
                != (
                    subject.qualification_sha256.clone(),
                    subject.draft_qualification_sha256.clone(),
                )
            || self.outbox_budget(&subject.proposal, subject.mode)? != subject.budget_sha256
        {
            return Err("outbox exact approved authority or content changed".into());
        }
        if subject.mode == Mode::Live {
            self.outbox_live_activation(subject)?;
        }
        let mime = mime(self, &prepared, &subject.proposal, subject.created_at)?;
        if digest(&mime) != subject.mime_sha256 {
            return Err("outbox approved MIME or attachment bytes changed".into());
        }
        Ok((prepared, mime))
    }
    fn outbox_live_activation(&self, subject: &Subject) -> Result<()> {
        let activation = self
            .state
            .outbox
            .activation
            .as_ref()
            .ok_or("owner outreach activation is unavailable")?;
        if activation.config_sha256 != subject.proposal.message.config_sha256
            || activation.policy_sha256 != subject.proposal.message.policy_sha256
            || activation
                .qualified_actors
                .get(&actor(&subject.proposal.message))
                != Some(&subject.qualification_sha256)
            || activation.expires_at < subject.proposal.message.expires_at
            || self.state.outbox.reply_qualification.as_ref()
                != Some(&activation.reply_qualification_sha256)
        {
            return Err(
                "outbox live certification, reply handler, or owner grant is unavailable".into(),
            );
        }
        self.current_sales_reply_qualification(&activation.reply_qualification_sha256)?;
        self.email_config(&activation.config_sha256, (self.clock)())?
            .smtp
            .as_ref()
            .ok_or("dedicated SMTP transport configuration is unavailable")?
            .check()
    }
    pub fn propose_sales_outbox(
        &mut self,
        access: &Access,
        proposal: Proposal,
        keys: &dyn email::MailboxCredentials,
    ) -> Result<Subject> {
        self.refresh()?;
        self.admin(access)?;
        id(&proposal.id)?;
        if proposal.schema != PROPOSAL_SCHEMA || self.state.outbox.paused {
            return Err("outbox proposal schema, capacity, or controller is unavailable".into());
        }
        if let Some(old) = self.state.outbox.records.get(&proposal.id) {
            let subject = old
                .subject
                .as_ref()
                .filter(|s| {
                    serde_json::to_vec(&s.proposal).ok() == serde_json::to_vec(&proposal).ok()
                })
                .ok_or("outbox proposal identity cannot be reused")?;
            privacy::check_credentials(
                &self.state,
                &serde_json::to_string(subject)
                    .map_err(|_| "outbox subject serialization failed")?,
            )?;
            return Ok(subject.clone());
        }
        if self.state.outbox.records.len() >= MAX_RECORDS {
            return Err("outbox proposal capacity reached".into());
        }
        let prepared = self.prepare_email(access, proposal.message.clone(), keys)?;
        let config = self
            .email_config(&proposal.message.config_sha256, (self.clock)())?
            .clone();
        let mode = mode(&config);
        let (qualification_sha256, draft_qualification_sha256) =
            self.outbox_qualification(access, &proposal, mode)?;
        let budget_sha256 = self.outbox_budget(&proposal, mode)?;
        self.outbox_message_kind(access, &proposal, mode, None)?;
        let day = agents::business_day((self.clock)())?;
        let policy = self.email_policy(&proposal.message.policy_sha256, (self.clock)())?;
        let (total, actors) = self.state.outbox.counts(day, mode, (self.clock)());
        let actor = actor(&proposal.message);
        let cap = if self.state.outbox.cap == 0 {
            5
        } else {
            self.state.outbox.cap
        };
        if total >= cap.min(policy.daily_floor_cap)
            || actors.get(&actor).copied().unwrap_or(0) >= policy.daily_agent_cap
        {
            return Err("outbox floor or actor message cap reached".into());
        }
        let created_at = (self.clock)();
        let mime = mime(self, &prepared, &proposal, created_at)?;
        let retain_until = self
            .state
            .leads
            .get(&proposal.message.lead)
            .ok_or("outbox original retention source is unavailable")?
            .details
            .data
            .retain_until;
        let contact_pins = privacy::contact_history_pins(
            &self.state,
            &format!("email:{}", proposal.message.recipient),
            &self
                .state
                .leads
                .get(&proposal.message.lead)
                .ok_or("outbox contact unavailable")?
                .details
                .account,
        )?;
        let subject = Subject {
            schema: SUBJECT_SCHEMA.into(),
            proposal: proposal.clone(),
            mode,
            message_sha256: prepared.sha256,
            mime_sha256: digest(&mime),
            scope_sha256: prepared.scope_sha256,
            qualification_sha256,
            draft_qualification_sha256,
            budget_sha256,
            controller_epoch: self.state.outbox.epoch,
            reserved_business_day: day,
            reservation_id: random_token(),
            created_at,
            retain_until,
            contact_pins: contact_pins.clone(),
        };
        if mode == Mode::Live {
            self.outbox_live_activation(&subject)?;
        }
        let mut next = self.state.clone();
        let at = created_at;
        next.outbox.records.insert(
            proposal.id.clone(),
            Record {
                id: proposal.id,
                lead: proposal.message.lead.clone(),
                subject_sha256: subject.sha256()?,
                mime_sha256: subject.mime_sha256.clone(),
                subject: Some(subject.clone()),
                mode,
                kind: proposal.kind,
                actor,
                business_day: day,
                expires_at: proposal.message.expires_at,
                maximum_cost_microusd: proposal.maximum_cost_microusd,
                model_reservation_reference: proposal.model_reservation_reference.clone(),
                created_at: at,
                retain_until,
                decision: None,
                phase: Phase::Proposed,
                count_consumed: false,
                attempt: None,
                attempt_started_at: None,
                observation_at: None,
                observation: None,
                minimized_at: None,
                contact_pins,
            },
        );
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        next.outbox.check()?;
        self.persist(next)?;
        Ok(subject)
    }
    /// One portable owner approval subject serves every supported surface.
    pub fn sales_outbox_projection(&mut self, access: &Access) -> Result<Projection> {
        self.refresh()?;
        self.admin(access)?;
        let day = agents::business_day((self.clock)())?;
        let mut rows = vec![];
        for record in self.state.outbox.records.values() {
            let readable = self
                .state
                .leads
                .get(&record.lead)
                .is_some_and(|lead| self.readable(access, lead).is_ok());
            let mut record = record.clone();
            if !readable {
                record.subject = None;
            }
            privacy::check_credentials(
                &self.state,
                &serde_json::to_string(&record)
                    .map_err(|_| "outbox record serialization failed")?,
            )?;
            rows.push(record);
        }
        for item in self.state.outbox.incidents.values() {
            privacy::check_credentials(
                &self.state,
                &serde_json::to_string(item).map_err(|_| "outbox incident serialization failed")?,
            )?;
        }
        for item in self.state.outbox.owner_reports.values() {
            privacy::check_credentials(
                &self.state,
                &serde_json::to_string(item)
                    .map_err(|_| "outbox owner report serialization failed")?,
            )?;
        }
        for item in self.state.outbox.reconciliations.values() {
            privacy::check_credentials(
                &self.state,
                &serde_json::to_string(item)
                    .map_err(|_| "outbox reconciliation serialization failed")?,
            )?;
        }
        Ok(Projection {
            batches: self.state.outbox.batches.clone(),
            schema: PROJECTION_SCHEMA.into(),
            revision: self.state.outbox.revision,
            controller_epoch: self.state.outbox.epoch,
            paused: self.state.outbox.paused,
            cap: if self.state.outbox.cap == 0 {
                5
            } else {
                self.state.outbox.cap
            },
            business_day: day,
            live_messages_and_reservations: self
                .state
                .outbox
                .counts(day, Mode::Live, (self.clock)())
                .0,
            fixture_messages_and_reservations: self
                .state
                .outbox
                .counts(day, Mode::Fixture, (self.clock)())
                .0,
            records: rows,
            incidents: self.state.outbox.incidents.clone(),
            owner_reports: self.state.outbox.owner_reports.clone(),
            reconciliations: self.state.outbox.reconciliations.clone(),
            owner_reports_are_delivery_evidence: false,
            reconciliations_are_delivery_evidence: false,
            outbound_authority: false,
            capabilities: vec![
                Capability {
                    surface: Surface::Terminal,
                    owner_binding_available: true,
                    reason: "current canonical Sales owner credential required".into(),
                },
                Capability {
                    surface: Surface::Phone,
                    owner_binding_available: false,
                    reason: "Sales owner binding is unavailable on this surface".into(),
                },
                Capability {
                    surface: Surface::Lectern,
                    owner_binding_available: false,
                    reason: "Sales owner binding is unavailable on this surface".into(),
                },
            ],
        })
    }
    pub fn sales_outbox_view(&mut self, access: &Access) -> Result<Value> {
        serde_json::to_value(self.sales_outbox_projection(access)?)
            .map_err(|_| "outbox projection serialization failed".into())
    }
    pub fn apply_sales_outbox(
        &mut self,
        access: &Access,
        bytes: &[u8],
        keys: &dyn email::MailboxCredentials,
    ) -> Result<u64> {
        self.refresh()?;
        self.admin(access)?;
        let command: Command = agents::parse(bytes)?;
        if command.schema != COMMAND_SCHEMA {
            return Err("outbox command schema is unavailable".into());
        }
        id(&command.id)?;
        privacy::check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "outbox command is not UTF-8")?,
        )?;
        let hash = digest(bytes);
        if let Some((actor, input, revision)) = self.state.outbox.commands.get(&command.id) {
            return if actor == access.principal() && input == &hash {
                Ok(*revision)
            } else {
                Err("outbox command idempotency conflict".into())
            };
        }
        if command.expected_revision != self.state.outbox.revision {
            return Err("outbox revision conflict".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        match command.operation {
            Operation::Reconcile {
                proposal,
                subject_sha256,
                mime_sha256,
                attempt,
                reference_sha256,
            } => {
                for sha in [&subject_sha256, &mime_sha256, &attempt, &reference_sha256] {
                    token(sha)?;
                }
                if next.outbox.reconciliations.len() >= MAX_RECORDS {
                    return Err("outbox reconciliation history reached its bound".into());
                }
                let row = next
                    .outbox
                    .records
                    .get(&proposal)
                    .ok_or("outbox original uncertain attempt is unavailable")?;
                if !row.count_consumed
                    || !matches!(row.phase, Phase::Unknown | Phase::Accepted)
                    || row.subject_sha256 != subject_sha256
                    || row.mime_sha256 != mime_sha256
                    || row.attempt.as_ref() != Some(&attempt)
                {
                    return Err(
                        "outbox reconciliation requires the exact uncertain native attempt".into(),
                    );
                }
                next.outbox.reconciliations.insert(
                    command.id.clone(),
                    Reconciliation {
                        id: command.id.clone(),
                        owner: access.principal().into(),
                        proposal,
                        subject_sha256,
                        mime_sha256,
                        attempt,
                        reference_sha256,
                        at: now,
                    },
                );
            }

            Operation::Decide {
                proposal,
                subject_sha256,
                approve,
            } => {
                let old = self
                    .state
                    .outbox
                    .records
                    .get(&proposal)
                    .ok_or("outbox proposal is unavailable")?
                    .clone();
                if old.subject_sha256 != subject_sha256
                    || old.phase != Phase::Proposed
                    || old.count_consumed
                {
                    return Err("outbox decision requires the exact unused subject".into());
                }
                if approve {
                    self.outbox_current(
                        access,
                        old.subject
                            .as_ref()
                            .ok_or("outbox original subject was minimized")?,
                        keys,
                    )?;
                    next = self.state.clone();
                }
                let row = next.outbox.records.get_mut(&proposal).unwrap();
                row.decision = Some(Decision {
                    subject_sha256,
                    owner: access.principal().into(),
                    approved: approve,
                    at: now,
                });
                row.phase = if approve {
                    Phase::Approved
                } else {
                    Phase::Rejected
                };
            }
            Operation::OwnerSent {
                proposal,
                subject_sha256,
                message_kind,
                sent_at,
                reference_sha256,
            } => {
                token(&reference_sha256)?;
                if sent_at > now
                    || sent_at < now.saturating_sub(90 * 86400)
                    || next.outbox.owner_reports.len() >= MAX_RECORDS
                {
                    return Err("owner sent report exceeds its time or history bound".into());
                }
                if let Some(sha) = &subject_sha256 {
                    token(sha)?;
                }
                let original = proposal
                    .as_ref()
                    .and_then(|id| self.state.outbox.records.get(id))
                    .cloned();
                let mut mode = Mode::Live;
                let mut counted_in_original_attempt = false;
                let mut supported = false;
                if let Some(record) = original {
                    let exact = subject_sha256.as_ref() == Some(&record.subject_sha256)
                        && record.kind == message_kind;
                    if exact {
                        mode = record.mode;
                    }
                    let human = record.subject.as_ref().is_some_and(|s| {
                        matches!(&s.proposal.message.sender,
                        email::Sender::Human{principal} if principal==access.principal())
                    });
                    let eligible = exact
                        && human
                        && record.phase == Phase::Approved
                        && !record.count_consumed
                        && sent_at >= record.decision.as_ref().map(|d| d.at).unwrap_or(now)
                        && agents::business_day(sent_at)? == record.business_day;
                    let admitted = eligible
                        && self
                            .outbox_intent(access, &record.id, &record.subject_sha256, keys)
                            .is_ok();
                    next = self.state.clone();
                    if admitted {
                        next.outbox.records.get_mut(&record.id).unwrap().phase =
                            Phase::OwnerReported;
                        counted_in_original_attempt = true;
                        supported = true;
                    } else if record.phase.outstanding() {
                        next.outbox.records.get_mut(&record.id).unwrap().phase = Phase::Invalidated;
                    }
                }
                next.outbox.owner_reports.insert(
                    command.id.clone(),
                    OwnerReport {
                        id: command.id.clone(),
                        owner: access.principal().into(),
                        mode,
                        kind: message_kind,
                        sent_at,
                        original_subject_sha256: subject_sha256,
                        reference_sha256: reference_sha256.clone(),
                        counted_in_original_attempt,
                    },
                );
                if !supported {
                    next.outbox.incidents.insert(
                        command.id.clone(),
                        Incident {
                            id: command.id.clone(),
                            kind: IncidentKind::UnsupportedSentClaim,
                            reference_sha256,
                            at: now,
                            resolved_at: None,
                        },
                    );
                    next.outbox.paused = true;
                    next.outbox.batches.reset(now);
                    next.outbox.epoch = next
                        .outbox
                        .epoch
                        .checked_add(1)
                        .ok_or("outbox epoch overflow")?;
                }
            }
            Operation::Pause {
                incident,
                reference_sha256,
            } => {
                token(&reference_sha256)?;
                next.outbox.incidents.insert(
                    command.id.clone(),
                    Incident {
                        id: command.id.clone(),
                        kind: incident,
                        reference_sha256,
                        at: now,
                        resolved_at: None,
                    },
                );
                next.outbox.paused = true;
                next.outbox.batches.reset(now);
                next.outbox.epoch = next
                    .outbox
                    .epoch
                    .checked_add(1)
                    .ok_or("outbox epoch overflow")?;
            }
            Operation::Restart {
                corrected_incidents,
                correction_sha256,
            } => {
                token(&correction_sha256)?;
                let unresolved = next
                    .outbox
                    .incidents
                    .values()
                    .filter(|i| i.resolved_at.is_none())
                    .map(|i| i.id.clone())
                    .collect::<std::collections::BTreeSet<_>>();
                if !next.outbox.paused
                    || unresolved != corrected_incidents.iter().cloned().collect()
                    || unresolved.len() != corrected_incidents.len()
                {
                    return Err("outbox restart requires every current incident and an explicit owner correction".into());
                }
                for id in corrected_incidents {
                    next.outbox.incidents.get_mut(&id).unwrap().resolved_at = Some(now);
                }
                next.outbox.paused = false;
                next.outbox.activation = None;
                next.outbox.epoch = next
                    .outbox
                    .epoch
                    .checked_add(1)
                    .ok_or("outbox epoch overflow")?;
            }
            Operation::Activate { activation } => {
                for sha in [
                    &activation.config_sha256,
                    &activation.policy_sha256,
                    &activation.reply_qualification_sha256,
                    &activation.owner_review_sha256,
                ] {
                    token(sha)?;
                }
                if activation.qualified_actors.is_empty() || activation.qualified_actors.len() > 32
                {
                    return Err(
                        "outbox activation requires a bounded exact actor qualification set".into(),
                    );
                }
                for (actor, qualification) in &activation.qualified_actors {
                    text(actor, 128)?;
                    token(qualification)?;
                }
                if activation.expires_at <= now
                    || next.outbox.reply_qualification.as_ref()
                        != Some(&activation.reply_qualification_sha256)
                {
                    return Err("native qualified reply handler is unavailable".into());
                }
                self.current_sales_reply_qualification(&activation.reply_qualification_sha256)?;
                let config = self.email_config(&activation.config_sha256, now)?;
                if config.provider != email::Provider::Smtp
                    || config.policy_sha256 != activation.policy_sha256
                {
                    return Err("outbox activation requires the exact native SMTP mailbox".into());
                }
                self.outbox_activation_actors(access, &activation)?;
                next = self.state.clone();
                next.outbox.activation = Some(activation);
                next.outbox.epoch = next
                    .outbox
                    .epoch
                    .checked_add(1)
                    .ok_or("outbox epoch overflow")?;
            }
            Operation::GrantBatch { grant } => {
                next = self.outbox_grant_batch(access, grant, keys, now)?;
            }
            Operation::RevokeBatch {
                grant,
                reference_sha256,
            } => batch::revoke(&mut next, &grant, reference_sha256, now)?,
            Operation::RaiseBatch {
                size,
                owner_review_sha256,
            } => batch::raise(&mut next, size, &owner_review_sha256, now)?,
            Operation::RaiseCap {
                cap,
                operating_week_sha256,
                owner_review_sha256,
            } => {
                token(&operating_week_sha256)?;
                token(&owner_review_sha256)?;
                let current = if next.outbox.cap == 0 {
                    5
                } else {
                    next.outbox.cap
                };
                if cap
                    != match current {
                        5 => 10,
                        10 => 20,
                        _ => 0,
                    }
                    || next.outbox.paused
                    || next.outbox.cap_started_at == 0
                    || !clean_week_elapsed(next.outbox.cap_started_at, now)?
                    || !next.outbox.records.values().any(|r| {
                        r.mode == Mode::Live
                            && r.phase == Phase::Delivered
                            && r.created_at >= next.outbox.cap_started_at
                    })
                    || next
                        .outbox
                        .incidents
                        .values()
                        .any(|i| i.at >= next.outbox.cap_started_at)
                {
                    return Err("outbox cap requires a clean real operating week, delivery evidence, and owner review".into());
                }
                let measured = digest(
                    &serde_json::to_vec(
                        &next
                            .outbox
                            .records
                            .values()
                            .filter(|r| {
                                r.mode == Mode::Live && r.created_at >= next.outbox.cap_started_at
                            })
                            .collect::<Vec<_>>(),
                    )
                    .map_err(|_| "outbox operating week serialization failed")?,
                );
                if measured != operating_week_sha256 {
                    return Err("outbox operating week source changed".into());
                }
                next.outbox.cap = cap;
                next.outbox.cap_started_at = now;
                next.outbox.epoch = next
                    .outbox
                    .epoch
                    .checked_add(1)
                    .ok_or("outbox epoch overflow")?;
            }
        }
        if next.outbox.epoch != self.state.outbox.epoch {
            next.outbox.invalidate_pending();
        }
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        next.outbox.commands.insert(
            command.id,
            (access.principal().into(), hash, next.outbox.revision),
        );
        next.outbox.check()?;
        let revision = next.outbox.revision;
        self.admin(access)?;
        self.persist(next)?;
        Ok(revision)
    }
    fn outbox_intent(
        &mut self,
        access: &Access,
        id: &str,
        subject_sha256: &str,
        keys: &dyn email::MailboxCredentials,
    ) -> Result<(Subject, email::Prepared, Vec<u8>)> {
        self.refresh()?;
        self.admin(access)?;
        let row = self
            .state
            .outbox
            .records
            .get(id)
            .ok_or("outbox proposal is unavailable")?
            .clone();
        if row.phase != Phase::Approved
            || row.count_consumed
            || row.subject_sha256 != subject_sha256
            || !row.decision.as_ref().is_some_and(|d| {
                d.approved && d.owner == access.principal() && d.subject_sha256 == subject_sha256
            })
        {
            return Err("outbox dispatch requires one exact unconsumed owner decision".into());
        }
        self.outbox_batch_current(id, subject_sha256, (self.clock)())?;
        let subject = row.subject.ok_or("outbox original subject was minimized")?;
        let (prepared, mime) = self.outbox_current(access, &subject, keys)?;
        let policy = self.email_policy(&subject.proposal.message.policy_sha256, (self.clock)())?;
        let (total, actors) =
            self.state
                .outbox
                .counts(subject.reserved_business_day, subject.mode, (self.clock)());
        let cap = if self.state.outbox.cap == 0 {
            5
        } else {
            self.state.outbox.cap
        };
        if total > cap.min(policy.daily_floor_cap)
            || actors.get(&row.actor).copied().unwrap_or(0) > policy.daily_agent_cap
        {
            return Err("outbox floor or actor cap changed before dispatch".into());
        }
        let mut next = self.state.clone();
        let record = next.outbox.records.get_mut(id).unwrap();
        record.phase = Phase::Unknown;
        record.count_consumed = true;
        record.attempt = Some(random_token());
        record.attempt_started_at = Some((self.clock)());
        if subject.mode == Mode::Live && next.outbox.cap_started_at == 0 {
            next.outbox.cap = 5;
            next.outbox.cap_started_at = (self.clock)();
        }
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        self.persist(next)?;
        Ok((subject, prepared, mime))
    }
    fn outbox_observed(
        &mut self,
        access: &Access,
        id: &str,
        observation: email::smtp::Observation,
    ) -> Result<Record> {
        self.refresh()?;
        self.sales_custody()?;
        let mut next = self.state.clone();
        let now = (self.clock)();
        let row = next
            .outbox
            .records
            .get_mut(id)
            .ok_or("outbox original attempt is unavailable")?;
        if !matches!(row.phase, Phase::DispatchIntent | Phase::Unknown)
            || !row.count_consumed
            || row.attempt.is_none()
            || row.mime_sha256 != observation.message_sha256
        {
            return Err("outbox provider result does not belong to the original intent".into());
        }
        row.phase = match observation.delivery {
            email::Delivery::Accepted => Phase::Accepted,
            email::Delivery::Delivered => {
                return Err("SMTP cannot assert confirmed delivery".into());
            }
            email::Delivery::Failed => Phase::Failed,
            email::Delivery::HardBounce => Phase::HardBounce,
            email::Delivery::AuthenticationFailed => Phase::Failed,
            email::Delivery::Unknown => Phase::Unknown,
            email::Delivery::Cancelled => Phase::Cancelled,
        };
        let lead = row.lead.clone();
        row.observation = Some(observation.clone());
        row.observation_at = Some(now);
        if matches!(
            observation.delivery,
            email::Delivery::AuthenticationFailed | email::Delivery::HardBounce
        ) {
            if observation.delivery == email::Delivery::HardBounce {
                if let Some(lead) = next.leads.get(&lead) {
                    let key = Self::suppression(&next, &lead.contact)?;
                    next.suppressions.entry(key).or_insert(Suppression {
                        at: now,
                        reference_digest: observation.reference_sha256.clone(),
                    });
                }
            }
            let incident = format!("smtp-{}", &digest(id.as_bytes())[..32]);
            next.outbox.incidents.insert(
                incident.clone(),
                Incident {
                    id: incident,
                    kind: if observation.delivery == email::Delivery::HardBounce {
                        IncidentKind::HardBounce
                    } else {
                        IncidentKind::AuthenticationFailure
                    },
                    reference_sha256: observation.reference_sha256,
                    at: now,
                    resolved_at: None,
                },
            );
            next.outbox.paused = true;
            next.outbox.batches.reset((self.clock)());
            next.outbox.epoch = next
                .outbox
                .epoch
                .checked_add(1)
                .ok_or("outbox epoch overflow")?;
        }
        next.outbox.revision = next
            .outbox
            .revision
            .checked_add(1)
            .ok_or("outbox revision overflow")?;
        next.outbox.check()?;
        self.persist(next)?;
        let mut record = self.state.outbox.records[id].clone();
        if self.admin(access).is_err() {
            record.subject = None;
        }
        privacy::check_credentials(
            &self.state,
            &serde_json::to_string(&record).map_err(|_| "outbox receipt serialization failed")?,
        )?;
        Ok(record)
    }
    pub fn dispatch_sales_outbox_fixture(
        &mut self,
        access: &Access,
        id: &str,
        subject_sha256: &str,
        keys: &dyn email::MailboxCredentials,
        transport: &mut email::FakeTransport,
        cancel: &AtomicBool,
    ) -> Result<Record> {
        self.refresh()?;
        self.admin(access)?;
        if self.state.outbox.records.get(id).map(|r| r.mode) != Some(Mode::Fixture) {
            return Err("outbox approval does not belong to this transport".into());
        }
        let (subject, prepared, mime) = self.outbox_intent(access, id, subject_sha256, keys)?;
        if subject.mode != Mode::Fixture {
            return Err("fixture transport cannot consume live SMTP authority".into());
        }
        let result = self.observe_email_fixture(access, prepared, keys, transport, cancel);
        let observed = match result {
            Ok(value) => email::smtp::Observation {
                message_sha256: digest(&mime),
                delivery: value.delivery,
                reply_code: None,
                reference_sha256: value.reference_sha256,
                tls: value.tls,
                authentication: value.authentication,
            },
            Err(_) => email::smtp::Observation {
                message_sha256: digest(&mime),
                delivery: email::Delivery::Unknown,
                reply_code: None,
                reference_sha256: digest(b"fixture provider observation unavailable"),
                tls: email::Validation::Unknown,
                authentication: email::Validation::Unknown,
            },
        };
        self.outbox_observed(access, id, observed)
    }
    /// Persist the single-use attempt, then drop this store before running SMTP.
    pub fn admit_sales_outbox_smtp(
        &mut self,
        access: &Access,
        id: &str,
        subject_sha256: &str,
        keys: &dyn email::MailboxCredentials,
    ) -> Result<SmtpAdmission> {
        self.refresh()?;
        self.admin(access)?;
        if self.state.outbox.records.get(id).map(|r| r.mode) != Some(Mode::Live) {
            return Err("fixture approval cannot admit live SMTP".into());
        }
        let (subject, _prepared, mime) = self.outbox_intent(access, id, subject_sha256, keys)?;
        Ok(SmtpAdmission {
            root: self
                .dir
                .parent()
                .ok_or("host root is unavailable")?
                .to_path_buf(),
            root_directory: self
                .root_directory
                .try_clone()
                .map_err(|_| "host custody unavailable")?,
            sales_directory: self
                .sales_directory
                .try_clone()
                .map_err(|_| "sales custody unavailable")?,
            native_keys: self.native_keys.clone(),
            clock: self.clock,
            access: Access {
                principal: access.principal.clone(),
                token_digest: access.token_digest.clone(),
            },
            attempt: self.state.outbox.records[id]
                .attempt
                .clone()
                .ok_or("native attempt unavailable")?,
            subject,
            mime,
        })
    }
}
/// A native, nonserializable, single-use dispatch capability. It grants no replay.
/// Drop the originating store so urgent owner changes can acquire the sales lock.
pub struct SmtpAdmission {
    root: PathBuf,
    root_directory: File,
    sales_directory: File,
    native_keys: std::sync::Arc<dyn super::super::agent_key::KeyStore>,
    clock: fn() -> u64,
    access: Access,
    attempt: String,
    subject: Subject,
    mime: Vec<u8>,
}
impl Drop for SmtpAdmission {
    fn drop(&mut self) {
        self.mime.fill(0);
    }
}
impl SmtpAdmission {
    fn open(&self) -> Result<Store> {
        agents::native::same_directory(&self.root, &self.root_directory)?;
        agents::native::same_directory(&self.root.join("sales"), &self.sales_directory)?;
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.native_keys.clone();
        let record = store
            .state
            .outbox
            .records
            .get(&self.subject.proposal.id)
            .ok_or("original outbox attempt unavailable")?;
        if !matches!(record.phase, Phase::DispatchIntent | Phase::Unknown)
            || !record.count_consumed
            || record.attempt.as_ref() != Some(&self.attempt)
            || record.subject_sha256 != self.subject.sha256()?
            || record.mime_sha256 != digest(&self.mime)
        {
            return Err("original native outbox attempt changed".into());
        }
        Ok(store)
    }
    fn verify(&self, keys: &dyn email::MailboxCredentials) -> Result<email::Config> {
        let mut store = self.open()?;
        let (_, mime) = store.outbox_current(&self.access, &self.subject, keys)?;
        if mime != self.mime {
            return Err("approved SMTP bytes changed".into());
        }
        Ok(store
            .email_config(&self.subject.proposal.message.config_sha256, (self.clock)())?
            .clone())
    }
    pub async fn execute(
        self,
        keys: &dyn email::MailboxCredentials,
        cancel: &AtomicBool,
    ) -> Result<Record> {
        let result = async {
            let config = self.verify(keys)?;
            let smtp = config.smtp.ok_or("dedicated SMTP transport unavailable")?;
            let secret = keys
                .load(&config.credential_account)
                .map_err(|_| "SMTP credential unavailable")?;
            if digest(secret.expose()) != config.credential_sha256 {
                return Err("SMTP credential changed".into());
            }
            let mut before_data = || {
                self.verify(keys)?;
                Ok(())
            };
            email::smtp::submit(
                &smtp,
                &config.sender,
                &self.subject.proposal.message.recipient,
                &self.mime,
                &secret,
                cancel,
                &mut before_data,
            )
            .await
        }
        .await;
        let observation = result.unwrap_or_else(|_| email::smtp::Observation {
            message_sha256: digest(&self.mime),
            delivery: email::Delivery::Unknown,
            reply_code: None,
            reference_sha256: digest(b"SMTP provider observation unavailable"),
            tls: email::Validation::Unknown,
            authentication: email::Validation::Unknown,
        });
        let mut store = self.open()?;
        store.outbox_observed(&self.access, &self.subject.proposal.id, observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_ramp_waits_for_seven_chicago_calendar_days_across_dst() {
        for (start, end) in [
            ("2026-03-07T18:00:00Z", "2026-03-14T17:00:00Z"),
            ("2026-10-31T17:00:00Z", "2026-11-07T18:00:00Z"),
        ] {
            let start: jiff::Timestamp = start.parse().unwrap();
            let end: jiff::Timestamp = end.parse().unwrap();
            let start = start.as_second() as u64;
            let end = end.as_second() as u64;
            assert!(!clean_week_elapsed(start, end - 1).unwrap());
            assert!(clean_week_elapsed(start, end).unwrap());
        }
    }
}
