use crate::device::MlDevice;
use crate::error::{MlError, Result};
use crate::model::{LoadedModel, ModelSource};
use crate::sampling::GenerationConfig;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket};

#[derive(Debug, Clone)]
pub struct DvmConfig {
    pub supported_kinds: Vec<u16>,
    pub min_price_per_token: u64,
    pub max_concurrent_jobs: usize,
    pub relays: Vec<String>,
}

impl Default for DvmConfig {
    fn default() -> Self {
        Self {
            supported_kinds: vec![5050],
            min_price_per_token: 1,
            max_concurrent_jobs: 1,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.info".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct JobRequest {
    pub input: String,
    pub model: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<usize>,
    pub repetition_penalty: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HandlerInfo {
    pub name: String,
    pub about: String,
    pub supported_kinds: Vec<u16>,
    pub supported_models: Vec<String>,
}

#[derive(Debug, Clone)]
struct ActiveJob {
    request_event: nostr::Event,
    started_at: web_time::Instant,
}

#[derive(Clone)]
pub struct BrowserDvmService {
    secret_key: [u8; 32],
    pubkey: String,
    model_id: String,
    model: Arc<Mutex<LoadedModel>>,
    config: DvmConfig,
    relays: Vec<String>,
    active_jobs: Arc<RwLock<HashMap<String, ActiveJob>>>,
}

impl BrowserDvmService {
    pub fn new(secret_key: [u8; 32], model: LoadedModel, config: DvmConfig) -> Result<Self> {
        let pubkey = nostr::get_public_key_hex(&secret_key)?;
        let model_id = model.id.clone();
        Ok(Self {
            secret_key,
            pubkey,
            model_id,
            model: Arc::new(Mutex::new(model)),
            relays: config.relays.clone(),
            config,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    pub fn start(&self) {
        let service = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = service.run().await {
                web_sys::console::error_1(&JsValue::from_str(&err.to_string()));
            }
        });
    }

    async fn run(&self) -> Result<()> {
        let (client, mut rx) = BrowserNostrClient::connect(&self.relays).await?;
        let client = Arc::new(client);

        self.publish_handler_info(&client).await?;

        let filter = Filter {
            kinds: self.config.supported_kinds.clone(),
            p: vec![self.pubkey.clone()],
            since: Some(current_time_secs()),
        };
        client.subscribe(&filter)?;

        while let Some(event) = rx.next().await {
            if let Err(err) = self.handle_event(client.clone(), event).await {
                web_sys::console::error_1(&JsValue::from_str(&err.to_string()));
            }
        }

        Ok(())
    }

    async fn handle_event(&self, client: Arc<BrowserNostrClient>, event: nostr::Event) -> Result<()> {
        if !self.should_handle(&event) {
            return Ok(());
        }

        if self.active_jobs.read().len() >= self.config.max_concurrent_jobs {
            self.publish_feedback(&client, &event, "error", "at capacity")
                .await?;
            return Ok(());
        }

        let mut request = self.parse_job_request(&event)?;
        if request.model.is_empty() {
            request.model = self.model_id.clone();
        }

        if request.model != self.model_id {
            self.publish_feedback(&client, &event, "error", "unsupported model")
                .await?;
            return Ok(());
        }

        self.publish_feedback(&client, &event, "processing", "").await?;

        self.active_jobs.write().insert(
            event.id.clone(),
            ActiveJob {
                request_event: event.clone(),
                started_at: web_time::Instant::now(),
            },
        );

        let service = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            service.process_job(client, event, request).await;
        });

        Ok(())
    }

    async fn process_job(
        &self,
        client: Arc<BrowserNostrClient>,
        event: nostr::Event,
        request: JobRequest,
    ) {
        let result = {
            let mut model = self.model.lock();
            let config = self.build_generation_config(&request);
            model.generate(&request.input, &config, None)
        };

        match result {
            Ok(outcome) => {
                let _ = self.publish_result(&client, &event, &outcome.text).await;
            }
            Err(err) => {
                let _ = self
                    .publish_feedback(&client, &event, "error", &err.to_string())
                    .await;
            }
        }

        self.active_jobs.write().remove(&event.id);
    }

    fn should_handle(&self, event: &nostr::Event) -> bool {
        if !self.config.supported_kinds.contains(&event.kind) {
            return false;
        }

        event.tags.iter().any(|tag| {
            tag.get(0).map(String::as_str) == Some("p") && tag.get(1) == Some(&self.pubkey)
        })
    }

    fn parse_job_request(&self, event: &nostr::Event) -> Result<JobRequest> {
        let mut input = String::new();
        let mut model = String::new();
        let mut max_tokens = None;
        let mut temperature = None;
        let mut top_p = None;
        let mut top_k = None;
        let mut repetition_penalty = None;

        for tag in &event.tags {
            if tag.get(0).map(String::as_str) == Some("i") {
                if let Some(value) = tag.get(1) {
                    input = value.clone();
                }
            }

            if tag.get(0).map(String::as_str) == Some("param") {
                let key = tag.get(1).map(|v| v.as_str()).unwrap_or_default();
                let value = tag.get(2).map(|v| v.as_str()).unwrap_or_default();
                match key {
                    "model" => model = value.to_string(),
                    "max_tokens" => max_tokens = value.parse().ok(),
                    "temperature" => temperature = value.parse().ok(),
                    "top_p" => top_p = value.parse().ok(),
                    "top_k" => top_k = value.parse().ok(),
                    "repetition_penalty" => repetition_penalty = value.parse().ok(),
                    _ => {}
                }
            }
        }

        if input.is_empty() {
            return Err(MlError::InvalidConfig("missing input".to_string()));
        }

        Ok(JobRequest {
            input,
            model,
            max_tokens,
            temperature,
            top_p,
            top_k,
            repetition_penalty,
        })
    }

    fn build_generation_config(&self, request: &JobRequest) -> GenerationConfig {
        let mut config = GenerationConfig::default();
        if let Some(max_tokens) = request.max_tokens {
            config.max_new_tokens = max_tokens;
        }
        if let Some(temp) = request.temperature {
            config.temperature = temp;
        }
        if let Some(top_p) = request.top_p {
            config.top_p = top_p;
        }
        if let Some(top_k) = request.top_k {
            config.top_k = top_k;
        }
        if let Some(repetition_penalty) = request.repetition_penalty {
            config.repetition_penalty = repetition_penalty;
        }
        config
    }

    async fn publish_feedback(
        &self,
        client: &BrowserNostrClient,
        request: &nostr::Event,
        status: &str,
        message: &str,
    ) -> Result<()> {
        let template = nostr::EventTemplate {
            created_at: current_time_secs(),
            kind: 7000,
            tags: vec![
                vec!["e".to_string(), request.id.clone()],
                vec!["p".to_string(), request.pubkey.clone()],
                vec!["status".to_string(), status.to_string()],
            ],
            content: message.to_string(),
        };
        let event = nostr::finalize_event(&template, &self.secret_key)?;
        client.publish(&event)?;
        Ok(())
    }

    async fn publish_result(
        &self,
        client: &BrowserNostrClient,
        request: &nostr::Event,
        result: &str,
    ) -> Result<()> {
        let request_json = serde_json::to_string(request).unwrap_or_default();
        let template = nostr::EventTemplate {
            created_at: current_time_secs(),
            kind: 6050,
            tags: vec![
                vec!["e".to_string(), request.id.clone()],
                vec!["p".to_string(), request.pubkey.clone()],
                vec!["request".to_string(), request_json],
            ],
            content: result.to_string(),
        };
        let event = nostr::finalize_event(&template, &self.secret_key)?;
        client.publish(&event)?;
        Ok(())
    }

    async fn publish_handler_info(&self, client: &BrowserNostrClient) -> Result<()> {
        let info = HandlerInfo {
            name: "WebGPU Inference Provider".to_string(),
            about: "Browser-based LLM inference via Candle".to_string(),
            supported_kinds: self.config.supported_kinds.clone(),
            supported_models: vec![self.model_id.clone()],
        };
        let content = serde_json::to_string(&info)?;
        let template = nostr::EventTemplate {
            created_at: current_time_secs(),
            kind: 31990,
            tags: vec![
                vec!["d".to_string(), "webgpu-inference".to_string()],
                vec!["k".to_string(), "5050".to_string()],
            ],
            content,
        };
        let event = nostr::finalize_event(&template, &self.secret_key)?;
        client.publish(&event)?;
        Ok(())
    }
}

#[derive(Serialize)]
struct Filter {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    kinds: Vec<u16>,
    #[serde(rename = "#p", skip_serializing_if = "Vec::is_empty")]
    p: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<u64>,
}

struct BrowserNostrClient {
    sockets: Vec<WebSocket>,
    _message_handlers: Vec<Closure<dyn FnMut(MessageEvent)>>,
    _open_handlers: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}

impl BrowserNostrClient {
    async fn connect(relays: &[String]) -> Result<(Self, UnboundedReceiver<nostr::Event>)> {
        let (tx, rx) = unbounded();
        let mut sockets = Vec::new();
        let mut message_handlers = Vec::new();
        let mut open_handlers = Vec::new();
        let mut open_futures = Vec::new();

        for relay in relays {
            let ws = WebSocket::new(relay).map_err(|e| MlError::Network(format!("ws error: {e:?}")))?;
            let (open_tx, open_rx) = futures::channel::oneshot::channel();

            let onopen = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                let _ = open_tx.send(());
            }) as Box<dyn FnMut(_)>);
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            open_handlers.push(onopen);

            let tx_clone: UnboundedSender<nostr::Event> = tx.clone();
            let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
                if let Some(text) = event.data().as_string() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(event) = parse_nostr_event(value) {
                            let _ = tx_clone.unbounded_send(event);
                        }
                    }
                }
            }) as Box<dyn FnMut(_)>);
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            message_handlers.push(onmessage);

            sockets.push(ws);
            open_futures.push(open_rx);
        }

        futures::future::join_all(open_futures).await;

        Ok((
            Self {
                sockets,
                _message_handlers: message_handlers,
                _open_handlers: open_handlers,
            },
            rx,
        ))
    }

    fn subscribe(&self, filter: &Filter) -> Result<String> {
        let sub_id = uuid::Uuid::new_v4().to_string();
        let msg = serde_json::to_string(&serde_json::json!(["REQ", sub_id, filter]))?;
        for ws in &self.sockets {
            ws.send_with_str(&msg)
                .map_err(|e| MlError::Network(format!("ws send error: {e:?}")))?;
        }
        Ok(sub_id)
    }

    fn publish(&self, event: &nostr::Event) -> Result<()> {
        let msg = serde_json::to_string(&serde_json::json!(["EVENT", event]))?;
        for ws in &self.sockets {
            ws.send_with_str(&msg)
                .map_err(|e| MlError::Network(format!("ws send error: {e:?}")))?;
        }
        Ok(())
    }
}

