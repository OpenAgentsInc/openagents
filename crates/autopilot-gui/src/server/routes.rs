//! HTTP routes for autopilot GUI

use actix_web::{get, web, HttpResponse, Responder};
use crate::views::layout;

/// Configure routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(index);
}

/// Dashboard route
#[get("/")]
async fn index() -> impl Responder {
    let html = layout::page("Autopilot GUI", layout::dashboard());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
