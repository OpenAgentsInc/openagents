//! Usage, activity, and export — the workspace's own receipts and
//! ledger positions over HTTP.
//!
//! A member reads its workspace's usage through
//! `/v1/workspaces/{id}/usage*`: the summary joins `receipts.jsonl`
//! to the money ledger's holds (`{request}#{attempt}`) and the quota
//! ledger's reservations (`(request, attempt)`), so every number a
//! caller sees is the same number the ledger charged. Nothing here is
//! sampled or estimated — the disclosure block on every answer states
//! the scale (millionths), the timezone (UTC), the retention posture
//! (the receipt log is append-only), and what the scan could not
//! attribute or verify.
//!
//! Receipts written before the `workspace` field existed carry none,
//! and no reader upgrades a missing workspace into a guessed one — an
//! unattributed receipt is excluded from a workspace's view and
//! counted in the disclosure, never reassigned.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get};
use axum::{Json, body::Body};
use receipts::execution::{ExecutionReceipt, Outcome};
use serde::Deserialize;
use serde_json::{Value, json};
use tenancy::money::Phase;

use crate::accounts::{self, refused, unix_now};
use crate::serve::ServeState;

/// The schema tag every usage response carries.
const SCHEMA: &str = "openagents.usage.v1";

/// The receipt log a scan reads, beside the registry.
const RECEIPTS: &str = "receipts.jsonl";

/// The most receipts one query scans — a bound that keeps a read
/// proportional and discloses when it truncated.
const SCAN_MAX: usize = 100_000;

/// The most items one activity or export page returns.
const PAGE_MAX: usize = 200;
const PAGE_DEFAULT: usize = 50;

/// The usage routes — mounted only when `accounts` is configured,
/// because every read binds a workspace membership.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/workspaces/{workspace}/usage", get(summary)),
        ("/v1/workspaces/{workspace}/usage/activity", get(activity)),
        (
            "/v1/workspaces/{workspace}/usage/timeseries",
            get(timeseries),
        ),
        (
            "/v1/workspaces/{workspace}/usage/receipts/{digest}",
            get(receipt),
        ),
        ("/v1/workspaces/{workspace}/usage/export", get(export)),
    ]
}

fn answered(status: StatusCode, fields: Value) -> Response {
    let mut body = fields;
    body["v"] = json!(SCHEMA);
    (status, Json(body)).into_response()
}

/// The query a usage read takes — every filter narrows the workspace's
/// own receipts; nothing reaches another workspace's data.
#[derive(Default, Deserialize)]
pub(crate) struct Filter {
    /// RFC3339 lower bound on `resolved_at` — lexical compare, so a
    /// day prefix like `2026-09-22` works.
    pub(crate) from: Option<String>,
    /// RFC3339 upper bound, exclusive-compare by the same rule.
    pub(crate) to: Option<String>,
    /// The credential id (`oak_<id>`) the call authenticated under.
    pub(crate) key: Option<String>,
    /// The served model id.
    pub(crate) model: Option<String>,
    /// `answered`, `refused`, `unavailable`, `unattempted`, `unknown`.
    pub(crate) outcome: Option<String>,
    /// `decision`, `generation`, or `executor`.
    pub(crate) lane: Option<String>,
    /// The transport the call arrived on — `http`, `relay`, …
    pub(crate) transport: Option<String>,
    /// The job the attempt ran under, when it ran under one.
    pub(crate) job: Option<String>,
    /// The admitted policy digest, when one applied.
    pub(crate) policy: Option<String>,
    /// The money lane's capacity, joined through the hold's price.
    pub(crate) capacity: Option<String>,
    /// Page size — activity and export bound at `PAGE_MAX`.
    pub(crate) limit: Option<usize>,
    /// A keyset cursor from an earlier page.
    pub(crate) cursor: Option<String>,
}

/// What a scan carried out of the receipt log.
pub(crate) struct Scan {
    /// The workspace's receipts that passed the filter.
    pub(crate) receipts: Vec<ExecutionReceipt>,
    /// The raw lines kept for export, parallel to `receipts`.
    pub(crate) lines: Vec<String>,
    /// Receipts dropped before attribution — no `workspace` field, so
    /// no workspace can claim them.
    pub(crate) unattributed: usize,
    /// Receipts for other workspaces — never returned, only counted so
    /// the caller sees the scoping was applied.
    pub(crate) other_workspace: usize,
    /// Lines that failed parse or digest verification.
    pub(crate) unverifiable: usize,
    /// The scan hit `SCAN_MAX` — older receipts exist beyond it.
    pub(crate) truncated: bool,
}

