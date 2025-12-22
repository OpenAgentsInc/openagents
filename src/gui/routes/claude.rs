//! Claude API routes

use actix_web::{web, HttpResponse};
use ui::ClaudeStatus;

use crate::gui::state::AppState;

/// Configure Claude API routes
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/status", web::get().to(get_status));
}

/// Get current Claude auth status
async fn get_status(state: web::Data<AppState>) -> HttpResponse {
    let claude_account = state.claude_account.read().await;
    let claude_status = match claude_account.as_ref() {
        Some(account) => ClaudeStatus::logged_in(
            account.email.clone().unwrap_or_default(),
            account.organization.clone(),
            account.subscription_type.clone(),
            account.token_source.clone(),
            account.api_key_source.clone(),
        ),
        None => ClaudeStatus::not_logged_in(),
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(claude_status.build().into_string())
}
