use std::collections::VecDeque;
use std::time::Instant;

use crate::state::job_inbox::JobExecutionParam;
use mox_serve::{
    CpuReferenceTextGenerationService, GenerationLoadState, GenerationOptions, GenerationRequest,
    TextGenerationExecutor,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use serde_json::{Map, Value, json};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionMetrics {
    pub total_duration_ns: Option<u64>,
    pub load_duration_ns: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration_ns: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration_ns: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionProvenance {
    pub backend: String,
    pub requested_model: Option<String>,
    pub served_model: String,
    pub normalized_prompt_digest: String,
    pub normalized_options_json: String,
    pub normalized_options_digest: String,
    pub base_url: String,
    pub total_duration_ns: Option<u64>,
    pub load_duration_ns: Option<u64>,
    pub prompt_token_count: Option<u64>,
    pub generated_token_count: Option<u64>,
    pub warm_start: Option<bool>,
}

impl LocalInferenceExecutionProvenance {
    pub fn receipt_payload(&self) -> Value {
        json!({
            "backend": self.backend,
            "requested_model": self.requested_model,
            "served_model": self.served_model,
            "normalized_prompt_digest": self.normalized_prompt_digest,
            "normalized_options": self.normalized_options_value(),
            "normalized_options_digest": self.normalized_options_digest,
            "base_url": self.base_url,
            "metrics": {
                "total_duration_ns": self.total_duration_ns,
                "load_duration_ns": self.load_duration_ns,
                "prompt_token_count": self.prompt_token_count,
                "generated_token_count": self.generated_token_count,
            },
            "warm_start": self.warm_start,
        })
    }

    fn normalized_options_value(&self) -> Value {
        serde_json::from_str(self.normalized_options_json.as_str())
            .unwrap_or_else(|_| Value::String(self.normalized_options_json.clone()))
    }
}

/// Backend-neutral local inference snapshot kept in the app seam.
#[derive(Clone, Debug, Default)]
pub struct LocalInferenceRuntimeSnapshot {
    pub base_url: String,
    pub reachable: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
}

impl LocalInferenceRuntimeSnapshot {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }
}

pub type LocalInferenceExecutionSnapshot = LocalInferenceRuntimeSnapshot;

/// App-owned local inference generation job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceGenerateJob {
    pub request_id: String,
    pub prompt: String,
    pub requested_model: Option<String>,
    pub params: Vec<JobExecutionParam>,
}

/// App-owned local inference runtime commands.
#[derive(Clone, Debug)]
pub enum LocalInferenceRuntimeCommand {
    Refresh,
    WarmConfiguredModel,
    UnloadConfiguredModel,
    Generate(LocalInferenceGenerateJob),
}

/// Local inference execution start notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionStarted {
    pub request_id: String,
    pub model: String,
}

/// Local inference execution completion notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionCompleted {
    pub request_id: String,
    pub model: String,
    pub output: String,
    pub metrics: LocalInferenceExecutionMetrics,
    pub provenance: LocalInferenceExecutionProvenance,
}

/// Local inference execution failure notification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInferenceExecutionFailed {
    pub request_id: String,
    pub error: String,
}

/// App-owned local inference runtime updates.
#[derive(Clone, Debug)]
pub enum LocalInferenceRuntimeUpdate {
    Snapshot(Box<LocalInferenceRuntimeSnapshot>),
    Started(LocalInferenceExecutionStarted),
    Completed(LocalInferenceExecutionCompleted),
    Failed(LocalInferenceExecutionFailed),
}

/// App-owned seam for the local inference runtime slot.
pub trait LocalInferenceRuntime {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String>;
    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate>;
}

pub fn default_local_inference_runtime() -> Result<Box<dyn LocalInferenceRuntime>, String> {
    MoxRuntimeAdapter::new_reference()
        .map(|adapter| Box::new(adapter) as Box<dyn LocalInferenceRuntime>)
}

/// In-process Mox adapter for the app-owned runtime seam.
pub struct MoxRuntimeAdapter {
    service: CpuReferenceTextGenerationService,
    snapshot: LocalInferenceRuntimeSnapshot,
    pending_updates: VecDeque<LocalInferenceRuntimeUpdate>,
}

