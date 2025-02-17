use crate::server::models::user::User;
use axum::{
    http::{header::SET_COOKIE, HeaderValue},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::{Duration, OffsetDateTime};
use tracing::info;
use time::format_description::well_known::Rfc2822;

pub const SESSION_COOKIE_NAME: &str = "session";
pub const SESSION_DURATION_DAYS: i64 = 30;
pub const MOBILE_APP_SCHEME: &str = "openagents://";

pub async fn create_session_and_redirect(user: &User, is_mobile: bool) -> Response {
    info!("Creating session for user ID: {}", user.id);

    let expiry = OffsetDateTime::now_utc() + Duration::days(SESSION_DURATION_DAYS);
    let cookie = create_session_cookie(&user.id.to_string(), expiry);

    let mut response = if is_mobile {
        let mobile_url = format!("{}auth?token={}", MOBILE_APP_SCHEME, user.id);
        info!("Redirecting to mobile URL: {}", mobile_url);
        Redirect::temporary(&mobile_url).into_response()
    } else {
        info!("Redirecting to web chat interface");
        Redirect::temporary("/chat").into_response()
    };

    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );

    response
}

pub async fn clear_session_and_redirect() -> Response {
    info!("Clearing session cookie and redirecting to login");

    let cookie = clear_session_cookie();
    let mut response = Redirect::temporary("/login").into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );

    response
}

pub fn create_session_cookie(session_id: &str, expiry: OffsetDateTime) -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE_NAME, session_id.to_string())
        .path("/")
        .secure(true)
        .http_only(true)
        .expires(expiry)
        .same_site(SameSite::Lax)
        .build()
}

pub fn clear_session_cookie() -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE_NAME, "")
        .path("/")
        .secure(true)
        .http_only(true)
        .expires(OffsetDateTime::now_utc() - Duration::days(1))
        .same_site(SameSite::Lax)
        .build()
}
