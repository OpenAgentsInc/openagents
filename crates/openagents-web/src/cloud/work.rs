//! Canonical task observation through an explicitly bound resident host.

use super::hosts::{Binding, Hosts};
use super::session::{CloudSession, SessionError, Viewer};
use super::{protect, refused, service, standing_value, ui, workspace_shell};
use crate::App;
use axum::Router;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::response::{Html, IntoResponse, Json, Redirect, Response, Sse, sse::Event};
use axum::routing::get;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use coder_access::protocol::{Operation, Outcome};
use coder_access::task_read::{self, ListQuery, OriginalQuery, PageQuery, Scope};
use coder_ui::observation;
use futures_util::stream;
use maud::{PreEscaped, html};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{convert::Infallible, time::Duration};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/hosts/{binding}/tasks", get(tasks))
        .route("/cloud/app/hosts/{binding}/tasks/{task}", get(task))
        .route(
            "/cloud/app/hosts/{binding}/tasks/{task}/original",
            get(original),
        )
        .route("/cloud/app/hosts/{binding}/standing", get(standing))
        .route("/cloud/app/hosts/{binding}/watch", get(watch))
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Alias {
    host: Option<String>,
    cursor: Option<String>,
}

pub(super) async fn task_alias(
    State(app): State<App>,
    headers: HeaderMap,
    Path(task): Path<String>,
    query: Result<Query<Alias>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return refused(SessionError::InvalidRequest);
    };
    let Some(host) = query.host else {
        return refused(SessionError::InvalidRequest);
    };
    if !identifier(&host) || !task_id(&task) {
        return refused(SessionError::InvalidRequest);
    }
    let (_, viewer, _) = match admitted(&app, &headers, &host).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let _ = viewer;
    let suffix = match query.cursor {
        Some(cursor)
            if cursor.len() <= 8192
                && cursor
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)) =>
        {
            format!("?cursor={cursor}")
        }
        Some(_) => return refused(SessionError::InvalidRequest),
        None => String::new(),
    };
    protect(Redirect::to(&format!("/cloud/app/hosts/{host}/tasks/{task}{suffix}")).into_response())
}

async fn admitted<'a>(
    app: &'a App,
    headers: &HeaderMap,
    id: &str,
) -> Result<(&'a CloudSession, Viewer, &'a Binding), Response> {
    if !identifier(id) {
        return Err(refused(SessionError::InvalidRequest));
    }
    let service = service(app)?;
    let viewer = service.authenticate(headers).await.map_err(refused)?;
    let hosts = app
        .config
        .cloud_hosts
        .as_ref()
        .ok_or_else(|| refused(SessionError::Unavailable))?;
    let binding = hosts.get(&viewer, id).map_err(refused)?;
    Ok((service, viewer, binding))
}

async fn read(
    service: &CloudSession,
    headers: &HeaderMap,
    binding: &Binding,
    viewer: &Viewer,
    operation: Operation,
) -> Result<Outcome, Response> {
    let original = authority_value(viewer);
    let answer = binding.read(viewer, operation).await.map_err(refused)?;
    let current = service.authenticate(headers).await.map_err(refused)?;
    if authority_value(&current) != original {
        return Err(refused(SessionError::Conflict));
    }
    Ok(answer)
}

