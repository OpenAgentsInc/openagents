//! Current team reports over native receipts, statements, and rebuilt private evidence.
//! These reads attach no execution, purchase, payment, or publication authority.
use crate::{
    accounts, config, dashboard, money,
    serve::{Caller, Context, Naming, ServeState},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get, post},
};
use receipts::execution::{ExecutionReceipt, Member, Outcome, digest_request};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use tenancy::{
    Accounts, MemberRef, Role,
    accounts::{
        MemberStatus, Store,
        team_reports::{Evidence, Reference, Verified},
    },
    money::{Hold, Phase, Statement},
};
const MAX_LOG: u64 = 64 * 1024 * 1024;
const MAX_ROWS: usize = 200;
const MAX_EVIDENCE: u64 = 256 * 1024 * 1024;
const EVIDENCE_BUDGET: &str = "Team evidence read budget exhausted.";
// Gym may read one final bounded file before refusing an aggregate or file-size
// overflow. Reserve that rejection read as well as its complete source allowance.
const REBUILD_READS: u64 = gym::sales_evidence::MAX_SOURCES + gym::sales_evidence::MAX_FILE + 1;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub evidence_root: PathBuf,
}
impl Config {
    pub fn check(&self, config: &config::Config) -> Result<(), String> {
        if config.accounts.is_none()
            || !config.require_workspace_membership
            || config
                .money
                .as_ref()
                .is_none_or(|m| !m.doors.values().any(|p| p.offer.is_some()))
            || !self.evidence_root.is_absolute()
        {
            return Err("Team reports require native membership, a selected decision offer, and an absolute private evidence root.".into());
        }
        private_root(&self.evidence_root)?;
        Ok(())
    }
}
fn private_root(root: &FsPath) -> Result<fs::Metadata, String> {
    let m = fs::symlink_metadata(root).map_err(|_| "Private team evidence root is unavailable.")?;
    if !m.is_dir() || m.mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
        return Err("Team evidence requires an owner-private directory.".into());
    }
    if root
        .canonicalize()
        .map_err(|_| "Cannot resolve team evidence root.")?
        != root
    {
        return Err("Team evidence root cannot traverse symlinks.".into());
    }
    Ok(m)
}
/// Only the native admission path creates live phase observations. Restart drops them.
#[derive(Default)]
pub(crate) struct Monitor(Mutex<BTreeMap<String, Progress>>);
#[derive(Clone)]
struct Progress {
    member: Member,
    request: String,
    attempt: u32,
    request_digest: String,
    artifact: String,
    phase: &'static str,
    wait_ms: Option<u64>,
}
pub(crate) struct ProgressGuard<'a> {
    monitor: &'a Monitor,
    key: String,
    start: Instant,
}
impl ProgressGuard<'_> {
    pub(crate) fn wait_ms(&self) -> Option<u64> {
        self.monitor
            .0
            .lock()
            .expect("team progress")
            .get(&self.key)
            .and_then(|p| p.wait_ms)
    }
    pub(crate) fn running(&self) -> u64 {
        let wait = self.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        if let Some(p) = self
            .monitor
            .0
            .lock()
            .expect("team progress")
            .get_mut(&self.key)
        {
            p.phase = "running";
            p.wait_ms = Some(wait);
        }
        wait
    }
}
impl Drop for ProgressGuard<'_> {
    fn drop(&mut self) {
        self.monitor
            .0
            .lock()
            .expect("team progress")
            .remove(&self.key);
    }
}
pub(crate) fn begin<'a>(
    state: &'a ServeState,
    caller: &Caller,
    naming: &Naming<'_>,
    ctx: &Context,
    hold: &Option<money::Hold>,
) -> Option<ProgressGuard<'a>> {
    if state.config.team_reports.is_none() || !hold.as_ref().is_some_and(|h| h.offer.is_some()) {
        return None;
    }
    let member = caller.member.clone()?;
    let key = format!("{}:{}#{}", member.workspace, naming.request, naming.attempt);
    let mut entries = state.team_progress.0.lock().expect("team progress");
    if entries.len() >= 4096 || entries.contains_key(&key) {
        return None;
    }
    entries.insert(
        key.clone(),
        Progress {
            member,
            request: naming.request.into(),
            attempt: naming.attempt,
            request_digest: naming.request_digest.into(),
            artifact: ctx.requested.artifact_signature.clone(),
            phase: "submitted",
            wait_ms: None,
        },
    );
    Some(ProgressGuard {
        monitor: &state.team_progress,
        key,
        start: Instant::now(),
    })
}
fn opaque(value: &str) -> String {
    digest_request(&json!(value))
}
fn value_digest(value: &impl Serialize) -> String {
    digest_request(&serde_json::to_value(value).expect("native projection serializes"))
}
#[derive(Clone, Debug, Serialize)]
pub struct Cost {
    pub component: gym::sales_evidence::CostComponent,
    pub basis: gym::sales_evidence::CostBasis,
    pub denomination: String,
    pub amount: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    pub reference: String,
    pub kind: gym::sales_evidence::AttemptKind,
    pub failed_checks: u64,
    pub unknown_checks: u64,
    pub setup_ms: u64,
    pub queue_ms: u64,
    pub check_ms: u64,
    pub support_ms: u64,
    pub costs: Vec<Cost>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Comparison {
    pub baseline: Vec<Attempt>,
    pub candidate: Vec<Attempt>,
    pub production_improvement: bool,
}
fn attempts(rows: &[gym::sales_evidence::Attempt]) -> Vec<Attempt> {
    rows.iter()
        .map(|a| {
            let mut costs: Vec<_> = a
                .costs
                .iter()
                .map(|c| Cost {
                    component: c.component,
                    basis: c.basis,
                    denomination: match c.unit.as_str() {
                        "USD_millionths" | "subscription_capacity_units" => c.unit.clone(),
                        _ => opaque(&c.unit),
                    },
                    amount: c.amount,
                })
                .collect();
            for component in [
                gym::sales_evidence::CostComponent::Provider,
                gym::sales_evidence::CostComponent::Compute,
                gym::sales_evidence::CostComponent::Support,
            ] {
                if !costs.iter().any(|c| c.component == component) {
                    costs.push(Cost {
                        component,
                        basis: gym::sales_evidence::CostBasis::Unknown,
                        denomination: "unavailable".into(),
                        amount: None,
                    });
                }
            }
            Attempt {
                reference: opaque(&a.id),
                kind: a.kind,
                failed_checks: a
                    .checks
                    .values()
                    .filter(|c| c.status == gym::sales_evidence::Status::Failed)
                    .count() as u64,
                unknown_checks: a
                    .checks
                    .values()
                    .filter(|c| {
                        matches!(
                            c.status,
                            gym::sales_evidence::Status::Unknown
                                | gym::sales_evidence::Status::Skipped
                        )
                    })
                    .count() as u64,
                setup_ms: a.setup_ms,
                queue_ms: a.queue_ms,
                check_ms: a.check_ms,
                support_ms: a.support_ms,
                costs,
            }
        })
        .collect()
}
#[derive(Clone, Debug, Serialize)]
pub struct TaskEvidence {
    pub status: &'static str,
    pub manifest: Option<String>,
    pub task: Option<String>,
    pub candidate: Option<String>,
    pub failed_checks: Option<u64>,
    pub unknown_checks: Option<u64>,
    pub accepted: bool,
    pub checker: Option<String>,
    pub acceptance: Option<String>,
    pub attributed_review: bool,
    pub independent_remote_attestation: bool,
    pub comparison: Option<Comparison>,
}
impl TaskEvidence {
    fn unavailable(status: &'static str) -> Self {
        Self {
            status,
            manifest: None,
            task: None,
            candidate: None,
            failed_checks: None,
            unknown_checks: None,
            accepted: false,
            checker: None,
            acceptance: None,
            attributed_review: false,
            independent_remote_attestation: false,
            comparison: None,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub task: String,
    pub payer_workspace: String,
    pub original_member: Option<Member>,
    pub receipt: Option<String>,
    pub team_policy_reference: Option<String>,
    pub budget_policy_reference: Option<String>,
    pub state: String,
    pub service_outcome: Option<Outcome>,
    pub requested_artifact: Option<String>,
    pub served_artifact: Option<String>,
    pub model_reference: Option<String>,
    pub plugin_release: Option<String>,
    pub placement: &'static str,
    pub wait_ms: Option<u64>,
    pub total_service_ms: Option<u64>,
    pub resolved_at: Option<String>,
    pub hold_phase: Phase,
    pub hold_reference: String,
    pub price_reference: String,
    pub statement_reference: String,
    pub reserved: u64,
    pub charged: Option<u64>,
    pub refunded: u64,
    pub provider_cost: Option<u64>,
    pub hosting_cost: Option<u64>,
    pub evidence: TaskEvidence,
}
#[derive(Default, Debug, Serialize)]
pub struct Totals {
    pub tasks: u64,
    pub accepted: u64,
    pub failed: u64,
    pub delivered: u64,
    pub unknown_charges: u64,
    pub known_charges: u64,
    pub refunded: u64,
    pub unknown_provider_costs: u64,
    pub known_provider_costs: u64,
    pub unknown_hosting_costs: u64,
    pub known_hosting_costs: u64,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub workspace: String,
    pub scope: &'static str,
    pub account_revision: String,
    pub statement_reference: String,
    pub receipt_log_reference: String,
    pub unit: tenancy::money::funding::Unit,
    pub rows: Vec<Row>,
    pub totals: Totals,
    pub more: bool,
    pub maximum_rows: usize,
    pub wallet_liquidity: Option<u64>,
    pub production_qualification: bool,
}
fn sum(target: &mut u64, value: u64) -> Result<(), String> {
    *target = target
        .checked_add(value)
        .ok_or("Native report total overflow.")?;
    Ok(())
}
struct NativeSources {
    receipts: BTreeMap<String, ExecutionReceipt>,
    log_reference: String,
    attempts: BTreeMap<String, Vec<String>>,
}
impl NativeSources {
    fn read(state: &ServeState, workspace: &str, held: &fs::File) -> Result<Self, String> {
        receipt_custody(state, held)?;
        let path = state.dir.join("receipts.jsonl");
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| "Native receipt log is unavailable.")?;
        let m = file.metadata().map_err(|e| e.to_string())?;
        let source = held
            .metadata()
            .map_err(|_| "Held receipt source is unavailable.")?;
        if (source.dev(), source.ino()) != (m.dev(), m.ino())
            || !m.is_file()
            || m.nlink() != 1
            || m.len() > MAX_LOG
        {
            return Err("Native receipt log exceeds its regular-file bound.".into());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_LOG + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_LOG || (!bytes.is_empty() && bytes.last() != Some(&b'\n')) {
            return Err("Native receipt log is incomplete or oversized.".into());
        }
        let after = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if m.ino() != after.ino() || m.dev() != after.dev() {
            return Err("Native receipt log custody changed.".into());
        }
        let mut receipts = BTreeMap::new();
        let mut lines = 0;
        for line in std::str::from_utf8(&bytes)
            .map_err(|_| "Native receipt log is not UTF-8.")?
            .lines()
        {
            lines += 1;
            if lines > 100_000 {
                return Err("Native receipt scan exceeds its bound.".into());
            }
            let receipt =
                ExecutionReceipt::parse(line).map_err(|_| "Native receipt verification failed.")?;
            if receipt.workspace.as_deref() == Some(workspace) {
                receipts.entry(receipt.digest.clone()).or_insert(receipt);
            }
        }
        // The public pin covers only this workspace's verified receipts.
        receipt_custody(state, held)?;
        let log_reference = value_digest(&receipts);
        let mut attempts: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in receipts.values().filter(|r| r.usage.is_some()) {
            attempts
                .entry(format!("{}#{}", r.request, r.attempt))
                .or_default()
                .push(r.digest.clone());
        }
        Ok(Self {
            receipts,
            log_reference,
            attempts,
        })
    }
    fn original<'a>(
        &'a self,
        attempt: &str,
        hold: &Hold,
    ) -> Result<Option<&'a ExecutionReceipt>, String> {
        let found: Vec<_> = self
            .attempts
            .get(attempt)
            .into_iter()
            .flatten()
            .filter_map(|key| self.receipts.get(key))
            .filter(|r| {
                r.request_digest == hold.request_digest && r.requested.model == hold.price.model
            })
            .collect();
        if found.len() > 1 {
            return Err("Conflicting original native task receipts.".into());
        }
        Ok(found.first().copied())
    }
}
fn receipt_custody(state: &ServeState, held: &fs::File) -> Result<(), String> {
    let m = held
        .metadata()
        .map_err(|_| "Held receipt source is unavailable.")?;
    let visible = fs::symlink_metadata(state.dir.join("receipts.jsonl"))
        .map_err(|_| "Native receipt source is unavailable.")?;
    if !visible.is_file()
        || visible.nlink() != 1
        || visible.uid() != unsafe { libc::geteuid() }
        || (m.dev(), m.ino()) != (visible.dev(), visible.ino())
    {
        return Err("Held native receipt source custody changed.".into());
    }
    Ok(())
}
fn private_read(root: &FsPath, r: &Reference, maximum: u64) -> Result<Vec<u8>, String> {
    let initial = private_root(root)?;
    let mut path = root.to_path_buf();
    for c in FsPath::new(&r.path).components() {
        let Component::Normal(part) = c else {
            return Err("Evidence must remain under its private root.".into());
        };
        path.push(part);
        let m = fs::symlink_metadata(&path).map_err(|_| "Current evidence is missing.")?;
        if m.file_type().is_symlink()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
        {
            return Err("Evidence is outside private custody.".into());
        }
    }
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|_| "Current evidence is unavailable.")?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.len() > maximum
    {
        return Err("Evidence must be a bounded private regular file.".into());
    }
    let mut b = Vec::new();
    (&mut f)
        .take(maximum + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    let after = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    let end = private_root(root)?;
    if b.len() as u64 > maximum
        || gym::sales_evidence::digest(&b) != r.sha256
        || after.mode() & 0o077 != 0
        || after.uid() != unsafe { libc::geteuid() }
        || after.nlink() != 1
        || m.ino() != after.ino()
        || m.dev() != after.dev()
        || initial.ino() != end.ino()
        || initial.dev() != end.dev()
    {
        return Err("Current pinned evidence changed.".into());
    }
    Ok(b)
}
fn references(m: &gym::sales_evidence::Manifest) -> Vec<Reference> {
    let mut refs = vec![Reference {
        path: m.inventory.path.clone(),
        sha256: m.inventory.sha256.clone(),
    }];
    if let Some(r) = &m.gym_store {
        refs.push(Reference {
            path: r.path.clone(),
            sha256: r.sha256.clone(),
        });
    }
    for task in &m.tasks {
        for a in task.baseline.iter().chain(&task.candidate) {
            for r in [&a.trace, &a.artifact]
                .into_iter()
                .chain(a.checks.values().filter_map(|c| c.evidence.as_ref()))
                .chain(a.costs.iter().filter_map(|c| c.evidence.as_ref()))
                .chain(
                    a.costs
                        .iter()
                        .filter_map(|c| c.price.as_ref().map(|p| &p.provenance)),
                )
                .chain(a.compute.as_ref().map(|c| &c.evidence))
                .chain(
                    a.acceptance
                        .as_ref()
                        .into_iter()
                        .flat_map(|a| [&a.check_review, &a.customer_decision]),
                )
            {
                refs.push(Reference {
                    path: r.path.clone(),
                    sha256: r.sha256.clone(),
                });
            }
        }
    }
    refs
}
fn reserve_evidence(spent: &mut u64, amount: u64) -> Result<(), String> {
    let next = spent.checked_add(amount).ok_or(EVIDENCE_BUDGET)?;
    if next > MAX_EVIDENCE {
        return Err(EVIDENCE_BUDGET.into());
    }
    *spent = next;
    Ok(())
}
fn bounded_evidence(
    root: &FsPath,
    r: &Reference,
    maximum: u64,
    spent: &mut u64,
) -> Result<Vec<u8>, String> {
    let remaining = MAX_EVIDENCE.checked_sub(*spent).ok_or(EVIDENCE_BUDGET)?;
    if remaining == 0 {
        return Err(EVIDENCE_BUDGET.into());
    }
    let maximum = maximum.min(remaining - 1);
    let reserved = maximum + 1;
    // Failed or changed reads retain their whole reservation. Successful reads
    // release only bytes the bounded descriptor reader did not consume.
    reserve_evidence(spent, reserved)?;
    let bytes = private_read(root, r, maximum)?;
    *spent -= reserved - bytes.len() as u64;
    Ok(bytes)
}
fn checked_evidence(
    state: &ServeState,
    e: &Evidence,
    receipt: &ExecutionReceipt,
    hold: &Hold,
    spent: &mut u64,
) -> Result<TaskEvidence, String> {
    e.validate()?;
    let root = &state
        .config
        .team_reports
        .as_ref()
        .ok_or("Team evidence is disabled.")?
        .evidence_root;
    if receipt.digest != e.receipt
        || receipt.member.is_none()
        || receipt.request_digest != hold.request_digest
        || receipt.served.model != "kev-0.6b"
        || !state.config.money.as_ref().is_some_and(|m| {
            m.doors.values().any(|p| {
                p.offer.as_ref().is_some_and(|o| {
                    o.identity.artifact_signature == receipt.served.artifact_signature
                }) && p.price == hold.price
            })
        })
    {
        return Err("Evidence requires the exact currently qualified original native decision receipt and price.".into());
    }
    reserve_evidence(spent, REBUILD_READS)?;
    let bytes = bounded_evidence(root, &e.manifest, 1024 * 1024, spent)?;
    let manifest: gym::sales_evidence::Manifest =
        serde_json::from_slice(&bytes).map_err(|_| "Pinned task manifest is invalid.")?;
    let refs = references(&manifest);
    for r in &refs {
        bounded_evidence(root, r, 8 * 1024 * 1024, spent)?;
    }
    let report = gym::sales_evidence::rebuild(root, &bytes)
        .map_err(|_| "Current pinned task evidence cannot be rebuilt.")?;
    let task = report
        .manifest
        .tasks
        .iter()
        .find(|t| t.id == e.task)
        .ok_or("The exact task is absent from the frozen inventory.")?;
    let a = task
        .candidate
        .iter()
        .find(|a| a.id == e.candidate)
        .ok_or("The exact candidate is absent from the frozen inventory.")?;
    let trace = bounded_evidence(
        root,
        &Reference {
            path: a.trace.path.clone(),
            sha256: a.trace.sha256.clone(),
        },
        8 * 1024 * 1024,
        spent,
    )?;
    let recording = atif::log::read_bytes(FsPath::new(&a.trace.path), &trace)
        .map_err(|_| "ATIF source is invalid.")?;
    if !recording.whole() {
        return Err("ATIF source is incomplete.".into());
    }
    let matches: Vec<_> = recording
        .steps
        .iter()
        .filter_map(|s| s.call.as_ref())
        .filter(|c| {
            c.is_decision()
                && c.extra.get("request_id").and_then(Value::as_str) == Some(&receipt.request)
                && c.extra.get("native_attempt").and_then(Value::as_u64)
                    == Some(u64::from(receipt.attempt))
                && digest_request(&c.arguments) == receipt.request_digest
                && c.extra.get("native_receipt").and_then(Value::as_str) == Some(&receipt.digest)
        })
        .collect();
    if matches.len() != 1 {
        return Err(
            "Task ATIF must bind exactly one original native request, attempt, envelope, and sealed receipt.".into(),
        );
    }
    let call = matches[0];
    let response: Value = serde_json::from_str(&call.output)
        .map_err(|_| "Task response is not exact native JSON.")?;
    if receipt.result_digest.as_deref()
        != Some(
            format!(
                "sha256:{}",
                gym::sales_evidence::digest(call.output.as_bytes())
            )
            .as_str(),
        )
        || receipt.result_digest.as_deref()
            != Some(format!("sha256:{}", a.artifact.sha256).as_str())
        || response.get("model").and_then(Value::as_str) != Some(&receipt.served.model)
    {
        return Err("Candidate output does not bind the exact native response artifact.".into());
    }
    let accepted = a.acceptance.is_some();
    if accepted && receipt.outcome != Outcome::Answered {
        return Err("A failed native service task cannot be accepted as delivered.".into());
    }
    let checker = a
        .acceptance
        .as_ref()
        .map(|a| opaque(&a.independent_checker));
    let acceptance = a.acceptance.as_ref().map(|a| {
        value_digest(&(
            a.check_review.sha256.clone(),
            a.customer_decision.sha256.clone(),
            a.candidate_digest.clone(),
        ))
    });
    // Never publish raw paths, task labels, checker names, customer text, or ATIF content.
    let comparison = Comparison {
        baseline: attempts(&task.baseline),
        candidate: attempts(&task.candidate),
        production_improvement: false,
    };
    // Pin all bytes again after the rebuild so a changed source cannot retain an old accepted projection.
    bounded_evidence(root, &e.manifest, 1024 * 1024, spent)?;
    for r in &refs {
        bounded_evidence(root, r, 8 * 1024 * 1024, spent)?;
    }
    Ok(TaskEvidence {
        status: "current_verified",
        manifest: Some(format!("sha256:{}", report.manifest_digest)),
        task: Some(opaque(&task.id)),
        candidate: Some(format!("sha256:{}", a.artifact.sha256)),
        failed_checks: Some(
            a.checks
                .values()
                .filter(|c| c.status == gym::sales_evidence::Status::Failed)
                .count() as u64,
        ),
        unknown_checks: Some(
            a.checks
                .values()
                .filter(|c| {
                    matches!(
                        c.status,
                        gym::sales_evidence::Status::Skipped | gym::sales_evidence::Status::Unknown
                    )
                })
                .count() as u64,
        ),
        accepted,
        checker,
        acceptance,
        attributed_review: accepted,
        independent_remote_attestation: false,
        comparison: Some(comparison),
    })
}
pub(crate) fn actor(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
    store: &Store,
) -> Result<MemberRef, String> {
    let p = accounts::principal(state, headers)
        .map_err(|_| "A current native reporting credential is required.")?;
    let account = accounts::member_account(&p).map_err(|_| "A native account is required.")?;
    let ws = store
        .workspaces
        .get(workspace)
        .ok_or("Unknown native workspace.")?;
    let member = ws
        .members
        .get(account)
        .filter(|m| m.status == MemberStatus::Active)
        .ok_or("Current native workspace membership is required.")?;
    Ok(MemberRef {
        workspace: workspace.into(),
        account: account.into(),
        role: member.role,
        epoch: member.epoch,
        members_epoch: ws.members_epoch,
    })
}
async fn statement(state: &ServeState, workspace: &str) -> Result<Statement, String> {
    let ledger = state
        .money_lock()
        .await
        .ok_or("Native decision statement is unavailable.")?;
    ledger.check_source(
        &state
            .config
            .money
            .as_ref()
            .ok_or("Money source is unavailable.")?
            .ledger,
    )?;
    ledger.statement(workspace)
}
fn build(
    state: &ServeState,
    store: &Store,
    member: &MemberRef,
    statement: &Statement,
    receipt_source: &fs::File,
) -> Result<Report, String> {
    let native = NativeSources::read(state, &member.workspace, receipt_source)?;
    let mut statement_reference = value_digest(statement);
    let progress = state
        .team_progress
        .0
        .lock()
        .map_err(|_| "Native progress is unavailable.")?
        .clone();
    let admin = matches!(member.role, Role::Owner | Role::Admin);
    let mut rows = Vec::new();
    let mut spent = 0_u64;
    let mut more = false;
    for (attempt, hold) in &statement.holds {
        let receipt = native.original(attempt, hold)?;
        let live = progress.values().find(|p| {
            p.member.workspace == member.workspace
                && format!("{}#{}", p.request, p.attempt) == *attempt
                && p.request_digest == hold.request_digest
        });
        let original = receipt
            .and_then(|r| r.member.clone())
            .or_else(|| live.map(|p| p.member.clone()));
        if !admin
            && original
                .as_ref()
                .is_none_or(|m| m.account != member.account)
        {
            continue;
        }
        if rows.len() == MAX_ROWS {
            more = true;
            break;
        }
        let evidence = if let Some(r) = receipt {
            if let Some(record) = store.team_reports.records.get(&r.digest) {
                checked_evidence(state, &record.evidence, r, hold, &mut spent).unwrap_or_else(
                    |reason| {
                        TaskEvidence::unavailable(if reason == EVIDENCE_BUDGET {
                            "read_budget_unavailable"
                        } else {
                            "source_unavailable_or_changed"
                        })
                    },
                )
            } else {
                TaskEvidence::unavailable("not_attached")
            }
        } else {
            TaskEvidence::unavailable("native_receipt_unavailable")
        };
        let status = if evidence.accepted {
            "accepted"
        } else if evidence.failed_checks.is_some_and(|n| n > 0) {
            "failed"
        } else if let Some(r) = receipt {
            if r.outcome == Outcome::Answered {
                "delivered"
            } else {
                "failed"
            }
        } else {
            live.map_or("unavailable", |p| p.phase)
        };
        rows.push(Row {
            task: opaque(attempt),
            payer_workspace: member.workspace.clone(),
            original_member: original,
            receipt: receipt.map(|r| r.digest.clone()),
            team_policy_reference: receipt
                .and_then(|r| r.team_policy.as_ref())
                .map(|s| s.policy.digest.clone()),
            budget_policy_reference: hold.budget.as_ref().map(|a| a.policy.clone()),
            state: status.into(),
            service_outcome: receipt.map(|r| r.outcome),
            requested_artifact: receipt
                .map(|r| r.requested.artifact_signature.clone())
                .or_else(|| live.map(|p| p.artifact.clone())),
            served_artifact: receipt
                .map(|r| r.served.artifact_signature.clone())
                .filter(|s| !s.is_empty()),
            model_reference: receipt.map(|r| opaque(&r.served.model)),
            plugin_release: None,
            placement: "native_gateway",
            wait_ms: receipt
                .and_then(|r| r.timing.queued_ms)
                .or_else(|| live.and_then(|p| p.wait_ms)),
            total_service_ms: receipt.and_then(|r| r.timing.latency_ms),
            resolved_at: receipt.and_then(|r| r.timing.resolved_at.clone()),
            hold_phase: hold.phase,
            hold_reference: value_digest(hold),
            price_reference: value_digest(&hold.price),
            statement_reference: statement_reference.clone(),
            reserved: hold.reserved,
            charged: hold.retail_charge,
            refunded: hold.refunded,
            provider_cost: hold.provider_cost,
            hosting_cost: hold.hosting_cost,
            evidence,
        });
    }

    let log_reference = if admin {
        native.log_reference
    } else {
        value_digest(
            &rows
                .iter()
                .filter_map(|r| r.receipt.clone())
                .collect::<Vec<_>>(),
        )
    };
    if !admin {
        statement_reference = value_digest(
            &rows
                .iter()
                .map(|r| (&r.hold_reference, &r.price_reference))
                .collect::<Vec<_>>(),
        );
        for row in &mut rows {
            row.statement_reference = statement_reference.clone();
        }
    }
    let mut totals = Totals::default();
    for r in &rows {
        sum(&mut totals.tasks, 1)?;
        match r.state.as_str() {
            "accepted" => sum(&mut totals.accepted, 1)?,
            "failed" => sum(&mut totals.failed, 1)?,
            "delivered" => sum(&mut totals.delivered, 1)?,
            _ => {}
        }
        if let Some(v) = r.charged {
            sum(&mut totals.known_charges, v)?
        } else {
            sum(&mut totals.unknown_charges, 1)?
        }
        sum(&mut totals.refunded, r.refunded)?;
        if let Some(v) = r.provider_cost {
            sum(&mut totals.known_provider_costs, v)?
        } else {
            sum(&mut totals.unknown_provider_costs, 1)?
        }
        if let Some(v) = r.hosting_cost {
            sum(&mut totals.known_hosting_costs, v)?
        } else {
            sum(&mut totals.unknown_hosting_costs, 1)?
        }
    }
    let report = Report {
        schema: "openagents.team-report.v1",
        workspace: member.workspace.clone(),
        scope: if admin {
            "workspace"
        } else {
            "own_original_tasks"
        },
        account_revision: store.digest.clone(),
        statement_reference,
        receipt_log_reference: log_reference,
        unit: statement.unit.clone(),
        rows,
        totals,
        more,
        maximum_rows: MAX_ROWS,
        wallet_liquidity: None,
        production_qualification: false,
    };
    if serde_json::to_vec(&report)
        .map_err(|e| e.to_string())?
        .len()
        > 2 * 1024 * 1024
    {
        return Err("Current report exceeds the bounded export size.".into());
    }
    Ok(report)
}
async fn project(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<Report, String> {
    // Authenticate before scanning any native financial or evidence source.
    let p = accounts::principal(state, headers)
        .map_err(|_| "A current native reporting credential is required.")?;
    let account = accounts::member_account(&p).map_err(|_| "A native account is required.")?;
    let accounts = Accounts::open(&state.dir).map_err(|e| e.to_string())?;
    accounts
        .authorize(workspace, account)
        .map_err(|_| "Current native workspace membership is required.")?;
    let receipt_source = state.receipt_source().await?;
    let statement = statement(state, workspace).await?;
    let (report, admission) = accounts.report_read(
        |store| actor(state, headers, workspace, store),
        |store, member| {
            Ok((
                build(state, store, member, &statement, &receipt_source)?,
                member.clone(),
            ))
        },
    )?;
    // No Accounts writer is held across an asynchronous financial read.
    {
        let ledger = state
            .money_lock()
            .await
            .ok_or("Money source is unavailable.")?;
        ledger.check_source(
            &state
                .config
                .money
                .as_ref()
                .ok_or("Money source is unavailable.")?
                .ledger,
        )?;
    }
    accounts.report_read(
        |store| actor(state, headers, workspace, store),
        |_, member| {
            if member != &admission {
                return Err("Reporting membership changed before export.".into());
            }
            receipt_custody(state, &receipt_source)?;
            Ok(report)
        },
    )
}
pub(crate) fn private_response(mut response: Response) -> Response {
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "vary",
        axum::http::HeaderValue::from_static("Authorization, Cookie"),
    );
    response
}
fn denied(e: String) -> Response {
    private_response(accounts::refused(
        StatusCode::FORBIDDEN,
        "team_report_unavailable",
        e,
    ))
}
async fn read(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    match project(&state, &headers, &workspace).await {
        Ok(r) => private_response(Json(r).into_response()),
        Err(e) => denied(e),
    }
}
async fn attach(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(e): Json<Evidence>,
) -> Response {
    let run=async {
        let p=accounts::principal(&state,&headers).map_err(|_|"A current native management credential is required.")?;
        let account=accounts::member_account(&p).map_err(|_|"A native account is required.")?;
        let accounts=Accounts::open(&state.dir).map_err(|e|e.to_string())?;
        let m=accounts.authorize(&workspace,account).map_err(|_|"Current native workspace membership is required.")?;
        if !matches!(m.role,Role::Owner|Role::Admin){return Err("Only a current owner or admin attaches evidence.".into());}
        let receipt_source=state.receipt_source().await?;
        let statement=statement(&state,&workspace).await?;
        let reference=e.clone();
        let record=accounts.report_attach(e,|store|actor(&state,&headers,&workspace,store),|_| {
            let native=NativeSources::read(&state,&workspace,&receipt_source)?;
            let receipt=native.receipts.get(&reference.receipt).ok_or("Exact native receipt is missing.")?;
            let member=receipt.member.as_ref().ok_or("Legacy receipts have no original native member attribution.")?;
            let hold=statement.holds.get(&format!("{}#{}",receipt.request,receipt.attempt)).ok_or("Exact native statement hold is missing.")?;
            checked_evidence(&state,&reference,receipt,hold,&mut 0)?;
            Ok(Verified {workspace:workspace.clone(),member:member.account.clone(),request:receipt.request.clone(),attempt:receipt.attempt})
        })?;
        // Only opaque pins cross this response; retained source labels remain private.
        Ok::<_,String>(json!({"schema":"openagents.team-evidence-attachment.v1","receipt":record.evidence.receipt,"binding":record.fingerprint,"account_revision":record.account_revision,"authority_granted":false,"charged":false}))
    }.await;
    match run {
        Ok(v) => private_response(Json(v).into_response()),
        Err(e) => denied(e),
    }
}
pub(crate) fn browser_headers(headers: &HeaderMap) -> Result<HeaderMap, String> {
    let token = dashboard::cookie_token(headers).ok_or("Sign in to read team reports.")?;
    let value = format!("Bearer {token}")
        .parse()
        .map_err(|_| "The native session is invalid.")?;
    let mut result = HeaderMap::new();
    result.insert("authorization", value);
    Ok(result)
}
async fn browser_export(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let headers = match browser_headers(&headers) {
        Ok(h) => h,
        Err(e) => return denied(e),
    };
    match project(&state, &headers, &workspace).await {
        Ok(r) => private_response(Json(r).into_response()),
        Err(e) => denied(e),
    }
}
pub(crate) async fn page(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let headers = match browser_headers(&headers) {
        Ok(h) => h,
        Err(e) => return denied(e),
    };
    match project(&state, &headers, &workspace).await {
        Ok(r) => {
            let mut body = String::from(
                "<h1>Team work report</h1><p>Known charges and measured native waits. Missing cost or phase evidence is unavailable. Acceptance is a pinned attributed review, not remote attestation.</p><table><tr><th>Task</th><th>State</th><th>Charge</th><th>Refunded</th><th>Wait (ms)</th><th>Provider cost</th><th>Evidence</th></tr>",
            );
            fn amount(v: Option<u64>) -> String {
                v.map_or("unknown".into(), |v| v.to_string())
            }
            for row in &r.rows {
                body.push_str(&format!("<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",dashboard::esc(&row.task),dashboard::esc(&row.state),amount(row.charged),row.refunded,amount(row.wait_ms),amount(row.provider_cost),row.evidence.status));
            }
            body.push_str(&format!("</table><p>Amounts use {} at scale {}. {} rows; bounded export includes the same current authorized scope and exact source pins.</p>",dashboard::esc(r.unit.currency()),r.unit.scale(),r.rows.len()));
            body.push_str(&format!("<p><a href=\"/dashboard/w/{}/reports/export\">Export current authorized report</a></p>",dashboard::esc(&workspace)));
            private_response(
                dashboard::page("Team work report", Some(&workspace), &body).into_response(),
            )
        }
        Err(e) => denied(e),
    }
}
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/workspaces/{workspace}/reports", get(read)),
        ("/v1/workspaces/{workspace}/reports/export", get(read)),
        ("/v1/workspaces/{workspace}/reports/evidence", post(attach)),
        ("/dashboard/w/{workspace}/reports", get(page)),
        (
            "/dashboard/w/{workspace}/reports/export",
            get(browser_export),
        ),
    ]
}
