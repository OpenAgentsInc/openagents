# Gateway

The Gateway abstraction for OANIX/runtime. How agents access external capabilities.

---

## What is a Gateway?

A Gateway is an abstraction layer between agents and external (or internal) services. It's an API proxy/adapter that:

1. **Uniform interface** - Agents don't care if they're hitting Cerebras, OpenAI, or local Pylon swarm
2. **Auth handling** - Manages API keys (user-provided or OpenAgents-proxied)
3. **Routing** - Directs requests to appropriate backends
4. **Metering** - Tracks usage for billing/budgets
5. **Fallback** - Auto-retry with alternate providers on failure

Think of Gateways as the "system calls" of OANIX. An agent doesn't directly hit APIs; it asks the runtime for capabilities through Gateways.

---

## Gateway Types

### InferenceGateway
LLM text generation. The primary gateway type.

**Providers:**
- Cerebras (GLM 4.7, Llama variants) - BLAZING fast
- OpenAI (GPT-4, o1, o3)
- OpenAI (Codex, Opus)
- Google (Gemini)
- Groq (fast Llama/Mixtral)
- Together.ai (open models)
- Fireworks (open models)
- **Pylon swarm** (our network - local Apple FM, Ollama, etc.)

### EmbeddingGateway
Vector embeddings for RAG, search, clustering.

**Providers:**
- OpenAI (text-embedding-3-*)
- Voyage (voyage-3, voyage-code-3)
- Cohere (embed-v3)
- Jina (jina-embeddings-v3)
- Local (via Pylon - nomic-embed, etc.)

### ImageGateway
Image generation/editing.

**Providers:**
- OpenAI (DALL-E 3)
- Midjourney (via API)
- Stability (SD3, SDXL)
- Black Forest Labs (Flux)
- Ideogram
- Local (via Pylon - SD, Flux)

### AudioGateway
Speech-to-text, text-to-speech.

**Providers:**
- OpenAI (Whisper, TTS)
- ElevenLabs
- Deepgram
- AssemblyAI
- Local (Whisper via Pylon)

### CodeExecutionGateway
Sandboxed code execution for agents.

**Providers:**
- E2B (cloud sandboxes)
- Modal (serverless)
- Replit
- Local sandbox (our runtime)

### SearchGateway
Web/knowledge search.

**Providers:**
- Perplexity
- Tavily
- Brave Search
- Exa
- SerpAPI

### FineTuningGateway
Model fine-tuning/training.

**Providers:**
- OpenAI (fine-tuning)
- OpenAI (fine-tuning)
- Together.ai
- Modal
- Cerebras (training)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         OANIX RUNTIME                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  Autopilot  │  │  Autopilot  │  │  Autopilot  │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│         └────────────────┼────────────────┘                 │
│                          │                                  │
│                          ▼                                  │
│              ┌───────────────────────┐                      │
│              │    Gateway Registry    │                      │
│              │  (discovers & routes)  │                      │
│              └───────────┬───────────┘                      │
│                          │                                  │
│    ┌─────────────────────┼─────────────────────┐           │
│    │                     │                     │           │
│    ▼                     ▼                     ▼           │
│ ┌──────────┐      ┌──────────┐      ┌──────────┐          │
│ │Inference │      │Embedding │      │  Image   │          │
│ │ Gateway  │      │ Gateway  │      │ Gateway  │          │
│ └────┬─────┘      └────┬─────┘      └────┬─────┘          │
└──────┼─────────────────┼─────────────────┼─────────────────┘
       │                 │                 │
       ▼                 ▼                 ▼
  ┌─────────┐      ┌─────────┐      ┌─────────┐
  │Cerebras │      │ OpenAI  │      │ DALL-E  │
  │ OpenAI  │      │ Voyage  │      │  Flux   │
  │  Pylon  │      │  Local  │      │  Local  │
  └─────────┘      └─────────┘      └─────────┘
