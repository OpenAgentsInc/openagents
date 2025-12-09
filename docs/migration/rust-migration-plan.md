# Rust Migration Plan: OpenAgents Commander

> **Status:** Planning Document
> **Created:** 2025-12-09
> **Scope:** Complete codebase migration from TypeScript/Bun to Rust

---

## Executive Summary

This document outlines a comprehensive plan to migrate the OpenAgents Commander codebase from TypeScript/Bun/Effect to Rust. The migration will preserve all functionality while gaining performance, memory safety, and better resource management for a desktop application that manages swarm compute and AI agents.

**Estimated Timeline:** 6-12 months (depending on team size and parallelization)

**Key Benefits:**
- **Performance:** Native compilation, zero-cost abstractions, better CPU/memory efficiency
- **Safety:** Memory safety without garbage collection overhead
- **Resource Management:** Explicit control over compute resources (critical for swarm compute)
- **Cross-Platform:** Easier Windows/Linux support beyond macOS
- **Long-term Maintainability:** Stronger type system, better tooling for large codebases

**Key Challenges:**
- Effect TypeScript → Rust functional programming patterns
- Effuse UI framework complete rewrite
- JavaScript ecosystem dependencies
- Learning curve for team

---

## Current Architecture Analysis

### Tech Stack Inventory

| Component | Current | Lines of Code (est.) | Complexity |
|-----------|---------|---------------------|------------|
| **Runtime** | Bun | - | Low (replacement needed) |
| **Core Framework** | Effect TypeScript | ~50k | High (needs Rust equivalent) |
| **UI Framework** | Effuse (Effect-native) | ~5k | Medium (custom framework) |
| **Desktop Shell** | webview-bun | ~500 | Low (Rust webview available) |
| **Agent System** | MechaCoder | ~15k | High (core business logic) |
| **HillClimber** | FM training | ~10k | High (research code) |
| **Terminal-Bench** | Integration | ~5k | Medium |
| **Storage** | SQLite + JSONL | ~3k | Low (direct port) |
| **LLM Clients** | OpenRouter, FM | ~5k | Medium |
| **Tools** | File ops, git, etc. | ~5k | Low |
| **CLI** | Various commands | ~10k | Low |
| **Swift Bridge** | Foundation Models | ~500 | Low (keep as-is) |

**Total Estimated:** ~113k lines of TypeScript

### Key Dependencies

**Critical (Must Replace):**
- `effect` - Functional programming framework
- `@effect/platform` - Platform abstractions
- `@effect/cli` - CLI framework
- `webview-bun` - Desktop windowing
- `bun` - Runtime

**External APIs (Keep):**
- OpenRouter SDK (Rust client needed)
- HuggingFace Hub (Rust client available)
- Nostr (Rust libraries available)
- Bitcoin/Lightning (Rust libraries available)

**Data Storage (Direct Port):**
- SQLite (rusqlite)
- JSONL (serde_json)

---

## Migration Strategy

### Phase 0: Foundation & Research (Weeks 1-4)

**Goal:** Establish Rust foundation, identify patterns, build proof-of-concepts

#### 0.1 Effect TypeScript → Rust Functional Patterns

