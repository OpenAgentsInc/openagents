use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::server::ws::handlers::chat::ChatHandlerService;

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
    State(handler): State<Arc<dyn ChatHandlerService>>,
    Form(form): Form<ToolToggle>,
) -> impl IntoResponse {
    let result = if form.enabled {
        handler.enable_tool(&form.tool).await
    } else {
        handler.disable_tool(&form.tool).await
    };

    match result {
        Ok(_) => (StatusCode::OK, "Tool status updated".to_string()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;
    use tower::ServiceExt;
    use crate::server::test_utils::*;
    use crate::tools::ToolError;

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
        let mut mock_handler = MockChatHandlerService::new();
        mock_handler
            .expect_enable_tool()
            .with(eq("test_tool"))
            .times(1)
            .returning(|_| Ok(()));

        let app = Router::new()
            .route("/tools/toggle", post(toggle_tool))
            .with_state(Arc::new(mock_handler) as Arc<dyn ChatHandlerService>);

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

    #[tokio::test]
    async fn test_toggle_tool_error() {
        let mut mock_handler = MockChatHandlerService::new();
        mock_handler
            .expect_enable_tool()
            .with(eq("test_tool"))
            .times(1)
            .returning(|_| Err(ToolError::InvalidArguments("test error".to_string())));

        let app = Router::new()
            .route("/tools/toggle", post(toggle_tool))
            .with_state(Arc::new(mock_handler) as Arc<dyn ChatHandlerService>);

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

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}