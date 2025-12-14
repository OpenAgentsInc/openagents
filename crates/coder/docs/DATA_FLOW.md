# Data Flow: How Information Moves Through Coder

This document traces how data flows through the entire Coder stack, from user input through backend processing and back to the UI.

## Overview

Coder uses **unidirectional data flow**: events flow down from the backend, commands flow up from the UI.

```
┌─────────────────────────────────────────────────┐
│ Backend (Event Source)                          │
│ - Generates DomainEvents                        │
│ - Sends via ServerMessage::Events               │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ Protocol Layer (WebSocket/HTTP)                 │
│ - Serializes events to JSON                     │
│ - Streams over wire                             │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ AppState (Event Application)                    │
│ - Receives events from protocol                 │
│ - Applies to projections (ChatView, etc.)       │
│ - Updates signals                               │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ Reactive Runtime (Signals/Effects)              │
│ - Signal updates trigger subscribers            │
│ - Effects re-run                                │
│ - Memos recompute if dirty                      │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ Scheduler (Frame Cycle)                         │
│ - Batches reactive updates                      │
│ - Runs in Update phase                          │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ Widget Tree (Paint)                             │
│ - Widgets read signals                          │
│ - Generate draw commands                        │
│ - Submit to wgpui                               │
└─────────────┬───────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────┐
│ GPU (Render)                                    │
│ - Execute draw commands                         │
│ - Display on screen                             │
└─────────────────────────────────────────────────┘

              ▲
              │
┌─────────────┴───────────────────────────────────┐
│ User Input → Commands → Backend                 │
└─────────────────────────────────────────────────┘
```

## Example Flows

### Flow 1: User Sends Message

Let's trace a complete round-trip when the user sends a chat message.

#### Step 1: User Input

User types "Hello, world!" and presses Enter in the `ChatInput` widget.

```rust
// coder/surfaces_chat/src/input.rs

impl Widget for ChatInput {
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::KeyDown { key: Key::Enter, modifiers } if !modifiers.shift => {
                let content = self.text.clone();
                let thread_id = self.thread_id;

                // Dispatch command
                cx.commands.dispatch(Command::SendMessage {
                    thread_id,
                    content,
                });

                // Clear input
                self.text.clear();
                self.cursor = 0;

                return EventResult::Handled;
            }
            _ => EventResult::Unhandled,
        }
    }
}
```

**Output**: `Command::SendMessage` queued in `CommandBus`.

#### Step 2: Command Processing

In the next frame's **Update** phase, the scheduler processes queued commands.

```rust
// coder/app/src/app.rs

impl App {
    fn update(&mut self) {
        // Process commands
        self.commands.process();

        // ... other update logic
    }
}
```

The registered command handler sends the message to the backend:

```rust
// coder/app/src/state.rs

impl AppState {
    fn handle_send_message(&mut self, cmd: &Command) -> CommandResult {
        if let Command::SendMessage { thread_id, content } = cmd {
            // Send to backend via protocol
            self.protocol.send(ClientMessage::SendMessage {
                thread_id: *thread_id,
                content: content.clone(),
            });

            CommandResult::Success
        } else {
            CommandResult::Error("Invalid command".into())
        }
    }
}
```

**Output**: `ClientMessage::SendMessage` sent over WebSocket.

#### Step 3: Backend Processing

Backend receives the client message, validates it, and generates domain events.

```rust
// Backend (pseudo-code)

fn handle_client_message(msg: ClientMessage) {
    match msg {
        ClientMessage::SendMessage { thread_id, content } => {
            // Validate
            if content.trim().is_empty() {
                return ServerMessage::Error {
                    code: ErrorCode::InvalidInput,
                    message: "Message cannot be empty".into(),
                };
            }

            // Generate events
            let message_id = MessageId::new();
            let events = vec![
                DomainEvent::MessageAdded {
                    thread_id,
                    message_id,
                    content: content.clone(),
                    role: Role::User,
                    tool_uses: SmallVec::new(),
                    timestamp: Utc::now(),
                },
            ];

            // Emit events
            emit_events(events);

            // Trigger AI response (async)
            spawn_assistant_response(thread_id, message_id);
        }
    }
}
```

**Output**: `DomainEvent::MessageAdded` emitted.

#### Step 4: Event Streaming

Backend sends events back to client:

```rust
// Backend (pseudo-code)

fn emit_events(events: Vec<DomainEvent>) {
    let msg = ServerMessage::Events {
        events: events.clone(),
    };

    // Send to connected client
    websocket.send(serde_json::to_string(&msg).unwrap());
}
```

**Output**: `ServerMessage::Events` sent over WebSocket.

#### Step 5: Event Reception

Client receives the server message:

```rust
// coder/app/src/state.rs

impl AppState {
    fn handle_server_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::Events { events } => {
                for event in events {
                    self.apply_event(event);
                }
            }
            ServerMessage::Error { code, message } => {
                eprintln!("Server error: {:?}: {}", code, message);
            }
            _ => {}
        }
    }
}
```

**Output**: Events queued for application.

#### Step 6: Event Application

AppState applies events to projections:

```rust
// coder/app/src/state.rs

impl AppState {
    fn apply_event(&mut self, event: DomainEvent) {
        match &event {
            DomainEvent::MessageAdded { thread_id, .. } => {
                // Get or create chat view signal
                let view_signal = self.threads
                    .entry(*thread_id)
                    .or_insert_with(|| Signal::new(ChatView::new(*thread_id)));

                // Update the projection
                view_signal.update(|view| {
                    view.apply(&event);
                });
            }
            // Handle other event types...
            _ => {}
        }
    }
}
```

**Output**: `ChatView` projection updated, signal notified.

#### Step 7: Reactive Update

Signal update triggers subscribed effects:

```rust
// The ChatThread widget has an effect that re-renders when chat_view changes

Effect::new(move || {
    let view = chat_view.get(); // ← Subscribes to chat_view signal

    // Trigger re-paint on next frame
    request_repaint();
});
```

When `chat_view.update()` is called in step 6, this effect is notified and queued for re-run.

#### Step 8: Frame Cycle

In the next frame, the scheduler runs the **Update** phase:

```rust
// coder/ui_runtime/src/scheduler.rs

impl Scheduler {
    pub fn frame(&mut self) {
        self.run_phase(Phase::Update); // ← Effects run here
        self.run_phase(Phase::Build);
        self.run_phase(Phase::Layout);
        self.run_phase(Phase::Paint);
        self.run_phase(Phase::Render);
    }
}
```

The effect from step 7 re-runs, reading the updated `chat_view` signal.

#### Step 9: Widget Paint

In the **Paint** phase, widgets render themselves:

```rust
// coder/surfaces_chat/src/thread.rs

impl Widget for ChatThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let view = self.chat_view.get(); // ← Reads updated view

        // Virtual scrolling
        let visible_start = (self.scroll_offset.get() / self.item_height) as usize;
        let visible_count = (bounds.height / self.item_height) as usize + 2;

        for i in visible_start..visible_start + visible_count {
            if let Some(entry) = view.entries.get(i) {
                let y = i as f32 * self.item_height - self.scroll_offset.get();
                let item_bounds = Bounds::new(bounds.x, bounds.y + y, bounds.width, self.item_height);

                match entry {
                    ChatEntry::Message(msg) => {
                        MessageBubble::new(msg.clone()).paint(item_bounds, cx);
                    }
                    ChatEntry::ToolUse(tool) => {
                        ToolUseIndicator::new(tool.clone()).paint(item_bounds, cx);
                    }
                }
            }
        }
    }
}
```

The new message appears in the chat thread!

#### Step 10: GPU Render

In the **Render** phase, wgpui submits draw commands to the GPU:

```rust
// wgpui/src/renderer.rs

impl Renderer {
    pub fn render(&mut self, scene: &Scene) {
        // Build command buffer
        let mut encoder = self.device.create_command_encoder(...);

        // Render quads (backgrounds, borders)
        self.render_quads(&scene.quads, &mut encoder);

        // Render text
        self.render_text(&scene.text_runs, &mut encoder);

        // Render images
        self.render_images(&scene.images, &mut encoder);

        // Submit to GPU
        self.queue.submit(Some(encoder.finish()));

        // Present to screen
        self.surface.present();
    }
}
```

**Result**: The new message is visible on screen!

---

### Flow 2: Streaming Assistant Response

When the assistant responds, messages stream in character-by-character.

#### Backend Streams Events

