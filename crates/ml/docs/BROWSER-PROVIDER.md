# Browser DVM Provider

Serve NIP-90 inference jobs from a browser tab using WebGPU.

## Overview

The `BrowserDvmService` enables users to contribute compute by simply having a browser tab open. It connects to Nostr relays, listens for NIP-90 job requests, runs inference via `WebGpuProvider`, and publishes results.

```
┌─────────────────────────────────────────────────────────────┐
│                      Browser Tab                            │
│                                                             │
│  ┌─────────────────┐     ┌──────────────────┐              │
│  │ BrowserDvmService│────▶│  WebGpuProvider  │              │
│  └─────────────────┘     └──────────────────┘              │
│          │                        │                         │
│          │                        ▼                         │
│          │               ┌──────────────────┐              │
│          │               │   WebGpuDevice   │              │
│          │               └──────────────────┘              │
│          │                        │                         │
│          │                        ▼                         │
│          │               ┌──────────────────┐              │
│          │               │   WGSL Kernels   │              │
│          │               └──────────────────┘              │
│          │                                                  │
│          ▼                                                  │
│  ┌─────────────────┐                                       │
│  │  Nostr Client   │                                       │
│  └─────────────────┘                                       │
│          │                                                  │
└──────────┼──────────────────────────────────────────────────┘
           │
           ▼
    ┌─────────────┐
    │ Nostr Relays │
    └─────────────┘
```

## NIP-90 Protocol

### Event Kinds

| Kind | Name | Purpose |
|------|------|---------|
| 5050 | Text Generation Request | Job request for LLM inference |
| 5051 | Text Embedding Request | Job request for embeddings |
| 6050 | Text Generation Result | Successful result |
| 6051 | Text Embedding Result | Successful result |
| 7000 | Job Feedback | Status updates, errors |

### Job Request (kind 5050)

```json
{
  "kind": 5050,
  "content": "",
  "tags": [
    ["i", "What is the capital of France?", "text"],
    ["param", "model", "llama-7b"],
    ["param", "max_tokens", "256"],
    ["param", "temperature", "0.7"],
    ["relays", "wss://relay.damus.io", "wss://relay.nostr.info"],
    ["p", "<provider_pubkey>"]
  ]
}
```

### Job Feedback (kind 7000)

```json
{
  "kind": 7000,
  "content": "processing",
  "tags": [
    ["e", "<job_request_id>"],
    ["p", "<customer_pubkey>"],
    ["status", "processing"],
    ["amount", "1000", "msats"]
  ]
}
```

### Job Result (kind 6050)

```json
{
  "kind": 6050,
  "content": "The capital of France is Paris.",
  "tags": [
    ["e", "<job_request_id>"],
    ["p", "<customer_pubkey>"],
    ["request", "<original_request_json>"]
  ]
}
```

## BrowserDvmService

