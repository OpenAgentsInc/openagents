# Comprehensive Rust-Only Testing Framework for Coder

## Overview

A fully custom, Rust-only testing framework supporting unit, feature, integration, and E2E tests across desktop, web (WASM), and mobile platforms. Designed to make ~100 user stories fully testable with a fluent DSL.

---

## Architecture

### Single Crate: `crates/coder_test`

```
crates/coder_test/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── story/
│   │   ├── mod.rs          # story!() macro and DSL
│   │   ├── builder.rs      # StoryBuilder fluent API
│   │   └── runner.rs       # Story execution engine
│   ├── harness/
│   │   ├── mod.rs          # TestHarness for headless widget testing
│   │   ├── context.rs      # MockPaintContext, MockEventContext
│   │   └── text.rs         # MockTextSystem (deterministic measurement)
│   ├── platform/
│   │   ├── mod.rs          # MockPlatform trait impl
│   │   ├── browser.rs      # Browser API simulation
│   │   └── webdriver.rs    # Real headless browser integration
│   ├── reactive/
│   │   ├── mod.rs          # SignalTracker, MemoTracker, EffectTracker
│   │   └── assertions.rs   # Reactive-specific assertions
│   ├── fixtures/
│   │   ├── mod.rs          # Fixture traits and registry
│   │   ├── domain.rs       # DomainFixture (sessions, threads, messages)
│   │   ├── ui.rs           # UIFixture (widgets, surfaces)
│   │   └── ai.rs           # MockChatService, MockProvider
│   ├── actions/
│   │   ├── mod.rs          # UserActions API
│   │   └── input.rs        # InputEvent builders
│   ├── assertions/
│   │   ├── mod.rs          # Custom assertion macros
│   │   ├── scene.rs        # SceneAssertions (quads, text)
│   │   └── visual.rs       # Optional visual regression
│   ├── runner/
│   │   ├── mod.rs          # Parallel test execution
│   │   └── isolation.rs    # Test isolation guarantees
│   └── report/
│       ├── mod.rs          # Reporter trait
│       ├── console.rs      # Console reporter
│       └── json.rs         # JSON reporter (CI integration)
└── Cargo.toml
```

---

## Core Components

### 1. Story DSL (`story!()` macro)

```rust
use coder_test::prelude::*;

story!("User can send a message in chat")
    .given(|cx| {
        cx.fixture::<DomainFixture>()
            .with_session("test-session")
            .with_thread("test-thread");
    })
    .when(|cx| {
        cx.actions()
            .type_text("Hello, world!")
            .press_key(Key::Enter);
    })
    .then(|cx| {
        cx.assert_that::<ChatSurface>()
            .contains_message("Hello, world!");
    })
    .run();
```

**Implementation:**

```rust
// story/mod.rs
#[macro_export]
macro_rules! story {
    ($name:expr) => {
        $crate::story::StoryBuilder::new($name)
    };
}

// story/builder.rs
pub struct StoryBuilder {
    name: String,
    given: Vec<Box<dyn Fn(&mut TestContext)>>,
    when: Vec<Box<dyn Fn(&mut TestContext)>>,
    then: Vec<Box<dyn Fn(&mut TestContext)>>,
    tags: Vec<String>,
}

impl StoryBuilder {
    pub fn given<F: Fn(&mut TestContext) + 'static>(mut self, f: F) -> Self {
        self.given.push(Box::new(f));
        self
    }

    pub fn when<F: Fn(&mut TestContext) + 'static>(mut self, f: F) -> Self {
        self.when.push(Box::new(f));
        self
    }

    pub fn then<F: Fn(&mut TestContext) + 'static>(mut self, f: F) -> Self {
        self.then.push(Box::new(f));
        self
    }

    pub fn tagged(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn run(self) {
        TestRunner::global().register(self.into());
    }
}
```

### 2. TestHarness (Headless Widget Testing)

