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
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Wallet Dashboard</h1><p>Coming soon...</p>")
}

async fn send_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Send Payment</h1><p>Coming soon...</p>")
}

async fn receive_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Receive Payment</h1><p>Coming soon...</p>")
}

async fn history_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Transaction History</h1><p>Coming soon...</p>")
}
