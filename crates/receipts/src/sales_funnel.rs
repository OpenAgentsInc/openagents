//! Consented private funnel evidence. Custody and current rights belong to the
//! sales store; task and purchase verification belong to the operating reader.

use crate::service_sale::{self, Reference};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "openagents.sales.funnel-journey.v1";
pub const EXPORT_SCHEMA: &str = "openagents.sales.funnel-export.v1";
pub const MAX_EVENTS: usize = 64;
pub const MAX_FAILURES: usize = 32;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    SelfServe,
    Assisted,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    Fixture,
    OwnerRecords,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    pub evidence: Reference,
    pub at: u64,
    pub expires_at: u64,
    /// A separate permission for reviewed, delayed, count-only aggregation.
    pub aggregate_counts: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub id: String,
    pub offer_version: String,
    pub cohort: String,
    pub lane: Lane,
    pub classification: EvidenceClass,
    pub consent: Consent,
}
/// Stable owner identities survive a later checkpoint or refund. These identify
/// a source to verify; supplying one does not establish a purchase.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FinancialIdentity {
    ServiceSale { sale: String },
    Settlement { key: String },
    Commercial { receipt: String },
}
impl FinancialIdentity {
    pub fn validate(&self) -> Result<(), String> {
        bounded(match self {
            Self::ServiceSale { sale } => sale,
            Self::Settlement { key } => key,
            Self::Commercial { receipt } => receipt,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct TaskIdentity {
    pub manifest_digest: String,
    pub task: String,
    pub task_digest: String,
    pub candidate_digest: String,
    pub trace_digest: String,
    pub customer_acceptance_digest: String,
}
impl TaskIdentity {
    /// A renamed comparison or task label is still the same accepted source.
    pub fn material_key(&self) -> (String, String, String, String) {
        (
            self.task_digest.clone(),
            self.candidate_digest.clone(),
            self.trace_digest.clone(),
            self.customer_acceptance_digest.clone(),
        )
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accept,
    Decline,
    Defer,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Acquisition {
        source: Option<String>,
        evidence: Reference,
    },
    Install {
        client: String,
        evidence: Reference,
    },
    ProviderActivation {
        provider: String,
        evidence: Reference,
    },
    Task {
        manifest: Reference,
        report: Reference,
        attribution: Reference,
        task: String,
    },
    Purchase {
        financial_offer: String,
        entry: String,
        source: FinancialIdentity,
        evidence: Reference,
    },
    PilotAgreed {
        evidence: Reference,
    },
    CustomerDecision {
        decision: Decision,
        evidence: Reference,
    },
}
impl Kind {
    pub fn references(&self) -> Vec<&Reference> {
        match self {
            Self::Task {
                manifest,
                report,
                attribution,
                ..
            } => vec![manifest, report, attribution],
            Self::Acquisition { evidence, .. }
            | Self::Install { evidence, .. }
            | Self::ProviderActivation { evidence, .. }
            | Self::Purchase { evidence, .. }
            | Self::PilotAgreed { evidence }
            | Self::CustomerDecision { evidence, .. } => vec![evidence],
        }
    }
    pub fn validate(&self, lane: Lane) -> Result<(), String> {
        for reference in self.references() {
            reference.validate()?;
        }
        match self {
            Self::Acquisition { source, .. } => {
                if let Some(source) = source {
                    bounded(source)?;
                }
            }
            Self::Install { client, .. } => bounded(client)?,
            Self::ProviderActivation { provider, .. } => bounded(provider)?,
            Self::Task { task, .. } => bounded(task)?,
            Self::Purchase {
                financial_offer,
                entry,
                source,
                ..
            } => {
                bounded(financial_offer)?;
                bounded(entry)?;
                source.validate()?;
                if matches!(source, FinancialIdentity::ServiceSale { .. })
                    && lane == Lane::SelfServe
                {
                    return Err(
                        "an invoiced assisted service cannot become a self-serve purchase".into(),
                    );
                }
            }
            Self::PilotAgreed { .. } | Self::CustomerDecision { .. } if lane != Lane::Assisted => {
                return Err("assisted decisions cannot relabel a self-serve journey".into());
            }
            _ => {}
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    pub id: String,
    pub at: u64,
    pub kind: Kind,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskAttribution {
    pub schema: String,
    pub account: Option<String>,
    pub offer_version: String,
    pub cohort: String,
    pub manifest_digest: String,
    pub task: String,
    pub customer_decision: Option<Reference>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub input: EventInput,
    pub recorded_by: String,
    pub recorded_at: u64,
    pub command_digest: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    UnknownAttribution,
    ActivationUnverified,
    TaskFailed,
    PaymentUnknown,
    Refunded,
    CustomerDeclined,
    Stalled,
    Other,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FailureInput {
    pub event: String,
    pub reason: FailureReason,
    pub responsible_human: String,
    pub next_action: String,
    pub due_at: u64,
    pub evidence: Reference,
    #[serde(default)]
    pub resolved: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub input: FailureInput,
    pub recorded_by: String,
    pub recorded_at: u64,
    pub command_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Journey {
    pub schema: String,
    pub admission: Admission,
    pub pipeline_lead: String,
    pub pipeline_revision_at_admission: u64,
    pub account: String,
    pub admitted_by: String,
    pub admitted_at: u64,
    pub admission_command_digest: String,
    pub admitted_recipients: Vec<String>,
    pub retain_until: u64,
    pub events: Vec<Event>,
    pub failures: Vec<Failure>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub schema: String,
    pub journey: Journey,
    /// Current linked service records prevent a stale paid observation from
    /// concealing a recorded reversal. No product balance is exported.
    pub services: BTreeMap<String, service_sale::Sale>,
    pub exported_by: String,
    pub exported_at: u64,
}
pub fn bounded(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err("invalid bounded funnel identifier".into())
    } else {
        Ok(())
    }
}
fn digest(value: &str) -> Result<(), String> {
    Reference {
        path: "digest".into(),
        sha256: value.into(),
    }
    .validate()
}
impl Admission {
    pub fn validate(&self, now: u64) -> Result<(), String> {
        for value in [&self.id, &self.offer_version, &self.cohort] {
            bounded(value)?;
        }
        self.consent.evidence.validate()?;
        if self.consent.at > now || self.consent.expires_at <= now {
            return Err("funnel enrollment needs explicit current telemetry consent".into());
        }
        Ok(())
    }
}
impl Journey {
    pub fn recipient(&self, human: &str) -> bool {
        self.admitted_recipients.contains(&format!("human:{human}"))
    }
    pub fn validate(&self) -> Result<(), String> {
        self.admission.validate(self.admitted_at)?;
        if self.schema != SCHEMA
            || self.pipeline_revision_at_admission == 0
            || self.retain_until <= self.admitted_at
            || self.events.len() > MAX_EVENTS
            || self.failures.len() > MAX_FAILURES
            || self.admitted_recipients.is_empty()
            || self.admitted_recipients.len() > 16
            || !self.recipient(&self.admitted_by)
        {
            return Err("unsupported or oversized consented funnel journey".into());
        }
        for value in [&self.pipeline_lead, &self.account, &self.admitted_by] {
            bounded(value)?;
        }
        for value in &self.admitted_recipients {
            bounded(value)?;
        }
        digest(&self.admission_command_digest)?;
        let mut ids = BTreeSet::new();
        let mut previous = self.admission.consent.at;
        let mut recorded_at = self.admitted_at;
        for event in &self.events {
            bounded(&event.input.id)?;
            bounded(&event.recorded_by)?;
            digest(&event.command_digest)?;
            event.input.kind.validate(self.admission.lane)?;
            if !ids.insert(&event.input.id)
                || event.input.at < previous
                || event.input.at >= self.admission.consent.expires_at
                || event.recorded_at >= self.admission.consent.expires_at
                || event.recorded_at < recorded_at
                || event.recorded_at < event.input.at
                || event.recorded_at >= self.retain_until
                || !self.recipient(&event.recorded_by)
            {
                return Err("funnel event exceeds its immutable consented history".into());
            }
            previous = event.input.at;
            recorded_at = event.recorded_at;
        }
        let mut previous = self.admitted_at;
        for failure in &self.failures {
            let input = &failure.input;
            for value in [
                &input.event,
                &input.responsible_human,
                &input.next_action,
                &failure.recorded_by,
            ] {
                bounded(value)?;
            }
            input.evidence.validate()?;
            digest(&failure.command_digest)?;
            if !ids.contains(&input.event)
                || input.due_at < failure.recorded_at
                || input.due_at >= self.retain_until
                || failure.recorded_at < previous
                || failure.recorded_at >= self.retain_until
                || !self.recipient(&failure.recorded_by)
                || !self.recipient(&input.responsible_human)
            {
                return Err("failed conversion needs retained accountable ownership and a dated next action".into());
            }
            previous = failure.recorded_at;
        }
        Ok(())
    }
}
impl Export {
    pub fn validate(&self, now: u64) -> Result<(), String> {
        self.journey.validate()?;
        bounded(&self.exported_by)?;
        if self.schema != EXPORT_SCHEMA
            || self.exported_at > now
            || self.exported_at < self.journey.admitted_at
            || now >= self.journey.retain_until
            || self.exported_at >= self.journey.retain_until
            || !self.journey.recipient(&self.exported_by)
            || self
                .journey
                .events
                .iter()
                .any(|e| e.recorded_at > self.exported_at)
            || self
                .journey
                .failures
                .iter()
                .any(|e| e.recorded_at > self.exported_at)
            || self.services.len() > MAX_EVENTS
        {
            return Err("funnel export exceeds current retention or disclosure".into());
        }
        let requested = self
            .journey
            .events
            .iter()
            .filter_map(|event| {
                if let Kind::Purchase {
                    source: FinancialIdentity::ServiceSale { sale },
                    ..
                } = &event.input.kind
                {
                    Some(sale)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();
        for (id, sale) in &self.services {
            sale.validate()?;
            if !requested.contains(id)
                || id != &sale.admission.id
                || sale.pipeline_lead != self.journey.pipeline_lead
                || sale.account != self.journey.account
                || sale.admission.offer_version != self.journey.admission.offer_version
                || sale.retain_until <= now
                || !sale
                    .admitted_recipients
                    .contains(&format!("human:{}", self.exported_by))
            {
                return Err("funnel service snapshot exceeds the original commercial scope".into());
            }
        }
        Ok(())
    }
}
