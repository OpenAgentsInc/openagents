# Plan: Pylon MVP - Complete NIP-90 + NIP-28 Implementation

**Goal:** Complete the Pylon MVP demo flow - serve inference, earn credits, request inference, chat with providers.

---

## Current State (After Exploration)

### ✅ Already Working
- Native window (winit + wgpu)
- FM Bridge integration (Apple FM)
- Nostr connection with NIP-42 auth to `wss://relay.openagents.com/`
- **NIP-90 Provider Loop**: Job request → FM inference → Publish result → Credits++
- Split-panel Bloomberg UI (jobs + chat)
- Chat message display
- Basic viz (status, credits, token rail)

### ❌ Not Working / Missing
1. **NIP-90 Client Mode** - Can't request inference from others
2. **NIP-28 Channel Setup** - No channel creation (kind 40), hardcoded channel ID
3. **Chat Tag Format** - Missing relay URL in "e" tags (NIP-28 non-compliant)
4. **Job Result Reception** - Don't handle incoming 6050 when we're requester
5. **Demo Polish** - Need the full demo flow working end-to-end

---

## Implementation Plan

### Step 1: NIP-90 Client Mode (Request Inference)

**Files:** `nostr_runtime.rs`, `app.rs`, `state.rs`

When user presses Cmd+Enter in Prompt panel (while connected):
1. Publish kind 5050 job request with prompt
2. Track as "our request" in state
3. Decrement credits
4. Wait for 6050 result from any provider

**Changes:**

`state.rs` - Add tracking for our requests:
```rust
pub pending_requests: HashMap<String, PendingRequest>,  // event_id -> request info
pub struct PendingRequest {
    pub prompt: String,
    pub requested_at: u64,
}
```

`nostr_runtime.rs` - Subscribe to results for our pubkey:
```rust
// After auth, also subscribe to results addressed to us
async fn handle_subscribe_our_results(relay: &RelayConnection, our_pubkey: &str) {
    let filter = serde_json::json!({
        "kinds": [6050],
        "#p": [our_pubkey],  // Results tagged to us
        "limit": 50
    });
    let _ = relay.subscribe("our_results", &[filter]).await;
}
```

`app.rs` - Handle Cmd+Enter to request:
```rust
// In handle_prompt_input, on Cmd+Enter:
if !state.fm_state.prompt_input.is_empty() && state.fm_state.credits > 0 {
    state.nostr_runtime.publish_job_request(&state.fm_state.prompt_input);
    state.fm_state.credits -= 1;
    state.fm_state.jobs_requested += 1;
    // Track pending request
    state.fm_state.pending_requests.insert(event_id, PendingRequest { ... });
}
```

`app.rs` - Handle incoming job results:
```rust
NostrEvent::JobResult { id, request_id, pubkey, content } => {
    // Check if this is a response to our request
    if state.fm_state.pending_requests.remove(&request_id).is_some() {
        // Display in token stream
        state.fm_state.token_stream = content;
        state.fm_state.stream_status = FmStreamStatus::Complete;
    }
}
```

---

### Step 2: NIP-28 Channel Creation

**Files:** `nostr_runtime.rs`, `app.rs`

Create/find the `#openagents-providers` channel on startup.

**Changes:**

`nostr_runtime.rs` - Add channel creation:
```rust
pub enum NostrCommand {
    // ... existing
    CreateOrFindChannel { name: String },
}

async fn handle_create_channel(relay: &RelayConnection, event_tx: &mpsc::Sender<NostrEvent>, secret_key: &[u8; 32], name: &str) {
    // First, query for existing channel with this name
    let filter = serde_json::json!({
        "kinds": [40],
        "limit": 100
    });
    // If found, use that channel_id
    // If not found, create kind 40 event:
    let metadata = serde_json::json!({
        "name": name,
        "about": "Provider chat for the OpenAgents inference network",
        "relays": ["wss://relay.openagents.com/"]
    });
    let template = EventTemplate {
        kind: 40,
        content: metadata.to_string(),
        tags: vec![],
        created_at: now(),
    };
    // Publish and store channel_id
}
```

`state.rs` - Store channel ID:
```rust
pub channel_id: Option<String>,  // The actual event ID of the channel
```

---

### Step 3: Fix Chat Message Tags

**File:** `nostr_runtime.rs:389-426`

Current (broken):
```rust
tags: vec![
    vec!["e".to_string(), channel_id.to_string(), String::new(), "root".to_string()],
]
```

Fixed (NIP-28 compliant):
```rust
tags: vec![
    vec!["e".to_string(), channel_id.to_string(), "wss://relay.openagents.com/".to_string(), "root".to_string()],
]
```

---

### Step 4: Add Self-Message Echo

**Files:** `nostr_runtime.rs`, `app.rs`

When we send a chat message, add it to our local state immediately (optimistic UI).

`app.rs` - After sending chat:
```rust
// After successful publish, add to local messages
let msg = ChatMessage {
    id: event_id,
    author: state.fm_state.pubkey.clone().unwrap_or_default(),
    content: content.clone(),
    timestamp: now(),
    is_self: true,
};
state.fm_state.add_chat_message(msg);
```

---

### Step 5: Improve Job Display

**File:** `ui/jobs_panel.rs`