```

---

## Plan 9-style Filesystem

OANIX exposes gateways as filesystem paths (Plan 9 philosophy):

```
/gw/                          # Gateway root
  inference/
    cerebras/
      models                  # cat → list models JSON
      complete               # echo request > complete; cat complete
      chat                   # streaming chat interface
      config                 # gateway configuration
      health                 # health check
    openai/
      ...
    pylon/                   # Our swarm network
      ...
    _default                 # symlink to preferred provider

  embedding/
    openai/
      embed                  # write text, read vector
      batch                  # batch embedding
    voyage/
      ...

  image/
    dalle/
      generate              # write prompt, read image URL/bytes
    flux/
      ...

  audio/
    whisper/
      transcribe            # write audio, read text
    elevenlabs/
      speak                 # write text, read audio

  search/
    tavily/
      search                # write query, read results
    perplexity/
      ...

  _registry                  # registry metadata
  _metrics                   # usage metrics
  _budget                    # budget status
```

Agent code:
```rust
// Using Plan 9-style paths
let result = runtime.read("/gw/inference/cerebras/chat", &chat_request)?;

// Or via registry
let inference = runtime.gateway("inference")?;
let result = inference.chat(request).await?;
```

---

## Type Definitions

### Where to put types?

```
crates/
  gateway/                   # NEW CRATE
    Cargo.toml
    src/
      lib.rs                 # Re-exports
      types.rs               # Core types
      traits.rs              # Gateway traits
      registry.rs            # GatewayRegistry
      config.rs              # Configuration

      inference/
        mod.rs
        types.rs             # ChatRequest, CompletionRequest, etc.
        trait.rs             # InferenceGateway trait
        cerebras.rs          # Cerebras impl
        openai.rs            # OpenAI impl
        openai.rs         # OpenAI impl
        pylon.rs             # Pylon swarm impl

      embedding/
        mod.rs
        types.rs             # EmbedRequest, etc.
        trait.rs
        openai.rs
        voyage.rs

      image/
        mod.rs
        types.rs
        trait.rs
        dalle.rs
        flux.rs
```

### Core Types

```rust
// crates/gateway/src/types.rs

/// What a gateway can do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Capability {
    TextGeneration,
    ChatCompletion,
    Streaming,
    FunctionCalling,
    Vision,
    Embedding,
    ImageGeneration,
    ImageEdit,
    SpeechToText,
    TextToSpeech,
    Search,
    CodeExecution,
    FineTuning,
}

/// Gateway health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayHealth {
    pub available: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub last_check: i64,
}

/// Usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub gateway_type: String,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
    pub cost_usd: f64,
    pub timestamp: i64,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_length: u32,
    pub capabilities: Vec<Capability>,
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_million: f64,   // USD per 1M input tokens
    pub output_per_million: f64,  // USD per 1M output tokens
}
```

### Gateway Traits

```rust
// crates/gateway/src/traits.rs

/// Base trait all gateways implement
pub trait Gateway: Send + Sync {
    /// Gateway type identifier (e.g., "inference", "embedding")
    fn gateway_type(&self) -> &str;

    /// Provider name (e.g., "cerebras", "openai")
    fn provider(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Check if properly configured
    fn is_configured(&self) -> bool;

    /// Get capabilities
    fn capabilities(&self) -> Vec<Capability>;

    /// Health check
    async fn health(&self) -> GatewayHealth;

    /// Get current usage metrics
    fn metrics(&self) -> UsageMetrics;
}

// crates/gateway/src/inference/trait.rs

#[async_trait]
pub trait InferenceGateway: Gateway {
    /// List available models
    async fn models(&self) -> Result<Vec<ModelInfo>>;

    /// Text completion
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Streaming completion
    async fn complete_stream(
        &self,
        request: CompletionRequest
    ) -> Result<Pin<Box<dyn Stream<Item = Result<CompletionChunk>> + Send>>>;

    /// Chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;

