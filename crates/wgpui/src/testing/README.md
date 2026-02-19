# WGPUI E2E Test Live Viewer

A live end-to-end testing framework for WGPUI that lets you watch automated tests execute in real-time with an input overlay showing mouse/keyboard visualization.

## Overview

The testing framework provides:

- **Automated Script Execution**: Pre-defined click sequences drive the test
- **Rust DSL**: Fluent API for test specification
- **Same Window Overlay**: Input overlay shows what's being pressed/clicked
- **Live Visualization**: Watch the test execute in real-time

## Quick Start

```rust
use wgpui::testing::{test, TestHarness};
use wgpui::MouseButton;

// 1. Define a test using the fluent DSL
let login_test = test("Login Flow")
    .click("#email-input")
    .type_text("user@example.com")
    .press_tab()
    .type_text("password123")
    .click("#login-button")
    .wait(500)
    .expect("#dashboard")
    .expect_text("#welcome", "Welcome back!")
    .build();

// 2. Wrap your component in a TestHarness
let harness = TestHarness::new(my_login_component)
    .with_runner(login_test)
    .show_overlay(true)
    .show_controls(true);

// 3. Render as normal - the harness handles test execution
window.render_root(&mut harness);
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    TestHarness<C>                        │
│  ┌─────────────────────────────────────────────────────┐ │
│  │              Control Bar (play/pause/step)          │ │
│  ├─────────────────────────────────────────────────────┤ │
│  │                                                      │ │
│  │              Component Under Test                    │ │
│  │                                                      │ │
│  ├─────────────────────────────────────────────────────┤ │
│  │              InputOverlay                            │ │
│  │  • Mouse cursor crosshair                           │ │
│  │  • Click ripple animations                          │ │
│  │  • Key press display                                │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│  TestRunner ──► EventInjection ──► Component.event()    │
└─────────────────────────────────────────────────────────┘
```

## DSL Reference

### Creating a Test

```rust
use wgpui::testing::test;

let my_test = test("Test Name")
    // ... add steps ...
    .build();
```

### Mouse Actions

| Method | Description |
|--------|-------------|
| `.click(selector)` | Left-click on an element |
| `.click_at(x, y)` | Click at specific coordinates |
| `.right_click(selector)` | Right-click on an element |
| `.double_click(selector)` | Double-click on an element |
| `.hover(selector)` | Move mouse to element without clicking |

```rust
test("Mouse Actions")
    .click("#submit-button")           // Click by ID
    .click("text:Submit")              // Click by visible text
    .click_at(100.0, 200.0)            // Click at coordinates
    .right_click("#context-menu-target")
    .double_click("#editable-text")
    .hover("#tooltip-trigger")
    .build();
```

### Keyboard Actions

| Method | Description |
|--------|-------------|
| `.type_text(text)` | Type text with 50ms delay per character |
| `.type_instant(text)` | Type text instantly |
| `.press_key(key)` | Press a single key |
| `.press_key_with(key, modifiers)` | Press key with modifiers |
| `.press_enter()` | Press Enter key |
| `.press_tab()` | Press Tab key |
| `.press_escape()` | Press Escape key |
| `.press_backspace()` | Press Backspace key |

```rust
use wgpui::{Key, NamedKey, Modifiers};

test("Keyboard Actions")
    .type_text("Hello, World!")        // Type with delay
    .type_instant("instant text")      // Type immediately
    .press_enter()
    .press_tab()
    .press_key(Key::Named(NamedKey::ArrowDown))
    .press_key_with(
        Key::Character("s".to_string()),
        Modifiers { ctrl: true, ..Default::default() }
    )  // Ctrl+S
    .build();
```

### Scroll Actions

| Method | Description |
|--------|-------------|
| `.scroll(selector, dx, dy)` | Scroll an element |
| `.scroll_up(selector, amount)` | Scroll up by amount |
| `.scroll_down(selector, amount)` | Scroll down by amount |

```rust
test("Scrolling")
    .scroll("#scroll-container", 0.0, -100.0)  // Scroll down
    .scroll_down("#list", 50.0)
    .scroll_up("#list", 25.0)
    .build();
```

### Timing

| Method | Description |
|--------|-------------|
| `.wait(ms)` | Wait for a duration in milliseconds |
| `.wait_for(selector)` | Wait for element to appear (5s timeout) |
| `.wait_for_timeout(selector, ms)` | Wait with custom timeout |

```rust
test("Timing")
    .click("#async-action")
    .wait(1000)                        // Wait 1 second
    .wait_for("#loading-complete")     // Wait for element
    .wait_for_timeout("#slow-element", 10000)  // 10s timeout
    .build();
```

### Assertions

| Method | Description |
|--------|-------------|
| `.expect(selector)` | Assert element exists |
| `.expect_text(selector, text)` | Assert element contains text |
| `.expect_visible(selector)` | Assert element is visible |

