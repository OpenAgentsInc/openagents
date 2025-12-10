# GPUI Complete Guide: GPU-Accelerated UI Framework

> **Source:** Based on analysis of `/Users/christopherdavid/code/zed/crates/gpui`  
> **Framework:** GPUI (GPU-accelerated UI framework from Zed team)  
> **Version:** 0.2.2  
> **Website:** https://www.gpui.rs/

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Core Concepts](#core-concepts)
4. [Application Setup](#application-setup)
5. [Views and Rendering](#views-and-rendering)
6. [State Management](#state-management)
7. [Styling System](#styling-system)
8. [Events and Interactions](#events-and-interactions)
9. [Async Support](#async-support)
10. [Platform Integration](#platform-integration)
11. [Migration from Effuse](#migration-from-effuse)
12. [Best Practices](#best-practices)

---

## Overview

GPUI is a **hybrid immediate and retained mode, GPU-accelerated UI framework** for Rust, designed to support a wide variety of applications. It's the framework powering the Zed code editor.

### Key Features

- **GPU Acceleration:** Direct-to-GPU rendering for high performance
- **Hybrid Mode:** Combines immediate mode simplicity with retained mode efficiency
- **Rust-Native:** Built entirely in Rust with memory safety
- **Cross-Platform:** macOS (Metal), Windows (DirectX), Linux (Wayland/X11)
- **Entity System:** Built-in state management with observation/subscription
- **Tailwind-like Styling:** Familiar styling API inspired by Tailwind CSS
- **Async Executor:** Integrated async executor with platform event loop
- **Keyboard-First:** Designed for keyboard-first interactivity with action system

### Platform Support

- **macOS:** Metal rendering, Core Text for fonts
- **Windows:** DirectX rendering, DirectWrite for fonts
- **Linux:** Blade (Vulkan) rendering, Wayland/X11 support, Cosmic Text for fonts

---

## Architecture

### Three Registers

GPUI offers three different "registers" (levels of abstraction) depending on your needs:

1. **Entity System** - State management and communication
2. **Views** - High-level, declarative UI (implements `Render` trait)
3. **Elements** - Low-level, imperative UI building blocks

### Rendering Pipeline

```
Application
    └─> Window (root view)
        └─> View::render() called each frame
            └─> Builds Element tree
                └─> Taffy layout engine
                    └─> Element::paint() called
                        └─> GPU rendering
```

### Ownership Model

- **App** owns all entity state
- **Entity<T>** is a handle (like `Rc`) to state owned by App
- Access to state requires `&mut App` or `&mut Context<T>`
- Reference counting enables shared ownership

---

## Core Concepts

### Entity<T>

An `Entity<T>` is a handle to state owned by the `App`. It's similar to `Rc<T>` but requires an `App` context to access the data.

```rust
// Create an entity
let counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });

// Read state
let count = counter.read(cx).count;

// Update state
counter.update(cx, |counter, cx| {
    counter.count += 1;
    cx.notify(); // Notify observers
});
```

**Key Methods:**
- `read(cx)` - Read state (immutable)
- `update(cx, f)` - Update state (mutable)
- `observe(cx, entity, callback)` - Observe another entity
- `subscribe(cx, entity, callback)` - Subscribe to events from another entity

### View

A `View` is an `Entity` that implements the `Render` trait. Views are the high-level UI components.

```rust
struct MyView {
    count: Entity<Counter>,
}

impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.count.read(cx).count;
        div()
            .text(format!("Count: {}", count))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.count.update(cx, |c, _| c.count += 1);
            }))
    }
}
```

### Context<T>

A `Context<T>` provides access to both the `App` and entity-specific services. It dereferences to `App`, so you can use any `App` method.

```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Context provides:
        // - App methods (cx.new(), cx.open_window(), etc.)
        // - Entity methods (cx.notify(), cx.observe(), etc.)
        // - Entity ID (cx.entity_id())
        
        cx.notify(); // Notify observers of this entity
        div()
    }
}
```

### Element

Elements are the low-level building blocks. Most of the time you'll use high-level elements like `div()`, but you can implement `Element` for custom rendering.

**Built-in Elements:**
- `div()` - Container element (most common)
- `text()` - Text rendering
- `img()` - Image rendering
- `svg()` - SVG rendering
- `canvas()` - Custom painting
- `list()` - Virtualized lists
- `uniform_list()` - Uniform item lists

### App

The root application context that owns all state and provides application-level services.

```rust
Application::new().run(|cx: &mut App| {
    // Application-level setup
    cx.open_window(WindowOptions::default(), |_, cx| {
        cx.new(|_cx| MyView::new())
    });
});
```

---

## Application Setup

### Basic Application

```rust
use gpui::{Application, App, WindowOptions, WindowBounds, Bounds, size, px};

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.0), px(600.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_cx| MyView {
                    // Initialize view state
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

### Application Configuration

```rust
Application::new()
    .with_assets(asset_source)  // Custom asset source
    .with_http_client(http_client)  // Custom HTTP client
    .with_quit_mode(QuitMode::Default)  // Quit behavior
    .run(|cx| {
        // ...
    });
```

### Headless Mode

For testing or server environments:

```rust
Application::headless().run(|cx| {
    // No windows, but full app functionality
});
```

---

## Views and Rendering

### Basic View

```rust
struct CounterView {
    count: u32,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_4()
            .child(format!("Count: {}", self.count))
            .child(
                div()
                    .text("Increment")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.count += 1;
                        cx.notify();
                    }))
            )
    }
}
```

### View with Entity State

```rust
struct Counter {
    count: u32,
}

struct CounterView {
    counter: Entity<Counter>,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.counter.read(cx).count;
        div()
            .text(format!("Count: {}", count))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.counter.update(cx, |counter, _| {
                    counter.count += 1;
                });
            }))
    }
}
```

### View Composition

```rust
struct AppView {
    header: Entity<HeaderView>,
    content: Entity<ContentView>,
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.header.clone())
            .child(self.content.clone())
    }
}
```

---

## State Management

### Entity System

The Entity system is GPUI's built-in state management. Entities are owned by `App` and accessed via handles.

#### Creating Entities

```rust
// In App context
let counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });

// In View context
let counter = cx.new(|_cx| Counter { count: 0 });
```

#### Reading State

```rust
// Read entire state
let counter = entity.read(cx);
let count = counter.count;

// Read with callback
entity.read(cx, |counter| {
    // Use counter
});
```

#### Updating State

```rust
// Update with callback
entity.update(cx, |counter, cx| {
    counter.count += 1;
    cx.notify(); // Notify observers
});

// Update and return value
let new_count = entity.update(cx, |counter, _| {
    counter.count += 1;
    counter.count
});
```

### Observation

Observe changes to entities:

```rust
// Observe another entity
cx.observe(&other_entity, |this, other, cx| {
    this.value = other.read(cx).value;
})
.detach(); // Keep subscription alive

// Observe self
cx.observe_self(|this, cx| {
    // React to own changes
})
.detach();
```

### Event Emission

For typed events:

```rust
// Define event
struct CounterChanged {
    new_value: u32,
}

// Implement EventEmitter
impl EventEmitter<CounterChanged> for Counter {}

// Emit event
cx.emit(CounterChanged { new_value: 42 });

// Subscribe to events
cx.subscribe(&counter, |this, counter, event, cx| {
    this.display_value = event.new_value;
})
.detach();
```

### Global State

For application-wide state:

```rust
// Define global
struct AppSettings {
    theme: String,
}

impl Global for AppSettings {}

// Set global
cx.set_global(AppSettings { theme: "dark".into() });

// Update global
cx.update_global(|settings, _| {
    settings.theme = "light".into();
});

// Read global
let theme = cx.read_global(|settings, _| settings.theme.clone());

// Observe global
cx.observe_global(|this, cx| {
    let theme = cx.read_global(|s, _| s.theme.clone());
    this.current_theme = theme;
})
.detach();
```

---

## Styling System

GPUI provides a Tailwind-like styling API with fluent builders.

### Basic Styling

```rust
div()
    .flex()                    // display: flex
    .flex_col()                // flex-direction: column
    .items_center()            // align-items: center
    .justify_between()         // justify-content: space-between
    .gap_4()                   // gap: 1rem
    .p_4()                     // padding: 1rem
    .bg(rgb(0x1a1a1a))         // background color
    .text_color(rgb(0xffffff)) // text color
    .border_1()                // border: 1px
    .border_color(rgb(0x0000ff)) // border color
    .rounded_md()              // border-radius: medium
    .shadow_lg()               // box-shadow: large