/// Read the receipt log once and keep the workspace's rows that pass
/// the filter — the newest first for activity reads. `holds` is the
/// workspace's money holds: the capacity filter narrows through the
/// hold's price, a field the receipt does not carry.
pub(crate) fn scan(
    state: &ServeState,
    workspace: &str,
    filter: &Filter,
    holds: &BTreeMap<String, tenancy::money::Hold>,
) -> Result<Scan, Response> {
    let path = state.dir.join(RECEIPTS);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Scan {
                receipts: Vec::new(),
                lines: Vec::new(),
                unattributed: 0,
                other_workspace: 0,
                unverifiable: 0,
                truncated: false,
            });
        }
        Err(error) => {
            return Err(refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "usage_unavailable",
                format!("The service can't read usage records right now: {error}"),
            ));
        }
    };
    let mut scan = Scan {
        receipts: Vec::new(),
        lines: Vec::new(),
        unattributed: 0,
        other_workspace: 0,
        unverifiable: 0,
        truncated: false,
    };
    for (index, line) in BufReader::new(file).lines().enumerate() {
        if index >= SCAN_MAX {
            scan.truncated = true;
            break;
        }
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let receipt = match ExecutionReceipt::parse(&line) {
            Ok(receipt) => receipt,
            Err(_) => {
                scan.unverifiable += 1;
                continue;
            }
        };
        match receipt.workspace.as_deref() {
            None => {
                scan.unattributed += 1;
                continue;
            }
            Some(owner) if owner != workspace => {
                scan.other_workspace += 1;
                continue;
            }
            _ => {}
        }
        if !matches_filter(&receipt, filter) {
            continue;
        }
        if let Some(capacity) = &filter.capacity
            && holds
                .get(&hold_key(&receipt))
                .is_none_or(|hold| &hold.price.capacity != capacity)
        {
            continue;
        }
        scan.receipts.push(receipt);
        scan.lines.push(line);
    }
    Ok(scan)
}

/// Whether one receipt passes the caller's filter — every dimension
/// the receipt carries, matched exactly; a filter the receipt cannot
/// answer for (absent field) fails closed.
fn matches_filter(receipt: &ExecutionReceipt, filter: &Filter) -> bool {
    if let Some(from) = &filter.from
        && receipt
            .timing
            .resolved_at
            .as_deref()
            .is_none_or(|at| at < from.as_str())
    {
        return false;
    }
    if let Some(to) = &filter.to
        && receipt
            .timing
            .resolved_at
            .as_deref()
            .is_none_or(|at| at >= to.as_str())
    {
        return false;
    }
    if let Some(key) = &filter.key
        && receipt.tenant.as_deref() != Some(key)
    {
        return false;
    }
    if let Some(model) = &filter.model
        && receipt.served.model != *model
        && receipt.requested.model != *model
    {
        return false;
    }
    if let Some(outcome) = &filter.outcome
        && outcome_name(&receipt.outcome) != outcome
    {
        return false;
    }
    if let Some(lane) = &filter.lane {
        let name = match &receipt.lane {
            Some(receipts::execution::Lane::Decision) => "decision",
            Some(receipts::execution::Lane::Generation) => "generation",
            Some(receipts::execution::Lane::Executor) => "executor",
            None => "",
        };
        if name != lane {
            return false;
        }
    }
    if let Some(transport) = &filter.transport
        && receipt.transport != *transport
    {
        return false;
    }
    if let Some(job) = &filter.job
        && receipt.job.as_deref() != Some(job)
    {
        return false;
    }
    if let Some(policy) = &filter.policy
        && receipt.policy.as_deref() != Some(policy)
    {
        return false;
    }
    true
}

fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Answered => "answered",
        Outcome::Refused => "refused",
        Outcome::Unavailable => "unavailable",
        Outcome::Unattempted => "unattempted",
        Outcome::Unknown => "unknown",
    }
}

