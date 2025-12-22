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
    let info = state.claude_info.read().await;

    let mut status = if info.loading {
        ClaudeStatus::loading()
    } else if info.authenticated {
        ClaudeStatus::authenticated()
    } else {
        ClaudeStatus::not_logged_in()
    };

    if let Some(ref model) = info.model {
        status = status.model(model.clone());
    }
    if let Some(ref version) = info.version {
        status = status.version(version.clone());
    }
    if let Some(sessions) = info.total_sessions {
        status = status.total_sessions(sessions);
    }
    if let Some(messages) = info.total_messages {
        status = status.total_messages(messages);
    }
    if let Some(tokens) = info.today_tokens {
        status = status.today_tokens(tokens);
    }

    // Add model usage
    for usage in &info.model_usage {
        status = status.add_model_usage(
            usage.model.clone(),
            usage.input_tokens,
            usage.output_tokens,
            usage.cache_read_tokens,
            usage.cache_creation_tokens,
        );
    }

    HttpResponse::Ok()
        .content_type("text/html")
        .body(status.build().into_string())
}
