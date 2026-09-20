//! Kev's refusal contract end to end: real `kev-serve` responses parsed by
//! `jev` and filed by `gym::eval::classify`.
//!
//! The door is the real router over a real `DecisionModel`, but the model is
//! a one-layer backbone this file writes itself — a few kilobytes of
//! safetensors and a word-level tokenizer — so no downloaded weights or paid
//! inference are involved. Three variants load: `kev-test`, which answers;
//! `kev-broken`, whose embedding table is too small for the tokenizer's ids,
//! so a valid request to it fails inside the forward pass — a door-owned
//! `inference_failure` on input the contract accepts; and `kev-wide`, which
//! answers like `kev-test` but with four times the attention heads, so one
//! forward of it costs more working memory and the door grants it fewer
//! slots.
//!
//! Costs the tests state come from [`Variant::forward_bytes`] on these
//! configs at the rig's token bound, so they are the door's own arithmetic
//! rather than a number typed here.
//!
//! Builds only under `--features serve`.

#![cfg(feature = "serve")]

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use candle_core::Device;
use gym::eval::{Disposition, observations};
use gym::row::RefusalCode;
use kev::decision::DecisionModel;
use kev::serve::{Admission, MIB, ServeState, Variant, router};
use serde_json::{Value, json};

const HIDDEN: usize = 16;
const INTER: usize = 32;
const DP: usize = 8;
/// Attention heads of `kev-test` and `kev-broken`.
const HEADS: usize = 2;
/// Attention heads of `kev-wide`; more heads mean more score memory per
/// forward, which is what the per-variant bound measures.
const WIDE_HEADS: usize = 8;
/// The rig's token bound; every forward cost below is at this length.
const RIG_TOKENS: usize = 200;
/// The rig's working-memory budget: enough that four `kev-test` forwards
/// fit beside each other, too little for four `kev-wide` forwards.
const RIG_BUDGET_MIB: usize = 12;

