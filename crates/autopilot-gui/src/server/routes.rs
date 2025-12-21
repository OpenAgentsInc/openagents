//! HTTP routes for autopilot GUI

use actix_web::{delete, get, web, HttpResponse, Responder};
use crate::server::state::AppState;
use crate::server::ws;
use crate::storage::PermissionStorage;
use crate::views::{chat, layout, permissions_view};
use std::sync::Arc;

/// Configure routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(index)
        .service(chat_view)
        .service(permissions_manager)
        .service(delete_permission_rule)
        .route("/ws", web::get().to(ws::websocket));
}

/// Dashboard route
#[get("/")]
async fn index() -> impl Responder {
    let html = layout::page("Autopilot GUI", layout::dashboard());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Chat interface route
#[get("/chat")]
async fn chat_view() -> impl Responder {
    let html = layout::page("Chat - Autopilot GUI", chat::chat_interface());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Permission rules manager route
#[get("/permissions")]
async fn permissions_manager() -> impl Responder {
    // TODO: Get storage from app state
    // For now, create temporary storage
    let storage = match PermissionStorage::new("autopilot-permissions.db") {
        Ok(s) => Arc::new(s),
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to open permissions database: {}", e));
        }
    };

    let rules = match storage.get_all_rules().await {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Failed to load permission rules: {}", e));
        }
    };

    let html = permissions_view(rules);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

/// Delete a permission rule
#[delete("/api/permissions/{id}")]
async fn delete_permission_rule(
    path: web::Path<i64>,
    _state: web::Data<AppState>,
) -> impl Responder {
    let rule_id = path.into_inner();

    // TODO: Get storage from app state
    let storage = match PermissionStorage::new("autopilot-permissions.db") {
        Ok(s) => Arc::new(s),
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to open permissions database: {}", e)
            }));
        }
    };

    match storage.delete_rule(rule_id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to delete rule: {}", e)
        })),
    }
}
