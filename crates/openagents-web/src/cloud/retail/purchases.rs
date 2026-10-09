//! Canonical retail purchases: progress, artifacts, cancellation, receipt,
//! and recovery over the same scoped delegation as funding and confirmation.
//!
//! Every value shown comes from the native retail owner through the
//! delegation's server-side principal, joined with this site's own request
//! journal. The first observation of each immutable part of a purchase
//! (payer, quote, approval, request, work identity, resource, settled
//! charges, artifact source, and acknowledged cleanup) is retained in a
//! private record; a later answer that differs is refused rather than shown
//! as current. Progress events are retained with their source cursors, so a
//! reload or a site restart shows the same purchase and reads on from the
//! same place. Payment, completion, acceptance, and publication are reported
//! separately: none implies another.

use super::super::session::SessionError;
use super::super::ui::{self, Details};
use super::super::{custody, refused, workspace_shell};
use super::{Delegation, Failure, Journal, PAGE, answer, context, fresh_request, target};
use crate::App;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use compute_workbench::credits;
use compute_workbench::retail::{self as native, Client};
use maud::{Markup, html};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path as FsPath;

const SCHEMA: &str = "openagents.cloud.retail-web-purchase.v1";
const EVENTS_MAX: usize = 1024;
const EVENT_BYTES_MAX: usize = 512 * 1024;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/billing/retail/{id}/purchases", get(list))
        .route(
            "/cloud/app/billing/retail/{id}/purchases/{execution}",
            get(detail),
        )
        .route(
            "/cloud/app/billing/retail/{id}/purchases/{execution}/artifacts/{name}",
            get(artifact),
        )
}

pub(crate) fn href(delegation: &str, execution: &str) -> String {
    format!("{PAGE}/{delegation}/purchases/{execution}")
}

// ---- Retained record --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    cursor: u64,
    text: String,
}

/// The site's private record of one purchase: first-observed immutable
/// bindings and the progress events read so far.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    schema: String,
    delegation: String,
    execution: String,
    binding: BTreeMap<String, Value>,
    events: Vec<Event>,
    next: u64,
    truncated: bool,
}

impl Retained {
    fn name(delegation: &Delegation, execution: &str) -> String {
        format!(
            "{}-{}.purchase.json",
            &delegation.identity["sha256:".len()..],
            &super::hex(&Sha256::digest(execution.as_bytes()))[..32]
        )
    }

    fn load(root: &FsPath, delegation: &Delegation, execution: &str) -> Result<Self, Failure> {
        custody::checked_root(root)?;
        let path = root.join(Self::name(delegation, execution));
        let Some(bytes) = custody::read_private(&path, 2 * 1024 * 1024)? else {
            return Ok(Self {
                schema: SCHEMA.into(),
                delegation: delegation.identity.clone(),
                execution: execution.into(),
                binding: BTreeMap::new(),
                events: Vec::new(),
                next: 0,
                truncated: false,
            });
        };
        let record: Self = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::Session(SessionError::Unavailable))?;
        if record.schema != SCHEMA
            || record.delegation != delegation.identity
            || record.execution != execution
        {
            return Err(Failure::Session(SessionError::Unavailable));
        }
        Ok(record)
    }

    fn save(&self, root: &FsPath, delegation: &Delegation) -> Result<(), Failure> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| Failure::Session(SessionError::Unavailable))?;
        custody::write_private(root, &Self::name(delegation, &self.execution), &bytes)?;
        Ok(())
    }

    /// Retain a first observation; a different later one is refused.
    fn bind(&mut self, field: &'static str, value: Value) -> Result<(), Failure> {
        match self.binding.get(field) {
            Some(old) if *old != value => Err(Failure::Changed(field)),
            Some(_) => Ok(()),
            None => {
                self.binding.insert(field.into(), value);
                Ok(())
            }
        }
    }

    fn append(&mut self, page: &retail_cloud::dispatch::Page) {
        let mut bytes: usize = self.events.iter().map(|e| e.text.len()).sum();
        for event in &page.events {
            if event.cursor <= self.next {
                continue;
            }
            if self.events.len() >= EVENTS_MAX || bytes + event.text.len() > EVENT_BYTES_MAX {
                self.truncated = true;
                return;
            }
            bytes += event.text.len();
            self.next = event.cursor;
            self.events.push(Event {
                cursor: event.cursor,
                text: event.text.clone(),
            });
        }
    }
}