```rust
// harness/mod.rs
pub struct TestHarness {
    scene: Scene,
    text_system: MockTextSystem,
    platform: MockPlatform,
    signals: SignalTracker,
}

impl TestHarness {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            text_system: MockTextSystem::new(),
            platform: MockPlatform::new(),
            signals: SignalTracker::new(),
        }
    }

    /// Mount a widget for testing
    pub fn mount<W: Widget + 'static>(&mut self, widget: W) -> MountedWidget<W> {
        MountedWidget::new(widget, self)
    }

    /// Render the mounted widget and capture scene
    pub fn render(&mut self) -> &Scene {
        // Create MockPaintContext, call widget.paint()
        &self.scene
    }

    /// Dispatch an input event
    pub fn dispatch(&mut self, event: InputEvent) -> EventResult {
        // Create MockEventContext, call widget.event()
        EventResult::Ignored
    }

    /// Get captured scene for assertions
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}

pub struct MountedWidget<W> {
    widget: W,
    bounds: Bounds,
}
```

### 3. MockTextSystem (Deterministic Text Measurement)

```rust
// harness/text.rs
pub struct MockTextSystem {
    char_width: f32,
    line_height: f32,
}

impl MockTextSystem {
    pub fn new() -> Self {
        Self {
            char_width: 8.0,  // Monospace assumption
            line_height: 16.0,
        }
    }

    pub fn measure(&self, text: &str) -> Size {
        let lines: Vec<&str> = text.lines().collect();
        let max_width = lines.iter()
            .map(|l| l.chars().count() as f32 * self.char_width)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
        let height = lines.len() as f32 * self.line_height;
        Size::new(max_width, height)
    }
}
```

### 4. UserActions API (Input Simulation)

```rust
// actions/mod.rs
pub struct UserActions<'a> {
    harness: &'a mut TestHarness,
}

impl<'a> UserActions<'a> {
    pub fn click(&mut self, position: Point) -> &mut Self {
        self.harness.dispatch(InputEvent::MouseDown {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        self.harness.dispatch(InputEvent::MouseUp {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        self
    }

    pub fn type_text(&mut self, text: &str) -> &mut Self {
        self.harness.dispatch(InputEvent::TextInput {
            text: text.to_string(),
        });
        self
    }

    pub fn press_key(&mut self, key: Key) -> &mut Self {
        self.harness.dispatch(InputEvent::KeyDown {
            key,
            code: key.into(),
            modifiers: Modifiers::empty(),
            repeat: false,
        });
        self.harness.dispatch(InputEvent::KeyUp {
            key,
            code: key.into(),
            modifiers: Modifiers::empty(),
        });
        self
    }

    pub fn drag(&mut self, from: Point, to: Point) -> &mut Self {
        self.harness.dispatch(InputEvent::MouseDown {
            position: from,
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        self.harness.dispatch(InputEvent::MouseMove {
            position: to,
            modifiers: Modifiers::empty(),
        });
        self.harness.dispatch(InputEvent::MouseUp {
            position: to,
            button: MouseButton::Left,
            modifiers: Modifiers::empty(),
        });
        self
    }

    pub fn scroll(&mut self, delta: Point) -> &mut Self {
        self.harness.dispatch(InputEvent::Wheel {
            delta,
            modifiers: Modifiers::empty(),
        });
        self
    }
}
```

### 5. Reactive Testing Helpers

```rust
// reactive/mod.rs
pub struct SignalTracker {
    changes: Vec<(String, Box<dyn Any>)>,
}

impl SignalTracker {
    pub fn track<T: Clone + 'static>(&mut self, name: &str, signal: &Signal<T>) {
        // Subscribe to signal changes
    }

    pub fn assert_changed<T: PartialEq + 'static>(&self, name: &str, expected: T) {
        // Verify signal changed to expected value
    }

    pub fn assert_change_count(&self, name: &str, count: usize) {
        // Verify number of changes
    }
}

pub struct EffectTracker {
    executions: Vec<String>,
}

impl EffectTracker {
    pub fn track(&mut self, name: &str, effect: &Effect) {
        // Track effect executions
    }

    pub fn assert_ran(&self, name: &str) {
        // Verify effect executed
    }

    pub fn assert_ran_times(&self, name: &str, times: usize) {
        // Verify execution count
    }
}
```

### 6. Scene Assertions

