//! The homepage: a composer that starts a chat at `/chat/{uuid}`. The
//! header links `/download`; the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::page;

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// The composer sits centered between the header and the footer.
async fn home() -> Response {
    let mut response = page(
        "OpenAgents",
        None,
        &format!(
            "<div class=\"home-stage\">{}</div>",
            super::chat::composer("/chat", "Start a chat")
        ),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(super::chat::COMPOSER_POLICY),
    );
    response
}