/// The retained first-observed bindings of one purchase, read without
/// observing the native owner again or creating a record.
pub(super) fn retained(
    delegation: &Delegation,
    root: &FsPath,
    execution: &str,
) -> Result<BTreeMap<String, Value>, Failure> {
    Ok(Retained::load(root, delegation, execution)?.binding)
}

// ---- Joining the journal and the native owner ------------------------------

/// What this page's own journal says about a purchase.
#[derive(Default)]
struct Local {
    confirm_request: Option<String>,
    review: Option<String>,
    quote_request: Option<String>,
    quote: Option<Value>,
    accepted: Option<Value>,
}

fn local(journal: &Journal, execution: &str) -> Local {
    let mut found = Local::default();
    for (request, record) in &journal.records {
        if record.op == "confirm"
            && record
                .outcome
                .as_ref()
                .is_some_and(|o| o["execution"] == json!(execution))
        {
            found.confirm_request = Some(request.clone());
            found.review = record.params["review"].as_str().map(String::from);
            found.accepted = record.outcome.clone();
        }
    }
    if let Some(review) = &found.review {
        for (request, record) in &journal.records {
            if record.op == "quote"
                && record
                    .outcome
                    .as_ref()
                    .is_some_and(|o| o["digest"] == json!(review))
            {
                found.quote_request = Some(request.clone());
                found.quote = record.outcome.clone();
            }
        }
    }
    found
}

/// Live progress is read separately from the canonical record.
enum Live {
    Read(Option<Value>),
    Unavailable,
    Bounded,
}

struct Observed {
    account: native::Account,
    execution: native::Execution,
    receipt: native::Receipt,
    live: Live,
    retained: Retained,
    local: Local,
}