```rust
// assertions/scene.rs
pub trait SceneAssertions {
    fn contains_text(&self, text: &str) -> bool;
    fn contains_quad_at(&self, bounds: Bounds) -> bool;
    fn text_at(&self, position: Point) -> Option<&str>;
    fn quad_count(&self) -> usize;
    fn text_run_count(&self) -> usize;
}

impl SceneAssertions for Scene {
    fn contains_text(&self, text: &str) -> bool {
        self.text_runs.iter().any(|tr| tr.text.contains(text))
    }

    fn contains_quad_at(&self, bounds: Bounds) -> bool {
        self.quads.iter().any(|q| q.bounds.intersects(&bounds))
    }

    fn text_at(&self, position: Point) -> Option<&str> {
        self.text_runs.iter()
            .find(|tr| tr.bounds.contains(position))
            .map(|tr| tr.text.as_str())
    }

    fn quad_count(&self) -> usize {
        self.quads.len()
    }

    fn text_run_count(&self) -> usize {
        self.text_runs.len()
    }
}

// Custom assertion macro
#[macro_export]
macro_rules! assert_scene {
    ($scene:expr, contains_text $text:expr) => {
        assert!(
            $scene.contains_text($text),
            "Expected scene to contain text '{}', but it didn't.\nScene text runs: {:?}",
            $text,
            $scene.text_runs
        );
    };
}
```

### 7. MockPlatform (Headless Platform)

```rust
// platform/mod.rs
pub struct MockPlatform {
    clipboard: String,
    window_size: Size,
    scale_factor: f32,
}

impl Platform for MockPlatform {
    fn clipboard_text(&self) -> Option<String> {
        Some(self.clipboard.clone())
    }

    fn set_clipboard_text(&mut self, text: &str) {
        self.clipboard = text.to_string();
    }

    fn window_size(&self) -> Size {
        self.window_size
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    // No GPU operations - all stubbed
}
```

### 8. Browser API Simulation

```rust
// platform/browser.rs
pub struct MockBrowserAPI {
    local_storage: HashMap<String, String>,
    session_storage: HashMap<String, String>,
    url: String,
    history: Vec<String>,
}

impl MockBrowserAPI {
    pub fn local_storage(&self) -> &HashMap<String, String> {
        &self.local_storage
    }

    pub fn set_local_storage(&mut self, key: &str, value: &str) {
        self.local_storage.insert(key.to_string(), value.to_string());
    }

    pub fn navigate(&mut self, url: &str) {
        self.history.push(self.url.clone());
        self.url = url.to_string();
    }

    pub fn current_url(&self) -> &str {
        &self.url
    }
}
```

### 9. WebDriver Integration (Real Browser E2E)

```rust
// platform/webdriver.rs
pub struct WebDriverSession {
    driver: fantoccini::Client,
}

impl WebDriverSession {
    pub async fn new() -> Result<Self, WebDriverError> {
        let caps = serde_json::json!({
            "browserName": "chrome",
            "goog:chromeOptions": {
                "args": ["--headless", "--disable-gpu"]
            }
        });
        let driver = fantoccini::ClientBuilder::native()
            .capabilities(caps)
            .connect("http://localhost:9515")
            .await?;
        Ok(Self { driver })
    }

    pub async fn navigate(&self, url: &str) -> Result<(), WebDriverError> {
        self.driver.goto(url).await?;
        Ok(())
    }

    pub async fn click(&self, selector: &str) -> Result<(), WebDriverError> {
        let element = self.driver.find(Locator::Css(selector)).await?;
        element.click().await?;
        Ok(())
    }

    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), WebDriverError> {
        let element = self.driver.find(Locator::Css(selector)).await?;
        element.send_keys(text).await?;
        Ok(())
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>, WebDriverError> {
        self.driver.screenshot().await
    }
}
```

### 10. Fixtures System