impl MoxRuntimeAdapter {
    pub fn new_reference() -> Result<Self, String> {
        let mut adapter = Self {
            service: CpuReferenceTextGenerationService::new()
                .map_err(|error| format!("failed to initialize Mox reference runtime: {error}"))?,
            snapshot: LocalInferenceRuntimeSnapshot {
                base_url: String::from("in-process://mox"),
                ..LocalInferenceRuntimeSnapshot::default()
            },
            pending_updates: VecDeque::new(),
        };
        adapter.refresh_snapshot(String::from("Mox local runtime ready"));
        Ok(adapter)
    }

    fn configured_model_id(&self) -> String {
        self.service.model_descriptor().model.model_id.clone()
    }

    fn refresh_snapshot(&mut self, action: String) {
        let configured_model = self.configured_model_id();
        let loaded = self.service.loaded_models();
        let loaded_models = loaded
            .models
            .into_iter()
            .map(|model| model.model)
            .collect::<Vec<_>>();
        self.snapshot.reachable = true;
        self.snapshot.configured_model = Some(configured_model.clone());
        self.snapshot.ready_model = Some(configured_model.clone());
        self.snapshot.available_models = vec![configured_model];
        self.snapshot.loaded_models = loaded_models;
        self.snapshot.last_action = Some(action);
        self.snapshot.last_error = None;
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
    }

    fn push_failure(&mut self, request_id: String, error: impl Into<String>) {
        let error = error.into();
        self.snapshot.last_request_id = Some(request_id.clone());
        self.snapshot.last_error = Some(error.clone());
        self.snapshot.last_action = Some(String::from("Mox generation failed"));
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Failed(
                LocalInferenceExecutionFailed { request_id, error },
            ));
    }

    fn handle_generate(&mut self, job: LocalInferenceGenerateJob) {
        let normalized_prompt = normalize_prompt(job.prompt.as_str());
        let options_map = match build_generate_options(job.params.as_slice()) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let configured_model = self.configured_model_id();
        if let Some(requested_model) = job
            .requested_model
            .as_deref()
            .and_then(normalize_optional_text)
            && requested_model != configured_model
        {
            self.push_failure(
                job.request_id,
                format!(
                    "Mox runtime only has '{}' configured, but '{}' was requested",
                    configured_model, requested_model
                ),
            );
            return;
        }

        let model = configured_model;
        self.snapshot.last_request_id = Some(job.request_id.clone());
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some(format!("Mox generation queued for {}", job.request_id));
        self.snapshot.refreshed_at = Some(Instant::now());
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Snapshot(Box::new(
                self.snapshot.clone(),
            )));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Started(
                LocalInferenceExecutionStarted {
                    request_id: job.request_id.clone(),
                    model: model.clone(),
                },
            ));

        let options = match mox_generation_options(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let request = GenerationRequest::new_text(
            job.request_id.as_str(),
            self.service.model_descriptor().clone(),
            None,
            normalized_prompt.as_str(),
            options,
        );
        let response = match self.service.generate(&request) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(
                    job.request_id,
                    format!("Mox local generation failed: {error}"),
                );
                return;
            }
        };
        let normalized_options_json = match canonical_options_json(&options_map) {
            Ok(value) => value,
            Err(error) => {
                self.push_failure(job.request_id, error);
                return;
            }
        };
        let normalized_options_digest = sha256_prefixed_text(normalized_options_json.as_str());
        let normalized_prompt_digest = sha256_prefixed_text(normalized_prompt.as_str());
        let provenance = LocalInferenceExecutionProvenance {
            backend: String::from("mox"),
            requested_model: job.requested_model.clone(),
            served_model: response.model_id.clone(),
            normalized_prompt_digest,
            normalized_options_json,
            normalized_options_digest,
            base_url: self.snapshot.base_url.clone(),
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_token_count: u64::try_from(response.usage.input_tokens).ok(),
            generated_token_count: u64::try_from(response.usage.output_tokens).ok(),
            warm_start: response
                .provenance
                .as_ref()
                .and_then(|value| match value.load_state {
                    GenerationLoadState::Cold => Some(false),
                    GenerationLoadState::Warm => Some(true),
                }),
        };
        let metrics = LocalInferenceExecutionMetrics {
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_eval_count: response
                .metrics
                .prompt_eval_count
                .and_then(|value| u64::try_from(value).ok()),
            prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
            eval_count: response
                .metrics
                .eval_count
                .and_then(|value| u64::try_from(value).ok()),
            eval_duration_ns: response.metrics.eval_duration_ns,
        };
        self.snapshot.last_metrics = Some(metrics.clone());
        self.snapshot.last_action =
            Some(format!("Mox generation completed for {}", job.request_id));
        self.snapshot.last_error = None;
        self.snapshot.refreshed_at = Some(Instant::now());
        self.refresh_snapshot(String::from("Mox local runtime refreshed after generation"));
        self.pending_updates
            .push_back(LocalInferenceRuntimeUpdate::Completed(
                LocalInferenceExecutionCompleted {
                    request_id: job.request_id,
                    model,
                    output: response.output.text,
                    metrics,
                    provenance,
                },
            ));
    }
}

