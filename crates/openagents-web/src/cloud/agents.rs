//! Alice, crew, and Agent Studio observation with exact native decisions.
//!
//! The resident host owns agents, Studio coordination, reviews, and merges.
//! This adapter reads them through the explicit binding, binds every control
//! to what the page displayed (the Studio stream and sequence, a decision's
//! basis, a review's three revisions), and stages each effect as one exact
//! retained native packet. A gap in the Studio stream refuses every control
//! until the page reads a fresh snapshot.

use super::controls::{Context, admitted, hidden, staged, submit};
use super::session::{SessionError, now};
use super::{failure, protect, refused, ticket, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use coder_access::Right;
use coder_access::agent::{self as wire, AgentView, Mode};
use coder_access::protocol::{Operation, Outcome, random_id};
use coder_access::review::{Completeness, TaskReview};
use coder_access::studio::{Kind, MergeDecision, Snapshot, Update, Verdict};
use serde::Deserialize;

/// The host's current request limits for one agent, until the placement
/// and parallel-queue work (#10929–#10931) changes them and qualifies.
const LIMITS: &str = "Current host limit: one running request and four waiting, on her home computer. The host does not report its waiting count to the browser. Placement on other computers and a parallel queue (#10929–#10931) are not available here.";
const METER: &str = "Her budget meter covers terminal-request planning, reporting, and Coder calls. Coding-task runs bypass that meter and use auto-start and provider capacity, so their spend is unmetered here and unknown. The meter is not a coding-task spending ceiling.";
const CONFIGURATION: &str = "Configuration: unavailable. Owner-key and file settings have no reviewed host operation, so the browser offers none.";
const STALE: &str = "The Studio stream changed or has a gap since this page read it. Controls stay disabled until you reopen the Studio for a fresh snapshot; cached decisions and reviews remain stale until then.";
const ISSUED_WINDOW: u64 = 60 * 60;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/agents", get(index))
        .route("/cloud/app/hosts/{binding}/agents", get(overview))
        .route("/cloud/app/hosts/{binding}/agents/{agent}", get(agent_page))
        .route("/cloud/app/hosts/{binding}/agents/{agent}/ask", post(ask))
        .route(
            "/cloud/app/hosts/{binding}/agents/{agent}/proposal",
            post(proposal),
        )
        .route("/cloud/app/hosts/{binding}/studio/goals", post(goal))
        .route("/cloud/app/hosts/{binding}/studio/seats", post(seat))
        .route(
            "/cloud/app/hosts/{binding}/studio/decisions",
            post(decision),
        )
        .route(
            "/cloud/app/hosts/{binding}/studio/tasks/{task}/review",
            get(review).post(merge),
        )
}

fn agents_url(binding: &str) -> String {
    format!("/cloud/app/hosts/{}/agents", escape(binding))
}

fn shell(context: &Context<'_>, headers: &HeaderMap, content: &str) -> Response {
    let resource = match context.resource(None, None, None, None) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    workspace_shell(
        context.app,
        headers,
        context.service,
        &context.viewer,
        "agents",
        Some(content),
        Some(resource),
    )
}

/// Why controls needing `right` are disabled, or `None` when offered.
fn gate(context: &Context<'_>, right: Right) -> Option<&'static str> {
    if !context.binding.access().grant.rights.contains(right) {
        return Some(match right {
            Right::Review => "The native grant admits no review decisions.",
            _ => "The native grant admits no agent or Studio effects.",
        });
    }
    let Ok(book) = context.hosts.effects(&context.viewer, context.binding.id()) else {
        return Some("Browser control custody is unavailable for this connection.");
    };
    match book.enrolled(&context.scope, context.binding.identity()) {
        Ok(true) => None,
        _ => Some("Enroll this browser at the Computer connection before any effect."),
    }
}

/// A native read whose failure leaves the rest of the page useful. Lost
/// authority still fails the whole page.
async fn read(context: &Context<'_>, operation: Operation) -> Result<Option<Outcome>, Response> {
    match context
        .binding
        .read(&context.viewer, operation.clone())
        .await
    {
        Ok(outcome) if outcome.validate().is_ok() && outcome.answers(&operation) => {
            Ok(Some(outcome))
        }
        Ok(_) => Ok(None),
        Err(error @ (SessionError::Forbidden | SessionError::Unauthenticated)) => {
            Err(refused(error))
        }
        Err(_) => Ok(None),
    }
}

fn agents_of(outcome: Option<Outcome>) -> Option<wire::Agents> {
    match outcome? {
        Outcome::Agent { agent } => serde_json::from_value(*agent).ok(),
        _ => None,
    }
}

fn snapshot_of(outcome: Option<Outcome>) -> Option<Snapshot> {
    match outcome? {
        Outcome::Studio { snapshot } => Some(*snapshot),
        _ => None,
    }
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match super::service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return protect(Redirect::to("/cloud/sign-in").into_response());
        }
        Err(error) => return refused(error),
    };
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    let mut content = String::from(
        "<h2>Agents</h2><p>Alice, crew members, and Agent Studio seats live on their host. Choose an explicitly bound host; each page checks current native Observe authority.</p><ul>",
    );
    if bindings.is_empty() {
        content.push_str(
            "<li>No agent or Studio host is admitted for this account and workspace.</li>",
        );
    }
    for binding in bindings {
        content.push_str(&format!(
            "<li><a href=\"{}\">Agents and Studio on {}</a></li>",
            agents_url(binding.id()),
            escape(binding.id())
        ));
    }
    content.push_str("</ul>");
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "agents",
        Some(&content),
        None,
    )
}

