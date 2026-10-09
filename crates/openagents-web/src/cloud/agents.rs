//! Alice, crew, and Agent Studio observation with exact native decisions.
//!
//! The resident host owns agents, Studio coordination, reviews, and merges.
//! This adapter reads them through the explicit binding, binds every control
//! to what the page displayed (the Studio stream and sequence, a decision's
//! basis, a review's three revisions), and stages each effect as one exact
//! retained native packet. A gap in the Studio stream refuses every control
//! until the page reads a fresh snapshot.

use super::controls::{Context, admitted, staged};
use super::session::{SessionError, now};
use super::ui::{self, Tone};
use super::{failure, protect, refused, workspace_shell};
use crate::App;
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
use maud::{Markup, PreEscaped, html};
use openagents_ui::content::CodeBlock;
use openagents_ui::forms::{Direction, Field, Input, RadioGroup, Select, Textarea};
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
    format!("/cloud/app/hosts/{binding}/agents")
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
    let content = html! {
        h2 { "Agents" }
        p { "Alice, crew members, and Agent Studio seats live on their host. Choose an explicitly bound host; each page checks current native Observe authority." }
        ul {
            @if bindings.is_empty() {
                li { "No agent or Studio host is admitted for this account and workspace." }
            }
            @for binding in &bindings {
                li { a href=(agents_url(binding.id())) { "Agents and Studio on " (binding.id()) } }
            }
        }
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "agents",
        Some(&content.into_string()),
        None,
    )
}

/// Owner attestation as the host reports it: identity, not execution rights.
fn attestation(view: &AgentView, at: u64) -> Markup {
    let identity = html! {
        @match view.pubkey.as_deref() {
            Some(key) => { "Key " code { (key) } "." }
            None => { "No key of her own; she is unattested." }
        }
    };
    let standing = match (&view.pubkey, &view.authorized_by, view.attested_until) {
        (Some(_), _, Some(until)) if until <= at => html! {
            "Attestation " strong { "expired" } " at Unix second " (until) ". "
            (wire::renewal_warning(until, at).unwrap_or_default()) "."
        },
        (Some(_), Some(owner), Some(until)) => html! {
            strong { "Attested" } ": " (wire::authorized_line(owner, until, at))
            " until Unix second " (until) "."
            @if wire::renewal_warning(until, at).is_some() { " Renewal is due within 14 days." }
        },
        (Some(_), None, Some(until)) => html! {
            strong { "Unattested" }
            ": no owner attestation verifies now (recorded expiry Unix second " (until) ")."
        },
        _ => html! { strong { "Unattested" } "." },
    };
    html! {
        p { (identity) " " (standing) " An attestation identifies her and supports signed spend evidence; it grants no execution rights." }
    }
}

fn word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn agent_card(binding: &str, view: &AgentView, at: u64) -> Markup {
    html! {
        section class="cloud-card" id=(format!("agent-{}", view.name)) {
            h3 {
                a href=(format!("{}/{}", agents_url(binding), view.name)) { (view.name) }
                " " (ui::status(&view.state, Tone::Neutral))
            }
            @if let Some(role) = &view.job_role {
                p { "Crew role: " (word(role)) ". Her charter limits what she may do; it grants nothing more." }
            }
            (attestation(view, at))
            p {
                "Activity: " (word(&view.activity)) ". Last report: "
                (if view.headline.is_empty() { "none" } else { view.headline.as_str() }) "."
            }
            p {
                "Engine: " code { (view.route) }
                " (her planning route, or the model that answered last). The actual delegate of each request appears in her journal."
            }
            p {
                @if view.busy { "A request is under way." } @else { "No request is under way." }
            }
            @if let Some(step) = &view.run {
                p { "Running step " (step.step) ": " code { (step.command) } " in " code { (step.cwd) } "." }
            }
            @if let Some(pending) = &view.pending {
                p { "Waiting proposal, step " (pending.step) ": " code { (pending.command) } ". " (pending.why) }
            }
            @if let Some(change) = &view.change {
                p { "Coding-task change in Studio task " code { (change.task) } ": " (change.stage) "." }
            }
            p {
                "Requests " (view.service.requests) ", finished " (view.service.finished)
                ", merged " (view.service.merged) ". Standing jobs on: " (view.jobs[0])
                " of " (view.jobs[1]) ". Proposed preferences waiting: " (view.candidates) "."
            }
            @if let Some(plan) = &view.plan {
                p {
                    "Day plan for " (plan.date) ": " (plan.blocks.len()) " blocks"
                    @if let Some(index) = plan.current { ", block " (index + 1) " under way" }
                    ". A day plan starts no work."
                }
            }
            p { (LIMITS) }
            p { (METER) }
        }
    }
}