/// A word-level tokenizer that knows the five delimiter tokens `encode`
/// needs and maps every other word to `[UNK]`: one token per word, which is
/// all the budget tests need.
const TOKENIZER_JSON: &str = r#"{
  "version": "1.0",
  "truncation": null,
  "padding": null,
  "added_tokens": [
    {"id": 1, "content": "<|fim_prefix|>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
    {"id": 2, "content": "<|fim_middle|>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
    {"id": 3, "content": "<|box_start|>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
    {"id": 4, "content": "<|box_end|>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true},
    {"id": 5, "content": "<|fim_suffix|>", "single_word": false, "lstrip": false, "rstrip": false, "normalized": false, "special": true}
  ],
  "normalizer": null,
  "pre_tokenizer": {"type": "WhitespaceSplit"},
  "post_processor": null,
  "decoder": null,
  "model": {
    "type": "WordLevel",
    "unk_token": "[UNK]",
    "vocab": {
      "[UNK]": 0,
      "<|fim_prefix|>": 1,
      "<|fim_middle|>": 2,
      "<|box_start|>": 3,
      "<|box_end|>": 4,
      "<|fim_suffix|>": 5,
      "word": 6
    }
  }
}"#;

/// One safetensors file: an 8-byte header length, the JSON header, then the
/// F32 payload each entry's `data_offsets` spans. Every tensor is zeros —
/// the forward pass still runs and the pointer head returns a uniform
/// distribution, which is all the answering path needs.
fn write_safetensors(path: &Path, tensors: &[(&str, &[usize])]) {
    let mut header = String::from("{");
    let mut data: Vec<u8> = Vec::new();
    for (i, (name, shape)) in tensors.iter().enumerate() {
        let count: usize = shape.iter().product();
        let start = data.len();
        data.resize(start + count * 4, 0);
        if i > 0 {
            header.push(',');
        }
        let shape = shape
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(",");
        header.push_str(&format!(
            "\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{shape}],\"data_offsets\":[{start},{}]}}",
            data.len()
        ));
    }
    header.push('}');
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(&data);
    std::fs::write(path, bytes).expect("write safetensors");
}

/// Write one variant's files and return `(base_dir, adapter_dir)` for
/// [`DecisionModel::load`]. `<name>-base/` holds the one-layer backbone with
/// `vocab` embedding rows and `heads` attention heads; `<name>/` holds the
/// adapter the serving path reads — tokenizer, pointer head, and a LoRA
/// config that targets nothing.
fn write_variant(root: &Path, name: &str, vocab: usize, heads: usize) -> (PathBuf, PathBuf) {
    let adapter = root.join(name);
    let base = root.join(format!("{name}-base"));
    std::fs::create_dir_all(&adapter).expect("adapter dir");
    std::fs::create_dir_all(&base).expect("base dir");

    std::fs::write(
        base.join("config.json"),
        json!({
            "architectures": ["Qwen2ForCausalLM"],
            "hidden_size": HIDDEN,
            "intermediate_size": INTER,
            "num_hidden_layers": 1,
            "num_attention_heads": heads,
            "num_key_value_heads": heads,
            "rms_norm_eps": 1e-6,
            "rope_theta": 10000.0,
            "vocab_size": vocab,
            "max_position_embeddings": 32768,
        })
        .to_string(),
    )
    .expect("write config");
    write_safetensors(
        &base.join("model.safetensors"),
        &[
            ("model.embed_tokens.weight", &[vocab, HIDDEN]),
            ("model.layers.0.self_attn.q_proj.weight", &[HIDDEN, HIDDEN]),
            ("model.layers.0.self_attn.q_proj.bias", &[HIDDEN]),
            ("model.layers.0.self_attn.k_proj.weight", &[HIDDEN, HIDDEN]),
            ("model.layers.0.self_attn.k_proj.bias", &[HIDDEN]),
            ("model.layers.0.self_attn.v_proj.weight", &[HIDDEN, HIDDEN]),
            ("model.layers.0.self_attn.v_proj.bias", &[HIDDEN]),
            ("model.layers.0.self_attn.o_proj.weight", &[HIDDEN, HIDDEN]),
            ("model.layers.0.mlp.gate_proj.weight", &[INTER, HIDDEN]),
            ("model.layers.0.mlp.up_proj.weight", &[INTER, HIDDEN]),
            ("model.layers.0.mlp.down_proj.weight", &[HIDDEN, INTER]),
            ("model.layers.0.input_layernorm.weight", &[HIDDEN]),
            ("model.layers.0.post_attention_layernorm.weight", &[HIDDEN]),
            ("model.norm.weight", &[HIDDEN]),
        ],
    );

    std::fs::write(adapter.join("tokenizer.json"), TOKENIZER_JSON).expect("write tokenizer");
    std::fs::write(
        adapter.join("adapter_config.json"),
        json!({"r": 1, "lora_alpha": 1.0, "target_modules": []}).to_string(),
    )
    .expect("write adapter config");
    write_safetensors(
        &adapter.join("adapter_model.safetensors"),
        &[("unused.weight", &[1])],
    );
    write_safetensors(
        &adapter.join("head.safetensors"),
        &[
            ("q.weight", &[DP, HIDDEN]),
            ("q.bias", &[DP]),
            ("k.weight", &[DP, HIDDEN]),
            ("k.bias", &[DP]),
        ],
    );
    std::fs::write(
        adapter.join("head_meta.json"),
        json!({"base": "tiny-test-base", "option_isolation": false}).to_string(),
    )
    .expect("write head meta");

    (base, adapter)
}

/// The tempdir the variants live in, kept alive for the test process.
struct Rig {
    _dir: tempfile::TempDir,
    state: Arc<ServeState>,
}

fn rig() -> &'static Rig {
    static RIG: OnceLock<Rig> = OnceLock::new();
    RIG.get_or_init(|| build_rig(4, RIG_BUDGET_MIB * MIB))
}

/// A rig of its own, with `concurrency` host forward slots and
/// `memory_budget_bytes` of working memory.
fn build_rig(concurrency: usize, memory_budget_bytes: usize) -> Rig {
    {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut variants = Vec::new();
        // `kev-broken` loads an embedding table smaller than any id the
        // tokenizer emits, so every valid request to it fails at the
        // embedding lookup — a deterministic inference failure.
        for (name, vocab, heads) in [
            ("kev-test", 64usize, HEADS),
            ("kev-broken", 4usize, HEADS),
            ("kev-wide", 64usize, WIDE_HEADS),
        ] {
            let (base, adapter) = write_variant(dir.path(), name, vocab, heads);
            let model = DecisionModel::load(&base, &adapter, Device::Cpu).expect("load");
            variants.push(Variant {
                model,
                model_id: name.to_string(),
                run: adapter.display().to_string(),
                base: "tiny-test-base".to_string(),
                base_revision: "test-revision".to_string(),
                lora: 1,
            });
        }
        Rig {
            _dir: dir,
            state: Arc::new(
                ServeState::new(
                    variants,
                    0,
                    vec!["jev-latest".to_string()],
                    "cpu".to_string(),
                    Admission {
                        max_questions: 3,
                        max_total_options: 20,
                        max_total_tokens: RIG_TOKENS,
                        concurrency,
                        memory_budget_bytes,
                        ..Admission::default()
                    },
                )
                .expect("a valid serving state"),
            ),
        }
    }
}

/// Bind the router on an ephemeral port; returns its base URL.
async fn serve() -> String {
    serve_state(rig().state.clone()).await
}

async fn serve_state(state: Arc<ServeState>) -> String {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, router(state)).await.expect("serve");
    });
    format!("http://{addr}")
}