/// Owner attestation as the host reports it: identity, not execution rights.
fn attestation(view: &AgentView, at: u64) -> String {
    let identity = view.pubkey.as_deref().map_or_else(
        || "No key of her own; she is unattested.".to_owned(),
        |key| format!("Key <code>{}</code>.", escape(key)),
    );
    let standing = match (&view.pubkey, &view.authorized_by, view.attested_until) {
        (Some(_), _, Some(until)) if until <= at => format!(
            "Attestation <strong>expired</strong> at Unix second {until}. {}.",
            escape(&wire::renewal_warning(until, at).unwrap_or_default())
        ),
        (Some(_), Some(owner), Some(until)) => {
            let mut line = format!(
                "<strong>Attested</strong>: {} until Unix second {until}.",
                escape(&wire::authorized_line(owner, until, at))
            );
            if wire::renewal_warning(until, at).is_some() {
                line.push_str(" Renewal is due within 14 days.");
            }
            line
        }
        (Some(_), None, Some(until)) => format!(
            "<strong>Unattested</strong>: no owner attestation verifies now (recorded expiry Unix second {until})."
        ),
        _ => "<strong>Unattested</strong>.".to_owned(),
    };
    format!(
        "<p>{identity} {standing} An attestation identifies her and supports signed spend evidence; it grants no execution rights.</p>"
    )
}

fn word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn agent_card(binding: &str, view: &AgentView, at: u64) -> String {
    let mut card = format!(
        "<section class=\"cloud-card\" id=\"agent-{name}\"><h3><a href=\"{base}/{name}\">{name}</a> · {state}</h3>",
        name = escape(&view.name),
        base = agents_url(binding),
        state = escape(&view.state)
    );
    if let Some(role) = &view.job_role {
        card.push_str(&format!(
            "<p>Crew role: {}. Her charter limits what she may do; it grants nothing more.</p>",
            escape(&word(role))
        ));
    }
    card.push_str(&attestation(view, at));
    card.push_str(&format!(
        "<p>Activity: {}. Last report: {}.</p><p>Engine: <code>{}</code> (her planning route, or the model that answered last). The actual delegate of each request appears in her journal.</p>",
        escape(&word(&view.activity)),
        escape(if view.headline.is_empty() { "none" } else { &view.headline }),
        escape(&view.route)
    ));
    card.push_str(&format!(
        "<p>{}</p>",
        if view.busy {
            "A request is under way."
        } else {
            "No request is under way."
        }
    ));
    if let Some(step) = &view.run {
        card.push_str(&format!(
            "<p>Running step {}: <code>{}</code> in <code>{}</code>.</p>",
            step.step,
            escape(&step.command),
            escape(&step.cwd)
        ));
    }
    if let Some(pending) = &view.pending {
        card.push_str(&format!(
            "<p>Waiting proposal, step {}: <code>{}</code>. {}</p>",
            pending.step,
            escape(&pending.command),
            escape(&pending.why)
        ));
    }
    if let Some(change) = &view.change {
        card.push_str(&format!(
            "<p>Coding-task change in Studio task <code>{}</code>: {}.</p>",
            escape(&change.task),
            escape(&change.stage)
        ));
    }
    card.push_str(&format!(
        "<p>Requests {}, finished {}, merged {}. Standing jobs on: {} of {}. Proposed preferences waiting: {}.</p>",
        view.service.requests,
        view.service.finished,
        view.service.merged,
        view.jobs[0],
        view.jobs[1],
        view.candidates
    ));
    if let Some(plan) = &view.plan {
        card.push_str(&format!(
            "<p>Day plan for {}: {} blocks{}. A day plan starts no work.</p>",
            escape(&plan.date),
            plan.blocks.len(),
            plan.current.map_or_else(String::new, |index| format!(
                ", block {} under way",
                index + 1
            ))
        ));
    }
    card.push_str(&format!("<p>{LIMITS}</p><p>{METER}</p></section>"));
    card
}

