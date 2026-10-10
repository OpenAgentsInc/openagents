//! The confirm card on a Coder chat's page (#11169, #11170).
//!
//! When Coder on the chat's computer waits for the owner's answer (a
//! production deploy, a pull request merge, another ability its approval
//! policy asks about), it reports the question with its running work
//! ([`crate::phone_api`]: the open chat's item, status `asking`). The chat's
//! page shows that question as a card with **Approve** and **Deny**
//! (`#chat-approval`, read again every few seconds while the computer is
//! online), and the answer goes back the way the phone's does: a command
//! Coder takes with its next report ([`crate::phone_api::queue_action`]),
//! marked as answered on the web so Coder's approval record says where the
//! owner answered.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /chat/{id}/approval` | The card, or its empty place, as a fragment |
//! | `POST /chat/{id}/approval` `{csrf, request_id, item, question, decision}` | Approve or deny that question |

use crate::phone_api::{self, Acted, Command, Item};

use super::*;

/// How often an open page reads the card again.
const EVERY_SECONDS: u32 = 4;

pub(super) fn routes() -> Router<App> {
    Router::new().route("/chat/{id}/approval", get(fragment).post(answer))
}

#[derive(Deserialize)]
struct Answer {
    csrf: String,
    request_id: String,
    item: String,
    question: String,
    decision: String,
}

/// The item of this chat waiting on a question, among `items` Coder
/// reported for its computer.
pub(super) fn asking<'a>(items: &'a [Item], session: &str) -> Option<&'a Item> {
    items.iter().find(|item| {
        item.kind == "chat"
            && item.status == "asking"
            && item.question.is_some()
            && (item.id == session || item.session.as_deref() == Some(session))
    })
}

/// The question Coder on the chat's computer waits on, if any.
async fn waiting(app: &App, chat: &Conversation) -> Option<Item> {
    let terminal = chat.terminal.as_ref()?;
    let agents = phone_api::read_agents(&app.config.chat_store, &chat.owner)
        .await
        .ok()?;
    let board = agents.boards.get(&terminal.computer)?;
    asking(&board.items, &terminal.session).cloned()
}

/// The card for `item`'s question, or its empty place; both read
/// themselves again while the page is open.
pub(super) fn card(app: &App, chat: &Conversation, item: Option<&Item>) -> Markup {
    let poll = format!("every {EVERY_SECONDS}s");
    let path = format!("/chat/{}/approval", chat.id);
    let Some((item, question)) = item.and_then(|item| Some((item, item.question.as_ref()?))) else {
        return html! {
            div #chat-approval hx-get=(path) hx-trigger=(poll) hx-swap="outerHTML" {}
        };
    };
    html! {
        div #chat-approval.oa-thread-notice role="alert" hx-get=(path) hx-trigger=(poll)
            hx-swap="outerHTML" {
            p { strong { "Coder is waiting for your answer" } }
            @for line in question.text.lines().filter(|line| !line.trim().is_empty()) {
                p { (line) }
            }
            p { "This answers this one action only, and Coder records that you answered here." }
            form method="post" action=(path) {
                input type="hidden" name="csrf" value=(csrf(app, &chat.owner));
                input type="hidden" name="request_id" value=(new_id());
                input type="hidden" name="item" value=(item.id);
                input type="hidden" name="question" value=(question.id);
                (Button::new("Approve").kind(ButtonType::Submit).name("decision").value("approve"))
                " "
                (Button::new("Deny")
                    .kind(ButtonType::Submit)
                    .name("decision")
                    .value("deny")
                    .color(Color::Secondary))
            }
        }
    }
}

/// The card as the chat's page shows it when it loads.
pub(super) async fn current(app: &App, chat: &Conversation) -> Markup {
    let item = waiting(app, chat).await;
    card(app, chat, item.as_ref())
}

async fn fragment(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    let chat = match load_owned(&app, &owner, &id).await {
        Ok(loaded) => loaded.conversation,
        Err(response) => return response,
    };
    if chat.terminal.is_none() {
        return missing();
    }
    crate::chat_html::protect(current(&app, &chat).await.into_response())
}

async fn answer(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(posted): Form<Answer>,
) -> Response {
    let owner = match validate_form(&app, &headers, &posted.csrf).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let action = match posted.decision.as_str() {
        "approve" | "deny" => posted.decision.clone(),
        _ => return refusal(StatusCode::BAD_REQUEST, "Choose Approve or Deny."),
    };
    if !valid_id(&posted.request_id) {
        return refusal(StatusCode::BAD_REQUEST, "Reload this page and try again.");
    }
    let chat = match load_owned(&app, &owner, &id).await {
        Ok(loaded) => loaded.conversation,
        Err(response) => return response,
    };
    let Some(terminal) = chat.terminal.as_ref() else {
        return missing();
    };
    // Only the question this chat's page showed, and only while it waits.
    let Some(item) = waiting(&app, &chat).await.filter(|item| {
        item.id == posted.item
            && item
                .question
                .as_ref()
                .is_some_and(|question| question.id == posted.question)
    }) else {
        return refusal(
            StatusCode::CONFLICT,
            "Coder isn't waiting on that question anymore. Go back to the chat.",
        );
    };
    let command = Command {
        id: posted.request_id,
        item: item.id,
        action,
        question: Some(posted.question),
        // Coder records an answer that says `web` as answered on the web.
        text: Some("web".to_owned()),
    };
    match phone_api::queue_action(&app.config.chat_store, &owner, &terminal.computer, command).await
    {
        Ok(Acted::Queued) => {
            crate::chat_html::protect(Redirect::to(&format!("/chat/{id}")).into_response())
        }
        Ok(Acted::Unknown) => refusal(
            StatusCode::CONFLICT,
            "Coder isn't waiting on that question anymore. Go back to the chat.",
        ),
        Ok(Acted::Offline) => refusal(
            StatusCode::CONFLICT,
            &format!(
                "Coder on {} isn't online now, so it can't take your answer.",
                terminal.computer
            ),
        ),
        Err(_) => refusal(
            StatusCode::SERVICE_UNAVAILABLE,
            "Your answer couldn't be saved. Try again in a minute.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, status: &str, question: bool) -> Item {
        Item {
            id: id.to_owned(),
            kind: "chat".to_owned(),
            title: "Deploy".to_owned(),
            engine: None,
            status: status.to_owned(),
            started_unix: 1,
            finished_unix: None,
            cost_usd: None,
            tokens: None,
            session: Some(id.to_owned()),
            question: question.then(|| crate::phone_api::Question {
                id: "3".to_owned(),
                text: "Deploy this image to production (openagents.com)?\n\nsha256:abc".to_owned(),
            }),
            line: None,
        }
    }

    #[test]
    fn only_this_chats_waiting_question_is_shown() {
        let items = vec![
            item("other", "asking", true),
            item("mine", "working", false),
            item("mine", "asking", true),
        ];
        let found = asking(&items, "mine").unwrap();
        assert_eq!(found.id, "mine");
        assert_eq!(found.status, "asking");
        assert!(asking(&items[..2], "mine").is_none());
        assert!(asking(&[item("mine", "asking", false)], "mine").is_none());
    }
}