Show whether job is incoming (we serve) or outgoing (we requested):
- `▶ SERVING` - We're processing this job
- `✓ SERVED +1` - We completed serving
- `◀ REQUESTED` - We requested this
- `✓ RECEIVED` - We got the result

---

### Step 6: Demo Flow Verification

Create test script to verify full flow:

1. **Launch** → Status shows "ONLINE", pubkey displayed
2. **Auto-join chat** → Subscribe to #openagents-providers
3. **Receive job** → Job appears in left panel, FM starts streaming
4. **Complete job** → Result published, credits++, job shows ✓
5. **Send chat** → "just served my first job!" appears in chat
6. **Request inference** → Cmd+Enter sends job request, credits--
7. **Receive result** → Another provider's response streams in

---

## All Files to Modify

| File | Changes |
|------|---------|
| `crates/pylon-desktop/Cargo.toml` | Add clap dependency |
| `crates/pylon-desktop/src/main.rs` | Add CLI arg parsing, dual-mode entry |
| `crates/pylon-desktop/src/core.rs` | **NEW** - Shared PylonCore struct |
| `crates/pylon-desktop/src/cli.rs` | **NEW** - Headless CLI mode runner |
| `crates/pylon-desktop/src/state.rs` | Add `pending_requests`, `channel_id` |
| `crates/pylon-desktop/src/nostr_runtime.rs` | Add result subscription, channel creation, fix chat tags |
| `crates/pylon-desktop/src/app.rs` | Use PylonCore, handle Cmd+Enter, handle JobResult, self-echo |
| `crates/pylon-desktop/src/ui/jobs_panel.rs` | Show incoming vs outgoing jobs |

---

## Demo Script Checklist

- [ ] Launch Pylon → see "ONLINE" status with pubkey
- [ ] Channel auto-created/joined → #openagents-providers in header
- [ ] Serve job from another user → FM streams, result published
- [ ] Credit increments → visible in header
- [ ] Chat message sent → appears immediately (self-echo)
- [ ] Cmd+Enter → request inference (credit decrements)
- [ ] Receive result → streams in token panel
- [ ] Tab between panels → focus indicator moves
- [ ] CLI mode serves jobs headlessly

---

## Step 7: Dual-Mode Operation (CLI + GUI)

**Goal:** Pylon works as both GUI app and headless CLI provider.

### Current Architecture (Good News)
The runtimes are **already 100% GUI-independent**:
- `FmRuntime` - tokio-based, channel-driven
- `NostrRuntime` - tokio-based, channel-driven
- `FmVizState` - pure state struct, no GUI deps
- `BridgeManager` - pure subprocess management

Only `RenderState` and `app.rs` are GUI-coupled.

### Changes Required

**A. Extract SharedAppState** (new file: `src/core.rs`)
```rust
pub struct PylonCore {
    pub bridge: BridgeManager,
    pub state: FmVizState,
    pub fm: FmRuntime,
    pub nostr: NostrRuntime,
}

impl PylonCore {
    pub fn new() -> Self { /* init runtimes */ }
    pub fn poll(&mut self) { /* process events */ }
    pub fn run_headless(&mut self) { /* CLI event loop */ }
}
```

**B. Refactor main.rs** (dual entry point)
```rust
use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Run as headless CLI provider
    #[arg(long)]
    cli: bool,

    /// Serve on specific port (implies --cli)
    #[arg(long)]
    serve: Option<u16>,
}

fn main() {
    let args = Args::try_parse().unwrap_or_default();

    if args.cli || args.serve.is_some() {
        run_cli_mode(args);
    } else {
        run_gui_mode();
    }
}
```

**C. CLI Mode** (new file: `src/cli.rs`)
```rust
pub fn run_cli_mode(args: Args) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut core = PylonCore::new();

        println!("Pylon provider starting...");
        println!("Pubkey: {}", core.state.pubkey.unwrap_or_default());

        // Headless event loop
        loop {
            core.poll();

            // Log job activity
            if let Some(job) = core.state.current_job_id.as_ref() {
                println!("Serving job: {}", job);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
}
```

**D. Cargo.toml** (add clap)
```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
```

### Usage

```bash
# GUI mode (default)
pylon-desktop

# CLI provider mode
pylon-desktop --cli

# With specific relay
pylon-desktop --cli --relay wss://relay.openagents.com

# Headless API server (future)
pylon-desktop --serve 8080
```

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/core.rs` | **NEW** - Shared PylonCore struct |
| `src/cli.rs` | **NEW** - CLI mode runner |
| `src/main.rs` | Add arg parsing, mode detection |
| `src/app.rs` | Use PylonCore instead of inline init |
| `Cargo.toml` | Add clap dependency |

---

## Order of Implementation

1. **Step 3: Fix chat tags** (5 min) - Quick fix, unblocks chat
2. **Step 4: Self-echo** (10 min) - Better chat UX
3. **Step 1: NIP-90 client** (30 min) - Core feature
4. **Step 2: Channel creation** (20 min) - Proper NIP-28
5. **Step 5: Job display** (15 min) - Polish
6. **Step 7: CLI mode** (30 min) - Extract core, add CLI
7. **Step 6: E2E test** (20 min) - Verify both modes