/// Run the blocking jev client off the test's runtime thread.
fn blocking<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(f).join().expect("client thread")
}

fn client(url: &str) -> jev::BlockingClient {
    jev::BlockingClient::new(
        jev::Config::new()
            .api_key("kev-test")
            .base_url(url)
            .default_model("kev-latest"),
    )
    .expect("client")
}

/// No retries: the refusal under test is the first answer, not the settled
/// one a retry policy would wait for.
fn no_retry() -> jev::RetryPolicy {
    jev::RetryPolicy {
        max_retries: 0,
        ..jev::RetryPolicy::default()
    }
}

/// One valid request: a short state and one `noul` question.
fn request(instructions: &str) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(
        "The package arrived two days late and the box was torn.",
        jev::Questions::new().with("verdict", jev::Noul::new(instructions)),
    )
    .retry(no_retry())
}

/// The `jev::Error` one refused call returns, and nothing else.
fn refused_error(result: jev::Result<jev::SystemOneResponse>) -> jev::Error {
    match result {
        Err(error) => error,
        Ok(_) => panic!("the door answered a request it should refuse"),
    }
}

/// The `jev::Error` the client builds for one raw response — the path
/// `system_one` takes for bodies jev's own checks will not send.
fn api_error(status: u16, headers: &reqwest::header::HeaderMap, body: Value) -> jev::Error {
    jev::Error::from(jev::ApiError {
        status,
        headers: headers.clone(),
        body: Some(jev::ResponseBody::Json(body)),
        request_id: None,
        endpoint: "POST http://kev-test/v1/systemone".to_string(),
        kind: jev::ApiErrorKind::of(status, headers),
    })
}

/// The item and the run a refused disposition turns into a row under.
fn item() -> gym::suite::Item {
    gym::suite::Item {
        id: "t-1".to_string(),
        family: "returns".to_string(),
        kind: "noul".to_string(),
        state: Value::Null,
        question: None,
        truth: "yes".to_string(),
        partition: gym::suite::Partition::Development,
        label_source: None,
        label_rule: None,
    }
}

fn run() -> gym::eval::Run {
    gym::eval::Run {
        suite: "test-suite".to_string(),
        suite_digest: "test-digest".to_string(),
        question_set: None,
        question_digest: None,
        door: "kev-test".to_string(),
        door_identity: gym::row::DoorIdentity::published("kev-test", "sig", "adapter"),
        estimator: "unreported".to_string(),
        samples: None,
        seed_base: None,
        recorded_at: "2026-09-20T00:00:00Z".to_string(),
        gate_id: None,
        gate_digest: None,
    }
}

/// The whole contract for one refusal: the wire body carries the typed code
/// beside `detail`, `classify` files it as the door's answer, and the row it
/// produces keeps the item in the denominator and out of the numerator.
fn expect_refusal(error: &jev::Error, status: u16, label: &str) {
    let jev::Error::Api(api) = error else {
        panic!("expected an API refusal, got {error:?}");
    };
    assert_eq!(api.status, status);
    let body = api
        .body
        .as_ref()
        .and_then(|body| body.as_json())
        .unwrap_or_else(|| panic!("expected a JSON refusal body, got {:?}", api.body));
    assert_eq!(body["error"]["code"], label, "body: {body}");
    assert!(body["detail"].is_string(), "body: {body}");
    assert_eq!(body["error"]["message"], body["detail"], "body: {body}");

    let disposition = gym::eval::classify(error);
    assert_eq!(
        disposition,
        Disposition::Refused(RefusalCode::from(label.to_string()))
    );
    assert!(disposition.is_recorded());
    let row = run()
        .row(&item(), None, &disposition, Some(1.0))
        .expect("a refusal produces a row");
    assert!(row.is_refused());
    assert!(!row.is_scored());
    row.check().expect("the refused row is consistent");
    assert!(observations(&[row]).is_empty());
}