    /// Streaming chat
    async fn chat_stream(
        &self,
        request: ChatRequest
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk>> + Send>>>;
}
```

### Inference Types

```rust
// crates/gateway/src/inference/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    pub name: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    Image { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

---

## Configuration

### Gateway Config

```rust
// crates/gateway/src/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Provider identifier
    pub provider: String,

    /// Enabled/disabled
    pub enabled: bool,

    /// API key (direct value - avoid in files, prefer env)
    #[serde(skip_serializing)]
    pub api_key: Option<String>,

    /// Environment variable containing API key
    pub api_key_env: Option<String>,

    /// Route through OpenAgents proxy instead
    pub use_openagents_proxy: bool,

    /// Custom endpoint override
    pub endpoint: Option<String>,

    /// Default model for this provider
    pub default_model: Option<String>,

    /// Priority (lower = preferred)
    pub priority: u32,

    /// Rate limit (requests per minute)
    pub rate_limit: Option<u32>,

    /// Budget limit (USD)
    pub budget_limit: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaysConfig {
    pub inference: Vec<GatewayConfig>,
    pub embedding: Vec<GatewayConfig>,
    pub image: Vec<GatewayConfig>,
    pub audio: Vec<GatewayConfig>,
    pub search: Vec<GatewayConfig>,
}
```

### OANIX config file

```toml
# ~/.openagents/oanix.toml

[gateways.inference.cerebras]
enabled = true
api_key_env = "CEREBRAS_API_KEY"
default_model = "zai-glm-4.7"
priority = 1

[gateways.inference.openai]
enabled = true
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"
priority = 2

[gateways.inference.openai]
enabled = true
api_key_env = "OPENAI_API_KEY"
default_model = "codex-sonnet-4-20250514"
priority = 3

[gateways.inference.pylon]
enabled = true
use_openagents_proxy = false  # Direct to swarm
priority = 10  # Fallback

[gateways.embedding.openai]
enabled = true
api_key_env = "OPENAI_API_KEY"
default_model = "text-embedding-3-large"

[gateways.embedding.voyage]
enabled = true
api_key_env = "VOYAGE_API_KEY"
default_model = "voyage-3"
```

---

## Cerebras Implementation

Cerebras has an OpenAI-compatible API. Fast as hell.

```rust
// crates/gateway/src/inference/cerebras.rs

pub struct CerebrasGateway {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
    metrics: Arc<Mutex<UsageMetrics>>,
}

impl CerebrasGateway {
    pub fn new(config: &GatewayConfig) -> Result<Self> {
        let api_key = config.api_key.clone()
            .or_else(|| config.api_key_env.as_ref()
                .and_then(|env| std::env::var(env).ok()))
            .ok_or_else(|| anyhow!("Cerebras API key not configured"))?;

        let endpoint = config.endpoint.clone()
            .unwrap_or_else(|| "https://api.cerebras.ai/v1".to_string());

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            endpoint,
            metrics: Arc::new(Mutex::new(UsageMetrics::default())),
        })
    }

    pub const MODELS: &'static [&'static str] = &[
        "zai-glm-4.7",
        "llama-3.3-70b",
        "llama-3.1-70b",
        "llama-3.1-8b",
    ];
}

#[async_trait]
impl InferenceGateway for CerebrasGateway {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let response = self.client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        // Track metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.input_tokens += response.usage.prompt_tokens as u64;
        metrics.output_tokens += response.usage.completion_tokens as u64;
        metrics.requests += 1;

        Ok(response)
    }

    // ... other methods
}
```

---

## Gateway Registry

```rust
// crates/gateway/src/registry.rs

pub struct GatewayRegistry {
    inference: Vec<Arc<dyn InferenceGateway>>,
    embedding: Vec<Arc<dyn EmbeddingGateway>>,
    image: Vec<Arc<dyn ImageGateway>>,
    // ...
    config: GatewaysConfig,
}

impl GatewayRegistry {
    /// Load from config and environment
    pub fn from_config(config: GatewaysConfig) -> Result<Self> {
        let mut registry = Self {
            inference: vec![],
            embedding: vec![],
            image: vec![],
            config,
        };

        // Initialize configured gateways
        for cfg in &registry.config.inference {
            if !cfg.enabled { continue; }

            let gw: Arc<dyn InferenceGateway> = match cfg.provider.as_str() {
                "cerebras" => Arc::new(CerebrasGateway::new(cfg)?),
                "openai" => Arc::new(OpenAIGateway::new(cfg)?),
                "openai" => Arc::new(OpenAIGateway::new(cfg)?),
                "pylon" => Arc::new(PylonGateway::new(cfg)?),
                _ => continue,
            };

            registry.inference.push(gw);
        }

        // Sort by priority
        registry.inference.sort_by_key(|g| g.priority());

        Ok(registry)
    }

    /// Get preferred inference gateway
    pub fn inference(&self) -> Option<&Arc<dyn InferenceGateway>> {
        self.inference.first()
    }

    /// Get all inference gateways
    pub fn inference_all(&self) -> &[Arc<dyn InferenceGateway>] {
        &self.inference
    }

    /// Get inference gateway by provider
    pub fn inference_provider(&self, provider: &str) -> Option<&Arc<dyn InferenceGateway>> {
        self.inference.iter().find(|g| g.provider() == provider)
    }

    /// Health check all gateways
    pub async fn health_check(&self) -> HashMap<String, GatewayHealth> {
        // Parallel health checks
        let futures: Vec<_> = self.inference.iter()
            .map(|g| async move {
                let health = g.health().await;
                (format!("inference/{}", g.provider()), health)
            })
            .collect();

        futures::future::join_all(futures).await.into_iter().collect()
    }
}
```

---

## RLM Integration

RLMs fan out to many providers. Gateway abstraction makes this clean:

```rust
// In crates/pylon/src/rlm/

pub async fn rlm_fanout(
    registry: &GatewayRegistry,
    sub_queries: Vec<SubQuery>,
    config: &RlmConfig,
) -> Vec<Result<SubResult>> {
    let gateways = registry.inference_all();

    // Distribute queries across gateways (round-robin, load-balanced, etc.)
    let tasks: Vec<_> = sub_queries
        .into_iter()
        .enumerate()
        .map(|(i, query)| {
            let gateway = &gateways[i % gateways.len()];
            let request = ChatRequest {
                model: config.model.clone(),
                messages: vec![Message::user(query.prompt)],
                max_tokens: Some(config.max_tokens),
                ..Default::default()
            };

            async move {
                gateway.chat(request).await
                    .map(|r| SubResult {
                        query_id: query.id,
                        content: r.choices[0].message.content.clone(),
                    })
            }
        })
        .collect();

    futures::future::join_all(tasks).await
}
```

---

## OpenAgents Gateway Service

We'll run our own gateway at `api.openagents.com` or `gateway.openagents.com`:

```
POST /v1/chat/completions     # OpenAI-compatible
POST /v1/embeddings           # OpenAI-compatible
POST /v1/images/generations   # OpenAI-compatible

# Extended endpoints
GET  /v1/gateways             # List available gateways
GET  /v1/gateways/:type       # List providers for type
GET  /v1/models               # All models across all providers
POST /v1/rlm/fanout           # RLM-specific batch endpoint
```

Users can:
1. Use their own API keys (pass-through)
2. Use OpenAgents credits (we bill, we pay providers)
3. Use Pylon swarm (Bitcoin micropayments)

---

## API Key Management

Three modes:

### 1. User-Provided Keys
User sets env vars, we just route:
```bash
export CEREBRAS_API_KEY=csk-xxx
export OPENAI_API_KEY=sk-xxx
```

### 2. OpenAgents Proxy
User has OpenAgents account with credits:
```toml
[gateways.inference.cerebras]
use_openagents_proxy = true
```

We proxy to Cerebras, user pays us, we pay Cerebras.

### 3. Pylon Swarm
Decentralized, Bitcoin-native:
```toml
[gateways.inference.pylon]
enabled = true
# Uses your Pylon wallet automatically
```

---

## Budget Controls

Agents need budget limits:

```rust
pub struct BudgetConfig {
    /// Max USD per hour
    pub hourly_limit: Option<f64>,

    /// Max USD per day
    pub daily_limit: Option<f64>,

    /// Max USD total (lifetime)
    pub total_limit: Option<f64>,

    /// Max tokens per request
    pub max_tokens_per_request: Option<u32>,

    /// Alert threshold (percentage)
    pub alert_threshold: f32,
}
```

Registry enforces:
```rust
impl GatewayRegistry {
    pub async fn chat_with_budget(
        &self,
        request: ChatRequest,
        budget: &BudgetConfig,
    ) -> Result<ChatResponse> {
        // Check budget before request
        self.check_budget(budget)?;

        let result = self.inference()?.chat(request).await?;

        // Track spend after request
        self.track_spend(&result.usage)?;

        Ok(result)
    }
}
```

---

## Next Steps

1. **Create `crates/gateway/`** with types and traits
2. **Implement CerebrasGateway** first (OpenAI-compatible, easy)
3. **Implement PylonGateway** (ties into existing NIP-90 flow)
4. **Add to OANIX runtime** - expose via Plan 9 paths
5. **Gateway CLI** - `oanix gateway list`, `oanix gateway test cerebras`
6. **RLM fanout** - Use gateways for async sub-query distribution

---

## Cerebras Quick Start

```bash
# Set API key
export CEREBRAS_API_KEY=your-key-here

# Test via curl (OpenAI-compatible)
curl https://api.cerebras.ai/v1/chat/completions \
  -H "Authorization: Bearer $CEREBRAS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "zai-glm-4.7",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

Docs: https://inference-docs.cerebras.ai/models/zai-glm-47

---

## Integration with Existing Runtime

Looking at SYNTHESIS_EXECUTION.md, the Runtime already has:

```
/compute/                # LLM inference jobs
├── providers/          # available backends
├── new                 # submit job
└── jobs/<id>/          # status, result, stream
```

**Gateway is the abstraction layer that POWERS `/compute`.**

The current `/compute` in Runtime routes to Pylon/Nexus for NIP-90 jobs. Gateway generalizes this:

```
/compute/                     # EXISTING - now powered by Gateway
├── providers/               # Registry.inference_all().map(|g| g.provider())
├── new                      # Registry.inference().chat(request)
└── jobs/<id>/

/gw/                         # NEW - explicit gateway access
├── inference/               # Multiple providers
│   ├── cerebras/
│   ├── openai/
│   ├── pylon/              # Swarm (current NIP-90 path)
│   └── _default            # Symlink to preferred
├── embedding/
├── image/
└── _registry
```

**Backward compatibility:**
- `/compute/new` still works - uses `Registry.inference()` (preferred provider)
- `/gw/inference/cerebras/chat` gives explicit provider control
- Agents can use either based on need

**Implementation path:**
1. Create `crates/gateway/` with traits + Cerebras impl
2. Refactor `crates/runtime/src/compute.rs` to use GatewayRegistry
3. Add `/gw/` mount point alongside existing `/compute/`
4. PylonGateway wraps existing NIP-90 code

---

## Relationship to Pylon

Pylon currently does two things for compute:
1. **Provider mode**: Receives NIP-90 jobs, runs local inference, gets paid
2. **Client mode**: Submits NIP-90 jobs, pays providers, gets results

**PylonGateway** wraps the client mode:

```rust
// crates/gateway/src/inference/pylon.rs

pub struct PylonGateway {
    relay_service: RelayService,
    dvm_client: DvmClient,
    wallet: SparkWallet,
}

#[async_trait]
impl InferenceGateway for PylonGateway {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // Convert to NIP-90 job
        let job = Nip90Job {
            kind: 5050,
            input: request.messages.last()?.content.to_string(),
            params: vec![
                ("model", &request.model),
                ("max_tokens", &request.max_tokens.unwrap_or(1024).to_string()),
            ],
            bid: 1000, // sats
        };