```rust
// crates/ml/src/provider/browser_dvm.rs

use nostr_core::{Event, Filter, Keys, Kind, Tag};
use std::sync::Arc;

pub struct BrowserDvmService {
    /// Provider identity
    keys: Keys,
    /// Compute provider
    provider: Arc<WebGpuProvider>,
    /// Connected relays
    relays: Vec<String>,
    /// Configuration
    config: DvmConfig,
    /// Active job tracking
    active_jobs: parking_lot::RwLock<HashMap<String, ActiveJob>>,
}

pub struct DvmConfig {
    /// Supported job kinds
    pub supported_kinds: Vec<u16>,
    /// Minimum price per token (msats)
    pub min_price_per_token: u64,
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: usize,
    /// Relay URLs
    pub relays: Vec<String>,
}

impl Default for DvmConfig {
    fn default() -> Self {
        Self {
            supported_kinds: vec![5050],  // Text generation
            min_price_per_token: 1,
            max_concurrent_jobs: 1,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.info".to_string(),
            ],
        }
    }
}

struct ActiveJob {
    request_event: Event,
    job_id: String,
    started_at: web_time::Instant,
}

impl BrowserDvmService {
    /// Create new DVM service
    pub fn new(
        keys: Keys,
        provider: Arc<WebGpuProvider>,
        config: DvmConfig,
    ) -> Self {
        Self {
            keys,
            provider,
            relays: config.relays.clone(),
            config,
            active_jobs: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Start the DVM service (browser entry point)
    #[cfg(target_arch = "wasm32")]
    pub fn start(&self) {
        let service = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            service.run().await;
        });
    }

    /// Main run loop
    pub async fn run(&self) {
        // Publish handler advertisement (NIP-89)
        self.publish_handler_info().await;

        // Connect to relays and subscribe to jobs
        let client = NostrClient::new(&self.relays).await;

        // Subscribe to job requests for our pubkey
        let filter = Filter::new()
            .kinds(self.config.supported_kinds.iter().map(|k| Kind::Custom(*k)).collect())
            .pubkey(self.keys.public_key())
            .since(web_time::Instant::now());

        let mut subscription = client.subscribe(vec![filter]).await;

        // Process incoming events
        while let Some(event) = subscription.next().await {
            if let Err(e) = self.handle_event(&client, event).await {
                log::error!("Error handling event: {}", e);
            }
        }
    }

    /// Handle incoming job request
    async fn handle_event(&self, client: &NostrClient, event: Event) -> Result<(), DvmError> {
        // Validate event
        if !self.should_handle(&event) {
            return Ok(());
        }

        // Check concurrent job limit
        if self.active_jobs.read().len() >= self.config.max_concurrent_jobs {
            self.publish_feedback(client, &event, "error", "Provider at capacity").await?;
            return Ok(());
        }

        // Parse job request
        let request = self.parse_job_request(&event)?;

        // Send "processing" feedback
        self.publish_feedback(client, &event, "processing", "").await?;

        // Submit to WebGpuProvider
        let job_id = self.provider.submit(ComputeRequest {
            model: request.model.clone(),
            prompt: request.input.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: true,
        })?;

        // Track active job
        self.active_jobs.write().insert(event.id.to_string(), ActiveJob {
            request_event: event.clone(),
            job_id: job_id.clone(),
            started_at: web_time::Instant::now(),
        });

        // Spawn job monitor
        self.spawn_job_monitor(client.clone(), event, job_id);

        Ok(())
    }

    /// Monitor job and publish result
    fn spawn_job_monitor(&self, client: NostrClient, request_event: Event, job_id: String) {
        let provider = Arc::clone(&self.provider);
        let keys = self.keys.clone();
        let active_jobs = self.active_jobs.clone();

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            Self::monitor_job(provider, client, keys, active_jobs, request_event, job_id).await;
        });

        #[cfg(not(target_arch = "wasm32"))]
        tokio::spawn(async move {
            Self::monitor_job(provider, client, keys, active_jobs, request_event, job_id).await;
        });
    }

    async fn monitor_job(
        provider: Arc<WebGpuProvider>,
        client: NostrClient,
        keys: Keys,
        active_jobs: parking_lot::RwLock<HashMap<String, ActiveJob>>,
        request_event: Event,
        job_id: String,
    ) {
        let mut result_text = String::new();

        // Poll for streaming output
        loop {
            match provider.poll_stream(&job_id) {
                Ok(Some(chunk)) => {
                    result_text.push_str(&chunk.text);

                    if chunk.is_final {
                        break;
                    }
                }
                Ok(None) => {
                    // Check job state
                    match provider.get_job(&job_id) {
                        Some(JobState::Completed { result }) => {
                            result_text = result.text;
                            break;
                        }
                        Some(JobState::Failed { error }) => {
                            // Publish error feedback
                            Self::publish_error(&client, &keys, &request_event, &error).await;
                            active_jobs.write().remove(&request_event.id.to_string());
                            return;
                        }
                        Some(JobState::Cancelled) => {
                            active_jobs.write().remove(&request_event.id.to_string());
                            return;
                        }
                        _ => {
                            // Still running, wait a bit
                            #[cfg(target_arch = "wasm32")]
                            gloo_timers::future::TimeoutFuture::new(100).await;
                            #[cfg(not(target_arch = "wasm32"))]
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                Err(e) => {
                    Self::publish_error(&client, &keys, &request_event, &e.to_string()).await;
                    active_jobs.write().remove(&request_event.id.to_string());
                    return;
                }
            }
        }

        // Publish result
        Self::publish_result(&client, &keys, &request_event, &result_text).await;
        active_jobs.write().remove(&request_event.id.to_string());
    }

    /// Check if we should handle this event
    fn should_handle(&self, event: &Event) -> bool {
        // Check kind
        let kind = event.kind.as_u16();
        if !self.config.supported_kinds.contains(&kind) {
            return false;
        }

        // Check if addressed to us
        let our_pubkey = self.keys.public_key().to_string();
        event.tags.iter().any(|tag| {
            matches!(tag, Tag::PubKey(pk, _) if pk.to_string() == our_pubkey)
        })
    }

    /// Parse job request from event
    fn parse_job_request(&self, event: &Event) -> Result<JobRequest, DvmError> {
        let mut input = String::new();
        let mut model = String::new();
        let mut max_tokens = None;
        let mut temperature = None;

        for tag in &event.tags {
            match tag {
                Tag::Input(value, mime) => {
                    if mime == "text" || mime.is_empty() {
                        input = value.clone();
                    }
                }
                Tag::Param(key, value) => match key.as_str() {
                    "model" => model = value.clone(),
                    "max_tokens" => max_tokens = value.parse().ok(),
                    "temperature" => temperature = value.parse().ok(),
                    _ => {}
                },
                _ => {}
            }
        }

        if input.is_empty() {
            return Err(DvmError::InvalidRequest("Missing input".to_string()));
        }

        Ok(JobRequest {
            input,
            model,
            max_tokens,
            temperature,
        })
    }

    /// Publish job feedback event
    async fn publish_feedback(
        &self,
        client: &NostrClient,
        request: &Event,
        status: &str,
        message: &str,
    ) -> Result<(), DvmError> {
        let feedback = Event::new(
            Kind::Custom(7000),
            message.to_string(),
            vec![
                Tag::Event(request.id.clone(), None, None),
                Tag::PubKey(request.pubkey.clone(), None),
                Tag::custom("status", vec![status]),
            ],
            &self.keys,
        )?;

        client.publish(feedback).await?;
        Ok(())
    }

    /// Publish job result
    async fn publish_result(
        client: &NostrClient,
        keys: &Keys,
        request: &Event,
        result: &str,
    ) {
        let result_event = Event::new(
            Kind::Custom(6050),  // Text generation result
            result.to_string(),
            vec![
                Tag::Event(request.id.clone(), None, None),
                Tag::PubKey(request.pubkey.clone(), None),
                Tag::custom("request", vec![&serde_json::to_string(request).unwrap_or_default()]),
            ],
            keys,
        ).unwrap();

        let _ = client.publish(result_event).await;
    }

    /// Publish error feedback
    async fn publish_error(
        client: &NostrClient,
        keys: &Keys,
        request: &Event,
        error: &str,
    ) {
        let feedback = Event::new(
            Kind::Custom(7000),
            error.to_string(),
            vec![
                Tag::Event(request.id.clone(), None, None),
                Tag::PubKey(request.pubkey.clone(), None),
                Tag::custom("status", vec!["error"]),
            ],
            keys,
        ).unwrap();

        let _ = client.publish(feedback).await;
    }

    /// Publish NIP-89 handler advertisement
    pub async fn publish_handler_info(&self) {
        let info = HandlerInfo {
            name: "WebGPU Inference Provider".to_string(),
            about: "Browser-based LLM inference via WebGPU".to_string(),
            supported_kinds: self.config.supported_kinds.clone(),
            supported_models: self.provider.info().supported_models,
        };

        let event = Event::new(
            Kind::Custom(31990),  // Handler information (NIP-89)
            serde_json::to_string(&info).unwrap(),
            vec![
                Tag::custom("d", vec!["webgpu-inference"]),
                Tag::custom("k", vec!["5050"]),  // Handles text generation
            ],
            &self.keys,
        ).unwrap();

        // Publish to all relays
        for relay in &self.relays {
            let client = NostrClient::new(&[relay.clone()]).await;
            let _ = client.publish(event.clone()).await;
        }
    }
}
```

