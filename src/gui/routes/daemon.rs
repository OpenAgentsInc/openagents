//! Daemon routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(status));
}

async fn status() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Daemon Status</h1><p>Coming soon...</p>")
}
