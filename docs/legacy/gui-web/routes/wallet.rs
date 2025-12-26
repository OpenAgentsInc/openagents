//! Wallet routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(dashboard))
        .route("/send", web::get().to(send_page))
        .route("/receive", web::get().to(receive_page))
        .route("/history", web::get().to(history_page));
}

async fn dashboard() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Wallet dashboard UI not yet implemented")
}

async fn send_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Send payment UI not yet implemented")
}

async fn receive_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Receive payment UI not yet implemented")
}

async fn history_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Transaction history UI not yet implemented")
}
