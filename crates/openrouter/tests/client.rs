//! The client against a local fake server: no network, no key.

use std::sync::{Arc, Mutex};

use openrouter::{ApiErrorKind, ApiKey, ChatRequest, Client, Config, Error, Message};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One canned response: status, extra headers, body.
type Canned = (u16, Vec<(&'static str, &'static str)>, String);

/// Serves `responses` in order, one per connection, and records each
/// request's head and body.
async fn serve(responses: Vec<Canned>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        for (status, headers, body) in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                buffer.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buffer).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if buffer.len() >= end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            log.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&buffer).to_string());
            let mut head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                body.len()
            );
            for (name, value) in headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
            socket.shutdown().await.ok();
        }
    });
    (url, seen)
}

fn client(url: &str) -> Client {
    Client::new(Config::new(ApiKey::new("sk-or-test-secret")).base_url(url)).unwrap()
}

fn completion(content: &str) -> String {
    serde_json::json!({
        "id": "gen-1",
        "model": "openai/gpt-6-luna",
        "choices": [{"message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120, "cost": 0.00042,
                  "completion_tokens_details": {"reasoning_tokens": 8}}
    })
    .to_string()
}

#[derive(Debug, Deserialize)]
struct Next {
    rationale: String,
    commands: Vec<String>,
}

fn schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {"rationale": {"type": "string"},
        "commands": {"type": "array", "items": {"type": "string"}}},
        "required": ["rationale", "commands"], "additionalProperties": false})
}

#[tokio::test]
async fn a_structured_reply_parses_with_its_usage_and_cost() {
    let (url, seen) = serve(vec![(
        200,
        vec![],
        completion(r#"{"rationale":"look first","commands":["ls"]}"#),
    )])
    .await;
    let request = ChatRequest::new("openai/gpt-6-luna", vec![Message::user("go")]);
    let reply = client(&url)
        .structured::<Next>(request, "next_action", schema())
        .await
        .unwrap();
    assert_eq!(reply.value.rationale, "look first");
    assert_eq!(reply.value.commands, ["ls"]);
    assert_eq!(reply.model, "openai/gpt-6-luna");
    assert_eq!(reply.usage.cost, Some(0.00042));
    assert_eq!(reply.usage.prompt_tokens, 100);
    let request = &seen.lock().unwrap()[0];
    assert!(request.starts_with("POST /chat/completions"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-or-test-secret")
    );
    assert!(request.contains("\"json_schema\""));
    assert!(request.contains("\"strict\":true"));
}

#[tokio::test]
async fn a_reply_that_misses_the_schema_is_a_schema_error() {
    let (url, _) = serve(vec![(200, vec![], completion(r#"{"rationale":"x"}"#))]).await;
    let request = ChatRequest::new("m", vec![Message::user("go")]);
    let error = client(&url)
        .structured::<Next>(request, "next_action", schema())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Schema { .. }), "{error}");
}

#[tokio::test]
async fn an_error_status_keeps_its_class_and_message_and_never_the_key() {
    let (url, _) = serve(vec![(
        402,
        vec![],
        r#"{"error":{"code":402,"message":"Insufficient credits"}}"#.to_string(),
    )])
    .await;
    let request = ChatRequest::new("m", vec![Message::user("go")]);
    let error = client(&url).chat(&request).await.unwrap_err();
    match &error {
        Error::Api {
            kind,
            status,
            message,
        } => {
            assert_eq!(*kind, ApiErrorKind::PaymentRequired);
            assert_eq!(*status, 402);
            assert_eq!(message, "Insufficient credits");
        }
        other => panic!("{other:?}"),
    }
    assert!(!format!("{error} {error:?}").contains("secret"));
}

#[tokio::test]
async fn a_rate_limit_is_retried_after_its_wait() {
    let (url, seen) = serve(vec![
        (
            429,
            vec![("retry-after", "0")],
            r#"{"error":{"message":"slow down"}}"#.to_string(),
        ),
        (
            200,
            vec![],
            completion(r#"{"rationale":"ok","commands":[]}"#),
        ),
    ])
    .await;
    let request = ChatRequest::new("m", vec![Message::user("go")]);
    let reply = client(&url)
        .structured::<Next>(request, "next_action", schema())
        .await
        .unwrap();
    assert_eq!(reply.value.rationale, "ok");
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_bad_request_is_not_retried() {
    let (url, seen) = serve(vec![
        (400, vec![], r#"{"error":{"message":"bad"}}"#.to_string()),
        (200, vec![], completion("{}")),
    ])
    .await;
    let request = ChatRequest::new("m", vec![Message::user("go")]);
    let error = client(&url).chat(&request).await.unwrap_err();
    assert!(matches!(
        error,
        Error::Api {
            kind: ApiErrorKind::BadRequest,
            ..
        }
    ));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_provider_error_inside_a_200_is_an_error() {
    let (url, _) = serve(vec![(
        200,
        vec![],
        r#"{"error":{"code":502,"message":"upstream failed"}}"#.to_string(),
    )])
    .await;
    let mut config = Config::new(ApiKey::new("k")).base_url(&url);
    config.retries = 0;
    let request = ChatRequest::new("m", vec![Message::user("go")]);
    let error = Client::new(config)
        .unwrap()
        .chat(&request)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::Api {
                kind: ApiErrorKind::BadGateway,
                ..
            }
        ),
        "{error}"
    );
}
