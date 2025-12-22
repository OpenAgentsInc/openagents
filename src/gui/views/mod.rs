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

    // For now show not logged in - will hook up to actual auth later
    let claude_status = ClaudeStatus::not_logged_in().build_positioned();

    let content = format!("{}{}", switch.into_string(), claude_status.into_string());

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout(&content))
}
