use opencode_sdk::{Event, OpencodeClient, OpencodeClientConfig, OpencodeServer, ServerOptions};
use serde_json::json;
use std::net::TcpListener;
use std::path::PathBuf;
use tokio::time::{Duration, timeout};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn session_json(session_id: &str) -> serde_json::Value {
    json!({
        "id": session_id,
        "projectID": "proj-1",
        "directory": "/repo",
        "title": "Test Session",
        "version": "1",
        "time": {
            "created": 1.0,
            "updated": 1.0
        }
    })
}

fn user_message_json(session_id: &str) -> serde_json::Value {
    json!({
        "id": "msg-1",
        "sessionID": session_id,
        "role": "user",
        "time": { "created": 1.0 },
        "agent": "default",
        "model": {
            "providerID": "anthropic",
            "modelID": "claude-sonnet-4"
        }
    })
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

#[tokio::test]
async fn test_health_connects_to_server() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/global/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "healthy": true,
            "version": "1.0.0"
        })))
        .mount(&server)
        .await;

    let client = OpencodeClient::new(OpencodeClientConfig::new().base_url(server.uri()))
        .expect("client build");

    let health = client.health().await.expect("health response");
    assert!(health.healthy);
    assert_eq!(health.version, "1.0.0");
}

#[tokio::test]
async fn test_session_prompt_and_messages() {
    let server = MockServer::start().await;
    let session_id = "ses-123";

    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_json(session_id)))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/session/{}/prompt", session_id)))
        .and(body_json(json!({
            "parts": [{
                "type": "text",
                "text": "Hello"
            }]
        })))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/session/{}/message", session_id)))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([user_message_json(session_id)])),
        )
        .mount(&server)
        .await;

    let client = OpencodeClient::new(OpencodeClientConfig::new().base_url(server.uri()))
        .expect("client build");

    let session = client
        .session_create(Default::default())
        .await
        .expect("session create");
    assert_eq!(session.id.as_str(), session_id);

    client
        .session_prompt(session_id, "Hello")
        .await
        .expect("prompt send");

    let messages = client.session_messages(session_id).await.expect("messages");
    assert_eq!(messages.len(), 1);

    let message_value = serde_json::to_value(&messages[0]).expect("serialize message");
    assert_eq!(message_value["sessionID"], session_id);
    assert_eq!(message_value["role"], "user");

    server.verify().await;
}

#[tokio::test]
async fn test_event_stream_receives_sse() {
    let server = MockServer::start().await;
    let session_id = "ses-456";

    let event_payload = json!({
        "type": "session.created",
        "info": session_json(session_id)
    });
    let body = format!("data: {}\n\n", event_payload.to_string());

    Mock::given(method("GET"))
        .and(path("/global/event"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;

    let client = OpencodeClient::new(OpencodeClientConfig::new().base_url(server.uri()))
        .expect("client build");

    let mut stream = client.events().await.expect("event stream");
    let next = timeout(Duration::from_secs(1), stream.next_event())
        .await
        .expect("event timeout")
        .expect("event result")
        .expect("event parse");

    match next {
        Event::SessionCreated { info } => {
            assert_eq!(info.id.as_str(), session_id);
        }
        other => panic!("Unexpected event: {:?}", other),
    }
}

#[tokio::test]
async fn test_provider_list() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/provider"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "anthropic",
                "name": "Anthropic",
                "models": [
                    {
                        "id": "claude-sonnet-4",
                        "name": "Claude Sonnet 4"
                    }
                ]
            }
        ])))
        .mount(&server)
        .await;

    let client = OpencodeClient::new(OpencodeClientConfig::new().base_url(server.uri()))
        .expect("client build");

    let providers = client.provider_list().await.expect("providers");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "anthropic");
    assert_eq!(providers[0].name, "Anthropic");
    assert!(
        providers[0]
            .models
            .as_ref()
            .unwrap()
            .iter()
            .any(|model| model.id == "claude-sonnet-4")
    );
}

fn stub_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_opencode_stub"))
}

#[tokio::test]
async fn test_server_spawn_and_health() {
    let port = free_port();
    let options = ServerOptions::new()
        .port(port)
        .hostname("127.0.0.1")
        .timeout_ms(2000)
        .executable(stub_executable());

    let mut server = OpencodeServer::spawn(options).await.expect("server spawn");

    assert!(server.is_running());
    assert_eq!(server.port(), port);
    assert!(server.url().ends_with(&format!(":{}", port)));

    server.close().await.expect("close server");
}

#[tokio::test]
async fn test_server_close_stops_health() {
    let port = free_port();
    let options = ServerOptions::new()
        .port(port)
        .hostname("127.0.0.1")
        .timeout_ms(2000)
        .executable(stub_executable());

    let server = OpencodeServer::spawn(options).await.expect("server spawn");

    let health_url = format!("http://127.0.0.1:{}/global/health", port);

    server.close().await.expect("close server");

    let client = reqwest::Client::new();
    let mut closed = false;
    for _ in 0..10 {
        let result = timeout(Duration::from_millis(200), client.get(&health_url).send()).await;
        if result.is_err() || result.unwrap().is_err() {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(closed, "server should stop accepting health checks");
}