```

### Layout Properties

```rust
div()
    .size(px(500.0), px(300.0))  // width, height
    .w_full()                     // width: 100%
    .h_full()                     // height: 100%
    .min_w(px(200.0))            // min-width
    .max_w(px(800.0))            // max-width
    .flex()                       // display: flex
    .flex_row()                   // flex-direction: row
    .flex_col()                   // flex-direction: column
    .items_start()                // align-items: flex-start
    .items_center()                // align-items: center
    .items_end()                   // align-items: flex-end
    .justify_start()              // justify-content: flex-start
    .justify_center()             // justify-content: center
    .justify_end()                // justify-content: flex-end
    .justify_between()            // justify-content: space-between
    .gap_2()                      // gap: 0.5rem
    .gap_4()                      // gap: 1rem
```

### Colors

```rust
// RGB
rgb(0x1a1a1a)
rgba(0x1a1a1a, 0.5)  // with alpha

// HSLA
hsla(0.0, 0.0, 0.5, 1.0)

// Predefined
gpui::black()
gpui::white()
gpui::red()
gpui::green()
gpui::blue()
gpui::yellow()

// Color operations
color.blend(other_color, opacity)
color.darken(amount)
color.lighten(amount)
```

### Typography

```rust
div()
    .text_size(px(16.0))         // font-size
    .text_color(rgb(0xffffff))   // color
    .font_weight(FontWeight::Bold)
    .font_style(FontStyle::Italic)
    .line_height(px(24.0))       // line-height
    .text_align(TextAlign::Center)
```

### Spacing

```rust
div()
    .p_2()    // padding: 0.5rem
    .p_4()    // padding: 1rem
    .px_2()   // padding-left/right: 0.5rem
    .py_4()   // padding-top/bottom: 1rem
    .pt_2()   // padding-top: 0.5rem
    .pb_4()   // padding-bottom: 1rem
    .pl_2()   // padding-left: 0.5rem
    .pr_4()   // padding-right: 1rem
    .m_2()    // margin: 0.5rem
    .mx_4()   // margin-left/right: 1rem
    .my_2()   // margin-top/bottom: 0.5rem
```

### Borders

```rust
div()
    .border_1()                  // border: 1px
    .border_2()                  // border: 2px
    .border_color(rgb(0x0000ff)) // border color
    .border_radius(px(8.0))      // border-radius
    .rounded_md()                 // border-radius: medium
    .rounded_lg()                 // border-radius: large
    .border_solid()              // border-style: solid
    .border_dashed()             // border-style: dashed
    .border_dotted()             // border-style: dotted
```

### Shadows

```rust
div()
    .shadow_sm()  // small shadow
    .shadow_md()  // medium shadow
    .shadow_lg()  // large shadow
    .shadow_xl()  // extra large shadow
```

### Hover States

```rust
div()
    .hover(|style| {
        style
            .bg(rgb(0x2a2a2a))
            .cursor_pointer()
    })
```

### Conditional Styling

```rust
div()
    .when(some_condition, |style| {
        style.bg(rgb(0xff0000))
    })
    .when_else(some_condition, |style| {
        style.bg(rgb(0x00ff00))
    }, |style| {
        style.bg(rgb(0x0000ff))
    })
```

---

## Events and Interactions

### Mouse Events

```rust
div()
    .on_mouse_down(MouseButton::Left, cx.listener(|this, event, window, cx| {
        // Handle mouse down
    }))
    .on_mouse_up(MouseButton::Left, cx.listener(|this, event, window, cx| {
        // Handle mouse up
    }))
    .on_mouse_move(cx.listener(|this, event, window, cx| {
        // Handle mouse move
    }))
    .on_click(cx.listener(|this, event, window, cx| {
        // Handle click (mouse down + up on same element)
    }))
    .on_double_click(cx.listener(|this, event, window, cx| {
        // Handle double click
    }))
    .hover(|style| {
        style.cursor_pointer()
    })
```

### Keyboard Events

GPUI uses an **action system** for keyboard input (keyboard-first design).

#### Define Actions

```rust
// Simple action
#[gpui::action]
struct Increment;

// Or use macro
actions!(counter, [Increment, Decrement]);

// Complex action
#[gpui::action]
struct SetValue {
    value: u32,
}
```

#### Bind Actions to Keys

```rust
cx.bind_keys([
    KeyBinding::new("up", Increment, None),
    KeyBinding::new("down", Decrement, None),
    KeyBinding::new("cmd-s", Save, None),
]);
```

#### Handle Actions in Views

```rust
impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("counter")  // Declare key context
            .on_action(cx.listener(|this, _: &Increment, _window, cx| {
                this.count += 1;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Decrement, _window, cx| {
                this.count -= 1;
                cx.notify();
            }))
    }
}
```

#### Key Contexts

Key contexts allow different key bindings in different parts of the UI:

```rust
div()
    .key_context("menu")  // This subtree uses "menu" context
    .on_action(|this, action, window, cx| {
        // Handle actions in menu context
    })
