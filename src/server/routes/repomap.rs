use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::{Deserialize, Serialize};
use crate::server::services::repomap::{RepomapService, RepomapRequest};

#[derive(Template)]
#[template(path = "pages/repomap.html")]
struct RepomapTemplate {}

#[get("/repomap")]
pub async fn get_repomap() -> impl Responder {
    let template = RepomapTemplate {};
    match template.render() {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

#[post("/repomap/generate")]
pub async fn generate_repomap(
    req: web::Json<RepomapRequest>,
    data: web::Data<RepomapService>,
) -> impl Responder {
    match data.generate_repomap(req.repo_url.clone()).await {
        Ok(repomap) => HttpResponse::Ok().json(repomap),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to generate repomap: {}", e)
        }))
    }
}