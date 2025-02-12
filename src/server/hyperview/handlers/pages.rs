use crate::server::config::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
};

pub async fn main_page(State(state): State<AppState>) -> Response {
    // Check auth first
    let user = sqlx::query!(
        "SELECT * FROM users
         WHERE github_id IS NOT NULL
         ORDER BY last_login_at DESC NULLS LAST
         LIMIT 1"
    )
    .fetch_optional(&state.pool)
    .await;

    match user {
        Ok(Some(user)) if user.github_id.is_some() => {
            // User is authenticated, serve the main page
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
                .body(include_str!("../../../../templates/pages/main.xml").into())
                .unwrap()
        }
        _ => {
            // Not authenticated, force redirect to login
            auth_error_response("Not authenticated")
        }
    }
}

pub async fn login_page() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(include_str!("../../../../templates/pages/auth/login.xml").into())
        .unwrap()
}

fn auth_error_response(_message: &str) -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(r###"<?xml version="1.0" encoding="UTF-8" ?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="black" alignItems="center" justifyContent="center" />
      <style id="title" fontSize="24" color="white" marginBottom="32" />
      <style id="button" backgroundColor="white" padding="16" borderRadius="8" marginTop="16" width="240" alignItems="center" />
      <style id="buttonText" color="black" fontSize="16" fontWeight="600" />
      <style id="loading" color="white" fontSize="14" marginTop="16" />
    </styles>
    <body style="container">
      <text style="title">Welcome to OpenAgents</text>

      <!-- Loading State -->
      <text id="loading-text" style="loading" display="none">Connecting to GitHub...</text>

      <!-- GitHub Login Button -->
      <view style="button" id="login-button">
        <behavior
          trigger="press"
          action="open-url"
          href="/auth/github/login?platform=mobile"
          verb="GET"
          show-during-load="loading-text"
          hide-during-load="login-button"
        />
        <text style="buttonText">Continue with GitHub</text>
      </view>
    </body>
  </screen>
</doc>"###.into())
        .unwrap()
}