fn observe(
    delegation: &Delegation,
    client: &mut Client,
    journals: &FsPath,
    execution: &str,
) -> Result<Observed, Failure> {
    let account = client.account()?;
    let record = client.execution(execution)?;
    let receipt = client.receipt(execution)?;
    let journal = Journal::load(journals, delegation)?;
    let local = local(&journal, execution);
    let mut retained = Retained::load(journals, delegation, execution)?;
    let admission = &record.admission;
    if admission.account != account.account || admission.execution != execution {
        return Err(Failure::Changed("payer"));
    }
    retained.bind(
        "payer",
        json!({"account":admission.account,"principal":delegation.client.principal,
            "endpoint":delegation.client.endpoint,"grant_generation":admission.grant_generation,
            "commercial":record.commercial}),
    )?;
    retained.bind(
        "work",
        json!({"execution":execution,"admission":admission.digest(),"source":admission.source,
            "task":admission.request,"computer":admission.computer_class,"class":admission.task_class}),
    )?;
    retained.bind(
        "quote",
        json!({"digest":super::digest(&serde_json::to_value(&record.quote).map_err(|_| Failure::Unknown)?),
            "book":record.quote.book,"version":record.quote.version,"max_sats":record.quote.max_sats,
            "max_seconds":record.quote.max_seconds,"max_charge_sats":admission.max_charge_sats}),
    )?;
    if let Some(accepted) = &local.accepted {
        if accepted["admission"] != json!(admission.digest()) {
            return Err(Failure::Changed("approval"));
        }
        if local
            .quote
            .as_ref()
            .is_some_and(|q| q["execution"] != json!(execution))
        {
            return Err(Failure::Changed("quote"));
        }
        retained.bind(
            "approval",
            json!({"confirm_request":local.confirm_request,"review":local.review,
                "quote_request":local.quote_request,"offer":accepted["offer"]}),
        )?;
        retained.bind("request", accepted["request"].clone())?;
    }
    let snapshot = record.snapshot.as_ref();
    if let Some(request) = snapshot.map(|s| s.request.clone()) {
        retained.bind("request", json!(request))?;
    }
    let settlement = receipt.settlement.as_ref();
    if let Some(settlement) = settlement {
        retained.bind("request", json!(settlement.request))?;
    }
    // The admitted sandbox, wherever the owner names it; never a replacement.
    if let Some(resource) = snapshot
        .and_then(|s| serde_json::to_value(&s.state).ok())
        .and_then(|state| state["resource"].as_str().map(String::from))
    {
        retained.bind("resource", json!(resource))?;
    }
    if let Some(usage) = snapshot.and_then(|s| s.usage.as_ref()) {
        retained.bind("resource", json!(usage.resource))?;
    }
    if let Some(retention) = &receipt.retention {
        if let Some(manifest) = &retention.manifest {
            retained.bind("resource", json!(manifest.resource))?;
            if manifest.execution != execution || manifest.source != admission.source {
                return Err(Failure::Changed("artifact source"));
            }
            if retention.complete {
                retained.bind(
                    "artifacts",
                    json!({"source":manifest.source,"engine":manifest.engine,
                        "task":manifest.task,"artifacts":manifest.artifacts}),
                )?;
            }
        }
        if retention.deleted() {
            retained.bind("cleanup", json!({"deleted":true}))?;
        }
    } else if retained.binding.contains_key("cleanup") {
        return Err(Failure::Changed("cleanup"));
    }
    if let Some(settlement) = settlement.filter(|s| s.charge_msat.is_some()) {
        retained.bind(
            "settlement",
            json!({"ending":settlement.ending,"charge_msat":settlement.charge_msat,
                "released_msat":settlement.released_msat,"settled_at":settlement.settled_at,
                "usage":settlement.usage_digest,"metered":settlement.resource}),
        )?;
    } else if retained.binding.contains_key("settlement") {
        return Err(Failure::Changed("settlement"));
    }
    let live = if retained.truncated {
        Live::Bounded
    } else {
        match client.progress_after(execution, retained.next) {
            Ok(page) => {
                retained.append(&page);
                Live::Read(
                    page.status
                        .map(|s| serde_json::to_value(s).unwrap_or_default()),
                )
            }
            Err(native::Error::Refused(message)) => return Err(Failure::Refused(message)),
            Err(_) => Live::Unavailable,
        }
    };
    retained.save(journals, delegation)?;
    Ok(Observed {
        account,
        execution: record,
        receipt,
        live,
        retained,
        local,
    })
}

// ---- Pages ------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct After {
    #[serde(default)]
    after: Option<String>,
}

async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<After>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if query.after.as_deref().is_some_and(|a| !super::valid_id(a)) {
        return refused(SessionError::InvalidRequest);
    }
    let after = query.after.clone();
    let listed = context
        .retail
        .native(
            &context.viewer,
            &id,
            move |delegation, client, journals, _| {
                let page = client.executions(after.as_deref())?;
                let journal = Journal::load(journals, delegation)?;
                Ok((page, journal))
            },
        )
        .await;
    let (page, journal) = match listed {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let retail = format!("{PAGE}#retail-{id}");
    let pending: Vec<&String> = journal
        .records
        .iter()
        .filter(|(_, r)| r.op == "confirm" && r.outcome.is_none())
        .map(|(request, _)| request)
        .collect();
    let executions = page["executions"].as_array().cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(executions.len());
    for entry in &executions {
        let Some(execution) = entry["execution"].as_str() else {
            continue;
        };
        let here = journal.records.values().any(|r| {
            r.op == "confirm"
                && r.outcome
                    .as_ref()
                    .is_some_and(|o| o["execution"] == json!(execution))
        });
        rows.push((execution.to_owned(), here));
    }
    let next = page["next"].as_str().filter(|_| executions.len() >= 32);
    let content = html! {
        h2 { "Purchases" }
        p {
            "Funded executions for delegation " code { (id) }
            ", read from the retail service with its native principal. Each purchase keeps its own payer, quote, approval, work identity, meter, charges, artifacts, and cleanup record. Payment, completion, acceptance, and publication are separate."
        }
        p { a href=(retail) { "Back to Retail" } }
        @for request in &pending {
            div class="retail-pending" {
                (ui::outcome_unknown(
                    html! {
                        "Approval request " (&request[..8]) " \u{b7} Outcome unknown. "
                        a href=(retail) { "Retry the same request" }
                        " from the Retail page; it recovers the original funded execution and never confirms twice."
                    },
                    None,
                ))
            }
        }
        @if executions.is_empty() {
            p { "No funded purchases." }
        } @else {
            ul class="retail-purchases" {
                @for (execution, here) in &rows {
                    li {
                        a href=(href(&id, execution)) { "Purchase " (execution) }
                        " \u{b7} "
                        @if *here { "approved on this page" } @else { "approved by another client of this principal" }
                    }
                }
            }
        }
        @if let Some(next) = next {
            p { a href=(format!("{PAGE}/{id}/purchases?after={next}")) { "Older purchases" } }
        }
    };
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(content),
        None,
    )
}

