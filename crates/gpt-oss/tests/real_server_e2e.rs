//! End-to-end test against a real GPT-OSS server.
//!
//! Run with:
//!   GPT_OSS_E2E_URL=http://localhost:8000 \
//!   cargo test -p gpt-oss --test real_server_e2e -- --ignored

use gpt_oss::{GptOssClient, GptOssRequest};

#[tokio::test]
#[ignore]
async fn test_real_server_e2e() {
    let base_url = std::env::var("GPT_OSS_E2E_URL")
        .or_else(|_| std::env::var("GPT_OSS_URL"))
        .unwrap_or_default();
    if base_url.trim().is_empty() {
        panic!("Set GPT_OSS_E2E_URL or GPT_OSS_URL to run this test.");
    }

    let model = std::env::var("GPT_OSS_E2E_MODEL").unwrap_or_else(|_| "gpt-oss-20b".to_string());

    let client = GptOssClient::builder()
        .base_url(base_url)
        .default_model(&model)
        .build()
        .expect("Failed to build GPT-OSS client");

    let ready = client.health().await.expect("Health check failed");
    assert!(ready, "GPT-OSS server is not ready");

    let models = client.models().await.expect("Model list failed");
    assert!(!models.is_empty(), "Expected non-empty model list");

    let request = GptOssRequest {
        model,
        prompt: "Reply with a single short sentence.".to_string(),
        max_tokens: Some(32),
        temperature: Some(0.2),
        top_p: None,
        stop: None,
        stream: false,
        json_schema: None,
    };

    let response = client.complete(request).await.expect("Completion failed");
    assert!(!response.text.trim().is_empty(), "Response was empty");
}
