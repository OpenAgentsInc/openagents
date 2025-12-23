//! GitAfter routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(dashboard))
        .route("/repos", web::get().to(repos_page))
        .route("/issues", web::get().to(issues_page));
}

async fn dashboard() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("GitAfter dashboard UI not yet implemented")
}

async fn repos_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Repositories UI not yet implemented")
}

async fn issues_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Issues UI not yet implemented")
}
