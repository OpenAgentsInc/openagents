# Port TypeScript E2E Tests to Rust/GPUI

## Summary

Migrate 8 TypeScript Playwright E2E test files (~46-60 tests) to Rust using GPUI's native testing infrastructure. Create both component-level tests and full E2E infrastructure.

## Scope

### TypeScript Tests to Port

| Test File | Tests | Target Crate |
|-----------|-------|--------------|
| `e2e/tests/smoke/basic-smoke.spec.ts` | 4 | `hud` |
| `e2e/tests/integration/golden-loop.spec.ts` | 15 | `commander` |
| `e2e/tests/errors/error-handling.spec.ts` | 8 | `hud` |
| `e2e/tests/visual/render.spec.ts` | 6 | `hud` |
| `e2e/tests/interactions/canvas.spec.ts` | 7 | `hud` |
| `e2e/tests/realtime/realtime-updates.spec.ts` | 10 | `hud` |
| `e2e/tests/screenshots/desktop-screenshots.spec.ts` | 5 | `storybook` |
| `e2e/tests/effuse/mainview-load.spec.ts` | 5 | `commander` |

---

## Architecture

### New Crate: `crates/hud_test/`

Test infrastructure crate providing:
- HUD protocol types (Rust equivalents of `src/hud/protocol.ts`)
- Test fixtures (Page Object Model pattern)
- Message injection (replaces HTTP/WebSocket with direct entity updates)
- Assertion helpers

```
crates/hud_test/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── protocol.rs          # HudMessage enum + types
│   ├── fixtures/
│   │   ├── mod.rs
│   │   ├── graph_fixture.rs # GraphViewFixture (MainviewPage equivalent)
│   │   ├── injector.rs      # HudInjector (message injection)
│   │   └── assertions.rs    # GraphAssertions
│   └── messages/
│       ├── mod.rs
│       ├── factories.rs     # Message factory functions
│       └── sequences.rs     # Golden Loop, APM sequences
```

### Test Modules in Existing Crates

```
crates/hud/src/tests/
├── mod.rs
├── smoke.rs           # basic-smoke tests
├── canvas.rs          # canvas interaction tests
├── error_handling.rs  # error handling tests
├── render.rs          # visual/layout tests
└── realtime.rs        # real-time update tests

crates/commander/src/tests/
├── mod.rs
├── golden_loop.rs     # golden loop integration tests
└── load.rs            # component loading tests

crates/storybook/src/tests/
├── mod.rs
└── visual.rs          # visual state render tests
```

---

## Implementation Steps

### Step 1: Create `hud_test` Crate

**Files to create:**
- `crates/hud_test/Cargo.toml`
- `crates/hud_test/src/lib.rs`
- `crates/hud_test/src/protocol.rs`

**Port from:** `src/hud/protocol.ts` (lines 1-200)

**Key types:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HudMessage {
    SessionStart { session_id: String, timestamp: String },
    SessionComplete { success: bool, summary: String },
    TaskSelected { task: HudTaskInfo },
    TaskDecomposed { subtasks: Vec<HudSubtaskInfo> },
    SubtaskStart { subtask: HudSubtaskInfo },
    SubtaskComplete { subtask: HudSubtaskInfo, result: HudSubagentResult },
    VerificationStart { command: String },
    VerificationComplete { command: String, passed: bool, output: Option<String> },
    CommitCreated { sha: String, message: String },
    PushComplete { branch: String },
    ApmUpdate { session_apm: f64, total_actions: u64, ... },
    Error { phase: String, error: String },
}
```

### Step 2: Create GraphViewFixture

**File:** `crates/hud_test/src/fixtures/graph_fixture.rs`

**Port from:** `e2e/fixtures/mainview.fixture.ts` (MainviewPage class)

```rust
pub struct GraphViewFixture {
    pub view: Entity<GraphView>,
    cx: VisualTestContext,
}

