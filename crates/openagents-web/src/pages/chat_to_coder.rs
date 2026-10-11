//! Run a chat's message in Coder on the person's computer (#11318).
//!
//! Under the newest answer of a chat that began here, a signed-in person
//! with Coder online sees **Run in Coder on {computer}**: it sends their
//! last message there, with the chat so far, the same way as picking
//! Coder on {computer} for it ([`super::start_on_coder`]), and opens the
//! Coder chat. It is the person's choice, offered whatever the answer
//! said, so work the router read as a question still reaches Coder. A
//! message already handed to Coder (the router read it as work on code,
//! [`super::hand_to_coder`]) has its Coder chat and no offer.
//!
//! The offer loads itself (`GET /chat/{id}/to-coder`), so a chat's page
//! never waits on the computers' check-ins; it is empty when nothing can
//! run. `POST /chat/{id}/to-coder` `{csrf, request_id, computer}` sends it.

use axum::response::Html;

use super::*;

/// At most this many computers are offered.
const MAX_OFFERED: usize = 3;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/chat/{id}/to-coder", get(offer).post(send))
}

/// The person's newest message, when an answer here follows it: what the
/// offer runs, and where it is in the chat.
fn last_asked(chat: &Conversation) -> Option<(usize, &Message)> {
    let at = chat.messages.iter().rposition(|m| m.role == Role::User)?;
    let message = &chat.messages[at];
    let answered = chat.messages[at + 1..]
        .iter()
        .any(|m| m.role == Role::Assistant && !m.text.trim().is_empty());
    (answered && message.request_id.as_deref().is_some_and(valid_id)).then_some((at, message))
}

/// Where the offer loads, under the newest answer: only on an account's
/// chat that began here, with nothing being answered.
pub(super) fn slot(chat: &Conversation) -> Markup {
    if chat.terminal.is_some()
        || chat.pending.is_some()
        || !crate::chat_store::is_account_owner(&chat.owner)
        || last_asked(chat).is_none()
    {
        return html! {};
    }
    html! {
        div #chat-to-coder hx-get=(format!("/chat/{}/to-coder", chat.id))
            hx-trigger="load" hx-swap="outerHTML" {}
    }
}

/// The offer: one button per online computer, or nothing.
async fn offer(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let empty = || crate::chat_html::protect(Html(String::new()).into_response());
    let Ok(loaded) = load(&app, &headers, &id).await else {
        return empty();
    };
    let chat = &loaded.conversation;
    if chat.terminal.is_some()
        || chat.pending.is_some()
        || !crate::chat_store::is_account_owner(&chat.owner)
    {
        return empty();
    }
    let Some((_, asked)) = last_asked(chat) else {
        return empty();
    };
    let request = asked.request_id.clone().unwrap_or_default();
    // Already handed to Coder: its chat is linked from the answer.
    let handed = crate::coder_sync::chat_id(&chat.owner, &crate::coder_sync::web_session(&request));
    if matches!(
        app.config.chat_store.load(&chat.owner, &handed).await,
        Ok(Some(_))
    ) {
        return empty();
    }
    let computers = crate::composer_row::online_computers(&app, &chat.owner).await;
    if computers.is_empty() {
        return empty();
    }
    let csrf = csrf(&app, &chat.owner);
    crate::chat_html::protect(
        Html(
            html! {
                div #chat-to-coder.oa-thread-notice {
                    @for computer in computers.iter().take(MAX_OFFERED) {
                        form method="post" action=(format!("/chat/{}/to-coder", chat.id)) {
                            input type="hidden" name="csrf" value=(csrf);
                            input type="hidden" name="request_id" value=(request);
                            input type="hidden" name="computer" value=(computer);
                            (Button::new(format!("Run in Coder on {computer}"))
                                .kind(ButtonType::Submit)
                                .color(Color::Secondary))
                        }
                    }
                }
            }
            .into_string(),
        )
        .into_response(),
    )
}

#[derive(Deserialize)]
struct ToCoder {
    csrf: String,
    request_id: String,
    computer: String,
}

/// Sends the person's message to Coder on the computer, with the chat so
/// far, and opens the Coder chat.
async fn send(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(posted): Form<ToCoder>,
) -> Response {
    let owner = match validate_form(&app, &headers, &posted.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let loaded = match load_owned(&app, &owner, &id).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let chat = &loaded.conversation;
    if chat.terminal.is_some() || !crate::chat_store::is_account_owner(&owner) {
        return missing();
    }
    let Some(at) = chat.messages.iter().position(|m| {
        m.role == Role::User && m.request_id.as_deref() == Some(posted.request_id.as_str())
    }) else {
        return refusal(
            StatusCode::CONFLICT,
            "That message isn't in this chat anymore. Reload the chat.",
        );
    };
    if chat
        .requests
        .iter()
        .any(|r| r.id == posted.request_id && !r.files.is_empty())
    {
        return refusal(StatusCode::BAD_REQUEST, crate::chat_files::NOT_TO_CODER);
    }
    start_on_coder(
        &app,
        &headers,
        &owner,
        &posted.computer,
        &posted.request_id,
        &chat.messages[at].text,
        chat.project.clone(),
        &chat.messages[..at],
    )
    .await
}
