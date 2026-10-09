//! Project supervision and operator Cloud work over separately admitted owners.
//! Reads never advance a worker. Effects use the exact reviewed native packet.

use super::controls::{self, Context, admitted, digest, hidden, show, submit};
use super::session::SessionError;
use super::{colors, protect, refused, service, ticket, work, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use coder_access::protocol::{Operation, Outcome, random_id};
use coder_access::{Right, cloud, project};
use coder_ui::{control, coordination};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/projects", get(index))
        .route("/cloud/app/hosts/{binding}/projects", get(projects))
        .route(
            "/cloud/app/hosts/{binding}/projects/{project}",
            get(project_page),
        )
        .route(
            "/cloud/app/hosts/{binding}/projects/{project}/original",
            get(project_original),
        )
        .route("/cloud/app/hosts/{binding}/cloud", get(cloud_projects))
        .route("/cloud/app/hosts/{binding}/cloud/{project}", get(jobs))
        .route(
            "/cloud/app/hosts/{binding}/cloud/{project}/new",
            get(new_job).post(stage_submit),
        )
        .route(
            "/cloud/app/hosts/{binding}/cloud/{project}/jobs/{job}",
            get(job).post(stage_job),
        )
        .route(
            "/cloud/app/hosts/{binding}/cloud/{project}/jobs/{job}/original",
            get(cloud_original),
        )
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadOptions {
    q: Option<String>,
    download: Option<String>,
}

pub(super) fn encoded(value: &impl Serialize) -> Result<String, Response> {
    let bytes = serde_json::to_vec(value).map_err(|_| refused(SessionError::Conflict))?;
    if bytes.len() > 4096 {
        return Err(refused(SessionError::InvalidRequest));
    }
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn decoded<T: DeserializeOwned>(value: &str) -> Result<T, Response> {
    if value.len() > 5500 {
        return Err(refused(SessionError::InvalidRequest));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| refused(SessionError::InvalidRequest))?;
    if bytes.len() > 4096 {
        return Err(refused(SessionError::InvalidRequest));
    }
    serde_json::from_slice(&bytes).map_err(|_| refused(SessionError::InvalidRequest))
}

fn pretty(value: &impl Serialize) -> String {
    escape(&serde_json::to_string_pretty(value).unwrap_or_else(|_| "Unknown".into()))
}

fn project_url(context: &Context<'_>, project: &str) -> String {
    format!(
        "/cloud/app/hosts/{}/projects/{project}",
        context.binding.id()
    )
}
pub(super) fn cloud_url(context: &Context<'_>, project: &str) -> String {
    format!("/cloud/app/hosts/{}/cloud/{project}", context.binding.id())
}
pub(super) fn job_url(binding: &str, scope: &cloud::Scope) -> String {
    format!(
        "/cloud/app/hosts/{binding}/cloud/{}/jobs/{}",
        scope.project, scope.job
    )
}

async fn read(
    context: &Context<'_>,
    headers: &HeaderMap,
    operation: &Operation,
) -> Result<Outcome, Response> {
    if operation.validate().is_err() {
        return Err(refused(SessionError::InvalidRequest));
    }
    let outcome = context
        .binding
        .read(&context.viewer, operation.clone())
        .await
        .map_err(refused)?;
    if outcome.validate().is_err() || !outcome.answers(operation) {
        return Err(refused(SessionError::Conflict));
    }
    context.current(headers).await?;
    Ok(outcome)
}

pub(super) fn page(
    context: &Context<'_>,
    headers: &HeaderMap,
    content: &str,
    operation: &Operation,
    outcome: &Outcome,
    effectful: bool,
) -> Response {
    let resource = match work::owner_resource(
        context.binding,
        &context.viewer,
        effectful.then_some(context.hosts),
        operation,
        outcome,
    ) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    workspace_shell(
        context.app,
        headers,
        context.service,
        &context.viewer,
        "projects",
        Some(content),
        Some(resource),
    )
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    let mut content = String::from(
        "<h2>Projects and operator Cloud jobs</h2><p>Choose a resident connection. Its project policy and Cloud executor policy admit these views separately.</p><ul>",
    );
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    if bindings.is_empty() {
        content.push_str("<li>No current resident connection.</li>");
    }
    for binding in bindings {
        content.push_str(&format!("<li>Connection {} · <a href=\"/cloud/app/hosts/{}/projects\">Project supervision</a> · <a href=\"/cloud/app/hosts/{}/cloud\">Operator Cloud jobs</a></li>", escape(binding.id()), escape(binding.id()), escape(binding.id())));
    }
    content.push_str("</ul>");
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "projects",
        Some(&content),
        None,
    )
}

async fn projects(
    State(app): State<App>,
    headers: HeaderMap,
    Path(binding): Path<String>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = Operation::ProjectList {
        workspace: context.binding.workspace().into(),
    };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::ProjectList { projects } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let mut content = format!(
        "{}<h2>Native project supervision</h2><p>Only project aliases admitted by this native owner are listed. Reading does not claim issues, recover a scheduler, create worktrees, or start work.</p><ul>",
        controls::link(context.binding)
    );
    for row in &projects.rows {
        content.push_str(&format!(
            "<li><a href=\"{}\">{}</a> · goals {} · snapshot <code>{}</code></li>",
            project_url(&context, &row.id),
            escape(&row.label),
            escape(&row.goals_state),
            escape(&row.snapshot_digest)
        ));
    }
    if projects.rows.is_empty() {
        content.push_str("<li>No admitted project records.</li>");
    }
    content.push_str("</ul>");
    page(&context, &headers, &content, &operation, &outcome, false)
}

fn source<'a>(
    context: &'a Context<'_>,
    record: &'a str,
    revision: &'a str,
    source_digest: &'a str,
    key: &'a str,
) -> coordination::Source<'a> {
    coordination::Source {
        key,
        host: context.binding.host(),
        generation: context.binding.generation(),
        workspace: context.binding.workspace(),
        record,
        revision,
        source_digest,
    }
}

