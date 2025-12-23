//! Daemon routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(status));
}

async fn status() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Daemon status UI not yet implemented")
}
