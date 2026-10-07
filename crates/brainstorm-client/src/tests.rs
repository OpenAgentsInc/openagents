use super::*;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[derive(Clone)]
struct Reply {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
    delay: Duration,
    body_delay: Duration,
    declared_length: Option<usize>,
    chunked: bool,
}

impl Reply {
    fn json(value: serde_json::Value) -> Self {
        Self::raw(200, &value.to_string())
    }
    fn raw(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.into(),
            headers: Vec::new(),
            delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            declared_length: None,
            chunked: false,
        }
    }
    fn delayed(mut self, millis: u64) -> Self {
        self.delay = Duration::from_millis(millis);
        self
    }
    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Clone, Debug)]
struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

#[derive(Default)]
struct State {
    replies: Mutex<HashMap<String, VecDeque<Reply>>>,
    requests: Mutex<Vec<Request>>,
    active: AtomicUsize,
    maximum: AtomicUsize,
}

struct Fake {
    origin: String,
    state: Arc<State>,
    task: JoinHandle<()>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Fake {
    async fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(State::default());
        let shared = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let state = shared.clone();
                tokio::spawn(async move {
                    serve(stream, state).await;
                });
            }
        });
        let fake = Self {
            origin,
            state,
            task,
        };
        fake.reply(DISCOVERY_PATH, capabilities());
        fake.reply(HOUSE_PATH, house(A));
        fake
    }
    fn reply(&self, path: &str, reply: Reply) {
        self.state
            .replies
            .lock()
            .unwrap()
            .entry(path.into())
            .or_default()
            .push_back(reply);
    }
    fn replace(&self, path: &str, replies: Vec<Reply>) {
        self.state
            .replies
            .lock()
            .unwrap()
            .insert(path.into(), replies.into());
    }
    fn requests(&self) -> Vec<Request> {
        self.state.requests.lock().unwrap().clone()
    }
    async fn seen(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while self.requests().len() < count {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
    }
    fn client(&self, limits: Limits) -> Client {
        let client = Client::fixture(Config {
            origin: self.origin.clone(),
            limits,
        })
        .unwrap();
        client.set_enabled(true);
        client
    }
}

