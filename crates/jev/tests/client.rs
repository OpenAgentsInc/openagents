//! The client against a local HTTP server: what one request carries, what each
//! error status raises, what is retried, and what a timeout looks like.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use jev::{
    ApiErrorKind, Choice, Client, Config, Entry, Error, ListOptions, Noul, NoulCriteria, Questions,
    RetryPolicy, Score, SystemOneRequest,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// The recorded request of a real `POST /v1/systemone`.
const RECORDED_REQUEST: &str = include_str!("fixtures/systemone-request.json");

/// The recorded response that request produced.
const RECORDED_RESPONSE: &str = include_str!("fixtures/systemone-response.json");

/// One scripted reply.
struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
    delay: Duration,
    send: Delivery,
}

/// How much of the declared body the socket writes.
enum Delivery {
    /// The whole body, with an honest content length.
    Full,
    /// A content length longer than the body sent, then the socket closes.
    Short(usize),
    /// The head and the first bytes of the body, then nothing more.
    Stalled(usize),
}

impl Reply {
    /// A reply that answers at once with a JSON body.
    fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type", "application/json".to_string())],
            body: body.to_string(),
            delay: Duration::ZERO,
            send: Delivery::Full,
        }
    }

    /// The same reply with one more header.
    fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.push((name, value.to_string()));
        self
    }

    /// The same reply, sent after a wait.
    fn after(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// The same reply, promising a longer body than it sends before closing.
    fn truncated(mut self) -> Self {
        self.send = Delivery::Short(self.body.len() + 100);
        self
    }

    /// The same reply, sending its head and a few body bytes then holding.
    fn stalled(mut self) -> Self {
        self.send = Delivery::Stalled(self.body.len() + 100);
        self
    }
}

/// One request the server read.
#[derive(Debug)]
struct Seen {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Seen {
    /// One header of the request, lowercased by the reader.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// The body as JSON.
    fn json(&self) -> Result<Value, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
}

/// Answer each request with the next scripted reply, and record what arrived.
///
/// The server answers one request per reply and then stops listening, so a call
/// that sends more requests than the script holds fails to connect.
async fn serve(script: Vec<Reply>) -> Result<(String, Arc<Mutex<Vec<Seen>>>), std::io::Error> {
    let replies = std::sync::Mutex::new(script.into_iter());
    serve_with(move |_| {
        replies
            .lock()
            .expect("the script lock is not poisoned")
            .next()
    })
    .await
}

/// Answer each request with the reply the handler picks for it, and record
/// what arrived. The server keeps listening, so it suits calls that run at
/// the same time and calls whose reply depends on what they sent. A handler
/// that answers `None` drops the connection unread.
async fn serve_with(
    handler: impl Fn(&Seen) -> Option<Reply> + Send + Sync + 'static,
) -> Result<(String, Arc<Mutex<Vec<Seen>>>), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::clone(&seen);
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let recording = Arc::clone(&recording);
            let handler = Arc::clone(&handler);
            // Each connection is answered on its own, so a reply that waits
            // does not hold up the attempt that follows it.
            tokio::spawn(async move {
                let Some(request) = read_request(&mut socket).await else {
                    return;
                };
                let Some(reply) = handler(&request) else {
                    recording.lock().await.push(request);
                    return;
                };
                recording.lock().await.push(request);
                if !reply.delay.is_zero() {
                    tokio::time::sleep(reply.delay).await;
                }
                let (declared, sent, stall) = match &reply.send {
                    Delivery::Full => (reply.body.len(), reply.body.as_str(), false),
                    Delivery::Short(declared) => (*declared, reply.body.as_str(), false),
                    Delivery::Stalled(declared) => {
                        (*declared, &reply.body[..reply.body.len().min(8)], true)
                    }
                };
                let mut head = format!(
                    "HTTP/1.1 {} Test\r\ncontent-length: {}\r\nconnection: close\r\n",
                    reply.status, declared
                );
                for (name, value) in &reply.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("\r\n");
                head.push_str(sent);
                // The caller may have stopped waiting, so a write that fails
                // is the test's business rather than the server's.
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.flush().await;
                if stall {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            });
        }
    });
    Ok((base, seen))
}

/// Accept connections and drop each one unread, counting them.
async fn drop_connections() -> Result<(String, Arc<AtomicUsize>), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let hits = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&hits);
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            counting.fetch_add(1, Ordering::Relaxed);
            drop(socket);
        }
    });
    Ok((base, hits))
}

/// One request off the wire: its start line, its headers, and its body.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Seen> {
    let mut bytes = Vec::new();
    let split = loop {
        let mut chunk = [0; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
    };
    let head = String::from_utf8(bytes[..split].to_vec()).ok()?;
    let mut lines = head.split("\r\n");
    let mut start = lines.next()?.split_whitespace();
    let method = start.next()?.to_string();
    let target = start.next()?.to_string();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.to_lowercase(), value.trim().to_string()))
        })
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    while bytes.len() < split + 4 + length {
        let mut chunk = [0; 4096];
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Some(Seen {
        method,
        target,
        headers,
        body: bytes[split + 4..].to_vec(),
    })
}

/// A client that talks to `base` and retries nothing, unless a test says so.
fn client(base: &str, retry: RetryPolicy) -> Result<Client, Box<Error>> {
    // The failure is boxed because `Error` is wider than the value it
    // travels with, which clippy reports once the workspace unifies the
    // features of the crates it holds.
    Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(base)
            .retry(retry),
    )
    .map_err(Box::new)
}

/// A policy that retries without waiting, so a test reads the attempts rather
/// than the clock.
fn eager(max_retries: u32) -> RetryPolicy {
    RetryPolicy {
        max_retries,
        backoff_initial: Duration::from_millis(1),
        backoff_max: Duration::from_millis(1),
        ..RetryPolicy::default()
    }
}

/// No retries, for a test that reads one attempt.
fn once() -> RetryPolicy {
    RetryPolicy {
        max_retries: 0,
        ..RetryPolicy::default()
    }
}

