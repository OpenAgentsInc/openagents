//! Private weekly conversion review over consented custody and rechecked task
//! and financial evidence. Observations grant no outreach or tracking authority.

use crate::{sales_evidence as evidence, sales_finance as finance};
use receipts::sales_funnel::{
    self as funnel, Decision, EvidenceClass, Export, Failure, FailureReason, FinancialIdentity,
    Kind, Lane, TaskIdentity,
};
use receipts::service_sale::Reference;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const SCHEMA: &str = "openagents.gym.sales-weekly.v1";
pub const WEEK: u64 = 7 * 24 * 60 * 60;
const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub manifest: Reference,
    pub report: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub owner: String,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub journeys: Vec<Reference>,
    pub finance: Option<Pair>,
    pub gaps: Vec<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Verified,
    OwnerObservation,
    Unknown,
    Failed,
    Refunded,
    Funding,
    Duplicate,
    Superseded,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageRow {
    pub event: String,
    pub at: u64,
    pub stage: String,
    pub state: State,
    pub failure: Option<Failure>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Counts {
    pub journeys: u64,
    pub known_acquisition: u64,
    pub unknown_acquisition: u64,
    pub install_observations: u64,
    pub provider_observations: u64,
    pub accepted_tasks: u64,
    pub settled_purchases: u64,
    pub repeat_purchases: u64,
    pub assisted_pilots: u64,
    pub assisted_accepts: u64,
    pub assisted_declines: u64,
    pub unknown_purchases: u64,
    pub refunded_purchases: u64,
    pub failed_tasks: u64,
    pub unaccepted_tasks: u64,
    pub actionable_failures: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cohort {
    pub lane: Lane,
    pub classification: EvidenceClass,
    pub offer_version: String,
    pub cohort: String,
    pub cumulative: Counts,
    pub period: Counts,
    pub acquired_to_accepted: Ratio,
    pub accepted_to_purchased: Ratio,
    pub retained_buyers: Ratio,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JourneyRow {
    pub id: String,
    pub pipeline_lead: String,
    pub account: String,
    pub offer_version: String,
    pub cohort: String,
    pub lane: Lane,
    pub classification: EvidenceClass,
    pub history: Vec<StageRow>,
    /// Timing is explicitly recorded by the owner, never inferred from an
    /// engine exit or a downloaded file.
    pub acquisition_to_accepted_seconds: Option<u64>,
    pub accepted_to_purchase_seconds: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub manifest_digest: String,
    pub manifest: Manifest,
    pub sources: Vec<Export>,
    pub journeys: Vec<JourneyRow>,
    pub cohorts: Vec<Cohort>,
    pub finances: Option<finance::Report>,
    pub contribution_scope: String,
    pub commercial_activation_attested: bool,
    pub limitations: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema: String,
    pub owner: String,
    pub report_digest: String,
    pub approved: bool,
    pub reviewed_at: u64,
    pub release_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicLane {
    pub lane: Lane,
    pub classification: EvidenceClass,
    pub cumulative: Counts,
    pub period: Counts,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Aggregate {
    pub schema: String,
    pub lanes: Vec<PublicLane>,
    pub commercial_activation_attested: bool,
    pub limitations: Vec<String>,
}

fn read(reader: &mut evidence::Reader<'_>, reference: &Reference) -> Result<Vec<u8>, String> {
    reference.validate()?;
    let bytes = reader.read(&evidence::Reference {
        path: reference.path.clone(),
        sha256: reference.sha256.clone(),
    })?;
    if bytes.is_empty() {
        return Err("weekly evidence must be nonempty".into());
    }
    Ok(bytes)
}
fn same_report<T: Serialize>(bytes: &[u8], report: &T) -> Result<(), String> {
    let retained: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "malformed retained operating report")?;
    if retained
        != serde_json::to_value(report).map_err(|_| "operating report serialization failed")?
    {
        return Err(
            "retained operating report differs from independently rechecked sources".into(),
        );
    }
    Ok(())
}
fn superseded(events: &[funnel::Event], index: usize) -> bool {
    let kind = &events[index].input.kind;
    events[index + 1..]
        .iter()
        .any(|later| match (kind, &later.input.kind) {
            (Kind::Acquisition { .. }, Kind::Acquisition { .. })
            | (Kind::CustomerDecision { .. }, Kind::CustomerDecision { .. }) => true,
            (Kind::Purchase { source: a, .. }, Kind::Purchase { source: b, .. }) => a == b,
            _ => false,
        })
}
fn total(to: &mut Counts, from: &Counts) -> Result<(), String> {
    macro_rules! fields { ($($field:ident),+ $(,)?) => { $(to.$field = to.$field.checked_add(from.$field).ok_or("weekly count overflow")?;)+ }; }
    fields!(
        journeys,
        known_acquisition,
        unknown_acquisition,
        install_observations,
        provider_observations,
        accepted_tasks,
        settled_purchases,
        repeat_purchases,
        assisted_pilots,
        assisted_accepts,
        assisted_declines,
        unknown_purchases,
        refunded_purchases,
        failed_tasks,
        unaccepted_tasks,
        actionable_failures
    );
    Ok(())
}
#[derive(Default)]
struct Group {
    cumulative: Counts,
    period: Counts,
    acquired: BTreeSet<String>,
    accepted: BTreeSet<String>,
    purchased: BTreeSet<String>,
    purchases: BTreeMap<String, Vec<(u64, TaskIdentity)>>,
}

pub fn rebuild(root: &Path, bytes: &[u8], now: u64) -> Result<Report, String> {
    if bytes.len() > 1024 * 1024 {
        return Err("weekly manifest exceeds bound".into());
    }
    let meta = std::fs::symlink_metadata(root)
        .map_err(|_| "private weekly evidence root is unavailable")?;
    if !crate::sales_evidence::private_dir(&meta) {
        return Err("weekly evidence root must be a private directory".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| "malformed weekly manifest")?;
    funnel::bounded(&manifest.owner)?;
    if manifest.schema != SCHEMA
        || manifest.period_end.checked_sub(manifest.period_start) != Some(WEEK)
        || manifest.generated_at < manifest.period_end
        || manifest.generated_at > now
        || manifest.journeys.len() > 512
        || manifest.gaps.len() > 32
    {
        return Err(
            "weekly review needs one completed seven-day window and bounded explicit sources"
                .into(),
        );
    }
    for gap in &manifest.gaps {
        funnel::bounded(gap)?;
    }
    let mut reader = evidence::Reader {
        root,
        bytes: 0,
        snapshots: BTreeMap::new(),
    };
    let finances = if let Some(pair) = &manifest.finance {
        let report = finance::rebuild(root, &read(&mut reader, &pair.manifest)?)?;
        same_report(&read(&mut reader, &pair.report)?, &report)?;
        if report.manifest.owner != manifest.owner
            || report.manifest.period_start > manifest.period_start
            || report.manifest.period_end < manifest.period_end
        {
            return Err("weekly finances must cover the declared window under its owner".into());
        }
        Some(report)
    } else {
        None
    };
    let mut exports = Vec::new();
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    let mut tasks = BTreeSet::new();
    let mut purchases = BTreeSet::new();
    let mut groups = BTreeMap::<(Lane, EvidenceClass, String, String), Group>::new();
    for reference in &manifest.journeys {
        let export: Export = serde_json::from_slice(&read(&mut reader, reference)?)
            .map_err(|_| "malformed private funnel export")?;
        export.validate(now)?;
        let journey = &export.journey;
        if !ids.insert(journey.admission.id.clone())
            || export.exported_at < manifest.period_end
            || export.exported_at > manifest.generated_at
        {
            return Err(
                "weekly journey snapshots must be unique and current through the window".into(),
            );
        }
        read(&mut reader, &journey.admission.consent.evidence)?;
        let mut failures = BTreeMap::new();
        for failure in &journey.failures {
            read(&mut reader, &failure.input.evidence)?;
            failures.insert(&failure.input.event, failure);
        }
        let group = groups
            .entry((
                journey.admission.lane,
                journey.admission.classification,
                journey.admission.offer_version.clone(),
                journey.admission.cohort.clone(),
            ))
            .or_default();
        group.cumulative.journeys += 1;
        if journey.admitted_at >= manifest.period_start && journey.admitted_at < manifest.period_end
        {
            group.period.journeys += 1;
        }
        let mut history = Vec::new();
        let mut first_acquisition = None;
        let mut first_accepted = None;
        let mut first_purchase = None;
        let mut journey_tasks = BTreeSet::new();
        let mut install_seen = false;
        let mut provider_seen = false;
        let mut pilot_seen = false;
        let events = journey
            .events
            .iter()
            .filter(|e| e.input.at < manifest.period_end)
            .cloned()
            .collect::<Vec<_>>();
        // Verify task evidence first so a purchase must name this journey's
        // independently accepted task, regardless of operator entry order.
        let mut checked_tasks = BTreeMap::new();
        for event in &events {
            if let Kind::Task {
                manifest: source,
                report,
                attribution,
                task,
            } = &event.input.kind
            {
                let checked = evidence::rebuild(root, &read(&mut reader, source)?)?;
                same_report(&read(&mut reader, report)?, &checked)?;
                if checked.manifest.offer_version != journey.admission.offer_version {
                    return Err("task observation names another offer".into());
                }
                let task = checked
                    .manifest
                    .tasks
                    .iter()
                    .find(|t| t.id == *task)
                    .ok_or("observed task is missing")?;
                let binding: funnel::TaskAttribution =
                    serde_json::from_slice(&read(&mut reader, attribution)?)
                        .map_err(|_| "malformed task account attribution")?;
                if binding.schema != "openagents.sales.task-account-attribution.v1"
                    || binding.manifest_digest != source.sha256
                    || binding.task != task.id
                    || binding.offer_version != journey.admission.offer_version
                    || binding.cohort != journey.admission.cohort
                    || binding
                        .account
                        .as_ref()
                        .is_some_and(|account| account != &journey.account)
                {
                    return Err(
                        "task attribution changes its pinned account/offer/cohort/source".into(),
                    );
                }
                let identity =
                    task.candidate
                        .iter()
                        .find(|a| a.acceptance.is_some())
                        .map(|candidate| TaskIdentity {
                            manifest_digest: checked.manifest_digest.clone(),
                            task: task.id.clone(),
                            task_digest: task.task_digest.clone(),
                            candidate_digest: candidate.artifact.sha256.clone(),
                            trace_digest: candidate.trace.sha256.clone(),
                            customer_acceptance_digest: candidate
                                .acceptance
                                .as_ref()
                                .unwrap()
                                .customer_decision
                                .sha256
                                .clone(),
                        });
                let attributed = binding.account.is_some();
                if let Some(candidate) = task.candidate.iter().find(|a| a.acceptance.is_some()) {
                    let decision = &candidate.acceptance.as_ref().unwrap().customer_decision;
                    if attributed
                        && !binding
                            .customer_decision
                            .as_ref()
                            .is_some_and(|r| r.path == decision.path && r.sha256 == decision.sha256)
                    {
                        return Err(
                            "accepted task attribution needs its exact customer decision".into(),
                        );
                    }
                }
                if attributed {
                    if let Some(identity) = &identity {
                        journey_tasks.insert(identity.clone());
                    }
                }
                let failed = task.candidate.iter().any(|attempt| {
                    attempt
                        .checks
                        .values()
                        .any(|check| check.status == evidence::Status::Failed)
                });
                checked_tasks.insert(event.input.id.clone(), (identity, attributed, failed));
            }
        }
        for (index, event) in events.iter().enumerate() {
            for reference in event.input.kind.references() {
                read(&mut reader, reference)?;
            }
            let mut counted = Counts::default();
            let mut reason = None;
            let stage;
            let mut state = State::OwnerObservation;
            let within = event.input.at >= manifest.period_start;
            if superseded(&events, index) {
                history.push(StageRow {
                    event: event.input.id.clone(),
                    at: event.input.at,
                    stage: "superseded_observation".into(),
                    state: State::Superseded,
                    failure: failures.get(&event.input.id).map(|f| (*f).clone()),
                });
                continue;
            }
            match &event.input.kind {
                Kind::Acquisition { source, .. } => {
                    stage = "acquisition";
                    if source.is_some() {
                        counted.known_acquisition = 1;
                        first_acquisition = Some(event.input.at);
                        group.acquired.insert(journey.admission.id.clone());
                    } else {
                        counted.unknown_acquisition = 1;
                        state = State::Unknown;
                        reason = Some(FailureReason::UnknownAttribution);
                    }
                }
                Kind::Install { .. } => {
                    stage = "install_observation";
                    if !install_seen {
                        counted.install_observations = 1;
                        install_seen = true;
                    }
                }
                Kind::ProviderActivation { .. } => {
                    stage = "provider_observation";
                    if !provider_seen {
                        counted.provider_observations = 1;
                        provider_seen = true;
                    }
                }
                Kind::PilotAgreed { .. } => {
                    stage = "assisted_pilot";
                    if !pilot_seen {
                        counted.assisted_pilots = 1;
                        pilot_seen = true;
                    }
                }
                Kind::CustomerDecision { decision, .. } => {
                    stage = "assisted_customer_decision";
                    match decision {
                        Decision::Accept => counted.assisted_accepts = 1,
                        Decision::Decline | Decision::Defer => {
                            counted.assisted_declines = 1;
                            state = State::Failed;
                            reason = Some(FailureReason::CustomerDeclined);
                        }
                    }
                }
                Kind::Task { .. } => {
                    stage = "task";
                    if let Some((Some(identity), true, _)) = checked_tasks.get(&event.input.id) {
                        state = State::Verified;
                        if tasks.insert((journey.account.clone(), identity.material_key())) {
                            counted.accepted_tasks = 1;
                            group.accepted.insert(journey.admission.id.clone());
                            first_accepted = Some(
                                first_accepted
                                    .map_or(event.input.at, |old: u64| old.min(event.input.at)),
                            );
                        } else {
                            state = State::Duplicate;
                        }
                    } else if checked_tasks
                        .get(&event.input.id)
                        .is_some_and(|(identity, _, _)| identity.is_some())
                    {
                        state = State::Unknown;
                        reason = Some(FailureReason::UnknownAttribution);
                    } else if checked_tasks
                        .get(&event.input.id)
                        .is_some_and(|(_, _, failed)| *failed)
                    {
                        state = State::Failed;
                        counted.failed_tasks = 1;
                        reason = Some(FailureReason::TaskFailed);
                    } else {
                        state = State::Unknown;
                        counted.unaccepted_tasks = 1;
                    }
                }
                Kind::Purchase {
                    financial_offer,
                    entry,
                    source,
                    ..
                } => {
                    stage = "purchase";
                    let outcome = finances.as_ref().and_then(|r| {
                        r.entry_outcomes
                            .iter()
                            .find(|o| o.offer == *financial_offer && o.entry == *entry)
                    });
                    if let Some(outcome) = outcome {
                        if outcome.source != *source
                            || outcome.account != journey.account
                            || outcome.offer_version != journey.admission.offer_version
                            || outcome.cohort != journey.admission.cohort
                        {
                            return Err("purchase observation changes its immutable account/offer/cohort/source".into());
                        }
                        let mut current = true;
                        if let FinancialIdentity::ServiceSale { sale } = source {
                            current = false;
                            if let Some(canonical) = export.services.get(sale) {
                                let financial = finances.as_ref().unwrap();
                                let matched = financial
                                    .manifest
                                    .offers
                                    .iter()
                                    .find(|o| o.id == *financial_offer)
                                    .and_then(|o| o.entries.iter().find(|e| e.id == *entry));
                                if let Some(finance::Entry {
                                    source: finance::Source::ServiceSale { export: source },
                                    ..
                                }) = matched
                                {
                                    let retained: receipts::service_sale::Export =
                                        serde_json::from_slice(&read(
                                            &mut reader,
                                            &Reference {
                                                path: source.path.clone(),
                                                sha256: source.sha256.clone(),
                                            },
                                        )?)
                                        .map_err(|_| "malformed current service evidence")?;
                                    if retained.sale != *canonical {
                                        return Err("financial service evidence differs from current canonical custody".into());
                                    }
                                    current = true;
                                }
                            }
                        }
                        match outcome.status {
                            finance::PurchaseStatus::Settled
                                if current
                                    && outcome.at == event.input.at
                                    && outcome
                                        .accepted_task
                                        .as_ref()
                                        .is_some_and(|task| journey_tasks.contains(task)) =>
                            {
                                state = State::Verified;
                                if purchases.insert(outcome.source.clone()) {
                                    counted.settled_purchases = 1;
                                    group.purchased.insert(journey.admission.id.clone());
                                    first_purchase =
                                        Some(first_purchase.map_or(event.input.at, |old: u64| {
                                            old.min(event.input.at)
                                        }));
                                    group
                                        .purchases
                                        .entry(journey.account.clone())
                                        .or_default()
                                        .push((
                                            event.input.at,
                                            outcome.accepted_task.clone().unwrap(),
                                        ));
                                } else {
                                    state = State::Duplicate;
                                }
                            }
                            finance::PurchaseStatus::Refunded => {
                                state = State::Refunded;
                                counted.refunded_purchases = 1;
                                reason = Some(FailureReason::Refunded);
                            }
                            finance::PurchaseStatus::Funding => {
                                state = State::Funding;
                                counted.unknown_purchases = 1;
                                reason = Some(FailureReason::PaymentUnknown);
                            }
                            finance::PurchaseStatus::FailedDelivery => {
                                state = State::Failed;
                                counted.unknown_purchases = 1;
                                reason = Some(FailureReason::TaskFailed);
                            }
                            _ => {
                                state = State::Unknown;
                                counted.unknown_purchases = 1;
                                reason = Some(FailureReason::PaymentUnknown);
                            }
                        }
                    } else {
                        state = State::Unknown;
                        counted.unknown_purchases = 1;
                        reason = Some(FailureReason::PaymentUnknown);
                    }
                }
            }
            let failure = failures.get(&event.input.id).map(|f| (*f).clone());
            if let Some(reason) = reason {
                if !failure.as_ref().is_some_and(|f| {
                    f.input.reason == reason || f.input.reason == FailureReason::Other
                }) {
                    return Err("actionable failed conversion needs its responsible human and dated next action".into());
                }
            }
            if failure
                .as_ref()
                .is_some_and(|failure| !failure.input.resolved)
            {
                counted.actionable_failures = 1;
            }
            total(&mut group.cumulative, &counted)?;
            if within {
                total(&mut group.period, &counted)?;
            }
            history.push(StageRow {
                event: event.input.id.clone(),
                at: event.input.at,
                stage: stage.into(),
                state,
                failure,
            });
        }
        rows.push(JourneyRow {
            id: journey.admission.id.clone(),
            pipeline_lead: journey.pipeline_lead.clone(),
            account: journey.account.clone(),
            offer_version: journey.admission.offer_version.clone(),
            cohort: journey.admission.cohort.clone(),
            lane: journey.admission.lane,
            classification: journey.admission.classification,
            history,
            acquisition_to_accepted_seconds: first_acquisition
                .and_then(|a| first_accepted.and_then(|b| b.checked_sub(a))),
            accepted_to_purchase_seconds: first_accepted
                .and_then(|a| first_purchase.and_then(|b| b.checked_sub(a))),
        });
        exports.push(export);
    }
    let mut cohorts = Vec::new();
    for ((lane, classification, offer_version, cohort), mut group) in groups {
        let mut prior = 0;
        let mut retained = 0;
        for purchases in group.purchases.values_mut() {
            purchases.sort();
            let mut tasks = BTreeSet::new();
            let mut had_prior = false;
            let mut returned = false;
            for (at, task) in purchases.iter() {
                if !tasks.insert(task.material_key()) {
                    continue;
                }
                if tasks.len() > 1 {
                    group.cumulative.repeat_purchases += 1;
                    if *at >= manifest.period_start {
                        group.period.repeat_purchases += 1;
                    }
                }
                if *at < manifest.period_start {
                    had_prior = true;
                } else if had_prior {
                    returned = true;
                }
            }
            prior += u64::from(had_prior);
            retained += u64::from(returned);
        }
        cohorts.push(Cohort {
            lane,
            classification,
            offer_version,
            cohort,
            cumulative: group.cumulative,
            period: group.period,
            acquired_to_accepted: Ratio {
                numerator: group.acquired.intersection(&group.accepted).count() as u64,
                denominator: group.acquired.len() as u64,
            },
            accepted_to_purchased: Ratio {
                numerator: group.accepted.intersection(&group.purchased).count() as u64,
                denominator: group.accepted.len() as u64,
            },
            retained_buyers: Ratio {
                numerator: retained,
                denominator: prior,
            },
        });
    }
    let contribution_scope = if let Some(finance) = &finances {
        if finance.manifest.period_start == manifest.period_start
            && finance.manifest.period_end == manifest.period_end
        {
            "declared_week"
        } else {
            "retained_financial_scope; weekly contribution unknown"
        }
    } else {
        "unknown; no checked operating evidence"
    };
    Ok(Report { schema: SCHEMA.into(), manifest_digest: evidence::digest(bytes), manifest, sources: exports, journeys: rows, cohorts, finances,
        contribution_scope: contribution_scope.into(), commercial_activation_attested: false, limitations: vec![
            "Attributable retained operator evidence, not remote attestation or proof of real buyer/payment truth.".into(),
            "Install/provider and assisted decisions are owner observations; verified activation requires an independently accepted task and currently settled qualified purchase.".into(),
            "Counts cover only explicitly consented, retained journeys; gaps and untracked customers are not inferred away. Retention uses distinct accepted tasks on the same commercial account, lane, offer version, and cohort.".into(),
            "Window counts use source/event time; current payment dispositions are revalued at review creation. Cumulative financial evidence does not imply a weekly margin.".into(),
            "Economics covers the independently declared private finance book, which can include accounts outside consented journeys. Matching weekly dates does not establish cohort attribution.".into(),
            "No publication, tracking, human handoff, or outreach authority is granted by this review.".into(),
        ] })
}

pub fn project(bytes: &[u8], review: &Review, now: u64) -> Result<Aggregate, String> {
    if bytes.len() > MAX_BYTES {
        return Err("private weekly report exceeds bound".into());
    }
    let report: Report =
        serde_json::from_slice(bytes).map_err(|_| "malformed private weekly report")?;
    if report.schema != SCHEMA
        || review.schema != "openagents.gym.sales-weekly-review.v1"
        || !review.approved
        || review.owner != report.manifest.owner
        || review.report_digest != evidence::digest(bytes)
        || review.reviewed_at < report.manifest.generated_at
        || review.release_at <= review.reviewed_at
        || review.release_at <= report.manifest.period_end
        || now < review.release_at
        || report.sources.iter().any(|s| {
            !s.journey.admission.consent.aggregate_counts
                || s.journey.admission.consent.expires_at <= now
                || s.journey.retain_until <= now
        })
    {
        return Err("public counts need exact owner review, separate aggregation consent, and an elapsed release delay".into());
    }
    let mut lanes = BTreeMap::<(Lane, EvidenceClass), (Counts, Counts)>::new();
    for cohort in report.cohorts {
        let target = lanes
            .entry((cohort.lane, cohort.classification))
            .or_default();
        total(&mut target.0, &cohort.cumulative)?;
        total(&mut target.1, &cohort.period)?;
    }
    Ok(Aggregate { schema: "openagents.gym.sales-weekly-aggregate.v1".into(), lanes: lanes.into_iter().map(|((lane, classification), (cumulative, period))| PublicLane { lane, classification, cumulative, period }).collect(), commercial_activation_attested: false,
        limitations: vec!["Explicitly reviewed delayed counts from retained consented evidence; not commercial activation or publication authority.".into(), "Customer/source names, messages, amounts, individual dates, timing, and real deal events are withheld.".into()] })
}

#[cfg(test)]
#[path = "sales_weekly_tests.rs"]
mod tests;
