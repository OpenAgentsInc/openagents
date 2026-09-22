//! The shared contract fixtures: `docs/decision-models/fixtures/` records
//! one exchange per contract case, and every supported client replays
//! the same files. This suite is the Rust client's run — a stub server
//! answers each fixture's recorded response and the assertions read the
//! fixture's `expect` block, so the fixture and its meaning can never
//! drift apart silently.

use std::path::{Path, PathBuf};

use jev::{
    ApiErrorKind, CallOptions, ClassifyRequest, Client, Config, Error, JobSubmit, ListOptions,
    RetryPolicy,
};
use reqwest::Method;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Outcome = Result<(), Box<dyn std::error::Error>>;

/// The fixtures directory, relative to the crate.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/decision-models/fixtures")
}

/// One fixture file, parsed.
fn fixture(name: &str) -> Value {
    let path = fixtures().join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{} parses: {error}", path.display()))
}

/// A client pointed at the stub, with retries off — a fixture records
/// one exchange, not a retry sequence.
fn client(base: &str) -> Result<Client, Box<dyn std::error::Error>> {
    Ok(Client::new(
        Config::new()
            .api_key("oak_fixture")
            .base_url(base)
            .default_model("shared-kev")
            .retry(RetryPolicy {
                max_retries: 0,
                ..RetryPolicy::default()
            }),
    )?)
}

/// Serve a fixture's recorded response once and hand back the base URL
/// plus the request the client sent.
async fn replay(
    response: &Value,
) -> Result<(String, tokio::sync::oneshot::Receiver<Value>), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let status = response["status"].as_u64().unwrap_or(200) as u16;
    let body = serde_json::to_vec(&response["body"]).unwrap_or_default();
    let headers: Vec<(String, String)> = response["headers"]
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(name, value)| (name.clone(), value.as_str().unwrap_or_default().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let (send, seen) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 8192];
        // Read until the headers end, then the content length's worth of body.
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            request.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&request);
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text[..end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|rest| rest.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() - (end + 4) >= length {
                    break;
                }
            }
        }
        let text = String::from_utf8_lossy(&request).to_string();
        let head = text
            .split("\r\n\r\n")
            .next()
            .unwrap_or_default()
            .to_string();
        let body_text = text
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        let _ = send.send(serde_json::json!({"head": head, "body": body_text}));
        let mut out = format!("HTTP/1.1 {status} \r\ncontent-length: {}\r\n", body.len());
        for (name, value) in &headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str("\r\n");
        socket.write_all(out.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
    });
    Ok((base, seen))
}

/// The request a fixture names, as a method and path.
fn request(fixture: &Value) -> (Method, String) {
    let method =
        Method::from_bytes(fixture["request"]["method"].as_str().unwrap().as_bytes()).unwrap();
    let path = fixture["request"]["path"].as_str().unwrap().to_string();
    (method, path)
}

/// A classify report answers the partial-failure fixture: the run is a
/// success carrying three outcomes, each item in input order with its
/// own cause.
#[tokio::test]
async fn a_mixed_report_counts_every_outcome() -> Outcome {
    let fixture = fixture("classify-partial-failure");
    let (base, seen) = replay(&fixture["response"]).await?;
    let report = client(&base)?
        .classify()
        .run(ClassifyRequest::new(fixture["request"]["body"].clone())?)
        .await?;
    assert_eq!(report.outcome, "mixed");
    assert_eq!(report.outcomes.answered, 1);
    assert_eq!(report.outcomes.refused, 1);
    assert_eq!(report.outcomes.unavailable, 1);
    assert_eq!(report.outcomes.unattempted, 0);
    let order: Vec<&str> = report
        .results
        .iter()
        .map(|item| item.input.as_str())
        .collect();
    assert_eq!(order, ["t1", "t2", "t3"], "results keep input order");
    assert_eq!(report.results[1].cause.as_deref(), Some("quota_exhausted"));
    assert_eq!(report.results[2].cause.as_deref(), Some("door_unavailable"));
    assert_eq!(
        report.results[0].units[0].selected,
        serde_json::json!("billing")
    );
    assert_eq!(report.usage.as_ref().map(|usage| usage.forwards), Some(3));

    let sent = seen.await?;
    let (method, path) = request(&fixture);
    assert_eq!(method, Method::POST);
    assert!(
        sent["head"]
            .as_str()
            .unwrap()
            .starts_with(&format!("POST {path} "))
    );
    Ok(())
}

/// The review fixture's null reviewer confidence survives: the report
/// decodes, the review record stays whole, and the answer stands on the
/// reviewer.
#[tokio::test]
async fn a_reviewers_null_confidence_still_decodes() -> Outcome {
    let fixture = fixture("classify-review-null-confidence");
    let (base, _seen) = replay(&fixture["response"]).await?;
    let report = client(&base)?
        .classify()
        .run(ClassifyRequest::new(fixture["request"]["body"].clone())?)
        .await?;
    assert_eq!(report.outcome, "answered");
    let unit = &report.results[0].units[0];
    assert_eq!(unit.final_source.as_deref(), Some("reviewer"));
    let review = unit.review.as_ref().expect("the review record is kept");
    assert_eq!(review["raw"]["confidence"], Value::Null);
    assert_eq!(review["outcome"], "answered");
    Ok(())
}

/// The idempotency fixture: changed content under a settled key is a
/// typed 409, surfaced as `Error::Api` — never retried, never a second
/// job.
#[tokio::test]
async fn changed_content_under_a_settled_key_is_the_conflict() -> Outcome {
    let fixture = fixture("jobs-idempotency-conflict");
    let (base, _seen) = replay(&fixture["response"]).await?;
    let submission = JobSubmit::new(fixture["request"]["body"]["classify"].clone())
        .options(CallOptions::new().idempotency_key("job-submit-0001")?);
    let error = client(&base)?
        .jobs()
        .submit(submission)
        .await
        .expect_err("the conflict is an error");
    match error {
        Error::Api(error) => {
            assert_eq!(error.status, 409);
            assert_eq!(
                error
                    .body
                    .as_ref()
                    .and_then(|body| body.as_json())
                    .and_then(|body| body["error"]["code"].as_str()),
                Some("idempotency_conflict")
            );
        }
        other => panic!("the conflict is a typed API error, not {other}"),
    }
    Ok(())
}

/// The revocation fixture: a revoked key is `Error::Api` with
/// `Authentication` kind on every route — not a connection failure, not
/// a retry.
#[tokio::test]
async fn a_revoked_key_is_the_typed_refusal() -> Outcome {
    let fixture = fixture("key-revocation");
    let (base, _seen) = replay(&fixture["response"]).await?;
    let error = client(&base)?
        .models()
        .list(ListOptions::new().retry(RetryPolicy {
            max_retries: 3,
            ..RetryPolicy::default()
        }))
        .await
        .expect_err("revocation is an error");
    match error {
        Error::Api(error) => {
            assert_eq!(error.status, 401);
            assert!(matches!(error.kind, ApiErrorKind::Authentication));
        }
        other => panic!("revocation is a typed API error, not {other}"),
    }
    Ok(())
}
