//! The homepage: a composer that starts a chat at `/chat/{uuid}`. The left
//! panel links `/download`; the Grid's screenshot is on `/docs/the-grid`.

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

/// The composer sits centered in the content area, above the footer.
async fn home(State(app): State<App>, headers: HeaderMap) -> Response {
    let (owner, fresh) = super::chat::visitor(&headers);
    let content = html! {
        div.oa-home-stage {
            (super::chat::composer("/chat", "Start a chat", None, html! {}))
            (crate::composer::state_field(&app, &owner, &Default::default(), false))
            input type="hidden" name="request_id" value=(super::chat::new_id()) form="chat-form";
            input type="hidden" name="csrf" value=(super::chat::csrf(&app,&owner)) form="chat-form";
        }
    };
    let page = UiPage::new("OpenAgents")
        .section("/")
        .path("/")
        .head(crate::chat_html::head())
        .content(content);
    let mut response = crate::chat_html::protect(page.respond(&headers));
    super::chat::cookie(&app, &owner, fresh.is_some(), &mut response);
    response
}
