#![allow(dead_code)]
use std::{collections::BTreeMap, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// One canned HTTP response.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn new(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Self {
        Self {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.to_vec(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            self.body.len()
        );
        for (name, value) in &self.headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        let mut response = response.into_bytes();
        response.extend_from_slice(&self.body);
        response
    }
}

async fn handle(mut socket: TcpStream, reply: &Reply) -> Request {
    let mut bytes = Vec::new();
    let split = loop {
        let mut chunk = [0; 4096];
        let len = socket.read(&mut chunk).await.expect("read");
        assert!(len > 0);
        bytes.extend_from_slice(&chunk[..len]);
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
    };
    let header = String::from_utf8(bytes[..split].to_vec()).expect("header");
    let mut lines = header.split("\r\n");
    let mut first = lines.next().expect("request line").split_whitespace();
    let method = first.next().expect("method").into();
    let target = first.next().expect("target").into();
    let headers: BTreeMap<String, String> = lines
        .map(|line| {
            let (key, value) = line.split_once(':').expect("header pair");
            (key.to_lowercase(), value.trim().into())
        })
        .collect();
    let length = headers
        .get("content-length")
        .map(|s| s.parse::<usize>().expect("length"))
        .unwrap_or(0);
    while bytes.len() < split + 4 + length {
        let mut chunk = [0; 4096];
        let len = socket.read(&mut chunk).await.expect("body");
        assert!(len > 0);
        bytes.extend_from_slice(&chunk[..len]);
    }
    socket.write_all(&reply.bytes()).await.expect("respond");
    Request {
        method,
        target,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let base = format!("http://{}/api/v1", listener.local_addr().expect("address"));
    (listener, base)
}

pub fn fast_retries() -> boat::RetryPolicy {
    boat::RetryPolicy {
        max_retries: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(1),
    }
}

pub fn builder(base: String) -> boat::ClientBuilder {
    boat::Client::builder(boat::ApiKey::new("test-secret").expect("key"))
        .base_url(base)
        .retry(fast_retries())
}

/// Answer exactly one request.
pub async fn serve(
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
) -> (boat::Client, tokio::task::JoinHandle<Request>) {
    let (listener, base) = listen().await;
    let reply = Reply::new(status, headers, body);
    let job = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        handle(socket, &reply).await
    });
    (builder(base).build().expect("client"), job)
}

/// Answer requests in order, one reply each. The job yields the requests it
/// saw once the client is dropped or the listener stops being used; await it
/// with a timeout via [`collect`].
pub async fn serve_sequence(
    replies: Vec<Reply>,
    configure: impl FnOnce(boat::ClientBuilder) -> boat::ClientBuilder,
) -> (boat::Client, tokio::task::JoinHandle<Vec<Request>>) {
    let (listener, base) = listen().await;
    let job = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in &replies {
            match tokio::time::timeout(Duration::from_millis(500), listener.accept()).await {
                Ok(Ok((socket, _))) => seen.push(handle(socket, reply).await),
                _ => break,
            }
        }
        seen
    });
    (configure(builder(base)).build().expect("client"), job)
}

/// A checked-in fixture file under `fixtures/`.
pub fn read_fixture(relative: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{relative}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

/// The spec fixture for an operation.
pub fn fixture(operation_id: &str) -> serde_json::Value {
    read_fixture(&format!("spec/{operation_id}.json"))
}

/// The operation's parameters from its fixture's `sdk_params_json`.
pub fn params<T: serde::de::DeserializeOwned>(fixture: &serde_json::Value) -> T {
    serde_json::from_value(fixture["request"]["sdk_params_json"].clone())
        .expect("fixture parameters")
}

/// Every redacted capture in `fixtures/observed/`, by file stem.
pub fn observed() -> Vec<(String, serde_json::Value)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/observed");
    let mut all: Vec<_> = std::fs::read_dir(dir)
        .expect("observed fixtures")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("name")
                .to_owned();
            (stem.clone(), read_fixture(&format!("observed/{stem}.json")))
        })
        .collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

