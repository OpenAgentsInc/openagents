# FM Bridge Visualizer — Phase 1 Plan

**Episode 201: Apple Silicon Fracking**

## Executive Summary

Build a real-time visualization HUD for Apple Foundation Models inference streamed via the FM Bridge HTTP/SSE interface. Unlike GPT-OSS (which has deep kernel-level telemetry), FM Bridge operates at the API boundary — we visualize what Apple exposes: token streams, session state, tool orchestration, and guided generation output.

This is **Phase 1** of the Apple Silicon compute lane. The narrative: "What happens when 100M+ Macs become inference nodes?" We need to show it happening — live, in the browser.

---

## What We Have (FM Bridge Architecture)

```
┌─────────────────────────────────────────────────────────────────┐
│  Swift Foundation Bridge (port 11435)                          │
│  ├── /v1/chat/completions  (POST, SSE streaming)               │
│  ├── /v1/sessions          (multi-turn context)                │
│  ├── /v1/sessions/{id}/tools  (tool registration)              │
│  ├── /v1/adapters          (custom model adapters)             │
│  └── /health               (model availability)                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ SSE: data: {"delta": {"content": "..."}}
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Browser Client (wgpui / WebGPU UI)                            │
│  ├── Token stream consumer                                      │
│  ├── Session state tracker                                      │
│  ├── Tool orchestration viz                                     │
│  └── HUD renderer                                               │
└─────────────────────────────────────────────────────────────────┘
```

### SSE Stream Format (OpenAI-compatible)

```json
data: {"id":"fm-uuid","object":"chat.completion.chunk","created":1733400000,"model":"apple-foundation-model","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"fm-uuid","object":"chat.completion.chunk","created":1733400000,"model":"apple-foundation-model","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"fm-uuid","object":"chat.completion.chunk","created":1733400000,"model":"apple-foundation-model","choices":[{"index":0,"delta":{"content":""},"finish_reason":"stop"}]}

data: [DONE]
```

---

## What Apple Exposes vs. What We Want

| Data | GPT-OSS (have) | FM Bridge (have) | FM Bridge (want) |
|------|----------------|------------------|------------------|
| Token stream | ✅ Full telemetry | ✅ SSE deltas | ✅ |
| Token probabilities | ✅ top-k with logprobs | ❌ Not exposed | ⚠️ Estimate via timing heuristics |
| Attention weights | ✅ Per layer/head | ❌ Not exposed | ❌ Skip |
| Layer activations | ✅ Norms per layer | ❌ Not exposed | ❌ Skip |
| KV cache state | ✅ seq_len, offset | ❌ Not exposed | ⚠️ Estimate from token count |
| Memory usage | ✅ GPU/cache/activations | ❌ Not exposed | ⚠️ System-level if available |
| Session transcript | N/A | ✅ Full history | ✅ |
| Tool calls | N/A | ✅ Registration + invocation | ✅ |
| Guided generation | N/A | ✅ @Generable schemas | ✅ |
| Model health | ✅ | ✅ | ✅ |
| Adapter status | N/A | ✅ Load/compile state | ✅ |

**Key insight:** We trade kernel-level introspection for API-level orchestration visibility. The HUD shifts from "watch the GPU compute" to "watch the agent think."

---

## Visual Grammar (Inherited from GPT-OSS HUD)

| Primitive | Meaning | FM Bridge Application |
|-----------|---------|----------------------|
| **Fill** | Capacity/progress | Session context usage, adapter load progress |
| **Pulse** | Discrete event | Token arrival, tool invocation, SSE chunk |
| **Flow** | Data movement | Token stream, transcript updates |
| **Heat** | Intensity/frequency | Tokens/sec, session activity |
| **Topology** | Graph structure | Tool call tree, session dependency graph |

---

## HUD Components

### 1. Token Stream Panel (PRIMARY)

Live token-by-token rendering as SSE chunks arrive.

```
┌─────────────────────────────────────────────────────────────────┐
│ TOKEN STREAM                                          12.4 t/s  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  The capital of France is Paris, which is known for the        │
│  Eiffel Tower and its rich cultural heritage.█                  │
│                                                                 │
│  ───────────────────────────────────────────────────────────── │
│  TOKENS: 24 │ LATENCY: 847ms TTFT │ STREAM: ACTIVE              │
└─────────────────────────────────────────────────────────────────┘
```

**Telemetry:**
- `token_count`: Running count of tokens generated
- `ttft_ms`: Time to first token (latency)
- `tokens_per_sec`: Rolling average throughput
- `stream_status`: `idle` | `connecting` | `streaming` | `complete` | `error`

