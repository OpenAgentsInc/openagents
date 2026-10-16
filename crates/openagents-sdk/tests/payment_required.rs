use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, Request},
    response::Response,
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error, MAX_PAYMENT_REQUIRED_HEADER_BYTES,
    PaymentRequiredHeaderError, Session,
    computers::{self, CreateOptions, Refusal, RetryPolicy},
};
use serde_json::json;

const PRIVATE: &str = "private-invoice-request-fixture";

struct Step {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
    delay: Duration,
}

impl Step {
    fn challenge(header: Option<&str>) -> Self {
        let mut headers = HeaderMap::new();
        if let Some(value) = header {
            headers.insert("payment-required", HeaderValue::from_str(value).unwrap());
        }
        Self {
            status: 402,
            headers,
            // Payment errors must not depend on a JSON error envelope or read
            // this otherwise oversized response body.
            body: vec![b'x'; 1024 * 1024 + 1],
            delay: Duration::ZERO,
        }
    }
}

#[derive(Clone)]
struct Script {
    steps: Arc<Mutex<VecDeque<Step>>>,
    requests: Arc<Mutex<Vec<Seen>>>,
}

struct Seen {
    method: String,
    path: String,
    payment_signature: bool,
    key: Option<String>,
    body: Vec<u8>,
}

struct Server {
    client: Client,
    script: Script,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(State(script): State<Script>, request: Request<Body>) -> Response {
    assert_eq!(
        request.headers()["authorization"],
        format!("Bearer sess_{}", "a".repeat(64))
    );
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.unwrap().to_vec();
    script.requests.lock().unwrap().push(Seen {
        method: parts.method.to_string(),
        path: parts.uri.path().to_string(),
        payment_signature: parts.headers.contains_key("payment-signature"),
        key: parts
            .headers
            .get("idempotency-key")
            .map(|value| value.to_str().unwrap().to_owned()),
        body,
    });
    let step = script.steps.lock().unwrap().pop_front().unwrap();
    tokio::time::sleep(step.delay).await;
    let mut response = Response::builder().status(step.status);
    *response.headers_mut().unwrap() = step.headers;
    response.body(Body::from(step.body)).unwrap()
}

async fn mocked(mut steps: Vec<Step>, timeout: Duration) -> (Server, Session) {
    steps.insert(
        0,
        Step {
            status: 200,
            headers: HeaderMap::new(),
            body: serde_json::to_vec(
                &json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}),
            )
            .unwrap(),
            delay: Duration::ZERO,
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let script = Script {
        steps: Arc::new(Mutex::new(steps.into())),
        requests: Arc::new(Mutex::new(vec![])),
    };
    let router = Router::new().fallback(serve).with_state(script.clone());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
    let session = Client::new(deployment.clone())
        .unwrap()
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    script.requests.lock().unwrap().clear();
    (
        Server {
            client: Client::with_timeouts(deployment, timeout, timeout).unwrap(),
            script,
            task,
        },
        session,
    )
}

#[tokio::test]
async fn general_and_computer_errors_keep_the_exact_header_without_reading_or_logging_the_body() {
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!(r#"{{"x402Version":2,"private":"{PRIVATE}"}}"#),
    );
    let (server, session) = mocked(
        vec![
            Step::challenge(Some(&encoded)),
            Step::challenge(Some(&encoded)),
        ],
        Duration::from_secs(2),
    )
    .await;
    let error = server.client.account(&session).await.unwrap_err();
    assert!(matches!(error, Error::PaymentRequired(_)));
    assert_eq!(
        error.payment_required().unwrap().header(),
        Ok(encoded.as_str())
    );
    assert!(!format!("{error:?} {error}").contains(&encoded));
    let challenge = error.payment_required().unwrap();
    assert!(!format!("{challenge:?} {challenge}").contains(&encoded));
    assert!(oa_copy::violations(&error.to_string(), &[]).is_empty());

    let computers = server.client.computers(&session).unwrap();
    let request = computers.create_request(CreateOptions::default()).unwrap();
    let error = computers
        .create(&request, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, computers::Error::PaymentRequired(_)));
    assert_eq!(error.status(), Some(402));
    assert_eq!(error.refusal(), Some(Refusal::PaymentRequired));
    assert_eq!(
        error.payment_required().unwrap().header(),
        Ok(encoded.as_str())
    );
    assert!(!format!("{error:?} {error}").contains(&encoded));
    assert!(oa_copy::violations(&error.to_string(), &[]).is_empty());
    let seen = server.script.requests.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].path.as_str()),
        ("GET", "/v1/account")
    );
    assert_eq!(
        (seen[1].method.as_str(), seen[1].path.as_str()),
        ("POST", "/v1/computers")
    );
    assert!(seen.iter().all(|request| !request.payment_signature));
    assert!(server.script.steps.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_repeated_non_text_and_oversized_headers_remain_typed_402_errors() {
    let mut repeated = Step::challenge(Some("first"));
    repeated
        .headers
        .append("payment-required", HeaderValue::from_static("second"));
    let mut non_text = Step::challenge(None);
    non_text
        .headers
        .insert("payment-required", HeaderValue::from_bytes(&[255]).unwrap());
    let oversized = "x".repeat(MAX_PAYMENT_REQUIRED_HEADER_BYTES + 1);
    let (server, session) = mocked(
        vec![
            Step::challenge(None),
            repeated,
            non_text,
            Step::challenge(Some(&oversized)),
        ],
        Duration::from_secs(2),
    )
    .await;
    for expected in [
        PaymentRequiredHeaderError::Missing,
        PaymentRequiredHeaderError::Repeated,
        PaymentRequiredHeaderError::InvalidText,
        PaymentRequiredHeaderError::TooLarge,
    ] {
        let error = server.client.account(&session).await.unwrap_err();
        assert!(matches!(error, Error::PaymentRequired(_)), "{error:?}");
        assert_eq!(error.payment_required().unwrap().header(), Err(expected));
    }
    assert_eq!(server.script.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn the_header_limit_is_inclusive_and_does_not_require_a_protocol_parser() {
    let maximum = "x".repeat(MAX_PAYMENT_REQUIRED_HEADER_BYTES);
    let (server, session) = mocked(
        vec![Step::challenge(Some(&maximum))],
        Duration::from_secs(2),
    )
    .await;
    let error = server.client.account(&session).await.unwrap_err();
    assert_eq!(
        error.payment_required().unwrap().header(),
        Ok(maximum.as_str())
    );
    assert!(!format!("{error:?} {error}").contains(&maximum));
}

#[tokio::test]
async fn a_402_after_a_lost_mutation_response_keeps_the_original_recovery_identity() {
    let mut lost = Step::challenge(Some("first-challenge"));
    lost.delay = Duration::from_millis(250);
    let (server, session) = mocked(
        vec![lost, Step::challenge(Some(PRIVATE))],
        Duration::from_millis(40),
    )
    .await;
    let computers = server
        .client
        .computers(&session)
        .unwrap()
        .with_retries(RetryPolicy {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        })
        .unwrap();
    let request = computers.create_request(CreateOptions::default()).unwrap();
    let error = computers
        .create(&request, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, computers::Error::Uncertain { key, .. } if key.as_deref()==Some(request.key().as_str()))
    );
    assert_eq!(error.status(), Some(402));
    assert_eq!(error.payment_required().unwrap().header(), Ok(PRIVATE));
    assert!(!format!("{error:?} {error}").contains(PRIVATE));
    let seen = server.script.requests.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].key, seen[1].key);
    assert_eq!(seen[0].body, seen[1].body);
    assert!(seen.iter().all(|request| !request.payment_signature));
}

