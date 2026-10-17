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

/// The OpenAgents mark a saved agent's session wears in lists (#11966).
pub(crate) fn agent_mark() -> Markup {
    html! {
        svg.oa-agent-mark viewBox="0 0 820 940" width="12" height="14" fill="currentColor" aria-hidden="true" {
            path d="M0 600h100v100h-100ZM0 720h100v100h-100ZM240 840h100v100h-100ZM240 720h100v100h-100ZM120 840h100v100h-100ZM120 600h100v100h-100ZM240 600h100v100h-100ZM0 480h100v100h-100ZM0 360h100v100h-100ZM480 840h100v100h-100ZM480 720h100v100h-100ZM120 360h100v100h-100ZM120 240h100v100h-100ZM240 360h100v100h-100ZM600 720h100v100h-100ZM480 600h100v100h-100ZM360 360h100v100h-100ZM240 240h100v100h-100ZM600 600h100v100h-100ZM720 600h100v100h-100ZM720 480h100v100h-100ZM240 120h100v100h-100ZM600 380h100v100h-100ZM720 240h100v100h-100ZM720 0h100v100h-100ZM480 240h100v100h-100ZM480 0h100v100h-100ZM120 480h100v100h-100ZM240 480h100v100h-100ZM360 840h100v100h-100ZM360 720h100v100h-100ZM360 600h100v100h-100ZM360 480h100v100h-100ZM120 720h100v100h-100Z" {}
        }
    }
}

#[derive(Default, Deserialize)]
struct Scope {
    project: Option<String>,
    #[serde(default, rename = "continue")]
    follow: bool,
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
    if scope.follow && own {
        if let Some(part) = &row.continued_by {
            return crate::chat_html::protect(
                Redirect::to(&crate::continuations::part_href(part)).into_response(),
            );
        }
    }
    let pending = if own {
        match crate::session_moves::pending(&app.config.chat_store, &owner, &id).await {
            Ok(value) => value,
            Err(error) => return unavailable(error),
        }
    } else {
        false
    };
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
        @if let Some(agent) = &row.agent {
            p.oa-session-agent { (agent_mark()) " " (agent) }
        }
        p.oa-session-where { (where_.join(" · ")) }
        p.oa-session-state { (state.join(" · ")) }
        @if !online {
            p.oa-thread-notice #session-offline {
                (row.computer) " is offline. You can read this summary."
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
                (ButtonLink::new("Open the latest part", crate::continuations::part_href(chat))
                    .color(Color::Secondary)
                    .variant(ButtonVariant::Soft)
                    .size(ControlSize::Sm)
                    .pill(true))
            } @else if own && pending {
                p.oa-thread-notice { "Waiting for " (row.computer) " to continue this chat." }
            } @else if own && row.home_transfer && matches!(row.status, session_index::Status::Idle | session_index::Status::Errored) {
                form method="post" action=(format!("/chat/sessions/{id}/continue")) {
                    input type="hidden" name="csrf" value=(token);
                    (Button::new("Continue on the web")
                        .kind(ButtonType::Submit)
                        .color(Color::Secondary))
                }
            } @else if own && !row.home_transfer {
                p.oa-thread-notice { "Update OpenAgents on " (row.computer) " to continue this chat here." }
            } @else if own {
                p.oa-thread-notice { "Stop the current answer on " (row.computer) " before continuing here." }
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
        .head(html! { (crate::chat_html::head()) @if scope.follow && pending { meta http-equiv="refresh" content="3"; } })
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
    let row = found.row();
    if let Some(target) = &row.continued_by {
        return crate::chat_html::protect(
            Redirect::to(&crate::continuations::part_href(target)).into_response(),
        );
    }
    if !row.home_transfer {
        return refusal(
            StatusCode::CONFLICT,
            "Update OpenAgents on this computer before continuing here.",
        );
    }
    if !matches!(
        row.status,
        session_index::Status::Idle | session_index::Status::Errored
    ) {
        return refusal(
            StatusCode::CONFLICT,
            "Stop the current answer on this computer before continuing here.",
        );
    }
    let target = chat_home::PartRef {
        id: continuation_id(&owner, &id),
        home: chat_home::Home::Cloud,
    };
    if let Err(error) =
        crate::session_moves::begin(&app.config.chat_store, &owner, &id, target).await
    {
        return unavailable(error);
    }
    crate::chat_html::protect(
        Redirect::to(&format!("/chat/sessions/{id}?continue=true")).into_response(),
    )
}

fn now_unix() -> u64 {
    crate::chat_store::now_unix()
}