/// One response body a fixture expects the SDK to decode.
pub struct Case {
    pub label: String,
    pub status: u16,
    pub body: Vec<u8>,
    /// A recorded vendor deviation: the SDK must still refuse this body, so
    /// the test notices when the schema or the server changes.
    pub deviation: bool,
}

/// The success bodies (schema sample, `oneOf` variants, published
/// examples), the failure bodies, and the observed captures for a fixture.
pub fn bodies(fixture: &serde_json::Value) -> Vec<Case> {
    let mut cases = Vec::new();
    let success = &fixture["success"];
    let status: u16 = success["status"]
        .as_str()
        .expect("status")
        .parse()
        .expect("number");
    let json = |label: String, status: u16, value: &serde_json::Value| Case {
        label,
        status,
        body: serde_json::to_vec(value).expect("json"),
        deviation: false,
    };
    if success["binary"].as_bool().unwrap_or(false) {
        cases.push(Case {
            label: "binary".into(),
            status,
            body: b"\x00\x01binary".to_vec(),
            deviation: false,
        });
    }
    if !success["schema_sample"].is_null() {
        cases.push(json(
            "schema sample".into(),
            status,
            &success["schema_sample"],
        ));
    }
    for (i, variant) in success["variant_samples"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        cases.push(json(format!("variant {}", i + 2), status, variant));
    }
    for (name, example) in success["published_examples"]
        .as_object()
        .into_iter()
        .flatten()
    {
        cases.push(json(format!("published {name}"), status, example));
    }
    for failure in fixture["failure_cases"].as_array().into_iter().flatten() {
        let status: u16 = failure["status"]
            .as_str()
            .expect("status")
            .parse()
            .expect("number");
        cases.push(json(format!("failure {status}"), status, &failure["body"]));
    }
    for (stem, capture) in observed() {
        if capture["operation_id"] != fixture["operation_id"] {
            continue;
        }
        let status = u16::try_from(capture["status"].as_u64().expect("status")).expect("status");
        let binary = capture["body"]["note"].is_string() && status < 300;
        cases.push(Case {
            label: format!("observed {stem}"),
            status,
            body: if binary {
                b"\x00\x01binary".to_vec()
            } else {
                serde_json::to_vec(&capture["body"]).expect("json")
            },
            deviation: capture["known_deviation"].is_string(),
        });
    }
    assert!(
        !cases.is_empty(),
        "{} has no bodies",
        fixture["operation_id"]
    );
    cases
}

/// Answer one request with a fixture body. Retries are off so a 429 or 5xx
/// comes back as the API error itself.
pub async fn serve_fixture(
    status: u16,
    body: &[u8],
) -> (boat::Client, tokio::task::JoinHandle<Request>) {
    let (listener, base) = listen().await;
    let reply = Reply::new(status, &[("content-type", "application/json")], body);
    let job = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        handle(socket, &reply).await
    });
    let client = builder(base)
        .retry(boat::RetryPolicy::none())
        .build()
        .expect("client");
    (client, job)
}

/// The request matches the fixture, a 2xx body decodes, and a failure comes
/// back as an API error carrying the parsed envelope.
pub fn check(fixture: &serde_json::Value, case: &Case, result: boat::Result<()>, request: Request) {
    let id = fixture["operation_id"].as_str().expect("id");
    let label = &case.label;
    assert_eq!(
        request.method,
        fixture["method"].as_str().expect("method"),
        "{id} {label}"
    );
    assert_eq!(
        request.target,
        fixture["request"]["target"].as_str().expect("target"),
        "{id} {label}"
    );
    let expected = if case.status < 300 {
        result.is_ok()
    } else {
        matches!(&result, Err(boat::Error::Api(e))
            if e.status.as_u16() == case.status && e.envelope.is_some())
    };
    if case.deviation {
        assert!(
            !expected,
            "{id} {label} now decodes; drop its known_deviation"
        );
    } else {
        assert!(expected, "{id} {label}: {result:?}");
    }
}
