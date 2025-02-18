use crate::server::models::user::User;
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderValue},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;
use time::OffsetDateTime;
use tracing::info;

use crate::server::config::AppState;

pub const SESSION_COOKIE_NAME: &str = "session";
pub const SESSION_DURATION_DAYS: i64 = 30;
pub const MOBILE_APP_SCHEME: &str = "openagents://";

pub async fn create_session_and_redirect(
    state: &AppState,
    user: &User,
    is_mobile: bool,
) -> Response {
    info!("Creating session for user ID: {}", user.id);

    let expiry = OffsetDateTime::now_utc() + Duration::days(SESSION_DURATION_DAYS);
    let cookie = create_session_cookie(&user.id.to_string(), expiry);

    let mut response = if is_mobile {
        let mobile_url = format!("{}auth?token={}", MOBILE_APP_SCHEME, user.id);
        info!("Redirecting to mobile URL: {}", mobile_url);
        Redirect::temporary(&mobile_url).into_response()
    } else {
        info!("Redirecting to web chat interface");
        let chat_url = format!("{}/chat", state.frontend_url);
        Redirect::temporary(&chat_url).into_response()
    };

    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );

    response
}

pub async fn clear_session_and_redirect(State(state): State<AppState>) -> Response {
    info!("Clearing session cookie and redirecting to login");

    let cookie = clear_session_cookie();
    let login_url = format!("{}/login", state.frontend_url);
    let mut response = Redirect::temporary(&login_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );

    response
}

fn create_session_cookie(session_id: &str, expiry: OffsetDateTime) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, session_id.to_string());
    cookie.set_path("/");
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_expires(expiry);
    cookie.set_same_site(SameSite::Lax);
    cookie
}

fn clear_session_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, "");
    cookie.set_path("/");
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_expires(OffsetDateTime::now_utc() - Duration::days(1));
    cookie.set_same_site(SameSite::Lax);
    cookie
}
