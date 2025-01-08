use actix_web::{get, HttpResponse, Responder};
use actix_files::NamedFile;
use std::path::PathBuf;

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