fn encode(value: &impl Serialize) -> Result<String, SessionError> {
    let bytes = serde_json::to_vec(value).map_err(|_| SessionError::InvalidRequest)?;
    if bytes.len() > 4096 {
        return Err(SessionError::InvalidRequest);
    }
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn decode<T: DeserializeOwned>(value: &str) -> Result<T, SessionError> {
    if value.len() > 8192 {
        return Err(SessionError::InvalidRequest);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| SessionError::InvalidRequest)?;
    if bytes.len() > 4096 {
        return Err(SessionError::InvalidRequest);
    }
    serde_json::from_slice(&bytes).map_err(|_| SessionError::InvalidRequest)
}

fn identity(binding: &Binding, pin: &Value) -> String {
    let bytes = json!({"binding":binding.identity(),"pin":pin}).to_string();
    format!(
        "sha256:{}",
        Sha256::digest(bytes.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

pub(super) fn authority_value(viewer: &Viewer) -> Value {
    let mut authority = standing_value(viewer);
    authority
        .as_object_mut()
        .expect("standing is an object")
        .remove("expires_at");
    authority
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub(super) fn task_id(value: &str) -> bool {
    coder_access::studio::id(value).is_ok()
}

// The native read DTOs below retain original owner identities and bounded pages.

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PageInput {
    cursor: Option<String>,
    #[serde(default)]
    fragment: bool,
    pin: Option<String>,
    identity: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Pin {
    List {},
    Task {
        query: PageQuery,
        scope: Scope,
        source: Option<String>,
    },
    Original {
        query: OriginalQuery,
    },
    Control {
        control: ControlPin,
    },
    OwnerRead {
        read: OwnerReadPin,
        control: Option<ControlPin>,
    },
    Workbench {
        configuration_digest: String,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerReadPin {
    /// Checked against the closed project and operator observation operations.
    operation: Operation,
    expected_digest: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlPin {
    scope_digest: String,
    enrolled: bool,
    request: Option<RequestPin>,
    task: Option<ControlTaskPin>,
    queue_digest: Option<String>,
    review: Option<ReviewPin>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestPin {
    id: String,
    packet_digest: String,
    state: super::effects::State,
    snapshot_digest: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlTaskPin {
    query: PageQuery,
    scope: Scope,
    source: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewPin {
    task: String,
    base: String,
    head_commit: String,
    head: String,
}

fn metadata_digest(value: &Value) -> String {
    format!("sha256:{:x}", Sha256::digest(value.to_string().as_bytes()))
}

fn request_pin(snapshot: &super::effects::Snapshot) -> RequestPin {
    RequestPin {
        id: snapshot.id.clone(),
        packet_digest: snapshot.packet_digest.clone(),
        state: snapshot.state,
        snapshot_digest: metadata_digest(&json!({
            "id":snapshot.id,"packet":snapshot.packet_digest,"state":snapshot.state,
            "action":snapshot.action,"expires_at":snapshot.expires_at,
            "outcome":snapshot.outcome,"refusal":snapshot.refusal,
            "failure":snapshot.failure.map(|failure|failure.code())
        })),
    }
}

fn pin_identity(binding: &Binding, pin: &Pin, viewer: &Viewer) -> String {
    let value = match pin {
        Pin::List {} => json!({"kind":"list"}),
        Pin::Task { scope, source, .. } => json!({"kind":"task","scope":scope,"source":source}),
        Pin::Original { query } => {
            json!({"kind":"original","scope":query.scope,"original":query.original})
        }
        Pin::Control { control } => json!({"kind":"control","control":control}),
        Pin::OwnerRead { read, control } => {
            json!({"kind":"owner_read","read":read,"control":control})
        }
        Pin::Workbench {
            configuration_digest,
        } => json!({"kind":"workbench","configuration_digest":configuration_digest}),
    };
    identity(
        binding,
        &json!({"resource":value,"account_standing":authority_value(viewer)}),
    )
}

fn resource(binding: &Binding, pin: &Pin, viewer: &Viewer) -> Result<Value, SessionError> {
    let encoded = encode(pin)?;
    let endpoint = format!("/cloud/app/hosts/{}/standing?pin={encoded}", binding.id());
    if endpoint.len() > 4096 {
        return Err(SessionError::InvalidRequest);
    }
    Ok(json!({"endpoint":endpoint,"identity":pin_identity(binding,pin,viewer)}))
}

fn owner_workspace(operation: &Operation) -> Result<&str, SessionError> {
    match operation {
        Operation::ProjectList { workspace } => Ok(workspace),
        Operation::ProjectRead { query } => Ok(&query.workspace),
        Operation::ProjectOriginal { query } => Ok(&query.workspace),
        Operation::CloudProjects { workspace } => Ok(workspace),
        Operation::CloudCatalog { query } => Ok(&query.workspace),
        Operation::CloudList { query } => Ok(&query.workspace),
        Operation::CloudRead { query } => Ok(&query.workspace),
        Operation::CloudOriginal { query } => Ok(&query.scope.workspace),
        Operation::EnvironmentRead { query } => Ok(&query.workspace),
        Operation::EnvironmentEvidence { query } => Ok(&query.workspace),
        _ => Err(SessionError::InvalidRequest),
    }
}

fn owner_digest(operation: &Operation, outcome: &Outcome) -> Result<String, SessionError> {
    owner_workspace(operation)?;
    if operation.validate().is_err() || outcome.validate().is_err() || !outcome.answers(operation) {
        return Err(SessionError::Conflict);
    }
    Ok(metadata_digest(&json!(outcome)))
}

/// Pin one original native project or operator read without copying its body
/// into a URL. An effectful page also pins the current browser enrollment.
pub(super) fn owner_resource(
    binding: &Binding,
    viewer: &Viewer,
    hosts: Option<&Hosts>,
    operation: &Operation,
    outcome: &Outcome,
) -> Result<Value, SessionError> {
    if owner_workspace(operation)? != binding.workspace() {
        return Err(SessionError::InvalidRequest);
    }
    owner_digest(operation, outcome)?;
    let mut operation = operation.clone();
    match (&mut operation, outcome) {
        (Operation::ProjectRead { query }, Outcome::ProjectRead { project }) => {
            query.snapshot = Some(project.snapshot_digest.clone());
        }
        (Operation::CloudRead { query }, Outcome::CloudRead { job }) => {
            query.revision = Some(job.scope.revision.clone());
        }
        _ => {}
    }
    let expected_digest = owner_digest(&operation, outcome)?;
    let control = hosts
        .map(|hosts| -> Result<ControlPin, SessionError> {
            let scope = hosts.control_scope(viewer, binding);
            let enrolled = hosts
                .effects(viewer, binding.id())?
                .enrolled(&scope, binding.identity())?;
            Ok(ControlPin {
                scope_digest: metadata_digest(&scope),
                enrolled,
                request: None,
                task: None,
                queue_digest: None,
                review: None,
            })
        })
        .transpose()?;
    resource(
        binding,
        &Pin::OwnerRead {
            read: OwnerReadPin {
                operation,
                expected_digest,
            },
            control,
        },
        viewer,
    )
}

async fn owner_standing(
    app: &App,
    headers: &HeaderMap,
    service: &CloudSession,
    binding: &Binding,
    viewer: &Viewer,
    pin: &Pin,
    original: &OwnerReadPin,
    control: Option<&ControlPin>,
) -> Result<Response, Response> {
    if owner_workspace(&original.operation).map_err(refused)? != binding.workspace()
        || original.operation.validate().is_err()
        || coder_access::cloud::digest(&original.expected_digest).is_err()
        || matches!(&original.operation, Operation::ProjectRead { query } if query.snapshot.is_none())
        || matches!(&original.operation, Operation::CloudRead { query } if query.revision.is_none())
    {
        return Err(refused(SessionError::InvalidRequest));
    }
    let hosts = control
        .map(|control| {
            if control.request.is_some()
                || control.task.is_some()
                || control.queue_digest.is_some()
                || control.review.is_some()
            {
                return Err(refused(SessionError::InvalidRequest));
            }
            let hosts = app
                .config
                .cloud_hosts
                .as_deref()
                .ok_or_else(|| refused(SessionError::Unavailable))?;
            check_control(hosts, binding, viewer, control).map_err(refused)?;
            Ok(hosts)
        })
        .transpose()?;
    let result = read(
        service,
        headers,
        binding,
        viewer,
        original.operation.clone(),
    )
    .await?;
    if owner_digest(&original.operation, &result).map_err(refused)? != original.expected_digest {
        return Err(refused(SessionError::Conflict));
    }
    let current = service.authenticate(headers).await.map_err(refused)?;
    if authority_value(&current) != authority_value(viewer) {
        return Err(refused(SessionError::Conflict));
    }
    if let (Some(hosts), Some(control)) = (hosts, control) {
        check_control(hosts, binding, &current, control).map_err(refused)?;
    }
    Ok(protect(
        Json(json!({"active":true,"identity":pin_identity(binding,pin,&current)})).into_response(),
    ))
}

/// Bind a control page to current enrollment and original native evidence.
pub(super) fn control_resource(
    binding: &Binding,
    viewer: &Viewer,
    hosts: &Hosts,
    snapshot: Option<&super::effects::Snapshot>,
    page: Option<&task_read::Page>,
    queue_digest: Option<&str>,
    review: Option<&coder_access::review::TaskReview>,
) -> Result<Value, SessionError> {
    let scope = hosts.control_scope(viewer, binding);
    let book = hosts.effects(viewer, binding.id())?;
    let enrolled = book.enrolled(&scope, binding.identity())?;
    let request = snapshot.map(request_pin);
    if let Some(expected) = &request
        && request_pin(&book.lookup(&scope, &expected.id)?) != *expected
    {
        return Err(SessionError::Conflict);
    }
    let task = page
        .map(|page| {
            page.validate().map_err(|_| SessionError::InvalidRequest)?;
            if page.scope.workspace != binding.workspace() {
                return Err(SessionError::InvalidRequest);
            }
            Ok(ControlTaskPin {
                query: PageQuery {
                    workspace: page.scope.workspace.clone(),
                    task: page.scope.task.clone(),
                    revision: Some(page.scope.revision),
                    cursor: page.next.clone(),
                    limit: 1,
                },
                scope: page.scope.clone(),
                source: page
                    .evidence
                    .original
                    .as_ref()
                    .map(|source| source.source.clone()),
            })
        })
        .transpose()?;
    if queue_digest.is_some() && task.is_none()
        || review.is_some_and(|review| {
            review.validate().is_err()
                || task
                    .as_ref()
                    .is_none_or(|task| task.scope.task != review.task)
        })
    {
        return Err(SessionError::InvalidRequest);
    }
    let control = ControlPin {
        scope_digest: metadata_digest(&scope),
        enrolled,
        request,
        task,
        queue_digest: queue_digest.map(str::to_owned),
        review: review.map(|review| ReviewPin {
            task: review.task.clone(),
            base: review.base.clone(),
            head_commit: review.head_commit.clone(),
            head: review.head.clone(),
        }),
    };
    resource(binding, &Pin::Control { control }, viewer)
}

fn check_control(
    hosts: &Hosts,
    binding: &Binding,
    viewer: &Viewer,
    control: &ControlPin,
) -> Result<(), SessionError> {
    let scope = hosts.control_scope(viewer, binding);
    if metadata_digest(&scope) != control.scope_digest {
        return Err(SessionError::Conflict);
    }
    let book = hosts.effects(viewer, binding.id())?;
    if book.enrolled(&scope, binding.identity())? != control.enrolled {
        return Err(SessionError::Conflict);
    }
    if let Some(request) = &control.request {
        let snapshot = book.lookup(&scope, &request.id)?;
        if request_pin(&snapshot) != *request {
            return Err(SessionError::Conflict);
        }
        if snapshot
            .action
            .required()
            .is_none_or(|right| !binding.access().grant.rights.contains(right))
        {
            return Err(SessionError::Forbidden);
        }
    }
    Ok(())
}

async fn control_standing(
    app: &App,
    headers: &HeaderMap,
    service: &CloudSession,
    binding: &Binding,
    viewer: &Viewer,
    pin: &Pin,
    control: &ControlPin,
) -> Result<Response, Response> {
    let hosts = app
        .config
        .cloud_hosts
        .as_deref()
        .ok_or_else(|| refused(SessionError::Unavailable))?;
    check_control(hosts, binding, viewer, control).map_err(refused)?;
    if let Some(task) = &control.task {
        if task.query.workspace != binding.workspace()
            || task.scope.workspace != binding.workspace()
            || task.query.task != task.scope.task
            || task.query.revision != Some(task.scope.revision)
            || task.query.limit != 1
            || task.query.validate().is_err()
        {
            return Err(refused(SessionError::InvalidRequest));
        }
        let result = read(
            service,
            headers,
            binding,
            viewer,
            Operation::ReadTask {
                query: task.query.clone(),
            },
        )
        .await?;
        if !matches!(result, Outcome::Task { task: page } if page.answers(&task.query)
            && page.scope == task.scope
            && page.evidence.original.as_ref().map(|source| &source.source) == task.source.as_ref())
        {
            return Err(refused(SessionError::Conflict));
        }
    } else {
        if control.queue_digest.is_some() || control.review.is_some() {
            return Err(refused(SessionError::InvalidRequest));
        }
        let query = ListQuery {
            workspace: binding.workspace().into(),
            cursor: None,
            limit: 1,
        };
        let result = read(
            service,
            headers,
            binding,
            viewer,
            Operation::ListTasks {
                query: query.clone(),
            },
        )
        .await?;
        if !matches!(result, Outcome::Tasks { tasks } if tasks.answers(&query)) {
            return Err(refused(SessionError::Conflict));
        }
    }
    if let Some(expected) = &control.queue_digest {
        let task = control
            .task
            .as_ref()
            .ok_or_else(|| refused(SessionError::InvalidRequest))?;
        let operation = Operation::QueueTaskAtRevision {
            task: task.scope.task.clone(),
            revision: task.scope.revision,
            edit: coder_access::protocol::QueueEdit::List {},
            queue_digest: None,
        };
        let result = binding
            .read_queue(viewer, operation.clone())
            .await
            .map_err(refused)?;
        if !result.answers(&operation)
            || !matches!(result, Outcome::QueueAtRevision { queue_digest, .. } if queue_digest == *expected)
        {
            return Err(refused(SessionError::Conflict));
        }
    }
    if let Some(review) = &control.review {
        let task = control
            .task
            .as_ref()
            .ok_or_else(|| refused(SessionError::InvalidRequest))?;
        if review.task != task.scope.task {
            return Err(refused(SessionError::InvalidRequest));
        }
        let operation = Operation::ReviewTask {
            task: review.task.clone(),
        };
        let result = read(service, headers, binding, viewer, operation.clone()).await?;
        if !result.answers(&operation)
            || !matches!(result, Outcome::Review { review: current } if current.base == review.base
                && current.head_commit == review.head_commit && current.head == review.head)
        {
            return Err(refused(SessionError::Conflict));
        }
    }
    if let Some(request) = &control.request {
        let scope = hosts.control_scope(viewer, binding);
        let snapshot = hosts
            .effects(viewer, binding.id())
            .and_then(|book| book.lookup(&scope, &request.id))
            .map_err(refused)?;
        // A reviewed packet can precede the native request book. Its private
        // projection still requires current operator source and profile policy.
        super::operator::admit_action(binding, viewer, &snapshot.action)
            .await
            .map_err(refused)?;
        let operation = Operation::RequestOperation {
            request: request.id.clone(),
            request_event: request.packet_digest.clone(),
        };
        let result = read(service, headers, binding, viewer, operation.clone()).await?;
        if !result.answers(&operation) {
            return Err(refused(SessionError::Conflict));
        }
        let Outcome::RequestOperation { result, .. } = result else {
            return Err(refused(SessionError::Conflict));
        };
        let same_source = match result.as_deref() {
            Some(coder_access::protocol::ReplyResult::Ok { outcome }) => {
                snapshot.state == super::effects::State::Answered
                    && snapshot.outcome.as_ref() == Some(outcome)
                    && snapshot.refusal.is_none()
            }
            Some(coder_access::protocol::ReplyResult::Refused { code, missing }) => {
                let state = if matches!(
                    code,
                    coder_access::Code::Unavailable | coder_access::Code::Transport
                ) {
                    super::effects::State::Unknown
                } else {
                    super::effects::State::Refused
                };
                snapshot.state == state
                    && snapshot
                        .refusal
                        .as_ref()
                        .is_some_and(|refusal| refusal.code == *code && refusal.missing == *missing)
                    && snapshot.outcome.is_none()
            }
            None => {
                matches!(
                    snapshot.state,
                    super::effects::State::Prepared | super::effects::State::Unknown
                ) && snapshot.outcome.is_none()
                    && snapshot.refusal.is_none()
            }
        };
        if !same_source {
            return Err(refused(SessionError::Conflict));
        }
    }
    let current = service.authenticate(headers).await.map_err(refused)?;
    if authority_value(&current) != authority_value(viewer) {
        return Err(refused(SessionError::Conflict));
    }
    check_control(hosts, binding, &current, control).map_err(refused)?;
    Ok(protect(
        Json(json!({"active":true,"identity":pin_identity(binding,pin,&current)})).into_response(),
    ))
}

pub(super) fn list_resource(binding: &Binding, viewer: &Viewer) -> Result<Value, SessionError> {
    resource(binding, &Pin::List {}, viewer)
}

pub(super) fn workbench_resource(
    binding: &Binding,
    viewer: &Viewer,
    configuration_digest: String,
) -> Result<Value, SessionError> {
    resource(
        binding,
        &Pin::Workbench {
            configuration_digest,
        },
        viewer,
    )
}

pub(super) fn task_resource(
    binding: &Binding,
    page: &task_read::Page,
    viewer: &Viewer,
) -> Result<Value, SessionError> {
    resource(
        binding,
        &Pin::Task {
            query: PageQuery {
                workspace: page.scope.workspace.clone(),
                task: page.scope.task.clone(),
                revision: Some(page.scope.revision),
                cursor: page.next.clone(),
                limit: 1,
            },
            scope: page.scope.clone(),
            source: page
                .evidence
                .original
                .as_ref()
                .map(|original| original.source.clone()),
        },
        viewer,
    )
}

fn show(view: &rust_native::View<observation::ObservationIntent>) -> Result<String, Response> {
    rust_native_web::render_view(view).map_err(|_| refused(SessionError::Conflict))
}

fn source<'a>(
    binding: &'a Binding,
    scope: &'a Scope,
    attempt: Option<&'a str>,
    revision: &'a str,
    key: &'a str,
) -> observation::Source<'a> {
    observation::Source {
        key,
        host: binding.host(),
        workspace: &scope.workspace,
        task: &scope.task,
        attempt,
        revision,
    }
}

async fn tasks(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Query<PageInput>, QueryRejection>,
) -> Response {
    let Ok(Query(input)) = input else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer, binding) = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = verify_fragment(service, &headers, binding, &viewer, &input).await {
        return response;
    }
    let cursor = match input
        .cursor
        .as_deref()
        .map(decode::<task_read::ListCursor>)
        .transpose()
    {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let query = ListQuery {
        workspace: binding.workspace().into(),
        cursor,
        limit: 32,
    };
    if query.validate().is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let answer = match read(
        service,
        &headers,
        binding,
        &viewer,
        Operation::ListTasks {
            query: query.clone(),
        },
    )
    .await
    {
        Ok(Outcome::Tasks { tasks }) if tasks.answers(&query) => tasks,
        Ok(_) => return refused(SessionError::Conflict),
        Err(response) => return response,
    };
    let mut rows = Vec::with_capacity(answer.rows.len());
    for (index, row) in answer.rows.iter().enumerate() {
        let attempt = row.attempt.map(|v| v.to_string());
        let revision = row.revision.to_string();
        let phase = format!("{:?}", row.phase);
        let key = format!("task-row-{index}");
        let view = observation::task(
            &observation::Task {
                source: observation::Source {
                    key: &key,
                    host: binding.host(),
                    workspace: &answer.workspace,
                    task: &row.task,
                    attempt: attempt.as_deref(),
                    revision: &revision,
                },
                title: &row.title,
                status: &phase,
                execution: "Open canonical task evidence",
                checks: "Open canonical task evidence",
                delivery: "Unknown in list",
                integration: "Unknown in list",
                stop: "Unknown in list",
                cleanup: "Unknown in list",
                cost: "Unknown in list",
            },
            super::colors(),
        );
        match show(&view) {
            Ok(view) => rows.push(ui::card(html! {
                (ui::native(&view))
                p { a href=(format!("/cloud/app/hosts/{}/tasks/{}", binding.id(), row.task)) { "Open task" } }
            })),
            Err(response) => return response,
        }
    }
    let mut next = None;
    if answer.more_available
        && let Some(cursor) = &answer.next
    {
        let encoded = match encode(cursor) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        next = Some(format!(
            "/cloud/app/hosts/{}/tasks?cursor={encoded}",
            binding.id()
        ));
    }
    let content = html! {
        h2 { "Resident tasks" }
        (ui::Details::new()
            .row("Host", html! { code { (binding.host()) } })
            .row("Generation", binding.generation())
            .row("Workspace", html! { code { (answer.workspace) } })
            .row("Snapshot", html! { code { (answer.snapshot_digest) } })
            .row("Read at", super::session::now()))
        p { "Closing this view detaches observation." }
        p { a href=(format!("/cloud/app/hosts/{}/tasks", binding.id())) { "Refresh tasks" } }
        @if answer.rows.is_empty() {
            (ui::empty("No tasks", "No tasks in this admitted workspace."))
        }
        @for row in &rows { (row) }
        @if let Some(next) = &next {
            p { a href=(next) { "Next tasks" } }
        }
    }
    .into_string();
    observed(
        &app,
        &headers,
        service,
        &viewer,
        binding,
        &input,
        &Pin::List {},
        "cloud-list-content",
        &format!("/cloud/app/hosts/{}/tasks", binding.id()),
        &content,
    )
}

async fn task(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    input: Result<Query<PageInput>, QueryRejection>,
) -> Response {
    if !task_id(&task) {
        return refused(SessionError::InvalidRequest);
    }
    let Ok(Query(input)) = input else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer, binding) = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(response) = verify_fragment(service, &headers, binding, &viewer, &input).await {
        return response;
    }
    let cursor = match input
        .cursor
        .as_deref()
        .map(decode::<task_read::Cursor>)
        .transpose()
    {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    let query = PageQuery {
        workspace: binding.workspace().into(),
        task: task.clone(),
        revision: cursor.as_ref().map(|c| c.scope.revision),
        cursor,
        limit: 24,
    };
    if query.validate().is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let page = match read(
        service,
        &headers,
        binding,
        &viewer,
        Operation::ReadTask {
            query: query.clone(),
        },
    )
    .await
    {
        Ok(Outcome::Task { task }) if task.answers(&query) => task,
        Ok(_) => return refused(SessionError::Conflict),
        Err(r) => return r,
    };
    let reviewable = coder_access::protocol::identity(&task).is_ok();
    let controllable = reviewable
        && app
            .config
            .cloud_hosts
            .as_ref()
            .is_some_and(|hosts| hosts.effects(&viewer, &id).is_ok());
    let base = format!("/cloud/app/hosts/{id}/tasks/{task}");
    let list = format!("/cloud/app/hosts/{id}/tasks");
    let mut parts: Vec<maud::Markup> = vec![html! {
        (ui::links([
            (list.as_str(), "Resident tasks"),
            (base.as_str(), "Reopen this task"),
        ]))
        p {
            "Resident generation " (binding.generation())
            ". Bounded original evidence refreshes while this admission remains current. Reopen the task after its source or revision changes."
        }
        @if reviewable {
            p { a href=(format!("{base}/review")) { "Read exact candidate review" } }
            @if controllable {
                p { a href=(format!("{base}/actions")) { "Review granted task controls" } }
            }
        }
    }];
    let attempt = page.scope.attempt.map(|v| v.to_string());
    let revision = page.scope.revision.to_string();
    let phase = format!("{:?}", page.phase);
    let cost = page.cost_microusd.map_or_else(
        || format!("Unknown · {}", page.cost_status),
        |v| format!("{} USD · {}", v as f64 / 1_000_000.0, page.cost_status),
    );
    let task_source = source(
        binding,
        &page.scope,
        attempt.as_deref(),
        &revision,
        "canonical-task",
    );
    let summary = observation::task(
        &observation::Task {
            source: task_source,
            title: &page.title,
            status: &phase,
            execution: &page.execution,
            checks: &page.verification,
            delivery: &page.delivery,
            integration: &page.integration,
            stop: &page.termination,
            cleanup: &page.cleanup,
            cost: &cost,
        },
        super::colors(),
    );
    match show(&summary) {
        Ok(v) => parts.push(ui::native(&v)),
        Err(r) => return r,
    }
    parts.push(html! {
        h3 { "Request" }
        @if page.prompt.is_empty() {
            p { "No inline request is included. Read the canonical task journal below for the original request." }
        } @else {
            pre { (page.prompt) }
        }
    });
    let description = format!(
        "{} original steps; {} faults included{}.",
        page.evidence.total_steps,
        page.evidence.faults.len(),
        if page.evidence.more_faults {
            "; more faults retained in original"
        } else {
            ""
        }
    );
    let evidence = observation::evidence(
        &observation::Evidence {
            source: task_source,
            label: "Transcript evidence",
            state: &page.evidence.state,
            summary: &description,
        },
        super::colors(),
    );
    match show(&evidence) {
        Ok(v) => parts.push(ui::native(&v)),
        Err(r) => return r,
    }
    for step in &page.evidence.steps {
        match step {
            task_read::Step::Original { index, step } => {
                let projected = observation::Step {
                    source: task_source,
                    index: *index,
                    label: "Original ATIF record",
                    record: step,
                };
                let view = observation::step(&projected, super::colors());
                match show(&view) {
                    Ok(v) => parts.push(ui::card(ui::native(&v))),
                    Err(r) => return r,
                }
                match show(&observation::original_step(&projected, super::colors())) {
                    Ok(v) => parts.push(html! {
                        details {
                            summary { "Original step " (index) }
                            (ui::native(&v))
                        }
                    }),
                    Err(r) => return r,
                }
            }
            task_read::Step::Gap {
                index, original, ..
            } => {
                let url = match original_url(binding, &page.scope, original) {
                    Ok(v) => v,
                    Err(e) => return refused(e),
                };
                parts.push(ui::card(html! {
                    h3 { "Step " (index) " \u{b7} Oversized record" }
                    p { "The original is retained. " a href=(url) { "Read bounded original chunks" } "." }
                }));
            }
        }
    }
    if page.more_available
        && let Some(cursor) = &page.next
    {
        let encoded = match encode(cursor) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        parts.push(html! {
            p { a href=(format!("{base}?cursor={encoded}")) { "Next original steps" } }
        });
    }
    let transcript = match &page.evidence.original {
        Some(original) => match original_url(binding, &page.scope, original) {
            Ok(url) => Some((url, original.digest.clone())),
            Err(e) => return refused(e),
        },
        None => None,
    };
    let mut artifacts = Vec::with_capacity(page.artifacts.len());
    for artifact in &page.artifacts {
        let original = match &artifact.original {
            Some(original) => match original_url(binding, &page.scope, original) {
                Ok(url) => Some((url, original.bytes, original.digest.clone())),
                Err(e) => return refused(e),
            },
            None => None,
        };
        artifacts.push((artifact, original));
    }
    parts.push(html! {
        h3 { "Child references" }
        p { "Child references retain their original parent step and call. A reference grants no child task, session, or control access." }
        @if page.children.is_empty() {
            p { "No structured child references are included in this view." }
        }
        @for child in &page.children {
            (ui::card(html! {
                p {
                    "Parent step " (child.source_step) " \u{b7} Call " code { (child.call_id) }
                    " \u{b7} Agent " (child.agent)
                }
                p {
                    "Child reference: " code { (child.reference.as_deref().unwrap_or("Unavailable")) }
                    " \u{b7} " (child.state)
                }
            }))
        }
        @if page.more_children {
            p { "Additional child references remain in the original transcript." }
        }
        h3 { "Artifacts and original records" }
        p { "These are resident evidence. An executor ending, checks passing, delivery, integration, stop, and cleanup remain separate." }
        @if let Some((url, digest)) = &transcript {
            p { a href=(url) { "Original transcript" } " \u{b7} " code { (digest) } }
        }
        @for (artifact, original) in &artifacts {
            p { (artifact.label) " \u{b7} " (artifact.state) }
            @if let Some((url, bytes, digest)) = original {
                p {
                    a href=(url) { "Read original artifact" }
                    " \u{b7} " (bytes) " bytes \u{b7} " code { (digest) }
                }
            }
        }
    });
    let content = html! { @for part in &parts { (part) } }.into_string();
    let pin = Pin::Task {
        query: PageQuery {
            workspace: page.scope.workspace.clone(),
            task: page.scope.task.clone(),
            revision: Some(page.scope.revision),
            cursor: page.next.clone(),
            limit: 1,
        },
        scope: page.scope.clone(),
        source: page.evidence.original.as_ref().map(|o| o.source.clone()),
    };
    observed(
        &app,
        &headers,
        service,
        &viewer,
        binding,
        &input,
        &pin,
        "cloud-task-content",
        &format!("/cloud/app/hosts/{}/tasks/{task}", binding.id()),
        &content,
    )
}

/// Keep the admitted shell and all reviewed forms outside the observation swap.
async fn verify_fragment(
    service: &CloudSession,
    headers: &HeaderMap,
    binding: &Binding,
    viewer: &Viewer,
    input: &PageInput,
) -> Result<(), Response> {
    if !input.fragment {
        return Ok(());
    }
    let (encoded, identity) = input
        .pin
        .as_deref()
        .zip(input.identity.as_deref())
        .ok_or_else(|| refused(SessionError::InvalidRequest))?;
    let pin: Pin = decode(encoded).map_err(refused)?;
    if pin_identity(binding, &pin, viewer) != identity {
        return Err(refused(SessionError::Conflict));
    }
    let operation = watch_operation(binding, &pin).map_err(refused)?;
    // Recheck the original source prefix before reading a newer projection.
    let answer = read(service, headers, binding, viewer, operation.clone()).await?;
    if !watch_answer(&pin, &operation, &answer) {
        return Err(refused(SessionError::Conflict));
    }
    Ok(())
}

/// Project a read without replacing its original admission descriptor.
#[allow(clippy::too_many_arguments)]
fn observed(
    app: &App,
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    binding: &Binding,
    input: &PageInput,
    pin: &Pin,
    element: &str,
    path: &str,
    content: &str,
) -> Response {
    let identity = pin_identity(binding, pin, viewer);
    if input.fragment {
        let Some((encoded, expected)) = input.pin.as_deref().zip(input.identity.as_deref()) else {
            return refused(SessionError::InvalidRequest);
        };
        let original: Pin = match decode(encoded) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        if watch_operation(binding, &original).is_err()
            || pin_identity(binding, &original, viewer) != identity
            || expected != identity
        {
            return refused(SessionError::Conflict);
        }
        let mut response = protect(Html(content.to_owned()).into_response());
        let standing = json!({"active":true,"identity":identity}).to_string();
        if let Ok(value) = HeaderValue::from_str(&standing) {
            response
                .headers_mut()
                .insert("x-openagents-resource", value);
        }
        return response;
    }
    if input.pin.is_some() || input.identity.is_some() {
        return refused(SessionError::InvalidRequest);
    }
    let encoded = match encode(pin) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let resource = match resource(binding, pin, viewer) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let cursor = input
        .cursor
        .as_deref()
        .map_or_else(String::new, |cursor| format!("&cursor={cursor}"));
    let refresh = format!("{path}?fragment=true&pin={encoded}&identity={identity}{cursor}");
    let watch = format!(
        "/cloud/app/hosts/{}/watch?pin={encoded}&identity={identity}",
        binding.id()
    );
    let content = html! {
        section id="cloud-observation" hx-ext="sse" sse-connect=(watch) sse-close="retire" {
            div id="cloud-live-status" sse-swap="gap,retire" hx-swap="innerHTML" aria-live="polite" {
                p class="dim" { "Observing canonical records. A changed source or admission requires a fresh view." }
            }
            div id=(element) hx-get=(refresh) hx-trigger="sse:refresh,sse:gap"
                hx-target="this" hx-swap="innerHTML" hx-sync="this:drop" {
                (PreEscaped(content))
            }
        }
    };
    workspace_shell(
        app,
        headers,
        service,
        viewer,
        "tasks",
        Some(content),
        Some(resource),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WatchInput {
    pin: String,
    identity: String,
}

fn watch_operation(binding: &Binding, pin: &Pin) -> Result<Operation, SessionError> {
    match pin {
        Pin::List {} => Ok(Operation::ListTasks {
            query: ListQuery {
                workspace: binding.workspace().into(),
                cursor: None,
                limit: 1,
            },
        }),
        Pin::Task { query, scope, .. }
            if query.workspace == binding.workspace()
                && scope.workspace == binding.workspace()
                && query.task == scope.task
                && query.revision == Some(scope.revision)
                && query.limit == 1
                && query.validate().is_ok()
                && scope.validate().is_ok() =>
        {
            Ok(Operation::ReadTask {
                query: query.clone(),
            })
        }
        _ => Err(SessionError::InvalidRequest),
    }
}

fn watch_answer(pin: &Pin, operation: &Operation, answer: &Outcome) -> bool {
    match (pin, operation, answer) {
        (Pin::List {}, Operation::ListTasks { query }, Outcome::Tasks { tasks }) => {
            tasks.validate().is_ok() && tasks.answers(query)
        }
        (
            Pin::Task { scope, source, .. },
            Operation::ReadTask { query },
            Outcome::Task { task },
        ) => {
            task.validate().is_ok()
                && task.answers(query)
                && task.scope == *scope
                && task
                    .evidence
                    .original
                    .as_ref()
                    .map(|original| &original.source)
                    == source.as_ref()
        }
        _ => false,
    }
}

struct Watch {
    app: App,
    headers: HeaderMap,
    binding: String,
    pin: Pin,
    identity: String,
    previous: Option<String>,
    first: bool,
    reads: u16,
    ended: bool,
}

const RETIRED: &str = "Observation stopped. Reopen this view to check current account, host, and original source standing.";

async fn watch(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Query<WatchInput>, QueryRejection>,
) -> Response {
    let Ok(Query(input)) = input else {
        return refused(SessionError::InvalidRequest);
    };
    let (_, viewer, binding) = match admitted(&app, &headers, &id).await {
        Ok(value) => value,
        Err(_) if super::reconnect(&headers) => return super::retired_stream(RETIRED),
        Err(response) => return response,
    };
    let pin: Pin = match decode(&input.pin) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if watch_operation(binding, &pin).is_err()
        || input.identity != pin_identity(binding, &pin, &viewer)
    {
        if super::reconnect(&headers) {
            return super::retired_stream(RETIRED);
        }
        return refused(SessionError::Conflict);
    }
    let prefix = format!("v1:{}:", input.identity.trim_start_matches("sha256:"));
    let previous = match headers.get("last-event-id") {
        None => None,
        Some(value) => {
            let Some(value) = value.to_str().ok().filter(|value| {
                value.strip_prefix(&prefix).is_some_and(|digest| {
                    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            }) else {
                return super::retired_stream(RETIRED);
            };
            Some(value.into())
        }
    };
    let state = Watch {
        app,
        headers,
        binding: id,
        pin,
        identity: input.identity,
        previous,
        first: true,
        reads: 0,
        ended: false,
    };
    let stream = stream::unfold(state, |mut state| async move {
        if state.ended {
            return None;
        }
        if !state.first {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        let snapshot = watch_snapshot(&state).await;
        state.reads += 1;
        let event = match snapshot {
            Some(digest) if state.reads <= 120 => {
                let id = format!(
                    "v1:{}:{}",
                    state.identity.trim_start_matches("sha256:"),
                    digest.trim_start_matches("sha256:")
                );
                if state.previous.as_ref() == Some(&id) {
                    Event::default().comment("canonical standing checked")
                } else {
                    let gap = state.first && state.previous.is_some();
                    state.previous = Some(id.clone());
                    let data = if gap {
                        html! { p class="dim" { "The canonical snapshot changed while detached. Reading the current bounded projection; original records remain available below." } }.into_string()
                    } else {
                        "Canonical snapshot changed".into()
                    };
                    Event::default()
                        .event(if gap { "gap" } else { "refresh" })
                        .id(id)
                        .data(data)
                }
            }
            _ => {
                state.ended = true;
                Event::default().event("retire").data(
                    html! {
                        p { (RETIRED) }
                    }
                    .into_string(),
                )
            }
        };
        state.first = false;
        Some((Ok::<_, Infallible>(event), state))
    });
    let mut response = protect(Sse::new(stream).into_response());
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

async fn watch_snapshot(state: &Watch) -> Option<String> {
    let (service, viewer, binding) = admitted(&state.app, &state.headers, &state.binding)
        .await
        .ok()?;
    if pin_identity(binding, &state.pin, &viewer) != state.identity {
        return None;
    }
    let operation = watch_operation(binding, &state.pin).ok()?;
    let answer = read(service, &state.headers, binding, &viewer, operation.clone())
        .await
        .ok()?;
    watch_answer(&state.pin, &operation, &answer).then(|| metadata_digest(&json!(answer)))
}

fn original_url(
    binding: &Binding,
    scope: &Scope,
    original: &task_read::Original,
) -> Result<String, SessionError> {
    let query = OriginalQuery {
        scope: scope.clone(),
        original: original.clone(),
        cursor: None,
        limit: task_read::MAX_CHUNK_BYTES as u32,
    };
    Ok(format!(
        "/cloud/app/hosts/{}/tasks/{}/original?cursor={}",
        binding.id(),
        scope.task,
        encode(&query)?
    ))
}

async fn original(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    input: Result<Query<PageInput>, QueryRejection>,
) -> Response {
    if !task_id(&task) {
        return refused(SessionError::InvalidRequest);
    }
    let Ok(Query(input)) = input else {
        return refused(SessionError::InvalidRequest);
    };
    if input.fragment || input.pin.is_some() || input.identity.is_some() {
        return refused(SessionError::InvalidRequest);
    }
    let Some(input) = input.cursor else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer, binding) = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let query: OriginalQuery = match decode(&input) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    if query.scope.workspace != binding.workspace()
        || query.scope.task != task
        || query.validate().is_err()
    {
        return refused(SessionError::InvalidRequest);
    }
    let chunk = match read(
        service,
        &headers,
        binding,
        &viewer,
        Operation::ReadTaskOriginal {
            query: query.clone(),
        },
    )
    .await
    {
        Ok(Outcome::TaskOriginal { original }) if original.answers(&query) => original,
        Ok(_) => return refused(SessionError::Conflict),
        Err(r) => return r,
    };
    let bytes = match STANDARD.decode(&chunk.data) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    let text = std::str::from_utf8(&bytes).ok();
    let attempt = chunk.scope.attempt.map(|v| v.to_string());
    let revision = chunk.scope.revision.to_string();
    let length = format!(
        "{} total; chunk starts at {}, {} bytes",
        chunk.original.bytes,
        chunk.start,
        bytes.len()
    );
    let file = observation::file(
        &observation::File {
            source: source(
                binding,
                &chunk.scope,
                attempt.as_deref(),
                &revision,
                "original-record",
            ),
            label: &chunk.original.source,
            media_type: &chunk.original.media_type,
            bytes: &length,
            digest: &chunk.original.digest,
            retention: "Resident owner policy; no expiry inferred",
            content: text,
        },
        super::colors(),
    );
    let file = match show(&file) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut next_chunk = None;
    if chunk.more_available
        && let Some(cursor) = &chunk.next
    {
        let mut next = query.clone();
        next.cursor = Some(cursor.clone());
        let encoded = match encode(&next) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        next_chunk = Some(format!(
            "/cloud/app/hosts/{id}/tasks/{task}/original?cursor={encoded}"
        ));
    }
    // A chunk can split a UTF-8 sequence. The original bytes stay available
    // without replacing invalid bytes or silently skipping them.
    let content = html! {
        p { a href=(format!("/cloud/app/hosts/{id}/tasks/{task}")) { "Canonical task" } }
        (ui::native(&file))
        details {
            summary { "Original chunk bytes (base64)" }
            pre { (chunk.data) }
        }
        @if let Some(next) = &next_chunk {
            p { a href=(next) { "Next original chunk" } }
        }
    };
    let resource = match resource(binding, &Pin::Original { query }, &viewer) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "tasks",
        Some(content),
        Some(resource),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StandingInput {
    pin: String,
}

async fn standing(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Query<StandingInput>, QueryRejection>,
) -> Response {
    let Ok(Query(input)) = input else {
        return refused(SessionError::InvalidRequest);
    };
    let (service, viewer, binding) = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let pin: Pin = match decode(&input.pin) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    if let Pin::Control { control } = &pin {
        return match control_standing(&app, &headers, service, binding, &viewer, &pin, control)
            .await
        {
            Ok(response) | Err(response) => response,
        };
    }
    if let Pin::OwnerRead { read, control } = &pin {
        return match owner_standing(
            &app,
            &headers,
            service,
            binding,
            &viewer,
            &pin,
            read,
            control.as_ref(),
        )
        .await
        {
            Ok(response) | Err(response) => response,
        };
    }
    if let Pin::Workbench {
        configuration_digest,
    } = &pin
    {
        if !super::workbench::assets_ready(&app) {
            return refused(SessionError::Unavailable);
        }
        if super::workbench::configuration_digest(binding, &viewer).as_ref()
            != Ok(configuration_digest)
        {
            return refused(SessionError::Conflict);
        }
        if let Err(error) = super::workbench::qualify(binding, &viewer).await {
            return refused(error);
        }
        let current = match service.authenticate(&headers).await {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        if authority_value(&current) != authority_value(&viewer) {
            return refused(SessionError::Conflict);
        }
        return protect(
            Json(json!({"active":true,"identity":pin_identity(binding,&pin,&current)}))
                .into_response(),
        );
    }
    let operation = match &pin {
        Pin::List {} => Operation::ListTasks {
            query: ListQuery {
                workspace: binding.workspace().into(),
                cursor: None,
                limit: 1,
            },
        },
        Pin::Task {
            query,
            scope,
            source: _,
        } => {
            if query.workspace != binding.workspace()
                || query.task != scope.task
                || query.revision != Some(scope.revision)
                || query.validate().is_err()
            {
                return refused(SessionError::InvalidRequest);
            }
            Operation::ReadTask {
                query: query.clone(),
            }
        }
        Pin::Original { query } => {
            if query.scope.workspace != binding.workspace() || query.validate().is_err() {
                return refused(SessionError::InvalidRequest);
            }
            Operation::ReadTaskOriginal {
                query: query.clone(),
            }
        }
        Pin::Control { .. } => return refused(SessionError::InvalidRequest),
        Pin::OwnerRead { .. } => return refused(SessionError::InvalidRequest),
        Pin::Workbench { .. } => return refused(SessionError::InvalidRequest),
    };
    let result = match read(service, &headers, binding, &viewer, operation).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let matches = match (&pin, result) {
        (Pin::List {}, Outcome::Tasks { tasks }) => tasks.workspace == binding.workspace(),
        (
            Pin::Task {
                query,
                scope,
                source,
            },
            Outcome::Task { task },
        ) => {
            task.answers(query)
                && task.scope == *scope
                && task.evidence.original.as_ref().map(|o| &o.source) == source.as_ref()
        }
        (Pin::Original { query }, Outcome::TaskOriginal { original }) => original.answers(query),
        _ => false,
    };
    if !matches {
        return refused(SessionError::Conflict);
    }
    protect(
        Json(json!({"active":true,"identity":pin_identity(binding,&pin,&viewer)})).into_response(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_pins_are_closed_reads_and_keep_original_job_bodies_out_of_urls() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let scope = coder_access::cloud::Scope {
            workspace: "checkout".into(),
            project: "explicit-project".into(),
            job: "original-job".into(),
            revision: digest.clone(),
            attempt: 1,
            profile: "explicit-profile".into(),
            profile_revision: digest.clone(),
            source_digest: digest,
        };
        let operation = Operation::CloudRead {
            query: coder_access::cloud::ReadQuery {
                workspace: scope.workspace.clone(),
                project: scope.project.clone(),
                job: scope.job.clone(),
                revision: Some(scope.revision.clone()),
            },
        };
        let job = coder_access::cloud::Job {
            scope,
            state: "completed".into(),
            placement: "boat".into(),
            mode: "coder".into(),
            executor: "original-executor".into(),
            model: Some("requested-model".into()),
            served_model: None,
            pool: "admitted-pool".into(),
            prompt: "Private request for person@example.invalid".into(),
            prompt_omitted: false,
            credential_names: vec!["PRIVATE_CREDENTIAL_NAME".into()],
            remote_task: None,
            continuation: "unknown".into(),
            cancellation: "requested".into(),
            cleanup: "unknown".into(),
            artifact_state: "unknown".into(),
            usage: None,
            error: None,
            details_omitted: false,
            originals: Vec::new(),
            environment: None,
        };
        let outcome = Outcome::CloudRead {
            job: Box::new(job.clone()),
        };
        let expected_digest = owner_digest(&operation, &outcome).unwrap();
        let pin = Pin::OwnerRead {
            read: OwnerReadPin {
                operation: operation.clone(),
                expected_digest: expected_digest.clone(),
            },
            control: None,
        };
        let encoded = encode(&pin).unwrap();
        let decoded = URL_SAFE_NO_PAD.decode(encoded).unwrap();
        let text = std::str::from_utf8(&decoded).unwrap();
        for private in [
            "Private request",
            "person@example.invalid",
            "PRIVATE_CREDENTIAL_NAME",
            "requested-model",
        ] {
            assert!(!text.contains(private));
        }
        let mut changed = job;
        changed.served_model = Some("explicitly-recorded-served-model".into());
        let changed = Outcome::CloudRead {
            job: Box::new(changed),
        };
        assert_ne!(owner_digest(&operation, &changed).unwrap(), expected_digest);
        assert!(owner_workspace(&snapshot().action).is_err());
        assert!(owner_digest(&snapshot().action, &outcome).is_err());
    }

    #[test]
    fn owner_projection_pins_reject_scope_or_visible_metadata_substitution() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let operation = Operation::ProjectList {
            workspace: "checkout".into(),
        };
        let projects = coder_access::project::List {
            workspace: "checkout".into(),
            rows: vec![coder_access::project::Row {
                id: "explicit-project".into(),
                label: "Original project label".into(),
                snapshot_digest: digest,
                goals_state: "unknown".into(),
            }],
        };
        let expected = owner_digest(
            &operation,
            &Outcome::ProjectList {
                projects: projects.clone(),
            },
        )
        .unwrap();
        let mut changed = projects.clone();
        changed.rows[0].label = "Changed label".into();
        assert_ne!(
            owner_digest(&operation, &Outcome::ProjectList { projects: changed }).unwrap(),
            expected
        );
        let mut changed = projects;
        changed.workspace = "another-workspace".into();
        assert!(owner_digest(&operation, &Outcome::ProjectList { projects: changed }).is_err());
    }

    fn snapshot() -> super::super::effects::Snapshot {
        super::super::effects::Snapshot {
            id: "a".repeat(64),
            packet_digest: "b".repeat(64),
            action: Operation::CancelTask {
                task: "c".repeat(64),
                revision: 7,
                reason: "Private cancellation request for person@example.invalid".into(),
            },
            expires_at: 2000,
            state: super::super::effects::State::Prepared,
            outcome: None,
            refusal: None,
            failure: None,
        }
    }

    #[test]
    fn cursors_are_bounded_original_data_and_targets_are_closed() {
        assert!(!identifier("../host"));
        assert!(!task_id("../task"));
        assert!(task_id(&"a".repeat(64)));
        assert!(decode::<Value>(&"A".repeat(9000)).is_err());
        let encoded = encode(&json!({"prefix":"original","revision":7})).unwrap();
        assert_eq!(decode::<Value>(&encoded).unwrap()["revision"], 7);
        assert!(encode(&"x".repeat(5000)).is_err());
    }

    #[test]
    fn request_pins_keep_private_action_bytes_out_of_standing_urls() {
        let mut snapshot = snapshot();
        let original = request_pin(&snapshot);
        let encoded = encode(&original).unwrap();
        let decoded: Value = decode(&encoded).unwrap();
        let json = decoded.to_string();
        assert!(!json.contains("Private cancellation") && !json.contains("example.invalid"));
        assert!(decoded.get("action").is_none() && decoded.get("reason").is_none());
        snapshot.state = super::super::effects::State::Unknown;
        snapshot.failure = Some(SessionError::Unavailable);
        assert!(request_pin(&snapshot) != original);
        snapshot.state = super::super::effects::State::Refused;
        snapshot.failure = None;
        snapshot.refusal = Some(super::super::effects::Refusal {
            code: coder_access::Code::Conflict,
            missing: None,
        });
        let refusal = request_pin(&snapshot);
        snapshot.refusal.as_mut().unwrap().code = coder_access::Code::Unavailable;
        assert!(request_pin(&snapshot) != refusal);
    }

    #[test]
    fn combined_control_sources_fit_the_pinned_endpoint_bound() {
        let scope = Scope {
            workspace: "w".repeat(128),
            task: "c".repeat(64),
            revision: 7,
            attempt: Some(3),
            intent_digest: format!("sha256:{}", "d".repeat(64)),
        };
        let pin = Pin::Control {
            control: ControlPin {
                scope_digest: format!("sha256:{}", "a".repeat(64)),
                enrolled: true,
                request: Some(request_pin(&snapshot())),
                task: Some(ControlTaskPin {
                    query: PageQuery {
                        workspace: scope.workspace.clone(),
                        task: scope.task.clone(),
                        revision: Some(scope.revision),
                        cursor: Some(task_read::Cursor {
                            scope: scope.clone(),
                            source: "trace:1234567890123456".into(),
                            source_digest: format!("sha256:{}", "e".repeat(64)),
                            source_bytes: 1_048_576,
                            next_step: 32,
                            prefix_digest: format!("sha256:{}", "f".repeat(64)),
                        }),
                        limit: 1,
                    },
                    scope: scope.clone(),
                    source: Some("trace:1234567890123456".into()),
                }),
                queue_digest: Some(format!("sha256:{}", "b".repeat(64))),
                review: Some(ReviewPin {
                    task: scope.task.clone(),
                    base: "a".repeat(64),
                    head_commit: "b".repeat(64),
                    head: "c".repeat(64),
                }),
            },
        };
        let encoded = encode(&pin).unwrap();
        let endpoint = format!(
            "/cloud/app/hosts/{}/standing?pin={encoded}",
            "b".repeat(128)
        );
        assert!(endpoint.len() <= 4096);
        assert!(decode::<Pin>(&encoded).is_ok());
        let text = String::from_utf8(URL_SAFE_NO_PAD.decode(&encoded).unwrap()).unwrap();
        assert!(!text.contains("example.invalid") && !text.contains("Private cancellation"));
    }
}
