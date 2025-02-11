use axum::{
    http::{header, StatusCode},
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;
use tracing::info;

use crate::server::models::user::User;

use super::SESSION_COOKIE_NAME;

const MOBILE_APP_SCHEME: &str = "onyx";

pub async fn create_session_and_redirect(user: User, is_mobile: bool) -> Response {
    info!("Creating session for user: {:?}", user);
    info!("Is mobile: {}", is_mobile);

    let session_token = user.scramble_id.clone();
    let cookie = Cookie::build((SESSION_COOKIE_NAME, session_token.clone().unwrap_or_default()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(7))
        .build();

    // For mobile app, redirect to deep link with session token
    let redirect_url = if is_mobile {
        info!("Redirecting to mobile app with token");
        format!("{}://{}", MOBILE_APP_SCHEME, session_token.unwrap_or_default())
    } else {
        info!("Redirecting to web app");
        "/".to_string()
    };

    info!("Redirect URL: {}", redirect_url);

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::LOCATION, redirect_url)
        .body(axum::body::Body::empty())
        .unwrap()
}

pub async fn clear_session_and_redirect() -> Response {
    info!("Clearing session");

    let cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build();

    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::LOCATION, "/login")
        .body(axum::body::Body::empty())
        .unwrap()
}

pub async fn render_login_template() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(axum::body::Body::from(include_str!(
            "../../../../templates/pages/login.html"
        )))
        .unwrap()
}

pub async fn render_signup_template() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(axum::body::Body::from(include_str!(
            "../../../../templates/pages/signup.html"
        )))
        .unwrap()
}