/// One question about one state.
fn asking() -> SystemOneRequest {
    SystemOneRequest::new(
        "I was charged twice for one order.",
        Questions::new().with(
            "requestsRefund",
            Noul::new("Does the customer want money back?"),
        ),
    )
}

#[tokio::test]
async fn a_request_carries_the_headers_the_api_reads() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).header("x-typesafe-request-id", "req_01local"),
    ])
    .await?;
    let client = client(&base, once())?;
    let response = client.system_one(asking()).await?;
    assert_eq!(response.request_id(), Some("req_01local"));

    let seen = seen.lock().await;
    let request = seen.first().ok_or("the server read one request")?;
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/v1/systemone");
    assert_eq!(
        request.header("authorization"),
        Some("Bearer ts-test-key-abcd1234")
    );
    assert_eq!(request.header("accept"), Some("application/json"));
    assert_eq!(request.header("content-type"), Some("application/json"));
    let agent = format!("jev-rust/{}", jev::VERSION);
    assert_eq!(request.header("user-agent"), Some(agent.as_str()));
    assert_eq!(request.header("x-typesafe-sdk"), Some(agent.as_str()));
    let runtime = request
        .header("x-typesafe-runtime")
        .ok_or("the runtime header is sent")?;
    assert!(runtime.starts_with("rust/"), "{runtime}");
    assert!(
        request.header("x-typesafe-retry-count").is_none(),
        "a first attempt carries no retry count"
    );
    Ok(())
}

#[tokio::test]
async fn a_request_names_the_state_the_model_and_the_questions() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    client(&base, once())?.system_one(asking()).await?;
    let seen = seen.lock().await;
    let body = seen.first().ok_or("the server read one request")?.json()?;
    assert_eq!(body["state"], json!("I was charged twice for one order."));
    assert_eq!(body["model"], json!("jev-latest"));
    assert_eq!(body["questions"]["requestsRefund"]["type"], json!("noul"));
    Ok(())
}

#[tokio::test]
async fn a_request_names_the_model_the_call_asks_for() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(format!("{base}/"))
            .default_model("jev-1.12")
            .retry(once()),
    )?;
    assert_eq!(client.base_url(), base, "a trailing slash is dropped");
    assert_eq!(client.default_model(), "jev-1.12");
    client.system_one(asking().model("jev-1.13.0")).await?;
    let seen = seen.lock().await;
    let body = seen.first().ok_or("the server read one request")?.json()?;
    assert_eq!(body["model"], json!("jev-1.13.0"));
    Ok(())
}

/// The recorded exchange is replayed: the crate builds the same request body the
/// API answered with the recorded response.
#[tokio::test]
async fn the_recorded_request_is_the_body_this_crate_sends() -> Outcome {
    let recorded: Value = serde_json::from_str(RECORDED_REQUEST)?;
    let questions = Questions::new()
        .with(
            "department",
            Choice::new(
                json!({"question": "Which team should handle this?"}),
                IndexMap::new(),
            )
            .option(
                "billing",
                json!({"includes": ["Charges", "Invoices", "Refunds"]}),
            )
            .option("technical", json!(["Bugs", "Outages"]))
            .bare_option("other"),
        )
        .with(
            "severity",
            Score::new(json!(["How severe is the issue?"]), Vec::new())
                .level(json!({"meaning": "Cosmetic; functionality works"}))
                .level("Functionality impaired; workaround exists")
                .level("Blocking; no workaround"),
        )
        .with(
            "requestsRefund",
            Noul::with_criteria(
                "Is the customer requesting a refund?",
                NoulCriteria::new()
                    .when_true(json!({"meaning": "Explicit request for money back"}))
                    .when_false(Entry::Null),
            ),
        );

    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    client(&base, once())?
        .system_one(SystemOneRequest::new(
            Entry::from(recorded["state"].clone()),
            questions,
        ))
        .await?;
    let seen = seen.lock().await;
    let sent = seen.first().ok_or("the server read one request")?.json()?;
    assert_eq!(sent, recorded);
    Ok(())
}

/// A caller's header reaches the request, and one that names a header the API
/// reads to identify the request does not replace it.
#[tokio::test]
async fn a_caller_cannot_replace_the_headers_the_api_reads() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let mut defaults = HeaderMap::new();
    defaults.insert(
        HeaderName::from_static("x-tenant"),
        HeaderValue::from_static("acme"),
    );
    defaults.insert(
        HeaderName::from_static("x-typesafe-retry-count"),
        HeaderValue::from_static("9"),
    );
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .default_headers(defaults)
            .retry(once()),
    )?;
    assert_eq!(client.default_headers().len(), 2);

    let mut per_call = HeaderMap::new();
    per_call.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer another-key"),
    );
    per_call.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("text/plain"),
    );
    per_call.insert(
        HeaderName::from_static("x-tenant"),
        HeaderValue::from_static("beta"),
    );
    client.system_one(asking().headers(per_call)).await?;

    let seen = seen.lock().await;
    let request = seen.first().ok_or("the server read one request")?;
    assert_eq!(
        request.header("authorization"),
        Some("Bearer ts-test-key-abcd1234")
    );
    assert_eq!(request.header("accept"), Some("application/json"));
    assert_eq!(request.header("x-tenant"), Some("beta"));
    assert!(request.header("x-typesafe-retry-count").is_none());
    Ok(())
}

#[tokio::test]
async fn extra_body_fields_are_merged_last() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let mut extra = serde_json::Map::new();
    extra.insert("model".to_string(), json!("jev-from-extra"));
    extra.insert("future".to_string(), json!({"any": "shape"}));
    client(&base, once())?
        .system_one(asking().model("jev-1.12").extra_body(extra))
        .await?;
    let seen = seen.lock().await;
    let body = seen.first().ok_or("the server read one request")?.json()?;
    assert_eq!(body["model"], json!("jev-from-extra"));
    assert_eq!(body["future"], json!({"any": "shape"}));
    Ok(())
}