impl LocalInferenceRuntime for MoxRuntimeAdapter {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String> {
        match command {
            LocalInferenceRuntimeCommand::Refresh => {
                self.refresh_snapshot(String::from("Mox local runtime refreshed"));
                Ok(())
            }
            LocalInferenceRuntimeCommand::WarmConfiguredModel => {
                let model_id = self.configured_model_id();
                self.service
                    .warm_model(model_id.as_str(), 300_000)
                    .map_err(|error| format!("failed to warm Mox model '{model_id}': {error}"))?;
                self.refresh_snapshot(format!("Mox model '{}' warmed", model_id));
                Ok(())
            }
            LocalInferenceRuntimeCommand::UnloadConfiguredModel => {
                let model_id = self.configured_model_id();
                self.service
                    .unload_model(model_id.as_str())
                    .map_err(|error| format!("failed to unload Mox model '{model_id}': {error}"))?;
                self.refresh_snapshot(format!("Mox model '{}' unloaded", model_id));
                Ok(())
            }
            LocalInferenceRuntimeCommand::Generate(job) => {
                self.handle_generate(job);
                Ok(())
            }
        }
    }

    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate> {
        self.pending_updates.drain(..).collect()
    }
}

fn mox_generation_options(options: &Map<String, Value>) -> Result<GenerationOptions, String> {
    let max_output_tokens = options
        .get("num_predict")
        .and_then(Value::as_i64)
        .map(usize::try_from)
        .transpose()
        .map_err(|_| "num_predict must be non-negative".to_string())?
        .unwrap_or(64);
    let sampled = options.contains_key("temperature")
        || options.contains_key("top_k")
        || options.contains_key("top_p")
        || options.contains_key("seed");
    let mut generation_options = if sampled {
        GenerationOptions::sample(max_output_tokens)
    } else {
        GenerationOptions::greedy(max_output_tokens)
    };
    generation_options.temperature = options
        .get("temperature")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.top_k = options
        .get("top_k")
        .and_then(Value::as_i64)
        .map(usize::try_from)
        .transpose()
        .map_err(|_| "top_k must be non-negative".to_string())?;
    generation_options.top_p = options
        .get("top_p")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.presence_penalty = options
        .get("presence_penalty")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.frequency_penalty = options
        .get("frequency_penalty")
        .and_then(Value::as_f64)
        .map(|value| value as f32);
    generation_options.seed = options
        .get("seed")
        .and_then(Value::as_i64)
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "seed must be non-negative".to_string())?;
    generation_options.stop_sequences = options
        .get("stop")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(generation_options)
}

