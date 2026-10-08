//! The homepage: a composer that starts a chat at `/chat/{uuid}`. The
//! header links `/download`; the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use maud::{PreEscaped, html};

use crate::App;
use crate::layout::document;

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// The composer sits centered between the header and the footer.
async fn home(State(app): State<App>, headers: HeaderMap) -> Response {
    let (owner, fresh) = super::chat::visitor(&headers);
    let body = html! {
        div.home-stage {
            (PreEscaped(super::chat::composer("/chat", "Start a chat", None)))
            div #composer-panel {}
            (crate::composer::state_field(&app, &owner, &Default::default(), false))
            input type="hidden" name="request_id" value=(super::chat::new_id()) form="chat-form";
            input type="hidden" name="csrf" value=(super::chat::csrf(&app,&owner)) form="chat-form";
        }
    };
    let document = document("OpenAgents", None, &body.into_string()).replace(
        "</head>",
        &format!("{}</head>", crate::chat_html::head().into_string()),
    );
    let mut response = crate::chat_html::protect(Html(document).into_response());
    super::chat::cookie(&app, &owner, fresh.is_some(), &mut response);
    response
}
