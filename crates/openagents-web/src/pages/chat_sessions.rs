//! A computer session's page on the web (#11897): the summary first.
//!
//! A row from [`crate::session_index`] opens here. The page shows the
//! title, the computer, the project, what the session is doing, when it
//! was last active, and its recap. It says plainly when the computer is
//! offline. Its actions open the chat in the OpenAgents app and continue
//! it here as a new web chat linked to the session. The transcript shows
//! only when its owner turned it on.
//!
//! | Route | What |
//! | --- | --- |
//! | `GET /chat/sessions/{id}[?project=]` | The summary page |
//! | `POST /chat/sessions/{id}/continue` `{csrf}` | Start the linked web chat, or open the one already started |

use openagents_ui::actions::{ButtonLink, ButtonVariant, ControlSize};
use sha2::{Digest, Sha256};

use crate::session_index::{self, CopiedRole, Row};

use super::*;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/sessions/{id}", get(page))
        .route("/chat/sessions/{id}/continue", post(start))
}

/// The web page of session `id`.
pub(crate) fn href(id: &str) -> String {
    format!("/chat/sessions/{id}")
}

#[derive(Default, Deserialize)]
struct Scope {
    project: Option<String>,
}

fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(scope): Query<Scope>,
) -> Response {
    let Some(owner) = reader(&app, &headers).await else {
        return missing();
    };
    if !crate::chat_store::is_account_owner(&owner) {
        return missing();
    }
    let found =
        match session_index::find(&app, &headers, &owner, &id, scope.project.as_deref()).await {
            Ok(found) => found,
            Err(_) => return missing(),
        };
    let row = found.row();
    let now = now_unix();
    let online = found.index.online(row, now);
    let own = found.owner == owner;
    let copy = if found.transcript(&owner) {
        session_index::copy(&app.config.chat_store, &found.owner, &id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let projects = crate::projects::sidebar(&app).await;
    let project = row
        .project
        .as_deref()
        .and_then(|p| projects.as_deref()?.project(p))
        .map(|p| p.name.clone());
    let mut where_ = vec![format!("On {}", row.computer)];
    if let Some(folder) = &row.folder {
        where_.push(folder.clone());
    }
    if let Some(project) = project {
        where_.push(project);
    }
    let mut state = Vec::new();
    if let Some(label) = row.status.label() {
        state.push(label.to_owned());
    }
    state.push(format!("Last active {}", ago(row.last_activity_unix, now)));
    let token = csrf(&app, &owner);
    let content = html! {
        h1 { (row.title) }
        p.oa-session-where { (where_.join(" · ")) }
        p.oa-session-state { (state.join(" · ")) }
        @if !online {
            p.oa-thread-notice #session-offline {
                (row.computer) " is offline. You can read this summary, and continue the chat here."
            }
        }
        @if !row.recap.is_empty() {
            p.oa-session-recap { (row.recap) }
        }
        div.oa-session-actions {
            @if let Some(link) = row.app_link() {
                (ButtonLink::new("Open in the app", link)
                    .color(Color::Secondary)
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Sm)
                    .pill(true))
            }
            @if let Some(chat) = &row.continued_by {
                (ButtonLink::new("Open the web chat", format!("/chat/{}", chat.id))
                    .color(Color::Secondary)
                    .variant(ButtonVariant::Soft)
                    .size(ControlSize::Sm)
                    .pill(true))
            } @else if own {
                form method="post" action=(format!("/chat/sessions/{id}/continue")) {
                    input type="hidden" name="csrf" value=(token);
                    (Button::new("Continue on the web")
                        .kind(ButtonType::Submit)
                        .color(Color::Secondary))
                }
            }
        }
        @if let Some(copy) = &copy {
            h2 { "Transcript" }
            div #session-transcript {
                @for (n, m) in copy.messages.iter().enumerate() {
                    @match m.role {
                        CopiedRole::User => (ThreadMessage::user(&m.text).id(format!("session-message-{n}"))),
                        CopiedRole::Assistant => (ThreadMessage::assistant(html! {
                            @for part in m.text.split("\n\n") { p { (part) } }
                        }).author(row.computer.clone()).id(format!("session-message-{n}"))),
                        CopiedRole::Tool => (ThreadMessage::status(&m.text).id(format!("session-message-{n}"))),
                    }
                }
            }
        } @else {
            p.oa-session-private #session-private { "The transcript stays on " (row.computer) "." }
        }
    };
    let sidebar = chat_list(&app, &owner, Some(&id), false, false).await;
    let page = UiPage::new(&row.title)
        .path(&href(&id))
        .head(crate::chat_html::head())
        .sidebar_section(sidebar)
        .content(crate::ui_page::prose(content));
    crate::chat_html::protect(page.respond(&headers))
}

