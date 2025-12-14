# Plan: Connect Chat UI to MechaCoder Router

## Goal
Replace the markdown streaming demo in `cargo desktop` with a functional chat interface that routes messages through the mechacoder router to Claude Code (or local agent stub as fallback).

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  winit Event Loop (main thread)                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ App (coder_app)                                      │   │
│  │   ├─ ChatThread widget (messages + input)            │   │
│  │   ├─ Signal<ChatView> (reactive state)               │   │
│  │   └─ tx: mpsc::Sender<ClientMessage>                │   │
│  └─────────────────────────────────────────────────────┘   │
│              │ send message          ▲ receive ServerMessage │
│              ▼                       │                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ mpsc channels (thread-safe)                          │   │
│  │   client_tx → client_rx   server_tx → server_rx      │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
              │                       ▲
              ▼                       │
┌─────────────────────────────────────────────────────────────┐
│  Background Thread (tokio runtime)                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ MessageHandler                                       │   │
│  │   ├─ Router (detects backends)                       │   │
│  │   ├─ Claude Code session (via mechacoder)            │   │
│  │   └─ Local Agent stub (fallback)                     │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Files to Modify

### 1. `crates/coder/app/Cargo.toml`
Add dependencies:
- `mechacoder = { path = "../../mechacoder", features = ["server"] }`
- `tokio = { version = "1", features = ["rt-multi-thread", "sync"] }`
- `crossbeam-channel` or use `std::sync::mpsc`

### 2. `crates/coder/app/src/app.rs` (main changes)
- Remove markdown streaming demo code (`demo_streaming`, `demo_char_index`, `DEMO_MARKDOWN`)
- Add `ChatThread` widget
- Add message channels (`client_tx`, `server_rx`)
- Store `Signal<ChatView>` for reactive updates
- Wire `ChatInput.on_send` to send `ClientMessage` via channel
- In `update()`: poll `server_rx` for incoming `ServerMessage` and update `ChatView`

### 3. `crates/coder/app/src/lib.rs` (new)
Create `ChatHandler` struct that runs in background thread:
- Spawns tokio runtime
- Receives `ClientMessage` from UI thread
- Routes via `mechacoder::Router`
- Calls `run_claude_session()` for Claude Code backend
- Implements local agent stub for fallback
- Sends `ServerMessage` back to UI thread

### 4. `crates/coder/app/src/main.rs`
- Spawn background thread with tokio runtime before entering winit event loop
- Pass channels to `App`

## Implementation Steps

### Step 1: Add Dependencies
```toml
# crates/coder/app/Cargo.toml
mechacoder = { path = "../../mechacoder", features = ["server"] }
tokio = { version = "1", features = ["rt-multi-thread", "sync"] }
```

### Step 2: Create ChatHandler Module
Create `crates/coder/app/src/chat_handler.rs`:
```rust
pub struct ChatHandler {
    client_rx: mpsc::UnboundedReceiver<ClientMessage>,
    server_tx: mpsc::UnboundedSender<ServerMessage>,
    router: Router,
}

impl ChatHandler {
    pub fn spawn(
        client_rx: mpsc::UnboundedReceiver<ClientMessage>,
        server_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let mut handler = ChatHandler::new(client_rx, server_tx);
                handler.run().await;
            });
        })
    }

    async fn handle_message(&mut self, msg: ClientMessage) {
        match self.router.route() {
            Some(Backend::ClaudeCode) => {
                // Use run_claude_session from mechacoder
            }
            Some(Backend::Pi) | None => {
                // Local agent stub
                log::info!("[LocalAgent] Received: {}", msg.content);
                self.server_tx.send(ServerMessage::TextDelta {
                    text: format!("Local agent received: {}\n", msg.content),
                }).ok();
                self.server_tx.send(ServerMessage::Done { error: None }).ok();
            }
            _ => { /* Handle other backends */ }
        }
    }
}
```

### Step 3: Modify App Struct
Replace demo fields with chat state:
```rust
pub struct App {
    // ... existing fields ...

    // Chat state
    chat_thread: ChatThread,
    chat_view: Signal<ChatView>,

    // Message channels (for communicating with background handler)
    client_tx: mpsc::UnboundedSender<ClientMessage>,
    server_rx: mpsc::UnboundedReceiver<ServerMessage>,

    // Current working directory for messages
    cwd: String,
}
```