fn studio_section(
    context: &Context<'_>,
    headers: &HeaderMap,
    snapshot: &Snapshot,
) -> Result<String, Response> {
    let binding = context.binding.id();
    let view = &snapshot.view;
    let mut html = format!(
        "<h2>Agent Studio</h2><p>Stream <code>{}</code> at sequence {}. Controls below bind this exact stream and sequence; a gap or a restarted host refuses them until a fresh snapshot.</p>",
        escape(&snapshot.stream),
        snapshot.sequence
    );
    let bound = |html: &mut String| {
        html.push_str(&hidden("stream", &snapshot.stream));
        html.push_str(&hidden("sequence", &snapshot.sequence.to_string()));
    };
    let operate = gate(context, Right::Operate);
    html.push_str("<h3>Seats</h3>");
    if view.seats.is_empty() {
        html.push_str("<p>No seats.</p>");
    }
    for seat in &view.seats {
        html.push_str(&format!(
            "<p><strong>{}</strong> · {} · route <code>{}</code> · {}{}{} · spend {}</p>",
            escape(&seat.seat),
            word(&seat.role),
            escape(&seat.route),
            word(&seat.activity),
            seat.task
                .as_deref()
                .map_or_else(String::new, |task| format!(
                    " · task <code>{}</code>",
                    escape(task)
                )),
            if seat.paused { " · paused" } else { "" },
            escape(&seat.spend.label())
        ));
        if operate.is_none() {
            let request = random_id();
            let csrf = context.csrf(headers, "studio-seat", &request)?;
            let button = submit(&format!("seat-{}", seat.seat), "Send seat action", true)?;
            html.push_str(&format!(
                "<form method=\"post\" action=\"/cloud/app/hosts/{}/studio/seats\">{}{}{}",
                escape(binding),
                ticket(&csrf),
                hidden("request", &request),
                hidden("seat", &seat.seat)
            ));
            bound(&mut html);
            html.push_str(&format!(
                "<label>Action <select name=\"action\"><option value=\"message\">Message</option><option value=\"pause\">Pause</option><option value=\"resume\">Resume</option><option value=\"stop\">Stop</option></select></label><label>Message <input name=\"text\" maxlength=\"4096\"></label>{button}</form>"
            ));
        }
    }
    html.push_str("<h3>Goals</h3>");
    if view.goals.is_empty() {
        html.push_str("<p>No goals.</p>");
    }
    for goal in &view.goals {
        html.push_str(&format!(
            "<p>{} · on <code>{}</code> · lead {} · {} · {} of {} tasks over · spend {}</p>",
            escape(&goal.text),
            escape(&goal.workspace),
            escape(&goal.lead),
            word(&goal.status),
            goal.final_tasks,
            goal.total_tasks,
            escape(&goal.spend.label())
        ));
    }
    if operate.is_none() && !view.repositories.is_empty() {
        let request = random_id();
        let csrf = context.csrf(headers, "studio-goal", &request)?;
        let button = submit("goal-submit", "Review goal submission", true)?;
        let options: String = view
            .repositories
            .iter()
            .map(|r| format!("<option value=\"{0}\">{0}</option>", escape(&r.workspace)))
            .collect();
        html.push_str(&format!(
            "<form method=\"post\" action=\"/cloud/app/hosts/{}/studio/goals\">{}{}",
            escape(binding),
            ticket(&csrf),
            hidden("request", &request)
        ));
        bound(&mut html);
        html.push_str(&format!("<label>Repository <select name=\"workspace\">{options}</select></label><label>Goal <textarea name=\"text\" maxlength=\"4096\" required></textarea></label><p>The lead seat plans a submitted goal. Submission records intent; it merges, pushes, and deploys nothing.</p>{button}</form>"));
    }
    html.push_str("<h3>Tasks</h3>");
    if view.tasks.is_empty() {
        html.push_str("<p>No tasks.</p>");
    }
    for task in &view.tasks {
        html.push_str(&format!(
            "<p><code>{}</code> {} · seat {} · {}{}</p>",
            escape(&task.task),
            escape(&task.title),
            escape(&task.seat),
            word(&task.status),
            if task.status == coder_access::studio::TaskStatus::Done {
                format!(
                    " · <a href=\"/cloud/app/hosts/{}/studio/tasks/{}/review?stream={}&amp;sequence={}\">Open review</a>",
                    escape(binding),
                    escape(&task.task),
                    escape(&snapshot.stream),
                    snapshot.sequence
                )
            } else {
                String::new()
            }
        ));
    }
    html.push_str("<h3>Decisions and questions</h3>");
    if view.decisions.is_empty() {
        html.push_str("<p>No open decisions.</p>");
    }
    for decision in &view.decisions {
        html.push_str(&format!(
            "<section class=\"cloud-card\"><p>{} for goal <code>{}</code>{}: {}</p><p>Basis {}.</p>",
            word(&decision.kind),
            escape(&decision.goal),
            decision
                .seat
                .as_deref()
                .map_or_else(String::new, |seat| format!(" from {}", escape(seat))),
            escape(&decision.text),
            decision.based_on
        ));
        if let Some(approval) = &decision.approval {
            html.push_str(&format!(
                "<p>{}: <code>{}</code> runs <code>{}</code> in <code>{}</code>. {}</p>",
                approval.risk.label(),
                escape(&approval.tool),
                escape(&approval.command),
                escape(&approval.cwd),
                escape(&approval.reason)
            ));
        }
        match operate {
            Some(reason) => html.push_str(&format!("<p>{}</p>", escape(reason))),
            None => {
                let request = random_id();
                let csrf = context.csrf(headers, "studio-decision", &request)?;
                let button = submit(
                    &format!("decision-{}", decision.decision),
                    "Review this exact answer",
                    true,
                )?;
                html.push_str(&format!(
                    "<form method=\"post\" action=\"/cloud/app/hosts/{}/studio/decisions\">{}{}{}{}{}{}",
                    escape(binding),
                    ticket(&csrf),
                    hidden("request", &request),
                    hidden("decision", &decision.decision),
                    hidden("based_on", &decision.based_on.to_string()),
                    hidden("command", &random_id()),
                    hidden("issued_at", &now().to_string())
                ));
                bound(&mut html);
                html.push_str(&format!(
                    "<label>Answer <textarea name=\"text\" maxlength=\"65536\" required></textarea></label>{button}</form>"
                ));
            }
        }
        html.push_str("</section>");
    }
    if let Some(reason) = operate {
        html.push_str(&format!("<p>Studio controls: {}</p>", escape(reason)));
    }
    Ok(html)
}

