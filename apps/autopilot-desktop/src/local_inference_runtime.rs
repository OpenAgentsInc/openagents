use std::collections::VecDeque;
use std::time::Instant;

use openagents_kernel_core::ids::sha256_prefixed_text;
use serde_json::{Map, Value};

use crate::ollama_execution::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance, OllamaExecutionCommand,
    OllamaExecutionConfig, OllamaExecutionSnapshot, OllamaExecutionUpdate, OllamaExecutionWorker,
    build_generate_options, canonical_options_json, normalize_optional_text, normalize_prompt,
};
use crate::state::job_inbox::JobExecutionParam;
use mox_serve::{
    CpuReferenceTextGenerationService, GenerationLoadState, GenerationOptions, GenerationRequest,
    TextGenerationExecutor,
};

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

impl From<LocalInferenceRuntimeSnapshot> for OllamaExecutionSnapshot {
    fn from(value: LocalInferenceRuntimeSnapshot) -> Self {
        Self {
            base_url: value.base_url,
            reachable: value.reachable,
            configured_model: value.configured_model,
            ready_model: value.ready_model,
            available_models: value.available_models,
            loaded_models: value.loaded_models,
            last_error: value.last_error,
            last_action: value.last_action,
            last_request_id: value.last_request_id,
            last_metrics: value.last_metrics,
            refreshed_at: value.refreshed_at,
        }
    }
}

impl From<OllamaExecutionSnapshot> for LocalInferenceRuntimeSnapshot {
    fn from(value: OllamaExecutionSnapshot) -> Self {
        Self {
            base_url: value.base_url,
            reachable: value.reachable,
            configured_model: value.configured_model,
            ready_model: value.ready_model,
            available_models: value.available_models,
            loaded_models: value.loaded_models,
            last_error: value.last_error,
            last_action: value.last_action,
            last_request_id: value.last_request_id,
            last_metrics: value.last_metrics,
            refreshed_at: value.refreshed_at,
        }
    }
}

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

/// Adapter that keeps the current Ollama worker behind the app-owned runtime seam.
pub struct OllamaRuntimeAdapter {
    worker: OllamaExecutionWorker,
}

impl OllamaRuntimeAdapter {
    #[must_use]
    pub fn spawn() -> Self {
        Self::spawn_with_config(OllamaExecutionConfig::default())
    }

    #[must_use]
    pub fn spawn_with_config(config: OllamaExecutionConfig) -> Self {
        Self {
            worker: OllamaExecutionWorker::spawn_with_config(config),
        }
    }
}

impl LocalInferenceRuntime for OllamaRuntimeAdapter {
    fn enqueue(&mut self, command: LocalInferenceRuntimeCommand) -> Result<(), String> {
        let command = match command {
            LocalInferenceRuntimeCommand::Refresh => OllamaExecutionCommand::Refresh,
            LocalInferenceRuntimeCommand::WarmConfiguredModel => {
                OllamaExecutionCommand::WarmConfiguredModel
            }
            LocalInferenceRuntimeCommand::UnloadConfiguredModel => {
                OllamaExecutionCommand::UnloadConfiguredModel
            }
            LocalInferenceRuntimeCommand::Generate(job) => {
                OllamaExecutionCommand::Generate(crate::ollama_execution::OllamaGenerateJob {
                    request_id: job.request_id,
                    prompt: job.prompt,
                    requested_model: job.requested_model,
                    params: job.params,
                })
            }
        };
        self.worker.enqueue(command)
    }

    fn drain_updates(&mut self) -> Vec<LocalInferenceRuntimeUpdate> {
        self.worker
            .drain_updates()
            .into_iter()
            .map(|update| match update {
                OllamaExecutionUpdate::Snapshot(snapshot) => {
                    LocalInferenceRuntimeUpdate::Snapshot(Box::new((*snapshot).into()))
                }
                OllamaExecutionUpdate::Started(started) => {
                    LocalInferenceRuntimeUpdate::Started(LocalInferenceExecutionStarted {
                        request_id: started.request_id,
                        model: started.model,
                    })
                }
                OllamaExecutionUpdate::Completed(completed) => {
                    LocalInferenceRuntimeUpdate::Completed(LocalInferenceExecutionCompleted {
                        request_id: completed.request_id,
                        model: completed.model,
                        output: completed.output,
                        metrics: completed.metrics,
                        provenance: completed.provenance,
                    })
                }
                OllamaExecutionUpdate::Failed(failed) => {
                    LocalInferenceRuntimeUpdate::Failed(LocalInferenceExecutionFailed {
                        request_id: failed.request_id,
                        error: failed.error,
                    })
                }
            })
            .collect()
    }
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

#[cfg(test)]
mod tests {
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use serde_json::Value;

    use super::{
        LocalInferenceGenerateJob, LocalInferenceRuntime, LocalInferenceRuntimeCommand,
        LocalInferenceRuntimeUpdate, MoxRuntimeAdapter, OllamaRuntimeAdapter,
    };
    use crate::ollama_execution::OllamaExecutionConfig;
    use crate::state::job_inbox::JobExecutionParam;
    use mox_serve::ReferenceWordDecoder;