pub fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn normalize_prompt(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

pub fn canonical_options_json(options: &Map<String, Value>) -> Result<String, String> {
    let canonical = options
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    serde_json::to_string(&canonical)
        .map_err(|error| format!("failed to encode normalized local inference options: {error}"))
}

pub fn build_generate_options(params: &[JobExecutionParam]) -> Result<Map<String, Value>, String> {
    let mut options = Map::<String, Value>::new();
    for param in params {
        let key = param.key.as_str();
        let value = param.value.as_str();
        match key {
            "max_tokens" => {
                options.insert("num_predict".to_string(), json!(parse_i64(key, value)?));
            }
            "temperature" => {
                options.insert("temperature".to_string(), json!(parse_f64(key, value)?));
            }
            "top_k" => {
                options.insert("top_k".to_string(), json!(parse_i64(key, value)?));
            }
            "top_p" => {
                options.insert("top_p".to_string(), json!(parse_f64(key, value)?));
            }
            "frequency_penalty" => {
                options.insert(
                    "frequency_penalty".to_string(),
                    json!(parse_f64(key, value)?),
                );
            }
            "presence_penalty" => {
                options.insert(
                    "presence_penalty".to_string(),
                    json!(parse_f64(key, value)?),
                );
            }
            "seed" => {
                options.insert("seed".to_string(), json!(parse_i64(key, value)?));
            }
            "stop" => {
                let Some(stop) = normalize_optional_text(value) else {
                    continue;
                };
                options.insert("stop".to_string(), json!([stop]));
            }
            _ => {}
        }
    }
    Ok(options)
}

fn parse_i64(key: &str, raw: &str) -> Result<i64, String> {
    raw.trim()
        .parse::<i64>()
        .map_err(|error| format!("invalid {} value '{}': {}", key, raw.trim(), error))
}

fn parse_f64(key: &str, raw: &str) -> Result<f64, String> {
    raw.trim()
        .parse::<f64>()
        .map_err(|error| format!("invalid {} value '{}': {}", key, raw.trim(), error))
}

#[cfg(test)]
mod tests {
    use super::{
        LocalInferenceGenerateJob, LocalInferenceRuntime, LocalInferenceRuntimeCommand,
        LocalInferenceRuntimeUpdate, MoxRuntimeAdapter, default_local_inference_runtime,
    };
    use crate::state::job_inbox::JobExecutionParam;
    use mox_serve::ReferenceWordDecoder;

    #[test]
    fn mox_runtime_adapter_refreshes_and_generates() {
        let mut adapter = MoxRuntimeAdapter::new_reference().expect("mox adapter");
        adapter
            .enqueue(LocalInferenceRuntimeCommand::Refresh)
            .expect("refresh");
        let snapshot = adapter
            .drain_updates()
            .into_iter()
            .find_map(|update| match update {
                LocalInferenceRuntimeUpdate::Snapshot(value) => Some(value),
                _ => None,
            })
            .expect("snapshot");
        assert_eq!(
            snapshot.ready_model.as_deref(),
            Some(ReferenceWordDecoder::MODEL_ID)
        );

        adapter
            .enqueue(LocalInferenceRuntimeCommand::Generate(
                LocalInferenceGenerateJob {
                    request_id: "req-mox-1".to_string(),
                    prompt: "hello".to_string(),
                    requested_model: Some(ReferenceWordDecoder::MODEL_ID.to_string()),
                    params: vec![JobExecutionParam {
                        key: "max_tokens".to_string(),
                        value: "2".to_string(),
                    }],
                },
            ))
            .expect("generate");
        let mut completed = None;
        for update in adapter.drain_updates() {
            if let LocalInferenceRuntimeUpdate::Completed(value) = update {
                completed = Some(value);
            }
        }
        let completed = completed.expect("completed update");
        assert_eq!(completed.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(completed.provenance.backend, "mox");
        assert!(!completed.output.is_empty());
    }

    #[test]
    fn default_local_inference_runtime_uses_mox_reference_runtime() {
        let mut runtime = default_local_inference_runtime().expect("default local runtime");
        runtime
            .enqueue(LocalInferenceRuntimeCommand::Refresh)
            .expect("refresh");
        let snapshot = runtime
            .drain_updates()
            .into_iter()
            .find_map(|update| match update {
                LocalInferenceRuntimeUpdate::Snapshot(value) => Some(value),
                _ => None,
            })
            .expect("snapshot");
        assert_eq!(snapshot.base_url, "in-process://mox");
        assert_eq!(
            snapshot.ready_model.as_deref(),
            Some(ReferenceWordDecoder::MODEL_ID)
        );
    }
}
