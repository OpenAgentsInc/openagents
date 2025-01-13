use actix_files::NamedFile;
use actix_web::{get, web, HttpResponse, Responder};
use std::path::PathBuf;

use crate::emailoptin::subscribe;

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

#[get("/video-series")]
pub async fn video_series_page() -> impl Responder {
    let path: PathBuf = "./static/video-series.html".into();
    NamedFile::open(path)
}

#[get("/changelog")]
pub async fn changelog_page() -> impl Responder {
    let path: PathBuf = "./static/changelog.html".into();
    NamedFile::open(path)
}

#[get("/mobile-app")]
pub async fn mobile_app_page() -> impl Responder {
    let path: PathBuf = "./static/mobile-app.html".into();
    NamedFile::open(path)
}

#[get("/business")]
pub async fn business_page() -> impl Responder {
    let path: PathBuf = "./static/business.html".into();
    NamedFile::open(path)
}

#[get("/company")]
pub async fn company_page() -> impl Responder {
    let path: PathBuf = "./static/company.html".into();
    NamedFile::open(path)
}

#[get("/contact")]
pub async fn contact_page() -> impl Responder {
    let path: PathBuf = "./static/contact.html".into();
    NamedFile::open(path)
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check)
        .service(new_page)
        .service(agents_page)
        .service(video_series_page)
        .service(changelog_page)
        .service(mobile_app_page)
        .service(business_page)
        .service(company_page)
        .service(contact_page)
        .route("/subscriptions", web::post().to(subscribe));
}