    fn spawn_mock_ollama_server(
        available_models: &[&str],
        loaded_models: &[&str],
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock ollama server");
        listener
            .set_nonblocking(true)
            .expect("set mock ollama listener nonblocking");
        let address = format!("http://{}", listener.local_addr().expect("local addr"));
        let available_models = available_models
            .iter()
            .map(|model| format!("{{\"name\":\"{model}\"}}"))
            .collect::<Vec<_>>()
            .join(",");
        let loaded_models = loaded_models
            .iter()
            .map(|model| format!("{{\"name\":\"{model}\"}}"))
            .collect::<Vec<_>>()
            .join(",");
        let handle = thread::spawn(move || {
            let mut last_activity = std::time::Instant::now();
            loop {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _addr)) => break stream,
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            if last_activity.elapsed() >= Duration::from_secs(1) {
                                return;
                            }
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        Err(error) => panic!("accept mock ollama request: {error}"),
                    }
                };
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let body = request
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                let (status, response_body) = if request.starts_with("GET /api/tags ") {
                    (200, format!("{{\"models\":[{available_models}]}}"))
                } else if request.starts_with("GET /api/ps ") {
                    (200, format!("{{\"models\":[{loaded_models}]}}"))
                } else if request.starts_with("POST /api/show ") {
                    (200, "{}".to_string())
                } else if request.starts_with("POST /api/generate ") {
                    let parsed: Value =
                        serde_json::from_str(body.as_str()).expect("parse generate body");
                    let model = parsed["model"].as_str().expect("generate model");
                    let keep_alive = parsed["keep_alive"].as_str();
                    if parsed["prompt"].as_str() == Some("") && keep_alive.is_some() {
                        (200, "{\"response\":\"\"}".to_string())
                    } else {
                        assert_eq!(model, "llama3.2:latest");
                        (
                            200,
                            r#"{
                                "response": "hello from adapter",
                                "total_duration": 120000000,
                                "load_duration": 30000000,
                                "prompt_eval_count": 5,
                                "prompt_eval_duration": 40000000,
                                "eval_count": 3,
                                "eval_duration": 80000000
                            }"#
                            .to_string(),
                        )
                    }
                } else {
                    panic!("unexpected mock ollama request {request:?} body={body}");
                };
                let response = format!(
                    "HTTP/1.1 {status} OK\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
                last_activity = std::time::Instant::now();
            }
        });
        (address, handle)
    }

    #[test]
    fn ollama_runtime_adapter_forwards_updates() {
        let (base_url, server_handle) =
            spawn_mock_ollama_server(&["llama3.2:latest"], &["llama3.2:latest"]);
        let mut adapter = OllamaRuntimeAdapter::spawn_with_config(OllamaExecutionConfig {
            base_url,
            configured_model: Some("llama3.2:latest".to_string()),
            refresh_interval: Duration::from_secs(60),
        });

        adapter
            .enqueue(LocalInferenceRuntimeCommand::Refresh)
            .expect("queue refresh");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut snapshot = None;
        while std::time::Instant::now() < deadline {
            let updates = adapter.drain_updates();
            snapshot = updates.into_iter().find_map(|update| match update {
                LocalInferenceRuntimeUpdate::Snapshot(value) => Some(value),
                _ => None,
            });
            if snapshot.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let snapshot = snapshot.expect("runtime snapshot");
        assert_eq!(snapshot.ready_model.as_deref(), Some("llama3.2:latest"));

        adapter
            .enqueue(LocalInferenceRuntimeCommand::Generate(
                LocalInferenceGenerateJob {
                    request_id: "req-adapter-1".to_string(),
                    prompt: "hello".to_string(),
                    requested_model: Some("llama3.2:latest".to_string()),
                    params: vec![JobExecutionParam {
                        key: "max_tokens".to_string(),
                        value: "32".to_string(),
                    }],
                },
            ))
            .expect("queue generate");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut completed = None;
        let mut failure = None;
        let mut last_snapshot = None;
        while std::time::Instant::now() < deadline {
            let updates = adapter.drain_updates();
            for update in updates {
                match update {
                    LocalInferenceRuntimeUpdate::Completed(value) => {
                        completed = Some(value);
                        break;
                    }
                    LocalInferenceRuntimeUpdate::Failed(value) => {
                        failure = Some(value.error);
                    }
                    LocalInferenceRuntimeUpdate::Snapshot(value) => {
                        last_snapshot = Some(format!(
                            "ready_model={:?} last_error={:?} last_action={:?}",
                            value.ready_model, value.last_error, value.last_action
                        ));
                    }
                    LocalInferenceRuntimeUpdate::Started(_) => {}
                }
            }
            if completed.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let completed = completed.unwrap_or_else(|| {
            panic!("completed update failure={failure:?} snapshot={last_snapshot:?}")
        });
        assert_eq!(completed.output, "hello from adapter");
        assert_eq!(completed.provenance.backend, "ollama");

        server_handle.join().expect("mock server thread");
    }

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
}