**Visual effects:**
- Cursor pulse on active generation
- Fade-in animation per token (subtle, <100ms)
- Throughput gauge with color gradient (green > 10 t/s, yellow 5-10, red < 5)

### 2. Session Timeline Panel

Visualize multi-turn conversation as a vertical timeline.

```
┌─────────────────────────────────────────────────────────────────┐
│ SESSION: fm-a1b2c3d4                             TURNS: 3       │
├─────────────────────────────────────────────────────────────────┤
│  ┌─ USER ──────────────────────────────────────────────────┐   │
│  │ What is the capital of France?                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│           │                                                     │
│           ▼                                                     │
│  ┌─ ASSISTANT ─────────────────────────────────────────────┐   │
│  │ The capital of France is Paris.                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│           │                                                     │
│           ▼                                                     │
│  ┌─ USER ──────────────────────────────────────────────────┐   │
│  │ What is it famous for?                                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│           │                                                     │
│           ▼ (streaming...)                                      │
│  ┌─ ASSISTANT ─────────────────────────────────────────────┐   │
│  │ Paris is famous for the Eiffel Tower...█                │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Telemetry:**
- `session_id`: Current session UUID
- `turn_count`: Number of user/assistant exchanges
- `context_tokens_estimate`: Approximate tokens in context (heuristic: ~4 chars/token)
- `messages[]`: Transcript array

### 3. Tool Orchestration Panel

When tools are registered and invoked, show the agent's "hands."

```
┌─────────────────────────────────────────────────────────────────┐
│ TOOLS                                            REGISTERED: 3  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  read_file ──────────────●──────────────▶ RESULT               │
│             invoked 2.3s ago    completed                       │
│                                                                 │
│  write_file ─────────────○                                      │
│              idle                                               │
│                                                                 │
│  verify_progress ────────◐──────────────▶ PENDING              │
│                   invoked 0.1s ago                              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Telemetry:**
- `tools[]`: Registered tool definitions (name, description, parameters)
- `tool_invocations[]`: Log of tool calls with timestamps
- `pending_tools[]`: Tools awaiting response

**Visual effects:**
- Idle tools: dim, hollow circle
- Invoked tools: pulsing, filling circle
- Completed tools: solid circle with flow line to result
- Failed tools: red X

### 4. Guided Generation Panel

When using `response_format: { type: "json_schema" }`, visualize the structured output as it streams.

```
┌─────────────────────────────────────────────────────────────────┐
│ GUIDED: test_generation                                         │
├─────────────────────────────────────────────────────────────────┤
│  {                                                              │
│    "requirements": [                                            │
│      "Email must contain @ symbol",                             │
│      "Domain must have valid TLD"█                              │
│    ],                                                           │
│    "assumptions": [...],                                        │
│    "tests": [                                                   │
│      {                                                          │
│        "id": "test_1",                                          │
│        "input": "user@example.com",                             │
│        "category": "happy_path",                                │
│        "confidence": 0.95                                       │
│      }                                                          │
│    ]                                                            │
│  }                                                              │
│  ───────────────────────────────────────────────────────────── │
│  SCHEMA: TestGenerationResult │ FIELDS: 4/4 │ VALID: ✓         │
└─────────────────────────────────────────────────────────────────┘
```

**Telemetry:**
- `schema_type`: Which @Generable schema is active
- `json_partial`: Current JSON fragment being streamed
- `fields_completed`: Count of schema fields populated
- `validation_status`: Real-time schema validation

**Visual effects:**
- Syntax highlighting for JSON
- Field completion indicator (fill bar per field)
- Validation checkmark when complete

### 5. Health & Status Bar

Always-visible status line at top or bottom.

```
┌─────────────────────────────────────────────────────────────────┐
│ FM BRIDGE │ ● CONNECTED │ apple-foundation-model │ macOS 15.1  │
│ ADAPTER: default │ HEALTH: ok │ LATENCY: 23ms                  │
└─────────────────────────────────────────────────────────────────┘
```

**Telemetry:**
- `bridge_url`: Connection endpoint
- `connection_status`: `disconnected` | `connecting` | `connected` | `error`
- `model_id`: Active model name
- `model_available`: Boolean from /health
- `adapter_id`: Active adapter if any
- `ping_latency_ms`: Rolling average to bridge

### 6. Adapter Status Panel (Optional)

When custom adapters are loaded, show their state.

