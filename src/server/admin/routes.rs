use actix_web::{get, web, HttpResponse, Responder};
use serde_json::json;

#[get("/stats")]
pub async fn admin_stats() -> impl Responder {
    // TODO: Implement actual database stats
    HttpResponse::Ok().json(json!({
        "total_events": 0,
        "events_by_kind": {},
        "storage_usage": "0 MB",
        "index_usage": []
    }))
}

pub fn admin_config(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_stats);
}
