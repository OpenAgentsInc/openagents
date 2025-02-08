use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;
use time::Duration;
use tracing::info;

use crate::server::{config::AppState, models::user::User};

use super::SESSION_COOKIE_NAME;

const MOBILE_APP_SCHEME: &str = "onyx";

#[derive(Debug, Deserialize)]
pub struct PlatformQuery {
    platform: Option<String>,
}

pub async fn create_session_and_redirect(user: User, is_mobile: bool) -> Response {
    info!("Creating session for user: {:?}", user);
    info!("Is mobile: {}", is_mobile);

    let session_token = user.scramble_id.clone();
    let cookie = Cookie::builder((SESSION_COOKIE_NAME, session_token.clone()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(7))
        .build();

    // For mobile app, redirect to deep link with session token
    let redirect_url = if is_mobile {
        info!("Redirecting to mobile app with token");
        format!(
            "{}://auth/success?token={}",
            MOBILE_APP_SCHEME, session_token
        )
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

pub async fn clear_session_and_redirect(
    State(_state): State<AppState>,
    Query(params): Query<PlatformQuery>,
) -> Response {
    info!("Clearing session");
    info!("Platform: {:?}", params.platform);

    let cookie = Cookie::builder((SESSION_COOKIE_NAME, ""))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0))
        .build();

    // For mobile app, redirect to deep link
    let redirect_url = if params.platform.as_deref() == Some("mobile") {
        info!("Redirecting to mobile app after logout");
        format!("{}://auth/logout", MOBILE_APP_SCHEME)
    } else {
        info!("Redirecting to web login");
        "/login".to_string()
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::LOCATION, redirect_url)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(r#"{"status":"ok"}"#))
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