fn project_view(context: &Context<'_>, value: &project::Page) -> Result<String, Response> {
    let claims: Vec<String> = value
        .tasks
        .iter()
        .map(|task| {
            task.claim.as_ref().map_or_else(
                || "Unknown: no retained native claim".into(),
                |claim| serde_json::to_string(claim).unwrap_or_else(|_| "Unknown".into()),
            )
        })
        .collect();
    let blockers: Vec<Vec<&str>> = value
        .tasks
        .iter()
        .map(|task| task.blockers.iter().map(String::as_str).collect())
        .collect();
    let worktrees: Vec<Vec<&str>> = value
        .tasks
        .iter()
        .map(|task| vec![task.worktree_state.as_str()])
        .collect();
    let dependencies: Vec<Vec<coordination::Dependency<'_>>> = value
        .tasks
        .iter()
        .map(|task| {
            task.dependencies
                .iter()
                .filter_map(|id| value.tasks.iter().find(|peer| peer.id == *id))
                .map(|peer| coordination::Dependency {
                    issue: peer.issue,
                    repository: None,
                    state: &peer.status,
                })
                .collect()
        })
        .collect();
    let keys: Vec<_> = (0..value.tasks.len())
        .map(|index| format!("issue-{index}"))
        .collect();
    let issues: Vec<_> = value
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| coordination::Issue {
            key: &keys[i],
            number: task.issue,
            title: &task.title,
            state: &value.tracker_state,
            status: &task.status,
            version: &task.content_digest,
            claim: &claims[i],
            dependencies: &dependencies[i],
            blockers: &blockers[i],
            worktrees: &worktrees[i],
        })
        .collect();
    let capacity = serde_json::to_string(&value.capacity).unwrap_or_else(|_| "Unknown".into());
    let review = serde_json::to_string(&value.review).unwrap_or_else(|_| "Unknown".into());
    let goals = [coordination::Goal {
        key: "native-goals",
        id: "goals",
        title: "Native goals",
        state: &value.goals_state,
        tasks: &[],
        blockers: &[],
    }];
    let mut html = show(&coordination::project(
        &coordination::Project {
            source: source(
                context,
                &value.project,
                &value.snapshot_digest,
                &value.snapshot_digest,
                "project",
            ),
            title: &value.label,
            repository: &value.project,
            goals: &goals,
            issues: &[],
            capacity: &capacity,
            review_backpressure: &review,
        },
        colors(),
    ))?;
    for issue in issues {
        html.push_str(&show(&coordination::issue(&issue, colors()))?);
    }
    Ok(html)
}