/// The valid requests the door refuses, sent through the real `jev` client.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn door_refusals_classify_through_jev_and_gym() {
    let url = serve().await;
    blocking(move || {
        let client = client(&url);

        // Model selection: the named variant is not loaded, and the refusal
        // names every id and alias the door does honor.
        let error = refused_error(client.system_one(request("Was it late?").model("kev-nope")));
        expect_refusal(&error, 503, "model_unavailable");
        let jev::Error::Api(api) = &error else {
            unreachable!("checked above")
        };
        let detail = api.body.as_ref().and_then(|b| b.as_json()).expect("json")["detail"]
            .as_str()
            .expect("detail")
            .to_string();
        assert!(detail.contains("jev-latest"), "{detail}");
        assert!(detail.contains("kev-test"), "{detail}");

        // An unmodified jev client sends `jev-latest`, the alias the
        // listing advertises; the default variant answers it.
        let unmodified =
            jev::BlockingClient::new(jev::Config::new().api_key("kev-test").base_url(&url))
                .expect("client");
        let answered = unmodified
            .system_one(request("Was it late?"))
            .expect("the advertised alias resolves");
        assert_eq!(answered.model, "kev-test");
        let explicit = client
            .system_one(request("Was it late?").model("kev-test"))
            .expect("the explicit id resolves");
        assert_eq!(explicit.model, "kev-test");

        // Admission, before any encoding: more questions than one pass
        // admits, and more options summed over the request than it admits,
        // each a typed refusal that names the bound.
        let mut questions = jev::Questions::new();
        for i in 0..4 {
            questions = questions.with(format!("q{i}"), jev::Noul::new("Was it late?"));
        }
        let error =
            refused_error(client.system_one(
                jev::SystemOneRequest::new("A short state.", questions).retry(no_retry()),
            ));
        expect_refusal(&error, 422, "invalid_request");
        assert!(error.to_string().contains("at most 3"), "{error}");
        let mut choice = jev::Choice::new("Which?", indexmap::IndexMap::new());
        for i in 0..21 {
            choice = choice.option(format!("option-{i}"), "one of many");
        }
        let error = refused_error(
            client.system_one(
                jev::SystemOneRequest::new(
                    "A short state.",
                    jev::Questions::new().with("which", choice),
                )
                .retry(no_retry()),
            ),
        );
        expect_refusal(&error, 422, "too_many_options");
        assert!(error.to_string().contains("at most 20"), "{error}");

        // Admission after encoding, before the mask: a packed sequence over
        // the total token budget, though every branch fits its own.
        let error = refused_error(
            client.system_one(
                jev::SystemOneRequest::new(
                    "word ".repeat(400),
                    jev::Questions::new().with("verdict", jev::Noul::new("Was it late?")),
                )
                .retry(no_retry()),
            ),
        );
        expect_refusal(&error, 413, "branch_too_long");
        assert!(error.to_string().contains("attention mask"), "{error}");

        // Capacity: a state over the serving budget on an otherwise valid
        // request — a door-owned refusal, not a malformed body.
        let error = refused_error(client.system_one(jev::SystemOneRequest::new(
            "word ".repeat(9000),
            jev::Questions::new().with("verdict", jev::Noul::new("Was it late?")),
        )));
        expect_refusal(&error, 413, "branch_too_long");

        // Inference failure: `kev-broken`'s embedding table is too small for
        // the tokenizer's ids, so the request fails inside the forward pass.
        let error = refused_error(client.system_one(request("Was it late?").model("kev-broken")));
        expect_refusal(&error, 500, "inference_failure");

        // A question type the SDK ships verbatim and the door does not
        // model: `invalid_request`, naming the question at fault.
        let error = refused_error(client.system_one(jev::SystemOneRequest::new(
            "A short state.",
            jev::Questions::new().with(
                "verdict",
                json!({"type": "mystery", "instructions": "Was it late?"}),
            ),
        )));
        expect_refusal(&error, 422, "invalid_request");
        let jev::Error::Api(api) = &error else {
            unreachable!("checked above")
        };
        let body = api.body.as_ref().and_then(|b| b.as_json()).expect("json");
        assert_eq!(body["error"]["question"], "verdict");
    });
}