```

### Focus Management

```rust
struct MyView {
    focus_handle: FocusHandle,
}

impl Focusable for MyView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle(cx))
            .when(self.focus_handle.is_focused(window), |style| {
                style.border_color(rgb(0x0000ff))
            })
    }
}

// Focus programmatically
window.focus(&view.focus_handle(cx));
```

### Drag and Drop

```rust
div()
    .on_drag_start(cx.listener(|this, event, window, cx| {
        // Start drag
        Some(DragData { /* ... */ })
    }))
    .on_drag_move(cx.listener(|this, event, window, cx| {
        // Handle drag over this element
    }))
    .on_drop(cx.listener(|this, event, window, cx| {
        // Handle drop
    }))
```

---

## Async Support

GPUI has an integrated async executor that works with the platform event loop.

### Spawning Async Tasks

```rust
// Spawn from Context
cx.spawn(|this, cx| async move {
    let result = some_async_operation().await;
    this.update(&mut cx, |this, _| {
        this.data = result;
    }).ok();
})
.detach();

// Spawn from App
cx.background_spawn(async {
    // Background task
})
.await;
```

### Async Context

For holding context across await points:

```rust
let async_cx = cx.to_async();
cx.spawn(|this, _| async move {
    let result = fetch_data().await;
    // Use async_cx here
    this.update(&mut async_cx, |this, _| {
        this.data = result;
    }).ok();
})
.detach();
```

### Observing Async Operations

```rust
cx.observe(&data_entity, |this, data, cx| {
    // Spawn async operation when data changes
    cx.spawn(|this, cx| async move {
        let processed = process_data(data.read(&cx).value).await;
        this.update(&mut cx, |this, _| {
            this.processed = processed;
        }).ok();
    })
    .detach();
})
.detach();
```

---

## Platform Integration

### Window Management

```rust
// Open window
let window = cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_background: WindowBackgroundAppearance::default(),
        focus: true,
        show: true,
        center: true,
        ..Default::default()
    },
    |_, cx| {
        cx.new(|_cx| MyView::new())
    },
)?;

// Update window
window.update(cx, |view, window, cx| {
    window.set_title("New Title");
});

// Close window
window.close(cx);
```

### Clipboard

```rust
// Read clipboard
let item = cx.read_from_clipboard();
if let Some(text) = item.and_then(|i| i.text()) {
    // Use text
}

// Write clipboard
cx.write_to_clipboard(ClipboardItem::new_string("Hello"));
```

### Menus

```rust
cx.set_menus(vec![
    OwnedMenu::new("File")
        .children(vec![
            MenuItem::action("New", NewFile),
            MenuItem::action("Open", OpenFile),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ]),
]);
```

### Dialogs

```rust
// Show prompt
let prompt = cx.prompt(
    PromptLevel::Info,
    "Confirm",
    "Are you sure?",
    vec![
        PromptButton::primary("Yes", |cx| {
            // Handle yes
        }),
        PromptButton::secondary("No", |cx| {
            // Handle no
        }),
    ],
);
```

---

## Migration from Effuse

### Component → View

**Effuse:**
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

**GPUI:**
```rust
struct Counter {
    count: u32,
}

struct CounterView {
    counter: Entity<Counter>,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.counter.read(cx).count;
        div()
            .text(format!("Count: {}", count))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.counter.update(cx, |counter, _| {
                    counter.count += 1;
                });
            }))
    }
}
```

### StateCell → Entity

**Effuse:**
```typescript
const state = yield* StateService.make(initial);
yield* state.update(s => ({ ...s, count: s.count + 1 }));
const value = yield* state.get;
```

**GPUI:**
```rust
let counter = cx.new(|_cx| Counter { count: 0 });
counter.update(cx, |counter, _| {
    counter.count += 1;
});
let count = counter.read(cx).count;
```

### HTML Templates → GPUI Elements

**Effuse:**
```typescript
html`
  <div class="container">
    <h1>${title}</h1>
    <button data-action="increment">+</button>
  </div>
`
```

**GPUI:**
```rust
div()
    .class("container")  // or use styling methods
    .child(div().text(title))
    .child(
        div()
            .text("+")
            .on_click(cx.listener(|this, _event, _window, cx| {
                // Handle increment
            }))
    )
```

### Event Delegation → Direct Handlers

**Effuse:**
```typescript
setupEvents: (ctx) =>
  Effect.gen(function* () {
    yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
      const action = (target as HTMLElement).dataset.action;
      Effect.runFork(ctx.emit({ type: action }));
    });
  })
