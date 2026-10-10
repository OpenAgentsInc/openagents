//! Vertex AI embeddings against a local fake `predict` endpoint.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;
use crate::search::{Embed, Embedded, Embedder, Retriever, rank};
use crate::{Base, Entry};

/// One request the fake endpoint received.
#[derive(Clone, Debug)]
struct Seen {
    line: String,
    authorization: String,
    body: Value,
}

type Reply = fn(usize, &Value) -> (u16, String);

/// A fake endpoint that records each request and answers with `reply`,
/// given the request's index and body.
async fn fake(reply: Reply) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/v1/projects/p/locations/l/publishers/google/models",
        listener.local_addr().unwrap()
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut data = Vec::new();
            let mut buffer = vec![0u8; 65_536];
            let (head, body) = loop {
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                if n == 0 {
                    break (String::new(), Vec::new());
                }
                data.extend_from_slice(&buffer[..n]);
                let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&data[..end]).to_string();
                let length = head
                    .lines()
                    .find_map(|l| {
                        let (name, value) = l.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                while data.len() < end + 4 + length {
                    let n = socket.read(&mut buffer).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buffer[..n]);
                }
                break (head, data[end + 4..].to_vec());
            };
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let index = {
                let mut log = log.lock().unwrap();
                log.push(Seen {
                    line: head.lines().next().unwrap_or_default().to_string(),
                    authorization: head
                        .lines()
                        .find_map(|l| {
                            let (name, value) = l.split_once(':')?;
                            name.eq_ignore_ascii_case("authorization")
                                .then(|| value.trim().to_string())
                        })
                        .unwrap_or_default(),
                    body: body.clone(),
                });
                log.len() - 1
            };
            let (status, text) = reply(index, &body);
            let response = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (url, seen)
}

/// A four-dimension vector for each instance, and the billable characters
/// (non-whitespace) of all of them, as Vertex reports them.
fn predictions(body: &Value) -> String {
    let instances = body["instances"].as_array().cloned().unwrap_or_default();
    let mut characters = 0;
    let predictions: Vec<Value> = instances
        .iter()
        .map(|i| {
            let text = i["content"].as_str().unwrap_or_default();
            characters += text.chars().filter(|c| !c.is_whitespace()).count();
            let lower = text.to_lowercase();
            let values: Vec<f32> = ["kernel", "bins", "bash"]
                .iter()
                .map(|w| if lower.contains(w) { 1.0 } else { 0.1 })
                .chain([if i["task_type"] == "RETRIEVAL_QUERY" { 0.2 } else { 0.0 }])
                .collect();
            json!({"embeddings": {"values": values, "statistics": {"truncated": false, "token_count": 3}}})
        })
        .collect();
    json!({"predictions": predictions, "metadata": {"billableCharacterCount": characters}})
        .to_string()
}