        // Submit to Nexus
        let job_id = self.dvm_client.submit(&job).await?;

        // Wait for invoice, pay, get result
        let invoice = self.dvm_client.wait_for_invoice(job_id).await?;
        self.wallet.pay(&invoice.bolt11).await?;
        let result = self.dvm_client.wait_for_result(job_id).await?;

        // Convert back to ChatResponse
        Ok(ChatResponse {
            id: job_id,
            model: request.model,
            choices: vec![Choice {
                message: Message::assistant(result.content),
                ..Default::default()
            }],
            usage: Usage::default(),
            created: chrono::Utc::now().timestamp(),
        })
    }
}
```

**Priority order** in GatewayRegistry:
1. Cerebras (fast, cheap for small models)
2. OpenAI (reliable, expensive)
3. OpenAI (Codex for complex tasks)
4. Pylon swarm (decentralized fallback, Bitcoin-native)

---

## First Implementation: CerebrasGateway

Minimal viable gateway to prove the abstraction:

```rust
// crates/gateway/src/inference/cerebras.rs

use crate::{
    Gateway, GatewayHealth, InferenceGateway, Capability,
    ChatRequest, ChatResponse, ModelInfo,
};

pub struct CerebrasGateway {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl CerebrasGateway {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("CEREBRAS_API_KEY")
            .map_err(|_| anyhow!("CEREBRAS_API_KEY not set"))?;

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            endpoint: "https://api.cerebras.ai/v1".into(),
        })
    }
}

