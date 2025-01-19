use axum::{
    routing::post,
    Router,
    Json,
    http::StatusCode,
};
use openagents::server::services::OpenRouterService;
use serde_json::json;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_inference() {
    // Start mock server
    let app = Router::new().route("/api/v1/chat/completions", post(mock_inference_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Create OpenRouter service with mock server URL
    let service = OpenRouterService::with_base_url(
        "test_key".to_string(),
        format!("http://{}", addr)
    );

    // Test successful inference
    let result = service.inference("Test prompt".to_string()).await.unwrap();
    assert_eq!(result.output, "Mock response");

    // Test authentication error
    let service = OpenRouterService::with_base_url(
        "invalid_key".to_string(),
        format!("http://{}", addr)
    );
    let error = service.inference("Test prompt".to_string()).await.unwrap_err();
    assert!(error.to_string().contains("Authentication failed"));
}

async fn mock_inference_handler(
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Verify request payload
    assert_eq!(payload["model"], "anthropic/claude-2");
    assert!(payload["messages"].as_array().unwrap().len() > 0);

    // Check auth header for error case
    let headers = axum::http::HeaderMap::new();
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if auth_header.contains("invalid_key") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Authentication failed"
            }))
        );
    }

    // Return mock response
    (
        StatusCode::OK,
        Json(json!({
            "choices": [{
                "message": {
                    "content": "Mock response"
                }
            }]
        }))
    )
}
