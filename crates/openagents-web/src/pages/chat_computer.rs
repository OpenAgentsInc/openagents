//! Screenshot and Copy a file on a Coder chat (#11185).
//!
//! While Coder on the chat's computer is online, the chat's page offers
//! **Screenshot** and **Copy a file** under the reply box. Each queues an
//! ask on the chat ([`crate::coder_sync::queue_ask`]), the same way a reply
//! waits there: Coder on the computer takes it at its next check-in, runs
//! it through that computer's own host, and uploads what came back
//! (`PUT /coder/sessions/{session}/captures/{ask}`). The website never
//! reaches the computer; the computer always pulls.
//!
//! The thread shows each ask where it was made: waiting, then the picture
//! inline, a text file's link, or why the computer couldn't. What came
//! back is kept in the account's private chat storage
//! ([`crate::chat_store::Store::capture_key`]) and served only to the
//! chat's owner, at `GET /chat/{id}/captures/{ask}`, never cached, with a
//! locked-down sandbox so nothing in a file runs here. Deleting the chat
//! deletes it.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /chat/{id}/computer` `{csrf, action: screenshot \| pull, path?}` | Ask; back to the chat |
//! | `GET /chat/{id}/captures/{ask}` | What came back, to the owner |

use super::*;
use crate::chat_store::{AskAction, AskState, ComputerAsk, Store};
use crate::coder_sync::Queued;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/computer", post(ask))
        .route("/chat/{id}/captures/{ask}", get(capture))
}

/// Which control was pressed. A typed form field, never parsed from text.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Screenshot,
    Pull,
}

#[derive(Deserialize)]
struct AskForm {
    csrf: String,
    action: Action,
    #[serde(default)]
    path: String,
}

/// The two controls under a Coder chat's reply box, while its computer is
/// online. Plain forms: each reloads the chat, which then shows the ask.
pub(super) fn controls(app: &App, chat: &Conversation, computer: &str) -> Markup {
    let action = format!("/chat/{}/computer", chat.id);
    let csrf = csrf(app, &chat.owner);
    html! {
        div.oa-thread-computer #chat-computer {
            form method="post" action=(action) hx-boost="false" {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="action" value="screenshot";
                (Button::new(format!("Screenshot {computer}")).kind(ButtonType::Submit).variant(openagents_ui::actions::ButtonVariant::Outline))
            }
            form method="post" action=(action) hx-boost="false" {
                input type="hidden" name="csrf" value=(csrf);
                input type="hidden" name="action" value="pull";
                label for="chat-computer-path" { "Copy a file from " (computer) }
                input #chat-computer-path type="text" name="path" required
                    maxlength=(crate::chat_store::MAX_ASK_PATH_BYTES)
                    placeholder="~/notes.txt" autocomplete="off" spellcheck="false";
                (Button::new("Copy").kind(ButtonType::Submit).variant(openagents_ui::actions::ButtonVariant::Outline))
            }
        }
    }
}

async fn ask(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<AskForm>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refusal(
            StatusCode::BAD_REQUEST,
            "Something went wrong. Reload this page.",
        );
    };
    let owner = match validate_form(&app, &headers, &form.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let action = match form.action {
        Action::Screenshot => AskAction::Screenshot,
        Action::Pull => {
            let path = form.path.trim().to_owned();
            let action = AskAction::Pull { path };
            if !action.valid() {
                return refusal(
                    StatusCode::BAD_REQUEST,
                    "Enter the file's path on one line, starting with / or ~/.",
                );
            }
            action
        }
    };
    match crate::coder_sync::queue_ask(&app.config.chat_store, &owner, &id, action).await {
        Ok(Queued::Queued) => {}
        Ok(Queued::Offline(computer)) => {
            return refusal(
                StatusCode::CONFLICT,
                &format!("Coder on {computer} isn't online now. Open Coder there first."),
            );
        }
        Ok(Queued::Full) => {
            return refusal(
                StatusCode::CONFLICT,
                "Wait for the computer to answer what you asked already.",
            );
        }
        Ok(Queued::Secret | Queued::Missing) => return missing(),
        Err(e) => return unavailable(e),
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
}

/// The bytes an ask brought back, to the chat's owner only.
async fn capture(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, ask)): Path<(String, String)>,
) -> Response {
    let loaded = match load(&app, &headers, &id).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let chat = &loaded.conversation;
    let Some(known) = chat
        .terminal
        .as_ref()
        .and_then(|terminal| terminal.asks.iter().find(|known| known.id == ask))
    else {
        return missing();
    };
    let AskState::Saved { media, .. } = &known.state else {
        return missing();
    };
    let Ok(key) = Store::capture_key(&chat.owner, &chat.id, &known.id) else {
        return missing();
    };
    let bytes = match app.config.chat_store.read_key(&key).await {
        Ok(Some((bytes, _))) => bytes,
        Ok(None) => return missing(),
        Err(e) => return unavailable(e),
    };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(media) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    if media == "application/octet-stream" {
        let name: String = file_name(&known.action)
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .take(100)
            .collect();
        let name = if name.is_empty() {
            "file".to_owned()
        } else {
            name
        };
        if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

/// The last part of an ask's path, or `screenshot.png`.
fn file_name(action: &AskAction) -> &str {
    match action {
        AskAction::Screenshot => "screenshot.png",
        AskAction::Pull { path } => path.rsplit('/').next().unwrap_or(path),
    }
}

/// Bytes as people read them.
fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} bytes"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// One ask in the thread: what was asked, then what came back.
pub(super) fn turn(chat: &Conversation, computer: &str, index: usize, ask: &ComputerAsk) -> Markup {
    let asked = match &ask.action {
        AskAction::Screenshot => format!("Screenshot of {computer}"),
        AskAction::Pull { path } => format!("Copy {path} from {computer}"),
    };
    let link = format!("/chat/{}/captures/{}", chat.id, ask.id);
    html! {
        (ThreadMessage::user(&asked).id(format!("chat-ask-{index}")))
        @match &ask.state {
            AskState::Waiting | AskState::Taken { .. } => {
                (ThreadMessage::status(format!("Waiting for Coder on {computer}…")).id(format!("chat-ask-{index}-answer")))
            }
            AskState::Failed { message } => {
                (ThreadMessage::status(format!("{computer} couldn't: {message}")).id(format!("chat-ask-{index}-answer")))
            }
            AskState::Saved { size: bytes, media } => {
                (ThreadMessage::assistant(html! {
                    @if media.starts_with("image/") {
                        a href=(link) target="_blank" rel="noopener" {
                            img.oa-thread-capture src=(link) alt=(asked) loading="lazy"
                                style="max-width:100%;height:auto;border-radius:8px";
                        }
                    } @else {
                        p {
                            a href=(link) target="_blank" rel="noopener" { (file_name(&ask.action)) }
                            " (" (size(*bytes)) ")"
                        }
                    }
                })
                .author(computer.to_owned())
                .id(format!("chat-ask-{index}-answer")))
            }
        }
    }
}
