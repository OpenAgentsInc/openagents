//! Tests for GPT-OSS types serialization and deserialization

use gpt_oss::{GptOssRequest, GptOssResponse, GptOssStreamChunk};
use serde_json::json;

#[test]
fn test_request_serialization() {
    let request = GptOssRequest {
        model: "gpt-4o-mini".to_string(),
        prompt: "What is Rust?".to_string(),
        max_tokens: Some(100),
        temperature: Some(0.7),
        top_p: Some(0.9),
        stop: Some(vec!["END".to_string()]),
        stream: false,
    };

    let json_str = serde_json::to_string(&request).unwrap();
    assert!(json_str.contains("gpt-4o-mini"));
    assert!(json_str.contains("What is Rust?"));
    assert!(json_str.contains("100"));
    assert!(json_str.contains("0.7"));
}

#[test]
fn test_request_serialization_minimal() {
    let request = GptOssRequest {
        model: "gpt-4o-mini".to_string(),
        prompt: "Test".to_string(),
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        stream: false,
    };

    let json_str = serde_json::to_string(&request).unwrap();

    // Optional fields should be skipped in serialization
    assert!(!json_str.contains("max_tokens"));
    assert!(!json_str.contains("temperature"));
    assert!(!json_str.contains("top_p"));
    assert!(!json_str.contains("stop"));
}

#[test]
fn test_request_deserialization() {
    let json_str = r#"{
        "model": "gpt-4o-mini",
        "prompt": "Test prompt",
        "max_tokens": 50,
        "temperature": 0.5,
        "stream": true
    }"#;

    let request: GptOssRequest = serde_json::from_str(json_str).unwrap();

    assert_eq!(request.model, "gpt-4o-mini");
    assert_eq!(request.prompt, "Test prompt");
    assert_eq!(request.max_tokens, Some(50));
    assert_eq!(request.temperature, Some(0.5));
    assert!(request.stream);
}

#[test]
fn test_request_deserialization_minimal() {
    let json_str = r#"{
        "model": "gpt-4o-mini",
        "prompt": "Test"
    }"#;

    let request: GptOssRequest = serde_json::from_str(json_str).unwrap();

    assert_eq!(request.model, "gpt-4o-mini");
    assert_eq!(request.prompt, "Test");
    assert_eq!(request.max_tokens, None);
    assert_eq!(request.temperature, None);
    assert!(!request.stream); // Default is false
}

#[test]
fn test_response_deserialization() {
    let json_str = r#"{
        "id": "resp-123",
        "model": "gpt-4o-mini",
        "text": "Rust is a systems programming language.",
        "finish_reason": "stop",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 15,
            "total_tokens": 25
        }
    }"#;

    let response: GptOssResponse = serde_json::from_str(json_str).unwrap();

    assert_eq!(response.id, "resp-123");
    assert_eq!(response.model, "gpt-4o-mini");
    assert!(response.text.contains("Rust"));
    assert_eq!(response.finish_reason, Some("stop".to_string()));

    let usage = response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 15);
    assert_eq!(usage.total_tokens, 25);
}

#[test]
fn test_response_deserialization_minimal() {
    let json_str = r#"{
        "id": "resp-456",
        "model": "gpt-4o-mini",
        "text": "Response text"
    }"#;

    let response: GptOssResponse = serde_json::from_str(json_str).unwrap();

    assert_eq!(response.id, "resp-456");
    assert_eq!(response.model, "gpt-4o-mini");
    assert_eq!(response.text, "Response text");
    assert_eq!(response.finish_reason, None);
    assert_eq!(response.usage, None);
}

#[test]
fn test_stream_chunk_deserialization() {
    let json_str = r#"{
        "id": "chunk-789",
        "model": "gpt-4o-mini",
        "delta": "word",
        "finish_reason": null
    }"#;

    let chunk: GptOssStreamChunk = serde_json::from_str(json_str).unwrap();

    assert_eq!(chunk.id, "chunk-789");
    assert_eq!(chunk.model, "gpt-4o-mini");
    assert_eq!(chunk.delta, "word");
    assert_eq!(chunk.finish_reason, None);
}

#[test]
fn test_stream_chunk_with_finish_reason() {
    let json_str = r#"{
        "id": "chunk-final",
        "model": "gpt-4o-mini",
        "delta": "",
        "finish_reason": "stop"
    }"#;

    let chunk: GptOssStreamChunk = serde_json::from_str(json_str).unwrap();

    assert_eq!(chunk.delta, "");
    assert_eq!(chunk.finish_reason, Some("stop".to_string()));
}

#[test]
fn test_request_roundtrip() {
    let original = GptOssRequest {
        model: "test-model".to_string(),
        prompt: "test prompt".to_string(),
        max_tokens: Some(200),
        temperature: Some(0.8),
        top_p: Some(0.95),
        stop: Some(vec!["STOP".to_string(), "END".to_string()]),
        stream: true,
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: GptOssRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.model, original.model);
    assert_eq!(deserialized.prompt, original.prompt);
    assert_eq!(deserialized.max_tokens, original.max_tokens);
    assert_eq!(deserialized.temperature, original.temperature);
    assert_eq!(deserialized.top_p, original.top_p);
    assert_eq!(deserialized.stop, original.stop);
    assert_eq!(deserialized.stream, original.stream);
}

#[test]
fn test_response_clone() {
    let response = GptOssResponse {
        id: "test".to_string(),
        model: "model".to_string(),
        text: "text".to_string(),
        finish_reason: None,
        usage: None,
    };

    let cloned = response.clone();
    assert_eq!(cloned.id, response.id);
    assert_eq!(cloned.model, response.model);
}

#[test]
fn test_types_are_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<GptOssRequest>();
    assert_sync::<GptOssRequest>();
    assert_send::<GptOssResponse>();
    assert_sync::<GptOssResponse>();
    assert_send::<GptOssStreamChunk>();
    assert_sync::<GptOssStreamChunk>();
}
