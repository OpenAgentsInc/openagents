use std::path::{Path as FilePath, PathBuf};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use coder::task::{self, Store};
use coder_ui::theme::{Intensity, NEAR_BLACK};
use serde::Deserialize;

const PAGE_SIZE: usize = 50;
const STYLE: &str = include_str!("style.css");

#[derive(Clone)]
struct App {
    store: PathBuf,
}

#[derive(Deserialize)]
struct Page {
    cursor: Option<String>,
}

pub fn router(store: PathBuf) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/style.css", get(style))
        .route("/app", get(tasks))
        .route("/app/tasks/{id}", get(task))
        .layer(middleware::from_fn(local_origin))
        .with_state(App { store })
}

async fn local_origin(request: Request<axum::body::Body>, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    if !matches!(host, Some("127.0.0.1:4300" | "localhost:4300")) {
        return (StatusCode::FORBIDDEN, "Use the local Coder address").into_response();
    }
    let mut response = next.run(request).await;
    let headers: &mut HeaderMap = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, "default-src 'none'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'".parse().expect("static CSP"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        "nosniff".parse().expect("static header"),
    );
    response
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="color-scheme" content="dark"><title>{} · Coder</title><link rel="stylesheet" href="/style.css"></head><body><a class="skip" href="#content">Skip to content</a><header><nav aria-label="Main"><a class="brand" href="/">◈ Coder<span class="brand-sub"> / OpenAgents</span></a><div class="navlinks"><a href="/app">Task browser</a><a href="https://github.com/OpenAgentsInc/openagents/tree/main/docs/coder">Documentation ↗</a></div></nav></header><main id="content">{body}</main><footer><span>Coder / OpenAgents</span><span>Local tasks, durable evidence.</span></footer></body></html>"##,
        escape(title),
    ))
}

async fn home() -> Html<String> {
    page(
        "Home",
        r#"<div class="hero"><div class="eyebrow">01 / YOUR WORK, IN VIEW</div><h1>Give work a<br><em>place to live.</em></h1><p class="intro">Coder helps you run agent work on your computer. Tasks retain their status, transcript, and artifacts so you can return to them after a terminal closes.</p><div class="actions"><a class="primary" href="/app">Open task browser <span aria-hidden="true">↗</span></a><a class="secondary" href="https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/guides/install.md">Get Coder →</a></div></div><section class="features" aria-label="What Coder does"><div><span class="index">01 /</span><h2>Keep the task</h2><p>Requested work stays in a local, durable inbox. Starting execution requires a separate grant.</p></div><div><span class="index">02 /</span><h2>See the evidence</h2><p>Read status, independent checks, and paged transcripts from the same task view as the terminal.</p></div><div><span class="index">03 /</span><h2>Pick up where you left off</h2><p>Inspect a task again without starting another run or losing its recorded result.</p></div></section><section class="terminal" aria-label="Coder task example"><div class="terminal-top"><span>CODER / TASKS</span><span>LOCAL VIEW</span></div><pre><span class="prompt">$</span> coder task list
<span class="muted"># Your durable tasks, available after you close the terminal.</span>
<span class="prompt">$</span> coder task view TASK_ID</pre></section>"#,
    )
}

async fn style() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            ":root{{--amber:#{:06x};--soft:#{:06x};--dark:#{:06x}}}\n{STYLE}",
            Intensity::Full.color(),
            Intensity::ThreeQuarters.color(),
            NEAR_BLACK,
        ),
    )
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
            "Task store is busy. Try again.",
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Task store is unavailable.",
        ),
    };
    (
        status,
        page(
            message,
            &format!("<h1>{message}</h1><p><a href=\"/app\">Back to tasks</a></p>"),
        ),
    )
        .into_response()
}

fn store_present(directory: &FilePath) -> Result<bool, task::Error> {
    if !directory.try_exists().map_err(task::Error::Io)? {
        return Ok(false);
    }
    if !directory
        .join(task::STORE_FILE)
        .try_exists()
        .map_err(task::Error::Io)?
    {
        return Err(task::Error::Corrupt("the task document is missing"));
    }
    Ok(true)
}