/// Bodies jev's own checks will not send, posted raw and classified through
/// the same `jev::Error` the client would have built.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_refusals_classify_too() {
    let url = serve().await;
    let http = reqwest::Client::new();
    let post = |body: &str| {
        let body = body.to_owned();
        let http = http.clone();
        let url = url.clone();
        async move {
            let response = http
                .post(format!("{url}/v1/systemone"))
                .header("content-type", "application/json")
                .body(body)
                .send()
                .await
                .expect("send");
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let body: Value = response.json().await.expect("json");
            (status, headers, body)
        }
    };

    // Malformed JSON: the body never became a request, and the door still
    // answers with a typed code rather than an error page.
    let (status, headers, body) = post("{not json").await;
    expect_refusal(&api_error(status, &headers, body), 422, "invalid_request");

    // Empty questions: jev refuses this before any request, so it goes over
    // the wire the way a raw caller sends it.
    let (status, headers, body) = post(r#"{"state":"x","questions":{}}"#).await;
    expect_refusal(&api_error(status, &headers, body), 422, "invalid_request");

    // A score with one level: below the contract's minimum, with the
    // question named.
    let (status, headers, body) =
        post(r#"{"state":"x","questions":{"verdict":{"type":"score","criteria":["only"]}}}"#).await;
    let error = api_error(status, &headers, body.clone());
    expect_refusal(&error, 422, "invalid_request");
    assert_eq!(body["error"]["question"], "verdict");

    // A choice over the contract's 255-option bound gets its own code.
    let criteria: serde_json::Map<String, Value> = (0..256)
        .map(|i| (format!("option-{i}"), Value::Null))
        .collect();
    let (status, headers, body) = post(
        &json!({
            "state": "x",
            "questions": {"verdict": {"type": "choice", "criteria": criteria}},
        })
        .to_string(),
    )
    .await;
    let error = api_error(status, &headers, body.clone());
    expect_refusal(&error, 422, "too_many_options");
    assert_eq!(body["error"]["question"], "verdict");
}

/// The failure modes that are not the door's answer stay out of the record.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_failures_stay_harness() {
    // A port nothing listens on: the request never became a response.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let url = format!("http://127.0.0.1:{port}");
    blocking(move || {
        let client = client(&url);
        let error = refused_error(client.system_one(request("Was it late?")));
        assert!(
            matches!(&error, jev::Error::Connection { .. }),
            "expected a connection failure, got {error:?}"
        );
        let disposition = gym::eval::classify(&error);
        assert!(
            matches!(disposition, Disposition::Harness(_)),
            "{disposition:?}"
        );
        assert!(!disposition.is_recorded());
        // A harness failure produces no row: the item leaves the record
        // entirely rather than reading as a door answer.
        assert!(run().row(&item(), None, &disposition, None).is_none());
    });
}

/// A reachable endpoint whose error body carries no code is not a refusal
/// either — the classifier reads the code, never the prose.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_error_body_without_a_code_stays_harness() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "client did not connect"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        let timeout = Some(std::time::Duration::from_secs(5));
        stream.set_read_timeout(timeout).expect("read timeout");
        stream.set_write_timeout(timeout).expect("write timeout");
        let mut buf = [0u8; 8192];
        assert!(stream.read(&mut buf).expect("request") > 0);
        let body = "too busy, come back later";
        stream.write_all(
            format!(
                "HTTP/1.1 503 Service Unavailable\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            ).as_bytes(),
        ).expect("response");
    });
    let url = format!("http://127.0.0.1:{port}");
    blocking(move || {
        let client = client(&url);
        let error = refused_error(client.system_one(request("Was it late?")));
        let jev::Error::Api(api) = &error else {
            panic!("a 503 is an API error: {error:?}");
        };
        assert_eq!(api.status, 503);
        assert!(api.body.as_ref().and_then(|b| b.as_json()).is_none());
        let disposition = gym::eval::classify(&error);
        assert!(
            matches!(disposition, Disposition::Harness(_)),
            "{disposition:?}"
        );
    });
    server.join().expect("server thread");
}

/// The body kev sent before this contract: `detail` alone is prose, not a
/// refusal code, and nothing may read it as one.
#[test]
fn a_detail_only_body_is_not_a_refusal() {
    let body = jev::ResponseBody::Json(json!({
        "detail": "questions must hold at least one question"
    }));
    let disposition = gym::eval::classify_response(422, Some(&body), "HTTP 422");
    assert!(
        matches!(disposition, Disposition::Harness(_)),
        "{disposition:?}"
    );
}

/// Sanity for the test model itself: a valid request is answered, not
/// refused, and jev decodes it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_valid_request_round_trips() {
    let url = serve().await;
    blocking(move || {
        let response = client(&url)
            .system_one(request("Was the delivery late?"))
            .expect("system_one");
        assert_eq!(response.model, "kev-test");
        assert_eq!(response.answers.len(), 1);
        let noul = response.noul("verdict").expect("noul answer");
        assert!((0.0..=1.0).contains(&noul.noul));
    });
}

/// The HTTP body cap is also a door-owned capacity refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_http_body_is_a_typed_refusal() {
    let url = serve().await;
    blocking(move || {
        let request = jev::SystemOneRequest::new(
            "x".repeat(3 * 1024 * 1024),
            jev::Questions::new().with("verdict", jev::Noul::new("Was it late?")),
        )
        .retry(no_retry());
        let error = refused_error(client(&url).system_one(request));
        expect_refusal(&error, 413, "payload_too_large");
    });
}

