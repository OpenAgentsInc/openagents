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

    // Get real Claude account info from state
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

    let content = format!(
        "{}{}",
        switch.into_string(),
        claude_status.build_positioned().into_string()
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout(&content))
}
