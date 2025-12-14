# Coder: GPU-Accelerated AI IDE

**Coder** is a complete, custom-built UI stack for AI-powered development tools. It implements the "own all six layers" philosophy: from domain model through GPU rendering, we control every layer of the stack.

## Why Build Our Own Stack?

Instead of using existing UI frameworks like Dioxus, Electron, or web technologies, Coder implements a custom architecture that provides:

- **Full Control**: Every layer from domain events to GPU buffers
- **Performance**: Direct GPU rendering with wgpu/WebGPU
- **Type Safety**: Rust's type system prevents entire classes of UI bugs
- **Fine-grained Reactivity**: Solid.js-inspired reactive system with automatic dependency tracking
- **Event Sourcing**: Time-travel debugging, audit logs, and replay-ability
- **No Runtime Overhead**: Compiled to native code, zero JavaScript

## The Six Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 6: Application (coder/app/)                          │
│  Entry point, bootstrap, event loop                         │
├─────────────────────────────────────────────────────────────┤
│  Layer 5: Shell (coder/shell/)                              │
│  Router, navigation, chrome, views                          │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: Surfaces (coder/surfaces_*)                       │
│  Chat, terminal, diff viewer, timeline                      │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: Widgets (coder/widgets/)                          │
│  Composable UI: Div, Text, Button, Input, ScrollView        │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: UI Runtime (coder/ui_runtime/)                    │
│  Signal<T>, Memo<T>, Effect, Scheduler, Commands            │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Domain Model (coder/domain/)                      │
│  Events, entities, projections (event sourcing)             │
├─────────────────────────────────────────────────────────────┤
│  Layer 0: Renderer (wgpui/)                                 │
│  wgpu, Taffy layout, cosmic-text, GPU buffers               │
└─────────────────────────────────────────────────────────────┘
```

Each layer has a clear responsibility and well-defined interfaces to adjacent layers.

## Architecture Highlights

### Event Sourcing
All state changes are **immutable events** appended to an event stream. Projections (read-optimized views) are built by applying events.

```rust
// Events are the source of truth
let event = DomainEvent::MessageAdded {
    thread_id,
    message_id,
    content: "Hello, world!".into(),
    role: Role::User,
    timestamp: Utc::now(),
};

// Projections are derived views
chat_view.apply(&event); // Updates UI-optimized ChatView
```

### Fine-Grained Reactivity
Signals track dependencies automatically. When you read a signal inside an effect or memo, it subscribes. When the signal updates, dependents re-run.

```rust
let count = Signal::new(0);
let doubled = Memo::new(move || count.get() * 2); // Auto-subscribes

Effect::new(move || {
    println!("Doubled: {}", doubled.get()); // Re-runs when count changes
});

count.set(5); // Triggers effect re-run
```

### Widget Composition
Widgets compose via a builder pattern. Every widget implements the `Widget` trait with `paint()` and `event()` methods.

```rust
Div::new()
    .background(theme::bg::SURFACE)
    .padding(8.0)
    .child(Text::new("Title").size(16.0).color(theme::text::PRIMARY))
    .child(Button::new("Click Me").on_click(|| {
        // Handle click
    }))
```

### Frame-Based Scheduler
UI updates happen in discrete frames (default 60fps). Each frame has phases:

1. **Update**: Run effects, process commands
2. **Build**: Construct widget tree
3. **Layout**: Calculate sizes and positions (Taffy)
4. **Paint**: Generate GPU commands
5. **Render**: Submit to GPU

### Virtual Scrolling
Large lists only render visible items. The `VirtualList` widget calculates which items are in the viewport and renders only those.

```rust
VirtualList::new()
    .item_height(60.0)
    .item_count(10_000)
    .render_item(|index, bounds, cx| {
        // Only called for visible items
        MessageBubble::new(messages[index]).paint(bounds, cx);
    })
```

## Project Structure

```
crates/coder/
├── domain/           # Event-sourced domain model
│   ├── event.rs      # DomainEvent enum
│   ├── message.rs    # Message entity
│   ├── run.rs        # Run/Step entities
│   ├── tool.rs       # ToolUse entity
│   └── projections/  # ChatView, etc.
├── protocol/         # Client/server wire protocol
│   ├── client.rs     # ClientMessage types
│   └── server.rs     # ServerMessage types
├── ui_runtime/       # Reactive runtime
│   ├── signal.rs     # Signal<T>
│   ├── memo.rs       # Memo<T>
│   ├── effect.rs     # Effect
│   ├── scheduler.rs  # Frame scheduler
│   └── command.rs    # CommandBus
├── widgets/          # UI building blocks
│   ├── widget.rs     # Widget trait
│   ├── div.rs        # Container widget
│   ├── text.rs       # Text widget
│   ├── button.rs     # Button widget
│   ├── text_input.rs # Input widget
│   ├── scroll.rs     # ScrollView widget
│   └── list.rs       # VirtualList widget
├── shell/            # Application shell
│   ├── router.rs     # Route enum, history
│   ├── navigation.rs # Navigation controller
│   ├── views.rs      # View trait, registry
│   └── chrome.rs     # Window chrome
├── surfaces_chat/    # Chat UI surface
│   ├── thread.rs     # ChatThread widget
│   ├── message.rs    # MessageBubble widget
│   ├── tool_use.rs   # ToolUseIndicator widget
│   └── input.rs      # ChatInput widget
├── surfaces_terminal/# Terminal emulator
│   ├── terminal.rs   # Terminal widget
│   ├── buffer.rs     # Scrollback buffer
│   └── ansi.rs       # ANSI parser
├── surfaces_diff/    # Diff viewer
│   ├── view.rs       # DiffView widget
│   └── diff.rs       # Diff computation
├── surfaces_timeline/# Timeline visualization
│   ├── timeline.rs   # Timeline widget
│   ├── step.rs       # Step representation
│   └── lane.rs       # Lane for parallel runs
├── storage/          # Persistence layer
│   └── lib.rs        # SQLite storage for threads, messages, sessions
├── permission/       # Permission system
│   └── lib.rs        # Async ask/respond pattern with "always allow"
├── session/          # Session management
│   ├── session.rs    # Session state and status
│   ├── processor.rs  # Main conversation loop
│   └── prompt.rs     # System prompt builder
├── agent/            # Agent definitions
│   ├── definition.rs # AgentDefinition types
│   ├── permission.rs # Agent permission presets
│   └── registry.rs   # Built-in agents (general, explore, plan, build)
├── app/              # Application entry
│   ├── main.rs       # Native entry point
│   ├── app.rs        # App struct
│   └── state.rs      # AppState
└── docs/             # Documentation (you are here)
    ├── README.md
    ├── ARCHITECTURE.md
    ├── DOMAIN_MODEL.md
    ├── REACTIVE_RUNTIME.md
    ├── DATA_FLOW.md
    ├── AI_INFRASTRUCTURE.md
    └── GETTING_STARTED.md

