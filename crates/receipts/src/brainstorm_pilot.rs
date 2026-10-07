//! Private, consented Brainstorm pilot claims. Validation grants no lookup,
//! publication, installation, task, referral, or payment authority.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "openagents.brainstorm.pilot.v1";
pub const MAX_RECORD_BYTES: usize = 512 * 1024;
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: String,
    pub sha256: String,
}

impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        if self.path.is_empty()
            || self.path.len() > 240
            || self.path.split('/').any(|part| !token(part))
            || !hex(&self.sha256)
        {
            return Err(
                "Evidence needs a relative file path and a lowercase SHA-256 digest.".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Fixture,
    OperatorRecorded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    pub evidence: Reference,
    pub approved_at_ms: u64,
    pub retain_until_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    SearchPeople,
    Rank,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Reported,
    Unknown,
    Unavailable,
    Absent,
}

/// An exact-key projection of a retained normalized observation. The reader
/// checks this against the pinned source, rather than trusting copied scores.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lookup {
    pub id: String,
    pub recorded_at_ms: u64,
    pub observation: Reference,
    pub operation: Operation,
    pub origin: String,
    pub configuration_digest: String,
    pub input_digest: String,
    pub house_pubkey: String,
    pub house_discovered_at_ms: u64,
    pub expires_at_ms: u64,
    pub partial: bool,
    pub relevance: Option<f64>,
    pub influence: Option<f64>,
    pub coverage: Coverage,
    pub responses: Vec<Response>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub endpoint: String,
    pub status: u16,
    pub algorithm: Option<String>,
    pub fetched_at_ms: u64,
    pub expires_at_ms: u64,
    pub input_digest: String,
    pub output_digest: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentState {
    Settled,
    Unknown,
    Failed,
    Reversed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Referral {
        id: String,
        at_ms: u64,
        evidence: Reference,
        lookup: Option<String>,
    },
    Install {
        id: String,
        at_ms: u64,
        evidence: Reference,
        package: String,
        version: String,
        manifest_digest: String,
        release_id: Option<String>,
    },
    AcceptedTask {
        id: String,
        at_ms: u64,
        evidence: Reference,
        task_id: String,
        buyer_id: String,
        artifact_digest: String,
    },
    PaidUse {
        id: String,
        at_ms: u64,
        evidence: Reference,
        accepted_task: String,
        state: PaymentState,
        amount: Option<u64>,
        unit: String,
    },
    RepeatUse {
        id: String,
        at_ms: u64,
        evidence: Reference,
        paid_uses: [String; 2],
    },
}

impl Event {
    pub fn id(&self) -> &str {
        match self {
            Self::Referral { id, .. }
            | Self::Install { id, .. }
            | Self::AcceptedTask { id, .. }
            | Self::PaidUse { id, .. }
            | Self::RepeatUse { id, .. } => id,
        }
    }
    pub fn at_ms(&self) -> u64 {
        match self {
            Self::Referral { at_ms, .. }
            | Self::Install { at_ms, .. }
            | Self::AcceptedTask { at_ms, .. }
            | Self::PaidUse { at_ms, .. }
            | Self::RepeatUse { at_ms, .. } => *at_ms,
        }
    }
    pub fn evidence(&self) -> &Reference {
        match self {
            Self::Referral { evidence, .. }
            | Self::Install { evidence, .. }
            | Self::AcceptedTask { evidence, .. }
            | Self::PaidUse { evidence, .. }
            | Self::RepeatUse { evidence, .. } => evidence,
        }
    }
    pub fn stage(&self) -> &'static str {
        match self {
            Self::Referral { .. } => "referral",
            Self::Install { .. } => "install",
            Self::AcceptedTask { .. } => "accepted_task",
            Self::PaidUse { .. } => "paid_use",
            Self::RepeatUse { .. } => "repeat_use",
        }
    }
}