async fn project_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    Query(options): Query<ReadOptions>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let query: project::Query = match options.q {
        Some(q) => match decoded(&q) {
            Ok(v) => v,
            Err(r) => return r,
        },
        None => project::Query {
            workspace: context.binding.workspace().into(),
            project: project.clone(),
            snapshot: None,
            cursor: None,
            limit: 32,
        },
    };
    if query.workspace != context.binding.workspace()
        || query.project != project
        || options.download.is_some()
    {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::ProjectRead { query };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::ProjectRead { project: value } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let mut content = match project_view(&context, value) {
        Ok(v) => v,
        Err(r) => return r,
    };
    content.push_str(&format!("<p>Sequence {} · {} tasks remain on later pages. Dependencies absent from this page remain unknown. The original native projection retains dependency IDs, resource footprints, backoff, claims, review capacity, exclusions, and observed worktree evidence.</p><details><summary>Original bounded native projection</summary><pre>{}</pre></details><h3>Retained source records</h3><ul>", value.sequence, value.remaining, pretty(value)));
    for retained in &value.sources {
        content.push_str(&format!(
            "<li>{} · {}",
            escape(&retained.id),
            escape(&retained.state)
        ));
        if let Some(original) = &retained.original {
            let query = project::OriginalQuery {
                workspace: value.workspace.clone(),
                project: value.project.clone(),
                snapshot_digest: value.snapshot_digest.clone(),
                original: original.clone(),
                cursor: None,
                limit: project::MAX_CHUNK_BYTES as u32,
            };
            let q = match encoded(&query) {
                Ok(v) => v,
                Err(r) => return r,
            };
            content.push_str(&format!(" · <a href=\"{}/original?q={q}\">Read original bytes</a> · {} bytes · <code>{}</code>", project_url(&context, &project), original.bytes, escape(&original.digest)));
        }
        content.push_str("</li>");
    }
    content.push_str("</ul>");
    if let Some(cursor) = &value.next {
        let query = project::Query {
            workspace: value.workspace.clone(),
            project: project.clone(),
            snapshot: Some(value.snapshot_digest.clone()),
            cursor: Some(cursor.clone()),
            limit: 32,
        };
        let q = match encoded(&query) {
            Ok(v) => v,
            Err(r) => return r,
        };
        content.push_str(&format!(
            "<p><a href=\"{}?q={q}\">Next original snapshot page</a></p>",
            project_url(&context, &project)
        ));
    }
    content.push_str(&controls::link(context.binding));
    page(&context, &headers, &content, &operation, &outcome, false)
}

async fn cloud_projects(
    State(app): State<App>,
    headers: HeaderMap,
    Path(binding): Path<String>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = Operation::CloudProjects {
        workspace: context.binding.workspace().into(),
    };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudProjects { projects } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let mut content = String::from(
        "<h2>Operator Cloud projects</h2><p>The native operator policy separately admits these projects and executor profiles. Retail purchases use their own admission.</p><ul>",
    );
    for project in &projects.projects {
        content.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            cloud_url(&context, project),
            escape(project)
        ));
    }
    if projects.projects.is_empty() {
        content.push_str("<li>No admitted operator Cloud projects.</li>");
    }
    content.push_str("</ul>");
    page(&context, &headers, &content, &operation, &outcome, false)
}

pub(super) async fn catalog(
    context: &Context<'_>,
    headers: &HeaderMap,
    project: &str,
) -> Result<(Operation, Outcome), Response> {
    let operation = Operation::CloudCatalog {
        query: cloud::CatalogQuery {
            workspace: context.binding.workspace().into(),
            project: project.into(),
        },
    };
    let outcome = read(context, headers, &operation).await?;
    Ok((operation, outcome))
}