#[tokio::test]
async fn each_error_status_names_its_kind() -> Outcome {
    let cases = [
        (302, ApiErrorKind::Other),
        (400, ApiErrorKind::BadRequest),
        (401, ApiErrorKind::Authentication),
        (403, ApiErrorKind::PermissionDenied),
        (404, ApiErrorKind::NotFound),
        (408, ApiErrorKind::Other),
        (409, ApiErrorKind::Other),
        (418, ApiErrorKind::Other),
        (422, ApiErrorKind::UnprocessableEntity),
        (429, ApiErrorKind::RateLimit { retry_after: None }),
        (500, ApiErrorKind::InternalServer),
        (529, ApiErrorKind::InternalServer),
    ];
    for (status, kind) in cases {
        let body = json!({"error": {"message": format!("status {status}")}}).to_string();
        let (base, _) = serve(vec![
            Reply::new(status, &body).header("x-typesafe-request-id", "req_01fail"),
        ])
        .await?;
        let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
            unreachable!("status {status} raises an API error");
        };
        assert_eq!(error.status, status);
        assert_eq!(error.kind, kind);
        assert_eq!(error.request_id.as_deref(), Some("req_01fail"));
        assert_eq!(error.message(), format!("status {status}"));
        assert!(
            error.to_string().starts_with("POST http://127.0.0.1:"),
            "{error}"
        );
        assert!(
            error.to_string().ends_with("(request_id=req_01fail)"),
            "{error}"
        );
        assert!(!error.to_string().contains("ts-test-key"), "{error}");
    }
    Ok(())
}

/// The message comes out of the body the way both official SDKs read it.
#[tokio::test]
async fn the_message_is_read_out_of_the_body() -> Outcome {
    let cases = [
        (json!("plain text").to_string(), "plain text"),
        (json!({"error": "named"}).to_string(), "named"),
        (
            json!({"error": {"message": "nested"}}).to_string(),
            "nested",
        ),
        (json!({"message": "top"}).to_string(), "top"),
        (json!({"detail": "detail text"}).to_string(), "detail text"),
        (
            json!({"detail": {"message": "detail nested"}}).to_string(),
            "detail nested",
        ),
        (
            json!({"detail": [
                {"loc": ["body", "questions"], "msg": "field required", "type": "missing"},
                {"loc": ["body", "state"], "msg": "not a string", "type": "type"},
            ]})
            .to_string(),
            "questions: field required; state: not a string",
        ),
        (json!({"unknown": 1}).to_string(), "{\"unknown\":1}"),
        ("not json at all".to_string(), "not json at all"),
    ];
    for (body, expected) in cases {
        let (base, _) = serve(vec![Reply::new(422, &body)]).await?;
        let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
            unreachable!("a 422 raises an API error");
        };
        assert_eq!(error.message(), expected);
    }
    Ok(())
}

#[tokio::test]
async fn an_empty_body_says_there_was_none() -> Outcome {
    let (base, _) = serve(vec![Reply::new(500, "")]).await?;
    let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
        unreachable!("a 500 raises an API error");
    };
    assert_eq!(error.message(), "status code (no body)");
    assert_eq!(error.body, None);
    Ok(())
}

/// A message the SDK extracted is returned whole, however long. Only the
/// raw-body fallback is cut, at 200 characters with the cut marked. Both
/// official SDKs draw the line the same way.
#[tokio::test]
async fn a_long_message_is_whole_and_a_long_body_is_cut() -> Outcome {
    let long = "x".repeat(300);
    let (base, _) = serve(vec![Reply::new(400, &json!({"error": &long}).to_string())]).await?;
    let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
        unreachable!("a 400 raises an API error");
    };
    assert_eq!(error.message(), long, "an extracted message is not cut");

    let unstructured = format!("{{\"unknown\":\"{long}\"}}");
    let (base, _) = serve(vec![Reply::new(400, &unstructured)]).await?;
    let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
        unreachable!("a 400 raises an API error");
    };
    let message = error.message();
    assert_eq!(message.chars().count(), 201);
    assert!(message.ends_with('…'), "{message}");

    let (base, _) = serve(vec![Reply::new(400, &"x".repeat(201))]).await?;
    let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
        unreachable!("a 400 raises an API error");
    };
    assert_eq!(
        error.message().chars().count(),
        201,
        "a text body is the message, not a fallback"
    );
    Ok(())
}

#[tokio::test]
async fn a_retried_status_runs_again_and_says_which_attempt_it_is() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(503, &json!({"error": "busy"}).to_string()),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let response = client(&base, eager(2))?.system_one(asking()).await?;
    assert_eq!(response.model, "jev-1.13.0");
    let seen = seen.lock().await;
    assert_eq!(seen.len(), 2);
    assert!(seen[0].header("x-typesafe-retry-count").is_none());
    assert_eq!(seen[1].header("x-typesafe-retry-count"), Some("1"));
    Ok(())
}

#[tokio::test]
async fn the_retries_run_out_and_the_last_failure_is_returned() -> Outcome {
    let body = json!({"error": "busy"}).to_string();
    let (base, seen) = serve(vec![
        Reply::new(503, &body),
        Reply::new(503, &body),
        Reply::new(503, &body),
    ])
    .await?;
    let Err(Error::Api(error)) = client(&base, eager(2))?.system_one(asking()).await else {
        unreachable!("three failures raise the last one");
    };
    assert_eq!(error.status, 503);
    assert_eq!(seen.lock().await.len(), 3, "one attempt and two retries");
    Ok(())
}

#[tokio::test]
async fn a_status_the_policy_does_not_retry_runs_once() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(400, &json!({"error": "bad"}).to_string()),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let Err(Error::Api(error)) = client(&base, eager(2))?.system_one(asking()).await else {
        unreachable!("a 400 is not retried");
    };
    assert_eq!(error.status, 400);
    assert_eq!(seen.lock().await.len(), 1);
    Ok(())
}

/// A server delay is honored, so the retry waits as long as the server asked.
#[tokio::test]
async fn a_server_delay_sets_the_wait() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(429, &json!({"error": "slow down"}).to_string()).header("retry-after-ms", "120"),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let began = Instant::now();
    client(&base, eager(2))?.system_one(asking()).await?;
    assert!(
        began.elapsed() >= Duration::from_millis(120),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(seen.lock().await.len(), 2);
    Ok(())
}

