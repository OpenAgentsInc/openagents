//! `/export` in a web chat's composer (#11615): the chat as one ATIF
//! document (`openagents.atif.v1`), the same shape Coder's terminal
//! `/export` writes (`coder_new::trajectory::document`), so either file
//! reads in the same tools.
//!
//! The composer's script (`/chat/export.js`) runs the command in the
//! browser; nothing is sent to a model. It posts here:
//!
//! - On the local stack (`openagents-web --local-downloads`, loopback
//!   only, and a request that came to the local address,
//!   [`crate::local_request`]) the server writes the file into the
//!   person's Downloads folder under a name it makes itself
//!   ([`file_name`]) and answers with the absolute path, which the page
//!   copies.
//! - Anywhere else (openagents.com) nothing is written on the server: the
//!   answer is the document itself as a download, and the page saves it
//!   with the browser.
//!
//! A Video Lab chat's answers keep their Lab card; the export carries each
//! card's document and version, on its step and gathered at the top
//! (`extra.video_lab`).

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::{Form, Path as UrlPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, ControlSize};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::App;
use crate::chat_store::{Conversation, Role, StepStatus};

/// The composer's script.
pub(crate) const SCRIPT_PATH: &str = "/chat/export.js";

/// Set on a response whose file the server already saved; its body is
/// `{"path": ABSOLUTE, "name": FILE}`.
const SAVED_HEADER: &str = "x-oa-export";

/// What the web chat calls its door in the document.
const DOOR: &str = "openagents-web";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(SCRIPT_PATH, get(script))
        .route("/chat/{id}/export", get(download).post(export))
}

#[derive(Deserialize)]
struct ExportForm {
    csrf: String,
}

/// The file's name: `openagents-chat-<UTC stamp>-<first 8 of the chat
/// id>.atif.json`. Made here from a validated chat id and the clock, never
/// from anything the browser sent.
pub(crate) fn file_name(chat_id: &str, at_ms: u64) -> String {
    let id8: String = chat_id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(8)
        .collect();
    format!("openagents-chat-{}-{id8}.atif.json", atif::stamp(at_ms))
}

/// Write `bytes` as a new file in `dir` named [`file_name`], or with `-2`,
/// `-3`, ... before the extension when that name is taken. Never replaces
/// a file and never writes outside `dir`.
pub(crate) fn save(dir: &Path, chat_id: &str, at_ms: u64, bytes: &[u8]) -> io::Result<PathBuf> {
    let name = file_name(chat_id, at_ms);
    let stem = name.trim_end_matches(".atif.json");
    for n in 1..100 {
        let candidate = if n == 1 {
            name.clone()
        } else {
            format!("{stem}-{n}.atif.json")
        };
        let path = dir.join(&candidate);
        if path.parent() != Some(dir) {
            return Err(io::Error::other("refused a name outside the folder"));
        }
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "too many exports of this chat this second",
    ))
}

/// The chat as an ATIF document, exported at `at_ms`.
pub(crate) fn document(chat: &Conversation, at_ms: u64) -> Value {
    let mut session = atif::Session::opening(
        &chat.id,
        "auto",
        DOOR,
        chat.selection
            .as_ref()
            .and_then(|s| s.repository.as_ref())
            .map(|r| r.repository.as_str())
            .unwrap_or_default(),
        env!("CARGO_PKG_VERSION"),
    );
    session.state = if chat.working() { "running" } else { "ended" }.into();
    let mut steps = Vec::new();
    let mut lab = Vec::<Value>::new();
    for (index, message) in chat.messages.iter().enumerate() {
        let mut step = match message.role {
            Role::User => {
                let mut step = atif::Step::said(atif::Source::User, &message.text);
                let files: Vec<Value> = crate::chat_files::of_message(chat, message)
                    .iter()
                    .map(|file| json!({ "name": file.name, "size": file.size }))
                    .collect();
                if !files.is_empty() {
                    step = step.noting("files", json!(files));
                }
                step
            }
            Role::Assistant => {
                let mut step = atif::Step::said(atif::Source::Agent, &message.text);
                if let Some(reply) = crate::suggestions::message_reply(chat, message) {
                    if let Some(route) = &reply.route {
                        step = step.noting("route", json!(route));
                    }
                    if let Some(card) = crate::lab::cards(reply).first() {
                        let reference = lab_reference(card);
                        if !lab.contains(&reference) {
                            lab.push(reference.clone());
                        }
                        step = step.noting("video_lab", reference);
                    }
                }
                step
            }
            Role::Thinking => atif::Step::thought(&message.text),
            Role::Tool => match chat.terminal.as_ref().and_then(|t| t.steps.get(&index)) {
                Some(detail) => atif::Step::called(atif::Call {
                    id: format!("call-{}", index + 1),
                    name: detail.name.clone(),
                    arguments: serde_json::from_str(&detail.input)
                        .unwrap_or_else(|_| json!(detail.input)),
                    output: detail.output.clone(),
                    outcome: match detail.status {
                        StepStatus::Running => atif::Outcome::Cancelled,
                        StepStatus::Done => atif::Outcome::Completed,
                        StepStatus::Failed => atif::Outcome::Failed,
                    },
                    milliseconds: 0,
                    purpose: None,
                    extra: Map::from_iter([(
                        "running".to_owned(),
                        json!(detail.status == StepStatus::Running),
                    )]),
                }),
                None => atif::Step::said(atif::Source::System, &message.text),
            },
        };
        // A web chat keeps no time per message: every step takes the
        // export's time, as Coder does for entries it has no time for.
        step.at = at_ms;
        if session.directive.is_empty() && step.source == atif::Source::User {
            session.directive = step.message.clone();
        }
        steps.push(step);
    }
    let mut document = atif::document_at(&session, &steps, at_ms);
    for step in document["steps"].as_array_mut().into_iter().flatten() {
        if step
            .pointer("/tool_calls/0/extra/running")
            .and_then(Value::as_bool)
            == Some(true)
        {
            step.as_object_mut().map(|s| s.remove("observation"));
        }
    }
    document["extra"]["timestamps"] = json!("export-time");
    document["extra"]["title"] = json!(chat.title);
    if !lab.is_empty() {
        document["extra"]["video_lab"] = json!(lab);
    }
    document
}

