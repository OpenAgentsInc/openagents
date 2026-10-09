//! The homepage, which is every new chat: it looks like a chat page, with
//! the composer docked at the bottom (the four starter questions over it)
//! and, where the thread would be, a grid of "learn about" cards
//! ([`LEARN`]). Sending a message starts a chat at `/chat/{uuid}`, which
//! has no cards. The visitor's recent chats are in the left panel. The
//! header links `/download`; the legal links sit centered under the composer
//! (only here); the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::html;
use openagents_ui::content::{LinkCard, LinkCards};
use openagents_ui::icons::Icon;
use serde::Deserialize;

use crate::App;
use crate::ui_page::UiPage;

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// The new chat's "learn about" cards: title, one line, where it goes, and
/// its icon. Every target is a live public page.
pub(crate) const LEARN: [(&str, &str, &str, Icon); 4] = [
    (
        "Explore the Verse",
        "A shared world you can walk around in with other players.",
        "/docs/verse",
        Icon::EarthTravelWorld,
    ),
    (
        "Meet Coder",
        "An AI coding assistant in your terminal.",
        "/docs/coder",
        Icon::Terminal,
    ),
    (
        "Tour the codebase",
        "Everything we build is open source on GitHub.",
        crate::layout::GITHUB,
        Icon::Code,
    ),
    (
        "Start with the basics",
        "What OpenAgents is and how to get it.",
        "/docs/what-is-openagents",
        Icon::BookOpen,
    ),
];

/// The [`LEARN`] cards as a grid.
fn learn_cards() -> LinkCards {
    LinkCards::new("Learn about OpenAgents").cards(
        LEARN
            .iter()
            .map(|(title, line, href, icon)| LinkCard::new(*title, *line, *href).icon(*icon)),
    )
}

#[derive(Default, Deserialize)]
struct Home {
    /// A project to start the chat in (a project group's "New chat").
    #[serde(default)]
    project: Option<String>,
}

/// The composer docks at the bottom as on a chat page, with the starter
/// questions over it; the cards fill the middle; the left panel lists the
/// visitor's recent chats. A signed-in person picks a project, a branch,
/// and where the message runs above the composer.
async fn home(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Home>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = query.map(|q| q.0).unwrap_or_default();
    let (owner, fresh) = super::chat::visitor(&app, &headers).await;
    let selection = Default::default();
    // Starter questions under the composer, as on the phone's new chat
    // (`crate::suggestions`). A visitor without the cookie has used none.
    let used = if fresh.is_none() {
        crate::suggestions::used(&app, &owner).await
    } else {
        Vec::new()
    };
    // Project, Branch, and Where it runs above the composer, for a
    // signed-in person (`crate::composer_row`).
    let row = crate::composer_row::home(&app, &headers, &owner, query.project.as_deref()).await;
    let content = html! {
        div.oa-thread-view {
            div.oa-thread {
                div.oa-thread-column.oa-home-stage { (learn_cards()) }
            }
        }
    };
    let dock = html! {
        (crate::suggestions::starters(&app, &owner, &used))
        (super::chat::composer("/chat", "Start a chat", row, html! {}))
        (crate::composer::state_field(&app, &owner, &selection, false))
        input type="hidden" name="request_id" value=(super::chat::new_id()) form="chat-form";
        input type="hidden" name="csrf" value=(super::chat::csrf(&app,&owner)) form="chat-form";
        (crate::ui_page::legal_links())
    };
    let mut page = UiPage::new("OpenAgents")
        .section("/")
        .path("/")
        .app()
        .head(crate::chat_html::head())
        .content(content)
        .composer(dock);
    // A visitor without the cookie has no chats yet.
    if fresh.is_none() {
        page = page.sidebar_section(super::chat::chat_list(&app, &owner, None, false, false).await);
    }
    let mut response = crate::chat_html::protect(page.respond(&headers));
    super::chat::cookie(&app, &owner, fresh.is_some(), &mut response);
    response
}