async fn detail(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, execution)): Path<(String, String)>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !super::valid_id(&execution) {
        return refused(SessionError::InvalidRequest);
    }
    let wanted = execution.clone();
    let observed = context
        .retail
        .native(
            &context.viewer,
            &id,
            move |delegation, client, journals, _| {
                let rights = client.account()?.capabilities;
                Ok((observe(delegation, client, journals, &wanted)?, rights))
            },
        )
        .await;
    let (observed, rights) = match observed {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let delegation = match context.retail.get(&context.viewer, &id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let can_cancel = !delegation.read_only() && rights.observe && rights.execute;
    let terminal = observed.receipt.settlement.is_some();
    let stop = if can_cancel && !terminal {
        let request = fresh_request();
        let token = match context.csrf(&headers, "retail-cancel", &target(delegation, &request)) {
            Ok(value) => value,
            Err(response) => return response,
        };
        Some(ui::card(html! {
            h3 { "Request a stop" }
            (ui::BoundForm::new(format!("{PAGE}/{id}/cancel"))
                .csrf(&token)
                .bind("request", &request)
                .bind("execution", &execution)
                .body(html! {
                    p { "A stop request is not a stopped meter. Executor acknowledgment, sandbox deletion, and settlement appear above when the service records them." }
                })
                .submit_with(ui::submit("Request stop", false)))
        }))
    } else {
        None
    };
    let content = html! {
        (render(&observed, &id))
        @if let Some(stop) = stop { (stop) }
    };
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(content),
        None,
    )
}

fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "unknown".into(),
        other => other.to_string(),
    }
}

fn payment(observed: &Observed) -> String {
    let settlement = observed.receipt.settlement.as_ref();
    if let Some(charge) = settlement.and_then(|s| s.charge_msat) {
        let s = settlement.unwrap();
        return format!(
            "Charged {} from purchased credits; unused hold released {} (not a payment refund)",
            credits(charge),
            credits(s.released_msat)
        );
    }
    match &observed.execution.hold {
        Some(hold) if hold.held_msat > 0 => format!(
            "Quote maximum reserved: {} held; usage unknown until metered, so funds remain held",
            credits(hold.held_msat)
        ),
        Some(hold) => format!("Hold {}; charge unknown", hold.state),
        None => "Unknown; no hold observed".into(),
    }
}

fn completion(observed: &Observed) -> String {
    if let Some(settlement) = &observed.receipt.settlement {
        let ending = text(&serde_json::to_value(settlement.ending).unwrap_or_default());
        return match ending.as_str() {
            "executor_ended" => "Ended: the executor finished (completed, failed, limited, or timed out)".into(),
            "cancelled" => "Stopped after the executor started".into(),
            "provider_lost_after_executor" => "Provider lost the sandbox after the executor started; a replacement needs a new offer".into(),
            "provider_lost_before_executor" => "Provider lost the sandbox before the executor started".into(),
            "provider_unavailable" => "The sandbox never became reachable".into(),
            "not_started" => "Not started".into(),
            _ => "Ending unknown; the service keeps reconciling".into(),
        };
    }
    let Some(snapshot) = &observed.execution.snapshot else {
        return "Not yet observed".into();
    };
    let state = serde_json::to_value(&snapshot.state).unwrap_or_default();
    match state["state"].as_str().unwrap_or("") {
        "awaiting_reservation" | "awaiting_provision" | "provision_observed"
        | "awaiting_material" => "Starting: the sandbox is being prepared".into(),
        "awaiting_dispatch" => "Starting: dispatch pending; a lost dispatch reply is reconciled, never repeated".into(),
        "task_observed" => format!("Task {}", text(&state["status"]["state"])),
        "unknown" => "Dispatch or provider outcome unknown; the service reconciles the same sandbox and never redispatches".into(),
        "new_offer_required" => "Provider lost the sandbox; a replacement needs a new offer and inherits no funds or authority".into(),
        "cleanup" => "Cleaning up".into(),
        "refused" => format!("Refused: {}", text(&state["reason"])),
        other => format!("State {other}"),
    }
}