/// The budget covers the first attempt and every wait, so a retry whose wait
/// would reach it does not run.
#[tokio::test]
async fn the_budget_stops_a_retry_before_it_waits() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(503, &json!({"error": "busy"}).to_string()),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let policy = RetryPolicy {
        max_retries: 2,
        backoff_initial: Duration::from_secs(30),
        budget: Some(Duration::from_millis(50)),
        ..RetryPolicy::default()
    };
    let Err(Error::Api(error)) = client(&base, policy)?.system_one(asking()).await else {
        unreachable!("the budget returns the last failure");
    };
    assert_eq!(error.status, 503);
    assert_eq!(seen.lock().await.len(), 1);
    Ok(())
}

/// A reply the loopback answers after about 200 milliseconds cannot succeed
/// under a 50-millisecond budget, however long the attempt's own timeout is.
/// This is the audit's reproduction: before the deadline applied, the call
/// waited the reply out and returned `accepted=true`.
#[tokio::test]
async fn the_budget_is_a_deadline_the_first_attempt_cannot_outlive() -> Outcome {
    let (base, _seen) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(200)),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(1))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(Duration::from_millis(50)),
                ..RetryPolicy::default()
            }),
    )?;
    let began = Instant::now();
    let Err(Error::Timeout { timeout }) = client.system_one(asking()).await else {
        unreachable!("a reply past the call's deadline cannot succeed");
    };
    assert!(
        timeout <= Duration::from_millis(50),
        "the attempt was capped by the remaining budget: {timeout:?}"
    );
    assert!(timeout > Duration::ZERO, "{timeout:?}");
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "the budget, not the attempt's own timeout, ended the call: {:?}",
        began.elapsed()
    );
    Ok(())
}

/// A body that stalls reads inside the attempt's timeout, and the budget caps
/// that timeout the same way it caps the head's arrival.
#[tokio::test]
async fn the_budget_bounds_a_body_that_stalls() -> Outcome {
    let (base, _seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE).stalled()]).await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(1))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(Duration::from_millis(60)),
                ..RetryPolicy::default()
            }),
    )?;
    let began = Instant::now();
    let Err(Error::Timeout { timeout }) = client.system_one(asking()).await else {
        unreachable!("a stalled body runs out of budget");
    };
    assert!(
        timeout <= Duration::from_millis(60),
        "the read was capped by the remaining budget: {timeout:?}"
    );
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "{:?}",
        began.elapsed()
    );
    Ok(())
}

/// A wait the server asks for that outlives the budget never runs: the call
/// returns the failure it already holds.
#[tokio::test]
async fn a_server_wait_past_the_budget_never_runs() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(429, "{}").header("retry-after-ms", "60000"),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .retry(RetryPolicy {
                max_retries: 2,
                budget: Some(Duration::from_millis(50)),
                ..RetryPolicy::default()
            }),
    )?;
    let began = Instant::now();
    let Err(Error::Api(error)) = client.system_one(asking()).await else {
        unreachable!("a wait past the budget returns the last failure");
    };
    assert_eq!(error.status, 429);
    assert_eq!(seen.lock().await.len(), 1, "no retry ran");
    assert!(
        began.elapsed() < Duration::from_secs(30),
        "the call did not wait out the server's delay: {:?}",
        began.elapsed()
    );
    Ok(())
}

/// A retry that runs gets only the time the call has left: its timeout is
/// the remaining budget, not the call's own.
#[tokio::test]
async fn a_retry_gets_only_the_time_the_call_has_left() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(503, "{}"),
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(500)),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(5))
            .retry(RetryPolicy {
                max_retries: 1,
                backoff_initial: Duration::from_millis(1),
                backoff_max: Duration::from_millis(1),
                backoff_jitter: 0.0,
                budget: Some(Duration::from_millis(80)),
                ..RetryPolicy::default()
            }),
    )?;
    let began = Instant::now();
    let Err(Error::Timeout { timeout }) = client.system_one(asking()).await else {
        unreachable!("the retry cannot outlive the call's deadline");
    };
    assert!(
        timeout <= Duration::from_millis(80),
        "the retry was capped by the remaining budget: {timeout:?}"
    );
    assert!(
        began.elapsed() < Duration::from_millis(500),
        "the call ended before the second reply arrived: {:?}",
        began.elapsed()
    );
    assert_eq!(seen.lock().await.len(), 2);
    Ok(())
}

/// The deadline holds on the unread path too: `system_one_raw` hands the
/// response back with its body unread, and the reply still cannot arrive
/// past the budget.
#[tokio::test]
async fn the_budget_bounds_the_raw_call_the_same_way() -> Outcome {
    let (base, _) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(200)),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(1))
            .retry(RetryPolicy {
                max_retries: 0,
                budget: Some(Duration::from_millis(50)),
                ..RetryPolicy::default()
            }),
    )?;
    let Err(Error::Timeout { timeout }) = client.system_one_raw(asking()).await else {
        unreachable!("a reply past the call's deadline cannot succeed");
    };
    assert!(timeout <= Duration::from_millis(50), "{timeout:?}");
    Ok(())
}

#[tokio::test]
async fn an_attempt_past_its_timeout_is_a_timeout() -> Outcome {
    let (base, _) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(400)),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_millis(60))
            .retry(once()),
    )?;
    assert_eq!(client.timeout(), Duration::from_millis(60));
    let Err(Error::Timeout { timeout }) = client.system_one(asking()).await else {
        unreachable!("an attempt past its timeout is a timeout");
    };
    assert_eq!(timeout, Duration::from_millis(60));
    Ok(())
}

/// A timeout named on the call overrides the client's, and the policy retries
/// the attempt that ran past it.
#[tokio::test]
async fn a_call_names_its_own_timeout_and_the_retry_follows_it() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).after(Duration::from_millis(400)),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url(&base)
            .timeout(Duration::from_secs(30))
            .retry(eager(1)),
    )?;
    let response = client
        .system_one(asking().timeout(Duration::from_millis(60)))
        .await?;
    assert_eq!(response.model, "jev-1.13.0");
    assert_eq!(seen.lock().await.len(), 2);
    Ok(())
}

