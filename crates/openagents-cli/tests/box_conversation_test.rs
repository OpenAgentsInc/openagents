//! What `oa box` says when it cannot resolve the account's conversation.
//!
//! Every box subcommand starts by turning "no `--conversation`" into a
//! conversation id, so this is the first thing a reader sees when anything is
//! wrong, and the sentence it prints is the one they act on. There are two
//! different situations behind it and they need two different sentences:
//!
//!   * the server read the credential and answered — a `401` because the token
//!     carries `forge:write` and not `box:control`, say. Nothing is broken;
//!     the reader needs `--conversation`, or a token with the scope.
//!   * the request never got an answer — a `502` from the edge, a gateway
//!     error page, a dead socket. Nothing about the account is known, least of
//!     all that it has no conversation.
//!
//! Reporting the second as the first was observed against production: a
//! transient `502` on `GET /api/v1/conversation` printed "This deployment does
//! not report a conversation for the account", for an account whose
//! conversation resolved a minute earlier and a minute later.

use openagents_cli::box_client::BoxClient;
use openagents_cli::tracker::ApiError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A server that answers every request with one canned status and body.
async fn start_stub(status_line: &'static str, body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0u8; 8192];
            if socket.read(&mut buffer).await.unwrap_or(0) == 0 {
                continue;
            }
            let response = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    format!("http://127.0.0.1:{port}/api/v1")
}

/// A gateway failure is reported as a gateway failure.
#[tokio::test]
async fn a_five_hundred_from_the_conversation_route_is_not_reported_as_a_missing_conversation() {
    let base = start_stub("502 Bad Gateway", "<html><title>502</title></html>").await;
    let client = BoxClient::new(&base, Some("token".to_string()));

    let error = client
        .resolve_conversation_id()
        .await
        .expect_err("a 502 must not resolve to a conversation id");

    match error {
        ApiError::Refused {
            status, message, ..
        } => {
            assert_eq!(
                status, 502,
                "the status the server actually sent must survive"
            );
            assert!(
                !message.contains("does not report a conversation"),
                "a 502 is not an account without a conversation; the CLI said: {message}"
            );
        }
        other => panic!("expected the server's own refusal, got {other:?}"),
    }
}

/// A transport failure is reported as a transport failure. Port 1 refuses
/// connections, so nothing about the account is ever known here.
#[tokio::test]
async fn an_unreachable_api_is_not_reported_as_a_missing_conversation() {
    let client = BoxClient::new("http://127.0.0.1:1/api/v1", Some("token".to_string()));

    let error = client
        .resolve_conversation_id()
        .await
        .expect_err("an unreachable API must not resolve to a conversation id");

    assert!(
        matches!(error, ApiError::Transport { .. }),
        "an unreachable API must surface as a transport failure, got {error:?}"
    );
}

/// The refusal the caller *can* act on keeps its sentence. A `401` on both
/// routes is a real answer from the server: the credential was read and turned
/// down, and naming `--conversation` is the way past it.
#[tokio::test]
async fn a_refused_credential_still_names_the_flag_that_unblocks_the_caller() {
    let base = start_stub(
        "401 Unauthorized",
        r#"{"error":{"code":"invalid_api_token"}}"#,
    )
    .await;
    let client = BoxClient::new(&base, Some("token".to_string()));

    let error = client
        .resolve_conversation_id()
        .await
        .expect_err("a 401 must not resolve to a conversation id");

    match error {
        ApiError::Refused {
            status, message, ..
        } => {
            assert_eq!(status, 401);
            assert!(
                message.contains("--conversation"),
                "the refusal must name the flag that unblocks the caller; it said: {message}"
            );
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
