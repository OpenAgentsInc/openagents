# FM Bridge: Complete Architecture

**Goal:** Fully wrap Apple FoundationModels framework via HTTP bridge + Rust client.

## API Design

### Current Endpoints (Implemented)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check + model availability |
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | Chat completion (non-streaming) |

### New Endpoints (To Implement)

#### 1. Streaming

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/chat/completions?stream=true` | Server-Sent Events stream |

**Request:** Same as non-streaming, add `stream: true`
**Response:** SSE with `data:` lines containing JSON chunks

```
data: {"id":"fm-123","choices":[{"delta":{"content":"Hello"},"index":0}]}
data: {"id":"fm-123","choices":[{"delta":{"content":" world"},"index":0}]}
data: {"id":"fm-123","choices":[{"delta":{},"finish_reason":"stop","index":0}]}
data: [DONE]
```

#### 2. Session Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/sessions` | Create persistent session |
| GET | `/v1/sessions/{id}` | Get session info (transcript, state) |
| POST | `/v1/sessions/{id}/messages` | Add message + generate response |
| DELETE | `/v1/sessions/{id}` | Delete session |
| POST | `/v1/sessions/{id}/prewarm` | Prewarm session with prompt prefix |

**Session object:**
```json
{
  "id": "sess_abc123",
  "created": 1234567890,
  "model": "apple-foundation-model",
  "transcript": {
    "entries": [
      {"type": "instructions", "content": "You are helpful"},
      {"type": "prompt", "content": "Hello"},
      {"type": "response", "content": "Hi there!"}
    ]
  },
  "is_responding": false
}
```

#### 3. Tool Calling

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/sessions/{id}/tools/register` | Register tool definitions |
| POST | `/v1/sessions/{id}/tools/unregister` | Unregister tools |
| GET | `/v1/sessions/{id}/tools` | List registered tools |

**Tool definition:**
```json
{
  "name": "get_weather",
  "description": "Get weather for a city",
  "parameters": {
    "type": "object",
    "properties": {
      "city": {
        "type": "string",
        "description": "City name"
      },
      "unit": {
        "type": "string",
        "enum": ["celsius", "fahrenheit"]
      }
    },
    "required": ["city"]
  }
}
```

**Tool call in response:**
```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "tool_calls": [{
        "id": "call_123",
        "type": "function",
        "function": {
          "name": "get_weather",
          "arguments": "{\"city\":\"San Francisco\",\"unit\":\"fahrenheit\"}"
        }
      }]
    },
    "finish_reason": "tool_calls"
  }]
}
```

#### 4. Adapter Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/adapters/load` | Load adapter from path |
| GET | `/v1/adapters` | List loaded adapters |
| POST | `/v1/adapters/{id}/compile` | Compile adapter for device |
| DELETE | `/v1/adapters/{id}` | Unload adapter |

**Adapter object:**
```json
{
  "id": "adapter_medical",
  "name": "medical-assistant",
  "path": "/path/to/model.mlpackage",
  "compiled": true,
  "metadata": {
    "creator": "OpenAgents",
    "version": "1.0"
  }
}
```

#### 5. Enhanced Model Info

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/models/{id}/locales` | Get supported locales |
| GET | `/v1/models/{id}/availability` | Detailed availability info |

**Availability response:**
```json
{
  "available": true,
  "reason": null,
  "use_case": "general",
  "guardrails": "default",
  "supported_languages": ["en", "es", "fr", "de", "zh"],
  "current_locale_supported": true
}
```

### Enhanced Request Parameters

#### GenerationOptions (in `/v1/chat/completions`)

```json
{
  "model": "apple-foundation-model",
  "messages": [...],
  "temperature": 0.7,
  "max_tokens": 1000,
  "sampling": {
    "mode": "random_top_k",  // "greedy" | "random_top_k" | "random_nucleus"
    "k": 40,  // for top_k
    "probability_threshold": 0.9,  // for nucleus
    "seed": 42  // optional
  },
  "use_case": "general",  // or "content_tagging"
  "guardrails": "default",  // or "permissive_content_transformations"
  "response_format": {
    "type": "json_schema",
    "schema_type": "test_generation"  // or custom schema
  }
}
```

## Swift Bridge Architecture

### File Structure

```
swift/foundation-bridge/Sources/foundation-bridge/
├── main.swift              # Entry point
├── Server.swift            # HTTP server (Network.framework)
├── Types.swift             # Request/response types
├── GuidedTypes.swift       # @Generable schemas
├── Handlers/
│   ├── ChatHandler.swift       # Chat completions (existing)
│   ├── StreamHandler.swift     # SSE streaming (NEW)
│   ├── SessionHandler.swift    # Session management (NEW)
│   ├── ToolHandler.swift       # Tool calling (NEW)
│   └── AdapterHandler.swift    # Adapter management (NEW)
├── Models/
│   ├── SessionStore.swift      # In-memory session storage (NEW)
│   ├── ToolRegistry.swift      # Tool definitions (NEW)
│   └── AdapterRegistry.swift   # Loaded adapters (NEW)
└── Utils/
    ├── SSEWriter.swift         # Server-Sent Events writer (NEW)
    └── ErrorHandler.swift      # Unified error handling (NEW)