## Job Request Types

```rust
// crates/ml/src/provider/types.rs

#[derive(Debug, Clone)]
pub struct JobRequest {
    pub input: String,
    pub model: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HandlerInfo {
    pub name: String,
    pub about: String,
    pub supported_kinds: Vec<u16>,
    pub supported_models: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DvmError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Compute error: {0}")]
    Compute(#[from] ComputeError),

    #[error("Nostr error: {0}")]
    Nostr(String),
}
```

## Nostr Client (Simplified)

```rust
// crates/ml/src/provider/nostr_client.rs
// Simplified client - in practice, use nostr_client crate

pub struct NostrClient {
    relays: Vec<String>,
    // WebSocket connections would go here
}

impl NostrClient {
    pub async fn new(relays: &[String]) -> Self {
        Self {
            relays: relays.to_vec(),
        }
    }

    pub async fn subscribe(&self, filters: Vec<Filter>) -> Subscription {
        // Connect to relays and create subscription
        // Returns stream of matching events
        todo!()
    }

    pub async fn publish(&self, event: Event) -> Result<(), DvmError> {
        // Publish event to all connected relays
        todo!()
    }
}

pub struct Subscription {
    // Event stream
}

impl Subscription {
    pub async fn next(&mut self) -> Option<Event> {
        // Yield next matching event
        todo!()
    }
}
```