async fn serve(mut stream: TcpStream, state: Arc<State>) {
    let mut bytes = Vec::new();
    let end = loop {
        let mut block = [0; 2048];
        let Ok(size) = stream.read(&mut block).await else {
            return;
        };
        if size == 0 || bytes.len() + size > 16 * 1024 {
            return;
        }
        bytes.extend_from_slice(&block[..size]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
    let first: Vec<_> = headers.lines().next().unwrap().split_whitespace().collect();
    let method = first[0].to_string();
    let path = first[1].to_string();
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or(0);
    while bytes.len() < end + length {
        let mut block = [0; 2048];
        let Ok(size) = stream.read(&mut block).await else {
            return;
        };
        if size == 0 {
            return;
        }
        bytes.extend_from_slice(&block[..size]);
    }
    state.requests.lock().unwrap().push(Request {
        method,
        path: path.clone(),
        body: bytes[end..end + length].to_vec(),
    });
    let reply = {
        let mut replies = state.replies.lock().unwrap();
        let values = replies.entry(path).or_default();
        if values.len() > 1 {
            values.pop_front().unwrap()
        } else {
            values
                .front()
                .cloned()
                .unwrap_or_else(|| Reply::raw(404, "{}"))
        }
    };
    let active = state.active.fetch_add(1, Ordering::SeqCst) + 1;
    state.maximum.fetch_max(active, Ordering::SeqCst);
    tokio::time::sleep(reply.delay).await;
    let length_header = if reply.chunked {
        "Transfer-Encoding: chunked\r\n".into()
    } else {
        format!(
            "Content-Length: {}\r\n",
            reply.declared_length.unwrap_or(reply.body.len())
        )
    };
    let mut headers = format!(
        "HTTP/1.1 {} fixture\r\nConnection: close\r\nContent-Type: application/json\r\n{length_header}",
        reply.status
    );
    for (name, value) in reply.headers {
        headers.push_str(&format!("{name}: {value}\r\n"));
    }
    headers.push_str("\r\n");
    if stream.write_all(headers.as_bytes()).await.is_ok() {
        tokio::time::sleep(reply.body_delay).await;
        if reply.chunked {
            for chunk in reply.body.as_bytes().chunks(256) {
                if stream
                    .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                    .await
                    .is_err()
                {
                    break;
                }
                if stream.write_all(chunk).await.is_err() {
                    break;
                }
                if stream.write_all(b"\r\n").await.is_err() {
                    break;
                }
            }
            let _ = stream.write_all(b"0\r\n\r\n").await;
        } else {
            let _ = stream.write_all(reply.body.as_bytes()).await;
        }
    }
    state.active.fetch_sub(1, Ordering::SeqCst);
}

fn capabilities() -> Reply {
    Reply::json(serde_json::json!({
        SEARCH_PATH: [{ "id": "relevance", "name": "relevance" }],
        RANK_PATH: [{ "id": "graperank", "name": "graperank" }]
    }))
}
fn house(key: &str) -> Reply {
    Reply::json(serde_json::json!({ "names": { "_": key } }))
}
fn data(values: &[(&str, f64)], ttl: u64) -> Reply {
    Reply::json(
        serde_json::json!({ "results": values.iter().map(|(key, rank)| serde_json::json!({ "pubkey": key, "rank": rank })).collect::<Vec<_>>(), "ttl": ttl }),
    )
}
fn keys() -> Vec<String> {
    vec![A.into(), B.into()]
}

#[tokio::test]
async fn search_joins_by_key_and_preserves_exact_response_evidence() {
    let fake = Fake::new().await;
    let search = data(&[(A, 0.8), (B, 0.4)], 70);
    let rank = data(&[(B, 7.5), (A, 0.0)], 40);
    fake.reply(SEARCH_PATH, search.clone());
    fake.reply(RANK_PATH, rank.clone());
    let client = fake.client(Limits::default());
    let result = client
        .search("public query", 2, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        result
            .subjects
            .iter()
            .map(|row| row.pubkey.as_str())
            .collect::<Vec<_>>(),
        vec![A, B]
    );
    assert_eq!(result.subjects[0].relevance, Some(0.8));
    assert_eq!(
        result.subjects[0].influence,
        Some(Influence {
            value: 0.0,
            coverage: Coverage::Unknown
        })
    );
    assert_eq!(result.subjects[1].influence.as_ref().unwrap().value, 7.5);
    assert_eq!(
        result.subjects[0].profile_url,
        format!("https://njump.me/{A}")
    );
    assert_eq!(result.completeness, Completeness::Bounded);
    assert_eq!(result.house.pubkey, A);
    assert_eq!(
        result.house.attribution,
        Attribution::SeparateHttpsObservation
    );
    assert_eq!(result.responses.len(), 4);
    assert_eq!(
        result.responses[2].requested_algorithm,
        Some(Algorithm::Relevance)
    );
    assert_eq!(
        result.responses[3].requested_algorithm,
        Some(Algorithm::Graperank)
    );
    assert_eq!(
        result.responses[2].output_digest,
        digest(search.body.as_bytes())
    );
    assert_eq!(
        result.responses[3].output_digest,
        digest(rank.body.as_bytes())
    );
    assert_eq!(
        result.responses[3].expires_at_ms - result.responses[3].fetched_at_ms,
        40_000
    );
    assert_eq!(
        result.expires_at_ms,
        result
            .responses
            .iter()
            .map(|response| response.expires_at_ms)
            .min()
            .unwrap()
    );
    let requests = fake.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].method, "GET");
    assert!(requests[0].body.is_empty());
    assert_eq!(requests[1].path, HOUSE_PATH);
    assert_eq!(requests[2].method, "POST");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[2].body).unwrap(),
        serde_json::json!({ "query": "public query", "algorithm": "relevance", "limit": 2 })
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[3].body).unwrap(),
        serde_json::json!({ "pubkeys": keys(), "algorithm": "graperank" })
    );
    assert_eq!(result.input_digest, digest(&requests[2].body));
    assert_eq!(result.responses[3].input_digest, digest(&requests[3].body));
}