```

### Session Store (In-Memory)

```swift
actor SessionStore {
  private var sessions: [String: LanguageModelSession] = [:]
  private var transcripts: [String: Transcript] = [:]

  func create(model: SystemLanguageModel, tools: [any Tool], instructions: Instructions?) -> String
  func get(_ id: String) -> LanguageModelSession?
  func delete(_ id: String)
  func updateTranscript(_ id: String, transcript: Transcript)
}
```

### Tool Registry

```swift
actor ToolRegistry {
  private var tools: [String: [ToolDefinition]] = [:]  // sessionId -> tools

  func register(sessionId: String, tools: [ToolDefinition])
  func unregister(sessionId: String, toolNames: [String])
  func get(sessionId: String) -> [ToolDefinition]
}
```

## Rust Client Architecture

### File Structure

```
fm-bridge/src/
├── lib.rs              # Public API exports
├── client.rs           # HTTP client (existing)
├── types.rs            # Request/response types (existing)
├── error.rs            # Error types (existing)
├── streaming.rs        # SSE streaming (NEW)
├── sessions.rs         # Session management (NEW)
├── tools.rs            # Tool calling API (NEW)
├── adapters.rs         # Adapter management (NEW)
└── bin/
    └── cli.rs          # CLI tool (existing)
```

### New Modules

#### `streaming.rs`

```rust
pub struct StreamingClient {
  client: FMClient,
}

impl StreamingClient {
  pub async fn stream(
    &self,
    request: CompletionRequest,
  ) -> Result<impl Stream<Item = Result<StreamChunk>>> {
    // Use eventsource-stream
  }
}

pub struct StreamChunk {
  pub id: String,
  pub delta: Delta,
  pub finish_reason: Option<FinishReason>,
}

pub struct Delta {
  pub content: Option<String>,
  pub tool_calls: Option<Vec<ToolCall>>,
}
```

#### `sessions.rs`

```rust
pub struct SessionClient {
  client: FMClient,
}

impl SessionClient {
  pub async fn create(&self, options: SessionOptions) -> Result<Session>;
  pub async fn get(&self, id: &str) -> Result<Session>;
  pub async fn send_message(&self, id: &str, message: Message) -> Result<CompletionResponse>;
  pub async fn delete(&self, id: &str) -> Result<()>;
  pub async fn prewarm(&self, id: &str, prefix: Option<String>) -> Result<()>;
}

pub struct Session {
  pub id: String,
  pub created: i64,
  pub model: String,
  pub transcript: Transcript,
  pub is_responding: bool,
}

pub struct Transcript {
  pub entries: Vec<TranscriptEntry>,
}

pub enum TranscriptEntry {
  Instructions(String),
  Prompt(String),
  Response(String),
  ToolCalls(Vec<ToolCall>),
  ToolOutput(Vec<ToolOutput>),
}
```

#### `tools.rs`

```rust
pub struct ToolClient {
  client: FMClient,
}

impl ToolClient {
  pub async fn register(&self, session_id: &str, tools: Vec<ToolDefinition>) -> Result<()>;
  pub async fn unregister(&self, session_id: &str, tool_names: Vec<String>) -> Result<()>;
  pub async fn list(&self, session_id: &str) -> Result<Vec<ToolDefinition>>;
}

