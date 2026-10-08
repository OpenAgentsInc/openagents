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
use super::super::{custody, refused, workspace_shell};
use super::{Delegation, Failure, Journal, PAGE, answer, context, fresh_request, target};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use compute_workbench::credits;
use compute_workbench::retail::{self as native, Client};
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

pub(super) fn href(delegation: &str, execution: &str) -> String {
    format!(
        "{PAGE}/{}/purchases/{}",
        escape(delegation),
        escape(execution)
    )
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
    let mut content = format!(
        "<h2>Purchases</h2><p>Funded executions for delegation <code>{}</code>, read from the retail service with its native principal. Each purchase keeps its own payer, quote, approval, work identity, meter, charges, artifacts, and cleanup record. Payment, completion, acceptance, and publication are separate.</p><p><a href=\"{PAGE}#retail-{}\">Back to Retail</a></p>",
        escape(&id),
        escape(&id)
    );
    let pending: Vec<&String> = journal
        .records
        .iter()
        .filter(|(_, r)| r.op == "confirm" && r.outcome.is_none())
        .map(|(request, _)| request)
        .collect();
    for request in pending {
        content.push_str(&format!(
            "<p class=\"retail-pending\">Approval request {} · Outcome unknown. <a href=\"{PAGE}#retail-{}\">Retry the same request</a> from the Retail page; it recovers the original funded execution and never confirms twice.</p>",
            escape(&request[..8]),
            escape(&id)
        ));
    }
    let executions = page["executions"].as_array().cloned().unwrap_or_default();
    if executions.is_empty() {
        content.push_str("<p>No funded purchases.</p>");
    } else {
        content.push_str("<ul class=\"retail-purchases\">");
    }
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
        content.push_str(&format!(
            "<li><a href=\"{}\">Purchase {}</a> · {}</li>",
            href(&id, execution),
            escape(execution),
            if here {
                "approved on this page"
            } else {
                "approved by another client of this principal"
            }
        ));
    }
    if !executions.is_empty() {
        content.push_str("</ul>");
    }
    if let Some(next) = page["next"].as_str().filter(|_| executions.len() >= 32) {
        content.push_str(&format!(
            "<p><a href=\"{PAGE}/{}/purchases?after={}\">Older purchases</a></p>",
            escape(&id),
            escape(next)
        ));
    }
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(&content),
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
    let mut content = render(&observed, &id);
    let terminal = observed.receipt.settlement.is_some();
    if can_cancel && !terminal {
        let request = fresh_request();
        let token = match context.csrf(&headers, "retail-cancel", &target(delegation, &request)) {
            Ok(value) => value,
            Err(response) => return response,
        };
        content.push_str(&format!(
            "<section class=\"cloud-card\"><h3>Request a stop</h3><form method=\"post\" action=\"{PAGE}/{}/cancel\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><input type=\"hidden\" name=\"execution\" value=\"{}\"><button type=\"submit\">Request stop</button></form><p class=\"dim\">A stop request is not a stopped meter. Executor acknowledgment, sandbox deletion, and settlement appear above when the service records them.</p></section>",
            escape(&id),
            super::super::ticket(&token),
            escape(&execution),
        ));
    }
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(&content),
        None,
    )
}

fn row(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!(
        "<dt>{}</dt><dd>{}</dd>",
        escape(label),
        escape(value)
    ));
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