### Step 4: Wire Up ChatThread
In `App::new()`:
```rust
let (client_tx, client_rx) = mpsc::unbounded_channel();
let (server_tx, server_rx) = mpsc::unbounded_channel();

// Spawn background handler
ChatHandler::spawn(client_rx, server_tx);

let thread_id = ThreadId::new();
let chat_view = Signal::new(ChatView::new(thread_id));
let chat_thread = ChatThread::new(thread_id)
    .chat_view(chat_view.clone());
```

### Step 5: Handle Send Callback
Wire the `on_send` callback in `ChatThread`:
```rust
// In App, create a sender clone for the callback
let tx = self.client_tx.clone();
let cwd = self.cwd.clone();

self.chat_thread = ChatThread::new(thread_id)
    .chat_view(chat_view.clone())
    .on_send(move |content| {
        // Add user message to ChatView
        // Send to backend
        tx.send(ClientMessage::SendMessage {
            content: content.to_string(),
            cwd: cwd.clone(),
        }).ok();
    });
```

### Step 6: Poll for Responses
In `App::update()`:
```rust
fn update(&mut self) {
    // Poll for server messages
    while let Ok(msg) = self.server_rx.try_recv() {
        match msg {
            ServerMessage::TextDelta { text } => {
                // Update streaming message in ChatView
                let mut view = self.chat_view.get();
                if let Some(streaming) = &mut view.streaming_message {
                    streaming.content_so_far.push_str(&text);
                } else {
                    view.streaming_message = Some(StreamingMessage {
                        content_so_far: text,
                        is_complete: false,
                    });
                }
                self.chat_view.set(view);
            }
            ServerMessage::Done { error } => {
                // Complete the streaming message
                let mut view = self.chat_view.get();
                if let Some(streaming) = view.streaming_message.take() {
                    // Add as completed message
                    view.entries.push(ChatEntry::Message(MessageView {
                        content: streaming.content_so_far,
                        role: Role::Assistant,
                        // ...
                    }));
                }
                self.chat_view.set(view);
            }
            // Handle ToolStart, ToolResult, etc.
            _ => {}
        }
    }

    // Run scheduler
    self.scheduler.run_frame();
}
```

### Step 7: Paint ChatThread
Replace `paint_home()` with ChatThread:
```rust
fn paint(&mut self, scene: &mut Scene, text_system: &mut TextSystem) {
    // ... background, chrome ...

    let content_bounds = self.chrome.content_bounds(bounds);
    self.chat_thread.paint(content_bounds, &mut cx);
}
```

### Step 8: Handle Events
Forward events to ChatThread:
```rust
fn handle_event(&mut self, event: &InputEvent) -> EventResult {
    // ... chrome events ...

    let result = self.chat_thread.event(event, content_bounds, &mut cx);
    if result.is_handled() {
        return result;
    }

    EventResult::Ignored
}
```

## Local Agent Stub Implementation
For the local agent (when Claude Code is not available):
```rust
async fn run_local_agent(
    message: String,
    tx: mpsc::UnboundedSender<ServerMessage>,
) {
    log::info!("[LocalAgent] Received message: {}", message);

    // Simulate streaming response
    let response = format!(
        "Local agent stub received your message: \"{}\"\n\n\
         Claude Code is not available. Install it with: npm install -g @anthropic/claude-code",
        message
    );

    for chunk in response.chars().collect::<Vec<_>>().chunks(10) {
        let text: String = chunk.iter().collect();
        tx.send(ServerMessage::TextDelta { text }).ok();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    tx.send(ServerMessage::Done { error: None }).ok();
}
```

## Testing Checklist
- [ ] `cargo desktop` opens with chat UI (text input at bottom, messages at top)
- [ ] Typing and sending a message works
- [ ] Message appears in chat view
- [ ] If Claude Code available: response streams back
- [ ] If Claude Code unavailable: local agent stub responds
- [ ] Console logs show routing decisions

## Files Summary
| File | Action |
|------|--------|
| `crates/coder/app/Cargo.toml` | Add mechacoder, tokio deps |
| `crates/coder/app/src/chat_handler.rs` | **New** - background message handler |
| `crates/coder/app/src/app.rs` | Replace demo with ChatThread |
| `crates/coder/app/src/main.rs` | Minor - spawn handler before event loop |
| `crates/coder/app/src/lib.rs` | Export chat_handler module |
