use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use crate::server::services::repomap::{RepomapService, RepomapRequest};

#[derive(Debug, Serialize)]
struct RepomapHtml {
    content: String,
}

#[get("/repomap")]
pub async fn get_repomap() -> impl Responder {
    let html = include_str!("../../../templates/pages/repomap.html");
    HttpResponse::Ok()
        .content_type("text/html")
        .body(html)
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