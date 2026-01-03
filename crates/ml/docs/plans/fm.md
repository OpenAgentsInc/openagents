# Plan: FM Bridge → Pylon Integration + Visualizer

**Goal:** Connect FM Bridge (Apple Foundation Models) into the Pylon runtime as an inference backend, with a `/fm` visualization route in the web client.

**Design Decisions:**
- **Backend:** Merge fm-bridge features into existing `AppleFmBackend` (enhance, not replace)
- **SSE Transport:** Browser connects directly to FM Bridge at localhost:11435
- **Session UX:** New session + prompt input — full demo flow

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           PYLON RUNTIME                                 │
├─────────────────────────────────────────────────────────────────────────┤
│  BackendRegistry                                                        │
│  ├── ollama (OllamaBackend)                                            │
│  ├── apple_fm (AppleFmBackend) ← ENHANCED with fm-bridge features      │
│  └── ml-candle (MlProvider)                                            │
│                                                                         │
│  DvmService                                                             │
│  └── Routes NIP-90 kind:5050 jobs to backends                          │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                              │ HTTP/SSE
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Swift Foundation Bridge (localhost:11435)                              │
│  ├── /v1/chat/completions (streaming)                                  │
│  ├── /v1/sessions (multi-turn)                                         │
│  ├── /v1/sessions/{id}/tools                                           │
│  └── /health                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                              │
                              │ Direct SSE connection (browser → bridge)
                              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  WEB CLIENT (/fm route)                                                │
│  ├── Prompt input → creates session → streams tokens                   │
│  ├── FmVizState (token stream, sessions, tools)                        │
│  └── build_fm_page() rendering                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Pylon Backend Integration

### 1.1 Enhance AppleFmBackend with fm-bridge Features

**File:** `crates/compute/src/backends/apple_fm.rs` (EDIT)

Refactor to use `fm-bridge` crate's `FMClient` instead of raw reqwest:

```rust
use fm_bridge::{FMClient, FMClientBuilder};

pub struct AppleFmBackend {
    client: FMClient,  // Was: reqwest::Client
}

impl AppleFmBackend {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = FMClientBuilder::new()
            .base_url(base_url)
            .build()?;
        Ok(Self { client })
    }

    // NEW: Session management
    pub async fn create_session(&self) -> Result<String>
    pub async fn complete_with_session(&self, session_id: &str, prompt: &str) -> Result<...>

    // NEW: Tool support
    pub async fn register_tools(&self, session_id: &str, tools: Vec<ToolDef>) -> Result<()>
}

#[async_trait]
impl InferenceBackend for AppleFmBackend {
    fn id(&self) -> &str { "apple_fm" }

    // Enhanced streaming via eventsource-stream
    async fn complete_stream(&self, request: CompletionRequest)
        -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        // Use self.client.stream() from fm-bridge crate
    }
}
```

**Enhancements over current implementation:**
- Uses `fm-bridge` crate's `FMClient` (proper SSE, sessions, tools)
- Session management via `/v1/sessions`
- Tool registration via `/v1/sessions/{id}/tools`
- Proper SSE streaming via `eventsource-stream`

### 1.2 Add fm-bridge Dependency

**File:** `crates/compute/Cargo.toml` (EDIT)

```toml
[dependencies]
fm-bridge = { path = "../fm-bridge" }
```

### 1.3 Existing Detection Works

The existing `BackendRegistry::detect()` already detects `apple_fm` at port 11435.
No changes needed — just enhancing the backend implementation.

---

## Part 2: Web Client Visualization Route

### 2.1 Worker Route

**File:** `crates/web/worker/src/routes/fm.rs` (NEW)

```rust
pub async fn view_fm(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>FM Bridge | OpenAgents</title>
    <meta charset="utf-8">
    <style>
        body { margin: 0; padding: 0; background: #0a0a0a; overflow: hidden; }
        canvas { display: block; width: 100vw; height: 100vh; }
    </style>
</head>
<body>
    <canvas id="canvas"></canvas>
    <script>
        window.FM_PAGE = true;
        const params = new URLSearchParams(window.location.search);
        const dataUrl = params.get("data");
        if (dataUrl) { window.FM_DATA_URL = dataUrl; }
    </script>
    <script type="module" src="/static/openagents_web_client.js"></script>
    <script type="module">
        import init, { start_demo } from "/static/openagents_web_client.js";
        init().then(() => start_demo("canvas"));
    </script>
</body>
</html>"#;
    // Return with HTML headers
}
```

