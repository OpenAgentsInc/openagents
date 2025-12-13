# Integration Guide

## Overview

wgpui is designed to be embedded in larger applications. This guide covers integration with Dioxus and general embedding patterns.

## Dioxus Integration

### Hybrid Architecture

The recommended pattern is a hybrid where Dioxus handles the application shell and wgpui handles performance-critical surfaces.

```
┌─────────────────────────────────────────────────────────────┐
│  Dioxus Application Shell                                    │
│  ├── Auth, routing, settings, forms                         │
│  └── HTML input (IME support)                               │
├─────────────────────────────────────────────────────────────┤
│  wgpui Canvas Surfaces                                       │
│  ├── Chat message thread                                    │
│  ├── Terminal emulator                                      │
│  └── Diff viewer, timeline, graphs                          │
└─────────────────────────────────────────────────────────────┘
```

### WgpuiCanvas Component

Create a Dioxus component that hosts a wgpui canvas:

```rust
// crates/dioxus/src/components/wgpui_canvas.rs
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq, Props)]
pub struct WgpuiCanvasProps {
    pub id: String,
    pub entries: Signal<Vec<ThreadEntry>>,
    pub streaming_text: Signal<String>,
}

#[component]
pub fn WgpuiCanvas(props: WgpuiCanvasProps) -> Element {
    let platform = use_signal(|| None::<Rc<RefCell<WebPlatform>>>);

    // Initialize on mount
    use_effect(move || {
        spawn(async move {
            let p = WebPlatform::init(&props.id).await.unwrap();
            platform.set(Some(Rc::new(RefCell::new(p))));
        });
    });

    // Re-render when state changes
    use_effect(move || {
        if let Some(p) = platform() {
            let mut platform = p.borrow_mut();
            let mut scene = build_chat_scene(
                &mut platform,
                props.entries(),
                props.streaming_text(),
            );
            platform.render(&scene).ok();
        }
    });

    rsx! {
        canvas {
            id: "{props.id}",
            style: "width: 100%; height: 100%; display: block;",
        }
    }
}
```

### Signal Bridge

Bridge Dioxus signals to wgpui state:

```rust
fn build_chat_scene(
    platform: &mut WebPlatform,
    entries: Vec<ThreadEntry>,
    streaming_text: String,
) -> Scene {
    let mut scene = Scene::new();
    let size = platform.logical_size();

    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, size.width, size.height))
            .with_background(theme::bg::APP)
    );

    // Render messages
    let mut y = 16.0;
    for entry in entries {
        match entry {
            ThreadEntry::Message(msg) => {
                let text = platform.text_system().layout(
                    &msg.content,
                    Point::new(16.0, y),
                    13.0,
                    if msg.role == "user" {
                        theme::accent::PRIMARY
                    } else {
                        theme::text::PRIMARY
                    }
                );
                scene.draw_text(text);
                y += 24.0;
            }
            ThreadEntry::ToolUse(tool) => {
                // Render tool status
                y += 20.0;
            }
        }
    }

    // Streaming text
    if !streaming_text.is_empty() {
        let text = platform.text_system().layout(
            &streaming_text,
            Point::new(16.0, y),
            13.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(text);
    }

    scene
}
```

### Event Handling

Handle canvas events and forward to Dioxus:

```rust
#[component]
pub fn WgpuiCanvas(props: WgpuiCanvasProps) -> Element {
    rsx! {
        canvas {
            id: "{props.id}",
            style: "width: 100%; height: 100%;",

            // Forward events to wgpui
            onmousedown: move |e| {
                let pos = Point::new(e.client_x() as f32, e.client_y() as f32);
                // Handle click in wgpui
            },

            onwheel: move |e| {
                let delta = Point::new(e.delta_x() as f32, e.delta_y() as f32);
                // Handle scroll in wgpui
            },
        }
    }
}
```

## Standalone Web

### Direct Initialization