/// A response cut off before its declared body length is a transport loss.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_response_body_stays_harness() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let server = tokio::spawn(async move {
        tokio::time::timeout(std::time::Duration::from_secs(5), async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut request = [0; 8192];
            assert!(stream.read(&mut request).await.expect("request") > 0);
            stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{}").await.expect("partial response");
            stream.shutdown().await.expect("disconnect");
        }).await.expect("bounded server");
    });
    blocking(move || {
        let error = refused_error(client(&url).system_one(request("Was it late?")));
        let disposition = gym::eval::classify(&error);
        assert!(
            matches!(disposition, Disposition::Harness(_)),
            "{disposition:?}"
        );
        assert!(run().row(&item(), None, &disposition, None).is_none());
    });
    server.await.expect("server task");
}

/// With every forward slot taken, the door answers `busy` at once instead
/// of queueing; the slot's release lets the next request through.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_door_answers_busy_and_recovers() {
    let rig = build_rig(1, RIG_BUDGET_MIB * MIB);
    let state = rig.state.clone();
    let url = serve_state(state.clone()).await;
    let held = state
        .slots
        .clone()
        .try_acquire_owned()
        .expect("the one slot");
    let refused = blocking({
        let url = url.clone();
        move || refused_error(client(&url).system_one(request("Was it late?")))
    });
    expect_refusal(&refused, 503, "busy");
    assert!(
        refused.to_string().contains("the host's forward slots"),
        "{refused}"
    );
    drop(held);
    blocking(move || {
        client(&url)
            .system_one(request("Was it late?"))
            .expect("the freed slot admits the next request");
    });
}

/// A request for `model`, so a test can address one variant.
fn request_for(model: &str, question: &str) -> jev::SystemOneRequest {
    let mut request = request(question);
    request.model = Some(model.to_string());
    request
}

/// Each variant's slots come from its own forward cost against the one
/// budget, so a heavier variant gets fewer, and none gets more than the
/// host's slots.
#[test]
fn each_variant_is_bounded_by_its_own_forward_cost() {
    let state = &rig().state;
    assert_eq!(state.memory_mib, RIG_BUDGET_MIB);
    let test = state.select_index("kev-test").expect("kev-test");
    let wide = state.select_index("kev-wide").expect("kev-wide");
    for (index, variant) in state.variants.iter().enumerate() {
        let share = &state.variant_slots[index];
        let expected_mib = variant.forward_bytes(RIG_TOKENS).div_ceil(MIB).max(1);
        assert_eq!(share.forward_mib, expected_mib, "{}", variant.model_id);
        assert_eq!(
            share.limit,
            (RIG_BUDGET_MIB / expected_mib).min(state.admission.concurrency),
            "{}",
            variant.model_id
        );
        assert!(share.limit >= 1, "{}", variant.model_id);
    }
    let (test, wide) = (&state.variant_slots[test], &state.variant_slots[wide]);
    assert!(
        wide.forward_mib > test.forward_mib,
        "kev-wide {} MiB, kev-test {} MiB",
        wide.forward_mib,
        test.forward_mib
    );
    assert!(
        wide.limit < test.limit,
        "kev-wide {} slots at {} MiB, kev-test {} slots at {} MiB",
        wide.limit,
        wide.forward_mib,
        test.limit,
        test.forward_mib
    );
    // More tokens cost more; the estimate is monotone.
    let variant = &state.variants[0];
    assert!(variant.forward_bytes(2 * RIG_TOKENS) > variant.forward_bytes(RIG_TOKENS));
    assert_eq!(variant.forward_bytes(0), 0);
}

/// `kev-wide` at its own limit answers `busy` naming that variant, while
/// `kev-test` on the same host keeps answering; freeing the slot lets the
/// next `kev-wide` request through.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saturated_variant_refuses_alone() {
    let rig = build_rig(4, RIG_BUDGET_MIB * MIB);
    let state = rig.state.clone();
    let url = serve_state(state.clone()).await;
    let wide = state.select_index("kev-wide").expect("kev-wide");
    let share = &state.variant_slots[wide];
    let held = share
        .slots
        .clone()
        .try_acquire_many_owned(u32::try_from(share.limit).expect("small"))
        .expect("every kev-wide slot");
    assert_eq!(state.in_flight(wide), share.limit);
    let refused = blocking({
        let url = url.clone();
        move || refused_error(client(&url).system_one(request_for("kev-wide", "Was it late?")))
    });
    expect_refusal(&refused, 503, "busy");
    assert!(
        refused.to_string().contains("the `kev-wide` forward slots"),
        "{refused}"
    );
    assert_eq!(
        state.slots.available_permits(),
        state.admission.concurrency,
        "a refusal holds no host slot"
    );
    assert_eq!(state.memory_in_use_mib(), 0, "a refusal holds no memory");
    blocking({
        let url = url.clone();
        move || {
            client(&url)
                .system_one(request_for("kev-test", "Was it late?"))
                .expect("kev-test answers while kev-wide is full");
        }
    });
    drop(held);
    blocking(move || {
        client(&url)
            .system_one(request_for("kev-wide", "Was it late?"))
            .expect("the freed kev-wide slot admits the next request");
    });
    assert_eq!(state.in_flight(wide), 0);
    assert_eq!(state.memory_in_use_mib(), 0);
}