```rust
// Backend (pseudo-code)

async fn generate_assistant_response(thread_id: ThreadId, user_message_id: MessageId) {
    let message_id = MessageId::new();

    // Start message
    emit_event(DomainEvent::MessageAdded {
        thread_id,
        message_id,
        content: "".into(),
        role: Role::Assistant,
        tool_uses: SmallVec::new(),
        timestamp: Utc::now(),
    });

    // Stream content
    let mut content = String::new();
    for chunk in ai_stream().await {
        content.push_str(&chunk);

        emit_event(DomainEvent::MessageStreaming {
            thread_id,
            message_id,
            content: content.clone(),
            timestamp: Utc::now(),
        });

        // Throttle: send updates every 100ms
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Complete
    emit_event(DomainEvent::MessageComplete {
        thread_id,
        message_id,
        final_content: content,
        timestamp: Utc::now(),
    });
}
```

#### Frontend Applies Streaming Events

```rust
// coder/domain/src/projections/chat_view.rs

impl ChatView {
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MessageStreaming { message_id, content, .. } => {
                // Update streaming state
                self.streaming_message = Some((*message_id, content.clone()));
            }

            DomainEvent::MessageComplete { message_id, final_content, .. } => {
                // Clear streaming state
                self.streaming_message = None;

                // Update message in entries
                for entry in &mut self.entries {
                    if let ChatEntry::Message(msg) = entry {
                        if msg.id == *message_id {
                            msg.content = final_content.clone();
                            break;
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
```

#### Render Streaming Message

```rust
// coder/surfaces_chat/src/thread.rs

impl Widget for ChatThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let view = self.chat_view.get();

        // Render regular entries
        for entry in view.visible_entries(visible_start, visible_count) {
            // ... render entry
        }

        // Render streaming message
        if let Some((message_id, content)) = &view.streaming_message {
            let y = view.entries.len() as f32 * self.item_height - self.scroll_offset.get();
            let bounds = Bounds::new(bounds.x, bounds.y + y, bounds.width, self.item_height);

            MessageBubble::new_streaming(content.clone()).paint(bounds, cx);

            // Typing indicator
            TypingIndicator::new().paint(
                Bounds::new(bounds.x + 10.0, bounds.y + bounds.height - 20.0, 50.0, 10.0),
                cx,
            );
        }
    }
}
```

**Result**: Message streams in smoothly with typing indicator!

---

### Flow 3: Tool Use Execution

When the assistant uses a tool, multiple events track its progress.

#### Backend Executes Tool

```rust
// Backend (pseudo-code)

async fn execute_tool(tool_use_id: ToolUseId, run_id: RunId, message_id: MessageId, tool_name: String, input: Value) {
    // Start
    emit_event(DomainEvent::ToolUseStarted {
        tool_use_id,
        run_id,
        message_id,
        tool_name: tool_name.clone(),
        input: input.clone(),
        timestamp: Utc::now(),
    });

    let start_time = Utc::now();

    // Execute (with progress updates)
    let result = match tool_name.as_str() {
        "read_file" => {
            let path = input["path"].as_str().unwrap();
            match tokio::fs::read_to_string(path).await {
                Ok(content) => ToolOutput::Text(content),
                Err(e) => {
                    emit_event(DomainEvent::ToolUseFailed {
                        tool_use_id,
                        error: e.to_string(),
                        timestamp: Utc::now(),
                    });
                    return;
                }
            }
        }
        "search_code" => {
            // Progress updates
            for progress in [0.25, 0.5, 0.75] {
                emit_event(DomainEvent::ToolUseProgress {
                    tool_use_id,
                    progress,
                    status_message: format!("Searching... {}%", (progress * 100.0) as u32),
                    timestamp: Utc::now(),
                });
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            // ... actual search logic
            ToolOutput::Json(search_results)
        }
        _ => ToolOutput::Empty,
    };

    // Complete
    let duration_ms = (Utc::now() - start_time).num_milliseconds() as u64;
    emit_event(DomainEvent::ToolUseComplete {
        tool_use_id,
        result,
        duration_ms,
        timestamp: Utc::now(),
    });
}
```

#### Frontend Renders Tool Progress