pub struct ToolDefinition {
  pub name: String,
  pub description: String,
  pub parameters: serde_json::Value,  // JSON schema
}

pub struct ToolCall {
  pub id: String,
  pub name: String,
  pub arguments: serde_json::Value,
}
```

#### `adapters.rs`

```rust
pub struct AdapterClient {
  client: FMClient,
}

impl AdapterClient {
  pub async fn load(&self, path: &str, name: Option<String>) -> Result<Adapter>;
  pub async fn list(&self) -> Result<Vec<Adapter>>;
  pub async fn compile(&self, id: &str) -> Result<()>;
  pub async fn unload(&self, id: &str) -> Result<()>;
}

pub struct Adapter {
  pub id: String,
  pub name: String,
  pub path: String,
  pub compiled: bool,
  pub metadata: HashMap<String, serde_json::Value>,
}
```

### Enhanced Types

#### `types.rs` additions

```rust
// Sampling modes
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SamplingMode {
  Greedy,
  RandomTopK { k: u32, seed: Option<u64> },
  RandomNucleus { probability_threshold: f64, seed: Option<u64> },
}

// Enhanced CompletionOptions
pub struct CompletionOptions {
  pub model: Option<String>,
  pub temperature: Option<f64>,
  pub max_tokens: Option<i32>,
  pub sampling: Option<SamplingMode>,
  pub use_case: Option<UseCase>,
  pub guardrails: Option<Guardrails>,
  pub stream: Option<bool>,
}

pub enum UseCase {
  General,
  ContentTagging,
}

pub enum Guardrails {
  Default,
  PermissiveContentTransformations,
}

// Enhanced error types
pub enum GenerationError {
  ExceededContextWindowSize { context: String },
  AssetsUnavailable { context: String },
  GuardrailViolation { context: String },
  UnsupportedGuide { context: String },
  UnsupportedLanguageOrLocale { context: String },
  DecodingFailure { context: String },
  RateLimited { context: String },
  ConcurrentRequests { context: String },
  Refusal { explanation: String, context: String },
}
```

## Implementation Phases

### Phase 1: Streaming (High Priority)
1. Add SSE support to Swift bridge
2. Add streaming client to Rust
3. Tests for streaming

### Phase 2: Session Management (High Priority)
1. Add SessionStore to Swift bridge
2. Add session endpoints
3. Add Rust session client
4. Tests for sessions

### Phase 3: Tool Calling (High Priority)
1. Add ToolRegistry to Swift bridge
2. Add tool endpoints
3. Add Rust tool client
4. Tests for tools

### Phase 4: Enhanced Options (Medium Priority)
1. Add sampling modes to Swift bridge
2. Add use_case/guardrails support
3. Add to Rust types
4. Tests

### Phase 5: Adapters (Medium Priority)
1. Add AdapterRegistry to Swift bridge
2. Add adapter endpoints
3. Add Rust adapter client
4. Tests

### Phase 6: Error Handling (Medium Priority)
1. Map all FM error types in Swift bridge
2. Add to Rust error types
3. Tests

## Testing Strategy

### Unit Tests
- Each Swift handler in isolation
- Each Rust client module in isolation

### Integration Tests
- End-to-end: Rust client → Swift bridge → FoundationModels
- Test files: `fm-bridge/tests/integration_test.rs`

### Test Coverage
- Streaming: partial updates, finish reasons, errors
- Sessions: create, resume, multi-turn, concurrent requests
- Tools: register, call, unregister, errors
- Adapters: load, compile, use in session
- Errors: all 9 error types
- Options: all sampling modes, use cases, guardrails

## Performance Considerations

- Session store: LRU cache with max 100 sessions
- Tool registry: Per-session, cleaned up on session delete
- Adapter registry: Global, shared across sessions
- Streaming: Backpressure handling for slow clients

## Security

- No authentication (local-only service)
- Adapter loading: Validate .mlpackage format
- Tool calling: Sandboxed execution (future)
- Rate limiting: Per-session (use FM's built-in)
