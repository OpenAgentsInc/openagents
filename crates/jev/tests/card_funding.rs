//! Private native checkout errors cannot echo provider payloads or retry a charge.
use jev::{CardFundingAction, Client, Config, Error, RetryPolicy};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::test]
async fn failed_native_checkout_does_not_retry_or_retain_private_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let seen = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            let text = String::from_utf8_lossy(&bytes);
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text[..end]
                    .lines()
                    .find_map(|s| {
                        s.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + len {
                    break;
                }
            }
        }
        let body = r#"{"error":{"code":"card_funding_unavailable","message":"private_native_payload_canary"}}"#;
        socket.write_all(format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nX-Request-Id: private_native_payload_canary\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        drop(socket);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(250), listener.accept())
                .await
                .is_err(),
            "Native checkout unexpectedly retried."
        );
        String::from_utf8(bytes).unwrap()
    });
    let client = Client::new(
        Config::new()
            .api_key("sess_private_sdk_fixture")
            .base_url(origin)
            .retry(RetryPolicy {
                max_retries: 5,
                ..RetryPolicy::default()
            }),
    )
    .unwrap();
    let error = client
        .account()
        .card_funding(
            "workspace",
            "door",
            &CardFundingAction::Checkout {
                id: "original_native_quote".into(),
                approved: "a".repeat(64),
            },
        )
        .await
        .unwrap_err();
    assert!(!format!("{error:?}").contains("private_native_payload_canary"));
    match error {
        Error::Api(e) => {
            assert_eq!(e.status, 503);
            assert!(e.body.is_none());
            assert!(e.headers.is_empty());
        }
        _ => panic!("Native server refusal changed error kind."),
    }
    let request = seen.await.unwrap();
    assert!(request.starts_with("POST /v1/workspaces/workspace/card-funding/door "));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("x-workspace-id: workspace")
    );
}

#[tokio::test]
async fn native_checkout_rejects_route_escape_before_transport() {
    let client = Client::new(
        Config::new()
            .api_key("sess_private_sdk_fixture")
            .base_url("http://127.0.0.1:1"),
    )
    .unwrap();
    for workspace in ["..", "other/workspace", "space%2Fother", ""] {
        assert!(matches!(
            client
                .account()
                .card_funding(
                    workspace,
                    "door",
                    &CardFundingAction::Read {
                        id: "original".into()
                    }
                )
                .await,
            Err(Error::Config(_))
        ));
    }
}
