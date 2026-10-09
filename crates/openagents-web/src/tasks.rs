//! The local task browser at `/app`: the durable task store and the paged
//! ATIF view that `coder task list` and `coder task view` read. It is read
//! only, answers only the local host (see `guard` in the crate root), and
//! never creates the store.

use std::path::Path as FilePath;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use coder::task::{self, Store};
use serde::Deserialize;

use maud::PreEscaped;

use crate::App;
use crate::layout::{escape, problem};
use crate::ui_page::{UiPage, prose};

const PAGE_SIZE: usize = 50;

#[derive(Deserialize)]
struct Cursor {
    cursor: Option<String>,
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/app", get(tasks))
        .route("/app/tasks/{id}", get(task))
}

fn error_response(error: task::Error) -> Response {
    let (status, message) = match error {
        task::Error::NotFound => (StatusCode::NOT_FOUND, "Task not found"),
        task::Error::Conflict => (
            StatusCode::CONFLICT,
            "The transcript changed. Reload the task.",
        ),
        task::Error::Busy => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Tasks are busy. Try again.",
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Tasks are unavailable right now.",
        ),
    };
    problem(status, message, message, ("/app", "Back to tasks"))
}

fn store_present(directory: &FilePath) -> Result<bool, task::Error> {
    if !directory.try_exists().map_err(task::Error::Io)? {
        return Ok(false);
    }
    if !task::present(directory) {
        return Err(task::Error::Corrupt("the task document is missing"));
    }
    Ok(true)
}

async fn tasks(State(app): State<App>, headers: HeaderMap) -> Response {
    let store = app.config.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        if store_present(&store)? {
            Store::open(&store)?.list()
        } else {
            Ok(Vec::new())
        }
    })
    .await;
    let list = match result {
        Ok(Ok(list)) => list,
        Ok(Err(error)) => return error_response(error),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Try again.",
            )
                .into_response();
        }
    };
    let mut body = String::from(
        "<p class=\"oa-page-eyebrow\">LOCAL / READ ONLY</p><h1>Your tasks</h1>\
<p class=\"oa-page-meta\">Your local task store. Open a task to read its status, evidence, and artifacts. \
<a href=\"/app\">Refresh</a></p><ul class=\"oa-item-list\">",
    );
    if list.is_empty() {
        body.push_str("<li><p class=\"oa-item-title\">No tasks yet</p><p>Use <code>coder task submit</code> to add work to your local inbox. The browser will show it here.</p></li>");
    }
    for item in list {
        body.push_str(&format!(
            "<li><p><a class=\"oa-item-title\" href=\"/app/tasks/{}\">{}</a></p><p class=\"oa-page-meta\">{} \u{b7} {:?} / {:?}</p></li>",
            escape(&item.task_id),
            escape(&item.intent.title),
            escape(&item.task_id),
            item.status,
            item.execution,
        ));
    }
    body.push_str("</ul>");
    UiPage::new("Tasks")
        .path("/app")
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(&headers)
}