fn verdict(observed: &Observed) -> Option<String> {
    observed
        .receipt
        .settlement
        .as_ref()
        .and_then(|s| s.checks)
        .or_else(|| observed.execution.snapshot.as_ref().and_then(|s| s.checks))
        .map(|v| text(&serde_json::to_value(v).unwrap_or_default()))
}

fn render(observed: &Observed, id: &str) -> Markup {
    let execution = &observed.execution;
    let admission = &execution.admission;
    let receipt = &observed.receipt;

    let status = Details::new()
        .row("Payment", payment(observed))
        .row("Completion", completion(observed))
        .row(
            "Acceptance",
            match verdict(observed) {
                Some(v) => format!(
                    "Not recorded. Checks {v} is evidence about the candidate, not your acceptance."
                ),
                None => "Not recorded. No checks verdict yet.".into(),
            },
        )
        .row(
            "Publication",
            "Not published. This purchase never applies or publishes the patch; you apply it yourself.",
        );

    let payer = Details::new()
        .row("Native account", &admission.account)
        .row(
            "Principal",
            text(&observed.retained.binding["payer"]["principal"]),
        )
        .row("Grant generation", admission.grant_generation.to_string())
        .row(
            "Current rights",
            format!(
                "observe={}, spend={}, execute={}, disclose={}",
                observed.account.capabilities.observe,
                observed.account.capabilities.spend,
                observed.account.capabilities.execute,
                observed.account.capabilities.disclose
            ),
        )
        .row(
            "Model payer",
            "Your own OpenAI key; model expense is billed by OpenAI to you and is not part of this charge",
        );

    let quote = Details::new()
        .row(
            "Price book",
            format!("{} / {}", execution.quote.version, execution.quote.book),
        )
        .row(
            "Maximum",
            format!(
                "{} sats for at most {} seconds",
                execution.quote.max_sats, execution.quote.max_seconds
            ),
        )
        .row(
            "Quote digest",
            text(&observed.retained.binding["quote"]["digest"]),
        );

    let mut approval = match &observed.local.accepted {
        Some(accepted) => Details::new()
            .row(
                "Approval",
                format!(
                    "Confirmed on this page, request {}",
                    observed.local.confirm_request.as_deref().unwrap_or("")
                ),
            )
            .row(
                "Review",
                observed.local.review.as_deref().unwrap_or("unknown"),
            )
            .row("Offer", text(&accepted["offer"])),
        None => Details::new().row("Approval", "Confirmed by another client of this principal"),
    };
    approval = approval.row(
        "Funded request",
        text(
            observed
                .retained
                .binding
                .get("request")
                .unwrap_or(&Value::Null),
        ),
    );

    let work = Details::new()
        .row("Execution", &execution.execution)
        .row("Admission", admission.digest().to_string())
        .row(
            "Public source",
            format!(
                "{} @ {}",
                admission.source.repository, admission.source.commit
            ),
        )
        .row(
            "Computer",
            format!("{} / {}", admission.computer_class, admission.task_class),
        )
        .row(
            "Sandbox",
            text(
                observed
                    .retained
                    .binding
                    .get("resource")
                    .unwrap_or(&Value::Null),
            ),
        );

    let usage = execution.snapshot.as_ref().and_then(|s| s.usage.as_ref());
    let mut meter = Details::new().row(
        "Metered seconds",
        match usage {
            Some(u) => format!(
                "{}{} ({} observations, {})",
                u.seconds
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                if u.final_usage {
                    ", final"
                } else {
                    ", not final"
                },
                u.observations,
                u.observations_digest
            ),
            None => "unknown".into(),
        },
    );
    if let Some(hold) = &execution.hold {
        meter = meter.row(
            "Hold",
            format!(
                "{} · held {} · charge {}",
                hold.state,
                credits(hold.held_msat),
                hold.charge_msat
                    .map(credits)
                    .unwrap_or_else(|| "unknown".into())
            ),
        );
    }

    let cancellation = match &receipt.cancellation {
        Some(cancel) => Details::new()
            .row(
                "Stop",
                format!(
                    "{} at {}",
                    text(&serde_json::to_value(cancel.reason).unwrap_or_default()),
                    cancel.requested_at
                ),
            )
            .row(
                "Stop sent",
                if cancel.stop_sent { "yes" } else { "not yet" },
            )
            .row(
                "Executor acknowledged",
                if cancel.executor.is_some() {
                    "yes"
                } else {
                    "not yet; the meter may still run"
                },
            )
            .row(
                "Provider deleted",
                if cancel.provider_deleted {
                    "yes"
                } else {
                    "not yet"
                },
            )
            .row("Remaining hold", credits(cancel.remaining_hold_msat)),
        None => Details::new().row("Stop", "No stop requested"),
    };

    let purchase = href(id, &execution.execution);
    let all = format!("{PAGE}/{id}/purchases");
    let retail = format!("{PAGE}#retail-{id}");
    html! {
        h2 { "Purchase " (execution.execution) }
        (ui::links([(all.as_str(), "All purchases"), (retail.as_str(), "Retail")]))
        section class="cloud-card retail-status" {
            h3 { "Status" }
            (status)
        }
        (ui::card(html! { h3 { "Payer" } (payer) }))
        (ui::card(html! {
            h3 { "Quote" }
            (quote)
            @match observed.local.quote.as_ref().and_then(|q| q["lines"].as_str()) {
                Some(lines) => {
                    p { "The exact review you confirmed:" }
                    pre class="retail-review" { (lines) }
                }
                None => {
                    p { "This quote was reviewed by another client of this principal; its review text is not held here." }
                }
            }
        }))
        (ui::card(html! { h3 { "Approval and request" } (approval) }))
        (ui::card(html! { h3 { "Work" } (work) }))
        (ui::card(html! {
            h3 { "Progress" }
            @match &observed.live {
                Live::Read(Some(status)) => { p { "Task owner status: " (text(&status["state"])) } }
                Live::Read(None) => { p { "Task owner status: not yet reported." } }
                Live::Unavailable => { p { "Live progress is unavailable now: not yet dispatched, custody has ended, or the service did not answer. Retained events remain below; reload to read on." } }
                Live::Bounded => { p { "The retained progress log reached its bound. Read the retained artifacts." } }
            }
            @if observed.retained.events.is_empty() {
                p { "No progress events retained." }
            } @else {
                ol class="retail-progress" {
                    @for event in &observed.retained.events {
                        li value=(event.cursor) { pre { (event.text) } }
                    }
                }
            }
        }))
        (ui::card(html! {
            h3 { "Meter and charges" }
            (meter)
            pre class="retail-receipt" { (receipt.lines()) }
        }))
        (ui::card(html! {
            h3 { "Artifacts" }
            @match receipt.retention.as_ref() {
                Some(retention) => {
                    @if let Some(manifest) = &retention.manifest {
                        p {
                            "Source " (manifest.source.repository) " @ " (manifest.source.commit)
                            " \u{b7} engine " (manifest.engine) " \u{b7} sandbox " (manifest.resource)
                        }
                        @if !retention.complete {
                            p { "Artifact retention is incomplete; incomplete artifacts are not offered." }
                        } @else if retention.expired {
                            p { "Retained artifacts have expired." }
                        } @else {
                            p { "Retained until " (retention.expires_at) "." }
                        }
                        ul class="retail-artifacts" {
                            @for artifact in &manifest.artifacts {
                                @let kind = text(&serde_json::to_value(&artifact.kind).unwrap_or_default());
                                @if retention.complete && !retention.expired {
                                    li {
                                        a href=(format!("{purchase}/artifacts/{}", artifact.name)) { (artifact.name) }
                                        " \u{b7} " (kind) " \u{b7} " (artifact.size) " bytes \u{b7} SHA-256 " (artifact.digest)
                                    }
                                } @else {
                                    li { (artifact.name) " \u{b7} " (kind) " \u{b7} " (artifact.size) " bytes" }
                                }
                            }
                        }
                    } @else {
                        p { "No artifact manifest was retained." }
                    }
                }
                None => { p { "Artifacts are retained when the sandbox is cleaned up." } }
            }
        }))
        (ui::card(html! { h3 { "Cancellation" } (cancellation) }))
        (ui::card(html! {
            h3 { "Cleanup" }
            @match &receipt.retention {
                Some(retention) => {
                    p {
                        @if retention.deleted() {
                            "Cleanup confirmed: every discovered sandbox resource acknowledged deletion."
                        } @else if !retention.discovery_complete {
                            "Cleanup unconfirmed: resource discovery is incomplete. The service keeps retrying."
                        } @else {
                            "Cleanup unconfirmed: deletion is not yet acknowledged for every resource. The service keeps retrying."
                        }
                    }
                    ul {
                        @for resource in &retention.resources {
                            li {
                                (resource.resource) " \u{b7} intent at " (resource.intent_at) " \u{b7} "
                                @match &resource.acknowledged_at {
                                    Some(t) => { "deletion acknowledged at " (t) }
                                    None => { "deletion unacknowledged" }
                                }
                                " \u{b7} attempts " (resource.attempts)
                            }
                        }
                    }
                }
                None => { p { "No cleanup recorded yet." } }
            }
        }))
    }
}

