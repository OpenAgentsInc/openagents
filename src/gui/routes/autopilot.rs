//! Autopilot routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(dashboard))
        .route("/sessions", web::get().to(sessions_page))
        .route("/metrics", web::get().to(metrics_page));
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