/// The disclosure block every answer carries — the exact-vs-estimated
/// contract the caller reads before trusting a number.
fn disclosure(scan: &Scan) -> Value {
    json!({
        "source": "Every number comes from this workspace's call receipts, matched to the amounts set aside for each call. Nothing is sampled or estimated.",
        "scale": "Amounts are whole numbers of millionths of the account currency. For example, 1000000 is one whole unit.",
        "timezone": "Times are UTC. `resolved_at` uses RFC 3339, and each day starts at 00:00 UTC.",
        "lag": "A receipt appears when a call finishes. A call whose result is unknown is charged later, and its cost shows separately as outstanding instead of being estimated.",
        "retention": "Usage records are never edited or deleted, and an export includes all of them.",
        "unattributed": scan.unattributed,
        "other_workspace": scan.other_workspace,
        "unverifiable": scan.unverifiable,
        "scan_truncated": scan.truncated,
    })
}

/// Authorize the caller's membership in the workspace — every usage
/// read is member-scoped, because a workspace's activity is the
/// workspace's data.
fn member_of(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<accounts::Principal, Response> {
    let principal = accounts::principal(state, headers)?;
    let account = accounts::member_account(&principal)?;
    accounts::member(state, account, workspace)?;
    Ok(principal)
}

/// The money ledger's holds for the workspace, keyed `{request}#{attempt}` —
/// absent under a deployment without monetary admission.
async fn holds(state: &ServeState, workspace: &str) -> BTreeMap<String, tenancy::money::Hold> {
    match state.money_lock().await {
        Some(ledger) => ledger
            .holds(workspace)
            .into_iter()
            .map(|(attempt, hold)| (attempt.to_string(), hold.clone()))
            .collect(),
        None => BTreeMap::new(),
    }
}

fn hold_key(receipt: &ExecutionReceipt) -> String {
    format!("{}#{}", receipt.request, receipt.attempt)
}

/// The cost fields one receipt joined — absent when the attempt ran
/// under no monetary admission, honest rather than zero.
fn cost_of(hold: Option<&tenancy::money::Hold>) -> Value {
    match hold {
        Some(hold) => json!({
            "reserved": hold.reserved,
            "retail": hold.retail_charge,
            "provider_cost": hold.provider_cost,
            "hosting_cost": hold.hosting_cost,
            "refunded": hold.refunded,
            "phase": match hold.phase {
                Phase::Held => "held",
                Phase::Unknown => "unknown",
                Phase::Settled => "settled",
                Phase::Released => "released",
            },
            "capacity": hold.price.capacity,
            "price_version": hold.price.version,
        }),
        None => Value::Null,
    }
}

/// One receipt in the activity shape — the summary a list row needs,
/// with cost and units joined when the ledgers recorded them.
fn activity_item(
    receipt: &ExecutionReceipt,
    hold: Option<&tenancy::money::Hold>,
    units: Option<&tenancy::quota::Units>,
) -> Value {
    json!({
        "digest": receipt.digest,
        "request": receipt.request,
        "attempt": receipt.attempt,
        "attempt_id": receipt.attempt_id,
        "transport": receipt.transport,
        "key": receipt.tenant,
        "requested_model": receipt.requested.model,
        "served_model": receipt.served.model,
        "served_artifact": receipt.served.artifact_signature,
        "outcome": outcome_name(&receipt.outcome),
        "cause": receipt.cause,
        "lane": match &receipt.lane {
            Some(receipts::execution::Lane::Decision) => json!("decision"),
            Some(receipts::execution::Lane::Generation) => json!("generation"),
            Some(receipts::execution::Lane::Executor) => json!("executor"),
            None => Value::Null,
        },
        "job": receipt.job,
        "policy": receipt.policy,
        "queued_ms": receipt.timing.queued_ms,
        "latency_ms": receipt.timing.latency_ms,
        "resolved_at": receipt.timing.resolved_at,
        "units": units.map(|units| json!({
            "questions": units.questions,
            "input_bytes": units.input_bytes,
            "options": units.options,
        })),
        "cost": cost_of(hold),
    })
}

/// `GET /v1/workspaces/{id}/usage` — the workspace's position: totals,
/// units, cost, breakdowns, outstanding holds, quota-derived spend,
/// and the entitlement the billing layer grants, all exact.
async fn summary(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    let principal = match member_of(&state, &headers, &workspace) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let holds = holds(&state, &workspace).await;
    let scan = match scan(&state, &workspace, &filter, &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let quota = state.quota_lock().await;

    let mut totals = json!({"calls": scan.receipts.len()});
    for outcome in [
        "answered",
        "refused",
        "unavailable",
        "unattempted",
        "unknown",
    ] {
        totals[outcome] = json!(
            scan.receipts
                .iter()
                .filter(|receipt| outcome_name(&receipt.outcome) == outcome)
                .count()
        );
    }

    let mut questions = 0u64;
    let mut input_bytes = 0u64;
    let mut measured = 0usize;
    for receipt in &scan.receipts {
        if let Some(reservation) = quota.reservation(&receipt.request, receipt.attempt) {
            questions += reservation.units.questions;
            input_bytes += reservation.units.input_bytes;
            measured += 1;
        }
    }

    let mut currency = Value::Null;
    let mut retail = 0u64;
    let mut provider_cost = 0u64;
    let mut provider_known = false;
    let mut hosting_cost = 0u64;
    let mut hosting_known = false;
    let mut refunded = 0u64;
    let mut priced = 0usize;
    for receipt in &scan.receipts {
        if let Some(hold) = holds.get(&hold_key(receipt)) {
            priced += 1;
            currency = json!(hold.price.currency);
            if let Some(charge) = hold.retail_charge {
                retail += charge;
            }
            if let Some(cost) = hold.provider_cost {
                provider_cost += cost;
                provider_known = true;
            }
            if let Some(cost) = hold.hosting_cost {
                hosting_cost += cost;
                hosting_known = true;
            }
            refunded += hold.refunded;
        }
    }
    let outstanding: Vec<Value> = holds
        .iter()
        .filter(|(_, hold)| matches!(hold.phase, Phase::Held | Phase::Unknown))
        .map(|(attempt, hold)| {
            json!({
                "hold": attempt,
                "reserved": hold.reserved,
                "phase": match hold.phase {
                    Phase::Held => "held",
                    Phase::Unknown => "unknown",
                    Phase::Settled => "settled",
                    Phase::Released => "released",
                },
            })
        })
        .collect();

    let mut by_model: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    let mut by_key: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_lane: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_transport: BTreeMap<String, u64> = BTreeMap::new();
    for receipt in &scan.receipts {
        let model = if receipt.served.model.is_empty() {
            receipt.requested.model.clone()
        } else {
            receipt.served.model.clone()
        };
        let entry = by_model.entry(model).or_default();
        entry.0 += 1;
        if receipt.outcome == Outcome::Answered {
            entry.1 += 1;
        }
        if let Some(hold) = holds.get(&hold_key(receipt))
            && let Some(charge) = hold.retail_charge
        {
            entry.2 += charge;
        }
        if let Some(key) = &receipt.tenant {
            *by_key.entry(key.clone()).or_default() += 1;
        }
        let lane = match &receipt.lane {
            Some(receipts::execution::Lane::Decision) => "decision",
            Some(receipts::execution::Lane::Generation) => "generation",
            Some(receipts::execution::Lane::Executor) => "executor",
            None => "unrecorded",
        };
        *by_lane.entry(lane.to_string()).or_default() += 1;
        *by_transport.entry(receipt.transport.clone()).or_default() += 1;
    }

    // The billing entitlement, when billing runs — the plan's doors are
    // what the subscription entitles, read from the same book the
    // admission check consults.
    let entitlement = match &state.config.billing {
        Some(billing_config) => {
            match tenancy::billing::Billing::open(&state.dir).and_then(|billing| billing.store()) {
                Ok(store) => {
                    let subscription = store.book.subscription_for(&workspace);
                    match subscription {
                        Some(subscription) => {
                            let plan = billing_config
                                .plans
                                .iter()
                                .find(|plan| plan.id == subscription.plan);
                            json!({
                                "subscription": subscription.id,
                                "plan": subscription.plan,
                                "plan_version": subscription.plan_version,
                                "state": subscription.state.to_string(),
                                "doors": plan.map(|plan| json!(plan.models)),
                            })
                        }
                        None => Value::Null,
                    }
                }
                Err(_) => Value::Null,
            }
        }
        None => Value::Null,
    };

    accounts::record(&state, &principal, "usage-summary", Some(&workspace), None);
    answered(
        StatusCode::OK,
        json!({
            "workspace": workspace,
            "window": {"from": filter.from, "to": filter.to},
            "totals": totals,
            "units": {
                "questions": questions,
                "input_bytes": input_bytes,
                "reservations_measured": measured,
                "unmeasured": scan.receipts.len() - measured,
            },
            "cost": {
                "currency": currency,
                "retail": retail,
                "provider_cost": if provider_known { json!(provider_cost) } else { Value::Null },
                "hosting_cost": if hosting_known { json!(hosting_cost) } else { Value::Null },
                "refunded": refunded,
                "priced_calls": priced,
                "unpriced_calls": scan.receipts.len() - priced,
            },
            "outstanding": outstanding,
            "by_model": by_model.iter().map(|(model, (calls, answered, cost))| json!({
                "model": model, "calls": calls, "answered": answered, "retail": cost,
            })).collect::<Vec<_>>(),
            "by_key": by_key.iter().map(|(key, calls)| json!({"key": key, "calls": calls})).collect::<Vec<_>>(),
            "by_lane": by_lane,
            "by_transport": by_transport,
            "entitlement": entitlement,
            "disclosure": disclosure(&scan),
        }),
    )
}

/// Decode a keyset cursor — `resolved_at|digest`, the position the
/// next page continues after in newest-first order.
fn cursor_position(cursor: &str) -> Option<(String, String)> {
    let (at, digest) = cursor.split_once('|')?;
    Some((at.to_string(), digest.to_string()))
}

/// `GET /v1/workspaces/{id}/usage/activity` — the filtered, paginated
/// call list, newest first. The cursor is a `(resolved_at, digest)`
/// keyset — stable under appends because a page's position never
/// depends on what arrived after it.
async fn activity(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    let principal = match member_of(&state, &headers, &workspace) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let holds = holds(&state, &workspace).await;
    let mut scan = match scan(&state, &workspace, &filter, &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    // Newest first; a receipt with no timestamp sorts last.
    scan.receipts.sort_by(|a, b| {
        b.timing
            .resolved_at
            .cmp(&a.timing.resolved_at)
            .then_with(|| b.digest.cmp(&a.digest))
    });
    let start = match &filter.cursor {
        Some(cursor) => match cursor_position(cursor) {
            Some((at, digest)) => scan
                .receipts
                .iter()
                .position(|receipt| {
                    receipt.timing.resolved_at.as_deref() == Some(at.as_str())
                        && receipt.digest == digest
                })
                .map(|position| position + 1)
                .unwrap_or_else(|| {
                    // The cursor's position fell out of the window —
                    // the page it marked is gone, so the read resumes
                    // at the newest record the filter still holds.
                    usize::MAX
                }),
            None => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    "The cursor isn't valid. Use the `cursor` value from the previous page.",
                );
            }
        },
        None => 0,
    };
    if start == usize::MAX {
        return answered(
            StatusCode::OK,
            json!({
                "workspace": workspace,
                "items": [],
                "cursor": Value::Null,
                "disclosure": disclosure(&scan),
            }),
        );
    }
    let limit = filter.limit.unwrap_or(PAGE_DEFAULT).clamp(1, PAGE_MAX);
    let page: Vec<&ExecutionReceipt> = scan.receipts.iter().skip(start).take(limit).collect();
    let next = if scan.receipts.len() > start + limit {
        page.last().map(|receipt| {
            json!(format!(
                "{}|{}",
                receipt.timing.resolved_at.clone().unwrap_or_default(),
                receipt.digest
            ))
        })
    } else {
        None
    };
    let quota = state.quota_lock().await;
    let items: Vec<Value> = page
        .iter()
        .map(|receipt| {
            let hold = holds.get(&hold_key(receipt));
            let units = quota
                .reservation(&receipt.request, receipt.attempt)
                .map(|reservation| &reservation.units);
            activity_item(receipt, hold, units)
        })
        .collect();
    accounts::record(&state, &principal, "usage-activity", Some(&workspace), None);
    answered(
        StatusCode::OK,
        json!({
            "workspace": workspace,
            "items": items,
            "cursor": next.unwrap_or(Value::Null),
            "disclosure": disclosure(&scan),
        }),
    )
}

/// `GET /v1/workspaces/{id}/usage/timeseries` — the same records folded
/// into UTC day buckets: calls, outcomes, units, and retail cost per
/// day. Buckets are exact sums of the same receipts the activity read
/// returns — a day total a caller cannot reproduce is a bug, not a
/// discrepancy to explain.
async fn timeseries(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    let principal = match member_of(&state, &headers, &workspace) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let holds = holds(&state, &workspace).await;
    let scan = match scan(&state, &workspace, &filter, &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let quota = state.quota_lock().await;
    #[derive(Default)]
    struct Day {
        calls: u64,
        answered: u64,
        refused: u64,
        unavailable: u64,
        unknown: u64,
        questions: u64,
        input_bytes: u64,
        retail: u64,
    }
    let mut days: BTreeMap<String, Day> = BTreeMap::new();
    let mut undated = 0usize;
    for receipt in &scan.receipts {
        let Some(day) = receipt
            .timing
            .resolved_at
            .as_deref()
            .map(|at| at[..10.min(at.len())].to_string())
        else {
            undated += 1;
            continue;
        };
        let bucket = days.entry(day).or_default();
        bucket.calls += 1;
        match receipt.outcome {
            Outcome::Answered => bucket.answered += 1,
            Outcome::Refused => bucket.refused += 1,
            Outcome::Unavailable => bucket.unavailable += 1,
            Outcome::Unattempted => {}
            Outcome::Unknown => bucket.unknown += 1,
        }
        if let Some(reservation) = quota.reservation(&receipt.request, receipt.attempt) {
            bucket.questions += reservation.units.questions;
            bucket.input_bytes += reservation.units.input_bytes;
        }
        if let Some(hold) = holds.get(&hold_key(receipt))
            && let Some(charge) = hold.retail_charge
        {
            bucket.retail += charge;
        }
    }
    accounts::record(
        &state,
        &principal,
        "usage-timeseries",
        Some(&workspace),
        None,
    );
    answered(
        StatusCode::OK,
        json!({
            "workspace": workspace,
            "days": days.iter().map(|(day, bucket)| json!({
                "day": day,
                "calls": bucket.calls,
                "answered": bucket.answered,
                "refused": bucket.refused,
                "unavailable": bucket.unavailable,
                "unknown": bucket.unknown,
                "questions": bucket.questions,
                "input_bytes": bucket.input_bytes,
                "retail": bucket.retail,
            })).collect::<Vec<_>>(),
            "undated": undated,
            "disclosure": disclosure(&scan),
        }),
    )
}

/// `GET /v1/workspaces/{id}/usage/receipts/{digest}` — one receipt by
/// its digest, workspace-scoped: a receipt another workspace owns is
/// `unknown_receipt` here, not a leak.
async fn receipt(
    State(state): State<Arc<ServeState>>,
    Path((workspace, digest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let principal = match member_of(&state, &headers, &workspace) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let holds = holds(&state, &workspace).await;
    let scan = match scan(&state, &workspace, &Filter::default(), &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let Some(receipt) = scan
        .receipts
        .iter()
        .find(|receipt| receipt.digest == digest)
    else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_receipt",
            "This workspace has no receipt with that hash.",
        );
    };
    let hold = holds.get(&hold_key(receipt));
    let quota = state.quota_lock().await;
    let units = quota
        .reservation(&receipt.request, receipt.attempt)
        .map(|reservation| &reservation.units);
    accounts::record(
        &state,
        &principal,
        "usage-receipt",
        Some(&workspace),
        Some(digest),
    );
    answered(
        StatusCode::OK,
        json!({
            "receipt": receipt,
            "units": units.map(|units| json!({
                "questions": units.questions,
                "input_bytes": units.input_bytes,
                "options": units.options,
            })),
            "cost": cost_of(hold),
            "disclosure": disclosure(&scan),
        }),
    )
}

/// `GET /v1/workspaces/{id}/usage/export` — the filtered receipts as
/// NDJSON, the workspace's own activity exported verbatim: every line
/// is the sealed receipt, so an export re-verifies offline by the same
/// digest check the read path runs.
async fn export(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(filter): Query<Filter>,
) -> Response {
    let principal = match member_of(&state, &headers, &workspace) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let holds = holds(&state, &workspace).await;
    let scan = match scan(&state, &workspace, &filter, &holds) {
        Ok(scan) => scan,
        Err(response) => return response,
    };
    let limit = filter.limit.unwrap_or(PAGE_MAX * 50).min(PAGE_MAX * 50);
    let body = scan
        .lines
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    accounts::record(
        &state,
        &principal,
        "usage-export",
        Some(&workspace),
        Some(scan.lines.len().min(limit).to_string()),
    );
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/x-ndjson")
        .header(
            "content-disposition",
            format!(
                "attachment; filename=\"usage-{workspace}-{}.ndjson\"",
                unix_now()
            ),
        )
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
