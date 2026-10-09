//! The homepage, which is every new chat: it looks like a chat page, with
//! the composer docked at the bottom (the four starter questions over it)
//! and, where the thread would be, a grid of "learn about" cards
//! ([`learn`]). Sending a message starts a chat at `/chat/{uuid}`, which
//! has no cards. The visitor's recent chats are in the left panel. The
//! header links `/download`; the legal links sit centered under the composer
//! (only here); the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::html;
use openagents_chat::home_cards::{HOME_CARDS, HomeCard};
use openagents_ui::content::{LinkCard, LinkCards};
use openagents_ui::icons::Icon;
use serde::Deserialize;

use crate::App;
use crate::ui_page::UiPage;

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// Site pages a shared card may link that this site doesn't serve yet: a
/// card for one is left out, so no card goes nowhere. The roadmap card
/// waits for `/roadmap` (#11122); take its entry out when that page ships.
const NOT_YET_SERVED: &[&str] = &["/roadmap"];

/// What fills the grid to four while a shared card waits for its page.
const BASICS: HomeCard = HomeCard {
    id: "basics",
    title: "Start with the basics",
    line: "What OpenAgents is and how to get it.",
    href: "/docs/what-is-openagents",
    message: "What is OpenAgents?",
};

/// The new chat's "learn about" cards: the one shared list
/// ([`HOME_CARDS`], the phone shows the same), less any card whose page
/// isn't served yet, filled to four with [`BASICS`].
pub(crate) fn learn() -> Vec<HomeCard> {
    let mut cards: Vec<HomeCard> = HOME_CARDS
        .iter()
        .filter(|card| !NOT_YET_SERVED.contains(&card.href))
        .copied()
        .collect();
    if cards.len() < HOME_CARDS.len() {
        cards.push(BASICS);
    }
    cards
}

/// A card's icon, by its id.
fn icon(id: &str) -> Icon {
    match id {
        "verse" => Icon::EarthTravelWorld,
        "coder" => Icon::Terminal,
        "codebase" => Icon::Code,
        "roadmap" => Icon::Maps,
        _ => Icon::BookOpen,
    }
}

/// The [`learn`] cards as a grid.
fn learn_cards() -> LinkCards {
    LinkCards::new("Learn about OpenAgents").cards(
        learn()
            .into_iter()
            .map(|card| LinkCard::new(card.title, card.line, card.href).icon(icon(card.id))),
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
