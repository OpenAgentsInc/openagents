//! Project environment panel (ENV-07): draft and recipe revisions, setup
//! chat with steering, build and verification progress, saved history with
//! reviewed Promote and Select (rollback), paged retained evidence, and an
//! evidence export, all over the resident operator's native environment
//! operations.
//!
//! Reads have no side effects: they ask the native owner, which opens its
//! records read-only. Every effect is the exact reviewed native packet
//! staged through the shared request book, so a lost reply or a refresh
//! recovers the original request by its ID instead of sending another.
//! Pages pin the read they rendered; a changed record shows as stale.
//! Credentials never reach this view: the owner projects identities,
//! states, digests, and user text only.

use super::controls::{self, Context, admitted, digest, submit};
use super::operator::{bytes_response, can_operate, cloud_url, decoded, encoded, page};
use super::session::SessionError;
use super::ui::{self, BoundForm, Details};
use super::{protect, refused, workspace_shell};
use crate::App;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::{Engine, engine::general_purpose::STANDARD};
use coder_access::environment as env;
use coder_access::protocol::{Operation, Outcome, random_id};
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{Alert, Color, Variant};
use openagents_ui::forms::{Field, Textarea};
use serde::Deserialize;
use serde_json::json;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/cloud/app/hosts/{binding}/cloud/{project}/environment",
            get(panel).post(stage),
        )
        .route(
            "/cloud/app/hosts/{binding}/cloud/{project}/environment/evidence",
            get(evidence),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
}

pub(super) fn panel_url(binding: &str, project: &str) -> String {
    format!("/cloud/app/hosts/{binding}/cloud/{project}/environment")
}

/// The panel an environment request returns to, for the request page.
pub(super) fn action_url(binding: &str, action: &Operation) -> Option<String> {
    let (project, environment) = match action {
        Operation::EnvironmentPromote { intent } => (&intent.project, &intent.environment),
        Operation::EnvironmentSelect { intent } => (&intent.project, &intent.environment),
        Operation::EnvironmentSteer { intent } => (&intent.project, &intent.environment),
        _ => return None,
    };
    Some(format!(
        "{}?environment={environment}",
        panel_url(binding, project)
    ))
}

/// What a read that did not produce a view means for this panel.
enum Failed {
    State(SessionError),
    Page(Response),
}

async fn native_read(
    context: &Context<'_>,
    headers: &HeaderMap,
    operation: &Operation,
) -> Result<Outcome, Failed> {
    if operation.validate().is_err() {
        return Err(Failed::State(SessionError::InvalidRequest));
    }
    let outcome = context
        .binding
        .read(&context.viewer, operation.clone())
        .await
        .map_err(Failed::State)?;
    if outcome.validate().is_err() || !outcome.answers(operation) {
        return Err(Failed::State(SessionError::Conflict));
    }
    context.current(headers).await.map_err(Failed::Page)?;
    Ok(outcome)
}

/// The explicit state a refused read leaves the panel in.
fn state_section(error: &SessionError, retry: &str) -> Option<Markup> {
    let (title, body) = match error {
        SessionError::Forbidden => (
            "Denied",
            "The operator policy on this computer does not admit this project's environments for this device, or the environment named here is not this project's. Account sign-in alone grants no environment access.",
        ),
        SessionError::Unavailable => (
            "Unavailable",
            "The resident computer or its environment owner did not answer, or the retained record could not be read. Nothing was changed.",
        ),
        SessionError::Conflict => (
            "Stale",
            "The environment record or evidence changed while this page read it, or a page position no longer matches the retained record. Read the current record again.",
        ),
        SessionError::InvalidRequest => (
            "Invalid request",
            "This link does not name a valid environment view.",
        ),
        _ => return None,
    };
    // Every refused state is announced (`role="alert"`); a denial is
    // danger-colored, the others a warning.
    let denied = matches!(error, SessionError::Forbidden);
    let mut alert = Alert::new()
        .color(if denied {
            Color::Danger
        } else {
            Color::Warning
        })
        .variant(Variant::Soft)
        .description(body)
        .actions(html! { a href=(retry) { "Read again" } });
    if !denied {
        alert = alert.attr("role", "alert");
    }
    Some(html! {
        section class="cloud-card" aria-labelledby="environment-state" {
            h2 id="environment-state" { (title) }
            (alert)
        }
    })
}

