# coder_test - Comprehensive Testing Framework for Coder

A fully custom, Rust-only testing framework supporting unit, feature, integration, and E2E tests across desktop, web (WASM), and mobile platforms. Designed to make ~100 user stories fully testable with a fluent DSL.

## Features

- **Story DSL**: Fluent given/when/then API for behavior-driven testing
- **Headless Widget Testing**: Test widgets without GPU or window system
- **Reactive Testing**: Track Signal, Memo, and Effect changes
- **Platform Mocking**: Simulate clipboard, window, browser APIs
- **Parallel Execution**: Fast test runs with rayon-based parallelism
- **Multiple Reporters**: Console and JSON output for CI integration

## Quick Start

```rust
use coder_test::prelude::*;

// Simple story test
story!("User can send a message")
    .tagged("chat")
    .given(|cx| {
        cx.fixture::<DomainFixture>()
            .with_session("test")
            .with_thread("main");
    })
    .when(|cx| {
        cx.actions()
            .type_text("Hello, world!")
            .press_enter();
    })
    .then(|cx| {
        let fixture = cx.fixture::<DomainFixture>();
        fixture.assert_message_count(1);
    })
    .run();
```

## Architecture

```
crates/coder/test/
├── src/
│   ├── lib.rs              # Public API exports and macros
│   ├── story/
│   │   ├── mod.rs          # story!() macro and DSL
│   │   ├── builder.rs      # StoryBuilder fluent API
│   │   ├── context.rs      # TestContext for step functions
│   │   └── inventory.rs    # Story execution and results
│   ├── harness/
│   │   ├── mod.rs          # TestHarness for headless testing
│   │   ├── mock_text.rs    # MockTextSystem (deterministic)
│   │   └── mounted.rs      # MountedWidget wrapper
│   ├── actions/
│   │   ├── mod.rs          # UserActions fluent API
│   │   └── input.rs        # KeyBuilder, MouseBuilder
│   ├── assertions/
│   │   ├── mod.rs          # Assertion macros
│   │   └── scene.rs        # SceneAssertions trait
│   ├── reactive/
│   │   └── mod.rs          # SignalTracker, MemoTracker, EffectTracker
│   ├── platform/
│   │   ├── mod.rs          # Platform abstractions
│   │   ├── mock.rs         # MockPlatform (clipboard, window)
│   │   └── browser.rs      # MockBrowserAPI (localStorage, etc.)
│   ├── fixtures/
│   │   ├── mod.rs          # Fixture trait and registry
│   │   ├── domain.rs       # DomainFixture (sessions, messages)
│   │   ├── mock_chat.rs    # MockChatService (AI responses)
│   │   └── registry.rs     # FixtureRegistry (lazy init)
│   ├── runner/
│   │   └── mod.rs          # TestRunner with parallel execution
│   └── report/
│       ├── mod.rs          # Reporter trait
│       ├── console.rs      # ConsoleReporter
│       └── json.rs         # JsonReporter (CI integration)
└── Cargo.toml
```

## Core Components

### Story DSL

The story macro provides a fluent API for writing tests:

```rust
story!("Feature description")
    .tagged("tag1")           // Add tags for filtering
    .tagged("tag2")
    .given(|cx| { ... })      // Setup steps
    .given(|cx| { ... })      // Multiple given steps allowed
    .when(|cx| { ... })       // Action steps
    .then(|cx| { ... })       // Assertion steps
    .run();                   // Execute immediately
```

### TestContext

The `TestContext` provides access to all testing resources:

```rust
fn my_step(cx: &mut TestContext) {
    // Access fixtures (lazily created, cached)
    let domain = cx.fixture::<DomainFixture>();

    // Access test harness
    let harness = cx.harness_mut();

    // Simulate user actions
    cx.actions()
        .click(Point::new(100.0, 50.0))
        .type_text("Hello");

    // Store data between steps
    cx.store("key", 42);
    let value = cx.get::<i32>("key");

    // Wait for conditions
    cx.wait_until(Duration::from_secs(1), || condition());
}
```

### TestHarness

Headless widget testing without GPU:

```rust
let mut harness = TestHarness::new();

// Configure viewport
harness.set_viewport_size(Size::new(800.0, 600.0));
harness.set_scale_factor(2.0);

// Mount a widget
let mounted = harness.mount(MyWidget::new());

// Dispatch input events
harness.dispatch(&mut widget, &InputEvent::MouseDown { ... }, bounds);

// Access captured scene for assertions
let scene = harness.scene();
assert!(scene.quads.len() > 0);
```

### MockTextSystem

Deterministic text measurement (monospace assumption):

```rust
let text = MockTextSystem::new();  // 8px width, 16px height

let size = text.measure("Hello");
// size.width = 40.0 (5 chars * 8px)
// size.height = 16.0 (1 line)

let pos = text.char_position("Hello\nWorld", 8);
// Returns position of 'o' in "World"

let idx = text.char_index_at("Hello", Point::new(20.0, 0.0));
// Returns index 3 (rounds 2.5 chars)
```

