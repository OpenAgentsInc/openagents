//! The synchronous client: the same calls as `Client`, from a caller that
//! runs no `tokio` runtime of its own.
#![cfg(feature = "blocking")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use indexmap::IndexMap;
use jev::{
    BlockingClient, Choice, Config, Entry, ListOptions, Noul, Questions, Score, SystemOneRequest,
};
use serde_json::json;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// The recorded response of a real `POST /v1/systemone`.
const RECORDED_RESPONSE: &str = include_str!("fixtures/systemone-response.json");

/// Serve `body` to the first request that arrives, recording what it carried.
fn serve_once(body: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the listener binds");
    let base = format!("http://{}", listener.local_addr().expect("the port reads"));
    let (send, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok((socket, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(socket.try_clone().expect("the socket clones"));
        let mut request = String::new();
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            request.push_str(&line);
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body_bytes = vec![0u8; length];
        if reader.read_exact(&mut body_bytes).is_err() {
            return;
        }
        request.push_str(&String::from_utf8_lossy(&body_bytes));
        let _ = send.send(request);
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n",
            body.len()
        );
        let mut socket = socket;
        let _ = socket.write_all(head.as_bytes());
        let _ = socket.write_all(body.as_bytes());
        let _ = socket.flush();
    });
    (base, received)
}

/// A client pointed at the base.
fn client(base: &str) -> Result<BlockingClient, jev::Error> {
    BlockingClient::new(Config::new().api_key("ts-test-key").base_url(base))
}

#[test]
fn the_blocking_client_sends_and_reads_one_request() -> Outcome {
    let (base, received) = serve_once(RECORDED_RESPONSE);
    let client = client(&base)?;
    let mut options = IndexMap::new();
    options.insert("billing".to_string(), Some(Entry::from("Money questions")));
    options.insert("technical".to_string(), Some(Entry::from("Broken things")));
    let questions = Questions::new()
        .with("department", Choice::new("Which team?", options))
        .with(
            "severity",
            Score::new(
                "How severe?",
                vec![Some(Entry::from("Cosmetic")), Some(Entry::from("Blocking"))],
            ),
        )
        .with("requestsRefund", Noul::new("Is a refund asked for?"));
    let response = client.system_one(SystemOneRequest::new(
        json!({"subject": "Double charge", "body": "I was charged twice."}),
        questions,
    ))?;
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(response.choice("department")?.choice, "billing");
    assert!((response.score("severity")?.score - 0.97).abs() < 1e-9);
    assert!((response.noul("requestsRefund")?.noul - 0.99).abs() < 1e-9);
    assert_eq!(response.usage.input_tokens, Some(471));

    let request = received.recv()?;
    assert!(request.starts_with("POST /v1/systemone"), "{request}");
    assert!(
        request.contains("authorization: Bearer ts-test-key"),
        "{request}"
    );
    assert!(request.contains("\"department\""), "{request}");
    Ok(())
}

#[test]
fn the_blocking_client_lists_models() -> Outcome {
    let body: &'static str = Box::leak(
        json!({"models": [
            {"name": "jev-latest", "description": "The current model", "release_date": "2026-09-01"},
        ]})
        .to_string()
        .into_boxed_str(),
    );
    let (base, received) = serve_once(body);
    let models = client(&base)?.list_models(ListOptions::new())?;
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].name, "jev-latest");

    let request = received.recv()?;
    assert!(request.starts_with("GET /v1/models"), "{request}");
    Ok(())
}
