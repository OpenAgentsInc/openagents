use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::server::ws::handlers::chat::ChatHandler;

#[derive(Deserialize)]
pub struct ToolToggle {
    tool: String,
}

pub fn routes() -> Router {
    Router::new()
        .route("/", get(chat_home))
        .route("/:id", get(chat_session))
        .route("/tools/toggle", post(toggle_tool))
}

async fn chat_home() -> impl IntoResponse {
    // Render chat home template
    axum::response::Html(include_str!("../../../templates/layouts/chat_base.html"))
}

async fn chat_session(Path(id): Path<Uuid>) -> impl IntoResponse {
    // Render chat session template
    axum::response::Html(include_str!("../../../templates/layouts/chat_base.html"))
}

async fn toggle_tool(
    State(chat_handler): State<Arc<ChatHandler>>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let tool_name = form.get("tool").unwrap_or(&"".to_string());
    let enabled = form.get("enabled").map(|v| v == "true").unwrap_or(false);

    if enabled {
        if let Err(e) = chat_handler.enable_tool(tool_name).await {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to enable tool: {}", e),
            );
        }
    } else {
        if let Err(e) = chat_handler.disable_tool(tool_name).await {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to disable tool: {}", e),
            );
        }
    }

    (axum::http::StatusCode::OK, "Tool status updated")
}