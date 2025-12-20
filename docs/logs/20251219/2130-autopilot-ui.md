# Autopilot UI Mode: Live Recorder Visualization

## Overview

Add `--ui` flag to autopilot that launches the desktop app and streams recorder components in real-time as the agent works.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  autopilot --ui --full-auto "Start working"             │
│  ├─ Spawns desktop app (subprocess)                     │
│  ├─ Runs query() as normal                              │
│  └─ For each SDK message:                               │
│       1. Process as normal (trajectory collector)       │
│       2. Render as recorder component (Maud HTML)       │
│       3. POST fragment to http://127.0.0.1:PORT/events  │
└─────────────────────────────────────────────────────────┘
              ↓ (HTTP POST)
┌─────────────────────────────────────────────────────────┐
│  Desktop App (127.0.0.1:PORT)                           │
│  ├─ /events endpoint receives HTML fragments            │
│  ├─ WsBroadcaster broadcasts to WebSocket clients       │
│  └─ /autopilot page shows live recorder timeline        │
└─────────────────────────────────────────────────────────┘
              ↓ (WebSocket)
┌─────────────────────────────────────────────────────────┐
│  Webview (native window)                                │
│  ├─ Connects to ws://127.0.0.1:PORT/ws                  │
│  ├─ Receives HTML fragments                             │
│  └─ Appends recorder lines to timeline                  │
└─────────────────────────────────────────────────────────┘
```

## Issues to Create

### Issue 1: Add `/autopilot` route to desktop app
**Priority:** high
**Files:** `crates/desktop/src/server.rs`, `crates/desktop/src/views/autopilot.rs` (new)

- Add route `GET /autopilot` that renders a page with:
  - Session header (placeholder until first message)
  - Empty timeline container `<div id="timeline"></div>`
  - WebSocket connection script
  - CSS for recorder components

- Add route `POST /events` that:
  - Receives HTML fragment in body
  - Broadcasts via WsBroadcaster
  - Returns 200 OK

- Modify WebSocket JS to APPEND fragments (not replace):
```javascript
ws.onmessage = function(e) {
    document.getElementById('timeline').insertAdjacentHTML('beforeend', e.data);
    // Auto-scroll to bottom
    window.scrollTo(0, document.body.scrollHeight);
};
```

### Issue 2: Add `--ui` flag to autopilot CLI
**Priority:** high
**Files:** `crates/autopilot/src/main.rs`

- Add `#[arg(long)] ui: bool` to Run command
- Add `AUTOPILOT_UI` env var support
- When ui=true:
  1. Spawn `cargo run -p desktop` as subprocess
  2. Wait for server to be ready (poll /health endpoint)
  3. Store port number for sending events
  4. Open browser/webview to `/autopilot` page

### Issue 3: Create recorder component renderer for SDK messages
**Priority:** high
**Files:** `crates/autopilot/src/ui_renderer.rs` (new)

Map SdkMessage types to recorder components:

| SdkMessage | Recorder Component |
|------------|-------------------|
| Assistant (text) | `AgentLine::new(text).build()` |
| Assistant (tool_use) | `ToolLine::new(name, args, Pending).build()` |
| User (tool_result) | Update ToolLine with result |
| ToolProgress | Update ToolLine latency |
| System | `lifecycle_line(Start/Pause/etc)` |
| Result | `lifecycle_line(End)` |

Use builder pattern from ui crate:
```rust
use ui::recorder::organisms::ToolLine;
use ui::recorder::molecules::ResultType;

fn render_tool_call(name: &str, args: &str, id: &str) -> String {
    ToolLine::new(name, args, ResultType::Pending)
        .call_id(id)
        .build()
        .into_string()
}
```

### Issue 4: Stream rendered components to desktop app
**Priority:** high
**Files:** `crates/autopilot/src/main.rs`, `crates/autopilot/src/ui_renderer.rs`

In the main message processing loop, after `collector.process_message(&msg)`:

```rust
if ui_enabled {
    let html = ui_renderer::render_message(&msg);
    if !html.is_empty() {
        // POST to desktop app
        reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/events", ui_port))
            .body(html)
            .send()
            .await?;
    }
}
```

### Issue 5: Add session header and sidebar components
**Priority:** medium
**Files:** `crates/desktop/src/views/autopilot.rs`

- Render initial page with:
  - SessionHeader (model, prompt preview, mode)
  - Sidebar with budget_meter, cost_accumulator
  - Main timeline area

- Add route `POST /session/update` to update header/sidebar:
  - Update cost as it accumulates
  - Update turn count
  - Show token usage

### Issue 6: Handle tool result matching
**Priority:** medium
**Files:** `crates/autopilot/src/ui_renderer.rs`

Tool calls and results come as separate messages. Need to:
- Track pending tool calls by ID
- When result arrives, update the ToolLine:
  ```rust
  // Find element by call_id and update
  let update_html = format!(
      r#"<script>
          var el = document.querySelector('[data-call-id="{}"]');
          if (el) el.outerHTML = '{}';
      </script>"#,
      call_id,
      rendered_complete_tool_line
  );
  ```

### Issue 7: Add expand/collapse for tool details
**Priority:** low
**Files:** `crates/ui/src/recorder/organisms/tool_line.rs`

- ToolLine already supports `.expanded(bool)`
- Add HTMX or vanilla JS click handler to toggle
- Collapsed: show preview only
- Expanded: show full input/output

### Issue 8: Add auto-scroll toggle
**Priority:** low
**Files:** `crates/desktop/src/views/autopilot.rs`

- Add toggle button for auto-scroll behavior
- When enabled: scroll to bottom on new message
- When disabled: stay at current position
- Remember preference in localStorage

## Implementation Order

1. **Issue 1** - Desktop app routes (foundation)
2. **Issue 3** - Renderer (can test in isolation)
3. **Issue 2** - CLI flag (connects 1 & 3)
4. **Issue 4** - Streaming integration
5. **Issue 5** - Session header/sidebar
6. **Issue 6** - Tool result matching
7. **Issue 7** - Expand/collapse
8. **Issue 8** - Auto-scroll toggle

## Files to Modify/Create

| File | Action |
|------|--------|
| `crates/autopilot/src/main.rs` | Add --ui flag, spawn desktop, send events |
| `crates/autopilot/src/ui_renderer.rs` | NEW: Map SDK messages to recorder HTML |
| `crates/desktop/src/server.rs` | Add /autopilot and /events routes |
| `crates/desktop/src/views/autopilot.rs` | NEW: Autopilot viewer page |
| `crates/desktop/src/views/mod.rs` | Export autopilot module |
| `crates/ui/src/lib.rs` | Ensure recorder components exported |

## Testing

1. Run `cargo run -p autopilot -- run --ui "Hello"`
2. Desktop should open with autopilot page
3. Timeline should show messages as they stream
4. Tool calls should update when results arrive
5. Session header should show accurate metrics

## Notes

- Desktop app already has WebSocket broadcasting infrastructure
- Recorder components already have full builder pattern API
- Main work is connecting autopilot → desktop → webview pipeline
- Keep CLI output working alongside UI (not either/or)