impl Gateway for CerebrasGateway {
    fn gateway_type(&self) -> &str { "inference" }
    fn provider(&self) -> &str { "cerebras" }
    fn name(&self) -> &str { "Cerebras Cloud" }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::TextGeneration,
            Capability::ChatCompletion,
            Capability::Streaming,
        ]
    }

    async fn health(&self) -> GatewayHealth {
        let start = std::time::Instant::now();
        match self.models().await {
            Ok(_) => GatewayHealth {
                available: true,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: None,
                last_check: chrono::Utc::now().timestamp(),
            },
            Err(e) => GatewayHealth {
                available: false,
                latency_ms: None,
                error: Some(e.to_string()),
                last_check: chrono::Utc::now().timestamp(),
            },
        }
    }
}

#[async_trait]
impl InferenceGateway for CerebrasGateway {
    async fn models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "zai-glm-4.7".into(),
                name: "GLM 4.7".into(),
                provider: "cerebras".into(),
                context_length: 128_000,
                capabilities: vec![Capability::ChatCompletion],
                pricing: Some(ModelPricing {
                    input_per_million: 0.10,
                    output_per_million: 0.10,
                }),
            },
            ModelInfo {
                id: "llama-3.3-70b".into(),
                name: "Llama 3.3 70B".into(),
                provider: "cerebras".into(),
                context_length: 128_000,
                capabilities: vec![Capability::ChatCompletion],
                pricing: Some(ModelPricing {
                    input_per_million: 0.85,
                    output_per_million: 1.20,
                }),
            },
        ])
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }
}
```

---

## Crate Structure

```
crates/gateway/
├── Cargo.toml
└── src/
    ├── lib.rs              # pub use everything
    ├── types.rs            # Capability, GatewayHealth, Usage, ModelInfo
    ├── traits.rs           # Gateway, InferenceGateway, EmbeddingGateway...
    ├── config.rs           # GatewayConfig, GatewaysConfig
    ├── registry.rs         # GatewayRegistry
    │
    ├── inference/
    │   ├── mod.rs          # pub use
    │   ├── types.rs        # ChatRequest, ChatResponse, Message, etc.
    │   ├── cerebras.rs     # CerebrasGateway
    │   ├── openai.rs       # OpenAIGateway (later)
    │   └── pylon.rs        # PylonGateway (wraps NIP-90)
    │
    ├── embedding/
    │   ├── mod.rs
    │   ├── types.rs
    │   └── openai.rs
    │
    └── image/
        ├── mod.rs
        ├── types.rs
        └── dalle.rs
```

Dependencies:
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
anyhow = "1"
chrono = "0.4"
tokio = { version = "1", features = ["full"] }
futures = "0.3"
```

---

## Open Questions

1. ~~Should Gateway be its own crate or part of OANIX runtime?~~ **Own crate: `crates/gateway/`**
2. How do gateways interact with Nostr? NIP-89-style discovery for third-party gateways?
3. Rate limiting strategy - per-gateway or global?
4. Caching layer for identical requests?
5. How do gateways expose in MCP for Codex Code integration?
6. Should OpenAgents run a public gateway proxy at api.openagents.com?
7. How does billing work when using OpenAgents proxy vs direct keys?