crates/llm/           # LLM Provider abstraction
├── message/          # Request/response types
├── model/            # Model definitions and pricing
├── provider/         # Provider trait and implementations
│   └── anthropic.rs  # Anthropic Claude provider
└── stream/           # Streaming types and SSE parsing

crates/tool_registry/ # Tool execution framework
├── tool.rs           # Tool trait
├── registry.rs       # Tool registry
├── context.rs        # Execution context with cancellation
└── wrappers/         # Standard tools (bash, read, write, edit, grep, find)
```

## Key Concepts

### Entities vs Projections

- **Entities** (domain): Authoritative, append-only events
- **Projections**: Read-optimized views derived from events

Example: A `Message` entity exists as `MessageAdded` events. The `ChatView` projection organizes messages for efficient UI rendering.

### Signals vs State

- **Signals**: Reactive containers that track subscribers
- **Memos**: Cached computed values
- **Effects**: Side effects that auto-re-run

Signals form a reactive graph. Reading a signal inside a memo/effect creates a subscription.

### Widgets vs Components

- **Widgets**: Implement `Widget` trait, have `paint()` and `event()`
- Not like React components (no JSX, no virtual DOM)
- Immediate-mode UI: rebuild widget tree every frame

### Commands vs Events

- **DomainEvents**: Backend-generated, authoritative state changes
- **Commands**: UI-generated intents (SendMessage, Navigate, etc.)

Commands are requests. Events are facts.

## Documentation

### UI & Rendering
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - Deep dive into the six layers
- **[DOMAIN_MODEL.md](./DOMAIN_MODEL.md)** - Event sourcing, entities, projections
- **[REACTIVE_RUNTIME.md](./REACTIVE_RUNTIME.md)** - Signals, effects, scheduler
- **[WIDGETS.md](./WIDGETS.md)** - Widget system and composition
- **[SURFACES.md](./SURFACES.md)** - Chat, terminal, diff, timeline
- **[DATA_FLOW.md](./DATA_FLOW.md)** - How data flows through the system

### AI & Session Management
- **[AI_INFRASTRUCTURE.md](./AI_INFRASTRUCTURE.md)** - LLM providers, tools, sessions, agents, permissions

### Getting Started
- **[GETTING_STARTED.md](./GETTING_STARTED.md)** - Build, run, develop

## Quick Start

```bash
# Build
cargo build -p coder_app

# Run
cargo run -p coder_app

# Or use the alias
cargo coder
```

## Design Principles

1. **Own the Stack**: Control every layer, no hidden framework magic
2. **Type Safety**: Leverage Rust's type system to prevent bugs
3. **Event Sourcing**: Immutable events as source of truth
4. **Fine-Grained Reactivity**: Automatic dependency tracking
5. **Performance**: Direct GPU rendering, minimal overhead
6. **Composability**: Widgets compose like Lego blocks
7. **Testability**: Pure functions, dependency injection

## Comparison to Other Frameworks

| Feature | Coder | Dioxus | Electron | Web |
|---------|-------|--------|----------|-----|
| Language | Rust | Rust | JS/TS | JS/TS |
| Rendering | wgpu (GPU) | HTML/CSS | Chromium | Browser |
| Reactivity | Signal-based | Virtual DOM | React/Vue | React/Vue |
| Bundle Size | ~5MB | ~50MB | ~150MB | Varies |
| Cold Start | <100ms | ~500ms | ~2s | Instant |
| Memory | ~50MB | ~200MB | ~500MB | ~100MB |
| Type Safety | Full | Full | Partial | Partial |

## Performance Characteristics

- **60 FPS** rendering on modern hardware
- **<16ms** frame time (1/60th second)
- **10,000+ items** in virtual lists with smooth scrolling
- **<100MB** memory for typical workloads
- **Native performance**: No JavaScript runtime overhead

## Future Directions

- **Hot Reload**: Live code updates without restart
- **Time-Travel Debugging**: Replay events to any point in history
- **Multi-Window**: Multiple app windows with shared state
- **Accessibility**: Screen reader support, keyboard navigation
- **Mobile**: iOS/Android via winit + wgpu
- **WASM**: Run in browser via WebGPU

## License

See repository root for license information.