```
┌─────────────────────────────────────────────────────────────────┐
│ ADAPTERS                                                        │
├─────────────────────────────────────────────────────────────────┤
│  default          ●  ACTIVE   │ compiled │ 0 errors            │
│  code-assistant   ○  LOADED   │ compiled │ 0 errors            │
│  creative-writer  ◌  LOADING  │ ████░░░░ │ 45%                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Architecture

```
                    ┌─────────────────────────────────────┐
                    │  FmBridgeClient (Rust/WASM)         │
                    │  ├── connect()                      │
                    │  ├── stream_completion() → SSE      │
                    │  ├── create_session()               │
                    │  ├── register_tools()               │
                    │  └── poll_telemetry() → FmTelemetry │
                    └─────────────────────────────────────┘
                                      │
                                      ▼
                    ┌─────────────────────────────────────┐
                    │  FmVizState (Rust)                  │
                    │  ├── stream: TokenStreamState       │
                    │  ├── session: SessionState          │
                    │  ├── tools: ToolOrchestrationState  │
                    │  ├── guided: GuidedGenerationState  │
                    │  ├── health: BridgeHealthState      │
                    │  └── adapters: AdapterStatusState   │
                    └─────────────────────────────────────┘
                                      │
                                      ▼
                    ┌─────────────────────────────────────┐
                    │  FmVizPanel (wgpui)                 │
                    │  ├── render_token_stream()          │
                    │  ├── render_session_timeline()      │
                    │  ├── render_tool_orchestration()    │
                    │  ├── render_guided_generation()     │
                    │  └── render_status_bar()            │
                    └─────────────────────────────────────┘
```

### Telemetry Event Types

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FmBridgeTelemetry {
    /// Bridge connection status changed
    ConnectionStatus {
        status: ConnectionState,
        bridge_url: String,
        latency_ms: Option<u32>,
    },

    /// Health check result
    HealthCheck {
        model_available: bool,
        version: String,
        platform: String,
    },

    /// SSE stream started
    StreamStart {
        completion_id: String,
        model: String,
        is_guided: bool,
        schema_type: Option<String>,
    },

    /// Token chunk received
    TokenChunk {
        completion_id: String,
        delta: String,
        is_final: bool,
        finish_reason: Option<String>,
        timestamp_ms: u64,
    },

    /// Stream completed
    StreamEnd {
        completion_id: String,
        total_tokens: usize,
        duration_ms: u64,
        tokens_per_sec: f32,
    },

    /// Session created
    SessionCreated {
        session_id: String,
    },

    /// Session message appended
    SessionMessage {
        session_id: String,
        role: String,
        content: String,
    },

    /// Tool registered
    ToolRegistered {
        session_id: String,
        tool_name: String,
        description: String,
    },

    /// Tool invoked (detected from model output)
    ToolInvoked {
        session_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        timestamp_ms: u64,
    },

    /// Tool result received
    ToolResult {
        session_id: String,
        tool_name: String,
        result: String,
        success: bool,
        duration_ms: u64,
    },

    /// Adapter status changed
    AdapterStatus {
        adapter_id: String,
        status: AdapterState,
        progress: Option<f32>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterState {
    Unloaded,
    Loading,
    Loaded,
    Compiling,
    Compiled,
    Active,
    Error,
}
```

---

## Implementation Plan

### Gate 1: SSE Client + Token Stream Panel

**Goal:** Click button → connect to FM Bridge → stream tokens → render live.

**Files:**
- `crates/web/client/src/fm_bridge_client.rs` — SSE fetch + parsing
- `crates/web/client/src/fm_viz.rs` — State + telemetry application
- `crates/web/client/src/fm_viz_panel.rs` — wgpui rendering

**Steps:**
1. Implement `FmBridgeClient::stream_completion()` using fetch + ReadableStream
2. Parse SSE `data:` lines into `TokenChunk` events
3. Build `TokenStreamState` with rolling buffer, timing stats
4. Render token stream panel with cursor, throughput gauge
5. Wire up to `/fm` route in web client

**Definition of Done:**
- [ ] Can connect to `localhost:11435/v1/chat/completions?stream=true`
- [ ] Tokens render one-by-one as SSE chunks arrive
- [ ] Throughput gauge shows tokens/sec
- [ ] TTFT displayed after first token

### Gate 2: Session Timeline Panel

**Goal:** Create session → send multiple turns → see transcript visualized.

**Steps:**
1. Implement `FmBridgeClient::create_session()`, `complete_with_session()`
2. Track `SessionState` with message history
3. Render vertical timeline with user/assistant bubbles
4. Animate current response as it streams

**Definition of Done:**
- [ ] Can create session, send 3+ turns
- [ ] Timeline shows all messages with correct roles
- [ ] Current streaming response animates inline

### Gate 3: Health + Status Bar

**Goal:** Always-visible status showing bridge health.

