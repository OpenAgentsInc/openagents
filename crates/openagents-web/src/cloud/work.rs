//! Canonical task observation through an explicitly bound resident host.

use super::hosts::Binding;
use super::session::{CloudSession, SessionError, Viewer};
use super::{protect, refused, service, standing_value, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::routing::get;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use coder_access::protocol::{Operation, Outcome};
use coder_access::task_read::{self, ListQuery, OriginalQuery, PageQuery, Scope};
use coder_ui::observation;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/hosts/{binding}/tasks", get(tasks))
        .route("/cloud/app/hosts/{binding}/tasks/{task}", get(task))
        .route(
            "/cloud/app/hosts/{binding}/tasks/{task}/original",
            get(original),
        )
        .route("/cloud/app/hosts/{binding}/standing", get(standing))
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

fn authority_value(viewer: &Viewer) -> Value {
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
fn task_id(value: &str) -> bool {
    coder_access::studio::id(value).is_ok()
}

// The native read DTOs below retain original owner identities and bounded pages.

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PageInput {
    cursor: Option<String>,
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
}

fn pin_identity(binding: &Binding, pin: &Pin, viewer: &Viewer) -> String {
    let value = match pin {
        Pin::List {} => json!({"kind":"list"}),
        Pin::Task { scope, source, .. } => json!({"kind":"task","scope":scope,"source":source}),
        Pin::Original { query } => {
            json!({"kind":"original","scope":query.scope,"original":query.original})
        }
    };
    identity(
        binding,
        &json!({"resource":value,"account_standing":authority_value(viewer)}),
    )
}

fn resource(binding: &Binding, pin: &Pin, viewer: &Viewer) -> Result<Value, SessionError> {
    let encoded = encode(pin)?;
    Ok(
        json!({"endpoint":format!("/cloud/app/hosts/{}/standing?pin={encoded}",binding.id()),"identity":pin_identity(binding,pin,viewer)}),
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
    let mut content = format!(
        "<h2>Resident tasks</h2><p>Host <code>{}</code> · Generation {} · Workspace <code>{}</code></p><p>Snapshot <code>{}</code>. Read at {}. Closing this view detaches observation.</p><p><a href=\"/cloud/app/hosts/{}/tasks\">Refresh tasks</a></p>",
        escape(binding.host()),
        binding.generation(),
        escape(&answer.workspace),
        escape(&answer.snapshot_digest),
        super::session::now(),
        escape(binding.id())
    );
    if answer.rows.is_empty() {
        content.push_str("<p>No tasks in this admitted workspace.</p>");
    }
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
            Ok(html)=>content.push_str(&format!("<section class=\"cloud-card\">{html}<p><a href=\"/cloud/app/hosts/{}/tasks/{}\">Open task</a></p></section>",escape(binding.id()),escape(&row.task))),
            Err(response)=>return response,
        }
    }
    if answer.more_available
        && let Some(cursor) = &answer.next
    {
        let encoded = match encode(cursor) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        content.push_str(&format!(
            "<p><a href=\"/cloud/app/hosts/{}/tasks?cursor={encoded}\">Next tasks</a></p>",
            escape(binding.id())
        ));
    }
    let resource = match resource(binding, &Pin::List {}, &viewer) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "tasks",
        Some(&content),
        Some(resource),
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
    let mut content = format!(
        "<p><a href=\"/cloud/app/hosts/{}/tasks\">Resident tasks</a> · <a href=\"/cloud/app/hosts/{}/tasks/{}\">Refresh this task</a></p><p>Resident generation {}. This page is a bounded original snapshot; refresh to read newer records.</p>",
        escape(&id),
        escape(&id),
        escape(&task),
        binding.generation()
    );
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
        Ok(v) => content.push_str(&v),
        Err(r) => return r,
    }
    if page.prompt.is_empty() {
        content.push_str("<h3>Request</h3><p>No inline request is included. Read the canonical task journal below for the original request.</p>");
    } else {
        content.push_str(&format!(
            "<h3>Request</h3><pre>{}</pre>",
            escape(&page.prompt)
        ));
    }
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
        Ok(v) => content.push_str(&v),
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
                    Ok(v) => {
                        content.push_str(&format!("<section class=\"cloud-card\">{v}</section>"))
                    }
                    Err(r) => return r,
                }
                match show(&observation::original_step(&projected, super::colors())) {
                    Ok(v) => content.push_str(&format!(
                        "<details><summary>Original step {index}</summary>{v}</details>"
                    )),
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
                content.push_str(&format!("<section class=\"cloud-card\"><h3>Step {index} · Oversized record</h3><p>The original is retained. <a href=\"{}\">Read bounded original chunks</a>.</p></section>",escape(&url)));
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
        content.push_str(&format!("<p><a href=\"/cloud/app/hosts/{}/tasks/{}?cursor={encoded}\">Next original steps</a></p>",escape(&id),escape(&task)));
    }
    content.push_str("<h3>Child references</h3><p>Child references retain their original parent step and call. A reference grants no child task, session, or control access.</p>");
    if page.children.is_empty() {
        content.push_str("<p>No structured child references are included in this view.</p>");
    }
    for child in &page.children {
        content.push_str(&format!("<section class=\"cloud-card\"><p>Parent step {} · Call <code>{}</code> · Agent {}</p><p>Child reference: <code>{}</code> · {}</p></section>", child.source_step, escape(&child.call_id),escape(&child.agent),escape(child.reference.as_deref().unwrap_or("Unavailable")),escape(&child.state)));
    }
    if page.more_children {
        content.push_str("<p>Additional child references remain in the original transcript.</p>");
    }
    content.push_str("<h3>Artifacts and original records</h3><p>These are resident evidence. An executor ending, checks passing, delivery, integration, stop, and cleanup remain separate.</p>");
    if let Some(original) = &page.evidence.original {
        let url = match original_url(binding, &page.scope, original) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        content.push_str(&format!(
            "<p><a href=\"{}\">Original transcript</a> · <code>{}</code></p>",
            escape(&url),
            escape(&original.digest)
        ));
    }
    for artifact in &page.artifacts {
        content.push_str(&format!(
            "<p>{} · {}</p>",
            escape(&artifact.label),
            escape(&artifact.state)
        ));
        if let Some(original) = &artifact.original {
            let url = match original_url(binding, &page.scope, original) {
                Ok(v) => v,
                Err(e) => return refused(e),
            };
            content.push_str(&format!(
                "<p><a href=\"{}\">Read original artifact</a> · {} bytes · <code>{}</code></p>",
                escape(&url),
                original.bytes,
                escape(&original.digest)
            ));
        }
    }
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
    let resource = match resource(binding, &pin, &viewer) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "tasks",
        Some(&content),
        Some(resource),
    )
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
    let mut content = format!(
        "<p><a href=\"/cloud/app/hosts/{}/tasks/{}\">Canonical task</a></p>",
        escape(&id),
        escape(&task)
    );
    match show(&file) {
        Ok(v) => content.push_str(&v),
        Err(r) => return r,
    }
    // A chunk can split a UTF-8 sequence. The original bytes stay available
    // without replacing invalid bytes or silently skipping them.
    content.push_str(&format!(
        "<details><summary>Original chunk bytes (base64)</summary><pre>{}</pre></details>",
        escape(&chunk.data)
    ));
    if chunk.more_available
        && let Some(cursor) = &chunk.next
    {
        let mut next = query.clone();
        next.cursor = Some(cursor.clone());
        let encoded = match encode(&next) {
            Ok(v) => v,
            Err(e) => return refused(e),
        };
        content.push_str(&format!("<p><a href=\"/cloud/app/hosts/{}/tasks/{}/original?cursor={encoded}\">Next original chunk</a></p>",escape(&id),escape(&task)));
    }
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
        Some(&content),
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
    fn cursors_are_bounded_original_data_and_targets_are_closed() {
        assert!(!identifier("../host"));
        assert!(!task_id("../task"));
        assert!(task_id(&"a".repeat(64)));
        assert!(decode::<Value>(&"A".repeat(9000)).is_err());
        let encoded = encode(&json!({"prefix":"original","revision":7})).unwrap();
        assert_eq!(decode::<Value>(&encoded).unwrap()["revision"], 7);
        assert!(encode(&"x".repeat(5000)).is_err());
    }
}