### UserActions

Fluent API for input simulation:

```rust
UserActions::new(&mut harness)
    .with_shift()                           // Hold shift
    .click(Point::new(100.0, 50.0))         // Left click
    .right_click(Point::new(200.0, 50.0))   // Right click
    .drag(Point::new(0.0, 0.0), Point::new(100.0, 100.0))
    .type_text("Hello")                      // Type characters
    .press_enter()                           // Press Enter
    .press_key(Key::Named(NamedKey::Tab))   // Press Tab
    .scroll_y(-50.0)                         // Scroll
    .clear_modifiers();                      // Release modifiers
```

### Reactive Testing

Track changes to reactive primitives:

```rust
// Signal tracking
let mut tracker = SignalTracker::new();
tracker.record("count", 1);
tracker.record("count", 2);
tracker.assert_change_count("count", 2);
tracker.assert_last_value("count", 2);

// Effect tracking
let mut effects = EffectTracker::new();
effects.record_execution("log");
effects.record_execution("fetch");
effects.assert_order(&["log", "fetch"]);

// Memo tracking
let mut memos = MemoTracker::new();
memos.record_recompute("doubled", 4);
memos.assert_recomputed("doubled");
```

### Fixtures

Reusable test setup:

```rust
// Domain fixture
let mut fixture = DomainFixture::new();
fixture
    .with_session("test")
    .with_thread("main")
    .with_user_message("Hello")
    .with_assistant_message("Hi there!");

fixture.assert_message_count(2);
fixture.assert_last_message("Hi there!");

// Mock chat service
let mut chat = MockChatService::new();
chat.queue_text("Hello!");
chat.queue_streaming(&["Hello", " ", "world", "!"]);
chat.queue_tool_use("read_file", json!({ "path": "/tmp/test.txt" }));

let response = chat.send_message("Hi");
chat.assert_message_sent("Hi");
```

### Platform Mocking

```rust
// Mock platform
let mut platform = MockPlatform::new()
    .with_window_size(1920.0, 1080.0)
    .with_scale_factor(2.0);

platform.set_clipboard_text("Copied text");
platform.assert_clipboard_equals("Copied text");

platform.focus_out();
platform.assert_no_focus();

// Mock browser API
let mut browser = MockBrowserAPI::new();
browser.set_local_storage("key", "value");
browser.navigate("/page1");
browser.back();
browser.assert_url("http://localhost:3000");
```

### Test Runner

```rust
let mut runner = TestRunner::new(
    RunnerConfig::new()
        .parallelism(Parallelism::Auto)     // Use all CPUs
        .filter("chat")                      // Filter by name
        .tags(["smoke", "e2e"])             // Filter by tags
        .fail_fast()                         // Stop on first failure
        .reporter(ConsoleReporter::new())
        .reporter(JsonReporter::new())
);

runner.add_story(story!("test 1").then(|_| {}).build());
runner.add_story(story!("test 2").then(|_| {}).build());

let results = runner.run();
assert!(results.all_passed());
println!("Pass rate: {}%", results.pass_rate());
```

## Scene Assertions

```rust
use coder_test::prelude::*;

let scene = harness.scene();

// Using trait methods
assert!(scene.contains_text("Hello"));
assert_eq!(scene.quad_count(), 5);
assert!(scene.has_quad_intersecting(bounds));

// Using macros
assert_scene!(scene, contains_text "Hello");
assert_scene!(scene, not contains_text "Goodbye");
assert_scene!(scene, quad_count 5);
```

## Running Tests

```bash
# Run all coder_test tests
cargo test -p coder_test

# Run with specific tag (when using TestRunner)
# Tags are handled in code, not CLI

# Run in parallel (default)
cargo test -p coder_test -- --test-threads=8
```

## Test Categories

### Unit Tests
Test individual components in isolation:
```rust
#[test]
fn signal_updates_propagate() {
    let mut tracker = SignalTracker::new();
    tracker.record("value", 5);
    tracker.assert_last_value("value", 5);
}
```

### Feature Tests (Story-based)
Test user-facing features:
```rust
story!("User can create a new session")
    .tagged("session")
    .given(|cx| { cx.fixture::<DomainFixture>(); })
    .when(|cx| { cx.actions().click(Point::new(100.0, 50.0)); })
    .then(|cx| { /* verify session created */ })
    .run();
```

### Integration Tests
Test component interactions:
```rust
#[test]
fn chat_service_processes_messages() {
    let mut service = MockChatService::new();
    service.queue_text("Hello!");
    let response = service.send_message("Hi");
    assert!(matches!(response, Some(MockResponse::Text(_))));
}
```

## Dependencies

- `rayon` - Parallel execution
- `tokio` - Async runtime support
- `serde`/`serde_json` - Serialization for JSON reporter
- `chrono` - Timestamps
- `uuid` - Unique identifiers
- `lazy_static` - Global runner registration

## License

MIT