async fn overview(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let agents = match read(&context, Operation::ListAgents {}).await {
        Ok(value) => agents_of(value),
        Err(response) => return response,
    };
    let studio = match read(&context, Operation::StudioSnapshot {}).await {
        Ok(value) => snapshot_of(value),
        Err(response) => return response,
    };
    if let Err(response) = context.current(&headers).await {
        return response;
    }
    let at = now();
    let mut content = format!(
        "{}<h2>Agents on {}</h2><p>Host <code>{}</code>, generation {}. {CONFIGURATION}</p>",
        super::controls::link(context.binding),
        escape(context.binding.id()),
        escape(context.binding.host()),
        context.binding.generation()
    );
    match agents {
        Some(list) if list.agents.is_empty() => {
            content.push_str("<p>This host has no workshop agents.</p>")
        }
        Some(list) => {
            for view in &list.agents {
                content.push_str(&agent_card(context.binding.id(), view, at));
            }
        }
        None => content.push_str(
            "<p>Agents: unavailable. The host did not answer an admitted, bounded agent list.</p>",
        ),
    }
    match studio {
        Some(snapshot) => match studio_section(&context, &headers, &snapshot) {
            Ok(html) => content.push_str(&html),
            Err(response) => return response,
        },
        None => content.push_str("<h2>Agent Studio</h2><p>Unavailable. The host has no Studio, or did not answer an admitted, bounded snapshot. No decision or review control is offered.</p>"),
    }
    shell(&context, &headers, &content)
}