```rust
// fixtures/mod.rs
pub trait Fixture: Send + Sync {
    fn setup(&mut self, cx: &mut TestContext);
    fn teardown(&mut self, cx: &mut TestContext);
}

// fixtures/domain.rs
pub struct DomainFixture {
    storage: Storage,
    sessions: Vec<SessionId>,
    threads: Vec<ThreadId>,
}

impl DomainFixture {
    pub fn with_session(&mut self, id: &str) -> &mut Self {
        // Create a test session
        self
    }

    pub fn with_thread(&mut self, id: &str) -> &mut Self {
        // Create a test thread
        self
    }

    pub fn with_messages(&mut self, messages: Vec<Message>) -> &mut Self {
        // Populate messages
        self
    }
}

// fixtures/ai.rs
pub struct MockChatService {
    responses: VecDeque<MockResponse>,
}

impl MockChatService {
    pub fn queue_response(&mut self, response: MockResponse) {
        self.responses.push_back(response);
    }

    pub fn queue_streaming(&mut self, chunks: Vec<&str>) {
        // Queue streaming response chunks
    }

    pub fn queue_tool_use(&mut self, tool: &str, input: serde_json::Value) {
        // Queue tool use response
    }
}
```

### 11. Parallel Test Runner

```rust
// runner/mod.rs
pub struct TestRunner {
    stories: Vec<Story>,
    config: RunnerConfig,
}

pub struct RunnerConfig {
    pub parallelism: Parallelism,
    pub filter: Option<String>,
    pub tags: Vec<String>,
    pub reporters: Vec<Box<dyn Reporter>>,
}

pub enum Parallelism {
    Serial,
    Parallel { threads: usize },
    Auto, // Uses rayon's default
}

impl TestRunner {
    pub fn run(&self) -> TestResults {
        match self.config.parallelism {
            Parallelism::Serial => self.run_serial(),
            Parallelism::Parallel { threads } => self.run_parallel(threads),
            Parallelism::Auto => self.run_parallel(rayon::current_num_threads()),
        }
    }

    fn run_parallel(&self, threads: usize) -> TestResults {
        use rayon::prelude::*;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();

        pool.install(|| {
            self.stories.par_iter()
                .map(|story| self.run_story(story))
                .collect()
        })
    }
}
```

### 12. Optional Visual Regression

```rust
// assertions/visual.rs (optional feature)
pub struct VisualTester {
    baseline_dir: PathBuf,
    diff_threshold: f32,
}

impl VisualTester {
    pub fn capture(&self, scene: &Scene) -> Image {
        // Rasterize scene to image using software renderer
        SceneRasterizer::render(scene)
    }

    pub fn compare(&self, name: &str, current: &Image) -> VisualResult {
        let baseline_path = self.baseline_dir.join(format!("{}.png", name));

        if !baseline_path.exists() {
            current.save(&baseline_path)?;
            return VisualResult::BaselineCreated;
        }

        let baseline = Image::load(&baseline_path)?;
        let diff = image_diff(&baseline, current);

        if diff > self.diff_threshold {
            VisualResult::Changed { diff, baseline, current: current.clone() }
        } else {
            VisualResult::Matched
        }
    }
}
```

---

## Test Categories

### Unit Tests
Test individual components in isolation:
```rust
#[test]
fn signal_updates_propagate() {
    let signal = Signal::new(0);
    let memo = Memo::new(move || signal.get() * 2);

    signal.set(5);
    assert_eq!(memo.get(), 10);
}
```

### Feature Tests (Story-based)
Test user-facing features with DSL:
```rust
story!("User can create a new session")
    .tagged("session")
    .given(|cx| {
        cx.fixture::<DomainFixture>();
    })
    .when(|cx| {
        cx.actions().click(Point::new(100.0, 50.0)); // New session button
    })
    .then(|cx| {
        cx.assert_that::<SessionList>().has_session_count(1);
    })
    .run();
```

### Integration Tests
Test component interactions:
```rust
#[tokio::test]
async fn chat_service_processes_messages() {
    let mut service = MockChatService::new();
    service.queue_response(MockResponse::text("Hello!"));

    let response = service.send_message("Hi").await.unwrap();
    assert_eq!(response.text(), "Hello!");
}
```

### E2E Tests (Real Browser)
Test full application in real browser:
```rust
#[tokio::test]
async fn e2e_user_can_chat() {
    let session = WebDriverSession::new().await.unwrap();
    session.navigate("http://localhost:3000").await.unwrap();

    session.type_text("#chat-input", "Hello").await.unwrap();
    session.click("#send-button").await.unwrap();

    // Wait for response
    tokio::time::sleep(Duration::from_secs(2)).await;

    let messages = session.find_all(".message").await.unwrap();
    assert!(messages.len() >= 2); // User message + AI response
}
```

