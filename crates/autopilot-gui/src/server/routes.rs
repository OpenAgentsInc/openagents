//! HTTP routes for autopilot GUI

use actix_web::{get, web, HttpResponse, Responder};
use crate::views::{chat, layout};
use crate::server::ws;

/// Configure routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(index)
        .service(chat_view)
        .route("/ws", web::get().to(ws::websocket));
}

/// Dashboard route
#[get("/")]
async fn index() -> impl Responder {
    let html = layout::page("Autopilot GUI", layout::dashboard());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Chat interface route
#[get("/chat")]
async fn chat_view() -> impl Responder {
    let html = layout::page("Chat - Autopilot GUI", chat::chat_interface());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