async fn agent_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Response {
    if wire::name(&name).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let list = match read(&context, Operation::ListAgents {}).await {
        Ok(value) => agents_of(value),
        Err(response) => return response,
    };
    let Some(view) = list.and_then(|list| list.agents.into_iter().find(|a| a.name == name)) else {
        return failure(
            StatusCode::NOT_FOUND,
            "Agent unavailable",
            "This host reports no such agent, or its agent list is unavailable.",
        );
    };
    let jobs = match read(
        &context,
        Operation::ListAgentJobs {
            agent: name.clone(),
        },
    )
    .await
    {
        Ok(Some(Outcome::Agent { agent })) => serde_json::from_value::<wire::Jobs>(*agent).ok(),
        Ok(_) => None,
        Err(response) => return response,
    };
    let memory = match read(
        &context,
        Operation::ListAgentMemory {
            agent: name.clone(),
            after: None,
        },
    )
    .await
    {
        Ok(Some(Outcome::Agent { agent })) => serde_json::from_value::<wire::Memory>(*agent).ok(),
        Ok(_) => None,
        Err(response) => return response,
    };
    if let Err(response) = context.current(&headers).await {
        return response;
    }
    let at = now();
    let mut content = format!(
        "<p><a href=\"{}\">All agents and Studio</a></p>{}",
        agents_url(context.binding.id()),
        agent_card(context.binding.id(), &view, at)
    );
    content.push_str("<h3>Standing jobs</h3>");
    match jobs {
        Some(jobs) if jobs.jobs.is_empty() => content.push_str("<p>No standing jobs.</p>"),
        Some(jobs) => {
            for job in &jobs.jobs {
                content.push_str(&format!(
                    "<p><code>{}</code> {} · trigger {} · {} · {} of {} occurrences · expires Unix second {}{}</p>",
                    escape(&job.job),
                    escape(&job.title),
                    escape(&job.trigger),
                    if job.enabled { "enabled" } else { "disabled" },
                    job.occurrences,
                    job.max_occurrences,
                    job.expires_at,
                    job.last.as_deref().map_or_else(String::new, |last| format!(" · last: {}", escape(last)))
                ));
            }
            content.push_str("<p>New jobs start disabled and expire within 90 days; creating or renewing one needs the host's own confirmation.</p>");
        }
        None => content.push_str("<p>Unavailable.</p>"),
    }
    content.push_str("<h3>Memory</h3>");
    match memory {
        Some(memory) if memory.memory.is_empty() => content.push_str("<p>No memory entries.</p>"),
        Some(memory) => {
            for row in &memory.memory {
                content.push_str(&format!(
                    "<p>{} · {} · {}</p>",
                    escape(&row.kind),
                    escape(&row.state),
                    escape(&row.text)
                ));
            }
            content.push_str("<p>Proposed preferences, engrams, and opt-in sync keep their own privacy and acceptance rules.</p>");
        }
        None => content.push_str("<p>Unavailable, or larger than one bounded browser read.</p>"),
    }
    match gate(&context, Right::Operate) {
        Some(reason) => content.push_str(&format!("<h3>Requests</h3><p>{}</p>", escape(reason))),
        None => {
            let request = random_id();
            let csrf = match context.csrf(&headers, "agent-ask", &format!("{name}:{request}")) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let button = match submit("agent-ask", "Review this exact request", true) {
                Ok(value) => value,
                Err(response) => return response,
            };
            content.push_str(&format!(
                "<h3>New request</h3><form method=\"post\" action=\"{}/{}/ask\">{}{}<fieldset><legend>Mode</legend><label><input type=\"radio\" name=\"mode\" value=\"task\" required> Coding task: a change in her own worktree, merged only by a Studio review decision.</label><label><input type=\"radio\" name=\"mode\" value=\"terminal\" required> Terminal request: commands she plans; anything not read-only waits for your confirmation.</label></fieldset><label>Request <textarea name=\"text\" maxlength=\"16384\" required></textarea></label><p>The request identity is fixed when this form is shown. A retry after a lost reply or a host restart sends the same bytes and starts nothing new; changed text under the same identity is refused.</p>{button}</form>",
                agents_url(context.binding.id()),
                escape(&name),
                ticket(&csrf),
                hidden("request", &request)
            ));
            if let Some(pending) = &view.pending {
                let request = random_id();
                let csrf =
                    match context.csrf(&headers, "agent-proposal", &format!("{name}:{request}")) {
                        Ok(value) => value,
                        Err(response) => return response,
                    };
                let button = match submit("agent-proposal", "Review this exact answer", true) {
                    Ok(value) => value,
                    Err(response) => return response,
                };
                content.push_str(&format!(
                    "<h3>Waiting proposal</h3><form method=\"post\" action=\"{}/{}/proposal\">{}{}{}<label><input type=\"radio\" name=\"confirm\" value=\"yes\" required> Confirm step {}</label><label><input type=\"radio\" name=\"confirm\" value=\"no\" required> Reject</label>{button}</form>",
                    agents_url(context.binding.id()),
                    escape(&name),
                    ticket(&csrf),
                    hidden("request", &request),
                    hidden("step", &pending.step.to_string()),
                    pending.step
                ));
            }
        }
    }
    content.push_str(&format!("<p>{CONFIGURATION}</p>"));
    shell(&context, &headers, &content)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskForm {
    csrf: String,
    request: String,
    mode: String,
    text: String,
}

async fn ask(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
    form: Result<Form<AskForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(
        &headers,
        &form.csrf,
        "agent-ask",
        &format!("{name}:{}", form.request),
    ) {
        return response;
    }
    // The browser names the mode; the host's first-word heuristic is never
    // chosen on the person's behalf.
    let mode = match form.mode.as_str() {
        "task" => Mode::Task,
        "terminal" => Mode::Terminal,
        _ => return refused(SessionError::InvalidRequest),
    };
    if wire::name(&name).is_err() || wire::request_text(&form.text).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::AskAgent {
        agent: name,
        text: form.text,
        workspace: None,
        context: String::new(),
        mode,
        typist: false,
        computer: None,
    };
    staged(&context, &headers, &form.request, operation).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalForm {
    csrf: String,
    request: String,
    step: u64,
    confirm: String,
}

async fn proposal(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
    form: Result<Form<ProposalForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(
        &headers,
        &form.csrf,
        "agent-proposal",
        &format!("{name}:{}", form.request),
    ) {
        return response;
    }
    let confirm = match form.confirm.as_str() {
        "yes" => true,
        "no" => false,
        _ => return refused(SessionError::InvalidRequest),
    };
    if wire::name(&name).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    // The proposal must still be the one displayed.
    let list = match read(&context, Operation::ListAgents {}).await {
        Ok(value) => agents_of(value),
        Err(response) => return response,
    };
    let current = list
        .and_then(|list| list.agents.into_iter().find(|a| a.name == name))
        .and_then(|view| view.pending)
        .map(|pending| pending.step);
    if current != Some(form.step) {
        return refused(SessionError::Conflict);
    }
    let operation = Operation::AnswerAgent {
        agent: name,
        step: form.step,
        confirm,
    };
    staged(&context, &headers, &form.request, operation).await
}

/// The update from the displayed point: a gap, another stream, or an
/// unreadable answer refuses until a fresh snapshot.
async fn continuity(
    context: &Context<'_>,
    stream: &str,
    sequence: u64,
) -> Result<Update, Response> {
    let stale = || {
        failure(
            StatusCode::CONFLICT,
            "Fresh Studio snapshot required",
            STALE,
        )
    };
    if coder_access::studio::stream_id(stream).is_err() {
        return Err(refused(SessionError::InvalidRequest));
    }
    let operation = Operation::StudioUpdate {
        stream: stream.into(),
        since: sequence,
    };
    match context
        .binding
        .read(&context.viewer, operation.clone())
        .await
    {
        Ok(outcome) if outcome.validate().is_ok() && outcome.answers(&operation) => match outcome {
            Outcome::StudioUpdate { update } => Ok(*update),
            _ => Err(stale()),
        },
        Ok(_) | Err(SessionError::Conflict) => Err(stale()),
        Err(error) => Err(refused(error)),
    }
}

/// Whether `update` changed or removed the item `id` of `kind`.
pub(super) fn touched(update: &Update, kind: Kind, id: &str) -> bool {
    let put = &update.put;
    update
        .removed
        .iter()
        .any(|gone| gone.kind == kind && gone.id == id)
        || match kind {
            Kind::Decision => put.decisions.iter().any(|d| d.decision == id),
            Kind::Task => put.tasks.iter().any(|t| t.task == id),
            Kind::Seat => put.seats.iter().any(|s| s.seat == id),
            Kind::Goal => put.goals.iter().any(|g| g.goal == id),
            _ => false,
        }
}

fn fresh_issue(issued_at: u64) -> bool {
    let at = now();
    issued_at <= at && at - issued_at <= ISSUED_WINDOW
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalForm {
    csrf: String,
    request: String,
    stream: String,
    sequence: u64,
    workspace: String,
    text: String,
}

async fn goal(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<GoalForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(&headers, &form.csrf, "studio-goal", &form.request) {
        return response;
    }
    if let Err(response) = continuity(&context, &form.stream, form.sequence).await {
        return response;
    }
    let operation = Operation::SubmitGoal {
        text: form.text,
        workspace: form.workspace,
        lead: None,
    };
    staged(&context, &headers, &form.request, operation).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatForm {
    csrf: String,
    request: String,
    stream: String,
    sequence: u64,
    seat: String,
    action: String,
    #[serde(default)]
    text: String,
}

async fn seat(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<SeatForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(&headers, &form.csrf, "studio-seat", &form.request) {
        return response;
    }
    let update = match continuity(&context, &form.stream, form.sequence).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if update
        .removed
        .iter()
        .any(|gone| gone.kind == Kind::Seat && gone.id == form.seat)
    {
        return refused(SessionError::Conflict);
    }
    let seat = form.seat;
    let operation = match form.action.as_str() {
        "message" => Operation::MessageSeat {
            seat: Some(seat),
            text: form.text,
        },
        "pause" if form.text.is_empty() => Operation::PauseSeat { seat },
        "resume" if form.text.is_empty() => Operation::ResumeSeat { seat },
        "stop" if form.text.is_empty() => Operation::StopSeat { seat },
        _ => return refused(SessionError::InvalidRequest),
    };
    staged(&context, &headers, &form.request, operation).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionForm {
    csrf: String,
    request: String,
    stream: String,
    sequence: u64,
    decision: String,
    based_on: u64,
    command: String,
    issued_at: u64,
    text: String,
}

async fn decision(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<DecisionForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(&headers, &form.csrf, "studio-decision", &form.request) {
        return response;
    }
    if !fresh_issue(form.issued_at) {
        return refused(SessionError::Conflict);
    }
    let update = match continuity(&context, &form.stream, form.sequence).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    // A changed or answered decision needs a fresh read before any answer.
    if touched(&update, Kind::Decision, &form.decision) {
        return failure(
            StatusCode::CONFLICT,
            "Decision changed",
            "This decision changed or closed since the page displayed it. Reopen the Studio and review the current decision.",
        );
    }
    let operation = Operation::AnswerDecision {
        decision: form.decision,
        based_on: form.based_on,
        text: form.text,
        command: form.command,
        issued_at: form.issued_at,
    };
    staged(&context, &headers, &form.request, operation).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Displayed {
    stream: String,
    sequence: u64,
}

async fn read_review(context: &Context<'_>, task: &str) -> Result<TaskReview, Response> {
    let operation = Operation::OpenReview { task: task.into() };
    match read(context, operation).await? {
        Some(Outcome::Review { review }) if review.validate().is_ok() => Ok(*review),
        _ => Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Review unavailable",
            "The host did not answer an admitted, bounded review for this task. No merge decision is offered.",
        )),
    }
}

async fn review(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    axum::extract::Query(displayed): axum::extract::Query<Displayed>,
) -> Response {
    if coder_access::studio::id(&task).is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    // The review belongs to the displayed Studio point; a gap needs a fresh snapshot.
    let update = match continuity(&context, &displayed.stream, displayed.sequence).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if touched(&update, Kind::Task, &task) {
        return failure(
            StatusCode::CONFLICT,
            "Task changed",
            "This task changed since the Studio page displayed it. Reopen the Studio for a fresh snapshot.",
        );
    }
    let review = match read_review(&context, &task).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.current(&headers).await {
        return response;
    }
    let mut content = format!(
        "<p><a href=\"{}\">All agents and Studio</a></p><h2>Review of <code>{}</code></h2><p>Base <code>{}</code>, head commit <code>{}</code>, tree <code>{}</code>. A decision binds these three revisions; a changed candidate requires a fresh review.</p><p>{} files changed ({} shown), +{} −{}, {} uncounted.</p><ul>",
        agents_url(context.binding.id()),
        escape(&review.task),
        escape(&review.base),
        escape(&review.head_commit),
        escape(&review.head),
        review.files_total,
        review.files.len(),
        review.added,
        review.removed,
        review.uncounted
    );
    for file in &review.files {
        content.push_str(&format!(
            "<li><code>{}</code> {} +{} −{}</li>",
            escape(&file.path),
            word(&file.status),
            file.added.map_or_else(|| "?".into(), |n| n.to_string()),
            file.removed.map_or_else(|| "?".into(), |n| n.to_string())
        ));
    }
    content.push_str("</ul>");
    let complete = match &review.completeness {
        Completeness::Complete => {
            content.push_str("<p>The diff below is the whole change.</p>");
            true
        }
        Completeness::Truncated { shown, total } => {
            content.push_str(&format!(
                "<p>The diff is truncated at {shown} bytes of {}.</p>",
                total.map_or_else(|| "an unmeasured total".into(), |t| t.to_string())
            ));
            true
        }
        Completeness::Unknown { reason } => {
            content.push_str(&format!(
                "<p>The change could not be read: {}. Merge is not offered for an unread change.</p>",
                escape(reason)
            ));
            false
        }
    };
    content.push_str(&format!("<pre>{}</pre>", escape(&review.diff)));
    if let Some(publication) = &review.publication {
        content.push_str(&format!(
            "<p>Last publication: {} — {}.</p>",
            word(&publication.state),
            escape(&publication.note)
        ));
    }
    content.push_str("<p>Merge hands the reviewed tree to the host's landing path, which updates the host checkout. Merge implies no deploy; any push appears only as the host's reported publication state. The host refuses a dirty checkout, a detached head, a conflicting merge, or a changed candidate, and an uncertain result stays unknown. Agent completion and auto-start never approve a merge.</p>");
    match gate(&context, Right::Review) {
        Some(reason) => content.push_str(&format!("<p>{}</p>", escape(reason))),
        None => {
            let request = random_id();
            let csrf = match context.csrf(&headers, "studio-merge", &format!("{task}:{request}")) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let button = match submit("studio-merge", "Review this exact decision", true) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let merge = if complete {
                "<label><input type=\"radio\" name=\"verdict\" value=\"merge\" required> Merge at these revisions</label>"
            } else {
                ""
            };
            content.push_str(&format!(
                "<form method=\"post\">{}{}{}{}{}{}{}{}{}<fieldset><legend>Decision</legend>{merge}<label><input type=\"radio\" name=\"verdict\" value=\"request_changes\" required> Request changes</label><label><input type=\"radio\" name=\"verdict\" value=\"reject\" required> Reject</label></fieldset><label>Changes or reason <textarea name=\"text\" maxlength=\"16384\"></textarea></label>{button}</form>",
                ticket(&csrf),
                hidden("request", &request),
                hidden("stream", &displayed.stream),
                hidden("sequence", &displayed.sequence.to_string()),
                hidden("base", &review.base),
                hidden("head_commit", &review.head_commit),
                hidden("head", &review.head),
                hidden("command", &random_id()),
                hidden("issued_at", &now().to_string())
            ));
        }
    }
    shell(&context, &headers, &content)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeForm {
    csrf: String,
    request: String,
    stream: String,
    sequence: u64,
    base: String,
    head_commit: String,
    head: String,
    command: String,
    issued_at: u64,
    verdict: String,
    #[serde(default)]
    text: String,
}

async fn merge(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    form: Result<Form<MergeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(
        &headers,
        &form.csrf,
        "studio-merge",
        &format!("{task}:{}", form.request),
    ) {
        return response;
    }
    if !fresh_issue(form.issued_at) {
        return refused(SessionError::Conflict);
    }
    let verdict = match form.verdict.as_str() {
        "merge" => Verdict::Merge,
        "request_changes" => Verdict::RequestChanges,
        "reject" => Verdict::Reject,
        _ => return refused(SessionError::InvalidRequest),
    };
    let update = match continuity(&context, &form.stream, form.sequence).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if touched(&update, Kind::Task, &task) {
        return failure(
            StatusCode::CONFLICT,
            "Task changed",
            "This task changed since its review was displayed. Read a fresh review before deciding.",
        );
    }
    let current = match read_review(&context, &task).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if current.base != form.base
        || current.head_commit != form.head_commit
        || current.head != form.head
    {
        return failure(
            StatusCode::CONFLICT,
            "Candidate changed",
            "The candidate's base, head commit, or tree changed since the review was displayed. A changed candidate requires a fresh review.",
        );
    }
    if verdict == Verdict::Merge && matches!(current.completeness, Completeness::Unknown { .. }) {
        return refused(SessionError::Conflict);
    }
    let decision = MergeDecision {
        task,
        base: form.base,
        head_commit: form.head_commit,
        head: form.head,
        verdict,
        text: if verdict == Verdict::Merge {
            String::new()
        } else {
            form.text
        },
        command: form.command,
        issued_at: form.issued_at,
    };
    if decision.validate().is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::DecideMerge {
        decision: Box::new(decision),
    };
    staged(&context, &headers, &form.request, operation).await
}
