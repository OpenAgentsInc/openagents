//! Reviewed sales claims over current retained contracts, existing price books,
//! and replayed Gym evidence. The canonical host store owns review history;
//! this adapter creates no benchmark, price, payment, or outbound authority.

use super::*;
use gym::sales_evidence::{self, Reference};
use route_contract::price_book::{Placement, PriceBook, Quote};
use std::collections::BTreeSet;
use std::path::Component;

pub const SOURCE_SCHEMA: &str = "openagents.sales.claim-source.v1";
pub const CLAIM_SCHEMA: &str = "openagents.sales.claim.v1";
pub const DRAFT_SCHEMA: &str = "openagents.sales.claim-draft.v1";
const MAX_REVISIONS: usize = 256;
const MAX_DECISIONS: usize = 1024;
const MAX_FILE: usize = 1024 * 1024;
const MAX_VALIDITY: u64 = 90 * 86400;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub id: String,
    pub revision: u64,
}
impl Pin {
    fn key(&self) -> Result<String> {
        id(&self.id)?;
        if self.revision == 0 {
            return Err("claim revision must be positive".into());
        }
        Ok(format!("{}:{}", self.id, self.revision))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub product: String,
    pub offer_version: String,
    /// The exact qualified or reviewed source release, never a floating branch.
    pub release: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    Implemented,
    Available,
    Proposed,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Evidence {
    /// This exact sentence is a human-reviewed fact about the pinned contract.
    /// The owner must verify its meaning and disclosure rights; a hash does not.
    Capability {
        contract: Reference,
        reviewed_fact: String,
    },
    RetailPrice {
        book: Reference,
        computer: String,
        task: String,
        max_seconds: u64,
    },
    /// Reads proposed terms from the existing manual pilot template, not another
    /// price book. Publication requires a separate exact commercial activation.
    PilotOffer { template: Reference },
    Comparison {
        manifest: Reference,
        public_review: Reference,
        report_sha256: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInput {
    pub schema: String,
    pub pin: Pin,
    pub scope: Scope,
    /// Explicit current source root. Reads reuse this recorded root, never a
    /// caller-selected historical replacement or discovered home directory.
    pub root: PathBuf,
    pub evidence: Evidence,
    pub readiness: Readiness,
    pub limits: Vec<String>,
    pub review: Reference,
    pub expires_at: u64,
    /// Commercial or launch qualification is separately recorded over the exact
    /// source bytes, scope, reviewer, payer, and expiry. Ordinary review is not it.
    pub activation: Option<Reference>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activation {
    pub schema: String,
    pub scope: Scope,
    pub source_sha256: String,
    pub reviewer: String,
    pub approved: bool,
    pub reviewed_at: u64,
    pub expires_at: u64,
    pub payer: String,
    pub qualification: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub input: SourceInput,
    pub owner: String,
    pub reviewed_at: u64,
    pub input_sha256: String,
    pub reviewed: Verdict,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Capability,
    Price,
    Comparison,
    Launch,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimInput {
    pub pin: Pin,
    pub source: Pin,
    pub purpose: Purpose,
    /// The currently maintained playbook artifact, not an engram answer.
    pub playbook: Reference,
    pub expires_at: u64,
    pub review: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRecord {
    pub schema: String,
    pub input: ClaimInput,
    pub owner: String,
    pub reviewed_at: u64,
    pub input_sha256: String,
    pub reviewed: Verdict,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Rejection {
    UnknownClaim,
    UnknownSource,
    StaleRevision,
    Withdrawn,
    Expired,
    ReleaseMismatch,
    MissingEvidence,
    ChangedEvidence,
    UnqualifiedLaunch,
    ProposedCommercialTerms,
    WrongPurpose,
    UnsupportedComparison,
    SourceUnavailable,
    DraftChanged,
    UnsupportedWording,
    PriceVersionReuse,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceTerms {
    pub offer_version: String,
    pub currency: String,
    pub currency_scale: u64,
    pub service_fee_minor_units: u64,
    pub invoice_due_calendar_days: u64,
    pub resource_payer: String,
    pub promotional_credits: u64,
    pub provider_subsidy_minor_units: u64,
    pub free_discovery_minutes_cap: u64,
    pub operator_minutes_cap: u64,
    pub attempt_minutes_cap: u64,
    pub check_minutes_cap: u64,
    pub repair_attempts_cap: u64,
    pub duration_calendar_days_cap: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PriceTerms {
    Retail { quote: Quote },
    Service { terms: ServiceTerms },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allowed {
    pub wording: String,
    pub scope: Scope,
    pub readiness: Readiness,
    pub limits: Vec<String>,
    pub price: Option<PriceTerms>,
    pub comparison: Option<sales_evidence::PublicReport>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Verdict {
    Allowed {
        claim: Allowed,
    },
    /// Proposed terms may be inspected privately but never inserted in a draft.
    Unavailable {
        reason: Rejection,
        proposed_price: Option<PriceTerms>,
    },
    Rejected {
        reason: Rejection,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimView {
    pub pin: Pin,
    pub reviewed_sha256: Option<String>,
    pub verdict: Verdict,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftPin {
    pub claim: Pin,
    pub reviewed_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub schema: String,
    pub id: String,
    pub claims: Vec<DraftPin>,
    /// Composed only from reviewed clauses; free-form model text is not a claim.
    pub content: String,
    pub sha256: String,
    pub created_by: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub sequence: u64,
    pub at: u64,
    pub actor: String,
    pub subject: String,
    pub operation: String,
    pub reason: Option<Rejection>,
    pub reference_sha256: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    sources: BTreeMap<String, SourceRecord>,
    source_heads: BTreeMap<String, u64>,
    claims: BTreeMap<String, ClaimRecord>,
    claim_heads: BTreeMap<String, u64>,
    withdrawn: BTreeSet<String>,
    drafts: BTreeMap<String, Draft>,
    decisions: Vec<Decision>,
}
impl State {
    pub(super) fn check(&self) -> Result<()> {
        if self.sources.len() > MAX_REVISIONS
            || self.claims.len() > MAX_REVISIONS
            || self.drafts.len() > MAX_REVISIONS
            || self.decisions.len() > MAX_DECISIONS
            || self.withdrawn.len() > 2 * MAX_REVISIONS
            || self.source_heads.len() > MAX_REVISIONS
            || self.claim_heads.len() > MAX_REVISIONS
        {
            return Err("claims register exceeds bound".into());
        }
        for (key, source) in &self.sources {
            if key != &source.input.pin.key()?
                || digest(&serde_json::to_vec(&source.input).map_err(|e| e.to_string())?)
                    != source.input_sha256
            {
                return Err("claims source revision is inconsistent".into());
            }
        }
        for (key, claim) in &self.claims {
            if key != &claim.input.pin.key()?
                || claim.schema != CLAIM_SCHEMA
                || digest(&serde_json::to_vec(&claim.input).map_err(|e| e.to_string())?)
                    != claim.input_sha256
            {
                return Err("claim revision is inconsistent".into());
            }
        }
        Ok(())
    }
    fn decision(
        &mut self,
        actor: &str,
        subject: String,
        operation: &str,
        reason: Option<Rejection>,
        reference: &str,
        now: u64,
    ) -> Result<()> {
        if self.decisions.len() >= MAX_DECISIONS {
            return Err("claims review history bound reached".into());
        }
        self.decisions.push(Decision {
            sequence: self.decisions.len() as u64 + 1,
            at: now,
            actor: actor.into(),
            subject,
            operation: operation.into(),
            reason,
            reference_sha256: digest(reference.as_bytes()),
        });
        Ok(())
    }
}
fn hash(value: &str, len: usize) -> Result<()> {
    if value.len() != len
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("invalid exact source digest or release".into());
    }
    Ok(())
}
fn expiry(at: u64, now: u64) -> Result<()> {
    if at <= now || at > now.saturating_add(MAX_VALIDITY) {
        return Err("review expiry must be within 90 days".into());
    }
    Ok(())
}
fn read(root: &Path, reference: &Reference) -> std::result::Result<Vec<u8>, Rejection> {
    hash(&reference.sha256, 64).map_err(|_| Rejection::MissingEvidence)?;
    let relative = Path::new(&reference.path);
    if reference.path.len() > 512
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Rejection::MissingEvidence);
    }
    let mut path = root.to_path_buf();
    let root_meta = std::fs::symlink_metadata(root).map_err(|_| Rejection::MissingEvidence)?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return Err(Rejection::MissingEvidence);
    }
    for part in relative.components() {
        path.push(part);
        let meta = std::fs::symlink_metadata(&path).map_err(|_| Rejection::MissingEvidence)?;
        if meta.file_type().is_symlink() {
            return Err(Rejection::MissingEvidence);
        }
    }
    let meta = std::fs::metadata(&path).map_err(|_| Rejection::MissingEvidence)?;
    if !meta.is_file() || meta.len() > MAX_FILE as u64 {
        return Err(Rejection::MissingEvidence);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| Rejection::MissingEvidence)?
        .take((MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Rejection::MissingEvidence)?;
    if bytes.is_empty() || bytes.len() > MAX_FILE {
        return Err(Rejection::MissingEvidence);
    }
    if digest(&bytes) != reference.sha256 {
        return Err(Rejection::ChangedEvidence);
    }
    Ok(bytes)
}
fn terms(bytes: &[u8], scope: &Scope) -> std::result::Result<ServiceTerms, Rejection> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Rejection::SourceUnavailable)?;
    if value["schema"] != "openagents.sales.pilot-kit.v1"
        || value["offer"] != scope.offer_version
        || value["agreement"]["offer_version"] != scope.offer_version
        || value["agreement"]["commercial"]["kind"] != "service_invoice_after_acceptance"
        || value["agreement"]["commercial"]["provider_payer"] != "buyer"
    {
        return Err(Rejection::SourceUnavailable);
    }
    let commercial = &value["agreement"]["commercial"];
    let time = &value["agreement"]["time"];
    let number = |map: &serde_json::Value, name: &str| {
        map[name].as_u64().ok_or(Rejection::SourceUnavailable)
    };
    let result = ServiceTerms {
        offer_version: scope.offer_version.clone(),
        currency: commercial["currency"]
            .as_str()
            .ok_or(Rejection::SourceUnavailable)?
            .into(),
        currency_scale: number(commercial, "currency_scale")?,
        service_fee_minor_units: number(commercial, "service_fee_minor_units")?,
        invoice_due_calendar_days: number(commercial, "invoice_due_calendar_days")?,
        resource_payer: "buyer".into(),
        promotional_credits: number(commercial, "promotional_credits")?,
        provider_subsidy_minor_units: number(commercial, "provider_subsidy_minor_units")?,
        free_discovery_minutes_cap: number(commercial, "free_discovery_minutes_cap")?,
        operator_minutes_cap: number(time, "operator_minutes_cap")?,
        attempt_minutes_cap: number(time, "attempt_minutes_cap")?,
        check_minutes_cap: number(time, "check_minutes_cap")?,
        repair_attempts_cap: number(time, "repair_attempts_cap")?,
        duration_calendar_days_cap: number(time, "duration_calendar_days_cap")?,
    };
    if result.currency != "USD"
        || result.currency_scale != 100
        || result.promotional_credits != 0
        || result.provider_subsidy_minor_units != 0
        || result.invoice_due_calendar_days == 0
        || result.free_discovery_minutes_cap == 0
        || result.operator_minutes_cap == 0
        || result.attempt_minutes_cap == 0
        || result.check_minutes_cap == 0
        || result.duration_calendar_days_cap == 0
        || commercial["product_funding"]
            .as_array()
            .is_none_or(|v| !v.is_empty())
    {
        return Err(Rejection::SourceUnavailable);
    }
    Ok(result)
}
fn price_book(bytes: &[u8]) -> std::result::Result<PriceBook, Rejection> {
    // The maintained route-contract fixture wraps its authoritative book beside
    // quote/settlement cases. Also accept that owner's ordinary book document.
    if let Ok(book) = serde_json::from_slice::<PriceBook>(bytes) {
        return Ok(book);
    }
    let fixture: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Rejection::SourceUnavailable)?;
    serde_json::from_value(fixture["book"].clone()).map_err(|_| Rejection::SourceUnavailable)
}
fn activation(
    source: &SourceRecord,
    primary_digest: &str,
    now: u64,
) -> std::result::Result<(), Rejection> {
    let reference = source
        .input
        .activation
        .as_ref()
        .ok_or(Rejection::UnqualifiedLaunch)?;
    let bytes = read(&source.input.root, reference)?;
    let record: Activation =
        serde_json::from_slice(&bytes).map_err(|_| Rejection::UnqualifiedLaunch)?;
    if record.schema != "openagents.sales.claim-activation.v1"
        || record.scope != source.input.scope
        || record.source_sha256 != primary_digest
        || record.reviewer != source.owner
        || !record.approved
        || record.reviewed_at > source.reviewed_at
        || record.expires_at <= now
        || record.expires_at > source.input.expires_at
        || record.payer
            != if matches!(
                source.input.evidence,
                Evidence::RetailPrice { .. } | Evidence::PilotOffer { .. }
            ) {
                "buyer"
            } else {
                "not_applicable"
            }
    {
        return Err(Rejection::UnqualifiedLaunch);
    }
    read(&source.input.root, &record.qualification)?;
    Ok(())
}
fn evaluate(source: &SourceRecord, purpose: Purpose, now: u64) -> Verdict {
    let result = (|| -> std::result::Result<Allowed, Rejection> {
        if source.input.expires_at <= now {
            return Err(Rejection::Expired);
        }
        read(&source.input.root, &source.input.review)?;
        let mut allowed = Allowed {
            wording: String::new(),
            scope: source.input.scope.clone(),
            readiness: source.input.readiness,
            limits: source.input.limits.clone(),
            price: None,
            comparison: None,
        };
        match &source.input.evidence {
            Evidence::Capability {
                contract,
                reviewed_fact,
            } => {
                if !matches!(purpose, Purpose::Capability | Purpose::Launch) {
                    return Err(Rejection::WrongPurpose);
                }
                let lower = reviewed_fact.to_ascii_lowercase();
                if [
                    "guarantee",
                    "available now",
                    "launch ready",
                    "production ready",
                    "cheaper",
                    "faster",
                    "savings",
                    "100%",
                ]
                .iter()
                .any(|word| lower.contains(word))
                    || lower.contains('$')
                    || lower.contains('€')
                    || lower.split(|c: char| !c.is_ascii_alphabetic()).any(|word| {
                        [
                            "usd", "eur", "gbp", "sat", "sats", "msat", "msats", "credit",
                            "credits", "price", "prices", "fee", "fees", "charge", "charges",
                            "cost", "costs", "free", "refund", "refunds", "discount", "subsidy",
                            "dollar", "dollars",
                        ]
                        .contains(&word)
                    })
                {
                    return Err(Rejection::UnsupportedWording);
                }
                read(&source.input.root, contract)?;
                if purpose == Purpose::Launch || source.input.readiness == Readiness::Available {
                    if source.input.readiness != Readiness::Available {
                        return Err(Rejection::UnqualifiedLaunch);
                    }
                    activation(source, &contract.sha256, now)?;
                }
                if matches!(
                    source.input.readiness,
                    Readiness::Unknown | Readiness::Proposed
                ) {
                    return Err(Rejection::SourceUnavailable);
                }
                allowed.wording = reviewed_fact.clone();
            }
            Evidence::RetailPrice {
                book,
                computer,
                task,
                max_seconds,
            } => {
                if purpose != Purpose::Price {
                    return Err(Rejection::WrongPurpose);
                }
                let parsed = price_book(&read(&source.input.root, book)?)?;
                if parsed.effective_at > now {
                    return Err(Rejection::SourceUnavailable);
                }
                let quote = parsed
                    .quote(
                        &Placement::Retail {
                            computer: computer.clone(),
                            task: task.clone(),
                        },
                        *max_seconds,
                        None,
                    )
                    .map_err(|_| Rejection::SourceUnavailable)?
                    .ok_or(Rejection::SourceUnavailable)?;
                allowed.wording = format!(
                    "Price book {} quotes at most {} sats ({} credits) for {} seconds of {} on {}. Model use uses the buyer's own provider key and is billed separately. This quote is a ceiling, not observed cost or a funded qualification.",
                    quote.version,
                    quote.max_sats,
                    quote.max_credits,
                    quote.max_seconds,
                    quote.task,
                    quote.computer
                );
                allowed.price = Some(PriceTerms::Retail { quote });
                if source.input.readiness != Readiness::Available {
                    return Err(Rejection::ProposedCommercialTerms);
                }
                activation(source, &book.sha256, now)?;
            }
            Evidence::PilotOffer { template } => {
                if purpose != Purpose::Price {
                    return Err(Rejection::WrongPurpose);
                }
                let parsed = terms(&read(&source.input.root, template)?, &source.input.scope)?;
                allowed.wording = format!(
                    "Offer {} has a reviewed service fee of USD {}.{:02}, invoiced only after accepted delivery and due within {} calendar days. The buyer pays their provider separately; this service supplies zero product credits or provider subsidy.",
                    parsed.offer_version,
                    parsed.service_fee_minor_units / 100,
                    parsed.service_fee_minor_units % 100,
                    parsed.invoice_due_calendar_days
                );
                allowed.price = Some(PriceTerms::Service { terms: parsed });
                if source.input.readiness != Readiness::Available {
                    return Err(Rejection::ProposedCommercialTerms);
                }
                activation(source, &template.sha256, now)?;
            }
            Evidence::Comparison {
                manifest,
                public_review,
                report_sha256,
            } => {
                if purpose != Purpose::Comparison {
                    return Err(Rejection::WrongPurpose);
                }
                let bytes = read(&source.input.root, manifest)?;
                // Rebuild every attempt and nested source, including pinned price,
                // failed/repair rows and independent/customer acceptance.
                let report = sales_evidence::rebuild(&source.input.root, &bytes)
                    .map_err(|_| Rejection::UnsupportedComparison)?;
                if report.manifest.source_revision != source.input.scope.release
                    || report.manifest.offer_version != source.input.scope.offer_version
                {
                    return Err(Rejection::ReleaseMismatch);
                }
                let rebuilt = serde_json::to_vec_pretty(&report)
                    .map_err(|_| Rejection::UnsupportedComparison)?;
                if digest(&rebuilt) != *report_sha256 || report.candidate.accepted_tasks == 0 {
                    return Err(Rejection::UnsupportedComparison);
                }
                let review: sales_evidence::PublicReview =
                    serde_json::from_slice(&read(&source.input.root, public_review)?)
                        .map_err(|_| Rejection::UnsupportedComparison)?;
                if review.reviewer != source.owner {
                    return Err(Rejection::UnsupportedComparison);
                }
                let projection = sales_evidence::project(&rebuilt, &review)
                    .map_err(|_| Rejection::UnsupportedComparison)?;
                allowed.wording = format!(
                    "In one frozen inventory of {} tasks, the baseline used {} attempts; the candidate used {} attempts including {} repairs and {} retries, with {} failed checks and {} independently checked customer-accepted tasks. This comparison establishes no general savings, guaranteed outcome, or deployed routing improvement.",
                    report.manifest.tasks.len(),
                    report.baseline.attempts,
                    report.candidate.attempts,
                    report.candidate.repairs,
                    report.candidate.retries,
                    report.candidate.failed_checks,
                    report.candidate.accepted_tasks
                );
                allowed.limits.extend(projection.limitations.clone());
                allowed.comparison = Some(projection);
                // Accepted comparison evidence never establishes deployed
                // product availability, even when a reviewer supplied that tag.
                allowed.readiness = Readiness::Unknown;
            }
        }
        Ok(allowed)
    })();
    match result {
        Ok(claim) => Verdict::Allowed { claim },
        Err(
            reason @ (Rejection::SourceUnavailable
            | Rejection::ProposedCommercialTerms
            | Rejection::UnqualifiedLaunch),
        ) => {
            let proposed_price = match &source.input.evidence {
                Evidence::PilotOffer { template } => read(&source.input.root, template)
                    .ok()
                    .and_then(|b| terms(&b, &source.input.scope).ok())
                    .map(|terms| PriceTerms::Service { terms }),
                Evidence::RetailPrice {
                    book,
                    computer,
                    task,
                    max_seconds,
                } => read(&source.input.root, book)
                    .ok()
                    .and_then(|b| price_book(&b).ok())
                    .and_then(|b| {
                        b.quote(
                            &Placement::Retail {
                                computer: computer.clone(),
                                task: task.clone(),
                            },
                            *max_seconds,
                            None,
                        )
                        .ok()
                        .flatten()
                    })
                    .map(|quote| PriceTerms::Retail { quote }),
                _ => None,
            };
            Verdict::Unavailable {
                reason,
                proposed_price,
            }
        }
        Err(reason) => Verdict::Rejected { reason },
    }
}
fn rejection(verdict: &Verdict) -> Option<Rejection> {
    match verdict {
        Verdict::Allowed { .. } => None,
        Verdict::Rejected { reason } | Verdict::Unavailable { reason, .. } => Some(reason.clone()),
    }
}
fn price_identity(verdict: &Verdict) -> Option<(String, String)> {
    let price = match verdict {
        Verdict::Allowed { claim } => claim.price.as_ref(),
        Verdict::Unavailable { proposed_price, .. } => proposed_price.as_ref(),
        Verdict::Rejected { .. } => None,
    }?;
    Some(match price {
        PriceTerms::Retail { quote } => {
            (format!("retail:{}", quote.version), quote.book.to_string())
        }
        PriceTerms::Service { terms } => (
            format!("service:{}", terms.offer_version),
            digest(&serde_json::to_vec(terms).ok()?),
        ),
    })
}
impl Store {
    pub fn review_claim_source(
        &mut self,
        access: &Access,
        mut input: SourceInput,
    ) -> Result<SourceRecord> {
        self.refresh()?;
        self.admin(access)?;
        let key = input.pin.key()?;
        if serde_json::to_vec(&input).map_err(|e| e.to_string())?.len() > 32 * 1024 {
            return Err("claim source input exceeds bound".into());
        }
        if input.schema != SOURCE_SCHEMA {
            return Err("unsupported claim source schema".into());
        }
        text(&input.scope.product, 128)?;
        text(&input.scope.offer_version, 128)?;
        hash(&input.scope.release, 40)?;
        if input.limits.is_empty() || input.limits.len() > 16 {
            return Err("claim source must retain bounded known limits".into());
        }
        for limit in &input.limits {
            text(limit, 512)?;
        }
        if let Evidence::Capability { reviewed_fact, .. } = &input.evidence {
            text(reviewed_fact, 1024)?;
        }
        let root_meta = std::fs::symlink_metadata(&input.root).map_err(|e| e.to_string())?;
        if !input.root.is_absolute() || !root_meta.is_dir() || root_meta.file_type().is_symlink() {
            return Err("claim sources need an explicit regular absolute root".into());
        }
        input.root = input.root.canonicalize().map_err(|e| e.to_string())?;
        let input_sha256 = digest(&serde_json::to_vec(&input).map_err(|e| e.to_string())?);
        if let Some(old) = self.state.claims.sources.get(&key) {
            if old.input_sha256 == input_sha256 {
                return Ok(old.clone());
            }
            return Err("claim source revision is immutable".into());
        }
        expiry(input.expires_at, (self.clock)())?;
        if input.pin.revision
            != self
                .state
                .claims
                .source_heads
                .get(&input.pin.id)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("source revision overflow")?
        {
            return Err("claim source revision is not next".into());
        }
        // Review reads current exact bytes. A rejected head blocks older claims
        // and remains an immutable negative record until a new revision passes.
        let mut record = SourceRecord {
            input,
            owner: access.principal.clone(),
            reviewed_at: (self.clock)(),
            input_sha256,
            reviewed: Verdict::Unavailable {
                reason: Rejection::SourceUnavailable,
                proposed_price: None,
            },
        };
        let purpose = match record.input.evidence {
            Evidence::Capability { .. } => Purpose::Capability,
            Evidence::Comparison { .. } => Purpose::Comparison,
            _ => Purpose::Price,
        };
        let mut verdict = evaluate(&record, purpose, (self.clock)());
        if let Some((version, content)) = price_identity(&verdict) {
            // Retain reviewed price references, not a second book. The owning
            // book/offer version cannot silently acquire different terms.
            if self
                .state
                .claims
                .sources
                .values()
                .filter_map(|s| price_identity(&s.reviewed))
                .any(|(old_version, old_content)| old_version == version && old_content != content)
            {
                verdict = Verdict::Rejected {
                    reason: Rejection::PriceVersionReuse,
                };
            }
        }
        record.reviewed = verdict.clone();
        let reason = rejection(&verdict);
        let mut next = self.state.clone();
        next.claims.decision(
            &access.principal,
            format!("source:{key}"),
            "review_source",
            reason.clone(),
            &record.input_sha256,
            (self.clock)(),
        )?;
        if next.claims.sources.len() >= MAX_REVISIONS {
            return Err("claim source bound reached".into());
        }
        next.claims
            .source_heads
            .insert(record.input.pin.id.clone(), record.input.pin.revision);
        next.claims.sources.insert(key, record.clone());
        self.persist(next)?;
        Ok(record)
    }
    pub fn review_claim(&mut self, access: &Access, input: ClaimInput) -> Result<ClaimRecord> {
        self.refresh()?;
        self.admin(access)?;
        let key = input.pin.key()?;
        input.source.key()?;
        if serde_json::to_vec(&input).map_err(|e| e.to_string())?.len() > 32 * 1024 {
            return Err("claim input exceeds bound".into());
        }
        let input_sha256 = digest(&serde_json::to_vec(&input).map_err(|e| e.to_string())?);
        if let Some(old) = self.state.claims.claims.get(&key) {
            if old.input_sha256 == input_sha256 {
                return Ok(old.clone());
            }
            return Err("claim revision is immutable".into());
        }
        expiry(input.expires_at, (self.clock)())?;
        if input.pin.revision
            != self
                .state
                .claims
                .claim_heads
                .get(&input.pin.id)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("claim revision overflow")?
        {
            return Err("claim revision is not next".into());
        }
        if self.state.claims.claims.len() >= MAX_REVISIONS {
            return Err("claim revision bound reached".into());
        }
        let verdict = self.evaluate_claim(&input);
        let record = ClaimRecord {
            schema: CLAIM_SCHEMA.into(),
            input,
            owner: access.principal.clone(),
            reviewed_at: (self.clock)(),
            input_sha256,
            reviewed: verdict,
        };
        let mut next = self.state.clone();
        next.claims.decision(
            &access.principal,
            format!("claim:{key}"),
            "review_claim",
            rejection(&record.reviewed),
            &record.input_sha256,
            (self.clock)(),
        )?;
        next.claims
            .claim_heads
            .insert(record.input.pin.id.clone(), record.input.pin.revision);
        next.claims.claims.insert(key, record.clone());
        self.persist(next)?;
        Ok(record)
    }
    fn evaluate_claim(&self, input: &ClaimInput) -> Verdict {
        let result = (|| -> std::result::Result<Verdict, Rejection> {
            if input.expires_at <= (self.clock)() {
                return Err(Rejection::Expired);
            }
            let key = input.source.key().map_err(|_| Rejection::UnknownSource)?;
            if self
                .state
                .claims
                .withdrawn
                .contains(&format!("source:{key}"))
            {
                return Err(Rejection::Withdrawn);
            }
            if !self
                .state
                .claims
                .source_heads
                .contains_key(&input.source.id)
            {
                return Err(Rejection::UnknownSource);
            }
            if self.state.claims.source_heads.get(&input.source.id) != Some(&input.source.revision)
            {
                return Err(Rejection::StaleRevision);
            }
            let source = self
                .state
                .claims
                .sources
                .get(&key)
                .ok_or(Rejection::UnknownSource)?;
            if input.expires_at > source.input.expires_at {
                return Err(Rejection::Expired);
            }
            read(&source.input.root, &input.playbook)?;
            read(&source.input.root, &input.review)?;
            let current = evaluate(source, input.purpose, (self.clock)());
            if matches!(source.reviewed, Verdict::Rejected { .. }) {
                return Ok(if matches!(current, Verdict::Allowed { .. }) {
                    source.reviewed.clone()
                } else {
                    current
                });
            }
            Ok(current)
        })();
        result.unwrap_or_else(|reason| Verdict::Rejected { reason })
    }
    pub fn current_claims(
        &mut self,
        access: &Access,
        pins: &[Pin],
        release: &str,
    ) -> Result<Vec<ClaimView>> {
        self.refresh()?;
        self.check(access)?;
        hash(release, 40)?;
        if pins.is_empty() || pins.len() > 8 {
            return Err("claim consumer needs 1 to 8 distinct pins".into());
        }
        let mut seen = BTreeSet::new();
        let mut views = Vec::new();
        for pin in pins {
            let key = pin.key()?;
            if !seen.insert(key.clone()) {
                return Err("claim consumer repeated a revision".into());
            }
            let record = self.state.claims.claims.get(&key);
            let verdict = if self
                .state
                .claims
                .withdrawn
                .contains(&format!("claim:{key}"))
            {
                Verdict::Rejected {
                    reason: Rejection::Withdrawn,
                }
            } else if let Some(record) = record {
                if self.state.claims.claim_heads.get(&pin.id) != Some(&pin.revision) {
                    Verdict::Rejected {
                        reason: Rejection::StaleRevision,
                    }
                } else {
                    let source = self.state.claims.sources.get(&record.input.source.key()?);
                    if source.is_some_and(|s| s.input.scope.release != release) {
                        Verdict::Rejected {
                            reason: Rejection::ReleaseMismatch,
                        }
                    } else {
                        let current = self.evaluate_claim(&record.input);
                        // A rejected review cannot become an approval after a
                        // file reappears. Current negative evidence still wins.
                        if matches!(current, Verdict::Allowed { .. })
                            && !matches!(record.reviewed, Verdict::Allowed { .. })
                        {
                            record.reviewed.clone()
                        } else {
                            current
                        }
                    }
                }
            } else {
                Verdict::Rejected {
                    reason: Rejection::UnknownClaim,
                }
            };
            views.push(ClaimView {
                pin: pin.clone(),
                reviewed_sha256: record.map(|r| digest(&serde_json::to_vec(r).unwrap_or_default())),
                verdict,
            });
        }
        let mut next = self.state.clone();
        let mut changed = false;
        for view in &views {
            let Some(reason) = rejection(&view.verdict) else {
                continue;
            };
            let Some(record) = next.claims.claims.get(&view.pin.key()?) else {
                continue;
            };
            if !matches!(record.reviewed, Verdict::Allowed { .. }) {
                continue;
            }
            let subject = format!("claim:{}", view.pin.key()?);
            let previous = next
                .claims
                .decisions
                .iter()
                .rev()
                .find(|d| d.subject == subject && d.operation == "read_invalidated");
            if previous.is_some_and(|d| d.reason.as_ref() == Some(&reason)) {
                continue;
            }
            let reference = record.input_sha256.clone();
            next.claims.decision(
                &access.principal,
                subject,
                "read_invalidated",
                Some(reason),
                &reference,
                (self.clock)(),
            )?;
            changed = true;
        }
        if changed {
            self.persist(next)?;
        }
        Ok(views)
    }
    pub fn withdraw_claim_revision(
        &mut self,
        access: &Access,
        pin: &Pin,
        source: bool,
        reference: &str,
    ) -> Result<()> {
        self.refresh()?;
        self.admin(access)?;
        text(reference, 512)?;
        let key = pin.key()?;
        if !(if source {
            self.state.claims.sources.contains_key(&key)
        } else {
            self.state.claims.claims.contains_key(&key)
        }) {
            return Err("revision is unavailable".into());
        }
        let subject = format!("{}:{key}", if source { "source" } else { "claim" });
        if self.state.claims.withdrawn.contains(&subject) {
            return Ok(());
        }
        let mut next = self.state.clone();
        next.claims.decision(
            &access.principal,
            subject.clone(),
            "withdraw",
            Some(Rejection::Withdrawn),
            reference,
            (self.clock)(),
        )?;
        next.claims.withdrawn.insert(subject);
        self.persist(next)
    }
    pub fn claim_history(
        &mut self,
        access: &Access,
        after: usize,
        limit: usize,
    ) -> Result<Vec<Decision>> {
        self.refresh()?;
        self.admin(access)?;
        if !(1..=100).contains(&limit) {
            return Err("claims history limit must be 1 to 100".into());
        }
        Ok(self
            .state
            .claims
            .decisions
            .iter()
            .skip(after)
            .take(limit)
            .cloned()
            .collect())
    }
    pub fn compose_claim_draft(
        &mut self,
        access: &Access,
        name: &str,
        pins: Vec<DraftPin>,
        release: &str,
    ) -> Result<Draft> {
        self.refresh()?;
        self.check(access)?;
        id(name)?;
        let views = self.current_claims(
            access,
            &pins.iter().map(|p| p.claim.clone()).collect::<Vec<_>>(),
            release,
        )?;
        let mut clauses = Vec::new();
        for (pin, view) in pins.iter().zip(views) {
            hash(&pin.reviewed_sha256, 64)?;
            if view.reviewed_sha256.as_deref() != Some(&pin.reviewed_sha256) {
                return Err("draft claim review changed".into());
            }
            let Verdict::Allowed { claim } = view.verdict else {
                return Err("draft contains an unavailable or rejected claim".into());
            };
            // Limits and structured resource/cost details cannot be dropped from
            // the composed customer clause by a model or optional caller flag.
            clauses.push(serde_json::to_string_pretty(&claim).map_err(|e| e.to_string())?);
        }
        let content = clauses.join("\n\n");
        if content.len() > 128 * 1024 {
            return Err("claim draft exceeds bound".into());
        }
        let draft = Draft {
            schema: DRAFT_SCHEMA.into(),
            id: name.into(),
            claims: pins,
            sha256: digest(content.as_bytes()),
            content,
            created_by: access.principal.clone(),
            created_at: (self.clock)(),
        };
        if let Some(old) = self.state.claims.drafts.get(name) {
            if old.sha256 == draft.sha256
                && old
                    .claims
                    .iter()
                    .zip(&draft.claims)
                    .all(|(a, b)| a.claim == b.claim && a.reviewed_sha256 == b.reviewed_sha256)
                && old.claims.len() == draft.claims.len()
            {
                return Ok(old.clone());
            }
            return Err("claim draft is immutable; use a new draft ID".into());
        }
        if self.state.claims.drafts.len() >= MAX_REVISIONS {
            return Err("claim draft bound reached".into());
        }
        let mut next = self.state.clone();
        next.claims.drafts.insert(name.into(), draft.clone());
        self.persist(next)?;
        Ok(draft)
    }
    pub fn validate_claim_draft(
        &mut self,
        access: &Access,
        name: &str,
        release: &str,
    ) -> Result<Draft> {
        self.refresh()?;
        self.check(access)?;
        let draft = self
            .state
            .claims
            .drafts
            .get(name)
            .cloned()
            .ok_or("claim draft is unavailable")?;
        let pins = draft.claims.clone();
        let views = self.current_claims(
            access,
            &pins.iter().map(|p| p.claim.clone()).collect::<Vec<_>>(),
            release,
        )?;
        let mut current = Vec::new();
        let mut reason = None;
        for (pin, view) in pins.iter().zip(views) {
            if view.reviewed_sha256.as_deref() != Some(&pin.reviewed_sha256) {
                reason = Some(Rejection::DraftChanged);
                break;
            }
            match view.verdict {
                Verdict::Allowed { claim } => {
                    current.push(serde_json::to_string_pretty(&claim).map_err(|e| e.to_string())?)
                }
                other => {
                    reason = rejection(&other);
                    break;
                }
            }
        }
        if reason.is_none() && digest(current.join("\n\n").as_bytes()) != draft.sha256 {
            reason = Some(Rejection::DraftChanged);
        }
        if let Some(reason) = reason {
            let mut next = self.state.clone();
            next.claims.decision(
                &access.principal,
                format!("draft:{name}"),
                "draft_invalidated",
                Some(reason.clone()),
                &draft.sha256,
                (self.clock)(),
            )?;
            self.persist(next)?;
            return Err(format!("claim draft invalidated: {reason:?}"));
        }
        Ok(draft)
    }
}
#[cfg(test)]
mod tests;