**Steps:**
1. Implement `FmBridgeClient::health_check()` polling
2. Track `BridgeHealthState` with model availability, latency
3. Render status bar with connection indicator, model name

**Definition of Done:**
- [ ] Status bar visible on /fm route
- [ ] Green dot when connected, red when disconnected
- [ ] Model name displayed
- [ ] Latency updated every 5s

### Gate 4: Tool Orchestration Panel

**Goal:** Register tools → invoke → see orchestration flow.

**Steps:**
1. Implement `FmBridgeClient::register_tools()`, `list_tools()`
2. Track `ToolOrchestrationState` with registered tools, invocations
3. Parse model output for tool call patterns (heuristic or guided)
4. Render tool timeline with state indicators

**Definition of Done:**
- [ ] Can register 3 tools for a session
- [ ] Tools displayed with idle/invoked/completed states
- [ ] Tool invocations logged with timestamps

### Gate 5: Guided Generation Panel

**Goal:** Use `response_format: json_schema` → visualize streaming JSON.

**Steps:**
1. Implement guided generation request with `schema_type`
2. Track `GuidedGenerationState` with partial JSON, field completion
3. Render JSON with syntax highlighting
4. Show schema progress indicator

**Definition of Done:**
- [ ] Can request `test_generation` schema
- [ ] JSON renders with syntax highlighting as it streams
- [ ] Field completion bar shows progress
- [ ] Validation checkmark when complete

### Gate 6: Adapter Status Panel (Stretch)

**Goal:** Show loaded adapters and their state.

**Steps:**
1. Implement `FmBridgeClient::list_adapters()`, `load_adapter()`
2. Track `AdapterStatusState` with adapter list
3. Render adapter table with status indicators

---

## Route Structure

```
/fm                    — FM Bridge HUD (primary)
/fm/session/:id        — Session detail view
/fm/adapters           — Adapter management view
```

---

## Comparison: GPT-OSS vs FM Bridge HUD

| Aspect | GPT-OSS HUD | FM Bridge HUD |
|--------|-------------|---------------|
| **Primary focus** | Kernel-level compute | API-level orchestration |
| **Token viz** | Probability waterfall | Stream with timing |
| **Attention** | Heatmap per layer/head | N/A (not exposed) |
| **Layer activity** | Norm bars per layer | N/A (not exposed) |
| **Memory** | GPU/cache gauges | N/A (system-level only) |
| **Session** | N/A | Full transcript timeline |
| **Tools** | N/A | Registration + orchestration |
| **Guided gen** | N/A | JSON schema visualization |
| **Adapters** | N/A | Load/compile status |

**The FM Bridge HUD tells a different story:** Not "watch the matrix multiply" but "watch the agent reason, remember, and act."

---

## Episode 201 Narrative Hook

**"Apple Silicon Fracking"** — We're extracting inference from 100M+ devices that Apple never intended to be inference nodes.

The HUD shows:
1. **Token stream** — "This is your Mac thinking"
2. **Session timeline** — "It remembers what you said"
3. **Tool orchestration** — "It can use tools to act"
4. **Guided generation** — "It produces structured output"

**Demo flow:**
1. Open `/fm` — status bar shows "CONNECTED" to local FM Bridge
2. Type prompt → watch tokens stream in real-time
3. Continue conversation → session timeline grows
4. Register tools → see them light up when invoked
5. Request guided generation → watch JSON structure form

**Closing shot:** "And this is just one Mac. What happens when we connect a million?"

---

## Open Questions

1. **Token probability estimation:** Can we infer confidence from timing patterns? (Slower = more uncertain?)
2. **Memory telemetry:** Can we get system-level GPU memory from browser? (Unlikely)
3. **Tool call detection:** Apple's API doesn't expose tool_calls in Response — need to parse content or use guided generation with `ToolCallRequest` schema
4. **Adapter hot-loading:** Can we switch adapters mid-session without losing context?

---

## Success Criteria

**Phase 1 Complete When:**
- [ ] `/fm` route renders full HUD with 5 panels
- [ ] Can complete 5+ turn conversation with live visualization
- [ ] Throughput gauge accurate within 10%
- [ ] Tool orchestration visible for registered tools
- [ ] Guided generation renders streaming JSON
- [ ] Status bar reflects bridge health in real-time
- [ ] All rendering at 60fps (no jank)
- [ ] Works on Safari, Chrome, Firefox

**Demo-Ready When:**
- [ ] One-click flow from prompt to streaming response
- [ ] Visually compelling for video capture
- [ ] No error states visible in happy path
- [ ] Smooth animations, no flicker
