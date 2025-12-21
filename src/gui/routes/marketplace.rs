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
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Marketplace</h1><p>Coming soon...</p>")
}

async fn compute_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Compute Marketplace</h1><p>Coming soon...</p>")
}

async fn skills_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Skills Marketplace</h1><p>Coming soon...</p>")
}

async fn data_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Data Marketplace</h1><p>Coming soon...</p>")
}

async fn trajectories_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Trajectories</h1><p>Coming soon...</p>")
}
