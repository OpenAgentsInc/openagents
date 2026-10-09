//! The homepage: a composer that starts a chat at `/chat/{uuid}`, with the
//! visitor's recent chats in the left panel. The header links `/download`;
//! the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use maud::html;

use crate::App;
use crate::ui_page::UiPage;

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// The composer sits centered in the content area; the left panel lists the
/// visitor's recent chats.
async fn home(State(app): State<App>, headers: HeaderMap) -> Response {
    let (owner, fresh) = super::chat::visitor(&headers);
    let selection = Default::default();
    let selectors = crate::composer::selectors_shown(&app, &headers, &selection).await;
    let content = html! {
        div.oa-home-stage {
            (super::chat::composer("/chat", "Start a chat", None, selectors, html! {}))
            (crate::composer::state_field(&app, &owner, &selection, false))
            input type="hidden" name="request_id" value=(super::chat::new_id()) form="chat-form";
            input type="hidden" name="csrf" value=(super::chat::csrf(&app,&owner)) form="chat-form";
        }
    };
    let mut page = UiPage::new("OpenAgents")
        .section("/")
        .path("/")
        .head(crate::chat_html::head())
        .content(content);
    // A visitor without the cookie has no chats yet.
    if fresh.is_none() {
        page = page.sidebar_section(super::chat::chat_list(&app, &owner, None, false, false).await);
    }
    let mut response = crate::chat_html::protect(page.respond(&headers));
    super::chat::cookie(&app, &owner, fresh.is_some(), &mut response);
    response
}
