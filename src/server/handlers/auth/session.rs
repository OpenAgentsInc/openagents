use axum::{
    http::{header::SET_COOKIE, HeaderMap},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;
use tracing::info;
use askama::Template;

use crate::server::models::user::User;

use super::SESSION_COOKIE_NAME;

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    title: String,
}

#[derive(Template)]
#[template(path = "pages/signup.html")]
struct SignupTemplate {
    title: String,
}

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

pub async fn clear_session_and_redirect() -> Response {
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

pub fn render_login_template() -> Response {
    let template = LoginTemplate {
        title: "Log in".to_string(),
    };
    Html(template.render().unwrap()).into_response()
}

pub fn render_signup_template() -> Response {
    let template = SignupTemplate {
        title: "Sign up".to_string(),
    };
    Html(template.render().unwrap()).into_response()
}