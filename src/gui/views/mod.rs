//! View templates for the unified GUI

mod layout;

use actix_web::{web, HttpResponse};
use ui::{ClaudeStatus, FullAutoSwitch};

use crate::gui::state::AppState;

pub use layout::base_layout;

/// Home page - black screen with FullAutoSwitch centered, ClaudeStatus bottom right
pub async fn home(state: web::Data<AppState>) -> HttpResponse {
    let full_auto = *state.full_auto.read().await;
    let switch = FullAutoSwitch::new(full_auto).build();

    // Get Claude info from state
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

    let content = format!(
        "{}{}",
        switch.into_string(),
        status.build_positioned().into_string()
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout(&content))
}