```rust
// coder/surfaces_chat/src/tool_use.rs

impl Widget for ToolUseIndicator {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background based on status
        let bg = match self.status {
            ToolStatus::Pending => theme::bg::TOOL_PENDING,
            ToolStatus::Running => theme::bg::TOOL_RUNNING,
            ToolStatus::Success => theme::bg::TOOL_SUCCESS,
            ToolStatus::Failed => theme::bg::TOOL_FAILED,
            ToolStatus::Cancelled => theme::bg::TOOL_CANCELLED,
        };

        cx.scene.draw_quad(bounds, bg, None, 4.0);

        // Tool name
        cx.scene.draw_text(
            &format!("🔧 {}", self.tool_name),
            Point::new(bounds.x + 10.0, bounds.y + 10.0),
            cx.text_system,
            14.0,
            theme::text::PRIMARY,
        );

        // Progress bar (if running)
        if self.status == ToolStatus::Running {
            let progress_width = bounds.width * self.progress;
            let progress_bounds = Bounds::new(
                bounds.x,
                bounds.y + bounds.height - 4.0,
                progress_width,
                4.0,
            );

            cx.scene.draw_quad(progress_bounds, theme::accent::PRIMARY, None, 2.0);
        }

        // Result summary (if complete)
        if let Some(ref output_summary) = self.output_summary {
            cx.scene.draw_text(
                output_summary,
                Point::new(bounds.x + 10.0, bounds.y + 35.0),
                cx.text_system,
                12.0,
                theme::text::SECONDARY,
            );
        }
    }
}
```

**Result**: Tool progress updates smoothly, showing spinner → progress bar → result!

---

## Data Flow Patterns

### Pattern 1: Unidirectional Flow

Events flow **down** from backend to UI. Commands flow **up** from UI to backend.

```
Backend → Events → AppState → Signals → Effects → Widgets
Widgets → Commands → Backend
```

This separation prevents circular dependencies and makes reasoning about data flow easier.

### Pattern 2: Signal Propagation

Signals form a **dependency graph**. Updates propagate through the graph automatically.

```
DomainEvent
  ↓
Signal<ChatView>.update()
  ↓
Effect (subscribed to chat_view)
  ↓
request_repaint()
  ↓
Next frame renders updated view
```

### Pattern 3: Command Queuing

Commands are **queued** and processed in batches during the Update phase.

```
User clicks button
  ↓
Command::Navigate { route: Route::Settings }
  ↓
CommandBus.dispatch(command)
  ↓
[Queue: Navigate, ...]
  ↓
Update phase: CommandBus.process()
  ↓
Handle Navigate command
  ↓
Router.navigate(route)
```

This prevents race conditions and ensures consistent execution order.

### Pattern 4: Virtual Scrolling

Large lists only render **visible items** based on scroll offset.

```
ChatView has 10,000 messages
  ↓
Scroll offset: 5000.0 pixels
Item height: 60.0 pixels
  ↓
Visible range: [83, 100]
  ↓
Only render messages 83-100 (18 items)
  ↓
GPU renders ~1KB instead of ~1MB
```

### Pattern 5: Snapshot + Delta

Initial state is sent as a **snapshot**, subsequent updates as **events**.

```
Client connects
  ↓
ServerMessage::Snapshot { chat_view }
  ↓
Client has full state
  ↓
ServerMessage::Events { [MessageAdded, ...] }
  ↓
Client applies deltas to existing state
```

This optimizes network usage: snapshots are rare, events are frequent.

---

## Performance Characteristics

### Latency

| Operation | Typical Latency |
|-----------|----------------|
| User input → Command queued | <1ms |
| Command processing | 1-5ms |
| Network round-trip | 50-200ms |
| Event application to projection | <1ms |
| Signal update → Effect notification | <1ms |
| Widget paint | 5-10ms (for complex widgets) |
| GPU render | 3-8ms |
| **Total (input → screen)** | **60-230ms** |

### Throughput

| Metric | Value |
|--------|-------|
| Events/second (sustained) | ~1000 |
| Events/second (burst) | ~5000 |
| Commands/second | ~100 |
| Messages/second (streaming) | ~20 |
| Frames/second | 60 (target) |

### Memory

| Component | Memory Usage |
|-----------|--------------|
| ChatView (100 messages) | ~50KB |
| Signal<ChatView> | 24 bytes + view size |
| Effect | 32 bytes |
| Widget tree (complex UI) | ~500KB |
| GPU buffers | ~2MB |
| **Total (typical session)** | **~10MB** |

---

## Summary

Data flows through Coder in a **predictable, unidirectional manner**:

1. **Events** from backend → projections → signals
2. **Signals** update → effects/memos → widgets
3. **Widgets** paint → GPU → screen
4. **Commands** from widgets → backend

This architecture provides:
- **Predictability**: Clear data flow, easy to reason about
- **Performance**: Fine-grained updates, minimal re-computation
- **Testability**: Pure functions, dependency injection
- **Debuggability**: Event log, reactive graph visualization (future)