/// A form bound to the displayed Studio stream and sequence.
fn studio_form(action: String, csrf: &str, request: &str, snapshot: &Snapshot) -> ui::BoundForm {
    ui::BoundForm::new(action)
        .csrf(csrf)
        .bind("request", request)
        .bind("stream", &snapshot.stream)
        .bind("sequence", &snapshot.sequence.to_string())
}

fn studio_section(
    context: &Context<'_>,
    headers: &HeaderMap,
    snapshot: &Snapshot,
) -> Result<Markup, Response> {
    let binding = context.binding.id();
    let view = &snapshot.view;
    let operate = gate(context, Right::Operate);
    let mut seats = Vec::with_capacity(view.seats.len());
    for seat in &view.seats {
        let form = if operate.is_none() {
            let request = random_id();
            let csrf = context.csrf(headers, "studio-seat", &request)?;
            let id = format!("seat-{}", seat.seat);
            let action = Field::new(format!("{id}-action"), "Action");
            let message = Field::new(format!("{id}-text"), "Message");
            Some(
                ui::BoundForm::new(format!("/cloud/app/hosts/{binding}/studio/seats"))
                    .csrf(&csrf)
                    .bind("request", &request)
                    .bind("seat", &seat.seat)
                    .bind("stream", &snapshot.stream)
                    .bind("sequence", &snapshot.sequence.to_string())
                    .body(html! {
                        (action.clone().control(
                            Select::new("action")
                                .aria(action.aria())
                                .option("message", "Message")
                                .option("pause", "Pause")
                                .option("resume", "Resume")
                                .option("stop", "Stop"),
                        ))
                        (message.clone().control(
                            Input::new("text").aria(message.aria()).maxlength(4096),
                        ))
                    })
                    .submit("Send seat action"),
            )
        } else {
            None
        };
        seats.push((seat, form));
    }
    let goal_form = if operate.is_none() && !view.repositories.is_empty() {
        let request = random_id();
        let csrf = context.csrf(headers, "studio-goal", &request)?;
        let repository = Field::new("goal-workspace", "Repository");
        let text = Field::new("goal-text", "Goal").required(true);
        let mut select = Select::new("workspace").aria(repository.aria());
        for r in &view.repositories {
            select = select.option(r.workspace.clone(), r.workspace.clone());
        }
        Some(
            studio_form(
                format!("/cloud/app/hosts/{binding}/studio/goals"),
                &csrf,
                &request,
                snapshot,
            )
            .body(html! {
                (repository.clone().control(select))
                (text.clone().control(Textarea::new("text").aria(text.aria()).maxlength(4096)))
                p { "The lead seat plans a submitted goal. Submission records intent; it merges, pushes, and deploys nothing." }
            })
            .submit("Review goal submission"),
        )
    } else {
        None
    };
    let mut decisions = Vec::with_capacity(view.decisions.len());
    for decision in &view.decisions {
        let form = if operate.is_none() {
            let request = random_id();
            let csrf = context.csrf(headers, "studio-decision", &request)?;
            let answer =
                Field::new(format!("decision-{}-text", decision.decision), "Answer").required(true);
            Some(
                ui::BoundForm::new(format!("/cloud/app/hosts/{binding}/studio/decisions"))
                    .csrf(&csrf)
                    .bind("request", &request)
                    .bind("decision", &decision.decision)
                    .bind("based_on", &decision.based_on.to_string())
                    .bind("command", &random_id())
                    .bind("issued_at", &now().to_string())
                    .bind("stream", &snapshot.stream)
                    .bind("sequence", &snapshot.sequence.to_string())
                    .body(
                        answer
                            .clone()
                            .control(Textarea::new("text").aria(answer.aria()).maxlength(65536)),
                    )
                    .submit("Review this exact answer"),
            )
        } else {
            None
        };
        decisions.push((decision, form));
    }
    Ok(html! {
        h2 { "Agent Studio" }
        p {
            "Stream " code { (snapshot.stream) } " at sequence " (snapshot.sequence)
            ". Controls below bind this exact stream and sequence; a gap or a restarted host refuses them until a fresh snapshot."
        }
        h3 { "Seats" }
        @if view.seats.is_empty() { p { "No seats." } }
        @for (seat, form) in &seats {
            p {
                strong { (seat.seat) } " \u{b7} " (word(&seat.role)) " \u{b7} route " code { (seat.route) }
                " \u{b7} " (word(&seat.activity))
                @if let Some(task) = seat.task.as_deref() { " \u{b7} task " code { (task) } }
                @if seat.paused { " \u{b7} paused" }
                " \u{b7} spend " (seat.spend.label())
            }
            @if let Some(form) = form { (form) }
        }
        h3 { "Goals" }
        @if view.goals.is_empty() { p { "No goals." } }
        @for goal in &view.goals {
            p {
                (goal.text) " \u{b7} on " code { (goal.workspace) } " \u{b7} lead " (goal.lead)
                " \u{b7} " (word(&goal.status)) " \u{b7} " (goal.final_tasks) " of " (goal.total_tasks)
                " tasks over \u{b7} spend " (goal.spend.label())
            }
        }
        @if let Some(form) = &goal_form { (form) }
        h3 { "Tasks" }
        @if view.tasks.is_empty() { p { "No tasks." } }
        @for task in &view.tasks {
            p {
                code { (task.task) } " " (task.title) " \u{b7} seat " (task.seat) " \u{b7} " (word(&task.status))
                @if task.status == coder_access::studio::TaskStatus::Done {
                    " \u{b7} "
                    a href=(format!(
                        "/cloud/app/hosts/{binding}/studio/tasks/{}/review?stream={}&sequence={}",
                        task.task, snapshot.stream, snapshot.sequence
                    )) { "Open review" }
                }
            }
        }
        h3 { "Decisions and questions" }
        @if view.decisions.is_empty() { p { "No open decisions." } }
        @for (decision, form) in &decisions {
            section class="cloud-card" {
                p {
                    (word(&decision.kind)) " for goal " code { (decision.goal) }
                    @if let Some(seat) = decision.seat.as_deref() { " from " (seat) }
                    ": " (decision.text)
                }
                p { "Basis " (decision.based_on) "." }
                @if let Some(approval) = &decision.approval {
                    p {
                        (approval.risk.label()) ": " code { (approval.tool) } " runs " code { (approval.command) }
                        " in " code { (approval.cwd) } ". " (approval.reason)
                    }
                }
                @match (form, operate) {
                    (Some(form), _) => { (form) }
                    (None, Some(reason)) => { p { (reason) } }
                    (None, None) => {}
                }
            }
        }
        @if let Some(reason) = operate {
            (ui::unavailable("Studio controls", html! { "Studio controls: " (reason) }))
        }
    })
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
    let studio = match studio {
        Some(snapshot) => match studio_section(&context, &headers, &snapshot) {
            Ok(html) => Some(html),
            Err(response) => return response,
        },
        None => None,
    };
    let content = html! {
        (PreEscaped(super::controls::link(context.binding)))
        h2 { "Agents on " (context.binding.id()) }
        p {
            "Host " code { (context.binding.host()) } ", generation "
            (context.binding.generation()) ". " (CONFIGURATION)
        }
        @match &agents {
            Some(list) if list.agents.is_empty() => {
                (ui::empty("No workshop agents", "This host has no workshop agents."))
            }
            Some(list) => {
                @for view in &list.agents { (agent_card(context.binding.id(), view, at)) }
            }
            None => {
                (ui::unavailable("Agents: unavailable", "Agents: unavailable. The host did not answer an admitted, bounded agent list."))
            }
        }
        @match studio {
            Some(section) => { (section) }
            None => {
                h2 { "Agent Studio" }
                (ui::unavailable("Agent Studio unavailable", "Unavailable. The host has no Studio, or did not answer an admitted, bounded snapshot. No decision or review control is offered."))
            }
        }
    };
    shell(&context, &headers, &content.into_string())
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
    let base = format!("{}/{}", agents_url(context.binding.id()), name);
    let operate = gate(&context, Right::Operate);
    let mut ask_form = None;
    let mut proposal_form = None;
    if operate.is_none() {
        let request = random_id();
        let csrf = match context.csrf(&headers, "agent-ask", &format!("{name}:{request}")) {
            Ok(value) => value,
            Err(response) => return response,
        };
        let mode = Field::new("agent-ask-mode", "Mode")
            .group(true)
            .required(true);
        let text = Field::new("agent-ask-text", "Request").required(true);
        ask_form = Some(
            ui::BoundForm::new(format!("{base}/ask"))
                .csrf(&csrf)
                .bind("request", &request)
                .body(html! {
                    (mode.clone().control(
                        RadioGroup::new("mode")
                            .aria(mode.aria())
                            .direction(Direction::Col)
                            .option("task", "Coding task: a change in her own worktree, merged only by a Studio review decision.")
                            .option("terminal", "Terminal request: commands she plans; anything not read-only waits for your confirmation."),
                    ))
                    (text.clone().control(Textarea::new("text").aria(text.aria()).maxlength(16384)))
                    p { "The request identity is fixed when this form is shown. A retry after a lost reply or a host restart sends the same bytes and starts nothing new; changed text under the same identity is refused." }
                })
                .submit("Review this exact request"),
        );
        if let Some(pending) = &view.pending {
            let request = random_id();
            let csrf = match context.csrf(&headers, "agent-proposal", &format!("{name}:{request}"))
            {
                Ok(value) => value,
                Err(response) => return response,
            };
            let confirm = Field::new("agent-proposal-confirm", "Answer")
                .group(true)
                .required(true);
            proposal_form = Some(
                ui::BoundForm::new(format!("{base}/proposal"))
                    .csrf(&csrf)
                    .bind("request", &request)
                    .bind("step", &pending.step.to_string())
                    .body(
                        confirm.clone().control(
                            RadioGroup::new("confirm")
                                .aria(confirm.aria())
                                .direction(Direction::Col)
                                .option("yes", format!("Confirm step {}", pending.step))
                                .option("no", "Reject"),
                        ),
                    )
                    .submit("Review this exact answer"),
            );
        }
    }
    let content = html! {
        (ui::links([(agents_url(context.binding.id()).as_str(), "All agents and Studio")]))
        (agent_card(context.binding.id(), &view, at))
        h3 { "Standing jobs" }
        @match &jobs {
            Some(jobs) if jobs.jobs.is_empty() => { p { "No standing jobs." } }
            Some(jobs) => {
                @for job in &jobs.jobs {
                    p {
                        code { (job.job) } " " (job.title) " \u{b7} trigger " (job.trigger) " \u{b7} "
                        (if job.enabled { "enabled" } else { "disabled" }) " \u{b7} "
                        (job.occurrences) " of " (job.max_occurrences)
                        " occurrences \u{b7} expires Unix second " (job.expires_at)
                        @if let Some(last) = job.last.as_deref() { " \u{b7} last: " (last) }
                    }
                }
                p { "New jobs start disabled and expire within 90 days; creating or renewing one needs the host's own confirmation." }
            }
            None => { p { "Unavailable." } }
        }
        h3 { "Memory" }
        @match &memory {
            Some(memory) if memory.memory.is_empty() => { p { "No memory entries." } }
            Some(memory) => {
                @for row in &memory.memory {
                    p { (row.kind) " \u{b7} " (row.state) " \u{b7} " (row.text) }
                }
                p { "Proposed preferences, engrams, and opt-in sync keep their own privacy and acceptance rules." }
            }
            None => { p { "Unavailable, or larger than one bounded browser read." } }
        }
        @if let Some(reason) = operate {
            h3 { "Requests" }
            p { (reason) }
        }
        @if let Some(form) = &ask_form {
            h3 { "New request" }
            (form)
        }
        @if let Some(form) = &proposal_form {
            h3 { "Waiting proposal" }
            (form)
        }
        p { (CONFIGURATION) }
    };
    shell(&context, &headers, &content.into_string())
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
    let complete = !matches!(review.completeness, Completeness::Unknown { .. });
    let form = match gate(&context, Right::Review) {
        Some(_) => None,
        None => {
            let request = random_id();
            let csrf = match context.csrf(&headers, "studio-merge", &format!("{task}:{request}")) {
                Ok(value) => value,
                Err(response) => return response,
            };
            let decision = Field::new("studio-merge-verdict", "Decision")
                .group(true)
                .required(true);
            let mut verdicts = RadioGroup::new("verdict")
                .aria(decision.aria())
                .direction(Direction::Col);
            if complete {
                verdicts = verdicts.option("merge", "Merge at these revisions");
            }
            verdicts = verdicts
                .option("request_changes", "Request changes")
                .option("reject", "Reject");
            let text = Field::new("studio-merge-text", "Changes or reason");
            Some(
                ui::BoundForm::here()
                    .csrf(&csrf)
                    .bind("request", &request)
                    .bind("stream", &displayed.stream)
                    .bind("sequence", &displayed.sequence.to_string())
                    .bind("base", &review.base)
                    .bind("head_commit", &review.head_commit)
                    .bind("head", &review.head)
                    .bind("command", &random_id())
                    .bind("issued_at", &now().to_string())
                    .body(html! {
                        (decision.clone().control(verdicts))
                        (text.clone().control(
                            Textarea::new("text").aria(text.aria()).maxlength(16384),
                        ))
                    })
                    .submit("Review this exact decision"),
            )
        }
    };
    let reason = gate(&context, Right::Review);
    let files = review.files.iter().fold(
        ui::table("Changed files").header(["Path", "Status", "Added", "Removed"]),
        |table, file| {
            table.row([
                html! { code { (file.path) } },
                html! { (word(&file.status)) },
                html! { "+" (file.added.map_or_else(|| "?".into(), |n| n.to_string())) },
                html! { "\u{2212}" (file.removed.map_or_else(|| "?".into(), |n| n.to_string())) },
            ])
        },
    );
    let content = html! {
        (ui::links([(agents_url(context.binding.id()).as_str(), "All agents and Studio")]))
        h2 { "Review of " code { (review.task) } }
        p {
            "Base " code { (review.base) } ", head commit " code { (review.head_commit) }
            ", tree " code { (review.head) }
            ". A decision binds these three revisions; a changed candidate requires a fresh review."
        }
        p {
            (review.files_total) " files changed (" (review.files.len()) " shown), +" (review.added)
            " \u{2212}" (review.removed) ", " (review.uncounted) " uncounted."
        }
        @if !review.files.is_empty() { (files.numeric(2).numeric(3)) }
        @match &review.completeness {
            Completeness::Complete => { p { "The diff below is the whole change." } }
            Completeness::Truncated { shown, total } => {
                p {
                    "The diff is truncated at " (shown) " bytes of "
                    (total.map_or_else(|| "an unmeasured total".into(), |t| t.to_string())) "."
                }
            }
            Completeness::Unknown { reason } => {
                (ui::unavailable("Change unreadable", html! {
                    "The change could not be read: " (reason) ". Merge is not offered for an unread change."
                }))
            }
        }
        (CodeBlock::new(&review.diff))
        @if let Some(publication) = &review.publication {
            p { "Last publication: " (word(&publication.state)) " \u{2014} " (publication.note) "." }
        }
        p { "Merge hands the reviewed tree to the host's landing path, which updates the host checkout. Merge implies no deploy; any push appears only as the host's reported publication state. The host refuses a dirty checkout, a detached head, a conflicting merge, or a changed candidate, and an uncertain result stays unknown. Agent completion and auto-start never approve a merge." }
        @if let Some(reason) = reason { p { (reason) } }
        @if let Some(form) = &form { (form) }
    };
    shell(&context, &headers, &content.into_string())
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
