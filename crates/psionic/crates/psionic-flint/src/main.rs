//! Flint's bounded native text endpoint and offline checkpoint runner.
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{get, post},
};
use candle::Device;
use psionic_flint::{
    MODEL_ID, Message, Model, REVISION, Sampling, generate, load_tokenizer, render,
};
use serde::Deserialize;
use serde_json::{Value, json};

struct Runtime {
    model: Model,
    tokenizer: tokenizers::Tokenizer,
    context: usize,
}
type Shared = Arc<Mutex<Runtime>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    model: String,
    messages: Vec<Message>,
    #[serde(default = "default_tokens", alias = "max_completion_tokens")]
    max_tokens: usize,
    temperature: Option<f64>,
    top_p: Option<f64>,
    top_k: Option<usize>,
    seed: Option<u64>,
    #[serde(default)]
    stream: bool,
    tools: Option<Value>,
}
fn default_tokens() -> usize {
    2048
}

fn run(runtime: &Runtime, request: Request) -> Result<Value> {
    ensure!(
        request.model == MODEL_ID || request.model == "flint",
        "unknown model"
    );
    ensure!(
        !request.stream,
        "streaming is not available on this endpoint"
    );
    ensure!(
        request.tools.is_none(),
        "tool requests are not available on this endpoint"
    );
    ensure!(
        request.max_tokens <= 8192,
        "max_tokens must be at most 8192"
    );
    let prompt = render(&request.messages)?;
    let encoding = runtime
        .tokenizer
        .encode(prompt, false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut sampling = Sampling::default();
    if let Some(v) = request.temperature {
        sampling.temperature = v;
    }
    if let Some(v) = request.top_p {
        sampling.top_p = v;
    }
    if let Some(v) = request.top_k {
        sampling.top_k = v;
    }
    if let Some(v) = request.seed {
        sampling.seed = v;
    }
    let result = generate(
        &runtime.model,
        &runtime.tokenizer,
        encoding.get_ids(),
        request.max_tokens,
        runtime.context,
        &sampling,
        900.0,
    )?;
    static REQUEST_ID: AtomicU64 = AtomicU64::new(0);
    let created = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let id = format!(
        "chatcmpl-flint-{created}-{}",
        REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    );
    Ok(json!({
        "id": id, "created": created, "object": "chat.completion", "model": MODEL_ID,
        "choices": [{"index": 0, "message": {"role": "assistant", "content": result.answer.content,
            "reasoning_content": result.answer.reasoning_content}, "finish_reason": result.finish_reason}],
        "usage": {"prompt_tokens": result.prompt_tokens, "completion_tokens": result.token_ids.len(), "total_tokens": result.prompt_tokens + result.token_ids.len()},
        "psionic": {"engine": "rust_candle_flint", "backend": if runtime.model.device.is_cpu() { "cpu" } else { "cuda" },
            "revision": REVISION, "config_sha256": runtime.model.config_sha256, "numerical_profile":"bf16_weights_f32_compute_v1",
            "elapsed_seconds": result.elapsed_seconds, "prefill_seconds": result.prefill_seconds,
            "token_ids": result.token_ids}
    }))
}

async fn completion(
    State(state): State<Shared>,
    Json(request): Json<Request>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tokio::task::spawn_blocking(move || {
        let runtime = state.try_lock().map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error":{"message":"The model is busy. Try again shortly."}})),
            )
        })?;
        run(&runtime, request).map(Json).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":{"message":e.to_string()}})),
            )
        })
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"The request could not finish."}})),
        )
    })?
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    if mode == "--help" || mode.is_empty() {
        println!(
            "psionic-flint (serve|generate|logits|tokenize) --weights DIR [--backend cpu|cuda] [--context 8192] [--listen 127.0.0.1:8080] [--request FILE]"
        );
        return Ok(());
    }
    ensure!(
        ["serve", "generate", "logits", "tokenize"].contains(&mode.as_str()),
        "unknown command"
    );
    let mut weights = None;
    let mut backend = "cpu".to_string();
    let mut context = 8192;
    let mut listen: SocketAddr = "127.0.0.1:8080".parse()?;
    let mut request = None;
    while let Some(key) = args.next() {
        let value = args.next().context("missing option value")?;
        match key.as_str() {
            "--weights" => weights = Some(PathBuf::from(value)),
            "--backend" => backend = value,
            "--context" => context = value.parse()?,
            "--listen" => listen = value.parse()?,
            "--request" => request = Some(PathBuf::from(value)),
            _ => bail!("unknown option {key}"),
        }
    }
    let device = match backend.as_str() {
        "cpu" => Device::Cpu,
        #[cfg(feature = "cuda")]
        "cuda" => Device::new_cuda(0)?,
        _ => bail!("backend is unavailable in this build"),
    };
    let root = weights.context("--weights is required")?;
    let tokenizer = load_tokenizer(&root.join("tokenizer.json"))?;
    if mode == "tokenize" {
        let req: Request =
            serde_json::from_slice(&std::fs::read(request.context("--request is required")?)?)?;
        let rendered = render(&req.messages)?;
        let ids = tokenizer
            .encode(rendered.clone(), false)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        println!("{}", json!({"rendered":rendered,"token_ids":ids.get_ids()}));
        return Ok(());
    }
    let model = Model::load(&root, device)?;
    ensure!(
        context > 0 && context <= model.config.max_position_embeddings,
        "context exceeds the checkpoint limit"
    );
    let runtime = Runtime {
        model,
        tokenizer,
        context,
    };
    if mode == "logits" {
        let value: Value =
            serde_json::from_slice(&std::fs::read(request.context("--request is required")?)?)?;
        let ids: Vec<u32> = serde_json::from_value(value["token_ids"].clone())?;
        ensure!(
            !ids.is_empty() && ids.len() <= context,
            "invalid token input"
        );
        let mut state = runtime.model.session(context)?;
        for (position, token) in ids.iter().enumerate() {
            let logits = runtime.model.forward(*token, &mut state)?;
            println!(
                "{}",
                json!({"position":position,"token":token,"logits":logits})
            );
        }
    } else if mode == "generate" {
        let req =
            serde_json::from_slice(&std::fs::read(request.context("--request is required")?)?)?;
        println!("{}", serde_json::to_string_pretty(&run(&runtime, req)?)?);
    } else {
        let state = Arc::new(Mutex::new(runtime));
        let app = Router::new()
            .route("/healthz", get(|| async { Json(json!({"status":"ready"})) }))
            .route("/v1/models", get(|| async { Json(json!({"object":"list","data":[{"id":MODEL_ID,"object":"model","owned_by":"Recursive"}]})) }))
            .route("/v1/chat/completions", post(completion))
            .layer(DefaultBodyLimit::max(256 * 1024)).with_state(state);
        eprintln!("Flint ready on {listen}");
        axum::serve(tokio::net::TcpListener::bind(listen).await?, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
    }
    Ok(())
}