async fn tasks(State(app): State<App>) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        if store_present(&app.store)? {
            Store::open(&app.store)?.list()
        } else {
            Ok(Vec::new())
        }
    })
    .await;
    let list = match result {
        Ok(Ok(list)) => list,
        Ok(Err(error)) => return error_response(error),
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Task reader stopped").into_response();
        }
    };
    let mut body = String::from(
        "<div class=\"section-heading\"><div><span class=\"eyebrow\">LOCAL / READ ONLY</span><h1>Your tasks</h1></div><a class=\"secondary\" href=\"/app\">Refresh ↻</a></div><p class=\"hint\">Your local task store. Open a task to read its status, evidence, and artifacts.</p><div class=\"task-list\">",
    );
    if list.is_empty() {
        body.push_str("<div class=\"empty\"><h2>No tasks yet</h2><p>Use <code>coder task submit</code> to add work to your local inbox. The browser will show it here.</p></div>");
    }
    for item in list {
        body.push_str(&format!(
            "<a class=\"task-row\" href=\"/app/tasks/{}\"><span><strong>{}</strong><small>{}</small></span><span class=\"status\">{:?} / {:?}</span><span aria-hidden=\"true\">↗</span></a>",
            escape(&item.task_id),
            escape(&item.intent.title),
            escape(&item.task_id),
            item.status,
            item.execution,
        ));
    }
    body.push_str("</div>");
    page("Tasks", &body).into_response()
}