impl GraphViewFixture {
    pub fn new(cx: &mut TestAppContext) -> Self;
    pub fn wait_for_settled(&mut self);
    pub fn zoom_level(&self) -> f32;
    pub fn node_count(&self) -> usize;
    pub fn pan(&mut self, delta_x: f32, delta_y: f32);
    pub fn zoom(&mut self, delta: f32);
    pub fn reset_view(&mut self);
    pub fn click_node(&mut self, node_id: &str);
}
```

### Step 3: Create HudInjector

**File:** `crates/hud_test/src/fixtures/injector.rs`

**Port from:** `e2e/fixtures/mainview.fixture.ts` (HudInjector class)

```rust
pub struct HudInjector<'a> {
    view: &'a Entity<GraphView>,
    cx: &'a mut VisualTestContext,
}

impl<'a> HudInjector<'a> {
    pub fn inject(&mut self, message: HudMessage);
    pub fn inject_sequence(&mut self, messages: Vec<HudMessage>, delay_ms: u64);
    pub fn inject_raw(&mut self, data: &str);  // For error testing
    pub fn simulate_disconnect(&mut self);
    pub fn simulate_reconnect(&mut self);
}
```

### Step 4: Create Message Factories

**File:** `crates/hud_test/src/messages/factories.rs`

```rust
pub fn session_start(session_id: Option<&str>) -> HudMessage;
pub fn task_selected(task: HudTaskInfo) -> HudMessage;
pub fn apm_update(session_apm: f64, total_actions: u64) -> HudMessage;
pub fn error(phase: &str, message: &str) -> HudMessage;
pub fn golden_loop_sequence(task_id: Option<&str>) -> Vec<HudMessage>;
pub fn apm_progress_sequence() -> Vec<HudMessage>;
```

### Step 5: Extend GraphView for Message Handling

**File:** `crates/hud/src/graph_view.rs`

**Add methods:**
```rust
impl GraphView {
    pub fn handle_hud_message(&mut self, message: HudMessage, cx: &mut Context<Self>);
    pub fn handle_raw_message(&mut self, data: &str, cx: &mut Context<Self>);
    pub fn handle_disconnect(&mut self, cx: &mut Context<Self>);
    pub fn handle_reconnect(&mut self, cx: &mut Context<Self>);

    // Accessors for tests
    pub fn zoom(&self) -> f32;
    pub fn node_count(&self) -> usize;
    pub fn has_node(&self, id: &str) -> bool;
    pub fn current_apm(&self) -> f64;
    pub fn current_error(&self) -> Option<&str>;
}
```

### Step 6: Port Smoke Tests

**File:** `crates/hud/src/tests/smoke.rs`

**Port from:** `e2e/tests/smoke/basic-smoke.spec.ts`

```rust
#[gpui::test]
fn test_graph_view_renders(cx: &mut TestAppContext);

#[gpui::test]
fn test_canvas_pan_updates_transform(cx: &mut TestAppContext);

#[gpui::test]
fn test_zoom_via_scroll_wheel(cx: &mut TestAppContext);

#[gpui::test]
fn test_reset_returns_to_initial_state(cx: &mut TestAppContext);
```

### Step 7: Port Canvas Tests

**File:** `crates/hud/src/tests/canvas.rs`

**Port from:** `e2e/tests/interactions/canvas.spec.ts`

```rust
#[gpui::test]
fn test_pan_by_drag_updates_transform(cx: &mut TestAppContext);

#[gpui::test]
fn test_zoom_has_min_max_limits(cx: &mut TestAppContext);

#[gpui::test]
fn test_window_resize_preserves_zoom(cx: &mut TestAppContext);
```

### Step 8: Port Error Handling Tests

**File:** `crates/hud/src/tests/error_handling.rs`

**Port from:** `e2e/tests/errors/error-handling.spec.ts`

```rust
#[gpui::test]
fn test_no_crash_on_invalid_message(cx: &mut TestAppContext);

#[gpui::test]
fn test_error_indicator_visible_on_error(cx: &mut TestAppContext);

#[gpui::test]
fn test_recovery_after_multiple_errors(cx: &mut TestAppContext);

#[gpui::test]
fn test_invalid_apm_values_handled(cx: &mut TestAppContext);
```

### Step 9: Port Real-time Tests

**File:** `crates/hud/src/tests/realtime.rs`

**Port from:** `e2e/tests/realtime/realtime-updates.spec.ts`

```rust
#[gpui::test]
fn test_rapid_message_sequence_no_drops(cx: &mut TestAppContext);