#[tokio::test]
async fn a_request_that_reaches_nothing_is_a_connection_error() -> Outcome {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    drop(listener);
    let Err(Error::Connection { message, source }) =
        client(&base, once())?.system_one(asking()).await
    else {
        unreachable!("a closed port raises a connection error");
    };
    assert!(message.starts_with("connection error:"), "{message}");
    assert!(source.is_some());
    assert!(!message.contains("ts-test-key"), "{message}");
    Ok(())
}

#[tokio::test]
async fn the_questions_are_checked_before_any_request() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let empty = SystemOneRequest::new("any state", Questions::new());
    let Err(Error::Question { .. }) = client(&base, once())?.system_one(empty).await else {
        unreachable!("an empty set never reaches the API");
    };
    assert!(seen.lock().await.is_empty());
    Ok(())
}

#[tokio::test]
async fn the_raw_call_hands_back_the_response_unread() -> Outcome {
    let (base, _) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let response = client(&base, once())?.system_one_raw(asking()).await?;
    assert_eq!(response.status(), 200);
    let bytes = response.bytes().await?;
    assert_eq!(bytes.len(), RECORDED_RESPONSE.len());
    Ok(())
}

#[tokio::test]
async fn a_body_the_client_cannot_read_names_its_field() -> Outcome {
    let (base, _) = serve(vec![Reply::new(200, &json!({"answers": {}}).to_string())]).await?;
    let Err(Error::ResponseValidation {
        field_path, status, ..
    }) = client(&base, once())?.system_one(asking()).await
    else {
        unreachable!("a body without a model does not read");
    };
    assert_eq!(field_path, "model");
    assert_eq!(status, 200);
    Ok(())
}

#[tokio::test]
async fn the_models_listing_reads_the_envelope() -> Outcome {
    let body = json!({"models": [
        {"name": "jev-latest", "description": "The current model", "release_date": "2026-09-01"},
        {"name": "jev-1.12", "description": "The previous model", "release_date": "2026-06-15"},
    ]})
    .to_string();
    let (base, seen) = serve(vec![Reply::new(200, &body)]).await?;
    let client = client(&base, once())?;
    let models = client.models().list(ListOptions::new()).await?;
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].name, "jev-latest");
    assert_eq!(models[0].release_date, "2026-09-01");
    assert_eq!(models[1].description, "The previous model");
    let seen = seen.lock().await;
    let request = seen.first().ok_or("the server read one request")?;
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/v1/models");
    assert!(
        request.header("content-type").is_none(),
        "a request with no body names no content type"
    );
    Ok(())
}

#[tokio::test]
async fn a_models_body_of_another_shape_names_the_field() -> Outcome {
    let (base, _) = serve(vec![Reply::new(200, &json!({"data": []}).to_string())]).await?;
    let Err(Error::ResponseValidation { field_path, .. }) = client(&base, once())?
        .models()
        .list(ListOptions::new().timeout(Duration::from_secs(2)))
        .await
    else {
        unreachable!("a body without models does not read");
    };
    assert_eq!(field_path, "models");
    Ok(())
}

/// A call names its own policy, which replaces the client's.
#[tokio::test]
async fn a_call_names_its_own_retry_policy() -> Outcome {
    let body = json!({"error": "busy"}).to_string();
    let (base, seen) = serve(vec![Reply::new(503, &body), Reply::new(503, &body)]).await?;
    let client = client(&base, once())?;
    let Err(Error::Api(error)) = client.system_one(asking().retry(eager(1))).await else {
        unreachable!("the call's policy retries once");
    };
    assert_eq!(error.status, 503);
    assert_eq!(seen.lock().await.len(), 2);
    assert_eq!(
        client.retry().max_retries,
        0,
        "the client's policy is unchanged"
    );
    Ok(())
}

#[test]
fn a_client_refuses_settings_it_cannot_use() {
    let missing = Client::new(Config::new().base_url("https://api.typesafe.ai"));
    match missing {
        Err(Error::Config(message)) => assert!(message.contains("TYPESAFE_API_KEY"), "{message}"),
        // The environment of the machine running the tests may hold a key, and
        // a resolved client is the correct outcome then.
        Ok(_) => {}
        Err(other) => unreachable!("a missing key is a configuration failure: {other}"),
    }

    let scheme = Client::new(Config::new().api_key("k").base_url("ftp://api.typesafe.ai"));
    assert!(matches!(scheme, Err(Error::Config(_))));

    let unparsed = Client::new(Config::new().api_key("k").base_url("not a url"));
    assert!(matches!(unparsed, Err(Error::Config(_))));

    let zero = Client::new(Config::new().api_key("k").timeout(Duration::ZERO));
    assert!(matches!(zero, Err(Error::Config(_))));

    let jitter = Client::new(Config::new().api_key("k").retry(RetryPolicy {
        backoff_jitter: 2.0,
        ..RetryPolicy::default()
    }));
    assert!(matches!(jitter, Err(Error::Config(_))));
}

/// A client's own settings read back, and none of them carries the key.
#[test]
fn a_client_reads_back_its_settings() -> Outcome {
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key-abcd1234")
            .base_url("https://api.typesafe.ai///")
            .default_model("jev-1.12")
            .timeout(Duration::from_secs(3)),
    )?;
    assert_eq!(client.base_url(), "https://api.typesafe.ai");
    assert_eq!(client.default_model(), "jev-1.12");
    assert_eq!(client.timeout(), Duration::from_secs(3));
    assert_eq!(client.retry().max_retries, 2);
    assert!(client.default_headers().is_empty());
    let rendered = format!("{client:?}");
    assert!(!rendered.contains("ts-test-key"), "{rendered}");
    assert!(rendered.contains("***"), "{rendered}");
    Ok(())
}