#[derive(Deserialize)]
struct StartForm {
    csrf: String,
}

/// The web chat that continues session `id`: one per session and account.
pub(crate) fn continuation_id(owner: &str, id: &str) -> String {
    let digest = Sha256::digest(format!("openagents.web.session-continue.v1:{owner}:{id}"));
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The first message of the web chat: where it came from, the recap, and
/// the last few messages when the transcript is on.
fn intro(row: &Row, copy: Option<&session_index::Copy>) -> String {
    let mut text = format!(
        "This continues \u{201c}{}\u{201d} from {}.",
        row.title, row.computer
    );
    if !row.recap.is_empty() {
        text.push_str(&format!("\n\nWhere it stood: {}", row.recap));
    }
    if let Some(copy) = copy {
        let recent: Vec<String> = copy
            .messages
            .iter()
            .filter(|m| m.role != CopiedRole::Tool)
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|m| {
                let who = if m.role == CopiedRole::User {
                    "You"
                } else {
                    "Assistant"
                };
                let said: String = m.text.chars().take(300).collect();
                format!("{who}: {said}")
            })
            .collect();
        if !recent.is_empty() {
            text.push_str("\n\nRecent messages:\n");
            text.push_str(&recent.join("\n"));
        }
    }
    text
}

async fn start(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<StartForm>, axum::extract::rejection::FormRejection>,
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
    let found = match session_index::find(&app, &headers, &owner, &id, None).await {
        Ok(found) => found,
        Err(_) => return missing(),
    };
    let store = &app.config.chat_store;
    let row = found.row().clone();
    let chat = continuation_id(&owner, &id);
    match session_index::link(store, &owner, &id, &chat).await {
        Ok(true) => {}
        Ok(false) => {
            // Another chat continues it already: open that one.
            if let Some(part) = &row.continued_by {
                return crate::chat_html::protect(
                    Redirect::to(&format!("/chat/{}", part.id)).into_response(),
                );
            }
            return missing();
        }
        Err(e) => return unavailable(e),
    }
    match store.load(&owner, &chat).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            let copy = if found.transcript(&owner) {
                session_index::copy(store, &owner, &id).await.ok().flatten()
            } else {
                None
            };
            let now = now_unix();
            let record = Conversation {
                links: chat_home::Links {
                    continues: Some(chat_home::PartRef {
                        id: id.clone(),
                        home: row.home(),
                    }),
                    continued_by: None,
                },
                id: chat.clone(),
                owner: owner.clone(),
                revision: 1,
                title: row.title.chars().take(64).collect(),
                messages: vec![Message {
                    role: Role::Assistant,
                    text: intro(&row, copy.as_ref()),
                    request_id: None,
                }],
                pending: None,
                requests: Vec::new(),
                selection: None,
                updated_unix: now,
                pinned_unix: None,
                archived_unix: None,
                project: row.project.clone(),
                filed: false,
                terminal: None,
                environment: None,
                tasks: Vec::new(),
                opened_unix: None,
                branch: None,
            };
            match store.create(&record).await {
                Ok(_) | Err(Error::Conflict) => {}
                Err(e) => return unavailable(e),
            }
        }
        Err(e) => return unavailable(e),
    }
    crate::chat_html::protect(Redirect::to(&format!("/chat/{chat}")).into_response())
}

fn now_unix() -> u64 {
    crate::chat_store::now_unix()
}