## Browser Integration

### WASM Entry Point

```rust
// crates/ml/src/lib.rs

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct BrowserProvider {
    service: BrowserDvmService,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl BrowserProvider {
    #[wasm_bindgen(constructor)]
    pub async fn new(private_key: &str, model_url: &str) -> Result<BrowserProvider, JsValue> {
        // Set up panic hook for better error messages
        console_error_panic_hook::set_once();

        // Parse keys
        let keys = Keys::from_sk_str(private_key)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Create WebGPU provider
        let mut provider = WebGpuProvider::new(Default::default())
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Load model
        provider.load_model(model_url, ModelConfig::default())
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Create DVM service
        let service = BrowserDvmService::new(
            keys,
            Arc::new(provider),
            DvmConfig::default(),
        );

        Ok(BrowserProvider { service })
    }

    #[wasm_bindgen]
    pub fn start(&self) {
        self.service.start();
    }

    #[wasm_bindgen]
    pub fn pubkey(&self) -> String {
        self.service.keys.public_key().to_string()
    }
}
```

### JavaScript Usage

```html
<!DOCTYPE html>
<html>
<head>
    <title>WebGPU Inference Provider</title>
</head>
<body>
    <h1>WebGPU Inference Provider</h1>
    <p>Status: <span id="status">Initializing...</span></p>
    <p>Public Key: <span id="pubkey">-</span></p>
    <p>Jobs Completed: <span id="jobs">0</span></p>

    <script type="module">
        import init, { BrowserProvider } from './pkg/ml.js';

        async function main() {
            await init();

            const status = document.getElementById('status');
            const pubkeyEl = document.getElementById('pubkey');

            try {
                status.textContent = 'Loading model...';

                // Generate or load private key
                const privateKey = localStorage.getItem('nostr_pk') ||
                    generatePrivateKey();  // From nostr library
                localStorage.setItem('nostr_pk', privateKey);

                // Create provider
                const provider = await new BrowserProvider(
                    privateKey,
                    'https://huggingface.co/TheBloke/Llama-2-7B-GGUF/resolve/main/llama-2-7b.Q4_K_M.gguf'
                );

                pubkeyEl.textContent = provider.pubkey();
                status.textContent = 'Ready - Listening for jobs';

                // Start listening
                provider.start();

            } catch (e) {
                status.textContent = 'Error: ' + e;
                console.error(e);
            }
        }

        main();
    </script>
</body>
</html>
```