/// With the memory budget spent, a variant whose slots are free is still
/// refused, and the refusal names the budget in MiB.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_spent_memory_budget_refuses_with_free_slots() {
    let rig = build_rig(4, RIG_BUDGET_MIB * MIB);
    let state = rig.state.clone();
    let url = serve_state(state.clone()).await;
    let test = state.select_index("kev-test").expect("kev-test");
    let leave = state.variant_slots[test].forward_mib - 1;
    let held = state
        .memory
        .clone()
        .try_acquire_many_owned(u32::try_from(state.memory_mib - leave).expect("small"))
        .expect("most of the budget");
    assert_eq!(state.memory_in_use_mib(), state.memory_mib - leave);
    assert_eq!(state.in_flight(test), 0);
    let refused = blocking({
        let url = url.clone();
        move || refused_error(client(&url).system_one(request("Was it late?")))
    });
    expect_refusal(&refused, 503, "busy");
    assert!(
        refused
            .to_string()
            .contains("the working-memory budget in MiB"),
        "{refused}"
    );
    assert_eq!(state.in_flight(test), 0, "a refusal holds no variant slot");
    assert_eq!(
        state.slots.available_permits(),
        4,
        "a refusal holds no host slot"
    );
    drop(held);
    blocking(move || {
        client(&url)
            .system_one(request("Was it late?"))
            .expect("the freed budget admits the next request");
    });
    assert_eq!(state.memory_in_use_mib(), 0);
}

/// Callers that give up on a full door leave nothing behind: the door
/// refuses instead of queueing, so after a burst against a held slot every
/// counter reads what it did before, and permits held by forwards in flight
/// are returned when those forwards end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_against_a_full_door_leaves_no_queue() {
    let rig = build_rig(1, RIG_BUDGET_MIB * MIB);
    let state = rig.state.clone();
    let url = serve_state(state.clone()).await;
    let held = state
        .slots
        .clone()
        .try_acquire_owned()
        .expect("the one slot");
    let mut refusals = Vec::new();
    for _ in 0..8 {
        let url = url.clone();
        refusals.push(tokio::task::spawn_blocking(move || {
            refused_error(client(&url).system_one(request("Was it late?")))
        }));
    }
    for refusal in refusals {
        let refused = refusal.await.expect("a refusal, not a hang");
        expect_refusal(&refused, 503, "busy");
    }
    assert_eq!(
        state.slots.available_permits(),
        0,
        "the held slot is the only one taken"
    );
    assert_eq!(state.memory_in_use_mib(), 0);
    for index in 0..state.variants.len() {
        assert_eq!(state.in_flight(index), 0);
    }
    drop(held);
    let mut answers = Vec::new();
    for _ in 0..8 {
        let url = url.clone();
        answers.push(tokio::task::spawn_blocking(move || {
            client(&url).system_one(request("Was it late?"))
        }));
    }
    let mut admitted = 0;
    for answer in answers {
        match answer.await.expect("an answer or a refusal") {
            Ok(_) => admitted += 1,
            Err(error) => expect_refusal(&error, 503, "busy"),
        }
    }
    assert!(admitted >= 1, "the freed slot admits at least one");
    assert_eq!(state.slots.available_permits(), 1, "every permit came back");
    assert_eq!(state.memory_in_use_mib(), 0);
    assert_eq!(state.in_flight(0), 0);
}

#[test]
fn token_bounds_admit_at_the_limit_and_refuse_past_it() {
    let admission = Admission::default();
    assert!(admission.admit_tokens(8192).is_ok());
    let error = admission
        .admit_tokens(8193)
        .expect_err("the token limit applies");
    assert_eq!(error.refusal().status(), 413);
    let kev::Error::TooManyTokens {
        tokens,
        max,
        attention_bytes,
    } = error
    else {
        panic!("expected a token-bound refusal");
    };
    assert_eq!(tokens, 8193);
    assert_eq!(max, 8192);
    assert_eq!(attention_bytes, 4 * 8193 * 8193);

    let admission = Admission {
        max_total_tokens: 10_000,
        max_attention_bytes: Admission::attention_bytes(100),
        ..Admission::default()
    };
    assert!(admission.admit_tokens(100).is_ok());
    assert!(matches!(
        admission.admit_tokens(101),
        Err(kev::Error::TooManyTokens { tokens: 101, .. })
    ));
    assert_eq!(Admission::attention_bytes(usize::MAX), usize::MAX);
    assert_eq!(Admission::attention_bytes(0), 0);
}