---

## Implementation Plan

### Phase 1: Core Framework
1. Create `crates/coder_test` crate
2. Implement `story!()` macro and StoryBuilder
3. Implement TestContext and basic runner
4. Add console reporter

### Phase 2: Widget Testing
5. Implement TestHarness
6. Implement MockTextSystem
7. Implement MockPaintContext and MockEventContext
8. Add UserActions API

### Phase 3: Reactive Testing
9. Implement SignalTracker
10. Implement MemoTracker
11. Implement EffectTracker
12. Add reactive assertions

### Phase 4: Platform Testing
13. Implement MockPlatform
14. Implement MockBrowserAPI
15. Add browser API simulation

### Phase 5: Fixtures & Integration
16. Implement Fixture trait and registry
17. Add DomainFixture
18. Add UIFixture
19. Add MockChatService

### Phase 6: Parallel Execution
20. Implement parallel test runner with rayon
21. Add test isolation guarantees
22. Add JSON reporter for CI

### Phase 7: E2E & Visual (Optional)
23. Integrate fantoccini for WebDriver
24. Add SceneRasterizer for visual regression
25. Add baseline management

---

## Dependencies

```toml
[dependencies]
# Core
rayon = "1.10"

# Async
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

# Serialization (for fixtures, JSON reporter)
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Optional: E2E
fantoccini = { version = "0.21", optional = true }

# Optional: Visual regression
image = { version = "0.25", optional = true }

[features]
default = []
e2e = ["fantoccini"]
visual = ["image"]
```

---

## Example: Complete User Story Test

```rust
use coder_test::prelude::*;

// Test: User can send a message and receive AI response
story!("User can chat with AI assistant")
    .tagged("chat")
    .tagged("e2e")
    .given(|cx| {
        // Set up domain state
        let domain = cx.fixture::<DomainFixture>();
        domain.with_session("test").with_thread("main");

        // Set up mock AI
        let ai = cx.fixture::<MockChatService>();
        ai.queue_streaming(&["Hello", "!", " How", " can", " I", " help", "?"]);
    })
    .when(|cx| {
        // Mount chat surface
        let harness = cx.harness();
        harness.mount(ChatSurface::new());

        // User types and sends message
        cx.actions()
            .click(Point::new(400.0, 500.0)) // Focus input
            .type_text("What is Rust?")
            .press_key(Key::Enter);

        // Wait for streaming to complete
        cx.wait_for(Duration::from_millis(100));
    })
    .then(|cx| {
        // Verify user message appears
        let scene = cx.harness().scene();
        assert_scene!(scene, contains_text "What is Rust?");

        // Verify AI response appears
        assert_scene!(scene, contains_text "Hello! How can I help?");

        // Verify message count
        cx.assert_that::<ChatSurface>().has_message_count(2);
    })
    .run();
```

---

## Running Tests

```bash
# Run all tests
cargo test -p coder_test

# Run with specific tag
cargo test -p coder_test -- --tag chat

# Run in parallel (default)
cargo test -p coder_test -- --parallel

# Run with JSON output for CI
cargo test -p coder_test -- --reporter json > results.json

# Run E2E tests (requires chromedriver)
cargo test -p coder_test --features e2e -- --tag e2e

# Run visual regression tests
cargo test -p coder_test --features visual -- --tag visual
```

---

## Summary

This framework provides:
- **Rust DSL** with `story!()` macro for readable, maintainable tests
- **Headless widget testing** via TestHarness (no GPU required)
- **Reactive testing helpers** for Signal/Memo/Effect verification
- **Platform abstraction** via MockPlatform for cross-platform testing
- **Browser simulation** for WASM testing without real browser
- **Real browser E2E** via WebDriver integration (optional)
- **Visual regression** via scene rasterization (optional)
- **Full parallel execution** with proper test isolation
- **CI-friendly** with JSON reporter

All 100 user stories can be expressed as `story!()` tests, executed in parallel, and verified across desktop, web, and mobile platforms.
