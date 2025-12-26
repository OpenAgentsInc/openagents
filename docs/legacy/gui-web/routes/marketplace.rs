//! Marketplace routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(dashboard))
        .route("/compute", web::get().to(compute_page))
        .route("/skills", web::get().to(skills_page))
        .route("/data", web::get().to(data_page))
        .route("/trajectories", web::get().to(trajectories_page));
}

async fn dashboard() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Marketplace dashboard UI not yet implemented")
}

async fn compute_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Compute marketplace UI not yet implemented")
}

async fn skills_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Skills marketplace UI not yet implemented")
}

async fn data_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Data marketplace UI not yet implemented")
}

async fn trajectories_page() -> HttpResponse {
    HttpResponse::NotImplemented()
        .content_type("text/plain")
        .body("Trajectories UI not yet implemented")
}