async fn artifact(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, execution, name)): Path<(String, String, String)>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !super::valid_id(&execution)
        || name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return refused(SessionError::InvalidRequest);
    }
    let (wanted, logical) = (execution.clone(), name.clone());
    let read = context
        .retail
        .native(
            &context.viewer,
            &id,
            move |delegation, client, journals, _| {
                let observed = observe(delegation, client, journals, &wanted)?;
                let retention = observed
                    .receipt
                    .retention
                    .as_ref()
                    .filter(|r| r.complete && !r.expired)
                    .ok_or(Failure::Refused(
                        "Artifacts are absent, incomplete, or expired.",
                    ))?;
                let manifest = retention.manifest.clone().ok_or(Failure::Refused(
                    "Artifacts are absent, incomplete, or expired.",
                ))?;
                let declared = manifest
                    .artifacts
                    .iter()
                    .find(|a| a.name == logical)
                    .cloned()
                    .ok_or(Failure::Refused("No such retained artifact."))?;
                let value = client.artifact(&wanted, &logical)?;
                let text = value["text"].as_str().ok_or(Failure::Unknown)?.to_owned();
                if value["execution"] != json!(wanted)
                    || value["name"] != json!(logical)
                    || retail_cloud::sha256_hex(text.as_bytes()) != declared.digest
                {
                    return Err(Failure::Changed("artifact"));
                }
                Ok((manifest, declared, text))
            },
        )
        .await;
    let (manifest, declared, text) = match read {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let content = html! {
        h2 { "Artifact " (declared.name) }
        p { a href=(href(&id, &execution)) { "Purchase " (execution) } }
        (Details::new()
            .row(
                "Source",
                format!("{} @ {}", manifest.source.repository, manifest.source.commit),
            )
            .row("Engine", &manifest.engine)
            .row("Sandbox", &manifest.resource)
            .row(
                "SHA-256",
                format!("{} (matches the retained manifest)", declared.digest),
            )
            .row("Size", format!("{} bytes", declared.size)))
        p { "This is a retained candidate. Reading it is not acceptance, and nothing here applies or publishes it." }
        pre class="retail-artifact" { (text) }
    };
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(content),
        None,
    )
}