#[test]
fn the_delimiter_floor_is_refused_before_encoding() {
    assert_eq!(Admission::token_floor(2, 3), 11);
    let request = serde_json::from_value(json!({
        "state": "x",
        "questions": {
            "which": {
                "type": "choice",
                "instructions": "Which?",
                "criteria": {"a": null, "b": null, "c": null}
            },
            "late": {"type": "noul", "instructions": "Late?"}
        }
    }))
    .expect("request shape");

    let admission = Admission {
        max_total_tokens: 11,
        ..Admission::default()
    };
    assert!(admission.admit_shape(&request).is_ok());
    let admission = Admission {
        max_total_tokens: 10,
        ..Admission::default()
    };
    let error = admission
        .admit_shape(&request)
        .expect_err("the delimiter floor applies");
    assert!(matches!(
        error,
        kev::Error::SequenceFloorTooLong {
            questions: 2,
            options: 3,
            floor: 11,
            max: 10,
        }
    ));
    assert_eq!(error.refusal().status(), 413);
    assert_eq!(error.refusal().label(), "branch_too_long");
}

#[test]
fn every_advertised_alias_resolves() {
    let state = &rig().state;
    assert!(state.aliases.iter().any(|a| a == jev::defaults::MODEL));
    for model in ["jev-latest", "", "kev-latest"] {
        assert_eq!(
            state
                .select(model)
                .expect("default alias")
                .model_id
                .as_str(),
            "kev-test"
        );
    }
    assert_eq!(
        state
            .select("kev-broken")
            .expect("variant id")
            .model_id
            .as_str(),
        "kev-broken"
    );
    let error = match state.select("kev-nope") {
        Err(error) => error,
        Ok(_) => panic!("expected unknown model"),
    };
    let kev::Error::UnknownModel { model, known } = error else {
        panic!("expected unknown model");
    };
    assert_eq!(model, "kev-nope");
    for name in ["kev-test", "kev-broken", "kev-latest", "jev-latest"] {
        assert!(known.iter().any(|known| known == name), "{name}: {known:?}");
    }
}

/// The public constructor refuses a state the handlers could not serve
/// rather than letting a request find the hole.
#[test]
fn an_invalid_serving_state_is_refused_at_construction() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (base, adapter) = write_variant(dir.path(), "kev-test", 64, HEADS);
    let variant = || Variant {
        model: DecisionModel::load(&base, &adapter, Device::Cpu).expect("load"),
        model_id: "kev-test".to_string(),
        run: "kev-test".to_string(),
        base: "test-base".to_string(),
        base_revision: "test-revision".to_string(),
        lora: 1,
    };
    let aliases = || vec!["jev-latest".to_string()];
    let cpu = || "cpu".to_string();
    // Enough for one forward at the default token bound on this config.
    let admission = || Admission {
        memory_budget_bytes: 4096 * MIB,
        ..Admission::default()
    };
    let cases: [(&str, Result<ServeState, kev::Error>); 7] = [
        (
            "no variants",
            ServeState::new(Vec::new(), 0, aliases(), cpu(), admission()),
        ),
        (
            "default variant index 1",
            ServeState::new(vec![variant()], 1, aliases(), cpu(), admission()),
        ),
        (
            "alias `kev-test`",
            ServeState::new(
                vec![variant()],
                0,
                vec!["kev-test".to_string()],
                cpu(),
                admission(),
            ),
        ),
        (
            "admission bounds",
            ServeState::new(
                vec![variant()],
                0,
                aliases(),
                cpu(),
                Admission {
                    concurrency: 0,
                    ..admission()
                },
            ),
        ),
        (
            "admission bounds",
            ServeState::new(
                vec![variant()],
                0,
                aliases(),
                cpu(),
                Admission {
                    max_attention_bytes: 0,
                    ..admission()
                },
            ),
        ),
        (
            "admission bounds",
            ServeState::new(vec![variant()], 0, aliases(), cpu(), Admission::default()),
        ),
        (
            "the memory budget is 1 MiB",
            ServeState::new(
                vec![variant()],
                0,
                aliases(),
                cpu(),
                Admission {
                    memory_budget_bytes: 1,
                    ..Admission::default()
                },
            ),
        ),
    ];
    for (needle, result) in cases {
        let error = result.err().expect(needle);
        assert!(error.to_string().contains(needle), "{needle}: {error}");
    }
    assert!(
        ServeState::new(vec![variant()], 0, aliases(), cpu(), admission()).is_ok(),
        "a well-formed state constructs"
    );
}
