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

use super::controls::{self, Context, admitted, digest, hidden, submit};
use super::operator::{bytes_response, can_operate, cloud_url, decoded, encoded, page};
use super::session::SessionError;
use super::{protect, refused, ticket, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::{Engine, engine::general_purpose::STANDARD};
use coder_access::environment as env;
use coder_access::protocol::{Operation, Outcome, random_id};
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
fn state_section(error: &SessionError, retry: &str) -> Option<String> {
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
    Some(format!(
        "<section class=\"cloud-card\" role=\"alert\" aria-labelledby=\"environment-state\"><h2 id=\"environment-state\">{title}</h2><p>{body}</p><p><a href=\"{}\">Read again</a></p></section>",
        escape(retry)
    ))
}

fn state_page(context: &Context<'_>, headers: &HeaderMap, failed: Failed, retry: &str) -> Response {
    let error = match failed {
        Failed::Page(response) => return response,
        Failed::State(error) => error,
    };
    let Some(mut content) = state_section(&error, retry) else {
        return refused(error);
    };
    content.push_str(&controls::link(context.binding));
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

fn short(digest: &str) -> String {
    escape(&digest[..digest.len().min(12)])
}

fn human(state: &str) -> String {
    escape(&state.replace('_', " "))
}

/// One summary line for the whole environment, with the state named.
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
                    escape(&latest_verify.map(|v| v.id.clone()).unwrap_or_default()),
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
            format!(
                "Version {} is selected; new jobs for this project start from it.",
                escape(active)
            ),
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

fn steps(steps: &[env::Step]) -> String {
    let mut out = String::from("<ol class=\"environment-steps\">");
    for s in steps {
        out.push_str(&format!(
            "<li>{} · <time>{}</time>{}</li>",
            human(&s.state),
            when(s.at_ms),
            s.reason
                .as_deref()
                .map(|r| format!(" · {}", escape(r)))
                .unwrap_or_default()
        ));
    }
    out.push_str("</ol>");
    out
}

fn image(i: &env::Image) -> String {
    format!(
        "image <code>{}</code>{} · manifest <code>{}</code>",
        escape(&i.image_id),
        i.snapshot_id
            .as_deref()
            .map(|s| format!(" · snapshot <code>{}</code>", escape(s)))
            .unwrap_or_default(),
        short(&i.manifest_digest)
    )
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
        field: &str,
    ) -> Result<String, Response> {
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
        Ok(format!(
            "<form method=\"post\" action=\"{}\">{}{}{}{}{}{}{}{field}{button}</form>",
            escape(&panel_url(self.context.binding.id(), self.project)),
            ticket(&csrf),
            hidden("request", &request),
            hidden("action", action),
            hidden("environment", self.environment),
            hidden("target", target),
            hidden("fence", fence),
            hidden("basis", &basis),
        ))
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

fn render_detail(
    context: &Context<'_>,
    headers: &HeaderMap,
    project: &str,
    d: &env::Detail,
) -> Result<String, Response> {
    let binding = context.binding.id();
    let base = panel_url(binding, project);
    let enabled = can_operate(context) && !d.retired;
    let forms = Forms {
        context,
        headers,
        project,
        environment: &d.id,
        enabled,
    };
    let (kind, status) = overall(d);
    let mut out = format!(
        "<p class=\"environment-status\" role=\"status\" aria-live=\"polite\" data-state=\"{kind}\">{status}</p>"
    );
    if !enabled {
        out.push_str("<p>Promote, Select, and steering need current browser enrollment and native Operate authority on this computer; this page stays read-only.</p>");
    }

    // Draft and recipe revisions.
    out.push_str(&format!(
        "<section aria-labelledby=\"environment-draft\"><h3 id=\"environment-draft\">Draft and recipe revisions</h3><dl><dt>Environment</dt><dd><code>{}</code> · record revision {}</dd><dt>Source</dt><dd>{}<code>{}</code> · digest <code>{}</code></dd><dt>Draft</dt><dd>Recipe revision {} · <code>{}</code></dd></dl><ol reversed>",
        escape(&d.id),
        d.revision,
        d.source
            .repository
            .as_deref()
            .map(|r| format!("{} at ", escape(r)))
            .unwrap_or_default(),
        escape(&d.source.revision),
        short(&d.source.digest),
        d.draft_revision,
        short(&d.draft_digest),
    ));
    for r in &d.recipes {
        out.push_str(&format!(
            "<li>Revision {} · <code>{}</code> · <time>{}</time>{}</li>",
            r.revision,
            short(&r.digest),
            when(r.created_ms),
            if r.revision == d.draft_revision {
                " · current draft"
            } else {
                ""
            }
        ));
    }
    out.push_str("</ol></section>");

    // Setup chat.
    out.push_str("<section aria-labelledby=\"environment-setup\"><h3 id=\"environment-setup\">Setup sessions</h3>");
    match &d.setup {
        None => out.push_str("<p>Unavailable. No setup owner is composed with this computer's operator, so setup sessions cannot be read or steered here. Builds, verification, and saved history below come from the environment record itself.</p>"),
        Some(rows) if rows.is_empty() => out.push_str("<p>No setup session has run for this environment yet.</p>"),
        Some(rows) => {
            for s in rows {
                out.push_str(&format!(
                    "<article class=\"cloud-card\" aria-labelledby=\"setup-{id}\"><h4 id=\"setup-{id}\">Session <code>{id}</code> · {}</h4><p>Objective: {}</p>",
                    human(&s.state),
                    escape(&s.objective),
                    id = escape(&s.id),
                ));
                if let Some(q) = &s.question {
                    out.push_str(&format!("<p role=\"note\"><strong>Waiting for your input:</strong> {}</p>", escape(q)));
                }
                if let Some(r) = &s.reason {
                    out.push_str(&format!("<p>Reason: {}</p>", escape(r)));
                }
                out.push_str("<h5>Steering</h5><ol class=\"environment-chat\">");
                for t in &s.steering {
                    out.push_str(&format!(
                        "<li><time>{}</time> · {}</li>",
                        when(t.at_ms),
                        escape(&t.text)
                    ));
                }
                if s.steering.is_empty() {
                    out.push_str("<li>No steering yet.</li>");
                }
                out.push_str("</ol><h5>Commands</h5><ul>");
                for c in &s.commands {
                    out.push_str(&format!(
                        "<li><code>{}</code> · {} · {}</li>",
                        escape(&c.id),
                        human(&c.purpose),
                        human(&c.state)
                    ));
                }
                if s.commands.is_empty() {
                    out.push_str("<li>No commands yet.</li>");
                }
                out.push_str("</ul>");
                if s.steerable {
                    let field = format!(
                        "<label for=\"steer-{id}\">Steer this setup</label><textarea id=\"steer-{id}\" name=\"text\" maxlength=\"{}\" rows=\"3\" required></textarea>",
                        env::MAX_STEER_BYTES,
                        id = escape(&s.id)
                    );
                    out.push_str(&forms.form("steer", &s.id, &s.updated_ms.to_string(), "Review steering", true, &field)?);
                } else {
                    out.push_str("<p>This session has ended; it takes no more steering.</p>");
                }
                out.push_str("</article>");
            }
        }
    }
    out.push_str("</section>");

    // Builds.
    out.push_str(
        "<section aria-labelledby=\"environment-builds\"><h3 id=\"environment-builds\">Builds</h3>",
    );
    if d.builds.is_empty() {
        out.push_str(
            "<p>No build yet. A build rebuilds the pinned recipe on a fresh builder computer.</p>",
        );
    }
    for b in &d.builds {
        out.push_str(&format!(
            "<article class=\"cloud-card\" aria-labelledby=\"build-{id}\"><h4 id=\"build-{id}\">Build <code>{id}</code> · {}{}</h4><p>Recipe revision {} · <code>{}</code>{}</p>",
            human(&b.state),
            if b.stale { " · stale" } else { "" },
            b.recipe_revision,
            short(&b.recipe_digest),
            b.job
                .as_deref()
                .map(|j| format!(" · <a href=\"{}/jobs/{}\">run {}</a>", escape(&cloud_url(context, project)), escape(j), escape(j)))
                .unwrap_or_default(),
            id = escape(&b.id),
        ));
        if let Some(u) = &b.unresolved {
            out.push_str(&format!(
                "<p role=\"note\">Needs reconciliation: {}</p>",
                escape(u)
            ));
        }
        if let Some(i) = &b.image {
            out.push_str(&format!("<p>Output {}</p>", image(i)));
        }
        out.push_str(&steps(&b.steps));
        out.push_str("</article>");
    }
    out.push_str("</section>");

    // Verification.
    out.push_str("<section aria-labelledby=\"environment-verify\"><h3 id=\"environment-verify\">Verification</h3>");
    if d.verifications.is_empty() {
        out.push_str("<p>No verification yet. A verifier boots the sealed image on a different fresh computer.</p>");
    }
    for v in &d.verifications {
        out.push_str(&format!(
            "<article class=\"cloud-card\" aria-labelledby=\"verify-{id}\"><h4 id=\"verify-{id}\">Verification <code>{id}</code> · {}</h4><p>Build <code>{}</code>{} · evidence {}{}</p>",
            human(&v.state),
            escape(&v.build_id),
            v.job
                .as_deref()
                .map(|j| format!(" · <a href=\"{}/jobs/{}\">run {}</a>", escape(&cloud_url(context, project)), escape(j), escape(j)))
                .unwrap_or_default(),
            v.evidence_status.as_deref().map(human).unwrap_or_else(|| "not sealed".into()),
            v.evidence_digest
                .as_deref()
                .map(|e| format!(" · <code>{}</code>", short(e)))
                .unwrap_or_default(),
            id = escape(&v.id),
        ));
        if let Some(u) = &v.unresolved {
            out.push_str(&format!(
                "<p role=\"note\">Needs reconciliation: {}</p>",
                escape(u)
            ));
        }
        if v.evidence_readable {
            let q = encoded(&evidence_query(
                context, project, &d.id, &v.id, None, None, None,
            ))?;
            out.push_str(&format!(
                "<p><a href=\"{}/evidence?q={q}\">Page the retained evidence</a></p>",
                escape(&base)
            ));
        } else {
            out.push_str("<p>Retained evidence is not readable on this computer.</p>");
        }
        out.push_str(&steps(&v.steps));
        if let Some(c) = &v.candidate {
            out.push_str(&format!(
                "<h5>Candidate for review</h5><dl><dt>Candidate</dt><dd><code>{}</code></dd><dt>Recipe</dt><dd>Revision {} · <code>{}</code></dd><dt>Source</dt><dd><code>{}</code></dd><dt>Output</dt><dd>{}</dd><dt>Plan</dt><dd><code>{}</code></dd><dt>Evidence</dt><dd><code>{}</code></dd></dl><p>Save and select records exactly this candidate. If anything above changes before the owner applies it, the owner refuses the review as stale.</p>",
                short(&c.digest),
                c.recipe_revision,
                short(&c.recipe_digest),
                escape(&c.source_revision),
                image(&c.image),
                short(&c.plan_digest),
                short(&c.evidence_digest),
            ));
            out.push_str(&forms.form(
                "promote",
                &v.id,
                &format!("{}:{}", d.selection_revision, c.digest),
                "Review Save and select this version",
                true,
                "",
            )?);
        }
        out.push_str("</article>");
    }
    out.push_str("</section>");

    // Saved history.
    out.push_str("<section aria-labelledby=\"environment-history\"><h3 id=\"environment-history\">Saved versions</h3>");
    out.push_str(&format!(
        "<p>Selection revision {} · {}</p>",
        d.selection_revision,
        d.active
            .as_deref()
            .map(|a| format!("selected <code>{}</code>", escape(a)))
            .unwrap_or_else(|| "nothing selected".into())
    ));
    if d.history.is_empty() {
        out.push_str("<p>No saved version yet.</p>");
    }
    out.push_str("<ol class=\"environment-history\">");
    let selected_number = d.history.iter().find(|x| x.selected).map(|x| x.number);
    for h in &d.history {
        out.push_str(&format!(
            "<li aria-current=\"{}\"><strong>Version {} · <code>{}</code></strong>{} · <time>{}</time> · recipe revision {} · source <code>{}</code> · {} · evidence <code>{}</code>{}",
            if h.selected { "true" } else { "false" },
            h.number,
            escape(&h.id),
            if h.selected { " · selected" } else { "" },
            when(h.created_ms),
            h.recipe_revision,
            escape(&h.source_revision),
            image(&h.image),
            short(&h.evidence_digest),
            h.reviewer
                .as_deref()
                .map(|r| format!(" · reviewed by <code>{}</code>", short(r)))
                .unwrap_or_default(),
        ));
        if !h.selected {
            // An active version missing from this page is newer than it.
            let rollback = selected_number.map_or(d.active.is_some(), |n| h.number < n);
            let label = if rollback {
                "Review rollback to this version"
            } else {
                "Review selecting this version"
            };
            out.push_str(&forms.form(
                "select",
                &h.id,
                &d.selection_revision.to_string(),
                label,
                true,
                "",
            )?);
        }
        out.push_str("</li>");
    }
    out.push_str("</ol>");
    if let Some(before) = d.history_before {
        out.push_str(&format!(
            "<p><a href=\"{}?environment={}&amp;before={before}\">Older saved versions</a></p>",
            escape(&base),
            escape(&d.id)
        ));
    }
    if !d.changes.is_empty() {
        out.push_str("<h4>Selection changes</h4><ol reversed>");
        for c in &d.changes {
            out.push_str(&format!(
                "<li>Revision {} · {} <code>{}</code>{} · <time>{}</time></li>",
                c.revision,
                human(&c.kind),
                escape(&c.version_id),
                c.previous
                    .as_deref()
                    .map(|p| format!(" (was <code>{}</code>)", escape(p)))
                    .unwrap_or_default(),
                when(c.at_ms)
            ));
        }
        out.push_str("</ol>");
    }
    out.push_str("<p>Selecting or rolling back changes only jobs admitted afterwards; running, queued, and continued jobs keep the version they started with. No saved version is ever rewritten.</p></section>");

    // Optional terminal.
    out.push_str(&format!(
        "<section aria-labelledby=\"environment-terminal\"><h3 id=\"environment-terminal\">Terminal</h3><p>A terminal on a setup or builder computer is a separately granted native workbench session; this page grants none. <a href=\"/cloud/app/hosts/{}/workbench\">Open the native workbench</a> to enroll and attach where your host grant allows it.</p></section>",
        escape(binding)
    ));
    Ok(out)
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
    let mut content = format!(
        "<h2>Project environment · {}</h2><p>Reading this page changes nothing and never drives a builder or verifier. Leaving it detaches; work keeps running on its own computers.</p>",
        escape(&project)
    );
    if view.environments.len() > 1 {
        content.push_str("<nav aria-label=\"Environments\"><ul>");
        for e in &view.environments {
            let current = view.detail.as_ref().is_some_and(|d| d.id == e.id);
            content.push_str(&format!(
                "<li><a href=\"{}?environment={}\"{}>{}</a>{}</li>",
                escape(&base),
                escape(&e.id),
                if current {
                    " aria-current=\"page\""
                } else {
                    ""
                },
                escape(&e.id),
                if e.retired { " · retired" } else { "" }
            ));
        }
        content.push_str("</ul></nav>");
    }
    match &view.detail {
        None => content.push_str("<section class=\"cloud-card\" aria-labelledby=\"environment-empty\"><h3 id=\"environment-empty\">No environment yet</h3><p>This project has no repository environment. Jobs start from their admitted profile's runtime until a reviewed version is saved and selected here.</p></section>"),
        Some(detail) => match render_detail(&context, &headers, &project, detail) {
            Ok(v) => content.push_str(&v),
            Err(r) => return r,
        },
    }
    content.push_str(&format!(
        "<p><a href=\"{}\">Operator Cloud jobs for this project</a></p>",
        escape(&cloud_url(&context, &project))
    ));
    content.push_str(&controls::link(context.binding));
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
    let mut content = format!(
        "<h2>Retained evidence · verification <code>{}</code>{}</h2><p role=\"status\">Record <code>{}</code> · {} · {} · {} events{}</p><p><a href=\"{base}?environment={}\">Back to the environment</a> · <a href=\"{base}/evidence?q={summary_q}&amp;download=export\">Download the evidence export (JSON)</a></p>",
        escape(&p.verification),
        p.child
            .as_deref()
            .map(|c| format!(" · machine record <code>{}</code>", escape(c)))
            .unwrap_or_default(),
        escape(&p.evidence_id),
        human(&p.status),
        if p.complete {
            "complete"
        } else {
            "not complete"
        },
        p.head_seq,
        p.sealed_digest
            .as_deref()
            .map(|d| format!(" · sealed <code>{}</code>", short(d)))
            .unwrap_or_default(),
        escape(&p.environment),
        base = escape(&base),
    );
    content.push_str("<section aria-labelledby=\"evidence-gaps\"><h3 id=\"evidence-gaps\">Disclosed gaps</h3><ul>");
    for g in &p.gaps {
        content.push_str(&format!("<li><code>{}</code></li>", escape(g)));
    }
    if p.gaps.is_empty() {
        content.push_str("<li>None disclosed.</li>");
    }
    if p.gaps_omitted {
        content.push_str("<li>More gaps exist than this page shows; the export lists those this page carries.</li>");
    }
    content.push_str("</ul></section>");
    if !p.children.is_empty() {
        content.push_str("<section aria-labelledby=\"evidence-children\"><h3 id=\"evidence-children\">Machine records</h3><ul>");
        for c in &p.children {
            let q = match encoded(&evidence_query(
                &context,
                &project,
                &p.environment,
                &p.verification,
                Some(c),
                None,
                None,
            )) {
                Ok(v) => v,
                Err(r) => return r,
            };
            content.push_str(&format!(
                "<li><a href=\"{}/evidence?q={q}\">{}</a></li>",
                escape(&base),
                escape(c)
            ));
        }
        content.push_str("</ul></section>");
    }
    content.push_str(
        "<section aria-labelledby=\"evidence-calls\"><h3 id=\"evidence-calls\">Calls</h3><ul>",
    );
    for c in &p.calls {
        let mut links = String::new();
        for (stream, label, bytes) in [
            (env::Stream::Stdout, "stdout", c.stdout_bytes),
            (env::Stream::Stderr, "stderr", c.stderr_bytes),
        ] {
            let q = match encoded(&evidence_query(
                &context,
                &project,
                &p.environment,
                &p.verification,
                p.child.as_deref(),
                Some((&c.call, stream)),
                None,
            )) {
                Ok(v) => v,
                Err(r) => return r,
            };
            links.push_str(&format!(
                " · <a href=\"{}/evidence?q={q}\">{label} ({bytes} bytes)</a>",
                escape(&base)
            ));
        }
        content.push_str(&format!(
            "<li><code>{}</code> · {} · {}{links}</li>",
            escape(&c.call),
            escape(&c.tool),
            human(&c.outcome)
        ));
    }
    if p.calls.is_empty() {
        content.push_str("<li>No calls recorded.</li>");
    }
    if p.calls_omitted {
        content.push_str("<li>More calls exist than this page lists.</li>");
    }
    content.push_str("</ul></section>");
    if let Some(chunk) = &p.chunk {
        let bytes = match STANDARD.decode(&chunk.data) {
            Ok(v) => v,
            Err(_) => return refused(SessionError::Conflict),
        };
        let display = std::str::from_utf8(&bytes).map_or_else(
            |_| format!("Base64: {}", STANDARD.encode(&bytes)),
            str::to_owned,
        );
        content.push_str(&format!(
            "<section aria-labelledby=\"evidence-bytes\"><h3 id=\"evidence-bytes\">Original bytes · <code>{}</code> {}</h3><p>Byte offset {} · page {} bytes · {} bytes retained so far{} · <code>{}</code>. Pages carry the original retained bytes and may split a UTF-8 character; download a page to keep its exact bytes.</p><p><a href=\"{}&amp;download=yes\">Download these exact bytes</a></p><pre tabindex=\"0\">{}</pre>",
            escape(&chunk.call),
            match chunk.stream {
                env::Stream::Stdout => "stdout",
                env::Stream::Stderr => "stderr",
            },
            chunk.start,
            bytes.len(),
            chunk.length,
            if chunk.closed { " · stream ended" } else { " · stream open" },
            short(&chunk.digest),
            escape(&retry),
            escape(&display)
        ));
        if let Some(next) = &chunk.next {
            let mut q = query.clone();
            q.cursor = Some(next.clone());
            let q = match encoded(&q) {
                Ok(v) => v,
                Err(r) => return r,
            };
            content.push_str(&format!(
                "<p><a href=\"{}/evidence?q={q}\">Next original page</a></p>",
                escape(&base)
            ));
        } else if !chunk.closed {
            content.push_str(&format!(
                "<p>The stream is still open. <a href=\"{}\">Read this page again</a> to continue once more bytes are retained.</p>",
                escape(&retry)
            ));
        }
        content.push_str("</section>");
    }
    content.push_str(&controls::link(context.binding));
    page(&context, &headers, &content, &operation, &outcome, false)
}