```rust
test("Assertions")
    .click("#login")
    .expect("#dashboard")              // Element exists
    .expect_text("#welcome", "Hello")  // Has text
    .expect_visible("#main-content")   // Is visible
    .build();
```

## Element Selectors

Elements can be selected using several patterns:

| Pattern | Description | Example |
|---------|-------------|---------|
| `#123` | By ComponentId | `"#42"` |
| `text:Content` | By visible text | `"text:Submit"` |
| Plain string | Treated as text search | `"Submit"` |

```rust
// By component ID
.click("#42")

// By visible text (explicit)
.click("text:Click Me")

// By visible text (implicit - plain strings search for text)
.click("Submit")
```

### Programmatic Selectors

For more control, use the `ElementSelector` enum directly:

```rust
use wgpui::testing::{ElementSelector, ClickTarget};

// By ID
let by_id = ElementSelector::Id(42);

// By text
let by_text = ElementSelector::Text("Submit".to_string());

// By bounds (for pixel-perfect testing)
let by_bounds = ElementSelector::Bounds(Bounds::new(10.0, 20.0, 100.0, 50.0));

// Query string (parsed at runtime)
let by_query = ElementSelector::Query("#42".to_string());
```

## TestRunner

The `TestRunner` manages test execution state:

```rust
use wgpui::testing::{TestRunner, RunnerState, PlaybackSpeed};

// Create from DSL
let mut runner = test("My Test")
    .click("#button")
    .build();

// Control playback
runner.start();           // Begin execution
runner.pause();           // Pause execution
runner.resume();          // Resume from pause
runner.step();            // Execute single step
runner.abort();           // Stop and mark aborted

// Playback speed
runner.set_speed(PlaybackSpeed::SLOW);     // 0.5x
runner.set_speed(PlaybackSpeed::NORMAL);   // 1.0x
runner.set_speed(PlaybackSpeed::FAST);     // 2.0x
runner.set_speed(PlaybackSpeed::INSTANT);  // 10.0x

// Check state
match runner.state() {
    RunnerState::Idle => println!("Not started"),
    RunnerState::Running => println!("Executing"),
    RunnerState::Paused => println!("Paused"),
    RunnerState::Stepping => println!("Single-stepping"),
    RunnerState::Passed => println!("All assertions passed"),
    RunnerState::Failed => println!("Test failed"),
    RunnerState::Aborted => println!("Manually stopped"),
}

// Progress
let (current, total) = runner.progress();
println!("Step {}/{}", current + 1, total);
```

## TestHarness

The `TestHarness` wraps your component and provides the test execution environment:

```rust
use wgpui::testing::TestHarness;

// Create harness
let harness = TestHarness::new(my_component)
    .with_runner(test_runner)
    .show_overlay(true)      // Show input visualization
    .show_controls(true);    // Show control bar

// Configure overlay
let harness = TestHarness::new(my_component)
    .with_runner(runner)
    .overlay_cursor_size(20.0)      // Bigger cursor
    .overlay_ripple_radius(40.0);   // Bigger ripples
```

### Control Bar

When `show_controls(true)` is set, a control bar appears at the top:

```
┌────────────────────────────────────────────────────────────┐
│ ● RUNNING  │  Step 3/10  │  [P]lay [S]tep [Space] [1-4]   │
└────────────────────────────────────────────────────────────┘
```

**Keyboard Shortcuts:**
- `P` - Start/resume playback
- `S` - Execute single step
- `Space` - Pause/resume
- `Escape` - Abort test
- `1` - Slow speed (0.5x)
- `2` - Normal speed (1.0x)
- `3` - Fast speed (2.0x)
- `4` - Instant speed (10.0x)

## InputOverlay

The overlay visualizes test input:

### Mouse Cursor
A crosshair follows the virtual mouse position.

### Click Ripples
When a click occurs, an expanding circle animation plays:
- Left click: Primary accent color
- Right click: Warning color
- Middle click: Info color
- Duration: 400ms with EaseOutQuad easing

### Key Display
Recent key presses appear in a stack (configurable position):
- Shows up to 5 recent keys
- Fades out after 800ms
- Displays modifier combinations (Ctrl+S, Cmd+Enter, etc.)

```rust
use wgpui::testing::{InputOverlay, KeyDisplayPosition};

// Customize overlay
let overlay = InputOverlay::new()
    .with_cursor_size(16.0)
    .with_ripple_radius(30.0)
    .with_key_position(KeyDisplayPosition::BottomRight);
```

## ComponentRegistry

During test execution, the `ComponentRegistry` tracks element positions:

```rust
use wgpui::testing::ComponentRegistry;

let mut registry = ComponentRegistry::new();

// Register elements during paint
registry.register_id(42, bounds);
registry.register_text("Submit", bounds);

// Look up elements
let btn_bounds = registry.find_by_id(42);
let text_bounds = registry.find_by_text("Submit");
let center = registry.center_of_id(42);
```

