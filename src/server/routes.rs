use actix_files::NamedFile;
use actix_web::{get, web, HttpResponse, Responder};
use std::path::PathBuf;

use crate::emailoptin::subscribe;

#[get("/favicon.ico")]
pub async fn favicon() -> impl Responder {
    let path: PathBuf = "./static/favicon.ico".into();
    NamedFile::open(path)
}

#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

#[get("/new")]
pub async fn new_page() -> impl Responder {
    let path: PathBuf = "./static/new.html".into();
    NamedFile::open(path)
}

#[get("/agents")]
pub async fn agents_page() -> impl Responder {
    let path: PathBuf = "./static/agents.html".into();
    NamedFile::open(path)
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(favicon)
        .service(new_page)
        .service(agents_page)
        .route("/subscriptions", web::post().to(subscribe));
}