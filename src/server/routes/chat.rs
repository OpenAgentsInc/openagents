use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::server::ws::handlers::chat::ChatHandler;

pub fn chat_routes() -> Router {
    Router::new()
        .route("/chat/:id", get(chat_session))
        .route("/chat/tools/toggle", post(toggle_tool))
}

#[derive(Deserialize)]
pub struct ToolToggle {
    tool: String,
    enabled: bool,
}

async fn chat_session(Path(_id): Path<Uuid>) -> Response {
    // Session ID is currently unused but will be needed for session management
    StatusCode::OK.into_response()
}

async fn toggle_tool(
    State(handler): State<Arc<ChatHandler>>,
    Form(form): Form<ToolToggle>,
) -> impl IntoResponse {
    if form.enabled {
        handler.enable_tool(&form.tool).await
            .map(|_| (StatusCode::OK, "Tool enabled".to_string()))
            .unwrap_or_else(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    } else {
        handler.disable_tool(&form.tool).await
            .map(|_| (StatusCode::OK, "Tool disabled".to_string()))
            .unwrap_or_else(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;
    use tower::ServiceExt;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_chat_session() {
        let app = chat_routes();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/chat/123e4567-e89b-12d3-a456-426614174000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_toggle_tool() {
        let mut mock_handler = MockChatHandler::new();
        mock_handler
            .expect_enable_tool()
            .with(eq("test_tool"))
            .times(1)
            .returning(|_| Ok(()));

        let app = Router::new()
            .route("/tools/toggle", post(toggle_tool))
            .with_state(Arc::new(mock_handler));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/tools/toggle")
                    .method("POST")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("tool=test_tool&enabled=true"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}