fn render(observed: &Observed, id: &str) -> String {
    let execution = &observed.execution;
    let admission = &execution.admission;
    let receipt = &observed.receipt;
    let mut out = format!(
        "<h2>Purchase {}</h2><p><a href=\"{PAGE}/{}/purchases\">All purchases</a> · <a href=\"{PAGE}#retail-{}\">Retail</a></p>",
        escape(&execution.execution),
        escape(id),
        escape(id)
    );
    out.push_str("<section class=\"cloud-card retail-status\"><h3>Status</h3><dl>");
    row(&mut out, "Payment", &payment(observed));
    row(&mut out, "Completion", &completion(observed));
    row(
        &mut out,
        "Acceptance",
        &match verdict(observed) {
            Some(v) => format!(
                "Not recorded. Checks {v} is evidence about the candidate, not your acceptance."
            ),
            None => "Not recorded. No checks verdict yet.".into(),
        },
    );
    row(
        &mut out,
        "Publication",
        "Not published. This purchase never applies or publishes the patch; you apply it yourself.",
    );
    out.push_str("</dl></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Payer</h3><dl>");
    row(&mut out, "Native account", &admission.account);
    row(
        &mut out,
        "Principal",
        &text(&observed.retained.binding["payer"]["principal"]),
    );
    row(
        &mut out,
        "Grant generation",
        &admission.grant_generation.to_string(),
    );
    row(
        &mut out,
        "Current rights",
        &format!(
            "observe={}, spend={}, execute={}, disclose={}",
            observed.account.capabilities.observe,
            observed.account.capabilities.spend,
            observed.account.capabilities.execute,
            observed.account.capabilities.disclose
        ),
    );
    row(
        &mut out,
        "Model payer",
        "Your own OpenAI key; model expense is billed by OpenAI to you and is not part of this charge",
    );
    out.push_str("</dl></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Quote</h3><dl>");
    row(
        &mut out,
        "Price book",
        &format!("{} / {}", execution.quote.version, execution.quote.book),
    );
    row(
        &mut out,
        "Maximum",
        &format!(
            "{} sats for at most {} seconds",
            execution.quote.max_sats, execution.quote.max_seconds
        ),
    );
    row(
        &mut out,
        "Quote digest",
        &text(&observed.retained.binding["quote"]["digest"]),
    );
    out.push_str("</dl>");
    match observed.local.quote.as_ref().and_then(|q| q["lines"].as_str()) {
        Some(lines) => out.push_str(&format!(
            "<p>The exact review you confirmed:</p><pre class=\"retail-review\">{}</pre>",
            escape(lines)
        )),
        None => out.push_str("<p>This quote was reviewed by another client of this principal; its review text is not held here.</p>"),
    }
    out.push_str("</section>");

    out.push_str("<section class=\"cloud-card\"><h3>Approval and request</h3><dl>");
    match &observed.local.accepted {
        Some(accepted) => {
            row(
                &mut out,
                "Approval",
                &format!(
                    "Confirmed on this page, request {}",
                    observed.local.confirm_request.as_deref().unwrap_or("")
                ),
            );
            row(
                &mut out,
                "Review",
                observed.local.review.as_deref().unwrap_or("unknown"),
            );
            row(&mut out, "Offer", &text(&accepted["offer"]));
        }
        None => row(
            &mut out,
            "Approval",
            "Confirmed by another client of this principal",
        ),
    }
    row(
        &mut out,
        "Funded request",
        &text(
            observed
                .retained
                .binding
                .get("request")
                .unwrap_or(&Value::Null),
        ),
    );
    out.push_str("</dl></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Work</h3><dl>");
    row(&mut out, "Execution", &execution.execution);
    row(&mut out, "Admission", &admission.digest().to_string());
    row(
        &mut out,
        "Public source",
        &format!(
            "{} @ {}",
            admission.source.repository, admission.source.commit
        ),
    );
    row(
        &mut out,
        "Computer",
        &format!("{} / {}", admission.computer_class, admission.task_class),
    );
    row(
        &mut out,
        "Sandbox",
        &text(
            observed
                .retained
                .binding
                .get("resource")
                .unwrap_or(&Value::Null),
        ),
    );
    out.push_str("</dl></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Progress</h3>");
    match &observed.live {
        Live::Read(Some(status)) => out.push_str(&format!(
            "<p>Task owner status: {}</p>",
            escape(&text(&status["state"]))
        )),
        Live::Read(None) => out.push_str("<p>Task owner status: not yet reported.</p>"),
        Live::Unavailable => out.push_str("<p>Live progress is unavailable now: not yet dispatched, custody has ended, or the service did not answer. Retained events remain below; reload to read on.</p>"),
        Live::Bounded => out.push_str("<p>The retained progress log reached its bound. Read the retained artifacts.</p>"),
    }
    if observed.retained.events.is_empty() {
        out.push_str("<p class=\"dim\">No progress events retained.</p>");
    } else {
        out.push_str("<ol class=\"retail-progress\">");
        for event in &observed.retained.events {
            out.push_str(&format!(
                "<li value=\"{}\"><pre>{}</pre></li>",
                event.cursor,
                escape(&event.text)
            ));
        }
        out.push_str("</ol>");
    }
    out.push_str("</section>");

    out.push_str("<section class=\"cloud-card\"><h3>Meter and charges</h3><dl>");
    let usage = execution.snapshot.as_ref().and_then(|s| s.usage.as_ref());
    row(
        &mut out,
        "Metered seconds",
        &match usage {
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
        row(
            &mut out,
            "Hold",
            &format!(
                "{} · held {} · charge {}",
                hold.state,
                credits(hold.held_msat),
                hold.charge_msat
                    .map(credits)
                    .unwrap_or_else(|| "unknown".into())
            ),
        );
    }
    out.push_str("</dl><pre class=\"retail-receipt\">");
    out.push_str(&escape(&receipt.lines()));
    out.push_str("</pre></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Artifacts</h3>");
    match receipt.retention.as_ref() {
        Some(retention) => {
            if let Some(manifest) = &retention.manifest {
                out.push_str(&format!(
                    "<p>Source {} @ {} · engine {} · sandbox {}</p>",
                    escape(&manifest.source.repository),
                    escape(&manifest.source.commit),
                    escape(&manifest.engine),
                    escape(&manifest.resource)
                ));
                if !retention.complete {
                    out.push_str("<p>Artifact retention is incomplete; incomplete artifacts are not offered.</p>");
                } else if retention.expired {
                    out.push_str("<p>Retained artifacts have expired.</p>");
                } else {
                    out.push_str(&format!(
                        "<p class=\"dim\">Retained until {}.</p>",
                        retention.expires_at
                    ));
                }
                out.push_str("<ul class=\"retail-artifacts\">");
                for artifact in &manifest.artifacts {
                    let kind = text(&serde_json::to_value(&artifact.kind).unwrap_or_default());
                    if retention.complete && !retention.expired {
                        out.push_str(&format!(
                            "<li><a href=\"{}/artifacts/{}\">{}</a> · {} · {} bytes · SHA-256 {}</li>",
                            href(id, &execution.execution),
                            escape(&artifact.name),
                            escape(&artifact.name),
                            escape(&kind),
                            artifact.size,
                            escape(&artifact.digest)
                        ));
                    } else {
                        out.push_str(&format!(
                            "<li>{} · {} · {} bytes</li>",
                            escape(&artifact.name),
                            escape(&kind),
                            artifact.size
                        ));
                    }
                }
                out.push_str("</ul>");
            } else {
                out.push_str("<p>No artifact manifest was retained.</p>");
            }
        }
        None => out.push_str("<p>Artifacts are retained when the sandbox is cleaned up.</p>"),
    }
    out.push_str("</section>");

    out.push_str("<section class=\"cloud-card\"><h3>Cancellation</h3><dl>");
    match &receipt.cancellation {
        Some(cancel) => {
            row(
                &mut out,
                "Stop",
                &format!(
                    "{} at {}",
                    text(&serde_json::to_value(cancel.reason).unwrap_or_default()),
                    cancel.requested_at
                ),
            );
            row(
                &mut out,
                "Stop sent",
                if cancel.stop_sent { "yes" } else { "not yet" },
            );
            row(
                &mut out,
                "Executor acknowledged",
                if cancel.executor.is_some() {
                    "yes"
                } else {
                    "not yet; the meter may still run"
                },
            );
            row(
                &mut out,
                "Provider deleted",
                if cancel.provider_deleted {
                    "yes"
                } else {
                    "not yet"
                },
            );
            row(
                &mut out,
                "Remaining hold",
                &credits(cancel.remaining_hold_msat),
            );
        }
        None => row(&mut out, "Stop", "No stop requested"),
    }
    out.push_str("</dl></section>");

    out.push_str("<section class=\"cloud-card\"><h3>Cleanup</h3>");
    match &receipt.retention {
        Some(retention) => {
            out.push_str(&format!(
                "<p>{}</p><ul>",
                if retention.deleted() {
                    "Cleanup confirmed: every discovered sandbox resource acknowledged deletion."
                } else if !retention.discovery_complete {
                    "Cleanup unconfirmed: resource discovery is incomplete. The service keeps retrying."
                } else {
                    "Cleanup unconfirmed: deletion is not yet acknowledged for every resource. The service keeps retrying."
                }
            ));
            for resource in &retention.resources {
                out.push_str(&format!(
                    "<li>{} · intent at {} · {} · attempts {}</li>",
                    escape(&resource.resource),
                    resource.intent_at,
                    resource
                        .acknowledged_at
                        .map(|t| format!("deletion acknowledged at {t}"))
                        .unwrap_or_else(|| "deletion unacknowledged".into()),
                    resource.attempts
                ));
            }
            out.push_str("</ul>");
        }
        None => out.push_str("<p>No cleanup recorded yet.</p>"),
    }
    out.push_str("</section>");
    out
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
    let content = format!(
        "<h2>Artifact {}</h2><p><a href=\"{}\">Purchase {}</a></p><dl><dt>Source</dt><dd>{} @ {}</dd><dt>Engine</dt><dd>{}</dd><dt>Sandbox</dt><dd>{}</dd><dt>SHA-256</dt><dd>{} (matches the retained manifest)</dd><dt>Size</dt><dd>{} bytes</dd></dl><p class=\"dim\">This is a retained candidate. Reading it is not acceptance, and nothing here applies or publishes it.</p><pre class=\"retail-artifact\">{}</pre>",
        escape(&declared.name),
        href(&id, &execution),
        escape(&execution),
        escape(&manifest.source.repository),
        escape(&manifest.source.commit),
        escape(&manifest.engine),
        escape(&manifest.resource),
        escape(&declared.digest),
        declared.size,
        escape(&text),
    );
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(&content),
        None,
    )
}