For your component to be testable, it should register itself during painting:

```rust
impl Component for MyButton {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Register for testing if registry is available
        if let Some(id) = self.id {
            // The harness provides access to registry through context
            // (implementation depends on your integration approach)
        }

        // Normal painting...
    }
}
```

## Event Injection

The framework generates synthetic events for each test step:

```rust
use wgpui::testing::{EventSequence, EventPlayer};
use wgpui::MouseButton;

// Create event sequences manually
let click_seq = EventSequence::click(100.0, 200.0, MouseButton::Left);
// Generates: MouseMove, MouseDown (10ms), MouseUp (50ms)

let type_seq = EventSequence::type_text("hello", Some(Duration::from_millis(30)));
// Generates: KeyDown, KeyUp pairs for each character

let scroll_seq = EventSequence::scroll(100.0, 100.0, 0.0, -50.0);
// Generates: MouseMove, Scroll

// Play events with timing
let mut player = EventPlayer::new(click_seq);
player.start();

// Poll for next event (respects timing)
while let Some(event) = player.poll() {
    component.event(&event, bounds, cx);
}

// Or drain all events immediately
let all_events = player.drain();
```

## Complete Example

```rust
use wgpui::{App, Window, Component, Bounds};
use wgpui::testing::{test, TestHarness, PlaybackSpeed};

// Define your component
struct Counter {
    count: i32,
}

impl Component for Counter {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Paint counter UI...
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Handle increment/decrement clicks...
    }
}

fn main() {
    // Create test
    let counter_test = test("Counter Increment")
        .click("#increment")
        .expect_text("#count", "1")
        .click("#increment")
        .click("#increment")
        .expect_text("#count", "3")
        .click("#decrement")
        .expect_text("#count", "2")
        .build();

    // Set speed
    counter_test.set_speed(PlaybackSpeed::SLOW);

    // Create harness
    let counter = Counter { count: 0 };
    let harness = TestHarness::new(counter)
        .with_runner(counter_test)
        .show_overlay(true)
        .show_controls(true);

    // Run in your app
    let app = App::new();
    let window = Window::new(&app, "Counter Test");

    // The harness manages everything
    window.run(|cx| {
        harness.paint(window_bounds, cx);
    });
}
```

## Test Assertions

Assertions are checked during test execution:

```rust
use wgpui::testing::{TestAssertion, AssertionResult};

// Built-in assertions
TestAssertion::ElementExists(selector)      // Element is in registry
TestAssertion::TextEquals(selector, text)   // Element has exact text
TestAssertion::Visible(selector)            // Element bounds are valid

// Results
match result {
    AssertionResult::Passed => { /* continue */ }
    AssertionResult::Failed(message) => {
        eprintln!("Assertion failed: {}", message);
    }
}
```

## Module Structure

```
wgpui/src/testing/
├── mod.rs          # Public exports
├── step.rs         # TestStep, ElementSelector, ClickTarget
├── assertion.rs    # TestAssertion, AssertionResult
├── context.rs      # ComponentRegistry, TestContext
├── runner.rs       # TestRunner, RunnerState, PlaybackSpeed
├── dsl.rs          # Fluent Test builder
├── injection.rs    # EventSequence, EventPlayer
├── overlay.rs      # InputOverlay component
└── harness.rs      # TestHarness wrapper
```

## Imports

```rust
// Import everything you need
use wgpui::testing::{
    // DSL
    test, Test,

    // Runner
    TestRunner, RunnerState, PlaybackSpeed,

    // Harness
    TestHarness,

    // Overlay
    InputOverlay, ClickRipple, KeyDisplay, KeyDisplayPosition,

    // Event injection
    EventSequence, EventPlayer, TimedEvent,

    // Selectors
    TestStep, ElementSelector, ClickTarget,

    // Assertions
    TestAssertion, AssertionResult,

    // Context
    ComponentRegistry, TestContext,
};
```

## Tips

1. **Use IDs for stable selectors**: Text content may change, but ComponentIds are stable.

2. **Add waits after async actions**: If clicking triggers an async operation, use `.wait()` or `.wait_for()`.

3. **Start with slow speed**: Use `PlaybackSpeed::SLOW` when debugging to see exactly what's happening.

4. **Check assertions carefully**: The test will mark as `Failed` on the first failed assertion.

5. **Register components during paint**: Your components need to register their bounds for selectors to work.

## Troubleshooting

### "Could not resolve click target"
The selector didn't match any registered element. Check that:
- The element is registered in ComponentRegistry
- The selector syntax is correct (`#id` for ID, `text:` for text)
- The element has been painted at least once

### Test hangs on wait_for
The element never appeared within the timeout. Either:
- Increase the timeout with `wait_for_timeout`
- Check that the element actually gets rendered
- Verify the selector matches the element

### Events not reaching component
Make sure:
- The TestHarness is properly wrapping your component
- The harness's event method is being called
- Events aren't being consumed before reaching your component