#[tokio::test]
async fn redirects_and_wrong_deployments_cannot_forward_auth_or_payment_headers() {
    let (target, target_session) = mocked(vec![], Duration::from_secs(2)).await;
    let mut redirect = Step::challenge(Some(PRIVATE));
    redirect.status = 307;
    redirect.headers.insert(
        "location",
        HeaderValue::from_str(&format!(
            "{}computers",
            target.client.deployment().api_base()
        ))
        .unwrap(),
    );
    let (server, session) = mocked(vec![redirect], Duration::from_secs(2)).await;
    let computers = server.client.computers(&session).unwrap();
    let request = computers.create_request(CreateOptions::default()).unwrap();
    let error = computers
        .create(&request, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(error, computers::Error::Uncertain { cause, .. } if matches!(*cause,computers::Error::Redirect))
    );
    assert_eq!(server.script.requests.lock().unwrap().len(), 1);
    assert!(target.script.requests.lock().unwrap().is_empty());
    assert!(matches!(
        target.client.computers(&session),
        Err(computers::Error::Session(Error::WrongDeployment))
    ));
    assert!(matches!(
        target
            .client
            .computers(&target_session)
            .unwrap()
            .create(&request, &CancellationToken::new())
            .await,
        Err(computers::Error::Session(Error::WrongDeployment))
    ));
    assert!(target.script.requests.lock().unwrap().is_empty());
}
