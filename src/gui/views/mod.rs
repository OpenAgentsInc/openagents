//! View templates for the unified GUI

mod layout;

use actix_web::{web, HttpResponse};
use ui::FullAutoSwitch;

use crate::gui::state::AppState;

pub use layout::base_layout;

/// Home page - black screen with FullAutoSwitch centered
pub async fn home(state: web::Data<AppState>) -> HttpResponse {
    let full_auto = *state.full_auto.read().await;
    let switch = FullAutoSwitch::new(full_auto).build();
    let content = switch.into_string();

    HttpResponse::Ok()
        .content_type("text/html")
        .body(base_layout(&content))
}
