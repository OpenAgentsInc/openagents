//! Log lines: what reaches them and what never does. The SDK logs on the
//! `jev` target; these tests write every record to a buffer and read it back.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use jev::{Client, Config, Error, Noul, Questions, RetryPolicy, SystemOneRequest};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// The response a good answer carries.
const BODY: &str =
    r#"{"model":"jev-latest","answers":{"q":{"type":"noul","noul":0.5}},"usage":{}}"#;

/// A log buffer a test reads after the calls run.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    /// Every line written so far.
    fn read(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("the log lock holds")).into_owned()
    }
}

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("the log lock holds")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Set a thread-scoped subscriber at the level given. `tokio::test` polls on
/// this thread, so the records the client and the spawned server emit land in
/// the buffer.
fn logged(level: Level, capture: Capture) -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture)
        .with_max_level(level)
        .with_ansi(false)
        .without_time()
        .finish();
    tracing::subscriber::set_default(subscriber)
}

/// One reply to send: a status, extra headers, and a body.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    /// A reply with no extra headers.
    fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_string(),
        }
    }

    /// The same reply with one more header.
    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
}

/// Answer each request with the next scripted reply on one port.
async fn serve(script: Vec<Reply>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the listener binds");
    let base = format!("http://{}", listener.local_addr().expect("the port reads"));
    tokio::spawn(async move {
        for reply in script {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                // Read until the request head and its body arrive.
                loop {
                    let Ok(read) = socket.read(&mut chunk).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..read]);
                    if request_complete(&bytes) {
                        break;
                    }
                }
                let mut head = format!(
                    "HTTP/1.1 {} Test\r\ncontent-length: {}\r\n",
                    reply.status,
                    reply.body.len()
                );
                for (name, value) in &reply.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("connection: close\r\n\r\n");
                head.push_str(&reply.body);
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    base
}

/// Whether a request head and the body it declares have all arrived.
fn request_complete(bytes: &[u8]) -> bool {
    let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&bytes[..end]);
    let declared = head
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|value| value.trim().to_string())
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    bytes.len() >= end + 4 + declared
}

/// A client pointed at the base, with no retries.
fn client(base: &str) -> Result<Client, jev::Error> {
    Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(base)
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )
}

/// One question set, enough for these tests.
fn asking() -> SystemOneRequest {
    SystemOneRequest::new(
        "a message",
        Questions::new().with("q", Noul::new("Yes or no?")),
    )
}

/// Every credential a request or a response carries is masked before a line
/// reaches the log: the API key, the caller's secret headers, and the
/// server's.
#[tokio::test]
async fn a_credential_never_reaches_a_log_line() -> Outcome {
    let capture = Capture::default();
    let _guard = logged(Level::DEBUG, capture.clone());

    let base = serve(vec![
        Reply::new(400, r#"{"error":"denied"}"#)
            .header("set-cookie", "response-credential")
            .header("x-visible", "response-visible"),
    ])
    .await;
    let mut secrets = HeaderMap::new();
    for name in [
        "x-client-secret",
        "x-access-token",
        "api-key",
        "cookie",
        "x-mixed-token",
    ] {
        secrets.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static("request-credential"),
        );
    }
    secrets.insert("x-visible", HeaderValue::from_static("request-visible"));
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            })
            .default_headers(secrets),
    )?;
    let Err(Error::Api(..)) = client.system_one(asking()).await else {
        unreachable!("a 400 raises an API error");
    };

    let output = capture.read();
    assert!(output.contains("***"), "{output}");
    assert!(output.contains("request-visible"), "{output}");
    assert!(output.contains("response-visible"), "{output}");
    for credential in [
        "ts-test-key-abcd1234",
        "request-credential",
        "response-credential",
    ] {
        assert!(
            !output.contains(credential),
            "{credential} reached {output}"
        );
    }
    Ok(())
}

/// One answered request writes one summary line at info: the request number,
/// the method, the status, and the request id.
#[tokio::test]
async fn an_answer_logs_one_info_summary() -> Outcome {
    let capture = Capture::default();
    let _guard = logged(Level::INFO, capture.clone());

    let base = serve(vec![
        Reply::new(200, BODY).header("x-typesafe-request-id", "req_01log"),
    ])
    .await;
    client(&base)?.system_one(asking()).await?;

    let output = capture.read();
    assert!(output.contains("the API answered"), "{output}");
    assert!(output.contains("request=1"), "{output}");
    assert!(output.contains("status=200"), "{output}");
    assert!(output.contains("req_01log"), "{output}");
    assert!(
        !output.contains("sending an attempt"),
        "the debug line stays below info: {output}"
    );
    Ok(())
}

/// Every request a client sends is numbered, so two calls read apart.
#[tokio::test]
async fn requests_are_numbered() -> Outcome {
    let capture = Capture::default();
    let _guard = logged(Level::INFO, capture.clone());

    let base = serve(vec![Reply::new(200, BODY), Reply::new(200, BODY)]).await;
    let client = client(&base)?;
    client.system_one(asking()).await?;
    client.system_one(asking()).await?;

    let output = capture.read();
    assert!(output.contains("request=1"), "{output}");
    assert!(output.contains("request=2"), "{output}");
    Ok(())
}

/// A retried call logs the wait before the retry runs.
#[tokio::test]
async fn a_retry_is_logged_before_the_next_attempt() -> Outcome {
    let capture = Capture::default();
    let _guard = logged(Level::INFO, capture.clone());

    let base = serve(vec![
        Reply::new(429, "{}").header("retry-after-ms", "0"),
        Reply::new(200, BODY),
    ])
    .await;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .retry(RetryPolicy {
                max_retries: 1,
                backoff_initial: Duration::ZERO,
                backoff_max: Duration::ZERO,
                backoff_jitter: 0.0,
                ..RetryPolicy::default()
            }),
    )?;
    client.system_one(asking()).await?;

    let output = capture.read();
    assert!(
        output.contains("waiting before the next attempt"),
        "{output}"
    );
    assert!(output.contains("retry=1"), "{output}");
    Ok(())
}

/// A level the SDK does not speak at stays silent.
#[tokio::test]
async fn nothing_is_logged_above_the_level() -> Outcome {
    let capture = Capture::default();
    let _guard = logged(Level::WARN, capture.clone());

    let base = serve(vec![Reply::new(200, BODY)]).await;
    client(&base)?.system_one(asking()).await?;

    let output = capture.read();
    assert!(output.is_empty(), "{output}");
    Ok(())
}
