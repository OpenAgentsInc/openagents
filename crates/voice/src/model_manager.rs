use hf_hub::api::sync::Api;
use std::path::PathBuf;

/// HuggingFace repo containing whisper.cpp models
const HF_REPO: &str = "ggerganov/whisper.cpp";

/// Get the local cache directory for Whisper models
fn get_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = dirs::data_local_dir()
        .ok_or("Could not find local data directory")?
        .join("openagents")
        .join("whisper");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;

    Ok(cache_dir)
}

/// Ensure the specified model is downloaded and return its path
///
/// # Arguments
/// * `model` - Model name like "base.en", "small", "medium", etc.
///
/// # Returns
/// Path to the downloaded model file
pub fn ensure_model(model: &str) -> Result<PathBuf, String> {
    let cache_dir = get_cache_dir()?;
    let model_filename = format!("ggml-{}.bin", model);
    let model_path = cache_dir.join(&model_filename);

    // If model already exists, return the path
    if model_path.exists() {
        tracing::info!("Using cached model: {:?}", model_path);
        return Ok(model_path);
    }

    tracing::info!(
        "Downloading model {} from HuggingFace (this may take a minute)...",
        model
    );

    // Download from HuggingFace using sync API
    let api = Api::new().map_err(|e| format!("Failed to create HF API: {}", e))?;

    let repo = api.model(HF_REPO.to_string());

    let downloaded_path = repo
        .get(&model_filename)
        .map_err(|e| format!("Failed to download model: {}", e))?;

    // Copy to our cache directory
    std::fs::copy(&downloaded_path, &model_path)
        .map_err(|e| format!("Failed to copy model to cache: {}", e))?;

    tracing::info!("Model downloaded to: {:?}", model_path);
    Ok(model_path)
}

/// List available models
pub fn available_models() -> &'static [&'static str] {
    &[
        "tiny",
        "tiny.en",
        "base",
        "base.en",
        "small",
        "small.en",
        "medium",
        "medium.en",
        "large-v1",
        "large-v2",
        "large-v3",
    ]
}

/// Get approximate model size in MB
pub fn model_size_mb(model: &str) -> u64 {
    match model {
        "tiny" | "tiny.en" => 40,
        "base" | "base.en" => 140,
        "small" | "small.en" => 466,
        "medium" | "medium.en" => 1500,
        "large-v1" | "large-v2" | "large-v3" => 2900,
        _ => 0,
    }
}
