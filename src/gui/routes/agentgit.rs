//! AgentGit routes

use actix_web::{web, HttpResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(dashboard))
        .route("/repos", web::get().to(repos_page))
        .route("/issues", web::get().to(issues_page));
}

async fn dashboard() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>AgentGit</h1><p>Coming soon...</p>")
}

async fn repos_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Repositories</h1><p>Coming soon...</p>")
}

async fn issues_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Issues</h1><p>Coming soon...</p>")
}
