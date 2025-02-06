use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use time::{Duration, OffsetDateTime};
use tower_cookies::{Cookie, Cookies};
use tracing::info;

use crate::server::models::user::User;

use super::SESSION_COOKIE_NAME;

pub fn create_session_and_redirect(user: User) -> Response {
    info!("Creating session for user: {:?}", user);

    let cookie = Cookie::build(SESSION_COOKIE_NAME, user.scramble_id.clone())
        .path("/")
        .secure(true)
        .http_only(true)
        .expires(OffsetDateTime::now_utc() + Duration::days(7))
        .build();

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::LOCATION, "/")
        .body(axum::body::Body::empty())
        .unwrap()
}

pub fn clear_session_and_redirect() -> Response {
    info!("Clearing session");

    let cookie = Cookie::build(SESSION_COOKIE_NAME, "")
        .path("/")
        .secure(true)
        .http_only(true)
        .expires(OffsetDateTime::now_utc() - Duration::days(1))
        .build();

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::LOCATION, "/login")
        .body(axum::body::Body::empty())
        .unwrap()
}

pub fn render_login_template() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(axum::body::Body::from(include_str!("../../../../templates/pages/login.html")))
        .unwrap()
}

pub fn render_signup_template() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(axum::body::Body::from(include_str!("../../../../templates/pages/signup.html")))
        .unwrap()
}