fn state_page(context: &Context<'_>, headers: &HeaderMap, failed: Failed, retry: &str) -> Response {
    let error = match failed {
        Failed::Page(response) => return response,
        Failed::State(error) => error,
    };
    let Some(section) = state_section(&error, retry) else {
        return refused(error);
    };
    let content = html! {
        (section)
        (PreEscaped(controls::link(context.binding)))
    }
    .into_string();
    let status = match error {
        SessionError::Forbidden => StatusCode::FORBIDDEN,
        SessionError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        SessionError::Conflict => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    let mut response = workspace_shell(
        context.app,
        headers,
        context.service,
        &context.viewer,
        "projects",
        Some(&content),
        None,
    );
    if response.status().is_success() {
        *response.status_mut() = status;
    }
    response
}

/// UTC wall time for a retained millisecond timestamp.
pub(super) fn when(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The first 12 characters of a digest (plain text; Maud escapes it).
fn short(digest: &str) -> &str {
    digest.get(..digest.len().min(12)).unwrap_or(digest)
}

fn human(state: &str) -> String {
    state.replace('_', " ")
}

/// One summary line for the whole environment, with the state named. The
/// line is plain text; the caller escapes it.
pub(super) fn overall(d: &env::Detail) -> (&'static str, String) {
    let reconciling = d
        .builds
        .iter()
        .any(|b| b.unresolved.is_some() || b.state == "needs_reconciliation")
        || d.verifications
            .iter()
            .any(|v| v.unresolved.is_some() || v.state == "needs_reconciliation");
    if d.retired {
        return (
            "retired",
            "Retired. New drafts, builds, saves, and selections are refused; saved history stays readable.".into(),
        );
    }
    if reconciling {
        return (
            "reconciling",
            "Reconciling. The owner could not observe one outcome; it stays unknown here until a definite observation settles it, and dependent steps wait.".into(),
        );
    }
    let latest_build = d.builds.first();
    let latest_verify = d.verifications.first();
    match latest_verify.map(|v| v.state.as_str()) {
        Some("failed") | Some("incomplete") => {
            return (
                "failed",
                format!(
                    "Failed. Verification {} {}. A failed or incomplete verification can never be saved.",
                    latest_verify.map(|v| v.id.clone()).unwrap_or_default(),
                    human(latest_verify.map_or("", |v| v.state.as_str()))
                ),
            );
        }
        Some("cancelled") => {
            return (
                "cancelled",
                "Cancelled. The latest verification was cancelled; cleanup and usage remain their own retained facts.".into(),
            );
        }
        Some("requested" | "restoring" | "checking") => {
            return (
                "verifying",
                "Verifying on a fresh computer. Leaving this page does not stop it.".into(),
            );
        }
        _ => {}
    }
    match latest_build.map(|b| b.state.as_str()) {
        Some("failed") => {
            return (
                "failed",
                "Failed. The latest build failed; edit the recipe or rebuild.".into(),
            );
        }
        Some("cancelled") => {
            return (
                "cancelled",
                "Cancelled. The latest build was cancelled.".into(),
            );
        }
        Some(
            "requested" | "provisioning" | "installing" | "preparing_image" | "snapshot_pending",
        ) => {
            return (
                "building",
                "Building on a fresh builder computer. Leaving this page does not stop it.".into(),
            );
        }
        _ => {}
    }
    if latest_build.is_some_and(|b| b.stale) {
        return (
            "stale",
            format!(
                "Stale. The draft is at recipe revision {}; earlier builds cannot be verified or saved.",
                d.draft_revision
            ),
        );
    }
    if d.verifications.iter().any(|v| v.candidate.is_some()) {
        return (
            "review",
            "Ready for review. A passed verification with complete evidence can be saved.".into(),
        );
    }
    match &d.active {
        Some(active) => (
            "selected",
            format!("Version {active} is selected; new jobs for this project start from it."),
        ),
        None => (
            "draft",
            "Draft. No version is saved or selected yet.".into(),
        ),
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelOptions {
    environment: Option<String>,
    before: Option<u64>,
}

fn steps(steps: &[env::Step]) -> Markup {
    html! {
        ol class="environment-steps" {
            @for s in steps {
                li {
                    (human(&s.state)) " · " time { (when(s.at_ms)) }
                    @if let Some(r) = &s.reason { " · " (r) }
                }
            }
        }
    }
}

fn image(i: &env::Image) -> Markup {
    html! {
        "image " code { (i.image_id) }
        @if let Some(s) = &i.snapshot_id { " · snapshot " code { (s) } }
        " · manifest " code { (short(&i.manifest_digest)) }
    }
}

struct Forms<'a> {
    context: &'a Context<'a>,
    headers: &'a HeaderMap,
    project: &'a str,
    environment: &'a str,
    enabled: bool,
}
impl Forms<'_> {
    /// One reviewed effect form: a fresh request ID and a CSRF token bound
    /// to exactly the displayed values it submits.
    fn form(
        &self,
        action: &str,
        target: &str,
        fence: &str,
        label: &str,
        enabled: bool,
        field: Option<Markup>,
    ) -> Result<Markup, Response> {
        let request = random_id();
        let basis = basis(&request, action, self.environment, target, fence);
        let csrf = self
            .context
            .csrf(self.headers, "environment-action", &basis)?;
        let button = submit(
            &format!("environment-{action}"),
            label,
            self.enabled && enabled,
        )?;
        let mut form = BoundForm::new(panel_url(self.context.binding.id(), self.project))
            .csrf(&csrf)
            .bind("request", &request)
            .bind("action", action)
            .bind("environment", self.environment)
            .bind("target", target)
            .bind("fence", fence)
            .bind("basis", &basis)
            .submit_with(PreEscaped(button));
        if let Some(field) = field {
            form = form.body(field);
        }
        Ok(form.render())
    }
}

fn basis(request: &str, action: &str, environment: &str, target: &str, fence: &str) -> String {
    digest(&json!({
        "request": request,
        "action": action,
        "environment": environment,
        "target": target,
        "fence": fence,
    }))
}

/// A job link (` · run <job>`) when the record names one.
fn job_link(cloud: &str, job: Option<&str>) -> Markup {
    html! {
        @if let Some(j) = job {
            " · " a href=(format!("{cloud}/jobs/{j}")) { "run " (j) }
        }
    }
}

fn render_detail(
    context: &Context<'_>,
    headers: &HeaderMap,
    project: &str,
    d: &env::Detail,
) -> Result<Markup, Response> {
    let binding = context.binding.id();
    let base = panel_url(binding, project);
    let cloud = cloud_url(context, project);
    let enabled = can_operate(context) && !d.retired;
    let forms = Forms {
        context,
        headers,
        project,
        environment: &d.id,
        enabled,
    };
    let (kind, status) = overall(d);

    // Forms and links that need a fallible step, prepared before the markup.
    let mut steer = Vec::new();
    for s in d.setup.iter().flatten() {
        steer.push(if s.steerable {
            let id = format!("steer-{}", s.id);
            let field = Field::new(id.as_str(), "Steer this setup").required(true);
            let control = Textarea::new("text")
                .rows(3)
                .maxlength(u32::try_from(env::MAX_STEER_BYTES).unwrap_or(u32::MAX))
                .aria(field.aria());
            Some(forms.form(
                "steer",
                &s.id,
                &s.updated_ms.to_string(),
                "Review steering",
                true,
                Some(field.control(control).render()),
            )?)
        } else {
            None
        });
    }
    let mut verify = Vec::new();
    for v in &d.verifications {
        let evidence = if v.evidence_readable {
            Some(encoded(&evidence_query(
                context, project, &d.id, &v.id, None, None, None,
            ))?)
        } else {
            None
        };
        let promote = match &v.candidate {
            Some(c) => Some(forms.form(
                "promote",
                &v.id,
                &format!("{}:{}", d.selection_revision, c.digest),
                "Review Save and select this version",
                true,
                None,
            )?),
            None => None,
        };
        verify.push((evidence, promote));
    }
    let selected_number = d.history.iter().find(|x| x.selected).map(|x| x.number);
    let mut select = Vec::new();
    for h in &d.history {
        select.push(if h.selected {
            None
        } else {
            // An active version missing from this page is newer than it.
            let rollback = selected_number.map_or(d.active.is_some(), |n| h.number < n);
            let label = if rollback {
                "Review rollback to this version"
            } else {
                "Review selecting this version"
            };
            Some(forms.form(
                "select",
                &h.id,
                &d.selection_revision.to_string(),
                label,
                true,
                None,
            )?)
        });
    }

    let source = html! {
        @if let Some(r) = &d.source.repository { (r) " at " }
        code { (d.source.revision) } " · digest " code { (short(&d.source.digest)) }
    };
    let draft = Details::new()
        .row(
            "Environment",
            html! { code { (d.id) } " · record revision " (d.revision) },
        )
        .row("Source", source)
        .row(
            "Draft",
            html! { "Recipe revision " (d.draft_revision) " · " code { (short(&d.draft_digest)) } },
        );

    Ok(html! {
        p class="environment-status" role="status" aria-live="polite" data-state=(kind) { (status) }
        @if !enabled {
            p { "Promote, Select, and steering need current browser enrollment and native Operate authority on this computer; this page stays read-only." }
        }

        // Draft and recipe revisions.
        (ui::section("environment-draft", "Draft and recipe revisions", html! {
            (draft)
            ol reversed {
                @for r in &d.recipes {
                    li {
                        "Revision " (r.revision) " · " code { (short(&r.digest)) }
                        " · " time { (when(r.created_ms)) }
                        @if r.revision == d.draft_revision { " · current draft" }
                    }
                }
            }
        }))

        // Setup chat.
        (ui::section("environment-setup", "Setup sessions", html! {
            @match &d.setup {
                None => { p { "Unavailable. No setup owner is composed with this computer's operator, so setup sessions cannot be read or steered here. Builds, verification, and saved history below come from the environment record itself." } }
                Some(rows) if rows.is_empty() => { p { "No setup session has run for this environment yet." } }
                Some(rows) => {
                    @for (s, form) in rows.iter().zip(&steer) {
                        article class="cloud-card" aria-labelledby=(format!("setup-{}", s.id)) {
                            h4 id=(format!("setup-{}", s.id)) { "Session " code { (s.id) } " · " (human(&s.state)) }
                            p { "Objective: " (s.objective) }
                            @if let Some(q) = &s.question {
                                p role="note" { strong { "Waiting for your input:" } " " (q) }
                            }
                            @if let Some(r) = &s.reason { p { "Reason: " (r) } }
                            h5 { "Steering" }
                            ol class="environment-chat" {
                                @for t in &s.steering {
                                    li { time { (when(t.at_ms)) } " · " (t.text) }
                                }
                                @if s.steering.is_empty() { li { "No steering yet." } }
                            }
                            h5 { "Commands" }
                            ul {
                                @for c in &s.commands {
                                    li { code { (c.id) } " · " (human(&c.purpose)) " · " (human(&c.state)) }
                                }
                                @if s.commands.is_empty() { li { "No commands yet." } }
                            }
                            @if let Some(form) = form {
                                (form)
                            } @else {
                                p { "This session has ended; it takes no more steering." }
                            }
                        }
                    }
                }
            }
        }))

        // Builds.
        (ui::section("environment-builds", "Builds", html! {
            @if d.builds.is_empty() {
                p { "No build yet. A build rebuilds the pinned recipe on a fresh builder computer." }
            }
            @for b in &d.builds {
                article class="cloud-card" aria-labelledby=(format!("build-{}", b.id)) {
                    h4 id=(format!("build-{}", b.id)) {
                        "Build " code { (b.id) } " · " (human(&b.state))
                        @if b.stale { " · stale" }
                    }
                    p {
                        "Recipe revision " (b.recipe_revision) " · " code { (short(&b.recipe_digest)) }
                        (job_link(&cloud, b.job.as_deref()))
                    }
                    @if let Some(u) = &b.unresolved { p role="note" { "Needs reconciliation: " (u) } }
                    @if let Some(i) = &b.image { p { "Output " (image(i)) } }
                    (steps(&b.steps))
                }
            }
        }))

        // Verification.
        (ui::section("environment-verify", "Verification", html! {
            @if d.verifications.is_empty() {
                p { "No verification yet. A verifier boots the sealed image on a different fresh computer." }
            }
            @for (v, (evidence, promote)) in d.verifications.iter().zip(&verify) {
                article class="cloud-card" aria-labelledby=(format!("verify-{}", v.id)) {
                    h4 id=(format!("verify-{}", v.id)) { "Verification " code { (v.id) } " · " (human(&v.state)) }
                    p {
                        "Build " code { (v.build_id) }
                        (job_link(&cloud, v.job.as_deref()))
                        " · evidence "
                        (v.evidence_status.as_deref().map(human).unwrap_or_else(|| "not sealed".into()))
                        @if let Some(e) = &v.evidence_digest { " · " code { (short(e)) } }
                    }
                    @if let Some(u) = &v.unresolved { p role="note" { "Needs reconciliation: " (u) } }
                    @if let Some(q) = evidence {
                        p { a href=(format!("{base}/evidence?q={q}")) { "Page the retained evidence" } }
                    } @else {
                        p { "Retained evidence is not readable on this computer." }
                    }
                    (steps(&v.steps))
                    @if let Some(c) = &v.candidate {
                        h5 { "Candidate for review" }
                        (Details::new()
                            .row("Candidate", html! { code { (short(&c.digest)) } })
                            .row("Recipe", html! { "Revision " (c.recipe_revision) " · " code { (short(&c.recipe_digest)) } })
                            .row("Source", html! { code { (c.source_revision) } })
                            .row("Output", image(&c.image))
                            .row("Plan", html! { code { (short(&c.plan_digest)) } })
                            .row("Evidence", html! { code { (short(&c.evidence_digest)) } }))
                        p { "Save and select records exactly this candidate. If anything above changes before the owner applies it, the owner refuses the review as stale." }
                        @if let Some(form) = promote { (form) }
                    }
                }
            }
        }))

        // Saved history.
        (ui::section("environment-history", "Saved versions", html! {
            p {
                "Selection revision " (d.selection_revision) " · "
                @if let Some(a) = &d.active { "selected " code { (a) } } @else { "nothing selected" }
            }
            @if d.history.is_empty() { p { "No saved version yet." } }
            ol class="environment-history" {
                @for (h, form) in d.history.iter().zip(&select) {
                    li aria-current=(if h.selected { "true" } else { "false" }) {
                        strong { "Version " (h.number) " · " code { (h.id) } }
                        @if h.selected { " · selected" }
                        " · " time { (when(h.created_ms)) }
                        " · recipe revision " (h.recipe_revision)
                        " · source " code { (h.source_revision) }
                        " · " (image(&h.image))
                        " · evidence " code { (short(&h.evidence_digest)) }
                        @if let Some(r) = &h.reviewer { " · reviewed by " code { (short(r)) } }
                        @if let Some(form) = form { (form) }
                    }
                }
            }
            @if let Some(before) = d.history_before {
                p { a href=(format!("{base}?environment={}&before={before}", d.id)) { "Older saved versions" } }
            }
            @if !d.changes.is_empty() {
                h4 { "Selection changes" }
                ol reversed {
                    @for c in &d.changes {
                        li {
                            "Revision " (c.revision) " · " (human(&c.kind)) " " code { (c.version_id) }
                            @if let Some(p) = &c.previous { " (was " code { (p) } ")" }
                            " · " time { (when(c.at_ms)) }
                        }
                    }
                }
            }
            p { "Selecting or rolling back changes only jobs admitted afterwards; running, queued, and continued jobs keep the version they started with. No saved version is ever rewritten." }
        }))

        // Optional terminal.
        (ui::section("environment-terminal", "Terminal", html! {
            p {
                "A terminal on a setup or builder computer is a separately granted native workbench session; this page grants none. "
                a href=(format!("/cloud/app/hosts/{binding}/workbench")) { "Open the native workbench" }
                " to enroll and attach where your host grant allows it."
            }
        }))
    })
}

#[allow(clippy::too_many_arguments)]
fn evidence_query(
    context: &Context<'_>,
    project: &str,
    environment: &str,
    verification: &str,
    child: Option<&str>,
    call: Option<(&str, env::Stream)>,
    cursor: Option<env::EvidenceCursor>,
) -> env::EvidenceQuery {
    env::EvidenceQuery {
        workspace: context.binding.workspace().into(),
        project: project.into(),
        environment: environment.into(),
        verification: verification.into(),
        child: child.map(str::to_owned),
        call: call.map(|c| c.0.to_owned()),
        stream: call.map(|c| c.1),
        cursor,
        limit: env::MAX_CHUNK_BYTES,
    }
}

async fn panel(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    options: Result<Query<PanelOptions>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(options)) = options else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = Operation::EnvironmentRead {
        query: env::Query {
            workspace: context.binding.workspace().into(),
            project: project.clone(),
            environment: options.environment.clone(),
            before: options.before,
        },
    };
    let base = panel_url(context.binding.id(), &project);
    let outcome = match native_read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(failed) => return state_page(&context, &headers, failed, &base),
    };
    let Outcome::EnvironmentRead { view } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let detail = match &view.detail {
        None => None,
        Some(detail) => match render_detail(&context, &headers, &project, detail) {
            Ok(v) => Some(v),
            Err(r) => return r,
        },
    };
    let content = html! {
        h2 { "Project environment · " (project) }
        p { "Reading this page changes nothing and never drives a builder or verifier. Leaving it detaches; work keeps running on its own computers." }
        @if view.environments.len() > 1 {
            nav aria-label="Environments" {
                ul {
                    @for e in &view.environments {
                        @let current = view.detail.as_ref().is_some_and(|d| d.id == e.id);
                        li {
                            a href=(format!("{base}?environment={}", e.id)) aria-current=[current.then_some("page")] { (e.id) }
                            @if e.retired { " · retired" }
                        }
                    }
                }
            }
        }
        @if let Some(detail) = &detail {
            (detail)
        } @else {
            (ui::card(ui::section(
                "environment-empty",
                "No environment yet",
                html! { p { "This project has no repository environment. Jobs start from their admitted profile's runtime until a reviewed version is saved and selected here." } },
            )))
        }
        p { a href=(cloud_url(&context, &project)) { "Operator Cloud jobs for this project" } }
        (PreEscaped(controls::link(context.binding)))
    }
    .into_string();
    page(
        &context,
        &headers,
        &content,
        &operation,
        &outcome,
        context.book().is_ok(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionForm {
    csrf: String,
    request: String,
    action: String,
    environment: String,
    target: String,
    fence: String,
    basis: String,
    text: Option<String>,
}

async fn stage(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    form: Result<Form<ActionForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let expected = basis(
        &form.request,
        &form.action,
        &form.environment,
        &form.target,
        &form.fence,
    );
    if expected != form.basis {
        return refused(SessionError::Conflict);
    }
    if let Err(r) = context.verify(&headers, &form.csrf, "environment-action", &expected) {
        return r;
    }
    let workspace = context.binding.workspace().to_owned();
    let operation = match form.action.as_str() {
        "promote" if form.text.is_none() => {
            let Some((revision, candidate)) = form.fence.split_once(':') else {
                return refused(SessionError::InvalidRequest);
            };
            let Ok(revision) = revision.parse() else {
                return refused(SessionError::InvalidRequest);
            };
            Operation::EnvironmentPromote {
                intent: env::Promote {
                    workspace,
                    project,
                    environment: form.environment,
                    verification: form.target,
                    candidate_digest: candidate.into(),
                    expected_selection_revision: revision,
                },
            }
        }
        "select" if form.text.is_none() => {
            let Ok(revision) = form.fence.parse() else {
                return refused(SessionError::InvalidRequest);
            };
            Operation::EnvironmentSelect {
                intent: env::Select {
                    workspace,
                    project,
                    environment: form.environment,
                    version: form.target,
                    expected_selection_revision: revision,
                },
            }
        }
        "steer" => Operation::EnvironmentSteer {
            intent: env::Steer {
                workspace,
                project,
                environment: form.environment,
                session: form.target,
                text: form.text.unwrap_or_default(),
            },
        },
        _ => return refused(SessionError::InvalidRequest),
    };
    controls::staged(&context, &headers, &form.request, operation).await
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceOptions {
    q: Option<String>,
    download: Option<String>,
}

fn json_download(bytes: Vec<u8>, name: &'static str) -> Response {
    let mut response = protect(bytes.into_response());
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static(name));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn evidence(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    options: Result<Query<EvidenceOptions>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(options)) = options else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(q) = options.q else {
        return refused(SessionError::InvalidRequest);
    };
    let query: env::EvidenceQuery = match decoded(&q) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if query.workspace != context.binding.workspace() || query.project != project {
        return refused(SessionError::InvalidRequest);
    }
    let base = panel_url(context.binding.id(), &project);
    let retry = format!("{base}/evidence?q={q}");
    let operation = Operation::EnvironmentEvidence {
        query: query.clone(),
    };
    let outcome = match native_read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(failed) => return state_page(&context, &headers, failed, &retry),
    };
    let Outcome::EnvironmentEvidence { page: p } = &outcome else {
        return refused(SessionError::Conflict);
    };
    match options.download.as_deref() {
        None => {}
        Some("yes") => {
            let Some(chunk) = &p.chunk else {
                return refused(SessionError::InvalidRequest);
            };
            return match STANDARD.decode(&chunk.data) {
                Ok(bytes) => bytes_response(bytes),
                Err(_) => refused(SessionError::Conflict),
            };
        }
        Some("export") if p.chunk.is_none() => {
            let export = json!({
                "schema": "openagents.environment.evidence_panel_export.v1",
                "summary": p,
                "note": "Coverage, sealed manifest digest, and every disclosed gap of this record. Download each stream's original bytes page by page; each page is checked against its recorded digest before it is served.",
            });
            return json_download(
                serde_json::to_vec_pretty(&export).unwrap_or_default(),
                "attachment; filename=environment-evidence.json",
            );
        }
        Some(_) => return refused(SessionError::InvalidRequest),
    }
    let summary_q = match encoded(&evidence_query(
        &context,
        &project,
        &p.environment,
        &p.verification,
        p.child.as_deref(),
        None,
        None,
    )) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut children = Vec::with_capacity(p.children.len());
    for c in &p.children {
        match encoded(&evidence_query(
            &context,
            &project,
            &p.environment,
            &p.verification,
            Some(c),
            None,
            None,
        )) {
            Ok(q) => children.push((c, q)),
            Err(r) => return r,
        }
    }
    let mut calls = Vec::with_capacity(p.calls.len());
    for c in &p.calls {
        let mut links = Vec::with_capacity(2);
        for (stream, label, bytes) in [
            (env::Stream::Stdout, "stdout", c.stdout_bytes),
            (env::Stream::Stderr, "stderr", c.stderr_bytes),
        ] {
            match encoded(&evidence_query(
                &context,
                &project,
                &p.environment,
                &p.verification,
                p.child.as_deref(),
                Some((&c.call, stream)),
                None,
            )) {
                Ok(q) => links.push((q, label, bytes)),
                Err(r) => return r,
            }
        }
        calls.push((c, links));
    }
    let chunk = match &p.chunk {
        None => None,
        Some(chunk) => {
            let bytes = match STANDARD.decode(&chunk.data) {
                Ok(v) => v,
                Err(_) => return refused(SessionError::Conflict),
            };
            let display = std::str::from_utf8(&bytes).map_or_else(
                |_| format!("Base64: {}", STANDARD.encode(&bytes)),
                str::to_owned,
            );
            let next = match &chunk.next {
                Some(next) => {
                    let mut q = query.clone();
                    q.cursor = Some(next.clone());
                    match encoded(&q) {
                        Ok(v) => Some(v),
                        Err(r) => return r,
                    }
                }
                None => None,
            };
            Some((chunk, bytes.len(), display, next))
        }
    };
    let content = html! {
        h2 {
            "Retained evidence · verification " code { (p.verification) }
            @if let Some(c) = &p.child { " · machine record " code { (c) } }
        }
        p role="status" {
            "Record " code { (p.evidence_id) } " · " (human(&p.status)) " · "
            (if p.complete { "complete" } else { "not complete" })
            " · " (p.head_seq) " events"
            @if let Some(d) = &p.sealed_digest { " · sealed " code { (short(d)) } }
        }
        p {
            a href=(format!("{base}?environment={}", p.environment)) { "Back to the environment" }
            " · "
            a href=(format!("{base}/evidence?q={summary_q}&download=export")) { "Download the evidence export (JSON)" }
        }
        (ui::section("evidence-gaps", "Disclosed gaps", html! {
            ul {
                @for g in &p.gaps { li { code { (g) } } }
                @if p.gaps.is_empty() { li { "None disclosed." } }
                @if p.gaps_omitted { li { "More gaps exist than this page shows; the export lists those this page carries." } }
            }
        }))
        @if !children.is_empty() {
            (ui::section("evidence-children", "Machine records", html! {
                ul {
                    @for (c, q) in &children {
                        li { a href=(format!("{base}/evidence?q={q}")) { (c) } }
                    }
                }
            }))
        }
        (ui::section("evidence-calls", "Calls", html! {
            ul {
                @for (c, links) in &calls {
                    li {
                        code { (c.call) } " · " (c.tool) " · " (human(&c.outcome))
                        @for (q, label, bytes) in links {
                            " · " a href=(format!("{base}/evidence?q={q}")) { (label) " (" (bytes) " bytes)" }
                        }
                    }
                }
                @if p.calls.is_empty() { li { "No calls recorded." } }
                @if p.calls_omitted { li { "More calls exist than this page lists." } }
            }
        }))
        @if let Some((chunk, len, display, next)) = &chunk {
            (ui::section(
                "evidence-bytes",
                html! {
                    "Original bytes · " code { (chunk.call) } " "
                    (match chunk.stream {
                        env::Stream::Stdout => "stdout",
                        env::Stream::Stderr => "stderr",
                    })
                },
                html! {
                    p {
                        "Byte offset " (chunk.start) " · page " (len) " bytes · " (chunk.length) " bytes retained so far"
                        (if chunk.closed { " · stream ended" } else { " · stream open" })
                        " · " code { (short(&chunk.digest)) }
                        ". Pages carry the original retained bytes and may split a UTF-8 character; download a page to keep its exact bytes."
                    }
                    p { a href=(format!("{retry}&download=yes")) { "Download these exact bytes" } }
                    pre tabindex="0" { (display) }
                    @if let Some(q) = next {
                        p { a href=(format!("{base}/evidence?q={q}")) { "Next original page" } }
                    } @else if !chunk.closed {
                        p {
                            "The stream is still open. "
                            a href=(retry) { "Read this page again" }
                            " to continue once more bytes are retained."
                        }
                    }
                },
            ))
        }
        (PreEscaped(controls::link(context.binding)))
    }
    .into_string();
    page(&context, &headers, &content, &operation, &outcome, false)
}