async fn task(
    Path(id): Path<String>,
    Query(query): Query<Page>,
    State(app): State<App>,
) -> Response {
    let cursor = match query.cursor {
        Some(encoded) if encoded.len() <= 8192 => {
            let Ok(bytes) = URL_SAFE_NO_PAD.decode(encoded) else {
                return (StatusCode::BAD_REQUEST, "Invalid page cursor").into_response();
            };
            match serde_json::from_slice::<task::view::Cursor>(&bytes) {
                Ok(cursor) => Some(cursor),
                Err(_) => return (StatusCode::BAD_REQUEST, "Invalid page cursor").into_response(),
            }
        }
        Some(_) => return (StatusCode::BAD_REQUEST, "Invalid page cursor").into_response(),
        None => None,
    };
    let result = tokio::task::spawn_blocking(move || {
        if !store_present(&app.store)? {
            return Err(task::Error::NotFound);
        }
        task::view::read(&app.store, &id, cursor.as_ref(), PAGE_SIZE)
    })
    .await;
    let view = match result {
        Ok(Ok(view)) => view,
        Ok(Err(error)) => return error_response(error),
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Task reader stopped").into_response();
        }
    };
    let task = &view.task;
    let mut body = format!(
        "<a class=\"back\" href=\"/app\">← All tasks</a><div class=\"section-heading\"><div><span class=\"eyebrow\">TASK / {}</span><h1>{}</h1></div><a class=\"secondary\" href=\"/app/tasks/{}\">Refresh ↻</a></div><dl class=\"facts\"><div><dt>Queue</dt><dd>{:?}</dd></div><div><dt>Execution</dt><dd>{:?}</dd></div><div><dt>Checks</dt><dd>{}</dd></div><div><dt>Integration</dt><dd>{}</dd></div><div><dt>Cost</dt><dd>{}</dd></div></dl><section class=\"detail\"><h2>Request</h2><pre>{}</pre><p class=\"hint\">Workspace: <code>{}</code> · Adapter: <code>{}</code></p></section>",
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
    body.push_str(&format!("<section class=\"detail\"><h2>Transcript</h2><p class=\"hint\">Evidence: {} · {} steps</p>", escape(&view.evidence.state), view.evidence.total_steps));
    if let Some(digest) = &view.evidence.digest {
        body.push_str(&format!(
            "<p class=\"hint\">Trace digest: <code>{}</code></p>",
            escape(digest)
        ));
    }
    for fault in &view.evidence.faults {
        body.push_str(&format!(
            "<p>Trace fault: <code>{}</code></p>",
            escape(&fault.to_string())
        ));
    }
    for step in &view.evidence.steps {
        let source = step["source"].as_str().unwrap_or("event");
        let message = step["message"].as_str().unwrap_or("");
        let number = step["step_id"].as_u64().unwrap_or(0);
        body.push_str(&format!(
            "<article class=\"step\"><span class=\"index\">{} / {}</span><pre>{}</pre>",
            number,
            escape(source),
            escape(message)
        ));
        body.push_str(&format!(
            "<details><summary>Step details</summary><pre>{}</pre></details>",
            escape(&serde_json::to_string_pretty(step).unwrap_or_default())
        ));
        body.push_str("</article>");
    }
    if let Some(next) = view.evidence.next.filter(|_| view.evidence.more_available) {
        let bytes = serde_json::to_vec(&next).unwrap_or_default();
        body.push_str(&format!(
            "<a class=\"primary\" href=\"/app/tasks/{}?cursor={}\">Next 50 steps →</a>",
            escape(&task.task_id),
            URL_SAFE_NO_PAD.encode(bytes)
        ));
    }
    body.push_str("</section><section class=\"detail\"><h2>Artifacts</h2>");
    if let Some(manifest) = &view.artifacts {
        body.push_str(&format!(
            "<p class=\"hint\">Snapshot: <code>{}</code> · Complete: {} · Omitted changes: {}</p>",
            escape(manifest.candidate_snapshot.as_deref().unwrap_or("unknown")),
            manifest.complete,
            manifest.omitted_changes
        ));
        for entry in &manifest.entries {
            body.push_str(&format!(
                "<p><code>{}</code> · {} · <code>{}</code></p>",
                escape(&entry.path.display().to_string()),
                escape(&entry.state),
                escape(entry.digest.as_deref().unwrap_or("no digest"))
            ));
            if let Some(target) = &entry.link_target {
                body.push_str(&format!(
                    "<p class=\"hint\">Link target: <code>{}</code></p>",
                    escape(&target.display().to_string())
                ));
            }
        }
        if manifest.entries.is_empty() {
            body.push_str("<p>No retained artifacts.</p>");
        }
    } else {
        body.push_str("<p>No retained artifacts.</p>");
    }
    if let Some(error) = &view.artifact_error {
        body.push_str(&format!(
            "<p>Artifact state unavailable: {}</p>",
            escape(error)
        ));
    }
    for fault in &view.artifact_faults {
        body.push_str(&format!("<p>Artifact fault: {}</p>", escape(fault)));
    }
    body.push_str("</section>");
    page(&task.intent.title, &body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    async fn request(router: Router, uri: &str, host: &str) -> (StatusCode, String) {
        let response = router
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn missing_store_does_not_create_data() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("uninitialized");
        let (status, body) = request(router(store.clone()), "/app", "127.0.0.1:4300").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("No tasks yet"));
        assert!(!store.exists());
        let (status, _) = request(
            router(store.clone()),
            "/app/tasks/not-found",
            "127.0.0.1:4300",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!store.exists());
    }

    #[tokio::test]
    async fn task_view_escapes_private_content_and_rejects_bad_cursor() {
        let root = tempfile::tempdir().unwrap();
        let store = root.path().join("tasks");
        let command = json!({
            "schema": task::COMMAND_SCHEMA,
            "command_id": "example-submit-1",
            "task_id": "example-task-1",
            "expected_revision": null,
            "action": {"type":"submit", "intent": {
                "title":"<script>alert(1)</script>", "prompt":"<img src=x onerror=alert(1)>",
                "workspace":{"path":"/workspace/example", "source_revision":null},
                "configuration":{"adapter":"coder", "model":null}
            }}
        });
        Store::open(&store)
            .unwrap()
            .apply(command.to_string().as_bytes())
            .unwrap();
        let (status, body) = request(router(store.clone()), "/app", "127.0.0.1:4300").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!body.contains("<script>"));
        let (status, body) = request(
            router(store.clone()),
            "/app/tasks/example-task-1",
            "127.0.0.1:4300",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
        assert!(body.contains("not_started"));
        let (status, _) = request(
            router(store),
            "/app/tasks/example-task-1?cursor=bogus",
            "127.0.0.1:4300",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rejects_nonlocal_host_header() {
        let root = tempfile::tempdir().unwrap();
        let (status, _) = request(
            router(root.path().join("tasks")),
            "/app",
            "attacker.example:4300",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