## Payment Integration

For paid inference, integrate with NIP-57 (Zaps) or Lightning invoices:

```rust
impl BrowserDvmService {
    /// Calculate price for job
    fn calculate_price(&self, request: &JobRequest) -> u64 {
        let max_tokens = request.max_tokens.unwrap_or(256) as u64;
        max_tokens * self.config.min_price_per_token
    }

    /// Handle payment flow
    async fn handle_paid_job(
        &self,
        client: &NostrClient,
        event: &Event,
        request: JobRequest,
    ) -> Result<(), DvmError> {
        let price = self.calculate_price(&request);

        // Send payment request feedback
        self.publish_feedback(
            client,
            event,
            "payment-required",
            &format!("{{\"amount\": {}, \"unit\": \"msats\"}}", price),
        ).await?;

        // Wait for payment (simplified)
        // In practice, monitor for zap receipt or invoice payment

        // Then proceed with job
        self.handle_event(client, event.clone()).await
    }
}
```

## Capacity Management

```rust
impl BrowserDvmService {
    /// Check if we can accept more jobs
    fn has_capacity(&self) -> bool {
        self.active_jobs.read().len() < self.config.max_concurrent_jobs
    }

    /// Estimate job completion time
    fn estimate_time(&self, request: &JobRequest) -> u64 {
        let tokens = request.max_tokens.unwrap_or(256);
        // Rough estimate: 50ms per token on average WebGPU
        (tokens as u64) * 50
    }

    /// Publish availability status
    pub async fn publish_availability(&self, client: &NostrClient) {
        let status = if self.has_capacity() {
            "available"
        } else {
            "busy"
        };

        let event = Event::new(
            Kind::Custom(31991),  // Provider status
            status.to_string(),
            vec![
                Tag::custom("d", vec!["webgpu-inference-status"]),
                Tag::custom("capacity", vec![
                    &self.config.max_concurrent_jobs.to_string(),
                    &self.active_jobs.read().len().to_string(),
                ]),
            ],
            &self.keys,
        ).unwrap();

        client.publish(event).await.ok();
    }
}
```

## Error Handling

```rust
impl BrowserDvmService {
    /// Handle various error conditions gracefully
    async fn handle_error(&self, client: &NostrClient, event: &Event, error: DvmError) {
        let (status, message) = match &error {
            DvmError::InvalidRequest(msg) => ("error", format!("Invalid request: {}", msg)),
            DvmError::Compute(ComputeError::NoModel) => ("error", "No model loaded".to_string()),
            DvmError::Compute(ComputeError::UnsupportedModel(m)) => {
                ("error", format!("Unsupported model: {}", m))
            }
            _ => ("error", error.to_string()),
        };

        let _ = self.publish_feedback(client, event, status, &message).await;
    }
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_request() {
        let event = Event {
            kind: Kind::Custom(5050),
            tags: vec![
                Tag::Input("Hello world".to_string(), "text".to_string()),
                Tag::Param("model".to_string(), "llama-7b".to_string()),
                Tag::Param("max_tokens".to_string(), "100".to_string()),
            ],
            ..Default::default()
        };

        let config = DvmConfig::default();
        let keys = Keys::generate();
        let provider = Arc::new(/* mock provider */);
        let service = BrowserDvmService::new(keys, provider, config);

        let request = service.parse_job_request(&event).unwrap();
        assert_eq!(request.input, "Hello world");
        assert_eq!(request.model, "llama-7b");
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_calculate_price() {
        let config = DvmConfig {
            min_price_per_token: 2,
            ..Default::default()
        };

        let request = JobRequest {
            input: "test".to_string(),
            model: "test".to_string(),
            max_tokens: Some(100),
            temperature: None,
        };

        let keys = Keys::generate();
        let provider = Arc::new(/* mock provider */);
        let service = BrowserDvmService::new(keys, provider, config);

        assert_eq!(service.calculate_price(&request), 200);  // 100 tokens * 2 msats
    }
}
```
