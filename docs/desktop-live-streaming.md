# Desktop Live Streaming Architecture

This document explains how autopilot streams events to the desktop UI in real-time.

## Overview

The live streaming feature allows autopilot to send HTML fragments to the desktop app as the agent runs, providing real-time visualization of the agent's actions.

## Architecture

```
┌─────────────┐
│  Autopilot  │
│   (main.rs) │
└──────┬──────┘
       │
       │ 1. Spawns desktop process
       │
       v
┌─────────────────┐
│   Desktop App   │
│  (desktop.rs)   │
│                 │
│  Prints port to │
│     stdout      │ ← DESKTOP_PORT=<port>
└────────┬────────┘
         │
         │ 2. Port detected
         │
         v
    ┌────────┐
    │ _ui_port │
    └────┬────┘
         │
         │ For each SdkMessage:
         │
         v
┌──────────────────────┐
│   UI Renderer        │
│  (ui_renderer.rs)    │
│                      │
│  render_sdk_message()│
│    ↓                 │
│  Returns Markup      │
└──────────┬───────────┘
           │
           │ 3. HTML fragment generated
           │
           v
┌──────────────────────┐
│  stream_to_desktop() │
│                      │
│  POST /events        │
│  Content-Type: html  │
└──────────┬───────────┘
           │
           │ 4. HTTP POST
           │
           v
┌──────────────────────┐
│  Desktop /events     │
│  handler             │
│                      │
│  broadcaster         │
│   .broadcast(html)   │
└──────────┬───────────┘
           │
           │ 5. WebSocket broadcast
           │
           v
┌──────────────────────┐
│  Browser WebSocket   │
│  client              │
│                      │
│  ws.onmessage        │
│    ↓                 │
│  DOM manipulation    │
│  - OOB swap if id    │
│  - Append to timeline│
│  - Auto-scroll       │
└──────────────────────┘
```

## Components

### 1. Desktop Launch (main.rs:498-543)

When autopilot is run with `--ui` flag:

```rust
let _ui_port: Option<u16> = if ui {
    // Spawn desktop app
    let mut child = std::process::Command::new("cargo")
        .args(["run", "--release", "-p", "desktop"])
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    // Read stdout for port
    // Looks for "DESKTOP_PORT=<port>"
    let port = detect_port_from_stdout(child.stdout);

    // Open browser
    open(format!("http://127.0.0.1:{}/autopilot", port));

    Some(port)
} else {
    None
};
```

### 2. UI Renderer (ui_renderer.rs)

Maps Codex app-server response events to recorder UI components:

```rust
pub fn render_event(event: &ResponseEvent) -> Option<Markup> {
    match event {
        ResponseEvent::AssistantMessage { .. } => {
            // Text blocks → AgentLine
            // Tool use → ToolLine with Pending state
        }
        ResponseEvent::UserMessage { .. } => {
            // Tool results → Update existing ToolLine
        }
        ResponseEvent::SystemInit { .. } => {
            // lifecycle_line(Start {...})
        }
        ResponseEvent::Complete { .. } | ResponseEvent::Error(_) => {
            // lifecycle_line(End {...})
        }
        _ => None,
    }
}
```

**Stateful Rendering:**

The `UiRenderer` struct tracks pending tool calls and generates update scripts:

```rust
let mut renderer = UiRenderer::new();

// When tool_use block arrives:
renderer.render(msg);
// → Returns ToolLine with Pending, stores (tool_id → tool_name, args)

// When tool result arrives:
renderer.render(msg);
// → Generates update script to replace pending line with completed line
```

### 3. Streaming Function (main.rs:1036-1048)

POSTs HTML to desktop `/events` endpoint:

```rust
async fn stream_to_desktop(port: u16, html: String) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/events", port);

    client
        .post(&url)
        .header("Content-Type", "text/html")
        .body(html)
        .send()
        .await?;

    Ok(())
}
```

### 4. Integration in Run Loop (main.rs:654-658)

Called after processing each SDK message:

```rust
// Collect trajectory
collector.process_message(&msg);

// Stream to desktop UI if enabled
if let Some(port) = _ui_port {
    if let Some(html) = autopilot::ui_renderer::render_sdk_message(&msg) {
        let _ = stream_to_desktop(port, html.into_string()).await;
    }
}
```

### 5. Desktop /events Handler (desktop/src/server.rs:58-62)

Receives HTML and broadcasts to all WebSocket clients:

```rust
async fn events(state: web::Data<AppState>, body: String) -> HttpResponse {
    // Broadcast the HTML fragment to all connected WebSocket clients
    state.broadcaster.broadcast(&body);
    HttpResponse::Ok().finish()
}
```

### 6. WebSocket Broadcasting (desktop/src/ws.rs)

Tokio broadcast channel sends fragments to all connected clients:

```rust
pub struct WsBroadcaster {
    sender: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn broadcast(&self, html: &str) {
        let _ = self.sender.send(html.to_string());
    }
}
```

### 7. Browser Client (desktop/src/views/autopilot.rs:63-123)

JavaScript WebSocket client receives and renders fragments:

```javascript
var ws = new WebSocket('ws://' + location.host + '/ws');

ws.onmessage = function(e) {
    var fragment = e.data;
    var tempDiv = document.createElement('div');
    tempDiv.innerHTML = fragment;
    var firstChild = tempDiv.firstElementChild;

    if (firstChild && firstChild.id) {
        // OOB swap - replace existing element
        var existing = document.getElementById(firstChild.id);
        if (existing) {
            existing.outerHTML = fragment;
            return;
        }
    }

    // Default: append to timeline
    timeline.insertAdjacentHTML('beforeend', fragment);

    // Auto-scroll
    if (autoScroll.checked) {
        window.scrollTo(0, document.body.scrollHeight);
    }
};
```

## Message Types Supported

| SDK Message Type | UI Component | Notes |
|-----------------|--------------|-------|
| `Assistant` (text) | `AgentLine` | Renders agent response text |
| `Assistant` (tool_use) | `ToolLine` (Pending) | Shows pending tool call |
| `User` (tool_result) | Update script | Replaces pending with result |
| `System::Init` | `LifecycleEvent::Start` | Session start |
| `Result::Success` | `LifecycleEvent::End` | Session end with stats |
| `Result::Error*` | `LifecycleEvent::End` | Session error with details |
| `ToolProgress` | None (future) | Placeholder for latency updates |

## Out-of-Band (OOB) Swap Pattern

When a fragment has an `id` attribute, it replaces the existing element with the same id:

```html
<!-- Initial (pending tool call) -->
<div id="tool-call-toolu_123" class="tool-line pending">
  Tool: Read → [pending]
</div>

<!-- Update (when result arrives) -->
<div id="tool-call-toolu_123" class="tool-line success">
  Tool: Read → [ok]
  <div class="result">File contents...</div>
</div>
```

This creates smooth updates without duplicating timeline entries.

## Usage

### Run autopilot with UI:

```bash
cargo run -p autopilot -- run "your prompt" --ui
```

### Environment variables:

```bash
# Enable UI by default
export AUTOPILOT_UI=true

# Run autopilot
cargo run -p autopilot -- run "your prompt"
```

### Programmatic:

```rust
use autopilot::run_task;

let result = run_task(RunArgs {
    prompt: "your prompt".to_string(),
    ui: true,  // Enable live streaming
    ..Default::default()
}).await?;
```

## Testing

1. Start autopilot with UI flag:
   ```bash
   cargo run -p autopilot -- run "list cargo.toml files" --ui
   ```

2. Desktop app should open automatically in browser

3. Watch real-time updates as autopilot:
   - Receives the prompt (UserLine)
   - Thinks and responds (AgentLine)
   - Calls tools (ToolLine → Pending)
   - Receives results (ToolLine → Success/Error)
   - Completes session (LifecycleEvent::End)

## Future Enhancements

- [ ] ToolProgress rendering (show latency updates)
- [ ] Sidebar stats updates (cumulative tokens, cost)
- [ ] Session header updates (budget meter)
- [ ] Pause/resume controls
- [ ] Replay speed controls (fast-forward/slow-motion for live sessions)
