//! Private sales comparisons over pinned ATIF logs and retained artifacts.
//! This joins evidence; it executes no task and makes no deployment claim.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};

pub const SCHEMA: &str = "openagents.gym.sales-evidence.v1";
const MAX_FILE: u64 = 8 * 1024 * 1024;
const MAX_STUDY: usize = 1024 * 1024;
const MAX_SOURCES: u64 = 64 * 1024 * 1024;

/// A retained snapshot, relative to an explicitly supplied private source root.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub offer_version: String,
    pub source_revision: String,
    pub baseline_method: String,
    pub candidate_method: String,
    /// Explicitly labels retrospective selection; neither mode proves deployment.
    pub retrospective_selection: bool,
    /// The frozen inventory attests that no attempts were omitted.
    pub inventory: Reference,
    pub gym_store: Option<Reference>,
    pub tasks: Vec<Task>,
    /// Missing evidence outside the inventory is retained as a limitation.
    pub skipped_evidence: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub task_digest: String,
    pub check_digests: BTreeMap<String, String>,
    pub baseline: Vec<Attempt>,
    pub candidate: Vec<Attempt>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub id: String,
    pub kind: AttemptKind,
    pub parent: Option<String>,
    pub executor: String,
    pub trace: Reference,
    pub artifact: Reference,
    pub checks: BTreeMap<String, Check>,
    pub setup_ms: u64,
    pub queue_ms: u64,
    pub check_ms: u64,
    pub support_ms: u64,
    pub costs: Vec<Cost>,
    pub compute: Option<ComputeUsage>,
    pub acceptance: Option<Acceptance>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeUsage {
    pub milliseconds: u64,
    pub evidence: Reference,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttemptKind {
    Primary,
    Repair,
    Retry,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passed,
    Failed,
    Skipped,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub check_digest: String,
    pub status: Status,
    pub evidence: Option<Reference>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub candidate_digest: String,
    pub independent_checker: String,
    pub check_review: Reference,
    pub customer_decision: Reference,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CostBasis {
    Billed,
    ListPrice,
    SubscriptionCapacity,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CostComponent {
    Provider,
    Compute,
    Support,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub version: String,
    pub provenance: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub component: CostComponent,
    pub basis: CostBasis,
    /// Integer denomination, including its scale (for example USD millionths).
    pub unit: String,
    pub amount: Option<u64>,
    pub evidence: Option<Reference>,
    pub price: Option<Price>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema: String,
    pub attempts: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostTotal {
    pub component: CostComponent,
    pub basis: CostBasis,
    pub unit: String,
    pub known_subtotal: u64,
    pub unknown_items: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Totals {
    pub attempts: u64,
    pub repairs: u64,
    pub retries: u64,
    pub accepted_tasks: u64,
    pub failed_checks: u64,
    pub unknown_checks: u64,
    pub incomplete_traces: u64,
    pub setup_ms: u64,
    pub queue_ms: u64,
    pub execution_ms: u64,
    pub check_ms: u64,
    pub support_ms: u64,
    pub compute_known_ms: u64,
    pub compute_unknown_attempts: u64,
    pub reported_input_tokens: u64,
    pub reported_output_tokens: u64,
    pub unknown_generation_usage_steps: u64,
    pub costs: Vec<CostTotal>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttemptEvidence {
    pub id: String,
    pub session: String,
    pub models: Vec<String>,
    pub provider: String,
    pub complete_trace: bool,
    pub artifact_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub manifest_digest: String,
    pub manifest: Manifest,
    pub baseline: Totals,
    pub candidate: Totals,
    pub attempts: Vec<AttemptEvidence>,
    pub deployed_routing_improvement: bool,
}

/// An explicit human review authorizes only the projection of these exact bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicReview {
    pub schema: String,
    pub report_digest: String,
    pub reviewer: String,
    pub approved: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicReport {
    pub schema: String,
    pub scope: String,
    pub baseline: Totals,
    pub candidate: Totals,
    pub limitations: Vec<String>,
    pub deployed_routing_improvement: bool,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn text(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 256 || s.chars().any(char::is_control) {
        Err("invalid bounded evidence identifier".into())
    } else {
        Ok(())
    }
}
fn hash(s: &str, length: usize) -> Result<(), String> {
    if s.len() != length
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Err("invalid lowercase digest".into())
    } else {
        Ok(())
    }
}
fn add(value: &mut u64, amount: u64) -> Result<(), String> {
    *value = value.checked_add(amount).ok_or("evidence total overflow")?;
    Ok(())
}

pub(crate) struct Reader<'a> {
    pub(crate) root: &'a Path,
    pub(crate) bytes: u64,
    pub(crate) snapshots: BTreeMap<String, (String, Vec<u8>)>,
}
impl Reader<'_> {
    pub(crate) fn read(&mut self, r: &Reference) -> Result<Vec<u8>, String> {
        hash(&r.sha256, 64)?;
        if r.path.is_empty() || r.path.len() > 512 {
            return Err("invalid evidence path".into());
        }
        if let Some((prior, bytes)) = self.snapshots.get(&r.path) {
            if prior != &r.sha256 {
                return Err("conflicting snapshot digest".into());
            }
            return Ok(bytes.clone());
        }
        let relative = Path::new(&r.path);
        let mut path = self.root.to_owned();
        for component in relative.components() {
            let Component::Normal(part) = component else {
                return Err("evidence path must remain under its root".into());
            };
            path.push(part);
            if fs::symlink_metadata(&path)
                .map_err(|_| "retained evidence is missing")?
                .file_type()
                .is_symlink()
            {
                return Err("symlink evidence is refused".into());
            }
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| "cannot open retained evidence")?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("evidence must be a regular file".into());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE {
            return Err("retained evidence exceeds file bound".into());
        }
        add(&mut self.bytes, bytes.len() as u64)?;
        if self.bytes > MAX_SOURCES {
            return Err("study evidence exceeds total bound".into());
        }
        if digest(&bytes) != r.sha256 {
            return Err("retained evidence digest mismatch".into());
        }
        self.snapshots
            .insert(r.path.clone(), (r.sha256.clone(), bytes.clone()));
        Ok(bytes)
    }
}

/// Rebuild a private comparison from the frozen inventory and exact retained bytes.
/// ATIF remains the trace format; this manifest only joins artifacts and attestations.
pub fn rebuild(root: &Path, manifest_bytes: &[u8]) -> Result<Report, String> {
    if manifest_bytes.len() > MAX_STUDY {
        return Err("comparison manifest exceeds bound".into());
    }
    let manifest: Manifest = serde_json::from_slice(manifest_bytes).map_err(|e| e.to_string())?;
    if manifest.schema != SCHEMA {
        return Err("unsupported comparison schema".into());
    }
    for s in [
        &manifest.offer_version,
        &manifest.baseline_method,
        &manifest.candidate_method,
    ] {
        text(s)?;
    }
    hash(&manifest.source_revision, 40)?;
    if manifest.tasks.is_empty()
        || manifest.tasks.len() > 32
        || manifest.skipped_evidence.len() > 32
    {
        return Err("comparison inventory exceeds bounds".into());
    }
    for s in &manifest.skipped_evidence {
        text(s)?;
    }
    let mut reader = Reader {
        root,
        bytes: 0,
        snapshots: BTreeMap::new(),
    };
    let inventory: Inventory =
        serde_json::from_slice(&reader.read(&manifest.inventory)?).map_err(|e| e.to_string())?;
    if inventory.schema != "openagents.gym.sales-inventory.v1" {
        return Err("unsupported inventory schema".into());
    }
    if let Some(reference) = &manifest.gym_store {
        let bytes = reader.read(reference)?;
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
            .map_err(|e| e.to_string())?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        if rows.len() > 10000
            || rows.iter().any(|r| {
                !crate::store::KNOWN_ROW_SCHEMAS.contains(&r["schema"].as_str().unwrap_or(""))
            })
            || !matches!(
                crate::store::verify_chain(&rows),
                crate::store::ChainVerdict::Ok { .. }
            )
        {
            return Err("retained Gym evidence chain is invalid".into());
        }
    }
    let mut expected = BTreeMap::new();
    let mut tasks = BTreeSet::new();
    let mut sessions = BTreeSet::new();
    let mut attempt_ids = BTreeSet::new();
    let mut attempts = Vec::new();
    let mut baseline = Totals::default();
    let mut candidate = Totals::default();
    for task in &manifest.tasks {
        text(&task.id)?;
        hash(&task.task_digest, 64)?;
        if !tasks.insert(&task.id) || task.check_digests.is_empty() || task.check_digests.len() > 8
        {
            return Err("invalid frozen task/check inventory".into());
        }
        for (id, d) in &task.check_digests {
            text(id)?;
            hash(d, 64)?;
        }
        for (side, rows, totals) in [
            ("baseline", &task.baseline, &mut baseline),
            ("candidate", &task.candidate, &mut candidate),
        ] {
            if rows.is_empty() || rows.len() > 64 {
                return Err("attempt inventory exceeds bound".into());
            }
            expected.insert(
                format!("{}/{side}", task.id),
                rows.iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
            );
            let mut seen = BTreeSet::new();
            let mut accepted = false;
            for a in rows {
                text(&a.id)?;
                text(&a.executor)?;
                if !attempt_ids.insert(&a.id) {
                    return Err("duplicate attempt identity".into());
                }
                match (a.kind, &a.parent) {
                    (AttemptKind::Primary, None) => {}
                    (AttemptKind::Repair | AttemptKind::Retry, Some(p)) if seen.contains(p) => {}
                    _ => {
                        return Err(
                            "repair/retry must name an earlier attempt on the same side".into()
                        );
                    }
                }
                seen.insert(a.id.clone());
                if a.checks.keys().collect::<Vec<_>>()
                    != task.check_digests.keys().collect::<Vec<_>>()
                {
                    return Err("attempt omitted or changed a frozen check".into());
                }
                let trace = reader.read(&a.trace)?;
                let recording = atif::log::read_bytes(Path::new(&a.trace.path), &trace)
                    .map_err(|e| e.to_string())?;
                if !sessions.insert(recording.session.id.clone()) {
                    return Err("a trace session was counted twice".into());
                }
                reader.read(&a.artifact)?;
                if let Some(compute) = &a.compute {
                    reader.read(&compute.evidence)?;
                    add(&mut totals.compute_known_ms, compute.milliseconds)?;
                } else {
                    add(&mut totals.compute_unknown_attempts, 1)?;
                }
                let mut models = BTreeSet::from([recording.session.model.clone()]);
                for step in &recording.steps {
                    if let Some((input, output)) = step.tokens {
                        add(&mut totals.reported_input_tokens, input)?;
                        add(&mut totals.reported_output_tokens, output)?;
                    } else if step.source == atif::Source::Agent && step.call.is_none() {
                        add(&mut totals.unknown_generation_usage_steps, 1)?;
                    }
                    if let Some(m) = &step.model {
                        models.insert(m.clone());
                    }
                }
                let complete = recording.whole();
                attempts.push(AttemptEvidence {
                    id: a.id.clone(),
                    session: recording.session.id.clone(),
                    models: models.into_iter().collect(),
                    provider: recording.session.door.clone(),
                    complete_trace: complete,
                    artifact_digest: a.artifact.sha256.clone(),
                });
                add(&mut totals.attempts, 1)?;
                if a.kind == AttemptKind::Repair {
                    add(&mut totals.repairs, 1)?;
                }
                if a.kind == AttemptKind::Retry {
                    add(&mut totals.retries, 1)?;
                }
                if !complete {
                    add(&mut totals.incomplete_traces, 1)?;
                }
                for (counter, amount) in [
                    (&mut totals.setup_ms, a.setup_ms),
                    (&mut totals.queue_ms, a.queue_ms),
                    (&mut totals.check_ms, a.check_ms),
                    (&mut totals.support_ms, a.support_ms),
                ] {
                    add(counter, amount)?;
                }
                add(
                    &mut totals.execution_ms,
                    recording
                        .session
                        .seconds
                        .checked_mul(1000)
                        .ok_or("trace duration overflow")?,
                )?;
                for (check_id, check) in &a.checks {
                    if task.check_digests[check_id] != check.check_digest {
                        return Err("check outcome names another command digest".into());
                    }
                    if let Some(r) = &check.evidence {
                        reader.read(r)?;
                    }
                    if matches!(check.status, Status::Passed | Status::Failed)
                        && check.evidence.is_none()
                    {
                        return Err("known check outcome needs retained evidence".into());
                    }
                    if check.status == Status::Failed {
                        add(&mut totals.failed_checks, 1)?;
                    }
                    if matches!(check.status, Status::Unknown | Status::Skipped) {
                        add(&mut totals.unknown_checks, 1)?;
                    }
                }
                if let Some(acceptance) = &a.acceptance {
                    text(&acceptance.independent_checker)?;
                    if accepted
                        || !complete
                        || acceptance.candidate_digest != a.artifact.sha256
                        || acceptance.independent_checker == a.executor
                        || a.checks.values().any(|c| c.status != Status::Passed)
                    {
                        return Err(
                            "acceptance lacks exact independently checked complete candidate"
                                .into(),
                        );
                    }
                    reader.read(&acceptance.check_review)?;
                    reader.read(&acceptance.customer_decision)?;
                    accepted = true;
                    add(&mut totals.accepted_tasks, 1)?;
                }
                if a.costs.len() > 3 {
                    return Err("cost inventory exceeds bound".into());
                }
                let mut components = BTreeSet::new();
                for cost in &a.costs {
                    text(&cost.unit)?;
                    if !components.insert(cost.component) {
                        return Err("duplicate cost component".into());
                    }
                    if cost.basis == CostBasis::Unknown {
                        if cost.amount.is_some() || cost.price.is_some() {
                            return Err("unknown cost cannot contain a price or amount".into());
                        }
                    } else {
                        if cost.amount.is_none() || cost.evidence.is_none() {
                            return Err("known cost needs amount and retained provenance".into());
                        }
                        if cost.basis == CostBasis::ListPrice && cost.price.is_none() {
                            return Err("list estimate requires pinned price".into());
                        }
                        if cost.basis == CostBasis::SubscriptionCapacity
                            && cost.unit != "subscription_capacity_units"
                        {
                            return Err("subscription capacity is not cash".into());
                        }
                    }
                    if let Some(r) = &cost.evidence {
                        reader.read(r)?;
                    }
                    if let Some(price) = &cost.price {
                        text(&price.version)?;
                        reader.read(&price.provenance)?;
                    }
                    accumulate(totals, cost.component, cost.basis, &cost.unit, cost.amount)?;
                }
                for component in [
                    CostComponent::Provider,
                    CostComponent::Compute,
                    CostComponent::Support,
                ] {
                    if !components.contains(&component) {
                        accumulate(totals, component, CostBasis::Unknown, "undeclared", None)?;
                    }
                }
            }
        }
    }
    if inventory.attempts != expected {
        return Err("comparison omitted, reordered, or added an inventory attempt".into());
    }
    Ok(Report {
        schema: SCHEMA.into(),
        manifest_digest: digest(manifest_bytes),
        manifest,
        baseline,
        candidate,
        attempts,
        deployed_routing_improvement: false,
    })
}
fn accumulate(
    totals: &mut Totals,
    component: CostComponent,
    basis: CostBasis,
    unit: &str,
    amount: Option<u64>,
) -> Result<(), String> {
    let index = totals
        .costs
        .iter()
        .position(|c| c.component == component && c.basis == basis && c.unit == unit);
    let index = index.unwrap_or_else(|| {
        totals.costs.push(CostTotal {
            component,
            basis,
            unit: unit.into(),
            known_subtotal: 0,
            unknown_items: 0,
        });
        totals.costs.len() - 1
    });
    if let Some(amount) = amount {
        add(&mut totals.costs[index].known_subtotal, amount)?;
    } else {
        add(&mut totals.costs[index].unknown_items, 1)?;
    }
    totals
        .costs
        .sort_by(|a, b| (a.component, a.basis, &a.unit).cmp(&(b.component, b.basis, &b.unit)));
    Ok(())
}

/// Export aggregates only after review of the exact private report. Private IDs,
/// paths, revisions, customer decisions, provider identities, and text stay private.
pub fn project(report_bytes: &[u8], review: &PublicReview) -> Result<PublicReport, String> {
    text(&review.reviewer)?;
    if review.schema != "openagents.gym.sales-review.v1"
        || !review.approved
        || review.report_digest != digest(report_bytes)
    {
        return Err("public projection requires approval of exact report bytes".into());
    }
    if report_bytes.len() > 2 * MAX_STUDY {
        return Err("private report exceeds bound".into());
    }
    let report: Report = serde_json::from_slice(report_bytes).map_err(|e| e.to_string())?;
    if report.schema != SCHEMA || report.deployed_routing_improvement {
        return Err("unsupported or misleading report".into());
    }
    // Cost unit strings are private configuration. Public output uses only a
    // small denomination allowlist and retains omitted amounts as unknown.
    fn public_totals(mut totals: Totals) -> Totals {
        for cost in &mut totals.costs {
            if !matches!(
                cost.unit.as_str(),
                "USD_millionths" | "msat" | "sat" | "subscription_capacity_units" | "undeclared"
            ) {
                cost.unit = "withheld_denomination".into();
                cost.known_subtotal = 0;
                cost.unknown_items = cost.unknown_items.saturating_add(1);
            }
        }
        totals
    }
    let mut limitations=vec!["One frozen task/check inventory; no generalization or deployed routing improvement is established.".into(),"Actual billed amounts, list-price estimates, unknown costs, and subscription capacity are separate; no cash savings are inferred.".into(),"Elapsed execution is the ATIF recorded duration in whole seconds; setup, queue, checks, and support are separately declared.".into()];
    if report.manifest.retrospective_selection {
        limitations.push("Retrospective selection: the preferred result was selected after outcomes were observed.".into());
    }
    if !report.manifest.skipped_evidence.is_empty() {
        limitations.push(format!(
            "{} explicitly declared evidence gaps; descriptions remain private.",
            report.manifest.skipped_evidence.len()
        ));
    }
    Ok(PublicReport{schema:"openagents.gym.sales-public.v1".into(),scope:"Pinned public-repository maintenance tasks with independently reviewed customer acceptance.".into(),baseline:public_totals(report.baseline),candidate:public_totals(report.candidate),limitations,deployed_routing_improvement:false})
}

/// Create a private output without overwriting an earlier reviewed record.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .and_then(|f| f.sync_all())
    .map_err(|e| e.to_string())
}

/// Offline CLI adapter. Detailed records never go to stdout.
pub fn command(args: &[String]) -> Result<(), String> {
    let mut flags = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !matches!(
                pair[0].as_str(),
                "--root" | "--manifest" | "--output" | "--report" | "--review"
            )
            || flags.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("use sales-evidence --root DIR --manifest FILE --output FILE, or --report FILE --review FILE --output FILE".into());
        }
    }
    fn bounded(path: &str) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take((2 * MAX_STUDY + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 2 * MAX_STUDY {
            return Err("input exceeds evidence bound".into());
        }
        Ok(bytes)
    }
    let output = flags.get("--output").ok_or("output path is required")?;
    let value = if let (Some(root), Some(manifest)) = (flags.get("--root"), flags.get("--manifest"))
    {
        if flags.len() != 3 {
            return Err("rebuild and projection flags cannot mix".into());
        }
        serde_json::to_vec_pretty(&rebuild(Path::new(root), &bounded(manifest)?)?)
            .map_err(|e| e.to_string())?
    } else if let (Some(report), Some(review)) = (flags.get("--report"), flags.get("--review")) {
        if flags.len() != 3 {
            return Err("rebuild and projection flags cannot mix".into());
        }
        let review: PublicReview =
            serde_json::from_slice(&bounded(review)?).map_err(|e| e.to_string())?;
        serde_json::to_vec_pretty(&project(&bounded(report)?, &review)?)
            .map_err(|e| e.to_string())?
    } else {
        return Err("supply complete rebuild or reviewed projection inputs".into());
    };
    if value.len() > 2 * MAX_STUDY {
        return Err("output exceeds evidence bound".into());
    }
    write_private(Path::new(output), &value)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tempfile::TempDir;
    fn retained(root: &Path, path: &str, bytes: &[u8]) -> Reference {
        fs::write(root.join(path), bytes).unwrap();
        Reference {
            path: path.into(),
            sha256: digest(bytes),
        }
    }
    fn attempt(root: &Path, id: &str, status: Status) -> Attempt {
        let session = atif::Session::opening(
            id,
            "synthetic-model",
            "synthetic-provider",
            "synthetic-private-name",
            "revision",
        );
        let file = root.join(format!("{id}.jsonl"));
        let mut log = atif::Log::create_at(&file, &session).unwrap();
        log.append(&atif::Step::said(
            atif::Source::User,
            "synthetic-private-name task",
        ))
        .unwrap();
        log.finish(atif::log::ENDED).unwrap();
        drop(log);
        let trace = Reference {
            path: format!("{id}.jsonl"),
            sha256: digest(&fs::read(file).unwrap()),
        };
        let artifact = retained(root, &format!("{id}.patch"), id.as_bytes());
        let evidence = retained(root, &format!("{id}.check"), b"synthetic check result");
        Attempt {
            id: id.into(),
            kind: AttemptKind::Primary,
            parent: None,
            executor: "executor".into(),
            trace,
            artifact,
            checks: BTreeMap::from([(
                "check".into(),
                Check {
                    check_digest: "c".repeat(64),
                    status,
                    evidence: Some(evidence),
                },
            )]),
            setup_ms: 10,
            queue_ms: 20,
            check_ms: 30,
            support_ms: 40,
            costs: vec![],
            compute: None,
            acceptance: None,
        }
    }
    pub(crate) fn fixture() -> (TempDir, Manifest) {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let baseline = attempt(root, "baseline", Status::Passed);
        let failure = attempt(root, "candidate-failed", Status::Failed);
        let mut repair = attempt(root, "candidate-repair", Status::Passed);
        repair.kind = AttemptKind::Repair;
        repair.parent = Some(failure.id.clone());
        repair.acceptance = Some(Acceptance {
            candidate_digest: repair.artifact.sha256.clone(),
            independent_checker: "checker".into(),
            check_review: retained(root, "review", b"checked exact candidate"),
            customer_decision: retained(root, "customer", b"synthetic-private-name accepted"),
        });
        repair.costs = vec![
            Cost {
                component: CostComponent::Provider,
                basis: CostBasis::ListPrice,
                unit: "USD_millionths".into(),
                amount: Some(50),
                evidence: Some(retained(root, "usage", b"synthetic usage")),
                price: Some(Price {
                    version: "synthetic-price-v1".into(),
                    provenance: retained(root, "price", b"synthetic pinned price"),
                }),
            },
            Cost {
                component: CostComponent::Compute,
                basis: CostBasis::SubscriptionCapacity,
                unit: "subscription_capacity_units".into(),
                amount: Some(99),
                evidence: Some(retained(root, "capacity", b"prepaid capacity consumed")),
                price: None,
            },
        ];
        let inventory = Inventory {
            schema: "openagents.gym.sales-inventory.v1".into(),
            attempts: BTreeMap::from([
                ("task/baseline".into(), vec![baseline.id.clone()]),
                (
                    "task/candidate".into(),
                    vec![failure.id.clone(), repair.id.clone()],
                ),
            ]),
        };
        let manifest = Manifest {
            schema: SCHEMA.into(),
            offer_version: "openagents.sales.coder-pilot.v1".into(),
            source_revision: "a".repeat(40),
            baseline_method: "manual".into(),
            candidate_method: "Coder".into(),
            retrospective_selection: false,
            inventory: retained(
                root,
                "inventory.json",
                &serde_json::to_vec(&inventory).unwrap(),
            ),
            gym_store: None,
            tasks: vec![Task {
                id: "task".into(),
                task_digest: "b".repeat(64),
                check_digests: BTreeMap::from([("check".into(), "c".repeat(64))]),
                baseline: vec![baseline],
                candidate: vec![failure, repair],
            }],
            skipped_evidence: vec!["hosting invoice unavailable".into()],
        };
        (dir, manifest)
    }
    fn build(dir: &TempDir, m: &Manifest) -> Result<Report, String> {
        rebuild(dir.path(), &serde_json::to_vec(m).unwrap())
    }
    #[test]
    fn rebuild_retains_failures_repairs_time_and_uncertain_cost() {
        let (dir, m) = fixture();
        let a = build(&dir, &m).unwrap();
        let b = build(&dir, &m).unwrap();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        assert_eq!(a.candidate.attempts, 2);
        assert_eq!(a.candidate.repairs, 1);
        assert_eq!(a.candidate.failed_checks, 1);
        assert_eq!(a.candidate.accepted_tasks, 1);
        assert_eq!(a.candidate.support_ms, 80);
        assert_eq!(a.candidate.setup_ms, 20);
        assert_eq!(a.candidate.compute_unknown_attempts, 2);
        assert!(
            a.candidate
                .costs
                .iter()
                .any(|c| c.component == CostComponent::Support && c.unknown_items == 2)
        );
        assert!(
            a.candidate
                .costs
                .iter()
                .any(|c| c.basis == CostBasis::ListPrice && c.known_subtotal == 50)
        );
        assert!(!a.deployed_routing_improvement);
    }
    #[test]
    fn inventory_omission_and_duplicate_sessions_refuse() {
        let (dir, mut m) = fixture();
        m.tasks[0].candidate.remove(0);
        m.tasks[0].candidate[0].kind = AttemptKind::Primary;
        m.tasks[0].candidate[0].parent = None;
        assert!(build(&dir, &m).unwrap_err().contains("inventory attempt"));
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[0].trace = m.tasks[0].baseline[0].trace.clone();
        assert!(build(&dir, &m).unwrap_err().contains("counted twice"));
    }
    #[test]
    fn exact_acceptance_and_independence_are_required() {
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1]
            .acceptance
            .as_mut()
            .unwrap()
            .independent_checker = "executor".into();
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1]
            .checks
            .get_mut("check")
            .unwrap()
            .status = Status::Unknown;
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1]
            .acceptance
            .as_mut()
            .unwrap()
            .candidate_digest = "d".repeat(64);
        assert!(build(&dir, &m).is_err());
    }
    #[test]
    fn changed_checks_and_incomplete_trace_stay_unaccepted() {
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1]
            .checks
            .get_mut("check")
            .unwrap()
            .check_digest = "d".repeat(64);
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        let r = &mut m.tasks[0].candidate[0].trace;
        let path = dir.path().join(&r.path);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"{incomplete");
        fs::write(path, &bytes).unwrap();
        r.sha256 = digest(&bytes);
        let report = build(&dir, &m).unwrap();
        assert_eq!(report.candidate.incomplete_traces, 1);
        let acceptance = m.tasks[0].candidate[1].acceptance.clone();
        m.tasks[0].candidate[0].acceptance = acceptance;
        assert!(build(&dir, &m).is_err());
    }
    #[test]
    fn retained_digests_traversal_and_symlinks_refuse() {
        let (dir, mut m) = fixture();
        fs::write(dir.path().join("candidate-repair.patch"), b"changed").unwrap();
        assert!(build(&dir, &m).is_err());
        m.inventory.path = "../inventory.json".into();
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        std::os::unix::fs::symlink("inventory.json", dir.path().join("alias")).unwrap();
        m.inventory.path = "alias".into();
        assert!(build(&dir, &m).is_err());
    }
    #[test]
    fn public_export_requires_exact_review_and_removes_private_text() {
        let (dir, mut m) = fixture();
        m.retrospective_selection = true;
        let report = serde_json::to_vec(&build(&dir, &m).unwrap()).unwrap();
        let mut review = PublicReview {
            schema: "openagents.gym.sales-review.v1".into(),
            report_digest: digest(&report),
            reviewer: "human".into(),
            approved: false,
        };
        assert!(project(&report, &review).is_err());
        review.approved = true;
        let public = project(&report, &review).unwrap();
        let text = serde_json::to_string(&public).unwrap();
        for private in [
            "synthetic-private-name",
            "candidate-repair",
            "synthetic-provider",
            "customer_decision",
            "task_digest",
        ] {
            assert!(!text.contains(private));
        }
        assert!(text.contains("Retrospective"));
        assert!(!public.deployed_routing_improvement);
        review.report_digest = "e".repeat(64);
        assert!(project(&report, &review).is_err());
    }
    #[test]
    fn capacity_never_becomes_cash_and_totals_check_overflow() {
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1].costs[1].unit = "USD_millionths".into();
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[0].support_ms = u64::MAX;
        assert!(build(&dir, &m).is_err());
        let (dir, mut m) = fixture();
        m.tasks[0].candidate[1].costs[0].price = None;
        assert!(build(&dir, &m).is_err());
    }
    #[test]
    fn cli_private_output_is_exclusive_and_projection_is_separate() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, m) = fixture();
        let manifest = dir.path().join("manifest.json");
        fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
        let output = dir.path().join("report.json");
        let args = vec![
            "--root".into(),
            dir.path().to_str().unwrap().into(),
            "--manifest".into(),
            manifest.to_str().unwrap().into(),
            "--output".into(),
            output.to_str().unwrap().into(),
        ];
        command(&args).unwrap();
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(command(&args).is_err());
        let report = fs::read(&output).unwrap();
        let review = PublicReview {
            schema: "openagents.gym.sales-review.v1".into(),
            report_digest: digest(&report),
            reviewer: "human".into(),
            approved: true,
        };
        let review_path = dir.path().join("review.json");
        fs::write(&review_path, serde_json::to_vec(&review).unwrap()).unwrap();
        let public = dir.path().join("public.json");
        command(&[
            "--report".into(),
            output.to_str().unwrap().into(),
            "--review".into(),
            review_path.to_str().unwrap().into(),
            "--output".into(),
            public.to_str().unwrap().into(),
        ])
        .unwrap();
        assert!(public.exists());
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn unknown_totals_do_not_turn_into_zero_cost() {
        let mut totals = Totals::default();
        accumulate(
            &mut totals,
            CostComponent::Provider,
            CostBasis::Billed,
            "USD_millionths",
            Some(u64::MAX),
        )
        .unwrap();
        assert!(
            accumulate(
                &mut totals,
                CostComponent::Provider,
                CostBasis::Billed,
                "USD_millionths",
                Some(1)
            )
            .is_err()
        );
        accumulate(
            &mut totals,
            CostComponent::Support,
            CostBasis::Unknown,
            "undeclared",
            None,
        )
        .unwrap();
        assert_eq!(
            totals
                .costs
                .iter()
                .find(|c| c.component == CostComponent::Support)
                .unwrap()
                .unknown_items,
            1
        );
    }
}
