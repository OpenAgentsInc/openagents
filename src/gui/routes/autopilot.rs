//! Autopilot routes

use actix_web::{web, HttpResponse};
use ui::FullAutoSwitch;

use crate::gui::state::AppState;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(dashboard))
        .route("/sessions", web::get().to(sessions_page))
        .route("/metrics", web::get().to(metrics_page));
}

/// Configure API routes (called from main routes)
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/toggle", web::post().to(toggle_full_auto));
}

/// Toggle full auto mode and return new switch HTML
async fn toggle_full_auto(state: web::Data<AppState>) -> HttpResponse {
    let mut full_auto = state.full_auto.write().await;
    *full_auto = !*full_auto;
    let new_state = *full_auto;
    drop(full_auto);

    // Return the new switch HTML for HTMX to swap
    let switch = FullAutoSwitch::new(new_state).build();
    HttpResponse::Ok()
        .content_type("text/html")
        .body(switch.into_string())
}

async fn dashboard() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Autopilot</h1><p>Coming soon...</p>")
}

async fn sessions_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Sessions</h1><p>Coming soon...</p>")
}

async fn metrics_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Metrics</h1><p>Coming soon...</p>")
}
