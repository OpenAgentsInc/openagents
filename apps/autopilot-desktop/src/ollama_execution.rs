use crate::state::job_inbox::JobExecutionParam;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

const LANE_POLL: Duration = Duration::from_millis(120);
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_KEEP_ALIVE: &str = "5m";
const UNLOAD_KEEP_ALIVE: u8 = 0;
const ENV_OLLAMA_BASE_URL: &str = "OPENAGENTS_OLLAMA_BASE_URL";
const ENV_OLLAMA_MODEL: &str = "OPENAGENTS_OLLAMA_MODEL";

#[derive(Clone, Debug)]
pub struct OllamaExecutionConfig {
    pub base_url: String,
    pub configured_model: Option<String>,
    pub refresh_interval: Duration,
}

impl Default for OllamaExecutionConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var(ENV_OLLAMA_BASE_URL)
                .ok()
                .and_then(|value| normalize_optional_text(value.as_str()))
                .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string()),
            configured_model: std::env::var(ENV_OLLAMA_MODEL)
                .ok()
                .and_then(|value| normalize_optional_text(value.as_str())),
            refresh_interval: REFRESH_INTERVAL,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct OllamaExecutionSnapshot {
    pub base_url: String,
    pub reachable: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<OllamaExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
}

impl OllamaExecutionSnapshot {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaExecutionMetrics {
    pub total_duration_ns: Option<u64>,
    pub load_duration_ns: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration_ns: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration_ns: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaGenerateJob {
    pub request_id: String,
    pub prompt: String,
    pub requested_model: Option<String>,
    pub params: Vec<JobExecutionParam>,
}

#[derive(Clone, Debug)]
pub enum OllamaExecutionCommand {
    Refresh,
    WarmConfiguredModel,
    UnloadConfiguredModel,
    Generate(OllamaGenerateJob),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaExecutionStarted {
    pub request_id: String,
    pub model: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaExecutionCompleted {
    pub request_id: String,
    pub model: String,
    pub output: String,
    pub metrics: OllamaExecutionMetrics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaExecutionFailed {
    pub request_id: String,
    pub error: String,
}

#[derive(Clone, Debug)]
pub enum OllamaExecutionUpdate {
    Snapshot(Box<OllamaExecutionSnapshot>),
    Started(OllamaExecutionStarted),
    Completed(OllamaExecutionCompleted),
    Failed(OllamaExecutionFailed),
}

pub struct OllamaExecutionWorker {
    command_tx: Sender<OllamaExecutionCommand>,
    update_rx: Receiver<OllamaExecutionUpdate>,
}

impl OllamaExecutionWorker {
    pub fn spawn() -> Self {
        Self::spawn_with_config(OllamaExecutionConfig::default())
    }

    pub fn spawn_with_config(config: OllamaExecutionConfig) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<OllamaExecutionCommand>();
        let (update_tx, update_rx) = mpsc::channel::<OllamaExecutionUpdate>();

        std::thread::spawn(move || run_ollama_execution_loop(command_rx, update_tx, config));

        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(&self, command: OllamaExecutionCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("Ollama execution worker offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<OllamaExecutionUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

struct OllamaExecutionState {
    config: OllamaExecutionConfig,
    snapshot: OllamaExecutionSnapshot,
    client: Option<Client>,
}

impl OllamaExecutionState {
    fn new(config: OllamaExecutionConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .build()
            .ok();
        let mut snapshot = OllamaExecutionSnapshot {
            base_url: config.base_url.clone(),
            configured_model: config.configured_model.clone(),
            last_action: Some("Ollama execution worker starting".to_string()),
            ..OllamaExecutionSnapshot::default()
        };
        if client.is_none() {
            snapshot.last_error = Some("Failed to initialize Ollama HTTP client".to_string());
        }
        Self {
            config,
            snapshot,
            client,
        }
    }

    fn publish_snapshot(&self, update_tx: &Sender<OllamaExecutionUpdate>) {
        let _ = update_tx.send(OllamaExecutionUpdate::Snapshot(Box::new(
            self.snapshot.clone(),
        )));
    }

    fn maybe_refresh_snapshot(&mut self, update_tx: &Sender<OllamaExecutionUpdate>, force: bool) {
        let should_refresh = force
            || self
                .snapshot
                .refreshed_at
                .is_none_or(|last| last.elapsed() >= self.config.refresh_interval);
        if !should_refresh {
            return;
        }
        self.refresh_snapshot(update_tx);
    }

    fn refresh_snapshot(&mut self, update_tx: &Sender<OllamaExecutionUpdate>) {
        self.snapshot.base_url = self.config.base_url.clone();
        self.snapshot.configured_model = self.config.configured_model.clone();
        self.snapshot.ready_model = None;
        self.snapshot.last_metrics = None;

        let Some(client) = self.client.clone() else {
            self.snapshot.reachable = false;
            self.snapshot.last_error = Some("Ollama HTTP client unavailable".to_string());
            self.snapshot.last_action = Some("Ollama refresh failed".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        };

        let base_url = match validate_local_base_url(self.config.base_url.as_str()) {
            Ok(value) => value,
            Err(error) => {
                self.snapshot.reachable = false;
                self.snapshot.available_models.clear();
                self.snapshot.loaded_models.clear();
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama refresh failed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                return;
            }
        };

        match fetch_available_models(&client, &base_url) {
            Ok(models) => {
                let loaded_models = fetch_loaded_models(&client, &base_url).unwrap_or_default();
                self.snapshot.reachable = true;
                self.snapshot.available_models = models;
                self.snapshot.loaded_models = loaded_models;
                self.snapshot.last_error = None;
                self.snapshot.last_action =
                    Some("Refreshed local Ollama model inventory".to_string());

                if let Some(configured_model) = self.config.configured_model.as_deref() {
                    if self
                        .snapshot
                        .available_models
                        .iter()
                        .any(|candidate| candidate == configured_model)
                    {
                        match validate_model(&client, &base_url, configured_model) {
                            Ok(()) => {
                                self.snapshot.ready_model = Some(configured_model.to_string());
                            }
                            Err(error) => {
                                self.snapshot.last_error = Some(error);
                            }
                        }
                    } else {
                        self.snapshot.last_error = Some(format!(
                            "Configured Ollama model '{}' is not installed locally",
                            configured_model
                        ));
                    }
                } else {
                    self.snapshot.last_error =
                        Some("Set OPENAGENTS_OLLAMA_MODEL to serve kind 5050 jobs".to_string());
                }
            }
            Err(error) => {
                self.snapshot.reachable = false;
                self.snapshot.available_models.clear();
                self.snapshot.loaded_models.clear();
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama refresh failed".to_string());
            }
        }

        self.snapshot.refreshed_at = Some(Instant::now());
        self.publish_snapshot(update_tx);
    }

    fn handle_generate(
        &mut self,
        update_tx: &Sender<OllamaExecutionUpdate>,
        job: OllamaGenerateJob,
    ) {
        let normalized_prompt = match normalize_optional_text(job.prompt.as_str()) {
            Some(value) => normalize_prompt(value.as_str()),
            None => {
                let error = "text-generation request missing prompt/text input".to_string();
                self.fail_job(update_tx, job.request_id.as_str(), error);
                return;
            }
        };

        self.maybe_refresh_snapshot(update_tx, true);
        let Some(client) = self.client.clone() else {
            self.fail_job(
                update_tx,
                job.request_id.as_str(),
                "Ollama HTTP client unavailable".to_string(),
            );
            return;
        };
        let base_url = match validate_local_base_url(self.config.base_url.as_str()) {
            Ok(value) => value,
            Err(error) => {
                self.fail_job(update_tx, job.request_id.as_str(), error);
                return;
            }
        };

        let selected_model = normalize_optional_text(
            job.requested_model
                .as_deref()
                .or(self.snapshot.ready_model.as_deref())
                .or(self.snapshot.configured_model.as_deref())
                .unwrap_or_default(),
        );
        let Some(model) = selected_model else {
            self.fail_job(
                update_tx,
                job.request_id.as_str(),
                "Ollama serving model is not configured".to_string(),
            );
            return;
        };

        if !self
            .snapshot
            .available_models
            .iter()
            .any(|candidate| candidate == &model)
        {
            self.fail_job(
                update_tx,
                job.request_id.as_str(),
                format!(
                    "Requested Ollama model '{}' is not installed locally",
                    model
                ),
            );
            return;
        }

        if let Err(error) = validate_model(&client, &base_url, model.as_str()) {
            self.fail_job(update_tx, job.request_id.as_str(), error);
            return;
        }

        let body = match build_generate_body(
            model.as_str(),
            normalized_prompt.as_str(),
            job.params.as_slice(),
        ) {
            Ok(value) => value,
            Err(error) => {
                self.fail_job(update_tx, job.request_id.as_str(), error);
                return;
            }
        };

        let _ = update_tx.send(OllamaExecutionUpdate::Started(OllamaExecutionStarted {
            request_id: job.request_id.clone(),
            model: model.clone(),
        }));

        match execute_generate(&client, &base_url, body) {
            Ok(result) => {
                self.snapshot.reachable = true;
                self.snapshot.last_error = None;
                self.snapshot.last_request_id = Some(job.request_id.clone());
                self.snapshot.last_metrics = Some(result.metrics.clone());
                self.snapshot.last_action = Some(format!(
                    "Completed local Ollama generation for {}",
                    job.request_id
                ));
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                let _ =
                    update_tx.send(OllamaExecutionUpdate::Completed(OllamaExecutionCompleted {
                        request_id: job.request_id,
                        model,
                        output: result.output,
                        metrics: result.metrics,
                    }));
            }
            Err(error) => {
                self.fail_job(update_tx, job.request_id.as_str(), error);
            }
        }
    }

    fn handle_warm_configured_model(&mut self, update_tx: &Sender<OllamaExecutionUpdate>) {
        self.maybe_refresh_snapshot(update_tx, true);
        let Some(client) = self.client.clone() else {
            self.snapshot.reachable = false;
            self.snapshot.last_error = Some("Ollama HTTP client unavailable".to_string());
            self.snapshot.last_action = Some("Ollama warm-up failed".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        };
        let base_url = match validate_local_base_url(self.config.base_url.as_str()) {
            Ok(value) => value,
            Err(error) => {
                self.snapshot.reachable = false;
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama warm-up failed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                return;
            }
        };
        let Some(model) = self.selected_model_for_runtime() else {
            self.snapshot.last_error =
                Some("Set OPENAGENTS_OLLAMA_MODEL to serve kind 5050 jobs".to_string());
            self.snapshot.last_action = Some("Ollama warm-up skipped".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        };
        if let Err(error) = validate_model(&client, &base_url, model.as_str()) {
            self.snapshot.last_error = Some(error);
            self.snapshot.last_action = Some("Ollama warm-up failed".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        }
        match execute_model_lifecycle_request(
            &client,
            &base_url,
            model.as_str(),
            DEFAULT_KEEP_ALIVE,
        )
        {
            Ok(()) => {
                self.snapshot.reachable = true;
                self.snapshot.ready_model = Some(model.clone());
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some(format!(
                    "Warmed local Ollama model '{}' for provider mode",
                    model
                ));
            }
            Err(error) => {
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama warm-up failed".to_string());
            }
        }
        self.refresh_loaded_models(&client, &base_url);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.publish_snapshot(update_tx);
    }

    fn handle_unload_configured_model(&mut self, update_tx: &Sender<OllamaExecutionUpdate>) {
        self.maybe_refresh_snapshot(update_tx, true);
        let Some(client) = self.client.clone() else {
            self.snapshot.reachable = false;
            self.snapshot.last_error = Some("Ollama HTTP client unavailable".to_string());
            self.snapshot.last_action = Some("Ollama unload failed".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        };
        let base_url = match validate_local_base_url(self.config.base_url.as_str()) {
            Ok(value) => value,
            Err(error) => {
                self.snapshot.reachable = false;
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama unload failed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                return;
            }
        };
        let Some(model) = self.selected_model_for_runtime() else {
            self.snapshot.last_action = Some("Ollama unload skipped".to_string());
            self.snapshot.refreshed_at = Some(Instant::now());
            self.publish_snapshot(update_tx);
            return;
        };
        match execute_model_lifecycle_request(
            &client,
            &base_url,
            model.as_str(),
            UNLOAD_KEEP_ALIVE,
        )
        {
            Ok(()) => {
                self.snapshot.reachable = true;
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some(format!(
                    "Unloaded local Ollama model '{}' after going offline",
                    model
                ));
            }
            Err(error) => {
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Ollama unload failed".to_string());
            }
        }
        self.refresh_loaded_models(&client, &base_url);
        self.snapshot.refreshed_at = Some(Instant::now());
        self.publish_snapshot(update_tx);
    }

    fn selected_model_for_runtime(&self) -> Option<String> {
        normalize_optional_text(
            self.snapshot
                .ready_model
                .as_deref()
                .or(self.snapshot.configured_model.as_deref())
                .unwrap_or_default(),
        )
    }

    fn refresh_loaded_models(&mut self, client: &Client, base_url: &Url) {
        if let Ok(models) = fetch_loaded_models(client, base_url) {
            self.snapshot.loaded_models = models;
        }
    }

    fn fail_job(
        &mut self,
        update_tx: &Sender<OllamaExecutionUpdate>,
        request_id: &str,
        error: String,
    ) {
        self.snapshot.last_request_id = Some(request_id.to_string());
        self.snapshot.last_error = Some(error.clone());
        self.snapshot.last_action = Some(format!("Ollama generation failed for {}", request_id));
        self.snapshot.refreshed_at = Some(Instant::now());
        self.publish_snapshot(update_tx);
        let _ = update_tx.send(OllamaExecutionUpdate::Failed(OllamaExecutionFailed {
            request_id: request_id.to_string(),
            error,
        }));
    }
}

fn run_ollama_execution_loop(
    command_rx: Receiver<OllamaExecutionCommand>,
    update_tx: Sender<OllamaExecutionUpdate>,
    config: OllamaExecutionConfig,
) {
    let mut state = OllamaExecutionState::new(config);
    state.maybe_refresh_snapshot(&update_tx, true);

    loop {
        match command_rx.recv_timeout(LANE_POLL) {
            Ok(OllamaExecutionCommand::Refresh) => state.maybe_refresh_snapshot(&update_tx, true),
            Ok(OllamaExecutionCommand::WarmConfiguredModel) => {
                state.handle_warm_configured_model(&update_tx)
            }
            Ok(OllamaExecutionCommand::UnloadConfiguredModel) => {
                state.handle_unload_configured_model(&update_tx)
            }
            Ok(OllamaExecutionCommand::Generate(job)) => state.handle_generate(&update_tx, job),
            Err(RecvTimeoutError::Timeout) => state.maybe_refresh_snapshot(&update_tx, false),
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn normalize_optional_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_prompt(raw: &str) -> String {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn validate_local_base_url(raw: &str) -> Result<Url, String> {
    let mut url = Url::parse(raw).map_err(|error| format!("invalid Ollama base URL: {error}"))?;
    if url.scheme() != "http" {
        return Err("Ollama base URL must use http".to_string());
    }
    let Some(host) = url.host_str() else {
        return Err("Ollama base URL must include a host".to_string());
    };
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return Err("Ollama base URL must remain local-only".to_string());
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err("Ollama base URL must not include a path".to_string());
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn build_generate_body(
    model: &str,
    prompt: &str,
    params: &[JobExecutionParam],
) -> Result<Value, String> {
    let options = build_generate_options(params)?;
    Ok(json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "keep_alive": DEFAULT_KEEP_ALIVE,
        "options": Value::Object(options),
    }))
}

fn build_generate_options(params: &[JobExecutionParam]) -> Result<Map<String, Value>, String> {
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

fn fetch_available_models(client: &Client, base_url: &Url) -> Result<Vec<String>, String> {
    let url = base_url
        .join("api/tags")
        .map_err(|error| format!("invalid Ollama tags URL: {error}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("Ollama tags request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama tags request failed with status {}",
            response.status()
        ));
    }
    let mut models = response
        .json::<OllamaModelsResponse>()
        .map_err(|error| format!("Ollama tags payload invalid: {error}"))?
        .models
        .into_iter()
        .filter_map(|model| normalize_optional_text(model.name.as_str()))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

fn fetch_loaded_models(client: &Client, base_url: &Url) -> Result<Vec<String>, String> {
    let url = base_url
        .join("api/ps")
        .map_err(|error| format!("invalid Ollama ps URL: {error}"))?;
    let response = client
        .get(url)
        .send()
        .map_err(|error| format!("Ollama ps request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama ps request failed with status {}",
            response.status()
        ));
    }
    let mut models = response
        .json::<OllamaModelsResponse>()
        .map_err(|error| format!("Ollama ps payload invalid: {error}"))?
        .models
        .into_iter()
        .filter_map(|model| normalize_optional_text(model.name.as_str()))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    Ok(models)
}

fn validate_model(client: &Client, base_url: &Url, model: &str) -> Result<(), String> {
    let url = base_url
        .join("api/show")
        .map_err(|error| format!("invalid Ollama show URL: {error}"))?;
    let response = client
        .post(url)
        .json(&json!({ "model": model }))
        .send()
        .map_err(|error| format!("Ollama show request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama model '{}' failed validation with status {}",
            model,
            response.status()
        ));
    }
    Ok(())
}

fn execute_generate(
    client: &Client,
    base_url: &Url,
    body: Value,
) -> Result<OllamaGenerateResult, String> {
    let url = base_url
        .join("api/generate")
        .map_err(|error| format!("invalid Ollama generate URL: {error}"))?;
    let response = client
        .post(url)
        .json(&body)
        .send()
        .map_err(|error| format!("Ollama generate request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama generate request failed with status {}",
            response.status()
        ));
    }
    let payload = response
        .json::<OllamaGenerateResponse>()
        .map_err(|error| format!("Ollama generate payload invalid: {error}"))?;
    let output = normalize_optional_text(payload.response.as_str())
        .ok_or_else(|| "Ollama returned an empty text-generation response".to_string())?;
    Ok(OllamaGenerateResult {
        output,
        metrics: OllamaExecutionMetrics {
            total_duration_ns: payload.total_duration,
            load_duration_ns: payload.load_duration,
            prompt_eval_count: payload.prompt_eval_count,
            prompt_eval_duration_ns: payload.prompt_eval_duration,
            eval_count: payload.eval_count,
            eval_duration_ns: payload.eval_duration,
        },
    })
}

fn execute_model_lifecycle_request<T: serde::Serialize>(
    client: &Client,
    base_url: &Url,
    model: &str,
    keep_alive: T,
) -> Result<(), String> {
    let url = base_url
        .join("api/generate")
        .map_err(|error| format!("invalid Ollama generate URL: {error}"))?;
    let response = client
        .post(url)
        .json(&json!({
            "model": model,
            "prompt": "",
            "stream": false,
            "keep_alive": keep_alive,
        }))
        .send()
        .map_err(|error| format!("Ollama lifecycle request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Ollama lifecycle request failed with status {}",
            response.status()
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct OllamaModelsResponse {
    #[serde(default)]
    models: Vec<OllamaModelSummary>,
}

#[derive(Deserialize)]
struct OllamaModelSummary {
    name: String,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    load_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
}

struct OllamaGenerateResult {
    output: String,
    metrics: OllamaExecutionMetrics,
}

#[cfg(test)]
mod tests {
    use super::{build_generate_body, build_generate_options, validate_local_base_url};
    use crate::state::job_inbox::JobExecutionParam;
    use serde_json::Value;

    #[test]
    fn generate_options_map_supported_dvm_params() {
        let options = build_generate_options(&[
            JobExecutionParam {
                key: "max_tokens".to_string(),
                value: "128".to_string(),
            },
            JobExecutionParam {
                key: "temperature".to_string(),
                value: "0.3".to_string(),
            },
            JobExecutionParam {
                key: "top_k".to_string(),
                value: "40".to_string(),
            },
            JobExecutionParam {
                key: "top_p".to_string(),
                value: "0.92".to_string(),
            },
            JobExecutionParam {
                key: "frequency_penalty".to_string(),
                value: "1.1".to_string(),
            },
            JobExecutionParam {
                key: "presence_penalty".to_string(),
                value: "0.6".to_string(),
            },
            JobExecutionParam {
                key: "seed".to_string(),
                value: "7".to_string(),
            },
            JobExecutionParam {
                key: "stop".to_string(),
                value: "</end>".to_string(),
            },
        ])
        .expect("options should map");

        assert_eq!(options.get("num_predict"), Some(&Value::from(128)));
        assert_eq!(options.get("temperature"), Some(&Value::from(0.3)));
        assert_eq!(options.get("top_k"), Some(&Value::from(40)));
        assert_eq!(options.get("top_p"), Some(&Value::from(0.92)));
        assert_eq!(options.get("frequency_penalty"), Some(&Value::from(1.1)));
        assert_eq!(options.get("presence_penalty"), Some(&Value::from(0.6)));
        assert_eq!(options.get("seed"), Some(&Value::from(7)));
        assert_eq!(options.get("stop"), Some(&serde_json::json!(["</end>"])));
    }

    #[test]
    fn local_base_url_rejects_remote_hosts() {
        assert!(validate_local_base_url("http://127.0.0.1:11434").is_ok());
        assert!(validate_local_base_url("http://localhost:11434").is_ok());
        assert!(validate_local_base_url("https://127.0.0.1:11434").is_err());
        assert!(validate_local_base_url("http://example.com:11434").is_err());
        assert!(validate_local_base_url("http://127.0.0.1:11434/api").is_err());
    }

    #[test]
    fn generate_body_sets_local_execution_defaults() {
        let body = build_generate_body(
            "llama3.2:latest",
            "Write a haiku about rust",
            &[
                JobExecutionParam {
                    key: "top_k".to_string(),
                    value: "16".to_string(),
                },
                JobExecutionParam {
                    key: "top_p".to_string(),
                    value: "0.95".to_string(),
                },
            ],
        )
        .expect("body should build");

        assert_eq!(body.get("model"), Some(&Value::from("llama3.2:latest")));
        assert_eq!(
            body.get("prompt"),
            Some(&Value::from("Write a haiku about rust"))
        );
        assert_eq!(body.get("stream"), Some(&Value::from(false)));
        assert_eq!(body.get("keep_alive"), Some(&Value::from("5m")));
        assert_eq!(body.pointer("/options/top_k"), Some(&Value::from(16)));
        assert_eq!(body.pointer("/options/top_p"), Some(&Value::from(0.95)));
    }
}