#[gpui::test]
fn test_message_burst_during_user_interaction(cx: &mut TestAppContext);

#[gpui::test]
fn test_session_lifecycle_messages_in_order(cx: &mut TestAppContext);
```

### Step 10: Port Golden Loop Tests

**File:** `crates/commander/src/tests/golden_loop.rs`

**Port from:** `e2e/tests/integration/golden-loop.spec.ts`

```rust
#[gpui::test]
fn test_golden_loop_sequence_renders(cx: &mut TestAppContext);

#[gpui::test]
fn test_task_selected_creates_node(cx: &mut TestAppContext);

#[gpui::test]
fn test_apm_updates_during_session(cx: &mut TestAppContext);

#[gpui::test]
fn test_subtask_completion_flow(cx: &mut TestAppContext);

#[gpui::test]
fn test_verification_pass_flow(cx: &mut TestAppContext);

#[gpui::test]
fn test_commit_and_push_flow(cx: &mut TestAppContext);
```

### Step 11: Port Visual Tests

**File:** `crates/storybook/src/tests/visual.rs`

**Port from:** `e2e/tests/screenshots/desktop-screenshots.spec.ts`

```rust
#[gpui::test]
fn test_idle_state_renders(cx: &mut TestAppContext);

#[gpui::test]
fn test_session_active_state_renders(cx: &mut TestAppContext);

#[gpui::test]
fn test_error_state_renders(cx: &mut TestAppContext);

#[gpui::test]
fn test_apm_widget_renders_all_values(cx: &mut TestAppContext);
```

### Step 12: Port Load Tests

**File:** `crates/commander/src/tests/load.rs`

**Port from:** `e2e/tests/effuse/mainview-load.spec.ts`

```rust
#[gpui::test]
fn test_commander_view_loads_without_error(cx: &mut TestAppContext);

#[gpui::test]
fn test_text_input_component_loads(cx: &mut TestAppContext);

#[gpui::test]
fn test_trajectory_store_initializes(cx: &mut TestAppContext);
```

---

## Key Files to Modify

| File | Changes |
|------|---------|
| `crates/hud/src/graph_view.rs` | Add `handle_hud_message()`, accessor methods |
| `crates/hud/src/lib.rs` | Add `#[cfg(test)] mod tests;` |
| `crates/hud/Cargo.toml` | Add dev-dependencies |
| `crates/commander/src/lib.rs` | Add `#[cfg(test)] mod tests;` |
| `crates/commander/Cargo.toml` | Add dev-dependencies |
| `crates/storybook/src/lib.rs` | Add `#[cfg(test)] mod tests;` |
| `Cargo.toml` (workspace) | Add `hud_test` to members |

---

## Dependencies

### `hud_test/Cargo.toml`
```toml
[package]
name = "hud_test"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui = { path = "../gpui" }
hud = { path = "../hud" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
uuid = { version = "1", features = ["v4"] }
```

### Dev-dependencies for test crates
```toml
[dev-dependencies]
gpui = { path = "../gpui", features = ["test-support"] }
hud_test = { path = "../hud_test" }
```

---

## Test Execution

```bash
# Run all Rust tests
cargo test

# Run specific crate tests
cargo test -p hud
cargo test -p commander
cargo test -p storybook

# Run with verbose output
cargo test -p hud -- --nocapture
```

---

## Migration Checklist

- [ ] Create `crates/hud_test/` crate
- [ ] Port HUD protocol types to Rust
- [ ] Create GraphViewFixture
- [ ] Create HudInjector
- [ ] Create message factories
- [ ] Extend GraphView with message handling
- [ ] Port smoke tests (4 tests)
- [ ] Port canvas tests (7 tests)
- [ ] Port error handling tests (8 tests)
- [ ] Port real-time tests (10 tests)
- [ ] Port golden loop tests (15 tests)
- [ ] Port visual tests (5 tests)
- [ ] Port load tests (5 tests)
- [ ] Update CI to run Rust tests