#[tokio::test]
async fn disabled_construction_and_enablement_perform_no_reads() {
    let fake = Fake::new().await;
    let client = Client::fixture(Config {
        origin: fake.origin.clone(),
        ..Config::default()
    })
    .unwrap();
    assert_eq!(
        client.discover(&Cancellation::default()).await,
        Err(Error::Disabled)
    );
    client.set_enabled(true);
    client.set_enabled(false);
    client.set_enabled(true);
    assert!(fake.requests().is_empty());
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(client.discover(&cancelled).await, Err(Error::Cancelled));
    assert!(fake.requests().is_empty());
}

#[test]
fn production_configuration_rejects_http_credentials_prefixes_and_widened_bounds() {
    for origin in [
        "http://127.0.0.1:1234",
        "https://user@example.com",
        "https://example.com/api",
        "https://example.com/?x=1",
        "https://example.com/#fragment",
        "file:///public",
    ] {
        assert!(
            matches!(
                Client::new(Config {
                    origin: origin.into(),
                    ..Config::default()
                }),
                Err(Error::InvalidConfiguration { .. })
            ),
            "{origin}"
        );
    }
    let mut limits = Limits::default();
    limits.concurrent_operations = 3;
    assert!(limits.validate().is_err());
    limits.concurrent_operations = 0;
    assert!(limits.validate().is_err());
    let client = Client::new(Config {
        origin: "https://EXAMPLE.com:443/".into(),
        ..Config::default()
    })
    .unwrap();
    assert_eq!(client.configuration().origin, "https://example.com");
    assert!(!client.is_enabled());
}

