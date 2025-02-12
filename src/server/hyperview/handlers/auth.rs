use axum::{
    http::{header, StatusCode},
    response::Response,
};
use tracing::info;

pub async fn mobile_logout() -> Response {
    info!("🔐 Starting mobile logout request");

    let cookie = crate::server::handlers::auth::session::clear_session_cookie();
    info!("🔐 Created clear cookie: {}", cookie);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .header(header::SET_COOKIE, cookie)
        .body(
            r###"<behavior
            xmlns="https://hyperview.org/hyperview"
            trigger="load"
            action="navigate"
            href="/templates/pages/auth/login.xml"
            new-stack="true"
            force-reset="true"
          />"###
                .into(),
        )
        .unwrap()
}
