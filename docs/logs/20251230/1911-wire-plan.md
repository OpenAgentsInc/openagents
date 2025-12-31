# Plan: Wire HUD to WebSocket for GTM

## Context

Backend infrastructure is complete:
- ✅ `autopilot-container` crate with Claude SDK streaming
- ✅ `AutopilotContainer` Durable Object
- ✅ Container routes (`/api/container/start`, `/api/container/status`, `/api/container/ws/`)
- ✅ Tunnel routes (`/api/tunnel/register`, `/api/tunnel/status`, `/api/tunnel/ws/`)
- ✅ HUD routes (`/hud/:username/:repo`, `/embed/:username/:repo`)
- ✅ HUD settings (public/private, embed_allowed)

**Missing:** The HUD page serves an HTML shell but doesn't connect to the WebSocket to display streaming content.

## Goal

Make the HUD actually work: connect to container/tunnel WebSocket and render streaming events.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  /hud/:username/:repo                                                │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │  HUD HTML (from serve_hud_html)                                  │ │
│  │  └── Loads WASM client                                          │ │
│  │      └── Connects to /api/container/ws/:session_id              │ │
│  │          └── Receives streaming events                          │ │
│  │              └── Renders in ThreadView pane                     │ │
│  └─────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Update HUD HTML to Include Session Info

**File:** `crates/web/worker/src/routes/hud.rs`

The HUD context needs to include the active session info so the client knows which WebSocket to connect to.

```rust
#[derive(Serialize)]
struct HudContext {
    username: String,
    repo: String,
    is_owner: bool,
    is_public: bool,
    embed_mode: bool,
    // NEW: Session info
    session_id: Option<String>,
    ws_url: Option<String>,
    status: String, // "idle", "running", "completed"
}
```

Query for active session in `view_hud`:
```rust
// Check for active session for this repo
let session = kv.get(&format!("session:{}:{}", user_id, repo)).await?;
```

### Step 2: Add Session Query Endpoint

**File:** `crates/web/worker/src/routes/hud.rs`

New endpoint to get/create session for a repo:

```rust
// GET /api/hud/session?repo=owner/repo
pub async fn get_session(user: AuthenticatedUser, env: Env, repo: String) -> Result<Response> {
    // Check for existing active session
    // If none, return { status: "idle", can_start: true }
    // If running, return { status: "running", session_id, ws_url }
}

// POST /api/hud/start
pub async fn start_session(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
    // Parse { repo, prompt }
    // Create new session via container routes
    // Return { session_id, ws_url }
}
```

### Step 3: Update WASM Client to Connect WebSocket

**File:** `crates/web/client/src/lib.rs`

The client needs to:
1. Read `window.HUD_CONTEXT`
2. If session is running, connect to WebSocket
3. Render incoming events in the center pane

```rust
// In start_demo or equivalent:
if let Some(ws_url) = context.ws_url {
    let ws = WebSocket::new(&ws_url)?;
    ws.set_onmessage(|event| {
        let msg: TaskEvent = serde_json::from_str(&event.data())?;
        // Dispatch to ThreadView
    });
}
```

### Step 4: Create ThreadView Component

**File:** `crates/web/client/src/views/thread_view.rs` (or within lib.rs)

Component to render streaming events:
- `chunk` → Append to message text
- `tool_start` → Show tool card (collapsed)
- `tool_done` → Update tool card with result
- `tool_progress` → Update progress indicator
- `usage` → Show in status bar
- `done` → Mark complete
- `error` → Show error message

### Step 5: Add "Start" Button

When no session is running, show a form:
- Text input for prompt
- "Start Autopilot" button
- On submit: POST to `/api/hud/start`

## Files to Modify

| File | Changes |
|------|---------|
| `crates/web/worker/src/routes/hud.rs` | Add session query, start endpoints, update context |
| `crates/web/worker/src/lib.rs` | Add new HUD routes |
| `crates/web/client/src/lib.rs` | WebSocket connection, event handling |

## Files to Create

| File | Purpose |
|------|---------|
| `crates/web/client/src/views/thread_view.rs` | ThreadView component (optional, can be inline) |

## Event Types (from autopilot-container)

```rust
pub enum TaskEvent {
    Status { task_id: String, status: String },
    Chunk { task_id: String, text: String },
    ToolStart { task_id: String, tool_name: String, tool_id: String, params: Value },
    ToolDone { task_id: String, tool_id: String, output: String, is_error: bool },
    ToolProgress { task_id: String, tool_id: String, elapsed_secs: f32 },
    Usage { task_id: String, input_tokens: u64, output_tokens: u64, total_cost_usd: f64 },
    Done { task_id: String, summary: String },
    Error { task_id: String, error: String },
}
```

## Execution Order

1. **Add session endpoints** to worker routes
2. **Update HUD context** with session info
3. **Update WASM client** to connect WebSocket
4. **Render events** in ThreadView
5. **Add start form** when idle
6. **Test end-to-end**

## Success Criteria

- Visit `/hud/username/repo`
- See "Start Autopilot" form
- Enter prompt, click start
- See streaming text and tool events
- See completion summary