```rust
#[wasm_bindgen(start)]
pub async fn main() {
    let platform = WebPlatform::init("my-canvas").await.unwrap();

    run_animation_loop(move || {
        let mut scene = Scene::new();
        // Build scene...
        platform.render(&scene).ok();
    });
}
```

### With Custom Events

```rust
use std::cell::RefCell;
use std::rc::Rc;

let platform = Rc::new(RefCell::new(platform));
let state = Rc::new(RefCell::new(AppState::new()));

// Set up event handlers
{
    let canvas = platform.borrow().canvas().clone();
    let state = state.clone();

    let closure = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e| {
        let mut state = state.borrow_mut();
        state.handle_click(e.client_x(), e.client_y());
    });

    canvas.add_event_listener_with_callback(
        "mousedown",
        closure.as_ref().unchecked_ref()
    ).unwrap();

    closure.forget();
}

// Render loop
run_animation_loop(move || {
    let platform = platform.borrow_mut();
    let state = state.borrow();

    let scene = build_scene(&state);
    platform.render(&scene).ok();
});
```

## JavaScript Interop

### Exposing to JS

```rust
#[wasm_bindgen]
pub struct WgpuiHandle {
    platform: WebPlatform,
}

#[wasm_bindgen]
impl WgpuiHandle {
    #[wasm_bindgen(constructor)]
    pub async fn new(canvas_id: &str) -> Result<WgpuiHandle, JsValue> {
        let platform = WebPlatform::init(canvas_id).await
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(Self { platform })
    }

    pub fn render(&mut self, data: JsValue) -> Result<(), JsValue> {
        let entries: Vec<Message> = serde_wasm_bindgen::from_value(data)?;
        let mut scene = build_scene(&entries);
        self.platform.render(&scene)
            .map_err(|e| JsValue::from_str(&e))
    }
}
```

### Calling from JS

```javascript
import init, { WgpuiHandle } from './wgpui.js';

await init();

const handle = await new WgpuiHandle('my-canvas');

function render(messages) {
    handle.render(messages);
}
```

## Performance Tips

### Minimize State Changes

Only rebuild scene when state changes:

```rust
let mut last_entries_hash = 0;

run_animation_loop(move || {
    let entries = get_entries();
    let hash = calculate_hash(&entries);

    if hash != last_entries_hash {
        let scene = build_scene(&entries);
        platform.render(&scene).ok();
        last_entries_hash = hash;
    }
});
```

### Virtual Scrolling

For large lists, only render visible items:

```rust
fn visible_range(scroll_offset: f32, viewport_height: f32, item_heights: &[f32]) -> Range<usize> {
    let mut y = 0.0;
    let mut start = 0;

    for (i, &h) in item_heights.iter().enumerate() {
        if y + h > scroll_offset && start == 0 {
            start = i;
        }
        if y > scroll_offset + viewport_height {
            return start..i;
        }
        y += h;
    }

    start..item_heights.len()
}
```

### Batch Text Layout

Reuse text measurements:

```rust
struct CachedMessage {
    content: String,
    size: Size,
    text_run: Option<TextRun>,
}

impl CachedMessage {
    fn ensure_layout(&mut self, text_system: &mut TextSystem, origin: Point, color: Hsla) {
        if self.text_run.is_none() {
            self.text_run = Some(text_system.layout(&self.content, origin, 13.0, color));
        }
    }
}
```

## Debugging

### Render Stats

```rust
let start = web_sys::window().unwrap().performance().unwrap().now();

platform.render(&scene).ok();

let elapsed = web_sys::window().unwrap().performance().unwrap().now() - start;
log::debug!("Frame time: {:.2}ms", elapsed);
```

### Atlas Visualization

```rust
// Render atlas texture for debugging
scene.draw_quad(
    Quad::new(Bounds::new(0.0, 0.0, 256.0, 256.0))
        .with_background(Hsla::white())
);
// Note: Requires exposing atlas texture as quad
```
