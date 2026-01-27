# Autopilot Desktop (WGPUI) — End-to-End Testability & Resilience Spec

## Purpose

Make the Effuse ➜ WGPUI native Rust app **fully testable end-to-end** while keeping the system resilient under change. The goal is to achieve the “Effect feeling” in Rust:

* **Everything depends on interfaces**, not concrete implementations.
* **Services compose cleanly**.
* **Production services are swappable** with mocks/fakes/simulators.
* We can run:

  * fast unit tests
  * deterministic integration tests
  * full E2E tests that exercise UI + runtime + tools
* We can reproduce and debug failures via **event logs + replay**.
* We match **Zed/GPUI test ergonomics**: immediate-mode rendering with context-driven state,
  layout derived from Taffy each frame, and deterministic test contexts for UI assertions.

This doc defines the architecture, contracts, and test harnesses to accomplish that.

## Current State Context (Grounded in Repo)

The existing autopilot-desktop codebase provides a strong foundation for testability:

### Existing Architecture
- **UI Framework**: Effuse (Effect-native) with 90+ TypeScript components in [`apps/autopilot-desktop/src/components/`](file:///Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/components/)
- **Backend**: Tauri-based with Rust services in [`apps/autopilot-desktop/src-tauri/`](file:///Users/christopherdavid/code/openagents/apps/autopilot-desktop/src-tauri/)
- **Core Agent Logic**: Lives in [`crates/autopilot-core/src/agent.rs`](file:///Users/christopherdavid/code/openagents/crates/autopilot-core/src/agent.rs) with DSPy-powered planning
- **Target UI Stack**: WGPUI immediate-mode UI in `crates/autopilot_ui/` and native host in
  `apps/autopilot-desktop-wgpu/`, moving toward Zed/GPUI-style layout and test harnesses.

### Current UI Event Flow
```
Backend (Adjutant) → AppEvent stream → Typed ViewModel → WGPUI Render tree
```

**Key Files:**
- App core: `crates/autopilot_app/`
- Desktop host: `apps/autopilot-desktop-wgpu/`
- Shared UI: `crates/autopilot_ui/`

### Current UI Components Structure
The codebase has extensive component catalog:
- **90+ AI Elements**: [`apps/autopilot-desktop/src/components/ai-elements/`](file:///Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/components/ai-elements/) (conversation, tools, code, planning)
- **30+ Base UI**: [`apps/autopilot-desktop/src/components/ui/`](file:///Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/components/ui/) (forms, layout, navigation)
- **Component Catalog**: [`apps/autopilot-desktop/src/components/catalog.ts`](file:///Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/components/catalog.ts) with validation

---

## Non-goals

* Perfect pixel-level UI golden testing (optional, not required for correctness).
* Testing external LLM vendors over the network in CI.
* Verifying OS-specific behaviors beyond minimal smoke tests.

---

## Core Testability Principles

### 1) Stable seam boundaries

Every dependency that can fail or change must sit behind a **service trait**:

* filesystem
* process execution
* git operations
* network/HTTP
* model/LLM calls
* clock/time
* randomness/ids
* persistence (sqlite/rocksdb/whatever)
* UI event sink/source

### 2) Deterministic execution

Any test should be runnable:

* offline
* without timing flakiness
* with fixed seeds/ids/timestamps
* with recorded responses when needed

### 3) “Log + Replay” as a first-class tool

The canonical replay stream is **`AppEvent` + `UserAction`**:

* record those streams
* make replay easy
* build tests that assert on logs rather than fragile UI queries

### 4) Single “App Core” driving everything

`crates/autopilot_app` must be runnable in:

* production desktop
* CLI
* test harness
* headless “simulated UI”

---

## Zed/GPUI Parity Testing Model

Zed’s GPUI approach is immediate-mode: each frame rebuilds the element tree
via `Render` / `RenderOnce`, layout is computed by Taffy, and views are backed
by `Entity<T>` with `Context<T>` for state updates. We mirror this in WGPUI:

- **Render each frame**: tests assert on element tree output, not side effects.
- **Context-driven state**: tests mutate entities through context APIs.
- **Layout snapshots**: assert bounds from the Taffy layout results (no pixel diffs).
- **Deterministic contexts**: emulate Zed’s `TestAppContext` and
  `VisualTestContext` using WGPUI’s testing harness (`crates/wgpui/src/testing`)
  and component registry for selectors.

This keeps UI tests stable and aligns directly with Zed’s testing ergonomics.

---

## System Decomposition

### The three layers

#### Layer A: Domain / App Core (`crates/autopilot_app`)

Pure orchestration:

* session lifecycle
* action dispatch
* tool scheduling
* event emission
* state reduction (or delegates to reducer)

Depends on a set of **service traits** and emits a stream of **events**.

#### Layer B: Adapters (production implementations)

Concrete services:

* `FsLive`, `GitLive`, `ProcLive`, `HttpLive`, `ModelLive`, `DbLive`, etc.
* These live in `crates/autopilot_services/*` or inside the crate but clearly separated.

#### Layer C: UI Host (`apps/autopilot-desktop-wgpu`)

WGPUI renderer + OS integration:

* subscribes to `AppEvent` stream
* dispatches `UserAction`
* renders using WGPUI (`Render` / `RenderOnce` element tree, Taffy layout)
* no business logic

---

## Service Model (Effect-like in Rust)

### The pattern

Each dependency is represented by:

* a `trait` for the capability
* a “live” implementation
* one or more “test” implementations (mock/fake/sim)
* an `AppContext` (or `Services`) struct holding trait objects or generics

You can implement this in two viable Rust styles:

### Style 1: Generic “Layered” App (maximum performance)

```rust
// crates/autopilot_app/src/app.rs
pub struct App<S: Services> { 
    services: S, 
    state: AppState,
    session_manager: SessionManager,
}

pub trait Services: Clone + Send + Sync + 'static {
  type Fs: Fs;
  type Git: Git;
  type Model: Model;
  type Proc: Proc;
  type Clock: Clock;
  type Store: Store;
  
  fn fs(&self) -> &Self::Fs;
  fn git(&self) -> &Self::Git;
  fn model(&self) -> &Self::Model;
  fn proc(&self) -> &Self::Proc;
  fn clock(&self) -> &Self::Clock;
  fn store(&self) -> &Self::Store;
}
```

### Style 2: Trait-object Service Container (maximum ergonomics)

```rust
// crates/autopilot_app/src/services.rs
pub struct Services {
  pub fs: Arc<dyn Fs>,
  pub git: Arc<dyn Git>,
  pub model: Arc<dyn Model>,
  pub proc: Arc<dyn Proc>,
  pub clock: Arc<dyn Clock>,
  pub store: Arc<dyn Store>,
  pub ui_sink: Arc<dyn UiSink>,
  pub telemetry: Arc<dyn Telemetry>,
}

impl Services {
    /// Production services
    pub fn live() -> Result<Self> {
        Ok(Self {
            fs: Arc::new(FsLive::new()?),
            git: Arc::new(GitLive::new()?),
            model: Arc::new(ModelLive::new()?),
            proc: Arc::new(ProcLive::new()),
            clock: Arc::new(ClockLive::new()),
            store: Arc::new(StoreLive::new("~/.autopilot/db")?),
            ui_sink: Arc::new(UiSinkLive::new()),
            telemetry: Arc::new(TelemetryLive::new()),
        })
    }

    /// Test services with deterministic behavior
    pub fn test_defaults() -> Self {
        Self {
            fs: Arc::new(FsMemory::new()),
            git: Arc::new(GitFake::new()),
            model: Arc::new(ModelReplay::from_fixtures("fixtures/model_responses/")),
            proc: Arc::new(ProcFake::new()),
            clock: Arc::new(ClockManual::new()),
            store: Arc::new(StoreMemory::new()),
            ui_sink: Arc::new(UiSinkCapture::new()),
            telemetry: Arc::new(TelemetryCapture::new()),
        }
    }
}
```

**Recommendation:** Use **trait-object container** for the first iteration. It’s closer to Effect’s “swap any layer” ergonomics and simplifies test harness assembly.

---

## Required Service Traits

These are the **minimum** traits to make E2E tests real and reliable.

### `Clock`

* `now() -> SystemTime`
* `sleep(duration) -> Future<()>` (in prod)
* In tests: manual clock that advances deterministically.

### `IdGen`

* `new_ulid()` / `new_uuid()`
* In tests: seeded deterministic generator.

### `Fs`

* read/write files, list dirs, watch (optional)
* In tests: in-memory FS or tempdir-backed FS with strict assertions.

### `Proc`

* spawn processes, capture stdout/stderr, exit codes
* In tests: fake process runner with scripted outcomes.

### `Git`

* status/diff/commit/checkout
* In tests: either fake git or temp repo runner with deterministic fixtures.

### `Http`

* generic request/response
* In tests: stub server or in-process mock router.

### `Model`

* “LLM-ish” interface used by agent loop
* In tests: record/replay model that returns fixture responses.

### `Store` (persistence)

* sessions, runs, logs, UI snapshots
* In tests: in-memory store or temp sqlite.

### `UiSink` / `Telemetry`

* structured events + spans
* in tests: capture everything for assertions

## Concrete Service Implementation Examples

Based on our existing codebase structure, here are detailed service trait implementations:

### `Clock` - Time & Scheduling

```rust
// crates/autopilot_services/src/clock.rs
#[async_trait]
pub trait Clock: Send + Sync {
    async fn now(&self) -> SystemTime;
    async fn sleep(&self, duration: Duration);
}

pub struct ClockLive;
impl Clock for ClockLive {
    async fn now(&self) -> SystemTime { SystemTime::now() }
    async fn sleep(&self, duration: Duration) { tokio::time::sleep(duration).await; }
}

pub struct ClockManual {
    time: Arc<Mutex<SystemTime>>,
}
impl ClockManual {
    pub fn new() -> Self { Self { time: Arc::new(Mutex::new(SystemTime::now())) } }
    pub fn advance(&self, duration: Duration) {
        let mut time = self.time.lock().unwrap();
        *time += duration;
    }
}
```

### `Model` - LLM Integration (Grounded in existing agent.rs)

```rust
// crates/autopilot_services/src/model.rs
#[async_trait] 
pub trait Model: Send + Sync {
    async fn complete(&self, req: ModelRequest) -> Result<ModelResponse>;
    async fn stream(&self, req: ModelRequest) -> Result<impl Stream<Item = ModelChunk>>;
}

// Based on existing AutopilotAgent planning system
pub struct ModelRequest {
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    // Matches DSPy planning input format from crates/autopilot-core/src/agent.rs
}

pub struct ModelLive {
    client: openai::Client,
}

pub struct ModelReplay {
    // Loads fixture responses from disk - integrates with existing replay system
    fixtures: HashMap<String, ModelResponse>,
    fixture_dir: PathBuf,
}

impl ModelReplay {
    pub fn from_fixtures(dir: &str) -> Self {
        // Load all .json files from fixtures/model_responses/
        Self { 
            fixtures: Self::load_fixtures(dir),
            fixture_dir: PathBuf::from(dir)
        }
    }
}
```

### `Store` - Persistence (Integrates with existing session management)

```rust
// crates/autopilot_services/src/store.rs
#[async_trait]
pub trait Store: Send + Sync {
    async fn save_session(&self, session: &Session) -> Result<SessionId>; 
    async fn load_session(&self, id: &SessionId) -> Result<Option<Session>>;
    async fn save_app_snapshot(&self, session_id: &SessionId, snap: &AppSnapshot) -> Result<()>;
    async fn load_app_snapshot(&self, session_id: &SessionId) -> Result<Option<AppSnapshot>>;
}

// Based on existing session management in apps/autopilot-desktop/src-tauri/
pub struct StoreLive {
    db_path: PathBuf,
    connection: SqlitePool,
}

impl StoreLive {
    pub fn new(db_path: &str) -> Result<Self> {
        // Uses existing ~/.autopilot/ directory structure
        let path = PathBuf::from(db_path);
        std::fs::create_dir_all(path.parent().unwrap())?;
        Ok(Self { db_path: path, connection: SqlitePool::connect(db_path)? })
    }
}

pub struct StoreMemory { 
    sessions: Arc<Mutex<HashMap<SessionId, Session>>>,
    app_snapshots: Arc<Mutex<HashMap<SessionId, AppSnapshot>>>,
}
```

---

## Contracts to Stabilize for Testing

### App events (`AppEvent`) - Grounded in existing runtime event flow

Must be **serializable**, **loggable**, and stable enough to replay.

```rust
// crates/autopilot_app/src/events.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    // Core lifecycle + UI-relevant events
    WorkspaceOpened {
        workspace_id: WorkspaceId,
        path: PathBuf,
    },
    SessionStarted {
        session_id: SessionId,
        label: Option<String>,
    },
    UserActionDispatched {
        session_id: SessionId,
        action: UserAction,
    },
    RunStateChanged {
        session_id: SessionId,
        old_state: RunState,
        new_state: RunState,
    },
    ToolStarted {
        session_id: SessionId,
        tool_id: ToolId,
        tool_name: String,
        input: JsonValue,
    },
    ToolFinished {
        session_id: SessionId,
        tool_id: ToolId,
        result: ToolResult,
        duration_ms: u64,
    },
    LogLine {
        session_id: SessionId,
        level: LogLevel,
        message: String,
        timestamp: SystemTime,
    },
    Error {
        session_id: SessionId,
        error_code: String,
        message: String,
        context: HashMap<String, JsonValue>,
    },
}
```

### Actions (`UserAction`) - Based on existing interaction patterns

Also serializable for replay, derived from current UI patterns:

```rust
// crates/autopilot_app/src/actions.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserAction {
    // Workspace management
    OpenWorkspace { path: PathBuf },
    CloseWorkspace,
    
    // Session lifecycle
    StartSession { config: SessionConfig },
    EndSession { session_id: SessionId },
    
    // Conversation
    SendMessage { 
        session_id: SessionId, 
        text: String,
        attachments: Vec<Attachment>,
    },
    
    // UI interactions (based on existing Effuse action system)
    Click { 
        element_id: String,
        action_id: Option<String>,  // Based on data-ez attributes
    },
    SetInput { 
        element_id: String, 
        value: String 
    },
    
    // Agent control
    CancelRun { session_id: SessionId },
    PauseRun { session_id: SessionId },
    ResumeRun { session_id: SessionId },
    
    // Canvas interactions (autopilot-canvas specific)
    CanvasClick { x: f32, y: f32 },
    CanvasZoom { factor: f32 },
    CanvasPan { dx: f32, dy: f32 },
}
```

### View model contract (typed state)

The UI contract is a **typed Rust view model**. UI state is derived from
`AppEvent` streams and rendered immediately each frame via `Render` /
`RenderOnce` components. There is no UI patch protocol in the new system.

```rust
// crates/autopilot_ui/src/view_model.rs
#[derive(Default, Clone)]
pub struct AppViewModel {
    workspace: Option<WorkspaceState>,
    session: Option<SessionState>,
    timeline: Vec<TimelineEntry>,
}

impl AppViewModel {
    pub fn apply_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::WorkspaceOpened { path, .. } => {
                self.workspace = Some(WorkspaceState::from(path));
            }
            AppEvent::SessionStarted { session_id, .. } => {
                self.session = Some(SessionState::new(*session_id));
            }
            AppEvent::UserActionDispatched { action, .. } => {
                self.timeline.push(TimelineEntry::from(action));
            }
            _ => {}
        }
    }
}
```

---

## Test Harnesses (What we will build)

### Harness 1: View model reducer tests (fastest)

**Scope:** `autopilot_ui` view model reducers

* Given initial `AppViewModel`, apply event sequences, assert final state.
* Snapshot tests on view model JSON are acceptable if stable.

**Deliverables:**
* `crates/autopilot_ui/tests/view_model.rs`
* `fixtures/view_model_events/*.jsonl`

### Harness 2: Headless App simulation (integration)

**Scope:** `autopilot_app` with mocked services

Run app core with test services:
* `ModelReplay` - Returns fixture responses for DSPy planning/execution
* `FsTempdir` - Isolated filesystem per test
* `ProcFake` - Scripted tool execution responses
* `ClockManual` - Deterministic time advancement
* `StoreMemory` - In-memory session storage

Drive via `UserAction`s and assert the resulting `AppEvent` stream.

Key property:

* **No UI rendering** required.
* Validates orchestration, tools, and contract emission.

Deliverables:

* `crates/autopilot_app/tests/headless_e2e.rs`
* “scenario runner” DSL (see below)

### Harness 3: Replay tests (golden)

**Scope:** AppEvent/UserAction replay correctness

Record a real run’s `AppEvent` + `UserAction` streams and replay into:

* the `AppViewModel` reducer
* the WGPUI render tree (no panics)

Assertions:

* no panics
* invariant checks pass
* optional: structural queries (“tool card count is N”)

Deliverables:

* `crates/autopilot_ui/tests/replay.rs`
* `fixtures/replays/*.jsonl`

### Harness 3b: Layout Snapshot Tests (Zed parity)

**Scope:** WGPUI layout correctness without pixel diffs.

Use the WGPUI test harness and component registry
(`crates/wgpui/src/testing`) to render a view, capture element bounds, and
assert structural layout:

* left sidebar width, header heights, and row spacing
* right status sections do not overlap under DPI/scale changes
* center thread area and composer preserve minimum sizes

This mirrors Zed’s `TestAppContext` / `VisualTestContext` approach: assert on
layout metadata instead of pixel-perfect rendering.

Deliverables:

* `crates/autopilot_ui/tests/layout_snapshots.rs`
* `fixtures/layout_snapshots/*.json` (optional bound snapshots)

### Harness 4: Full Desktop smoke E2E (optional but valuable)

**Scope:** compiled `autopilot-desktop-wgpu` in CI

* Launch desktop in headless mode if possible (or use OS runner)
* Use an automation driver only for a **thin smoke test**
* Most correctness comes from Harness 2 + 3

This keeps UI tests from being a flaky nightmare.

---

## Scenario Runner DSL (Effect-like Test Composition)

Create a tiny “scenario” framework so tests read like:

```rust
Scenario::new("send_message_emits_tool_card")
  .with_services(Services::test_defaults())
  .step(OpenWorkspace("fixtures/repos/tiny"))
  .step(StartSession(Default::default()))
  .step(SendMessage("refactor foo"))
  .expect_ui(|vm| vm.contains_tool_call("Tool Call"))
  .expect_event(|ev| matches!(ev, AppEvent::ToolStarted(_)))
  .run();
```

### Requirements

* Steps are `UserAction`s.
* Assertions can match:

  * events (stream-level)
  * derived UI state (via view model reducers)
  * tool outcomes
* Runner can:

  * impose timeouts deterministically (manual clock)
  * dump failing event logs as artifacts

---

## Record/Replay System (Critical)

### Recording format

Use JSON Lines (`.jsonl`) for simplicity:

* Each line: `{ "t": "...", "kind": "AppEvent", "payload": {...} }`
* Include deterministic metadata:

  * seed
  * version/build hash
  * scenario name

### Replay APIs

* `ReplayReader::open(path)` (autopilot_app)
* `ReplayReader::read_all() -> Vec<ReplayRecord>`
* `EventRecorder::record_event(&AppEvent)` for JSONL output

### Uses

* Debugging bugs
* Regression tests
* UI mapping tests
* Contract stability validation

### Contract Stability Gate

Add CI test:

* load all `fixtures/replays/*.jsonl`
* assert:

  * decode works
  * view model apply works
  * invariants hold

This is your “protocol compatibility suite.”

---

## Invariants and “Resilience Checks”

Introduce explicit invariants that are cheap to validate and catch drift:

### UI invariants

* no duplicate view IDs in the rendered tree
* action targets reference valid view model state
* no invalid enum variants
* required fields present for each UI component state

### App invariants

* session state machine transitions legal
* tool lifecycle events balanced (started → finished)
* cancellations end in terminal state
* errors always include context + code

Add:

* `validate_view_model(&AppViewModel) -> Result<()>`
* `validate_event(&AppEvent) -> Result<()>`

Run these validators:

* in debug builds
* in tests
* optionally in production behind a flag

---

## How We Avoid Flaky Tests

### Time

All core tests use `ClockManual`:

* no real sleeping
* manual ticks advance timers and scheduling

### Randomness / IDs

All tests use seeded `IdGenDeterministic`.

### Layout / Rendering

Avoid pixel-level golden snapshots by default. Prefer:

* element tree assertions
* Taffy-derived bounds checks
* component registry queries (IDs + text)

This mirrors Zed’s layout test style and keeps tests stable across platforms.

### LLM / Model calls

Two implementations:

1. `ModelReplay` (default for tests)
2. `ModelFake` (simple rule-based for unit tests)

### External processes

Replace with `ProcFake` except for a small set of integration tests that run real tools in a fixture repo.

---

## Proposed Directory Layout for Tests & Fixtures

```
crates/
  autopilot_app/
    src/
    tests/
      headless_scenarios.rs
  autopilot_ui/
    src/
    tests/
      view_model.rs
      layout_snapshots.rs
      replay.rs

fixtures/
  repos/
    tiny/
  replays/
    send_message_tool_call.jsonl
  view_model_events/
    dashboard_boot.jsonl
  layout_snapshots/
    desktop_panels.json
```

---

## CI Gates (Definition of “Fully Testable”)

### Gate A — Contract correctness

* View model reducer tests pass
* Invariant tests pass
* Replay suite loads and validates

### Gate B — App core determinism

* Headless scenario suite passes offline
* No network access required
* Produces replay logs as CI artifacts on failure

### Gate C — UI mapping regression

* Replaying known runs into UI runtime never panics
* Optional structural assertions hold
* Layout snapshot tests pass (bounds, gaps, and panel sizing)

### Gate D — Desktop smoke

* Desktop binary builds and launches (one platform minimum)
* Optional: render one frame and exit (headless mode)

---

## Deliverables Checklist (Implementation Spec)

### 1) Service container + test defaults

* `Services::live()`
* `Services::test_defaults()` returning deterministic mocks
* Each service trait + at least one fake implementation

### 2) App core emits structured events

* ensure all events serializable
* implement `EventRecorder` service (file sink)
* implement `EventCapture` for tests

### 3) View model + invariants

* typed view model state + reducers
* JSON serde stable (if snapshots are stored)
* fixture-driven tests

### 4) Scenario runner

* steps + expectations
* automatic log dump on failure
* ability to save replays

### 5) Replay suite

* load fixtures
* run validators
* (optional) map into WGPUI runtime

---

## Open Questions (decide, but don’t block)

* Do we want periodic `AppSnapshot` checkpoints in addition to event logs?

  * Either way, record/replay remains valuable.

---

## Acceptance Criteria for This Doc

We can confidently say “fully testable end-to-end” when:

1. A test can drive the system from `OpenWorkspace` → `SendMessage` → tool execution → UI updates **without a real UI**.
2. The same run produces an event log that can be replayed to reproduce the UI state.
3. All unstable external deps (LLM, FS, proc, time, ids) are replaceable via services.
4. CI catches contract drift via replay + invariants.
5. Layout snapshot tests validate Zed-style flex layouts without pixel diffs.

---

If you want, I can turn this into:

* exact trait definitions for the service set (`Fs`, `Proc`, `Model`, etc.)
* a concrete `Scenario` runner API
* a canonical JSONL schema for replays and a minimal recorder/replayer implementation outline.
