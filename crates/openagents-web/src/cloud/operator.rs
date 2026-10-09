//! Project supervision and operator Cloud work over separately admitted owners.
//! Reads never advance a worker. Effects use the exact reviewed native packet.

use super::controls::{self, Context, admitted, digest, show, submit};
use super::session::SessionError;
use super::ui::{self, BoundForm};
use super::{colors, protect, refused, service, work, workspace_shell};
use crate::App;
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
use maud::{Markup, PreEscaped, html};
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

/// The pretty JSON of an original native record (escaped when rendered).
fn pretty(value: &impl Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "Unknown".into())
}

/// The computer connection and resident task links every page ends with.
fn connection_links(context: &Context<'_>) -> Markup {
    PreEscaped(controls::link(context.binding))
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
        Some(PreEscaped(content.to_owned())),
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
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    let content = html! {
        h2 { "Projects and operator Cloud jobs" }
        p { "Choose a resident connection. Its project policy and Cloud executor policy admit these views separately." }
        ul {
            @if bindings.is_empty() {
                li { "No current resident connection." }
            }
            @for binding in &bindings {
                li {
                    "Connection " (binding.id()) " \u{b7} "
                    a href=(format!("/cloud/app/hosts/{}/projects", binding.id())) { "Project supervision" }
                    " \u{b7} "
                    a href=(format!("/cloud/app/hosts/{}/cloud", binding.id())) { "Operator Cloud jobs" }
                }
            }
        }
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "projects",
        Some(content),
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
    let content = html! {
        (connection_links(&context))
        h2 { "Native project supervision" }
        p { "Only project aliases admitted by this native owner are listed. Reading does not claim issues, recover a scheduler, create worktrees, or start work." }
        ul {
            @for row in &projects.rows {
                li {
                    a href=(project_url(&context, &row.id)) { (row.label) }
                    " \u{b7} goals " (row.goals_state)
                    " \u{b7} snapshot " code { (row.snapshot_digest) }
                }
            }
            @if projects.rows.is_empty() {
                li { "No admitted project records." }
            }
        }
    }
    .into_string();
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
    let mut html = String::new();
    html.push_str(&show(&coordination::project(
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
    ))?);
    for issue in issues {
        html.push_str(&show(&coordination::issue(&issue, colors()))?);
    }
    Ok(ui::native(&html).into_string())
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
    let mut sources = Vec::with_capacity(value.sources.len());
    for retained in &value.sources {
        let original = match &retained.original {
            Some(original) => {
                let query = project::OriginalQuery {
                    workspace: value.workspace.clone(),
                    project: value.project.clone(),
                    snapshot_digest: value.snapshot_digest.clone(),
                    original: original.clone(),
                    cursor: None,
                    limit: project::MAX_CHUNK_BYTES as u32,
                };
                match encoded(&query) {
                    Ok(q) => Some((q, original)),
                    Err(r) => return r,
                }
            }
            None => None,
        };
        sources.push((retained, original));
    }
    let next = match &value.next {
        Some(cursor) => {
            let query = project::Query {
                workspace: value.workspace.clone(),
                project: project.clone(),
                snapshot: Some(value.snapshot_digest.clone()),
                cursor: Some(cursor.clone()),
                limit: 32,
            };
            match encoded(&query) {
                Ok(q) => Some(q),
                Err(r) => return r,
            }
        }
        None => None,
    };
    let base = project_url(&context, &project);
    content.push_str(
        &html! {
            p {
                "Sequence " (value.sequence) " \u{b7} " (value.remaining)
                " tasks remain on later pages. Dependencies absent from this page remain unknown. The original native projection retains dependency IDs, resource footprints, backoff, claims, review capacity, exclusions, and observed worktree evidence."
            }
            details {
                summary { "Original bounded native projection" }
                pre { (pretty(value)) }
            }
            h3 { "Retained source records" }
            ul {
                @for (retained, original) in &sources {
                    li {
                        (retained.id) " \u{b7} " (retained.state)
                        @if let Some((q, original)) = original {
                            " \u{b7} "
                            a href=(format!("{base}/original?q={q}")) { "Read original bytes" }
                            " \u{b7} " (original.bytes) " bytes \u{b7} "
                            code { (original.digest) }
                        }
                    }
                }
            }
            @if let Some(q) = &next {
                p { a href=(format!("{base}?q={q}")) { "Next original snapshot page" } }
            }
            (connection_links(&context))
        }
        .into_string(),
    );
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
    let content = html! {
        h2 { "Operator Cloud projects" }
        p { "The native operator policy separately admits these projects and executor profiles. Retail purchases use their own admission." }
        ul {
            @for project in &projects.projects {
                li { a href=(cloud_url(&context, project)) { (project) } }
            }
            @if projects.projects.is_empty() {
                li { "No admitted operator Cloud projects." }
            }
        }
    }
    .into_string();
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
    let next = match &jobs.next {
        Some(cursor) => {
            let query = cloud::ListQuery {
                workspace: jobs.workspace.clone(),
                project: jobs.project.clone(),
                cursor: Some(cursor.clone()),
                limit: 32,
            };
            match encoded(&query) {
                Ok(q) => Some(q),
                Err(r) => return r,
            }
        }
        None => None,
    };
    let base = cloud_url(&context, &project);
    let environment = super::environment::panel_url(context.binding.id(), &project);
    let content = html! {
        h2 { "Operator Cloud jobs \u{b7} " (project) }
        p { "These are canonical native jobs. Listing and reading never submit, continue, cancel, or drive a worker." }
        (ui::links([
            (format!("{base}/new").as_str(), "Inspect admitted executor profiles and compose a job"),
            (environment.as_str(), "Project environment"),
        ]))
        ul {
            @for row in &jobs.rows {
                li {
                    a href=(job_url(context.binding.id(), &row.scope)) { (row.scope.job) }
                    " \u{b7} " (ui::status(&row.state, ui::Tone::Neutral))
                    " \u{b7} attempt " (row.scope.attempt)
                    " \u{b7} executor " (row.executor)
                    " \u{b7} requested model " (row.model.as_deref().unwrap_or("Unknown"))
                    " \u{b7} cleanup " (row.cleanup)
                }
            }
            @if jobs.rows.is_empty() {
                li { "No retained jobs in this admitted project." }
            }
        }
        @if let Some(q) = &next {
            p { a href=(format!("{base}?q={q}")) { "Next original list page" } }
        }
        (connection_links(&context))
    }
    .into_string();
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
    let mut cards = Vec::with_capacity(catalog.profiles.len());
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
        let name = crate::layout::escape(&profile.name);
        let field = match composer(&context, Some(&profile.executor), enabled) {
            Ok(v) => v
                .replace("task:prompt", &format!("{name}:prompt"))
                .replace(&format!("name=\"{name}:prompt\""), "name=\"task:prompt\""),
            Err(r) => return r,
        };
        let timeout = profile.max_timeout_seconds.to_string();
        let form = BoundForm::here()
            .csrf(&csrf)
            .bind("request", &request)
            .bind("profile", &profile.name)
            .bind("basis", &basis)
            .body(html! {
                (ui::native(&field))
                label {
                    "Timeout in seconds "
                    input type="number" name="timeout" min="1" max=(timeout)
                        value=(profile.max_timeout_seconds.min(3600)) required;
                }
            })
            .submit_with(PreEscaped(button));
        cards.push(ui::card(html! {
            h3 { (profile.name) }
            pre { (pretty(profile)) }
            (form)
        }));
    }
    let content = html! {
        h2 { "Admitted operator Cloud profiles" }
        p { "Each native profile fixes its source revision and digest, pool, placement, executor, model policy, credential names, and timeout bound. Review stages intent; confirmation submits the exact signed request. Browser drafts remain in this page." }
        @for card in &cards { (card) }
        @if catalog.profiles.is_empty() {
            (ui::empty("No admitted executor profiles", "No admitted executor profiles."))
        }
    }
    .into_string();
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
    let body = match &value.environment {
        None => html! {
            p { "This job started without a saved environment version; it uses its admitted profile's runtime." }
        },
        Some(pin) => html! {
            p {
                "Started from version " (pin.number) " (" code { (pin.version_id) } ") of environment "
                a href=(format!("{}?environment={}", super::environment::panel_url(binding, project), pin.environment)) {
                    code { (pin.environment) }
                }
                " at selection revision " (pin.selection_revision)
                " \u{b7} recipe revision " (pin.recipe_revision)
                " \u{b7} source " code { (pin.source_revision) }
                " \u{b7} image " code { (pin.image.image_id) }
                @if let Some(snapshot) = pin.image.snapshot_id.as_deref() {
                    " \u{b7} snapshot " code { (snapshot) }
                }
                " \u{b7} evidence " code { (&pin.evidence_digest[..pin.evidence_digest.len().min(12)]) }
                ". Continuations and retries keep this exact version; later selections or rollbacks never change it."
            }
        },
    };
    ui::section("job-environment", "Environment", body).into_string()
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
    .map(|view| ui::native(&view).into_string())
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
    let mut originals = Vec::with_capacity(value.originals.len());
    for original in &value.originals {
        let query = cloud::OriginalQuery {
            scope: value.scope.clone(),
            original: original.clone(),
            cursor: None,
            limit: cloud::MAX_CHUNK_BYTES,
        };
        match encoded(&query) {
            Ok(q) => originals.push((q, original)),
            Err(r) => return r,
        }
    }
    let job_href = job_url(context.binding.id(), &value.scope);
    let enabled = can_operate(&context);
    let mut forms = Vec::with_capacity(3);
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
                Ok(v) => ui::native(&v),
                Err(r) => return r,
            }
        } else if action == "cancel" {
            html! { label { "Reason " input name="reason" maxlength="4096" required; } }
        } else {
            html! {}
        };
        forms.push(
            BoundForm::new(job_href.as_str())
                .csrf(&csrf)
                .bind("request", &request)
                .bind("action", action)
                .bind("basis", &basis)
                .body(field)
                .submit_with(PreEscaped(button)),
        );
    }
    content.push_str(
        &html! {
            h3 { "Original job projection" }
            pre { (pretty(value)) }
            p { "Cost and publication remain unknown unless their own canonical records supply evidence. A cancellation request does not establish termination or cleanup. An omitted prompt or detail remains available only through an admitted retained original." }
            h3 { "Retained originals" }
            ul {
                @for (q, original) in &originals {
                    li {
                        a href=(format!("{job_href}/original?q={q}")) { (original.source) }
                        " \u{b7} " (original.bytes) " bytes \u{b7} "
                        code { (original.digest) }
                    }
                }
            }
            @for form in &forms { (form) }
            p { "Continue creates a separately reviewed native turn. Reconcile advances observation of the original job under its current admission. Review, apply artifacts, and publish remain separate native actions." }
            p { "Candidate review, artifact application, and publication are unavailable without their separately admitted native candidate owner. Leaving this page detaches observation and does not stop work." }
            (connection_links(&context))
        }
        .into_string(),
    );
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
    html! {
        h2 { "Original native source bytes" }
        p {
            "Byte offset " (offset) " \u{b7} chunk " (bytes.len()) " bytes \u{b7} original " (total)
            " bytes \u{b7} " code { (digest) }
            ". Chunks retain the original bytes and may split a UTF-8 character. Download this chunk to preserve its exact bytes."
        }
        p { a href=(format!("{url}&download=yes")) { "Download these exact bytes" } }
        pre { (display) }
    }
    .into_string()
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
        content.push_str(
            &html! {
                p { a href=(format!("{}/original?q={next}", project_url(&context, &project))) { "Next original chunk" } }
            }
            .into_string(),
        );
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
        content.push_str(
            &html! {
                p { a href=(format!("{}/original?q={next}", job_url(context.binding.id(), &chunk.scope))) { "Next original chunk" } }
            }
            .into_string(),
        );
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