/// Optional correlation IDs for separately authorized REV-05/REV-26 adapters.
/// Their presence establishes neither a lead nor a referral entitlement.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Joins {
    pub pipeline_lead_id: Option<String>,
    pub referral_attribution_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pilot {
    pub schema: String,
    pub id: String,
    pub basis: Basis,
    pub target_pubkey: String,
    pub consent: Consent,
    pub profile_approval: Option<Reference>,
    pub profile_event: Option<Reference>,
    pub approved_public_links: Vec<String>,
    pub lookups: Vec<Lookup>,
    pub funnel: Vec<Event>,
    pub joins: Joins,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub schema: &'static str,
    pub basis: Basis,
    pub stages: BTreeMap<String, usize>,
    pub missing_stages: Vec<String>,
    pub lookup_coverage: Vec<LookupSummary>,
    pub settled_payment_claims: usize,
    pub repeat_claims: usize,
    pub independently_verified_paid_conversion: bool,
    pub authority_granted: bool,
    pub limitations: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct LookupSummary {
    pub id: String,
    pub operation: Operation,
    pub coverage: Coverage,
    pub expired: bool,
    pub partial: bool,
}

impl Pilot {
    pub fn references(&self) -> Vec<&Reference> {
        let mut refs = vec![&self.consent.evidence];
        refs.extend(self.profile_approval.iter());
        refs.extend(self.profile_event.iter());
        refs.extend(self.lookups.iter().map(|lookup| &lookup.observation));
        refs.extend(self.funnel.iter().map(Event::evidence));
        refs
    }

    /// Checks claims and causal references. File digest and signed profile
    /// verification belong to the caller; this function performs no I/O.
    pub fn validate(&self, checked_at_ms: u64) -> Result<Summary, String> {
        if self.schema != SCHEMA || !token(&self.id) || !hex(&self.target_pubkey) {
            return Err("Invalid Brainstorm pilot schema, ID, or canonical public key.".into());
        }
        if self.consent.approved_at_ms == 0
            || checked_at_ms < self.consent.approved_at_ms
            || self.consent.retain_until_ms <= checked_at_ms
        {
            return Err(
                "Private pilot recording consent is unavailable or its retention expired.".into(),
            );
        }
        if self.lookups.len() > 16
            || self.funnel.len() > 64
            || self.approved_public_links.len() > 16
        {
            return Err("Brainstorm pilot exceeds its record limits.".into());
        }
        for reference in self.references() {
            reference.validate()?;
        }
        if self.profile_event.is_some() && self.profile_approval.is_none()
            || !self.approved_public_links.is_empty() && self.profile_approval.is_none()
        {
            return Err(
                "A profile or public links need separately retained owner approval.".into(),
            );
        }
        for link in &self.approved_public_links {
            if link.len() > 2048
                || !link.starts_with("https://")
                || link.chars().any(char::is_whitespace)
            {
                return Err("Approved public links must be bounded HTTPS URLs.".into());
            }
        }
        for value in [
            &self.joins.pipeline_lead_id,
            &self.joins.referral_attribution_id,
        ]
        .into_iter()
        .flatten()
        {
            if !token(value) {
                return Err("Invalid optional commercial correlation ID.".into());
            }
        }
        let mut ids = BTreeSet::new();
        for lookup in &self.lookups {
            if !token(&lookup.id) || !ids.insert(lookup.id.as_str()) {
                return Err("Pilot lookup IDs must be distinct.".into());
            }
            self.time(lookup.recorded_at_ms, checked_at_ms)?;
            validate_lookup(lookup)?;
        }
        let mut events: BTreeMap<&str, &Event> = BTreeMap::new();
        let mut tasks = BTreeSet::new();
        let mut payment_tasks = BTreeSet::new();
        let mut stages = BTreeMap::from([("discovery".into(), self.lookups.len())]);
        let mut settled_payment_claims = 0;
        let mut repeat_claims = 0;
        for event in &self.funnel {
            if !token(event.id()) || !ids.insert(event.id()) {
                return Err("Pilot event IDs must be distinct from events and lookups.".into());
            }
            self.time(event.at_ms(), checked_at_ms)?;
            match event {
                Event::Referral {
                    lookup: Some(id),
                    at_ms,
                    ..
                } => {
                    if !self
                        .lookups
                        .iter()
                        .any(|l| l.id == *id && l.recorded_at_ms <= *at_ms)
                    {
                        return Err("A referral's lookup must exist before that referral.".into());
                    }
                }
                Event::Install {
                    package,
                    version,
                    manifest_digest,
                    release_id,
                    ..
                } => {
                    let Some((publisher, slug)) = package.split_once(':') else {
                        return Err("Install needs an exact publisher:slug package ID.".into());
                    };
                    if !hex(publisher)
                        || !token(slug)
                        || !token(version)
                        || !digest(manifest_digest)
                        || release_id.as_deref().is_some_and(|id| !hex(id))
                    {
                        return Err(
                            "Install needs an exact package, version, and manifest digest.".into(),
                        );
                    }
                }
                Event::AcceptedTask {
                    task_id,
                    buyer_id,
                    artifact_digest,
                    ..
                } => {
                    if !token(task_id)
                        || !token(buyer_id)
                        || !tasks.insert(task_id)
                        || !digest(artifact_digest)
                    {
                        return Err(
                            "Accepted tasks need distinct task IDs and artifact digests.".into(),
                        );
                    }
                }
                Event::PaidUse {
                    accepted_task,
                    state,
                    amount,
                    unit,
                    ..
                } => {
                    prior(&events, accepted_task, event.at_ms())?;
                    if !matches!(
                        events.get(accepted_task.as_str()),
                        Some(Event::AcceptedTask { .. })
                    ) || !token(unit)
                        || state == &PaymentState::Settled && amount.unwrap_or(0) == 0
                    {
                        return Err("Paid use needs a prior accepted task, a denomination, and a positive settled amount when claimed.".into());
                    }
                    if !payment_tasks.insert(accepted_task) {
                        return Err("A task needs one current payment declaration; replace its state when settlement changes.".into());
                    }
                    if *state == PaymentState::Settled {
                        settled_payment_claims += 1;
                    }
                }
                Event::RepeatUse { paid_uses, .. } => {
                    if paid_uses[0] == paid_uses[1] {
                        return Err("Repeat use needs two distinct settled paid uses.".into());
                    }
                    let mut buyers = BTreeSet::new();
                    for id in paid_uses {
                        prior(&events, id, event.at_ms())?;
                        if !matches!(
                            events.get(id.as_str()),
                            Some(Event::PaidUse {
                                state: PaymentState::Settled,
                                ..
                            })
                        ) {
                            return Err(
                                "Repeat use needs two prior operator-recorded settlements.".into(),
                            );
                        }
                        if let Some(Event::PaidUse { accepted_task, .. }) = events.get(id.as_str())
                            && let Some(Event::AcceptedTask { buyer_id, .. }) =
                                events.get(accepted_task.as_str())
                        {
                            buyers.insert(buyer_id);
                        }
                    }
                    if buyers.len() != 1 {
                        return Err(
                            "Repeat use needs two accepted tasks for the same opaque buyer ID."
                                .into(),
                        );
                    }
                    repeat_claims += 1;
                }
                _ => {}
            }
            *stages.entry(event.stage().into()).or_default() += 1;
            events.insert(event.id(), event);
        }
        let missing_stages = [
            "discovery",
            "referral",
            "install",
            "accepted_task",
            "paid_use",
            "repeat_use",
        ]
        .into_iter()
        .filter(|stage| stages.get(*stage).copied().unwrap_or(0) == 0)
        .map(String::from)
        .collect();
        Ok(Summary {
            schema: SCHEMA,
            basis: self.basis,
            stages,
            missing_stages,
            lookup_coverage: self
                .lookups
                .iter()
                .map(|l| LookupSummary {
                    id: l.id.clone(),
                    operation: l.operation,
                    coverage: l.coverage,
                    expired: checked_at_ms >= l.expires_at_ms,
                    partial: l.partial,
                })
                .collect(),
            settled_payment_claims,
            repeat_claims,
            independently_verified_paid_conversion: false,
            authority_granted: false,
            limitations: vec![
                "Retained files establish local provenance, not upstream attestation.",
                "House identity is a separate unsigned HTTPS observation, not an atomic score binding.",
                "Missing, zero, or expired coverage establishes no capability availability.",
                "Funnel and settlement rows are fixture or operator claims; they do not qualify paid or repeat activation.",
                "Commercial correlation IDs require separately authorized REV-05/REV-26 adapters.",
            ],
        })
    }

    fn time(&self, at: u64, checked: u64) -> Result<(), String> {
        if at < self.consent.approved_at_ms || at > checked || at >= self.consent.retain_until_ms {
            return Err("Pilot evidence time lies outside its approved recording period.".into());
        }
        Ok(())
    }
}

fn prior(events: &BTreeMap<&str, &Event>, id: &str, at: u64) -> Result<(), String> {
    if !events.get(id).is_some_and(|event| event.at_ms() <= at) {
        return Err(
            "Funnel references must name a prior event with an earlier or equal time.".into(),
        );
    }
    Ok(())
}

fn validate_lookup(l: &Lookup) -> Result<(), String> {
    if l.origin.len() > 2048
        || !l.origin.starts_with("https://")
        || !hex(&l.configuration_digest)
        || !hex(&l.input_digest)
        || !hex(&l.house_pubkey)
        || l.responses.len() < 3
        || l.responses.len() > 4
        || l.house_discovered_at_ms > l.recorded_at_ms
        || l.relevance.is_some_and(|n| !n.is_finite())
        || l.influence.is_some_and(|n| !n.is_finite())
    {
        return Err("Invalid lookup provenance or score.".into());
    }
    match (l.coverage, l.influence, l.relevance) {
        (Coverage::Absent, None, None) | (Coverage::Unavailable, None, _) => {}
        (Coverage::Unknown, Some(n), _) if n == 0.0 => {}
        (Coverage::Reported, Some(n), _) if n != 0.0 => {}
        _ => return Err("Zero influence must retain unknown coverage; missing influence is absent or unavailable.".into()),
    }
    if l.operation == Operation::Rank && l.relevance.is_some() {
        return Err("A rank lookup cannot claim search relevance.".into());
    }
    let mut endpoints = BTreeSet::new();
    for r in &l.responses {
        if !endpoints.insert(r.endpoint.as_str())
            || !(100..600).contains(&r.status)
            || !hex(&r.input_digest)
            || !hex(&r.output_digest)
            || r.fetched_at_ms > l.recorded_at_ms
            || r.expires_at_ms < r.fetched_at_ms
            || l.expires_at_ms > r.expires_at_ms
        {
            return Err("Invalid response provenance, expiry, or duplicate endpoint.".into());
        }
        let algorithm = match r.endpoint.as_str() {
            "/.well-known/open-ranking.json" | "/.well-known/nostr.json?name=_" => None,
            "/search/pubkeys" => Some("relevance"),
            "/rank/pubkeys" => Some("graperank"),
            _ => return Err("Unknown Brainstorm read endpoint.".into()),
        };
        if r.algorithm.as_deref() != algorithm {
            return Err("Response algorithm does not match its endpoint.".into());
        }
    }
    let endpoint = if l.operation == Operation::Rank {
        "/rank/pubkeys"
    } else {
        "/search/pubkeys"
    };
    if !l
        .responses
        .iter()
        .any(|r| r.endpoint == endpoint && r.status == 200 && r.input_digest == l.input_digest)
        || !endpoints.contains("/.well-known/open-ranking.json")
        || !endpoints.contains("/.well-known/nostr.json?name=_")
        || l.responses
            .iter()
            .any(|r| r.algorithm.is_none() && r.status != 200)
    {
        return Err("A lookup needs successful operation evidence and separate discovery and house responses.".into());
    }
    Ok(())
}

pub fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest(value: &str) -> bool {
    hex(value.strip_prefix("sha256:").unwrap_or(value))
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

#[cfg(test)]
mod tests;