/// The Lab document and version a card shows.
fn lab_reference(card: &Value) -> Value {
    let mut reference = Map::new();
    for key in ["doc", "title", "kind", "version", "quality", "player"] {
        if let Some(value) = card.get(key).filter(|v| !v.is_null()) {
            reference.insert(key.to_owned(), value.clone());
        }
    }
    let versions: Vec<Value> = card["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.get("n").cloned())
        .collect();
    if !versions.is_empty() {
        reference.insert("versions".to_owned(), json!(versions));
    }
    if card["compare"].is_object() {
        reference.insert(
            "compare".to_owned(),
            json!({
                "a": card["compare"]["a"]["version"],
                "b": card["compare"]["b"]["version"],
            }),
        );
    }
    Value::Object(reference)
}

fn bytes(document: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(document).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The document as a download, named like a saved one.
fn attachment(chat: &Conversation, at_ms: u64) -> Response {
    let name = file_name(&chat.id, at_ms);
    let mut response = (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        bytes(&document(chat, at_ms)),
    )
        .into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    crate::chat_html::protect(response)
}

/// Where this request may have the file saved for it: the local stack's
/// Downloads folder, for a request that came to the local address.
fn downloads<'a>(app: &'a App, headers: &HeaderMap) -> Option<&'a Path> {
    app.config
        .downloads
        .as_deref()
        .filter(|_| crate::local_request(headers))
}

/// `GET /chat/{id}/export`: the document as a download, on any host.
async fn download(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    match crate::pages::chat::load(&app, &headers, &id).await {
        Ok(loaded) => attachment(&loaded.conversation, atif::now_ms()),
        Err(response) => response,
    }
}

/// `POST /chat/{id}/export`: saved into Downloads on the local stack,
/// otherwise the download itself.
async fn export(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    Form(form): Form<ExportForm>,
) -> Response {
    if let Err(response) = crate::pages::chat::validate_form(&app, &headers, &form.csrf).await {
        return response;
    }
    let loaded = match crate::pages::chat::load(&app, &headers, &id).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let chat = &loaded.conversation;
    let at_ms = atif::now_ms();
    let Some(dir) = downloads(&app, &headers) else {
        return attachment(chat, at_ms);
    };
    match save(dir, &chat.id, at_ms, &bytes(&document(chat, at_ms))) {
        Ok(path) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut response = axum::Json(json!({
                "path": path.to_string_lossy(),
                "name": name,
            }))
            .into_response();
            response
                .headers_mut()
                .insert(SAVED_HEADER, HeaderValue::from_static("saved"));
            crate::chat_html::protect(response)
        }
        Err(error) => {
            eprintln!("chat export: {error}");
            crate::chat_html::protect(
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "We couldn't save the file to your Downloads folder.",
                )
                    .into_response(),
            )
        }
    }
}

/// Under the composer: the `/export` hint while the box holds the start of
/// a command, and what the export did, with a Copy button for when the
/// browser refuses to copy by itself. Both stay hidden until the script
/// fills them; without JavaScript there is no command to run.
pub(crate) fn panel() -> Markup {
    let size = ControlSize::Sm;
    html! {
        p.oa-composer-feedback data-oa-export-hint hidden {
            (Button::new("/export")
                .kind(ButtonType::Button)
                .size(size)
                .variant(ButtonVariant::Ghost)
                .attr("data-oa-export-run", ""))
            " Save this chat as a file"
        }
        p.oa-composer-feedback data-oa-export-result role="status" aria-live="polite" hidden {
            span data-oa-export-text {}
            " "
            (Button::new("Copy")
                .kind(ButtonType::Button)
                .size(size)
                .variant(ButtonVariant::Outline)
                .attr("data-oa-export-copy", "")
                .attr("hidden", ""))
        }
    }
}

async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        include_str!("../static/chat-export.js"),
    )
        .into_response()
}

#[cfg(test)]
#[path = "chat_export_tests.rs"]
mod tests;