/// The body a caller can read without a send is the body the wire carries:
/// state, model, and questions, with `extra_body` merged in last. Issue
/// #897 records this for every call a turn makes.
#[test]
fn the_body_a_caller_records_is_the_body_the_wire_carries() -> Outcome {
    let body = asking().body("jev-latest")?;
    assert_eq!(body["state"], json!("I was charged twice for one order."));
    assert_eq!(body["model"], json!("jev-latest"));
    assert_eq!(body["questions"]["requestsRefund"]["type"], json!("noul"));

    // A model the request names wins over the caller's default, and a field
    // merged in last lands beside the three.
    let body = asking()
        .model("jev-1.13.0")
        .extra_body(serde_json::Map::from_iter([(
            "caller".to_string(),
            json!("a-test"),
        )]))
        .body("jev-latest")?;
    assert_eq!(body["model"], json!("jev-1.13.0"));
    assert_eq!(body["caller"], json!("a-test"));
    Ok(())
}

/// The Python SDK's parametrized error-body cases: every shape names the same
/// message and keeps the raw body it came from.
#[tokio::test]
async fn the_error_body_edge_cases_both_official_sdks_cover() -> Outcome {
    let cases: Vec<(String, &str)> = vec![
        (String::new(), "status code (no body)"),
        ("null".to_string(), "status code (no body)"),
        ("[]".to_string(), "[]"),
        ("42".to_string(), "42"),
        (r#""json string""#.to_string(), "json string"),
        ("plain error text".to_string(), "plain error text"),
        ("not json {".to_string(), "not json {"),
        ("   ".to_string(), "   "),
        (r#"{}"#.to_string(), "{}"),
        (r#"{"foo":"bar"}"#.to_string(), r#"{"foo":"bar"}"#),
        (r#"[1,2,3]"#.to_string(), "[1,2,3]"),
        (
            r#"{"detail":{"message":"detail.message wins"}}"#.to_string(),
            "detail.message wins",
        ),
        (
            r#"{"detail":[{"msg":"one"},{"msg":"two"}]}"#.to_string(),
            "one; two",
        ),
        (
            r#"{"detail":[{"loc":["body","questions","q"],"msg":"bad question"}]}"#.to_string(),
            "questions.q: bad question",
        ),
        // A `detail` the extractor cannot read falls back to the body itself.
        (
            r#"{"detail":{"errors":[{"msg":"nested"}]}}"#.to_string(),
            r#"{"detail":{"errors":[{"msg":"nested"}]}}"#,
        ),
        (r#"{"detail":null}"#.to_string(), r#"{"detail":null}"#),
        (r#"{"detail":[]}"#.to_string(), r#"{"detail":[]}"#),
        // An empty extracted message is no message: the body is the message.
        (
            r#"{"error":"","message":"ignored"}"#.to_string(),
            r#"{"error":"","message":"ignored"}"#,
        ),
        (
            r#"{"detail":[null,42,{"msg":4}]}"#.to_string(),
            r#"{"detail":[null,42,{"msg":4}]}"#,
        ),
    ];
    for (body, message) in cases {
        let (base, _) = serve(vec![Reply::new(400, &body)]).await?;
        let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
            unreachable!("a 400 raises an API error");
        };
        assert_eq!(error.message(), message, "body {body}");
        let kept = error.body.as_ref().map(ToString::to_string);
        if body.is_empty() {
            assert_eq!(kept, None, "an empty body stays empty");
        } else {
            assert_eq!(kept.as_deref(), Some(body.as_str()), "body {body}");
        }
    }
    Ok(())
}

/// A rate limit names the wait the server asked for, and `retry_after` reads
/// it from the error too.
#[tokio::test]
async fn a_rate_limit_names_the_wait_the_server_asked_for() -> Outcome {
    let (base, _) = serve(vec![
        Reply::new(429, r#"{"error":"slow down"}"#).header("retry-after-ms", "125"),
    ])
    .await?;
    let Err(Error::Api(error)) = client(&base, once())?.system_one(asking()).await else {
        unreachable!("a 429 raises an API error");
    };
    assert_eq!(
        error.kind,
        ApiErrorKind::RateLimit {
            retry_after: Some(Duration::from_millis(125)),
        }
    );
    assert_eq!(error.retry_after(), Some(Duration::from_millis(125)));
    Ok(())
}

/// The endpoint an error names keeps the URL's path and drops its
/// credentials, query, and fragment.
#[tokio::test]
async fn the_endpoint_names_the_path_and_omits_url_credentials() -> Outcome {
    let (base, _) = serve(vec![Reply::new(400, r#"{"error":"nope"}"#)]).await?;

    // A base URL carrying a path prefix and credentials: the wire requests
    // keep the path, the error's endpoint drops the credentials.
    let credentialed = base.replacen("http://", "http://user:password@", 1) + "/prefix";
    let client = Client::new(
        Config::new()
            .api_key("k")
            .base_url(&credentialed)
            .retry(once()),
    )?;
    let Err(Error::Api(error)) = client.system_one(asking()).await else {
        unreachable!("a 400 raises an API error");
    };
    assert!(!error.endpoint.contains("password"), "{}", error.endpoint);
    assert!(
        !error.endpoint.contains('?') && !error.endpoint.contains('#'),
        "{}",
        error.endpoint
    );
    assert!(
        error.endpoint.ends_with("/prefix/v1/systemone"),
        "{}",
        error.endpoint
    );
    Ok(())
}

/// A models listing that cannot be read names the first field that broke,
/// inside `models` or inside one card.
#[tokio::test]
async fn the_models_envelope_names_the_first_field_it_cannot_read() -> Outcome {
    let cases: Vec<(String, &str)> = vec![
        ("[]".to_string(), "models"),
        (r#"{"models": null}"#.to_string(), "models"),
        (r#"{"models": {}}"#.to_string(), "models"),
        (r#"{"models": ["x"]}"#.to_string(), "models[0]"),
        (
            r#"{"models": [{"name": "m", "description": "d", "release_date": "r"}, 4]}"#
                .to_string(),
            "models[1]",
        ),
        (
            r#"{"models": [{"description": "d", "release_date": "r"}]}"#.to_string(),
            "models[0].name",
        ),
        (
            r#"{"models": [{"name": "m", "release_date": "r"}]}"#.to_string(),
            "models[0].description",
        ),
        (
            r#"{"models": [{"name": "m", "description": "d", "release_date": 4}]}"#.to_string(),
            "models[0].release_date",
        ),
    ];
    for (body, field) in cases {
        let (base, _) = serve(vec![Reply::new(200, &body)]).await?;
        let Err(Error::ResponseValidation { field_path, .. }) = client(&base, once())?
            .models()
            .list(ListOptions::new())
            .await
        else {
            unreachable!("a broken listing is a validation error: {body}");
        };
        assert_eq!(field_path, field, "body {body}");
    }
    Ok(())
}

/// A raw listing keeps the fields a card drops, and both shapes ride on one
/// client.
#[tokio::test]
async fn the_models_raw_listing_keeps_what_the_card_drops() -> Outcome {
    let body = json!({"models": [
        {"name": "jev-latest", "description": "The current model", "release_date": "2026-09-01",
         "context_window": 128_000},
    ]})
    .to_string();
    let (base, _) = serve(vec![Reply::new(200, &body), Reply::new(200, &body)]).await?;
    let client = client(&base, once())?;
    let models = client.models().list(ListOptions::new()).await?;
    assert_eq!(models[0].name, "jev-latest");
    let raw = client.models().list_raw(ListOptions::new()).await?;
    assert_eq!(raw.status, 200);
    let parsed: serde_json::Value = serde_json::from_slice(&raw.bytes)?;
    assert_eq!(parsed["models"][0]["context_window"], json!(128_000));
    Ok(())
}

/// A body that stops halfway is a connection error, and the retry brings the
/// call back.
#[tokio::test]
async fn a_body_that_breaks_mid_read_is_retried() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE).truncated(),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let response = client(&base, eager(2))?.system_one(asking()).await?;
    assert_eq!(response.model, "jev-1.13.0");
    let seen = seen.lock().await;
    assert_eq!(seen.len(), 2, "the broken body was retried");
    assert_eq!(seen[0].header("x-typesafe-retry-count"), None);
    assert_eq!(seen[1].header("x-typesafe-retry-count"), Some("1"));
    Ok(())
}

/// A body that never finishes is a timeout, and the error names the limit.
#[tokio::test]
async fn a_body_that_stalls_mid_read_times_out() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE).stalled()]).await?;
    let client = client(&base, once())?;
    let Err(Error::Timeout { timeout }) = client
        .system_one(asking().timeout(Duration::from_millis(60)))
        .await
    else {
        unreachable!("a stalled body times out");
    };
    assert_eq!(timeout, Duration::from_millis(60));
    assert_eq!(seen.lock().await.len(), 1, "nothing is retried under once");
    Ok(())
}

/// A 204 carries no body, and the raw response says so plainly.
#[tokio::test]
async fn a_204_carries_no_body() -> Outcome {
    let (base, _) = serve(vec![Reply::new(204, "")]).await?;
    let raw = client(&base, once())?.system_one_raw(asking()).await?;
    assert_eq!(raw.status().as_u16(), 204);
    assert!(raw.bytes().await?.is_empty());
    Ok(())
}

/// The last failure of an exhausted policy is the one returned, with the last
/// attempt's request id.
#[tokio::test]
async fn the_last_failure_of_an_exhausted_policy_is_the_one_returned() -> Outcome {
    let (base, _) = serve(vec![
        Reply::new(429, r#"{"error":"first"}"#).header("x-typesafe-request-id", "req_01one"),
        Reply::new(500, r#"{"error":"second"}"#).header("x-typesafe-request-id", "req_02two"),
        Reply::new(503, r#"{"error":"third"}"#).header("x-typesafe-request-id", "req_03three"),
    ])
    .await?;
    let Err(Error::Api(error)) = client(&base, eager(2))?.system_one(asking()).await else {
        unreachable!("an exhausted policy returns an API error");
    };
    assert_eq!(error.status, 503);
    assert_eq!(error.message(), "third");
    assert_eq!(error.request_id.as_deref(), Some("req_03three"));
    Ok(())
}

/// A connection error is retried like any retryable failure, and the last one
/// is the one returned when the retries run out.
#[tokio::test]
async fn a_connection_error_is_retried_then_the_last_one_is_returned() -> Outcome {
    let (base, hits) = drop_connections().await?;
    let Err(Error::Connection { .. }) = client(&base, eager(2))?.system_one(asking()).await else {
        unreachable!("a dropped connection is a connection error");
    };
    assert_eq!(
        hits.load(Ordering::Relaxed),
        3,
        "one attempt plus two retries"
    );
    Ok(())
}

/// A policy with no backoff still retries; the retry count marks the attempt.
#[tokio::test]
async fn zero_backoff_still_retries() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(503, r#"{"error":"down"}"#),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let policy = RetryPolicy {
        max_retries: 1,
        backoff_initial: Duration::ZERO,
        backoff_max: Duration::ZERO,
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    let response = client(&base, policy)?.system_one(asking()).await?;
    assert_eq!(response.model, "jev-1.13.0");
    let seen = seen.lock().await;
    assert_eq!(seen[0].header("x-typesafe-retry-count"), None);
    assert_eq!(seen[1].header("x-typesafe-retry-count"), Some("1"));
    Ok(())
}

/// Every attempt carries the count of the retries before it.
#[tokio::test]
async fn the_retry_count_names_every_attempt() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(503, "{}"),
        Reply::new(503, "{}"),
        Reply::new(503, "{}"),
    ])
    .await?;
    let Err(Error::Api(error)) = client(&base, eager(2))?.system_one(asking()).await else {
        unreachable!("an exhausted policy returns an API error");
    };
    assert_eq!(error.status, 503);
    let seen = seen.lock().await;
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].header("x-typesafe-retry-count"), None);
    assert_eq!(seen[1].header("x-typesafe-retry-count"), Some("1"));
    assert_eq!(seen[2].header("x-typesafe-retry-count"), Some("2"));
    Ok(())
}

/// A per-call retry policy retries only the statuses it names, beside calls
/// that keep the client's policy.
#[tokio::test]
async fn a_call_policy_retries_only_the_statuses_it_names() -> Outcome {
    let busy = r#"{"error":"busy"}"#;
    let (base, seen) = serve_with(move |request| {
        let status = if request.header("x-call") == Some("override") {
            409
        } else {
            429
        };
        Some(Reply::new(status, busy).header("retry-after-ms", "0"))
    })
    .await?;
    let call_policy = RetryPolicy {
        max_retries: 2,
        http_statuses: [409].into_iter().collect(),
        backoff_initial: Duration::ZERO,
        backoff_max: Duration::ZERO,
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    let client = client(
        &base,
        RetryPolicy {
            max_retries: 1,
            backoff_initial: Duration::ZERO,
            backoff_max: Duration::ZERO,
            backoff_jitter: 0.0,
            ..RetryPolicy::default()
        },
    )?;
    let mut headers = HeaderMap::new();
    headers.insert("x-call", HeaderValue::from_static("override"));
    let override_call = asking().retry(call_policy).headers(headers.clone());
    let mut inherited = HeaderMap::new();
    inherited.insert("x-call", HeaderValue::from_static("inherited"));

    let _ = client.system_one(override_call.clone()).await;
    let _ = client.system_one(asking().headers(inherited)).await;
    let _ = client.system_one(override_call).await;

    let seen = seen.lock().await;
    let count = |name: &str| {
        seen.iter()
            .filter(|request| request.header("x-call") == Some(name))
            .count()
    };
    assert_eq!(count("override"), 6, "two calls at three attempts each");
    assert_eq!(count("inherited"), 2, "one call at two attempts");
    for request in seen.iter() {
        if request.header("x-call") == Some("override") {
            continue;
        }
        // The inherited call saw 429s and retried under the client's policy.
        assert_eq!(request.target, "/v1/systemone");
    }
    Ok(())
}

/// A caller-supplied reqwest client's default headers merge in, but the SDK's
/// own headers still win.
#[tokio::test]
async fn a_supplied_http_client_cannot_override_the_sdk_headers() -> Outcome {
    let mut defaults = HeaderMap::new();
    defaults.insert("authorization", HeaderValue::from_static("Bearer wrong"));
    defaults.insert("accept", HeaderValue::from_static("text/plain"));
    defaults.insert("x-http-default", HeaderValue::from_static("kept"));
    let http = reqwest::Client::builder()
        .default_headers(defaults)
        .build()?;
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let client = Client::new(
        Config::new()
            .api_key("ts-test-key")
            .base_url(&base)
            .retry(once())
            .http_client(http),
    )?;
    client.system_one(asking()).await?;
    let seen = seen.lock().await;
    let request = seen.first().ok_or("the server read one request")?;
    assert_eq!(
        request.header("authorization"),
        Some("Bearer ts-test-key"),
        "the SDK's credential wins over the client's default"
    );
    assert_eq!(request.header("accept"), Some("application/json"));
    assert_eq!(
        request.header("x-http-default"),
        Some("kept"),
        "an unrelated client default reaches the wire"
    );
    Ok(())
}

/// A call the caller abandons sends no retry.
#[tokio::test]
async fn an_aborted_call_sends_no_retry() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(429, "{}").header("retry-after-ms", "200"),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let client = client(&base, eager(2))?;
    let mut call = Box::pin(client.system_one(asking()));
    tokio::select! {
        _ = &mut call => unreachable!("the first reply was a 429"),
        _ = tokio::time::sleep(Duration::from_millis(100)) => {}
    }
    drop(call);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        seen.lock().await.len(),
        1,
        "dropping the call cancels the retry"
    );
    Ok(())
}

/// Array and null entries reach the wire in state and in instructions, the
/// way the official SDKs send them.
#[tokio::test]
async fn array_and_null_entries_reach_the_wire() -> Outcome {
    let (base, seen) = serve(vec![
        Reply::new(200, RECORDED_RESPONSE),
        Reply::new(200, RECORDED_RESPONSE),
    ])
    .await?;
    let client = client(&base, once())?;

    let array = SystemOneRequest::new(
        json!(["part one", "part two"]),
        Questions::new().with(
            "requestsRefund",
            Noul::new(json!(["Is this two parts?", {"note": "yes it is"}])),
        ),
    );
    client.system_one(array).await?;

    let null = SystemOneRequest::new(
        Entry::Null,
        Questions::new().with("requestsRefund", Noul::new("x?")),
    );
    client.system_one(null).await?;

    let seen = seen.lock().await;
    let first = seen[0].json()?;
    assert_eq!(first["state"], json!(["part one", "part two"]));
    assert_eq!(
        first["questions"]["requestsRefund"]["instructions"],
        json!(["Is this two parts?", {"note": "yes it is"}])
    );
    let second = seen[1].json()?;
    assert_eq!(second["state"], serde_json::Value::Null);
    Ok(())
}

/// An extra body keeps nulls and nested values, and the shallow merge lets it
/// replace the fields the request built.
#[tokio::test]
async fn extra_body_keeps_nulls_and_replaces_shallow_fields() -> Outcome {
    let (base, seen) = serve(vec![Reply::new(200, RECORDED_RESPONSE)]).await?;
    let request = asking().extra_body(serde_json::Map::from_iter([
        ("model".to_string(), json!("jev-override")),
        ("state".to_string(), serde_json::Value::Null),
        ("future_option".to_string(), serde_json::Value::Null),
        ("nested".to_string(), json!({"enabled": true})),
    ]));
    client(&base, once())?.system_one(request).await?;
    let seen = seen.lock().await;
    let body = seen[0].json()?;
    assert_eq!(body["model"], json!("jev-override"));
    assert_eq!(body["state"], serde_json::Value::Null);
    assert_eq!(body["future_option"], serde_json::Value::Null);
    assert_eq!(body["nested"], json!({"enabled": true}));
    assert_eq!(body["questions"]["requestsRefund"]["type"], json!("noul"));
    Ok(())
}