**Research:**
- Evaluate Rust functional programming libraries:
  - [Effect-RS](https://github.com/Effect-TS/effect-rs) (if exists)
  - [Rusty](https://github.com/rusty-ts/rusty) (TypeScript-like effects)
  - [fp-rs](https://github.com/fp-rs/fp-rs) (functional primitives)
  - [oxide](https://github.com/oxidecomputer/oxide) patterns
  - Custom Effect-like implementation

**Decision Matrix:**
- Type safety (Result<T, E> vs custom Error types)
- Async/await vs Effect.gen patterns
- Service/Context pattern (trait objects vs generics)
- Layer composition (dependency injection)

**POC:** Port a small Effect module (e.g., `src/storage/`) to validate approach

#### 0.2 GPUI Framework Research & Evaluation

**Research Tasks:**
- Study GPUI documentation and architecture
- Analyze Zed's GPUI implementation in their codebase
- Evaluate GPUI's component model vs Effuse
- Test GPU rendering performance
- Assess cross-platform support (macOS, Windows, Linux)
- Review Entity system for state management
- Evaluate styling system (Tailwind-like approach)
- Test async executor integration

**Key Questions to Answer:**
- How does GPUI's View/Entity model map to Effuse Component/StateCell?
- What's the learning curve for the team?
- Are there any platform-specific limitations?
- How does GPU rendering perform vs webview-based UI?
- Can we port all Effuse components to GPUI?

**Resources:**
- GPUI website: https://www.gpui.rs/
- Zed source code: https://github.com/zed-industries/zed
- GPUI crate documentation (when available)
- Zed blog posts about GPU UI architecture

**POC:** Port one Effuse component (e.g., `APMComponent`) to GPUI to validate:
- Component structure and patterns
- State management with Entity
- Styling approach
- Event handling
- Performance characteristics

#### 0.3 UI Framework Strategy

**Decision: GPUI (GPU-Accelerated UI Framework from Zed Team)**

**GPUI Overview:**
- **Direct-to-GPU Rendering:** GPU-accelerated UI framework developed by the creators of Zed editor
- **Hybrid Mode:** Combines immediate and retained mode UI paradigms
- **Rust-Native:** Built entirely in Rust, offering memory safety and performance
- **Declarative & Imperative APIs:** Both high-level declarative UI and low-level imperative control
- **Entity System:** Integrated state management with Entity system for application state
- **Tailwind-like Styling:** Styling system inspired by Tailwind CSS
- **Async Support:** Async executor integrated with platform event loop
- **High Performance:** Designed for smooth animations and responsive UIs at high frame rates

**Key Features:**
- GPU acceleration for rendering
- Memory-safe Rust implementation
- Cross-platform support (macOS, Windows, Linux)
- Flexible UI paradigms (immediate + retained mode)
- Built-in state management
- Modern styling system

**Resources:**
- Website: [gpui.rs](https://www.gpui.rs/)
- GitHub: [zed-industries/zed](https://github.com/zed-industries/zed) (GPUI is part of Zed)
- Documentation: GPUI crate docs and Zed source code

**Migration Approach:**
- Port Effuse components to GPUI's component model
- Leverage GPU acceleration for performance
- Use Entity system for state management (replaces StateCell)
- Adapt styling from Effuse to GPUI's Tailwind-like system

**POC:** Port one Effuse component (e.g., `APMComponent`) to GPUI to validate approach

#### 0.4 Desktop Architecture

**Current:** Bun process → webview-bun → HTTP server → WebSocket → Frontend

**Rust Target:**
- Rust binary → GPUI (native GPU-accelerated window) → Direct rendering (no webview)
- Optional: HTTP server → WebSocket for backend services (if needed)
- Pure Rust UI with GPUI - no webview dependency

**Decision:** GPUI for native GPU-accelerated UI, eliminating webview overhead

**Benefits:**
- No webview dependency (smaller binary, better performance)
- Direct GPU rendering (lower latency, smoother animations)
- Native platform integration
- Better resource control for swarm compute scenarios

#### 0.5 Swift Bridge Integration

**Strategy:** Keep Swift bridge as-is, call from Rust via FFI

**Implementation:**
- Rust FFI bindings to Swift binary
- HTTP client to foundation-bridge (current approach)
- Or: Direct Swift interop (if Rust-Swift bridge exists)

**POC:** Call foundation-bridge from Rust

---

### Phase 1: Core Infrastructure (Weeks 5-12)

**Goal:** Build Rust foundation that TypeScript code can gradually migrate to

#### 1.1 Project Structure

```
openagents-rust/
├── Cargo.toml
├── crates/
│   ├── core/              # Effect-like primitives
│   │   ├── effect/        # Effect<T, E, R> implementation
│   │   ├── context/       # Service/Context pattern
│   │   ├── layer/         # Layer composition
│   │   └── stream/        # Reactive streams
│   ├── platform/          # Platform abstractions
│   │   ├── fs/            # File system
│   │   ├── process/       # Process execution
│   │   ├── http/          # HTTP client/server
│   │   └── websocket/     # WebSocket
│   ├── storage/           # SQLite, JSONL
│   ├── schemas/           # Shared data structures
│   ├── cli/               # CLI framework
│   └── ui/                # GPUI-based UI framework
│       ├── components/    # GPUI components (ported from Effuse)
│       ├── entities/      # Entity-based state management
│       ├── styles/        # GPUI styling (Tailwind-like)
│       └── views/         # GPUI views and windows
├── src/
│   ├── main.rs            # Desktop entry point
│   ├── agent/             # MechaCoder (gradual migration)
│   ├── hillclimber/       # FM training (gradual migration)
│   └── ...
└── swift-bridge/          # Keep existing Swift code
```

#### 1.2 Core Effect-Like Framework

**Design:**
```rust
// Effect<T, E, R> equivalent
pub struct Effect<T, E, R> {
    // Implementation using async/await or custom executor
}

// Service/Context pattern
pub trait Service: Send + Sync {
    type Tag: ServiceTag;
}

pub struct Context<R> {
    // Service registry
}

// Layer composition
pub struct Layer<R> {
    // Dependency injection
}
```

**Key Features:**
- Type-safe error handling (Result<T, E>)
- Service/Context pattern for dependency injection
- Async/await with custom executor (if needed)
- Stream support for reactivity

**Deliverable:** `crates/core/` with Effect-like primitives

#### 1.3 Platform Abstractions

**Port from `@effect/platform`:**
- FileSystem service
- Process execution
- HTTP client/server
- WebSocket
- Path operations

**Deliverable:** `crates/platform/` with all platform services

#### 1.4 Storage Layer

**Direct Port:**
- SQLite via `rusqlite`
- JSONL via `serde_json`
- Schema migrations

**Deliverable:** `crates/storage/` with database and file operations

#### 1.5 CLI Framework

**Port from `@effect/cli`:**
- Command parsing
- Argument validation
- Help generation
- Subcommand support

**Deliverable:** `crates/cli/` with CLI framework

---

### Phase 2: UI Framework with GPUI (Weeks 13-20)

**Goal:** Rebuild UI using GPUI's GPU-accelerated framework

#### 2.1 GPUI Setup & Integration

**Setup:**
- Add GPUI dependency to `Cargo.toml`
- Initialize GPUI application with window creation
- Set up GPUI's async executor integration
- Configure GPU rendering pipeline

**Key GPUI Concepts:**
- **Window:** Top-level application window
- **View:** UI component (similar to Effuse Component)
- **Entity:** State management primitive (replaces StateCell)
- **Style:** Tailwind-like styling system
- **Event Loop:** GPUI's integrated async executor

#### 2.2 Component System (Effuse → GPUI)

**Port Effuse Components to GPUI Views:**

**TypeScript (Effuse):**
```typescript
const CounterComponent: Component<CounterState, CounterEvent> = {
  initialState: () => ({ count: 0 }),
  render: (ctx) => Effect.gen(function* () {
    const { count } = yield* ctx.state.get;
    return html`<div>Count: ${count}</div>`;
  }),
  handleEvent: (event, ctx) => Effect.gen(function* () {
    if (event.type === "increment") {
      yield* ctx.state.update(s => ({ count: s.count + 1 }));
    }
  }),
}
```

**Rust (GPUI):**
```rust
struct CounterView {
    count: Entity<u32>,
}

impl Render for CounterView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let count = self.count.read(cx);
        div()
            .text(format!("Count: {}", count))
            .on_click(cx.listener(|this, _event, cx| {
                this.count.update(cx, |count, _| *count += 1);
            }))
    }
}
```

**Migration Strategy:**
- Map Effuse `Component<S, E>` → GPUI `View` with `Render` trait
- Map Effuse `StateCell<T>` → GPUI `Entity<T>`
- Map Effuse `html` templates → GPUI declarative API
- Map Effuse event handlers → GPUI event callbacks

#### 2.3 State Management (Entity System)

**GPUI Entity System:**
```rust
// Entity replaces StateCell
let count = Entity::new(cx, 0u32);

// Read state
let value = count.read(cx);

// Update state
count.update(cx, |value, _| *value += 1);

// Subscribe to changes
count.observe(cx, |count, _| {
    // React to state changes
});
```

**Key Differences from Effuse:**
- Entity requires `ViewContext` for access (ensures proper lifecycle)
- Updates are synchronous (no async needed)
- Built-in observation system for reactivity

#### 2.4 Styling System

**GPUI Tailwind-like Styling:**
```rust
div()
    .flex()
    .items_center()
    .justify_between()
    .p_4()
    .bg(rgb(0x1a1a1a))
    .text_color(rgb(0xffffff))
```

**Migration from Effuse:**
- Convert Effuse CSS classes → GPUI style methods
- Use GPUI's color system (rgb, rgba, etc.)
- Leverage GPUI's layout system (flex, grid, etc.)

#### 2.5 Component Porting

**Port Effuse Components:**
- `APMComponent` → `APMView`
- `TBControlsComponent` → `TBControlsView`
- `TBOutputComponent` → `TBOutputView`
- `TBResultsComponent` → `TBResultsView`
- `MCTasksComponent` → `MCTasksView`
- All other Effuse components

**Pattern:**
1. Create `View` struct with state as `Entity<T>`
2. Implement `Render` trait
3. Use GPUI's declarative API for UI construction
4. Handle events with `on_click`, `on_key`, etc.
5. Use GPUI styling methods

#### 2.6 Window Management

**GPUI Window Setup:**
```rust
App::new()
    .run(|cx: &mut AppContext| {
        let bounds = Bounds::centered(None, size(em(1200.0), em(800.0)), cx);
        cx.open_window(WindowOptions {
            window_bounds: Some(bounds),
            ..Default::default()
        }, |cx| {
            cx.new_view(|_| MainView::new())
        })
        .unwrap();
    });
```

**Deliverable:** `crates/ui/` with all components ported to GPUI

---

### Phase 3: Agent System Migration (Weeks 21-32)

**Goal:** Port MechaCoder and agent infrastructure

#### 3.1 Core Agent Infrastructure

**Port:**
- `src/agent/session.ts` → Session management
- `src/agent/run.ts` → Run execution
- `src/agent/transport.ts` → Message transport
- `src/agent/orchestrator/` → Orchestrator system

**Strategy:** Port module by module, maintain TypeScript version in parallel

#### 3.2 Tools System

**Port:**
- `src/tools/` → All tool implementations
- File operations
- Git operations
- Process execution
- Code editing

**Key Challenge:** Tool execution safety (sandboxing)

#### 3.3 LLM Clients

**Port:**
- `src/llm/openrouter.ts` → OpenRouter client (Rust SDK)
- `src/llm/foundation-models.ts` → FM client (HTTP to Swift bridge)
- `src/llm/claude-code.ts` → Claude Code client

**External Dependencies:**
- Find or create Rust SDKs for LLM providers

#### 3.4 MechaCoder Core

**Port:**
- `src/agent/do-one-task.ts` → Single task execution
- `src/agent/overnight.ts` → Overnight loop
- `src/agent/overnight-parallel.ts` → Parallel execution

**Deliverable:** Fully functional MechaCoder in Rust

---

### Phase 4: HillClimber & Training (Weeks 33-40)

**Goal:** Port FM HillClimber and training infrastructure

#### 4.1 HillClimber Core

**Port:**
- `src/hillclimber/` → All hillclimber modules
- MAP architecture
- TestGen system
- Evolution system

**Key Challenge:** Preserve research code correctness

#### 4.2 Terminal-Bench Integration

**Port:**
- `src/bench/` → Terminal-Bench runner
- Docker integration
- Test execution
- Metrics collection

#### 4.3 Learning System

**Port:**
- `src/learning/` → Learning infrastructure
- `src/training/` → Training system
- `src/trainer/` → Trainer components

**Deliverable:** Complete training system in Rust

---

### Phase 5: Desktop App & Integration (Weeks 41-48)

**Goal:** Complete desktop application

#### 5.1 Desktop Application

**GPUI Application Setup:**
- Initialize GPUI `App` with window creation
- Set up main application view
- Configure window properties (size, title, etc.)
- Handle application lifecycle

**Backend Services (if needed):**
- Port `src/desktop/server.ts` → HTTP + WebSocket server (optional, for external clients)
- Port `src/desktop/handlers.ts` → Request handlers
- Port `src/desktop/protocol.ts` → Protocol definitions

**GPUI Integration:**
- No webview needed - GPUI renders directly to GPU
- Native window management via GPUI
- Direct Rust backend integration (no IPC needed)
- Optional: HTTP/WebSocket server for external tooling

#### 5.2 HUD System

**Port:**
- `src/hud/` → HUD event system
- `src/tbench-hud/` → Terminal-Bench HUD
- Real-time updates via WebSocket

#### 5.3 All Components

**Port Remaining:**
- `src/archivist/` → Archivist system
- `src/atif/` → ATIF integration
- `src/healer/` → Self-healing
- `src/researcher/` → Research tools
- `src/skills/` → Skills library
- `src/memory/` → Memory system
- `src/reflexion/` → Reflexion
- `src/telemetry/` → Telemetry
- `src/usage/` → Usage tracking

**Deliverable:** Complete desktop application in Rust

---

### Phase 6: Polish & Optimization (Weeks 49-52)

**Goal:** Performance, testing, documentation

#### 6.1 Performance Optimization

**Focus Areas:**
- Memory usage (critical for swarm compute)
- CPU efficiency
- Startup time
- UI responsiveness

**Tools:**
- `cargo flamegraph` for profiling
- `cargo bench` for benchmarks
- Memory profiling

#### 6.2 Testing

**Test Coverage:**
- Unit tests for all modules
- Integration tests
- E2E tests (Playwright for UI)
- Property-based tests (proptest)

#### 6.3 Documentation

**Deliverables:**
- API documentation (rustdoc)
- Migration guide for contributors
- Architecture documentation
- Performance benchmarks

---

## Technical Decisions

### Effect Pattern Translation

**TypeScript:**
```typescript
Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem;
  const content = yield* fs.readFileString(path);
  return content;
}).pipe(Effect.provide(FileSystemLive))
```

**Rust (Option 1 - Async/Await):**
```rust
async fn read_file(path: &Path) -> Result<String, Error> {
    let fs = FileSystem::new();
    let content = fs.read_file_string(path).await?;
    Ok(content)
}
```

**Rust (Option 2 - Custom Effect):**
```rust
fn read_file(path: &Path) -> Effect<String, Error, FileSystem> {
    Effect::gen(async move {
        let fs = yield Effect::service::<FileSystem>();
        let content = yield fs.read_file_string(path);
        content
    })
}
```

**Decision:** Use async/await for simplicity, custom Effect only if needed for advanced patterns

### Service/Context Pattern

**TypeScript:**
```typescript
interface MyService extends Context.Tag<"MyService", MyService> {}
const MyService = Context.GenericTag<MyService>("MyService");
```

**Rust:**
```rust
pub trait MyService: Send + Sync {
    async fn do_something(&self) -> Result<(), Error>;
}

// Dependency injection via trait objects or generics
pub struct MyComponent<S: MyService> {
    service: Arc<S>,
}
```

**Decision:** Use trait-based dependency injection (standard Rust pattern)

### State Management

**TypeScript (Effuse StateCell):**
```typescript
const state = yield* StateService.make(initial);
yield* state.update(s => ({ ...s, count: s.count + 1 }));
```

**Rust (GPUI Entity):**
```rust
// In View struct
struct MyView {
    count: Entity<u32>,
}

// In ViewContext
let count = Entity::new(cx, 0u32);
count.update(cx, |value, _| *value += 1);
let current = count.read(cx);
```

**Decision:** Use GPUI's Entity system for state management. Entity provides:
- Synchronous updates (no async needed)
- Built-in observation for reactivity
- ViewContext-based lifecycle management
- Type-safe state access

### Error Handling

**TypeScript (Effect):**
```typescript
Effect.fail(new CustomError("message"))
  .pipe(Effect.mapError(e => e.message))
```

**Rust:**
```rust
Err(CustomError::new("message"))
    .map_err(|e| e.message())
```

**Decision:** Use standard Rust `Result<T, E>` with custom error types

---

## Migration Tactics

### Parallel Development

**Strategy:** Run TypeScript and Rust versions in parallel during migration

**Approach:**
1. Build Rust foundation (Phases 0-2)
2. Port modules incrementally (Phases 3-5)
3. Feature flags to switch between implementations
4. Gradual cutover per module

**Benefits:**
- No development freeze
- Can test Rust version alongside TypeScript
- Rollback capability
- Incremental validation

### Testing Strategy

**For Each Ported Module:**
1. Port tests alongside code
2. Run both TypeScript and Rust tests
3. Compare outputs (property-based testing)
4. Integration tests for full workflows

**Tools:**
- `cargo test` for Rust unit tests
- Playwright for E2E (works with both versions)
- Custom comparison tests (TypeScript vs Rust outputs)

### Data Migration

**Strategy:** No data migration needed - both versions use same SQLite/JSONL

**Compatibility:**
- Same database schema
- Same file formats
- Same protocols (WebSocket, HTTP)

---

## Risk Mitigation

### Risk 1: Effect Pattern Complexity

**Risk:** Effect TypeScript patterns don't translate cleanly to Rust

**Mitigation:**
- POC in Phase 0 to validate approach
- Fallback to standard Rust patterns if needed
- Preserve semantics, not syntax

### Risk 2: GPUI Framework Complexity

**Risk:** GPUI learning curve or API limitations make migration difficult

**Mitigation:**
- Comprehensive POC in Phase 0 to validate approach
- Study Zed's implementation patterns as reference
- Early engagement with GPUI community (if available)
- Fallback options: egui or Iced if GPUI proves unsuitable
- Allocate extra time for team training on GPUI

### Risk 3: Performance Regression

**Risk:** Rust version is slower (unlikely but possible)

**Mitigation:**
- Benchmark early and often
- Profile both versions
- Optimize hot paths

### Risk 4: Timeline Overrun

**Risk:** Migration takes longer than estimated

**Mitigation:**
- Phased approach allows partial deployment
- Can run both versions in production
- Prioritize critical paths first

### Risk 5: Team Learning Curve

**Risk:** Team needs time to learn Rust

**Mitigation:**
- Training during Phase 0
- Pair programming (Rust + TypeScript)
- Code reviews
- Gradual migration (learn as you go)

---

## Success Criteria

### Functional Parity

- [ ] All TypeScript features work in Rust
- [ ] All tests pass
- [ ] Performance >= TypeScript version
- [ ] Memory usage <= TypeScript version

### Code Quality

- [ ] Zero unsafe Rust code (or minimal, well-documented)
- [ ] Comprehensive test coverage
- [ ] Documentation complete
- [ ] Linting/formatting (rustfmt, clippy)

### User Experience

- [ ] Desktop app works identically
- [ ] No regressions in functionality
- [ ] Startup time <= TypeScript version
- [ ] UI responsiveness >= TypeScript version

---

## Timeline Summary

| Phase | Duration | Key Deliverables |
|-------|----------|------------------|
| **Phase 0** | 4 weeks | POCs, research, decisions |
| **Phase 1** | 8 weeks | Core infrastructure |
| **Phase 2** | 8 weeks | UI framework |
| **Phase 3** | 12 weeks | Agent system |
| **Phase 4** | 8 weeks | HillClimber & training |
| **Phase 5** | 8 weeks | Desktop app |
| **Phase 6** | 4 weeks | Polish & optimization |
| **Total** | **52 weeks** | Complete migration |

**Accelerated Timeline (with larger team):**
- Parallel work on Phases 2-5: **32-36 weeks**
- Requires 3-4 developers working in parallel

---

## Next Steps

1. **Review & Approve Plan** - Team review, adjust timeline/approach
2. **Phase 0 Kickoff** - Start research and POCs
3. **Team Training** - Rust training for team members
4. **Tooling Setup** - Rust toolchain, CI/CD, development environment
5. **Begin Phase 1** - Start core infrastructure development

---

## Appendix

### Rust Ecosystem Alternatives

**Effect-like Libraries:**
- [fp-rs](https://github.com/fp-rs/fp-rs) - Functional primitives
- [oxide](https://github.com/oxidecomputer/oxide) - Effect patterns
- Custom implementation

**UI Framework:**
- **[GPUI](https://www.gpui.rs/)** - GPU-accelerated UI framework from Zed team (selected)
  - Direct-to-GPU rendering
  - Hybrid immediate/retained mode
  - Entity-based state management
  - Tailwind-like styling
  - Built-in async executor
  - Cross-platform (macOS, Windows, Linux)
- [egui](https://github.com/emilk/egui) - Immediate mode, pure Rust (alternative)
- [Iced](https://github.com/iced-rs/iced) - Elm-like, pure Rust (alternative)
- [Slint](https://slint-ui.com/) - Declarative, cross-platform (alternative)

**Async Runtimes:**
- [Tokio](https://tokio.rs/) - Most popular, full-featured
- [async-std](https://async-std.rs/) - std-like API
- Custom executor (if needed for Effect patterns)

**HTTP/WebSocket:**
- [axum](https://github.com/tokio-rs/axum) - Web framework
- [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) - WebSocket
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client

**Database:**
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite
- [sqlx](https://github.com/launchbadge/sqlx) - Async SQL

**Serialization:**
- [serde](https://serde.rs/) - JSON, TOML, etc.
- [serde_json](https://github.com/serde-rs/json) - JSON

**CLI:**
- [clap](https://github.com/clap-rs/clap) - Argument parsing
- [structopt](https://github.com/TeXitoi/structopt) - Derive-based (deprecated, use clap derive)

### References

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Effect TypeScript Docs](https://effect.website/)
- [GPUI Framework](https://www.gpui.rs/) - GPU-accelerated UI framework
- [Zed Editor Source](https://github.com/zed-industries/zed) - GPUI implementation reference
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)

---

**Last Updated:** 2025-12-09
**Status:** Draft - Awaiting Review