fn ok(_: usize, body: &Value) -> (u16, String) {
    (200, predictions(body))
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("knowledge-vertex-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A client for `url` with a token file holding `test-token`.
fn client(url: &str, name: &str) -> Vertex {
    let token = scratch(name).join("token");
    std::fs::write(&token, "test-token\n").unwrap();
    let mut vertex = Vertex::new(url, Token::File(token));
    vertex.retries = 0;
    vertex
}

fn entry(id: &str, title: &str, summary: &str, tags: &str) -> Entry {
    Entry::parse(&format!(
        "---\nid: {id}\nversion: 1\nkind: method\ntitle: {title}\nsummary: >-\n  {summary}\ntags: [{tags}]\napplies_when: >-\n  Code does this.\nstatus: admitted\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites: [\"A Book, 2001\"]\nevidence: []\n---\n\n## Details\n\nThe body of {id}.\n"
    ))
    .unwrap()
}

fn base() -> Base {
    Base {
        entries: vec![
            entry(
                "stats.mmd",
                "MMD estimators",
                "Discrepancy estimators.",
                "kernel",
            ),
            entry(
                "stats.psi",
                "Population stability",
                "Binned distributions.",
                "bins",
            ),
            entry("shell.quote", "Quoting", "Quote every expansion.", "bash"),
        ],
    }
}

fn price(characters: usize) -> f64 {
    characters as f64 * USD_PER_THOUSAND_CHARACTERS / 1_000.0
}

#[tokio::test]
async fn a_request_has_the_predict_shape_and_a_list_price_cost() {
    let (url, seen) = fake(ok).await;
    let embedder = Embedder::with_vertex(client(&url, "shape"));
    assert_eq!(embedder.model(), "vertex/text-embedding-005");
    assert_eq!(embedder.provider.as_str(), "vertex");
    assert_eq!(embedder.basis(), "list_price");
    let (documents, query, usd) = embedder
        .embed_with_query(
            vec!["a kernel".to_string(), "some bins".to_string()],
            Some("which kernel".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0], [1.0, 0.1, 0.1, 0.0]);
    assert_eq!(query.unwrap(), [1.0, 0.1, 0.1, 0.2]);
    // "akernel" + "somebins" + "whichkernel": 26 billable characters.
    assert!((usd.unwrap() - price(26)).abs() < 1e-15);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].line.starts_with(
            "POST /v1/projects/p/locations/l/publishers/google/models/text-embedding-005:predict "
        ),
        "{}",
        seen[0].line
    );
    assert_eq!(seen[0].authorization, "Bearer test-token");
    assert_eq!(
        seen[0].body,
        json!({
            "instances": [
                {"content": "a kernel", "task_type": "RETRIEVAL_DOCUMENT"},
                {"content": "some bins", "task_type": "RETRIEVAL_DOCUMENT"},
                {"content": "which kernel", "task_type": "RETRIEVAL_QUERY"},
            ],
            "parameters": {"autoTruncate": true},
        })
    );
}

#[test]
fn inputs_are_batched_by_count_and_by_tokens() {
    let short: Vec<(String, Task)> = (0..600)
        .map(|i| (format!("entry {i}"), Task::Document))
        .collect();
    assert_eq!(batches(&short), [0..250, 250..500, 500..600]);
    // Each long input counts its whole length (Vertex's request limit is
    // before truncation): 6,001 tokens, so three fit under 20,000 and a
    // fourth doesn't.
    let long: Vec<(String, Task)> = (0..8)
        .map(|_| ("x".repeat(10_000), Task::Document))
        .collect();
    assert_eq!(batches(&long), [0..3, 3..6, 6..8]);
    assert!(batches::<Task>(&[]).is_empty());
}