async fn jobs(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    Query(options): Query<ReadOptions>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let query: cloud::ListQuery = match options.q {
        Some(q) => match decoded(&q) {
            Ok(v) => v,
            Err(r) => return r,
        },
        None => cloud::ListQuery {
            workspace: context.binding.workspace().into(),
            project: project.clone(),
            cursor: None,
            limit: 32,
        },
    };
    if query.workspace != context.binding.workspace()
        || query.project != project
        || options.download.is_some()
    {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::CloudList { query };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudList { jobs } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let mut content = format!(
        "<h2>Operator Cloud jobs · {}</h2><p>These are canonical native jobs. Listing and reading never submit, continue, cancel, or drive a worker.</p><p><a href=\"{}/new\">Inspect admitted executor profiles and compose a job</a> · <a href=\"{}\">Project environment</a></p><ul>",
        escape(&project),
        cloud_url(&context, &project),
        escape(&super::environment::panel_url(
            context.binding.id(),
            &project
        ))
    );
    for row in &jobs.rows {
        content.push_str(&format!("<li><a href=\"{}\">{}</a> · {} · attempt {} · executor {} · requested model {} · cleanup {}</li>", job_url(context.binding.id(), &row.scope), escape(&row.scope.job), escape(&row.state), row.scope.attempt, escape(&row.executor), escape(row.model.as_deref().unwrap_or("Unknown")), escape(&row.cleanup)));
    }
    if jobs.rows.is_empty() {
        content.push_str("<li>No retained jobs in this admitted project.</li>");
    }
    content.push_str("</ul>");
    if let Some(cursor) = &jobs.next {
        let query = cloud::ListQuery {
            workspace: jobs.workspace.clone(),
            project: jobs.project.clone(),
            cursor: Some(cursor.clone()),
            limit: 32,
        };
        let q = match encoded(&query) {
            Ok(v) => v,
            Err(r) => return r,
        };
        content.push_str(&format!(
            "<p><a href=\"{}?q={q}\">Next original list page</a></p>",
            cloud_url(&context, &project)
        ));
    }
    content.push_str(&controls::link(context.binding));
    page(&context, &headers, &content, &operation, &outcome, false)
}

pub(super) fn can_operate(context: &Context<'_>) -> bool {
    context
        .binding
        .access()
        .grant
        .rights
        .contains(Right::Operate)
        && context.enrolled().is_ok()
}

/// Prepared packets have no cached native result yet. Check the current domain
/// policy before disclosing their private action or keeping a receipt mounted.
pub(super) async fn admit_action(
    binding: &super::hosts::Binding,
    viewer: &super::session::Viewer,
    action: &Operation,
) -> Result<(), SessionError> {
    let Some(admission) = cloud::Admission::for_operation(action) else {
        return Ok(());
    };
    if admission.workspace != binding.workspace() {
        return Err(SessionError::Forbidden);
    }
    let operation = Operation::CloudCatalog {
        query: cloud::CatalogQuery {
            workspace: admission.workspace.clone(),
            project: admission.project.clone(),
        },
    };
    let outcome = binding.read(viewer, operation.clone()).await?;
    if outcome.validate().is_err() || !outcome.answers(&operation) {
        return Err(SessionError::Conflict);
    }
    let Outcome::CloudCatalog { catalog } = outcome else {
        return Err(SessionError::Conflict);
    };
    if !catalog.profiles.iter().any(|profile| {
        profile.name == admission.profile
            && profile.revision == admission.profile_revision
            && profile.source_digest == admission.source_digest
    }) {
        return Err(SessionError::Forbidden);
    }
    if let Some(job) = admission.job {
        let operation = Operation::CloudRead {
            query: cloud::ReadQuery {
                workspace: admission.workspace,
                project: admission.project,
                job,
                revision: None,
            },
        };
        let outcome = binding.read(viewer, operation.clone()).await?;
        if outcome.validate().is_err() || !outcome.answers(&operation) {
            return Err(SessionError::Conflict);
        }
        let Outcome::CloudRead { job } = outcome else {
            return Err(SessionError::Conflict);
        };
        if job.scope.profile != admission.profile
            || job.scope.profile_revision != admission.profile_revision
            || job.scope.source_digest != admission.source_digest
        {
            return Err(SessionError::Forbidden);
        }
    }
    Ok(())
}
fn composer(
    context: &Context<'_>,
    engine: Option<&str>,
    enabled: bool,
) -> Result<String, Response> {
    show(&control::composer(
        &control::Composer {
            key: "task",
            target: control::Target {
                host: context.binding.host(),
                generation: context.binding.generation(),
                workspace: context.binding.workspace(),
                task: None,
                revision: None,
                attempt: None,
            },
            engine,
            engine_readiness: "The native operator policy admits this exact profile and source revision at confirmation.",
            max_bytes: 16 * 1024,
            enabled,
            reason: (!enabled)
                .then_some("Current browser enrollment and native Operate authority are required."),
        },
        colors(),
    ))
}

async fn new_job(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (operation, outcome) = match catalog(&context, &headers, &project).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudCatalog { catalog } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let enabled = can_operate(&context);
    let mut content = String::from(
        "<h2>Admitted operator Cloud profiles</h2><p>Each native profile fixes its source revision and digest, pool, placement, executor, model policy, credential names, and timeout bound. Review stages intent; confirmation submits the exact signed request. Browser drafts remain in this page.</p>",
    );
    for profile in &catalog.profiles {
        let request = random_id();
        let basis = digest(&json!({"request":request,"profile":profile,"project":project}));
        let csrf = match context.csrf(&headers, "cloud-submit", &basis) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let button = match submit(
            "cloud-submit",
            "Review Cloud submission",
            enabled && profile.availability == "configured",
        ) {
            Ok(v) => v,
            Err(r) => return r,
        };
        // Each composer has a unique semantic key while keeping the native form name.
        let field = match composer(&context, Some(&profile.executor), enabled) {
            Ok(v) => v
                .replace("task:prompt", &format!("{}:prompt", escape(&profile.name)))
                .replace(
                    &format!("name=\"{}:prompt\"", escape(&profile.name)),
                    "name=\"task:prompt\"",
                ),
            Err(r) => return r,
        };
        content.push_str(&format!("<section class=\"cloud-card\"><h3>{}</h3><pre>{}</pre><form method=\"post\">{}{}{}{}{field}<label>Timeout in seconds <input type=\"number\" name=\"timeout\" min=\"1\" max=\"{}\" value=\"{}\" required></label>{button}</form></section>", escape(&profile.name), pretty(profile), ticket(&csrf), hidden("request", &request), hidden("profile", &profile.name), hidden("basis", &basis), profile.max_timeout_seconds, profile.max_timeout_seconds.min(3600)));
    }
    if catalog.profiles.is_empty() {
        content.push_str("<p>No admitted executor profiles.</p>");
    }
    let resource_effectful = context.book().is_ok();
    page(
        &context,
        &headers,
        &content,
        &operation,
        &outcome,
        resource_effectful,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitForm {
    csrf: String,
    request: String,
    profile: String,
    basis: String,
    #[serde(rename = "task:prompt")]
    prompt: String,
    timeout: u64,
}

async fn stage_submit(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    form: Result<Form<SubmitForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (_, outcome) = match catalog(&context, &headers, &project).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudCatalog { catalog } = outcome else {
        return refused(SessionError::Conflict);
    };
    let Some(profile) = catalog
        .profiles
        .iter()
        .find(|profile| profile.name == form.profile)
    else {
        return refused(SessionError::Forbidden);
    };
    let basis = digest(&json!({"request":form.request,"profile":profile,"project":project}));
    if form.basis != basis || profile.availability != "configured" {
        return refused(SessionError::Conflict);
    }
    if let Err(r) = context.verify(&headers, &form.csrf, "cloud-submit", &basis) {
        return r;
    }
    let operation = Operation::CloudSubmit {
        intent: cloud::Submit {
            workspace: context.binding.workspace().into(),
            project,
            profile: profile.name.clone(),
            profile_revision: profile.revision.clone(),
            source_digest: profile.source_digest.clone(),
            prompt: form.prompt,
            timeout_seconds: form.timeout,
        },
    };
    if form.timeout > profile.max_timeout_seconds {
        return refused(SessionError::InvalidRequest);
    }
    controls::staged(&context, &headers, &form.request, operation).await
}

pub(super) async fn job_read(
    context: &Context<'_>,
    headers: &HeaderMap,
    project: &str,
    job: &str,
) -> Result<(Operation, Outcome), Response> {
    let operation = Operation::CloudRead {
        query: cloud::ReadQuery {
            workspace: context.binding.workspace().into(),
            project: project.into(),
            job: job.into(),
            revision: None,
        },
    };
    let outcome = read(context, headers, &operation).await?;
    Ok((operation, outcome))
}

/// The saved environment version a job started with (ENV-06). A job keeps
/// this pin; later selections and rollbacks never reach it.
pub(super) fn environment_pin(binding: &str, project: &str, value: &cloud::Job) -> String {
    let Some(pin) = &value.environment else {
        return "<section aria-labelledby=\"job-environment\"><h3 id=\"job-environment\">Environment</h3><p>This job started without a saved environment version; it uses its admitted profile's runtime.</p></section>".into();
    };
    format!(
        "<section aria-labelledby=\"job-environment\"><h3 id=\"job-environment\">Environment</h3><p>Started from version {} (<code>{}</code>) of environment <a href=\"{}?environment={}\"><code>{}</code></a> at selection revision {} · recipe revision {} · source <code>{}</code> · image <code>{}</code>{} · evidence <code>{}</code>. Continuations and retries keep this exact version; later selections or rollbacks never change it.</p></section>",
        pin.number,
        escape(&pin.version_id),
        escape(&super::environment::panel_url(binding, project)),
        escape(&pin.environment),
        escape(&pin.environment),
        pin.selection_revision,
        pin.recipe_revision,
        escape(&pin.source_revision),
        escape(&pin.image.image_id),
        pin.image
            .snapshot_id
            .as_deref()
            .map(|s| format!(" · snapshot <code>{}</code>", escape(s)))
            .unwrap_or_default(),
        escape(&pin.evidence_digest[..pin.evidence_digest.len().min(12)]),
    )
}

fn job_view(context: &Context<'_>, value: &cloud::Job) -> Result<String, Response> {
    let keys: Vec<_> = (0..value.originals.len())
        .map(|index| format!("original-{index}"))
        .collect();
    let artifacts: Vec<_> = value
        .originals
        .iter()
        .enumerate()
        .map(|(index, original)| coordination::Artifact {
            key: &keys[index],
            label: &original.source,
            digest: &original.digest,
            retention: "Retained by native owner",
            state: if original.source.starts_with("artifact:") {
                &value.artifact_state
            } else {
                "Retained original source"
            },
        })
        .collect();
    let attempt_id = value.scope.attempt.to_string();
    let attempts = [coordination::Attempt {
        key: "attempt",
        id: &attempt_id,
        state: &value.state,
        executor: &value.executor,
        requested_model: value.model.as_deref(),
        served_model: value.served_model.as_deref(),
        usage: value.usage.as_ref(),
        cost: None,
        continuation: Some(&value.continuation),
        stop_request: &value.cancellation,
        delivery: "Unknown",
        publish: "Unknown",
        cleanup: &value.cleanup,
    }];
    show(&coordination::job(
        &coordination::Job {
            source: source(
                context,
                &value.scope.job,
                &value.scope.revision,
                &value.scope.source_digest,
                "cloud-job",
            ),
            title: &value.scope.job,
            state: &value.state,
            executor: &value.executor,
            placement: &value.placement,
            requested_model: value.model.as_deref(),
            served_model: value.served_model.as_deref(),
            usage: value.usage.as_ref(),
            cost: None,
            continuation: Some(&value.continuation),
            stop_request: &value.cancellation,
            delivery: "Unknown",
            publish: "Unknown",
            cleanup: &value.cleanup,
            artifacts: &artifacts,
            attempts: &attempts,
        },
        colors(),
    ))
}

pub(super) async fn job(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project, job)): Path<(String, String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (operation, outcome) = match job_read(&context, &headers, &project, &job).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudRead { job: value } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let mut content = match job_view(&context, value) {
        Ok(v) => v,
        Err(r) => return r,
    };
    content.push_str(&environment_pin(context.binding.id(), &project, value));
    content.push_str(&format!("<h3>Original job projection</h3><pre>{}</pre><p>Cost and publication remain unknown unless their own canonical records supply evidence. A cancellation request does not establish termination or cleanup. An omitted prompt or detail remains available only through an admitted retained original.</p><h3>Retained originals</h3><ul>", pretty(value)));
    for original in &value.originals {
        let query = cloud::OriginalQuery {
            scope: value.scope.clone(),
            original: original.clone(),
            cursor: None,
            limit: cloud::MAX_CHUNK_BYTES,
        };
        let q = match encoded(&query) {
            Ok(v) => v,
            Err(r) => return r,
        };
        content.push_str(&format!(
            "<li><a href=\"{}/original?q={q}\">{}</a> · {} bytes · <code>{}</code></li>",
            job_url(context.binding.id(), &value.scope),
            escape(&original.source),
            original.bytes,
            escape(&original.digest)
        ));
    }
    content.push_str("</ul>");
    let enabled = can_operate(&context);
    for (action, label, prompt) in [
        ("continue", "Review a new continuation turn", true),
        ("cancel", "Review stop request", false),
        (
            "follow",
            "Review reconciliation of this original job",
            false,
        ),
    ] {
        let request = random_id();
        let basis = digest(&json!({"request":request,"scope":value.scope,"action":action}));
        let csrf = match context.csrf(&headers, "cloud-action", &basis) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let action_enabled = enabled
            && match action {
                "continue" => value.continuation == "available",
                "cancel" => value.cancellation != "requested",
                _ => true,
            };
        let button = match submit(&format!("cloud-{action}"), label, action_enabled) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let field = if prompt {
            match composer(&context, Some(&value.executor), action_enabled) {
                Ok(v) => v,
                Err(r) => return r,
            }
        } else if action == "cancel" {
            "<label>Reason <input name=\"reason\" maxlength=\"4096\" required></label>".into()
        } else {
            String::new()
        };
        content.push_str(&format!(
            "<form method=\"post\" action=\"{}\">{}{}{}{}{field}{button}</form>",
            escape(&job_url(context.binding.id(), &value.scope)),
            ticket(&csrf),
            hidden("request", &request),
            hidden("action", action),
            hidden("basis", &basis)
        ));
    }
    content.push_str("<p>Continue creates a separately reviewed native turn. Reconcile advances observation of the original job under its current admission. Review, apply artifacts, and publish remain separate native actions.</p>");
    content.push_str("<p>Candidate review, artifact application, and publication are unavailable without their separately admitted native candidate owner. Leaving this page detaches observation and does not stop work.</p>");
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
struct JobForm {
    csrf: String,
    request: String,
    action: String,
    basis: String,
    #[serde(rename = "task:prompt")]
    prompt: Option<String>,
    reason: Option<String>,
}

async fn stage_job(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project, job)): Path<(String, String, String)>,
    form: Result<Form<JobForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (_, outcome) = match job_read(&context, &headers, &project, &job).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudRead { job: value } = outcome else {
        return refused(SessionError::Conflict);
    };
    let basis = digest(&json!({"request":form.request,"scope":value.scope,"action":form.action}));
    if basis != form.basis {
        return refused(SessionError::Conflict);
    }
    if let Err(r) = context.verify(&headers, &form.csrf, "cloud-action", &basis) {
        return r;
    }
    let operation = match form.action.as_str() {
        "continue" if form.reason.is_none() => Operation::CloudContinue {
            intent: cloud::Continue {
                scope: value.scope,
                prompt: form.prompt.unwrap_or_default(),
            },
        },
        "cancel" if form.prompt.is_none() => Operation::CloudCancel {
            intent: cloud::Cancel {
                scope: value.scope,
                reason: form.reason.unwrap_or_default(),
            },
        },
        "follow" if form.prompt.is_none() && form.reason.is_none() => Operation::CloudFollow {
            intent: cloud::Follow { scope: value.scope },
        },
        _ => return refused(SessionError::InvalidRequest),
    };
    controls::staged(&context, &headers, &form.request, operation).await
}

pub(super) fn bytes_response(bytes: Vec<u8>) -> Response {
    let mut response = protect(bytes.into_response());
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=original-chunk.bin"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn chunk_content(bytes: &[u8], offset: u64, total: u64, digest: &str, url: &str) -> String {
    let display = std::str::from_utf8(bytes).map_or_else(
        |_| format!("Base64: {}", STANDARD.encode(bytes)),
        str::to_owned,
    );
    format!(
        "<h2>Original native source bytes</h2><p>Byte offset {offset} · chunk {} bytes · original {total} bytes · <code>{}</code>. Chunks retain the original bytes and may split a UTF-8 character. Download this chunk to preserve its exact bytes.</p><p><a href=\"{url}&amp;download=yes\">Download these exact bytes</a></p><pre>{}</pre>",
        bytes.len(),
        escape(digest),
        escape(&display)
    )
}

async fn project_original(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project)): Path<(String, String)>,
    Query(options): Query<ReadOptions>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(q) = options.q else {
        return refused(SessionError::InvalidRequest);
    };
    let query: project::OriginalQuery = match decoded(&q) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if query.workspace != context.binding.workspace() || query.project != project {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::ProjectOriginal {
        query: query.clone(),
    };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::ProjectOriginal { chunk } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let bytes = match STANDARD.decode(&chunk.data_base64) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    if options.download.as_deref() == Some("yes") {
        return bytes_response(bytes);
    }
    if options.download.is_some() {
        return refused(SessionError::InvalidRequest);
    }
    let url = format!("{}/original?q={q}", project_url(&context, &project));
    let mut content = chunk_content(
        &bytes,
        chunk.offset,
        chunk.original.bytes,
        &chunk.original.digest,
        &url,
    );
    if let Some(cursor) = &chunk.next {
        let mut next = query;
        next.cursor = Some(cursor.clone());
        let next = match encoded(&next) {
            Ok(v) => v,
            Err(r) => return r,
        };
        content.push_str(&format!(
            "<p><a href=\"{}/original?q={next}\">Next original chunk</a></p>",
            project_url(&context, &project)
        ));
    }
    page(&context, &headers, &content, &operation, &outcome, false)
}

