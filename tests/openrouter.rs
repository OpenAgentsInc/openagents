use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use futures::StreamExt;
use openagents::server::services::OpenRouterService;
use serde_json::json;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_inference() {
    // Start mock server
    let app = Router::new().route("/chat/completions", post(mock_inference_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Create OpenRouter service with mock server URL
    let service =
        OpenRouterService::with_base_url("test_key".to_string(), format!("http://{}", addr));

    // Test successful streaming inference
    let mut stream = service.inference_stream("Test prompt".to_string()).await.unwrap();
    
    let mut response = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        response.push_str(&chunk);
    }
    assert_eq!(response.trim(), "Mock response");

    // Test authentication error
    let service =
        OpenRouterService::with_base_url("invalid_key".to_string(), format!("http://{}", addr));
    let result = service
        .inference_stream("Test prompt".to_string())
        .await;
    match result {
        Ok(_) => panic!("Expected authentication error"),
        Err(e) => assert!(e.to_string().contains("Authentication failed")),
    }
}

async fn mock_inference_handler(
    headers: axum::http::HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Verify request payload
    assert_eq!(payload["model"], "deepseek/deepseek-chat");
    assert!(payload["messages"].as_array().unwrap().len() > 0);

    // Verify required headers
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    assert!(headers.contains_key("HTTP-Referer"));
    assert!(headers.contains_key("Content-Type"));

    if auth_header.contains("invalid_key") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Authentication failed"
            })),
        );
    }

    // Check if streaming is requested
    let is_stream = payload["stream"].as_bool().unwrap_or(false);
    
    if is_stream {
        let stream_response = futures::stream::iter(vec![
            Ok::<_, std::io::Error>(format!("data: {}\n\n", 
                serde_json::to_string(&json!({
                    "choices": [{
                        "delta": {
                            "content": "Mock "
                        }
                    }]
                })).unwrap()
            )),
            Ok(format!("data: {}\n\n",
                serde_json::to_string(&json!({
                    "choices": [{
                        "delta": {
                            "content": "response"
                        }
                    }]
                })).unwrap()
            )),
            Ok("data: [DONE]\n\n".to_string())
        ]);
        
        let body = axum::body::Body::from_stream(stream_response);
        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .body(body)
            .unwrap()
    } else {
        // Return non-streaming mock response
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
}
