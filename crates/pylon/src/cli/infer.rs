//! pylon infer - Run a local inference request against detected backends

use clap::Args;
use compute::backends::{BackendRegistry, CompletionRequest};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};

#[cfg(feature = "gpt-oss-gguf")]
use ml::GptOssGgufBackend;

/// Arguments for the infer command
#[derive(Args)]
pub struct InferArgs {
    /// Model ID to use
    #[arg(long)]
    pub model: Option<String>,
    /// Prompt text
    #[arg(long)]
    pub prompt: String,
    /// Stream tokens as they arrive
    #[arg(long, default_value_t = true)]
    pub stream: bool,
    /// Max new tokens
    #[arg(long, default_value_t = 128)]
    pub max_tokens: usize,
    /// Temperature (0 for greedy)
    #[arg(long)]
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling
    #[arg(long)]
    pub top_p: Option<f32>,
    /// Top-k sampling
    #[arg(long)]
    pub top_k: Option<usize>,
    /// Layer limit
    #[arg(long)]
    pub layers: Option<usize>,
    /// Max KV tokens
    #[arg(long)]
    pub max_kv: Option<usize>,
    /// Force MoE fallback (expert 0 only)
    #[arg(long)]
    pub moe_fallback: bool,
    /// Disable Harmony prompt wrapper
    #[arg(long)]
    pub no_harmony: bool,
}

pub async fn run(args: InferArgs) -> anyhow::Result<()> {
    let mut registry = BackendRegistry::detect().await;

    #[cfg(feature = "gpt-oss-gguf")]
    if let Ok(backend) = GptOssGgufBackend::from_env() {
        registry.register_with_id("gpt-oss-gguf", std::sync::Arc::new(tokio::sync::RwLock::new(backend)));
    }

    let models = registry.list_all_models().await;
    if models.is_empty() {
        anyhow::bail!("no local backends detected");
    }

    let model_id = resolve_model_id(&models, args.model)?;
    let backend = registry
        .get(&model_id.0)
        .ok_or_else(|| anyhow::anyhow!("backend not available: {}", model_id.0))?;

    let mut request = CompletionRequest::new(model_id.1.clone(), args.prompt);
    request.stream = args.stream;
    request.max_tokens = Some(args.max_tokens);
    if let Some(temp) = args.temperature {
        request.temperature = Some(temp);
    }
    if let Some(top_p) = args.top_p {
        request.top_p = Some(top_p);
    }

    let mut extra = HashMap::new();
    if let Some(top_k) = args.top_k {
        extra.insert("top_k".to_string(), Value::from(top_k as u64));
    }
    if let Some(layers) = args.layers {
        extra.insert("layers".to_string(), Value::from(layers as u64));
    }
    if let Some(max_kv) = args.max_kv {
        extra.insert("max_kv".to_string(), Value::from(max_kv as u64));
    }
    if args.moe_fallback {
        extra.insert("moe_fallback".to_string(), Value::from(true));
    }
    if args.no_harmony {
        extra.insert("harmony".to_string(), Value::from(false));
    }
    request.extra = extra;

    if request.stream {
        let rx = backend.read().await.complete_stream(request).await?;
        stream_tokens(rx).await?;
    } else {
        let response = backend.read().await.complete(request).await?;
        println!("{}", response.text);
    }

    Ok(())
}

fn resolve_model_id(
    models: &[(String, compute::backends::ModelInfo)],
    model: Option<String>,
) -> anyhow::Result<(String, String)> {
    if let Some(model) = model {
        let hit = models.iter().find(|(_, info)| info.id == model);
        if let Some((backend_id, info)) = hit {
            return Ok((backend_id.clone(), info.id.clone()));
        }
        anyhow::bail!("model not found: {model}");
    }

    if let Ok(model) = std::env::var("GPT_OSS_GGUF_MODEL_ID") {
        let hit = models.iter().find(|(_, info)| info.id == model);
        if let Some((backend_id, info)) = hit {
            return Ok((backend_id.clone(), info.id.clone()));
        }
    }

    models
        .first()
        .map(|(backend_id, info)| (backend_id.clone(), info.id.clone()))
        .ok_or_else(|| anyhow::anyhow!("no models available"))
}

async fn stream_tokens(
    mut rx: tokio::sync::mpsc::Receiver<compute::backends::Result<compute::backends::StreamChunk>>,
) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    while let Some(chunk) = rx.recv().await {
        let chunk = chunk?;
        if !chunk.delta.is_empty() {
            write!(stdout, "{}", chunk.delta)?;
            stdout.flush()?;
        }
        if let Some(reason) = &chunk.finish_reason {
            if !reason.is_empty() {
                writeln!(stdout, "\n\n[finish_reason={reason}]")?;
            }
        }
    }
    Ok(())
}