async fn cloud_original(
    State(app): State<App>,
    headers: HeaderMap,
    Path((binding, project, job)): Path<(String, String, String)>,
    Query(options): Query<ReadOptions>,
) -> Response {
    let context = match admitted(&app, &headers, &binding).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(q) = options.q else {
        return refused(SessionError::InvalidRequest);
    };
    let query: cloud::OriginalQuery = match decoded(&q) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if query.scope.workspace != context.binding.workspace()
        || query.scope.project != project
        || query.scope.job != job
    {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::CloudOriginal {
        query: query.clone(),
    };
    let outcome = match read(&context, &headers, &operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Outcome::CloudOriginal { chunk } = &outcome else {
        return refused(SessionError::Conflict);
    };
    let bytes = match STANDARD.decode(&chunk.data) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    if options.download.as_deref() == Some("yes") {
        return bytes_response(bytes);
    }
    if options.download.is_some() {
        return refused(SessionError::InvalidRequest);
    }
    let url = format!(
        "{}/original?q={q}",
        job_url(context.binding.id(), &chunk.scope)
    );
    let mut content = chunk_content(
        &bytes,
        chunk.start,
        chunk.original.bytes,
        &chunk.original.digest,
        &url,
    );
    if let Some(cursor) = &chunk.next {
        let mut next = query;
        next.cursor = Some(cursor.clone());
        let next = match encoded(&next) {
            Ok(v) => v,
            Err(r) => return r,
        };
        content.push_str(&format!(
            "<p><a href=\"{}/original?q={next}\">Next original chunk</a></p>",
            job_url(context.binding.id(), &chunk.scope)
        ));
    }
    page(&context, &headers, &content, &operation, &outcome, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_links_preserve_scopes_and_reject_unbounded_or_unknown_fields() {
        let query = project::Query {
            workspace: "checkout".into(),
            project: "project".into(),
            snapshot: None,
            cursor: None,
            limit: 32,
        };
        let q = encoded(&query).unwrap();
        let decoded: project::Query = super::decoded(&q).unwrap();
        assert_eq!(query, decoded);
        assert!(super::decoded::<project::Query>(&"a".repeat(5501)).is_err());
        let q = URL_SAFE_NO_PAD.encode(br#"{"workspace":"checkout","project":"project","snapshot":null,"cursor":null,"limit":32,"path":"/private"}"#);
        assert!(super::decoded::<project::Query>(&q).is_err());
    }

    #[test]
    fn original_chunk_display_escapes_text_and_preserves_binary_download() {
        let body = chunk_content(
            b"<script>private</script>",
            0,
            24,
            "sha256:synthetic",
            "/original?q=abc",
        );
        assert!(!body.contains("<script>"));
        assert!(body.contains("&lt;script&gt;"));
        assert!(body.contains("download=yes"));
        assert!(
            chunk_content(&[255, 0], 16, 18, "digest", "/original?q=abc").contains("Base64: /wA=")
        );
    }
}