#[tokio::test]
async fn a_long_list_goes_out_in_several_requests_in_order() {
    let (url, seen) = fake(ok).await;
    let vertex = client(&url, "batches");
    let inputs: Vec<(String, Task)> = (0..260)
        .map(|i| {
            let word = if i % 2 == 0 { "kernel" } else { "bins" };
            (format!("{word}{i}"), Task::Document)
        })
        .collect();
    let (vectors, usd) = vertex.embed(&inputs).await.unwrap();
    assert_eq!(vectors.len(), 260);
    assert_eq!(vectors[0][0], 1.0);
    assert_eq!(vectors[259][1], 1.0);
    let characters: usize = inputs.iter().map(|(t, _)| t.len()).sum();
    assert!((usd.unwrap() - price(characters)).abs() < 1e-15);
    let seen = seen.lock().unwrap();
    let sizes: Vec<usize> = seen
        .iter()
        .map(|s| s.body["instances"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [250, 10]);
}

#[tokio::test]
async fn an_error_status_is_a_refusal_that_names_vertex() {
    fn forbidden(_: usize, _: &Value) -> (u16, String) {
        (
            403,
            json!({"error": {"code": 403, "message": "Permission denied on resource project p.", "status": "PERMISSION_DENIED"}}).to_string(),
        )
    }
    let (url, _) = fake(forbidden).await;
    let error = client(&url, "forbidden")
        .embed(&[("a".to_string(), Task::Query)])
        .await
        .unwrap_err();
    assert!(error.refused);
    assert!(
        error.message.contains("Vertex AI returned HTTP 403"),
        "{}",
        error.message
    );
    assert!(error.message.contains("Permission denied"));
    assert!(!error.message.contains("OpenRouter"));
}

#[tokio::test]
async fn no_token_is_a_refusal_before_anything_is_sent() {
    let (url, seen) = fake(ok).await;
    let vertex = Vertex::new(&url, Token::File(scratch("no-token").join("absent")));
    let error = vertex
        .embed(&[("a".to_string(), Task::Query)])
        .await
        .unwrap_err();
    assert!(error.refused);
    assert!(error.message.contains("can't read the Vertex token"));
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failure_after_a_billed_request_leaves_the_cost_unknown() {
    fn second_fails(index: usize, body: &Value) -> (u16, String) {
        if index == 0 {
            (200, predictions(body))
        } else {
            (
                400,
                json!({"error": {"message": "too many tokens"}}).to_string(),
            )
        }
    }
    let (url, _) = fake(second_fails).await;
    let inputs: Vec<(String, Task)> = (0..251)
        .map(|i| (format!("t{i}"), Task::Document))
        .collect();
    let error = client(&url, "second").embed(&inputs).await.unwrap_err();
    assert!(!error.refused, "the first request was billed");
    assert!(
        error.message.contains("cost is unknown"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn a_malformed_response_is_not_a_refusal() {
    fn short(_: usize, _: &Value) -> (u16, String) {
        (
            200,
            json!({"predictions": [], "metadata": {"billableCharacterCount": 1}}).to_string(),
        )
    }
    let (url, _) = fake(short).await;
    let error = client(&url, "short")
        .embed(&[("a".to_string(), Task::Query)])
        .await
        .unwrap_err();
    assert!(!error.refused);
    assert!(error.message.contains("0 vectors for 1 inputs"));
}

#[tokio::test]
async fn a_response_without_billable_characters_has_an_unknown_cost() {
    fn no_metadata(_: usize, body: &Value) -> (u16, String) {
        let mut reply: Value = serde_json::from_str(&predictions(body)).unwrap();
        reply.as_object_mut().unwrap().remove("metadata");
        (200, reply.to_string())
    }
    let (url, _) = fake(no_metadata).await;
    let retriever = Retriever::new(base(), Embedder::with_vertex(client(&url, "unknown")), None);
    let search = retriever.search("kernel", 3).await;
    assert!(search.lexical_only.is_none());
    assert_eq!(
        search.usd, None,
        "an unreported cost is unknown, never zero"
    );
}

#[tokio::test]
async fn a_search_ranks_with_vertex_and_costs_its_characters() {
    let (url, seen) = fake(ok).await;
    let retriever = Retriever::new(base(), Embedder::with_vertex(client(&url, "search")), None);
    let search = retriever.search("kernel methods", 3).await;
    assert!(search.lexical_only.is_none());
    assert_eq!(search.hits[0].id, "stats.mmd");
    assert!(search.usd.unwrap() > 0.0);
    // The second search reuses the entry vectors and embeds only its query.
    let again = retriever.search("bins", 3).await;
    assert_eq!(again.hits[0].id, "stats.psi");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].body["instances"].as_array().unwrap().len(), 1);
    assert_eq!(seen[1].body["instances"][0]["task_type"], "RETRIEVAL_QUERY");
}

/// An embedder with three dimensions under another model name.
struct Other;

impl Embed for Other {
    fn model(&self) -> &str {
        "openai/text-embedding-3-small"
    }

    async fn embed(
        &self,
        inputs: Vec<String>,
    ) -> Result<(Vec<Vec<f32>>, Option<f64>), crate::search::EmbedError> {
        Ok((
            inputs.iter().map(|_| vec![0.5, 0.5, 0.5]).collect(),
            Some(0.0),
        ))
    }
}

#[tokio::test]
async fn the_cache_keeps_each_models_vectors_apart() {
    let cache = scratch("cache").join("embeddings.json");
    let other = Retriever::new(base(), Other, Some(cache.clone()));
    other.search("kernel", 3).await;
    let (url, seen) = fake(ok).await;
    let vertex = Retriever::new(
        base(),
        Embedder::with_vertex(client(&url, "cache-vertex")),
        Some(cache.clone()),
    );
    let search = vertex.search("kernel", 3).await;
    assert!(search.lexical_only.is_none(), "{:?}", search.lexical_only);
    // Vertex embedded every entry itself rather than reuse the other
    // model's cached vectors: three entries and the query.
    assert_eq!(
        seen.lock().unwrap()[0].body["instances"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let stored: HashMap<String, HashMap<String, Vec<f32>>> =
        serde_json::from_str(&std::fs::read_to_string(&cache).unwrap()).unwrap();
    let mut models: Vec<&String> = stored.keys().collect();
    models.sort();
    assert_eq!(
        models,
        ["openai/text-embedding-3-small", "vertex/text-embedding-005"]
    );
    assert!(
        stored["openai/text-embedding-3-small"]
            .values()
            .all(|v| v.len() == 3)
    );
    assert!(
        stored["vertex/text-embedding-005"]
            .values()
            .all(|v| v.len() == 4)
    );
}

#[test]
fn an_index_refuses_a_query_from_another_model() {
    let entries = base().entries;
    let index: HashMap<String, Vec<f32>> = entries
        .iter()
        .map(|e| (e.digest.clone(), vec![1.0, 0.0, 0.0, 0.0]))
        .collect();
    let same = Embedded {
        model: CACHE_MODEL.to_string(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
    };
    let scores = rank(CACHE_MODEL, &index, &entries, &same).unwrap();
    assert!(scores.iter().all(|s| (s - 1.0).abs() < 1e-9));
    let other = Embedded {
        model: "openai/text-embedding-3-small".to_string(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
    };
    let refused = rank(CACHE_MODEL, &index, &entries, &other).unwrap_err();
    assert!(
        refused
            .message
            .contains("vectors from different models can't be compared"),
        "{}",
        refused.message
    );
    // A vector of another length is another model's, whatever its key says.
    let wrong = Embedded {
        model: CACHE_MODEL.to_string(),
        vector: vec![1.0, 0.0, 0.0],
    };
    let refused = rank(CACHE_MODEL, &index, &entries, &wrong).unwrap_err();
    assert!(
        refused.message.contains("dimensions"),
        "{}",
        refused.message
    );
}

#[test]
fn the_models_url_names_project_and_region() {
    assert_eq!(
        models_url("openagentsgemini", "us-central1"),
        "https://us-central1-aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/us-central1/publishers/google/models"
    );
}

/// One live call to Vertex AI, for the verification record: three short
/// entries and a query, embedded with the configuration `Embedder::vertex`
/// reads. Run it by hand with `KB_VERTEX_PROJECT` set, gcloud
/// authenticated (`CLOUDSDK_CONFIG`), and `--ignored`.
#[tokio::test]
#[ignore = "calls Vertex AI and costs a fraction of a cent"]
async fn live_a_handful_of_strings() {
    let embedder = Embedder::vertex().unwrap();
    let cache = scratch("live").join("embeddings.json");
    let retriever = Retriever::new(base(), embedder, Some(cache.clone()));
    let search = retriever.search("kernel two-sample test", 3).await;
    assert!(search.lexical_only.is_none(), "{:?}", search.lexical_only);
    let characters: usize = base()
        .entries
        .iter()
        .map(Entry::search_text)
        .chain(["kernel two-sample test".to_string()])
        .map(|t| t.chars().filter(|c| !c.is_whitespace()).count())
        .sum();
    let stored: HashMap<String, HashMap<String, Vec<f32>>> =
        serde_json::from_str(&std::fs::read_to_string(&cache).unwrap()).unwrap();
    let dimensions = stored[CACHE_MODEL].values().next().map_or(0, Vec::len);
    println!(
        "live: model {CACHE_MODEL}, {} vectors of {dimensions} dimensions, top hit {} ({:.3}), \
         cost {:?} (about {:.10} expected for {characters} non-whitespace characters)",
        stored[CACHE_MODEL].len(),
        search.hits[0].id,
        search.hits[0].score,
        search.usd,
        price(characters)
    );
    assert!(search.usd.is_some_and(|usd| usd > 0.0 && usd < 0.01));
}