#[tokio::test]
async fn invalid_inputs_refuse_before_discovery() {
    let fake = Fake::new().await;
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    for query in ["".into(), " ".into(), "x".repeat(513), "界".repeat(342)] {
        assert!(matches!(
            client.search(&query, 1, &cancel).await,
            Err(Error::InvalidInput { .. })
        ));
    }
    for limit in [0, 11] {
        assert!(client.search("x", limit, &cancel).await.is_err());
    }
    for list in [
        vec![],
        vec!["npub1notvalid".into()],
        vec![A.into(); 21],
        vec![A.into(), A.to_uppercase()],
    ] {
        assert!(matches!(
            client.rank(&list, &cancel).await,
            Err(Error::InvalidInput { .. })
        ));
    }
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn discovery_refetches_house_identity_and_requires_global_algorithms() {
    let fake = Fake::new().await;
    fake.replace(HOUSE_PATH, vec![house(A), house(B)]);
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    assert_eq!(client.discover(&cancel).await.unwrap().house.pubkey, A);
    assert_eq!(client.discover(&cancel).await.unwrap().house.pubkey, B);
    fake.replace(
        DISCOVERY_PATH,
        vec![Reply::json(
            serde_json::json!({ SEARCH_PATH: [{ "id": "relevance", "pov": true }] }),
        )],
    );
    let discovery = client.discover(&cancel).await.unwrap();
    assert!(!discovery.search_supported);
    assert!(!discovery.rank_supported);
    assert_eq!(
        client.search("public", 1, &cancel).await.unwrap_err(),
        Error::UnsupportedOperation {
            endpoint: SEARCH_PATH.into()
        }
    );
    assert!(
        !fake
            .requests()
            .iter()
            .any(|request| request.path == SEARCH_PATH)
    );
}

#[tokio::test]
async fn unavailable_and_malformed_discovery_are_typed() {
    let fake = Fake::new().await;
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    fake.replace(
        DISCOVERY_PATH,
        vec![Reply::raw(503, "{\"private\":\"service details\"}")],
    );
    assert_eq!(
        client.discover(&cancel).await.unwrap_err(),
        Error::DiscoveryUnavailable {
            component: "capabilities".into(),
            cause: Box::new(Error::Service { status: 503 })
        }
    );
    fake.replace(DISCOVERY_PATH, vec![Reply::raw(200, "[]")]);
    assert!(matches!(
        client.discover(&cancel).await,
        Err(Error::DiscoveryUnavailable { .. })
    ));
    fake.replace(DISCOVERY_PATH, vec![capabilities()]);
    fake.replace(HOUSE_PATH, vec![house("bad")]);
    assert!(
        matches!(client.discover(&cancel).await, Err(Error::DiscoveryUnavailable { component, .. }) if component == "house_identity")
    );
}

#[tokio::test]
async fn rank_preserves_request_order_and_missing_scores_without_zeroes() {
    let fake = Fake::new().await;
    fake.reply(RANK_PATH, data(&[(B, 0.5)], 20));
    let client = fake.client(Limits::default());
    let result = client
        .rank(&[A.to_uppercase(), B.into()], &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(result.subjects[0].pubkey, A);
    assert_eq!(result.subjects[0].influence, None);
    assert_eq!(result.subjects[1].influence.as_ref().unwrap().value, 0.5);
    assert_eq!(result.completeness, Completeness::Partial);
}

#[tokio::test]
async fn failed_enrichment_keeps_relevance_and_failure_provenance() {
    let fake = Fake::new().await;
    fake.reply(SEARCH_PATH, data(&[(A, 0.3)], 20));
    fake.reply(RANK_PATH, Reply::raw(503, "{}"));
    let client = fake.client(Limits::default());
    let result = client
        .search("public", 1, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(result.subjects[0].relevance, Some(0.3));
    assert_eq!(result.subjects[0].influence, None);
    assert_eq!(
        result.enrichment_error,
        Some(Error::Service { status: 503 })
    );
    assert_eq!(result.responses.len(), 4);
    assert_eq!(result.responses[3].endpoint, RANK_PATH);
    assert_eq!(result.responses[3].status, 503);
    assert_eq!(result.responses[3].output_digest, digest(b"{}"));
    assert!(!result.is_fresh_at(now_ms()));
    assert_eq!(result.completeness, Completeness::Partial);
}

#[tokio::test]
async fn exact_query_boundaries_and_empty_search_skip_rank_enrichment() {
    let fake = Fake::new().await;
    fake.reply(SEARCH_PATH, data(&[], 20));
    let client = fake.client(Limits::default());
    for query in ["x".repeat(512), "é".repeat(512)] {
        let result = client
            .search(&query, 10, &Cancellation::default())
            .await
            .unwrap();
        assert!(result.subjects.is_empty());
        assert_eq!(result.completeness, Completeness::Bounded);
    }
    assert!(
        !fake
            .requests()
            .iter()
            .any(|request| request.path == RANK_PATH)
    );
}

#[tokio::test]
async fn search_without_advertised_rank_keeps_unavailable_influence() {
    let fake = Fake::new().await;
    fake.replace(
        DISCOVERY_PATH,
        vec![Reply::json(serde_json::json!({
            SEARCH_PATH: [{ "id": "relevance" }]
        }))],
    );
    fake.reply(SEARCH_PATH, data(&[(A, 0.5)], 20));
    let result = fake
        .client(Limits::default())
        .search("public", 1, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        result.enrichment_error,
        Some(Error::UnsupportedOperation {
            endpoint: RANK_PATH.into()
        })
    );
    assert_eq!(result.subjects[0].influence, None);
    assert_eq!(result.responses.len(), 3);
    assert!(
        !fake
            .requests()
            .iter()
            .any(|request| request.path == RANK_PATH)
    );
}

#[tokio::test]
async fn connection_failure_is_typed_and_does_not_retain_request_text() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let client = Client::fixture(Config {
        origin,
        ..Config::default()
    })
    .unwrap();
    client.set_enabled(true);
    let error = client
        .search("a public query", 1, &Cancellation::default())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        Error::DiscoveryUnavailable {
            component: "capabilities".into(),
            cause: Box::new(Error::Transport)
        }
    );
    assert!(!format!("{error:?}").contains("a public query"));
}

#[tokio::test]
async fn malformed_nonfinite_duplicate_unrequested_and_excessive_rows_refuse() {
    let fake = Fake::new().await;
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    for reply in [
        Reply::raw(200, "bad json"),
        Reply::raw(
            200,
            &format!("{{\"results\":[{{\"pubkey\":\"{A}\",\"rank\":1e400}}]}}"),
        ),
        data(&[(A, 1.0), (A, 2.0)], 10),
        data(&[(C, 1.0)], 10),
        data(&[("bad", 1.0)], 10),
        Reply::raw(
            200,
            &format!("{{\"results\":[{{\"pubkey\":\"{A}\",\"rank\":0.5}}],\"ttl\":-1}}"),
        ),
    ] {
        fake.replace(RANK_PATH, vec![reply]);
        assert!(matches!(
            client.rank(&keys(), &cancel).await,
            Err(Error::InvalidResponse { .. })
        ));
    }
    fake.replace(SEARCH_PATH, vec![data(&[(A, 1.0), (B, 2.0)], 10)]);
    assert!(
        matches!(client.search("public", 1, &cancel).await, Err(Error::InvalidResponse { field }) if field == "result_count")
    );
    assert!(!fake.requests().iter().any(|request| {
        request.path == RANK_PATH
            && serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["pubkeys"]
                .as_array()
                .unwrap()
                .len()
                == 1
    }));
}

#[tokio::test]
async fn typed_statuses_do_not_trigger_authentication_or_automatic_retries() {
    for (status, body, expected) in [
        (401, "{}", Error::AuthenticationRequired { status: 401 }),
        (403, "{}", Error::AuthenticationRequired { status: 403 }),
        (
            429,
            "{}",
            Error::RateLimited {
                retry_after_seconds: None,
            },
        ),
        (
            202,
            "{\"status\":\"computing\",\"retry_after\":60}",
            Error::Computing {
                retry_after_seconds: Some(60),
            },
        ),
        (422, "{}", Error::PerspectiveUnavailable),
        (502, "{}", Error::Service { status: 502 }),
    ] {
        let fake = Fake::new().await;
        fake.reply(RANK_PATH, Reply::raw(status, body));
        assert_eq!(
            fake.client(Limits::default())
                .rank(&keys(), &Cancellation::default())
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(
            fake.requests()
                .iter()
                .filter(|request| request.path == RANK_PATH)
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn retry_after_blocks_explicit_retry_without_a_network_loop() {
    let fake = Fake::new().await;
    fake.reply(RANK_PATH, Reply::raw(429, "{}").header("Retry-After", "60"));
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    let expected = Error::RateLimited {
        retry_after_seconds: Some(60),
    };
    assert_eq!(client.rank(&keys(), &cancel).await.unwrap_err(), expected);
    assert_eq!(client.rank(&keys(), &cancel).await.unwrap_err(), expected);
    assert_eq!(
        fake.requests()
            .iter()
            .filter(|request| request.path == RANK_PATH)
            .count(),
        1
    );
    let date_fake = Fake::new().await;
    let future = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(60));
    date_fake.reply(
        RANK_PATH,
        Reply::raw(429, "{}").header("Retry-After", &future),
    );
    assert!(
        matches!(date_fake.client(Limits::default()).rank(&keys(), &cancel).await, Err(Error::RateLimited { retry_after_seconds: Some(seconds) }) if (59..=60).contains(&seconds))
    );
}

#[tokio::test]
async fn redirect_is_refused_without_contacting_the_other_origin() {
    let fake = Fake::new().await;
    let other = Fake::new().await;
    fake.reply(
        RANK_PATH,
        Reply::raw(302, "{}").header("Location", &format!("{}{RANK_PATH}", other.origin)),
    );
    assert_eq!(
        fake.client(Limits::default())
            .rank(&keys(), &Cancellation::default())
            .await
            .unwrap_err(),
        Error::RedirectRefused
    );
    assert!(other.requests().is_empty());
}

#[tokio::test]
async fn response_bounds_apply_to_declared_chunked_and_error_bodies() {
    for (chunked, status) in [(false, 200), (true, 200), (true, 503)] {
        let fake = Fake::new().await;
        let mut reply = Reply::raw(status, &"x".repeat(4096));
        reply.chunked = chunked;
        fake.reply(RANK_PATH, reply);
        let limits = Limits {
            response_bytes: 1024,
            ..Limits::default()
        };
        assert_eq!(
            fake.client(limits)
                .rank(&keys(), &Cancellation::default())
                .await
                .unwrap_err(),
            Error::ResponseTooLarge
        );
    }
    let fake = Fake::new().await;
    let mut reply = data(&[(A, 1.0)], 1);
    reply.declared_length = Some(usize::MAX / 2);
    fake.reply(RANK_PATH, reply);
    assert_eq!(
        fake.client(Limits::default())
            .rank(&keys(), &Cancellation::default())
            .await
            .unwrap_err(),
        Error::ResponseTooLarge
    );
}

#[tokio::test]
async fn cancellation_and_disable_reenable_drop_pending_reads_and_prevent_later_dispatch() {
    for disable in [false, true] {
        let fake = Fake::new().await;
        fake.replace(DISCOVERY_PATH, vec![capabilities().delayed(100)]);
        let client = fake.client(Limits::default());
        let active = client.clone();
        let cancellation = Cancellation::default();
        let cancel = cancellation.clone();
        let task = tokio::spawn(async move { active.search("public", 1, &cancel).await });
        fake.seen(1).await;
        if disable {
            client.set_enabled(false);
            client.set_enabled(true);
        } else {
            cancellation.cancel();
        }
        assert_eq!(
            task.await.unwrap().unwrap_err(),
            if disable {
                Error::Disabled
            } else {
                Error::Cancelled
            }
        );
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(fake.requests().len(), 1);
    }
    let fake = Fake::new().await;
    let mut reply = capabilities();
    reply.body_delay = Duration::from_millis(100);
    fake.replace(DISCOVERY_PATH, vec![reply]);
    let client = fake.client(Limits::default());
    let cancel = Cancellation::default();
    let active_cancel = cancel.clone();
    let task = tokio::spawn(async move { client.discover(&active_cancel).await });
    fake.seen(1).await;
    cancel.cancel();
    assert_eq!(task.await.unwrap(), Err(Error::Cancelled));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn deadline_covers_discovery_search_and_enrichment_as_one_operation() {
    let fake = Fake::new().await;
    fake.replace(DISCOVERY_PATH, vec![capabilities().delayed(30)]);
    fake.replace(HOUSE_PATH, vec![house(A).delayed(30)]);
    fake.reply(SEARCH_PATH, data(&[(A, 0.4)], 5).delayed(30));
    fake.reply(RANK_PATH, data(&[(A, 0.5)], 5).delayed(30));
    let client = fake.client(Limits {
        deadline_ms: 105,
        ..Limits::default()
    });
    assert_eq!(
        client
            .search("public", 1, &Cancellation::default())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    assert!(fake.requests().len() <= 4);
}

#[tokio::test]
async fn concurrency_and_queued_cancellation_are_bounded() {
    let fake = Fake::new().await;
    fake.replace(DISCOVERY_PATH, vec![capabilities().delayed(40)]);
    fake.replace(HOUSE_PATH, vec![house(A).delayed(40)]);
    let client = fake.client(Limits::default());
    let first = client.clone();
    let second = client.clone();
    let a = tokio::spawn(async move { first.discover(&Cancellation::default()).await });
    let b = tokio::spawn(async move { second.discover(&Cancellation::default()).await });
    fake.seen(2).await;
    let third = client.clone();
    let cancel = Cancellation::default();
    let queued_cancel = cancel.clone();
    let c = tokio::spawn(async move { third.discover(&queued_cancel).await });
    tokio::time::sleep(Duration::from_millis(5)).await;
    cancel.cancel();
    assert_eq!(c.await.unwrap(), Err(Error::Cancelled));
    a.await.unwrap().unwrap();
    b.await.unwrap().unwrap();
    assert_eq!(fake.requests().len(), 4);
    assert!(fake.state.maximum.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn queue_time_counts_toward_deadline_and_disable_cancels_all_waiters() {
    for disable in [false, true] {
        let fake = Fake::new().await;
        fake.replace(DISCOVERY_PATH, vec![capabilities().delayed(200)]);
        let client = fake.client(Limits {
            deadline_ms: 60,
            ..Limits::default()
        });
        let mut tasks = Vec::new();
        for _ in 0..3 {
            let active = client.clone();
            tasks.push(tokio::spawn(async move {
                active.discover(&Cancellation::default()).await
            }));
        }
        fake.seen(2).await;
        if disable {
            client.set_enabled(false);
        }
        for task in tasks {
            assert_eq!(
                task.await.unwrap(),
                Err(if disable {
                    Error::Disabled
                } else {
                    Error::Timeout
                })
            );
        }
        assert_eq!(fake.requests().len(), 2);
    }
}

#[tokio::test]
async fn ttl_obeys_each_service_component_headers_and_local_maximum() {
    let fake = Fake::new().await;
    fake.replace(
        HOUSE_PATH,
        vec![house(A).header("Cache-Control", "max-age=2")],
    );
    fake.reply(SEARCH_PATH, data(&[(A, 1.0)], 20));
    fake.reply(RANK_PATH, data(&[(A, 0.5)], 1000));
    let client = fake.client(Limits {
        cache_ttl_seconds: 10,
        ..Limits::default()
    });
    let result = client
        .search("public", 1, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        result.responses[3].expires_at_ms - result.responses[3].fetched_at_ms,
        10_000
    );
    assert_eq!(result.expires_at_ms, result.responses[1].expires_at_ms);
    fake.replace(
        RANK_PATH,
        vec![data(&[(A, 0.5)], 100).header("Cache-Control", "no-store")],
    );
    let result = client
        .rank(&[A.into()], &Cancellation::default())
        .await
        .unwrap();
    assert!(!result.is_fresh_at(now_ms()));
    fake.replace(
        RANK_PATH,
        vec![Reply::json(
            serde_json::json!({ "results": [{ "pubkey": A, "rank": 0.5 }] }),
        )],
    );
    let result = client
        .rank(&[A.into()], &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        result.responses[2].expires_at_ms,
        result.responses[2].fetched_at_ms
    );
}

#[tokio::test]
async fn normalized_output_context_and_task_cache_remain_bounded() {
    let fake = Fake::new().await;
    fake.reply(RANK_PATH, data(&[(A, 1.0), (B, 0.0)], 200));
    let client = fake.client(Limits::default());
    let mut observation = client
        .rank(&keys(), &Cancellation::default())
        .await
        .unwrap();
    let json = bounded_json(&observation, 64 * 1024).unwrap();
    let too_small = fake.client(Limits {
        normalized_bytes: json.len() - 200,
        ..Limits::default()
    });
    assert_eq!(
        too_small
            .rank(&keys(), &Cancellation::default())
            .await
            .unwrap_err(),
        Error::OutputTooLarge
    );
    let full_context = observation.model_context().unwrap();
    observation.configuration.limits.context_bytes = full_context.len() - 100;
    let context = observation.model_context().unwrap();
    assert!(context.len() <= observation.configuration.limits.context_bytes);
    let context: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(context["context_truncated"], true);
    assert_eq!(
        context["observation"]["responses"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(context["observation"]["house"]["pubkey"], A);
    let clock = now_ms();
    let mut cache = TaskCache::new(Limits {
        cache_entries: 1,
        cache_ttl_seconds: 2,
        ..Limits::default()
    })
    .unwrap();
    let first = observation.cache_key();
    assert!(cache.insert(observation.clone(), clock).unwrap());
    assert!(cache.get(&first, clock + 1999).is_some());
    assert!(cache.get(&first, clock + 2000).is_none());
    observation.expires_at_ms = clock;
    assert!(!cache.insert(observation.clone(), clock).unwrap());
    observation.expires_at_ms = clock + 100_000;
    assert!(cache.insert(observation.clone(), clock).unwrap());
    observation.input_digest = digest(b"other public input");
    let second = observation.cache_key();
    assert!(cache.insert(observation.clone(), clock).unwrap());
    assert_eq!(cache.len(), 1);
    assert!(cache.get(&first, clock).is_none());
    assert!(cache.get(&second, clock).is_some());
    assert!(cache.bytes() <= Limits::default().cache_bytes);
    cache.clear();
    assert!(cache.is_empty());
    assert_eq!(cache.bytes(), 0);
    let mut byte_cache = TaskCache::new(Limits {
        cache_bytes: bounded_json(&observation, 64 * 1024).unwrap().len() + 100,
        ..Limits::default()
    })
    .unwrap();
    assert!(byte_cache.insert(observation.clone(), clock).unwrap());
    let replaced = observation.cache_key();
    observation.input_digest = digest(b"third public input");
    let retained = observation.cache_key();
    assert!(byte_cache.insert(observation.clone(), clock).unwrap());
    assert_eq!(byte_cache.len(), 1);
    assert!(byte_cache.get(&replaced, clock).is_none());
    assert!(byte_cache.get(&retained, clock).is_some());
    let tiny = Limits {
        cache_bytes: 10,
        ..Limits::default()
    };
    assert_eq!(
        TaskCache::new(tiny).unwrap().insert(observation, clock),
        Err(Error::OutputTooLarge)
    );
}