```

**GPUI:**
```rust
// Direct event handlers - no delegation needed
div()
    .on_click(cx.listener(|this, event, window, cx| {
        // Handle click
    }))
```

### Services → Context

**Effuse:**
```typescript
const fs = yield* FileSystem.FileSystem;
const content = yield* fs.readFileString(path);
```

**GPUI:**
```rust
// Services accessed through App/Context
// File operations would be through platform APIs or custom services
// GPUI doesn't have built-in file system service
```

### Subscriptions → Observe/Subscribe

**Effuse:**
```typescript
subscriptions: (ctx) => {
  return [
    pipe(
      Stream.unwrap(Effect.map(socket, s => s.getMessages())),
      Stream.filter(msg => msg.type === "my_event"),
      Stream.map(msg => ctx.state.update(s => ({ ...s, data: msg.data })))
    ),
  ];
}
```

**GPUI:**
```rust
// Observe entity changes
cx.observe(&data_entity, |this, data, cx| {
    this.data = data.read(cx).value;
})
.detach();

// Subscribe to events
cx.subscribe(&emitter, |this, emitter, event, cx| {
    this.handle_event(event);
})
.detach();
```

---

## Best Practices

### 1. Entity vs View State

- **Use Entity** for:
  - Shared state between multiple views
  - State that needs observation
  - State that should persist across view recreations
  
- **Use View fields** for:
  - View-specific UI state (hover, focus, etc.)
  - Temporary state that doesn't need observation
  - State that's only used in render

### 2. When to Call `notify()`

Call `cx.notify()` after updating entity state that should trigger re-renders:

```rust
counter.update(cx, |counter, cx| {
    counter.count += 1;
    cx.notify(); // Triggers observers and re-renders
});
```

### 3. Context Usage

- Use `Context<T>` in `Render::render()` and entity callbacks
- Use `App` for application-level operations
- Use `AsyncApp` for async operations

### 4. Performance

- Use `Entity` observation instead of polling
- Cache expensive computations in view state
- Use `cached()` on views that don't change often
- Prefer `observe` over `read` in loops

### 5. Styling

- Use fluent builder methods (`.flex()`, `.gap_4()`, etc.)
- Chain style methods for readability
- Use conditional styling with `.when()`
- Prefer style methods over CSS classes when possible

### 6. Event Handling

- Use `cx.listener()` for view state access in callbacks
- Prefer actions for keyboard shortcuts
- Use key contexts for scoped key bindings
- Handle focus properly with `FocusHandle`

### 7. Async Operations

- Always use `cx.spawn()` for async operations
- Convert to `AsyncApp` for holding context across await
- Detach tasks that should run independently
- Handle errors properly in async callbacks

---

## Examples

### Complete Counter Example

```rust
use gpui::*;

struct Counter {
    count: u32,
}

struct CounterView {
    counter: Entity<Counter>,
}

impl Render for CounterView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.counter.read(cx).count;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .p_4()
            .bg(rgb(0x1a1a1a))
            .text_color(rgb(0xffffff))
            .child(div().text(format!("Count: {}", count)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .text("+")
                            .px_4()
                            .py_2()
                            .bg(rgb(0x00ff00))
                            .rounded_md()
                            .hover(|style| style.bg(rgb(0x00cc00)))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.counter.update(cx, |counter, cx| {
                                    counter.count += 1;
                                    cx.notify();
                                });
                            }))
                    )
                    .child(
                        div()
                            .text("-")
                            .px_4()
                            .py_2()
                            .bg(rgb(0xff0000))
                            .rounded_md()
                            .hover(|style| style.bg(rgb(0xcc0000)))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.counter.update(cx, |counter, cx| {
                                    counter.count = counter.count.saturating_sub(1);
                                    cx.notify();
                                });
                            }))
                    )
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(400.0), px(300.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let counter = cx.new(|_cx| Counter { count: 0 });
                cx.new(|_cx| CounterView { counter })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

---

## Resources

- **Website:** https://www.gpui.rs/
- **GitHub:** https://github.com/zed-industries/zed (GPUI is part of Zed)
- **Documentation:** See `docs/` directory in GPUI crate
- **Examples:** See `examples/` directory in GPUI crate
- **Zed Source:** Best reference for real-world GPUI usage

---

**Last Updated:** 2025-12-09  
**Based on:** GPUI crate analysis from `/Users/christopherdavid/code/zed/crates/gpui`