async fn task(
    Path(id): Path<String>,
    Query(query): Query<Cursor>,
    State(app): State<App>,
    headers: HeaderMap,
) -> Response {
    let cursor = match query.cursor {
        Some(encoded) if encoded.len() <= 8192 => {
            let Ok(bytes) = URL_SAFE_NO_PAD.decode(encoded) else {
                return (StatusCode::BAD_REQUEST, "This page link is invalid.").into_response();
            };
            match serde_json::from_slice::<task::view::Cursor>(&bytes) {
                Ok(cursor) => Some(cursor),
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, "This page link is invalid.").into_response();
                }
            }
        }
        Some(_) => return (StatusCode::BAD_REQUEST, "This page link is invalid.").into_response(),
        None => None,
    };
    let store = app.config.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        if !store_present(&store)? {
            return Err(task::Error::NotFound);
        }
        task::view::read(&store, &id, cursor.as_ref(), PAGE_SIZE)
    })
    .await;
    let view = match result {
        Ok(Ok(view)) => view,
        Ok(Err(error)) => return error_response(error),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Try again.",
            )
                .into_response();
        }
    };
    let task = &view.task;
    let mut body = format!(
        "<p class=\"oa-page-meta\"><a href=\"/app\">All tasks</a> / {}</p><h1>{}</h1>\
<p><a href=\"/app/tasks/{}\">Refresh</a></p>\
<dl class=\"oa-facts\"><div><dt>Queue</dt><dd>{:?}</dd></div><div><dt>Execution</dt><dd>{:?}</dd></div>\
<div><dt>Checks</dt><dd>{}</dd></div><div><dt>Integration</dt><dd>{}</dd></div><div><dt>Cost</dt><dd>{}</dd></div></dl>\
<section><h2>Request</h2><pre>{}</pre><p class=\"oa-page-meta\">Workspace: <code>{}</code> \u{b7} Adapter: <code>{}</code></p></section>",
        escape(&task.task_id),
        escape(&task.intent.title),
        escape(&task.task_id),
        task.status,
        task.execution,
        escape(&view.verification),
        escape(&view.integration),
        view.cost_usd
            .map_or_else(|| escape(&view.cost_status), |cost| format!("${cost:.2}")),
        escape(task.effective_prompt()),
        escape(&task.intent.workspace.path),
        escape(&task.intent.configuration.adapter),
    );
    body.push_str(&format!(
        "<section><h2>Transcript</h2><p class=\"oa-page-meta\">{} \u{b7} {} steps</p>",
        escape(&view.evidence.state),
        view.evidence.total_steps
    ));
    if let Some(digest) = &view.evidence.digest {
        body.push_str(&format!(
            "<p class=\"oa-page-meta\">Trace ID: <code>{}</code></p>",
            escape(digest)
        ));
    }
    for fault in &view.evidence.faults {
        body.push_str(&format!(
            "<p>Trace problem: <code>{}</code></p>",
            escape(&fault.to_string())
        ));
    }
    body.push_str("<ol class=\"oa-event-list\">");
    for step in &view.evidence.steps {
        let source = step["source"].as_str().unwrap_or("event");
        let message = step["message"].as_str().unwrap_or("");
        let number = step["step_id"].as_u64().unwrap_or(0);
        body.push_str(&format!(
            "<li><span class=\"oa-page-meta\">{} / {}</span><pre>{}</pre><details><summary>Step details</summary><pre>{}</pre></details></li>",
            number,
            escape(source),
            escape(message),
            escape(&serde_json::to_string_pretty(step).unwrap_or_default())
        ));
    }
    body.push_str("</ol>");
    if let Some(next) = view.evidence.next.filter(|_| view.evidence.more_available) {
        let bytes = serde_json::to_vec(&next).unwrap_or_default();
        body.push_str(&format!(
            "<p><a href=\"/app/tasks/{}?cursor={}\">Next 50 steps</a></p>",
            escape(&task.task_id),
            URL_SAFE_NO_PAD.encode(bytes)
        ));
    }
    body.push_str("</section><section><h2>Artifacts</h2>");
    if let Some(manifest) = &view.artifacts {
        body.push_str(&format!(
            "<p class=\"oa-page-meta\">Snapshot: <code>{}</code> \u{b7} Complete: {} \u{b7} Omitted changes: {}</p>",
            escape(manifest.candidate_snapshot.as_deref().unwrap_or("unknown")),
            manifest.complete,
            manifest.omitted_changes
        ));
        for entry in &manifest.entries {
            body.push_str(&format!(
                "<p><code>{}</code> \u{b7} {} \u{b7} <code>{}</code></p>",
                escape(&entry.path.display().to_string()),
                escape(&entry.state),
                escape(entry.digest.as_deref().unwrap_or("no checksum"))
            ));
            if let Some(target) = &entry.link_target {
                body.push_str(&format!(
                    "<p class=\"oa-page-meta\">Link target: <code>{}</code></p>",
                    escape(&target.display().to_string())
                ));
            }
        }
        if manifest.entries.is_empty() {
            body.push_str("<p>No files saved.</p>");
        }
    } else {
        body.push_str("<p>No files saved.</p>");
    }
    if let Some(error) = &view.artifact_error {
        body.push_str(&format!("<p>Files unavailable: {}</p>", escape(error)));
    }
    for fault in &view.artifact_faults {
        body.push_str(&format!("<p>File problem: {}</p>", escape(fault)));
    }
    body.push_str("</section>");
    UiPage::new(task.intent.title.clone())
        .path(format!(
            "/app/tasks/{}",
            crate::layout::segment(&task.task_id)
        ))
        .scriptless()
        .content(prose(PreEscaped(body)))
        .respond(&headers)
}
