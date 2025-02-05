use axum::{
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;
use tracing::info;

use crate::server::models::user::User;

use super::SESSION_COOKIE_NAME;

pub fn create_session_and_redirect(user: User) -> Response {
    let cookie = Cookie::build((SESSION_COOKIE_NAME, user.scramble_id))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(super::SESSION_DURATION_DAYS))
        .build();

    info!("Created session cookie: {}", cookie.to_string());

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("Redirecting to home with session cookie");
    (headers, Redirect::temporary("/")).into_response()
}

pub fn clear_session_and_redirect() -> Response {
    info!("Processing logout request");

    // Create cookie that will expire immediately
    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.to_string().parse().unwrap());

    info!("Created logout cookie: {}", cookie.to_string());
    info!("Redirecting to home after logout");

    (headers, Redirect::temporary("/")).into_response()
}

// Helper function to render templates
pub fn render_template(template: &str) -> String {
    // TODO: Implement actual template rendering
    // For now, just return a placeholder
    format!("Template: {}", template)
}