**File:** `crates/web/worker/src/routes/mod.rs`
```rust
pub mod fm;
```

**File:** `crates/web/worker/src/lib.rs` (line ~232)
```rust
(Method::Get, "/fm") => routes::fm::view_fm(env).await,
```

### 2.2 Client State

**File:** `crates/web/client/src/state.rs`

Add to `AppView` enum:
```rust
pub(crate) enum AppView {
    // ... existing
    FmPage,
}
```

Add state struct:
```rust
pub(crate) struct FmVizState {
    pub(crate) frame_animator: FrameAnimator,
    pub(crate) scroll_offset: f32,
    pub(crate) content_bounds: Bounds,

    // Bridge status
    pub(crate) connection_status: ConnectionStatus,
    pub(crate) bridge_url: String,
    pub(crate) model_available: bool,
    pub(crate) ping_latency_ms: Option<u32>,

    // Token stream
    pub(crate) current_stream: Option<TokenStreamState>,
    pub(crate) tokens_per_sec: f32,
    pub(crate) ttft_ms: Option<u64>,

    // Session
    pub(crate) session_id: Option<String>,
    pub(crate) transcript: Vec<TranscriptMessage>,
    pub(crate) turn_count: u32,

    // Tools
    pub(crate) registered_tools: Vec<ToolInfo>,
    pub(crate) tool_invocations: Vec<ToolInvocation>,

    // Guided generation
    pub(crate) guided_schema: Option<String>,
    pub(crate) guided_json_partial: String,
    pub(crate) guided_fields_completed: usize,

    // Event log
    pub(crate) event_log: VecDeque<FmLogEntry>,
}
```

### 2.3 Telemetry Parser

**File:** `crates/web/client/src/fm_viz.rs` (NEW)

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum FmTelemetry {
    ConnectionStatus { status: String, bridge_url: String, latency_ms: Option<u32> },
    HealthCheck { model_available: bool, version: String },
    StreamStart { completion_id: String, model: String, is_guided: bool },
    TokenChunk { completion_id: String, delta: String, timestamp_ms: u64 },
    StreamEnd { completion_id: String, total_tokens: usize, duration_ms: u64 },
    SessionCreated { session_id: String },
    SessionMessage { session_id: String, role: String, content: String },
    ToolRegistered { session_id: String, tool_name: String },
    ToolInvoked { session_id: String, tool_name: String, timestamp_ms: u64 },
    ToolResult { session_id: String, tool_name: String, success: bool },
}

impl FmVizState {
    pub(crate) fn apply_telemetry(&mut self, event: FmTelemetry) {
        // Match and update state fields
    }
}

pub(crate) fn init_fm_viz_runtime(state: Rc<RefCell<AppState>>) {
    // Load from window.FM_DATA or fetch from window.FM_DATA_URL
}
```

### 2.4 View Rendering

**File:** `crates/web/client/src/views/fm.rs` (NEW)

```rust
pub(crate) fn build_fm_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Layout panels:
    // - Status bar (top)
    // - Token stream panel (left/center)
    // - Session timeline (right)
    // - Tool orchestration (bottom-right)
    // - Event log (bottom)
}
```

**File:** `crates/web/client/src/views/mod.rs`
```rust
mod fm;
pub(crate) use fm::build_fm_page;
```

### 2.5 Wire into App

**File:** `crates/web/client/src/app.rs` (around line 80)

```rust
let is_fm_page = web_sys::window()
    .and_then(|w| js_sys::Reflect::get(&w, &"FM_PAGE".into()).ok())
    .map(|v| v.is_truthy())
    .unwrap_or(false);

// In the view selection block:
} else if is_fm_page {
    state_guard.view = AppView::FmPage;
    init_fm_viz_runtime(state.clone());
}
```

**File:** `crates/web/client/src/lib.rs` (render loop)
```rust
match state.borrow().view {
    AppView::FmPage => build_fm_page(&mut scene, &mut text_system, &mut app_state, w, h, scale),
    // ... other views
}
```

---

## Part 3: Direct SSE Connection (Browser → FM Bridge)

### 3.1 Browser SSE Client

The browser connects directly to FM Bridge at `localhost:11435`. No worker proxy needed.

**File:** `crates/web/client/src/fm_client.rs` (NEW)

```rust
/// Browser-side FM Bridge client using fetch + ReadableStream
pub struct FmBridgeWebClient {
    base_url: String,
}