fn parse_nostr_event(value: serde_json::Value) -> Option<nostr::Event> {
    let arr = value.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    let kind = arr.get(0)?.as_str()?;
    if kind != "EVENT" {
        return None;
    }
    serde_json::from_value(arr.get(2)?.clone()).ok()
}

fn current_time_secs() -> u64 {
    let now = web_time::SystemTime::now();
    now.duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_secret_key(value: &str) -> Result<[u8; 32]> {
    if value.starts_with("nsec") {
        match nostr::decode(value)? {
            nostr::Nip19Entity::Secret(sk) => Ok(sk),
            _ => Err(MlError::InvalidConfig("invalid nsec key".to_string())),
        }
    } else {
        let bytes = hex::decode(value).map_err(|e| MlError::InvalidConfig(e.to_string()))?;
        let sk: [u8; 32] = bytes
            .try_into()
            .map_err(|_| MlError::InvalidConfig("invalid key length".to_string()))?;
        Ok(sk)
    }
}

#[wasm_bindgen]
pub struct BrowserDvm {
    service: BrowserDvmService,
}

#[wasm_bindgen]
impl BrowserDvm {
    #[wasm_bindgen(constructor)]
    pub async fn new(
        private_key: String,
        model_url: String,
        tokenizer_url: Option<String>,
    ) -> std::result::Result<BrowserDvm, JsValue> {
        let secret_key = parse_secret_key(&private_key).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let device = MlDevice::best_available()
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let tokenizer_url = tokenizer_url.unwrap_or_else(|| derive_tokenizer_url(&model_url));
        let source = ModelSource::llama2c_gguf("llama2c", model_url, tokenizer_url);
        let model = LoadedModel::load(&source, &device)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let service = BrowserDvmService::new(secret_key, model, DvmConfig::default())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(BrowserDvm { service })
    }

    pub fn start(&self) {
        self.service.start();
    }

    pub fn pubkey(&self) -> String {
        self.service.pubkey().to_string()
    }
}

fn derive_tokenizer_url(model_url: &str) -> String {
    if let Some((base, _)) = model_url.rsplit_once('/') {
        format!("{base}/tokenizer.json")
    } else {
        "tokenizer.json".to_string()
    }
}