impl FmBridgeWebClient {
    pub fn new() -> Self {
        Self { base_url: "http://localhost:11435".to_string() }
    }

    /// Check bridge health
    pub async fn health(&self) -> Result<bool>

    /// Create a new session
    pub async fn create_session(&self) -> Result<String>

    /// Stream completion with SSE
    /// Returns async iterator of token chunks
    pub async fn stream_completion(&self, session_id: &str, prompt: &str)
        -> Result<impl Stream<Item = FmTelemetry>>
}
```

**SSE Parsing:**
- Use `web_sys::ReadableStream` + `wasm_streams` crate
- Parse `data: {...}` lines into `FmTelemetry` events
- Handle `[DONE]` sentinel

### 3.2 Demo Flow

1. Page loads → check bridge health → update status bar
2. User types prompt → click "Run"
3. Create session via `POST /v1/sessions`
4. Start streaming via `POST /v1/chat/completions?stream=true`
5. Parse SSE chunks → emit `FmTelemetry::TokenChunk` events
6. Update `FmVizState` → re-render token stream panel
7. On `[DONE]` → emit `FmTelemetry::StreamEnd`

### 3.3 CORS Consideration

FM Bridge (Swift) needs to send CORS headers for browser access:
- `Access-Control-Allow-Origin: *` (already present in Server.swift)
- Browser at any origin can connect to localhost:11435

---

## File Summary

| File | Action | Purpose |
|------|--------|---------|
| `crates/compute/src/backends/apple_fm.rs` | EDIT | Enhance with fm-bridge FMClient |
| `crates/compute/Cargo.toml` | EDIT | Add fm-bridge dependency |
| `crates/web/worker/src/routes/fm.rs` | CREATE | /fm route handler |
| `crates/web/worker/src/routes/mod.rs` | EDIT | Export fm module |
| `crates/web/worker/src/lib.rs` | EDIT | Wire route |
| `crates/web/client/src/state.rs` | EDIT | FmVizState, AppView::FmPage |
| `crates/web/client/src/fm_client.rs` | CREATE | Browser SSE client |
| `crates/web/client/src/fm_viz.rs` | CREATE | Telemetry parser + state updates |
| `crates/web/client/src/views/fm.rs` | CREATE | HUD rendering |
| `crates/web/client/src/views/mod.rs` | EDIT | Export fm view |
| `crates/web/client/src/app.rs` | EDIT | View routing |
| `crates/web/client/src/lib.rs` | EDIT | Render loop |

---

## Implementation Order

### Phase 1: Backend Enhancement (Pylon)
1. Add fm-bridge dependency to compute crate
2. Refactor `AppleFmBackend` to use `FMClient`
3. Add session/tool methods to backend
4. Test with `pylon doctor` / `pylon compute list`

### Phase 2: Web Route Skeleton
5. Create `/fm` worker route with prompt input UI
6. Add `AppView::FmPage` and `FmVizState`
7. Wire view routing in app.rs
8. Test page renders at `/fm` with input box

### Phase 3: Browser SSE Client
9. Create `FmBridgeWebClient` with health/session/stream methods
10. Implement SSE parsing via ReadableStream
11. Create `FmTelemetry` enum
12. Test connection to local FM Bridge

### Phase 4: State + Rendering
13. Implement `apply_telemetry()` state updates
14. Implement `build_fm_page()` with status bar
15. Add token stream panel with live rendering
16. Add session timeline panel

### Phase 5: Full Demo Flow
17. Wire prompt input → create session → stream tokens
18. Test multi-turn conversation
19. Polish animations, throughput gauge, timing stats

---

## Success Criteria

- [ ] `pylon doctor` shows `fm-bridge` as available backend
- [ ] `/fm` route renders HUD with status bar
- [ ] Token streaming visible in real-time
- [ ] Session timeline updates on multi-turn
- [ ] Tool invocations visualized
- [ ] 60fps rendering, no jank
