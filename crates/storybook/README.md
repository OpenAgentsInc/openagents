# storybook

Visual component explorer for OpenAgents UI. Browse, inspect, and test Maud components in isolation with hot-reload.

## Overview

Storybook is a standalone Actix server that renders `ui` crate components in isolation. It provides:

- **Live component previews** - See components render with different props
- **Hot-reload** - Auto-refresh on code changes (with `systemfd`)
- **Navigation sidebar** - Browse components organized by type
- **Code examples** - Copy-paste ready usage snippets
- **WebSocket reload** - Reconnects automatically on server restart
- **Dark theme** - Uses same semantic tokens as main UI

Perfect for:
- Developing new components
- Testing component variants
- Visual regression testing
- Component documentation
- Design review

## Quick Start

### Simple Mode

```bash
# Run storybook (opens browser automatically)
cargo storybook

# Or manually
cargo run -p storybook
# Then open http://localhost:3030
```

### Hot-Reload Mode (Recommended)

```bash
# Install cargo-watch and systemfd if not already installed
cargo install cargo-watch systemfd

# Run with hot-reload
systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'
```

Now edit any file in `crates/ui/src/` or `crates/storybook/src/stories/` and the browser will auto-refresh.

## Navigation

The sidebar organizes stories by category:

### Components

General UI components:
- **Button** - Variants, sizes, states

### Recorder

Session log visualization organized by Atomic Design:

**Overview:**
- **Index** - Recorder component overview
- **Atoms** - All 14 atomic components
- **Molecules** - Composed component groups
- **Organisms** - Complete line renderers
- **Sections** - Page-level layouts
- **Demo** - Full session viewer example

**Individual Atoms:**
- Status Dot
- Line Type Label
- Step Badge
- Timestamp Badge
- Call ID Badge
- Cost Badge
- Token Badge
- Latency Badge
- Attempt Badge
- TID Badge
- Blob Ref
- Redacted Value
- Result Arrow

## Architecture

### Tech Stack

- **Actix-web** - HTTP server framework
- **Actix-ws** - WebSocket support for hot-reload
- **Maud** - Type-safe HTML templates
- **Tokio** - Async runtime
- **systemfd** - File descriptor passing for hot-reload
- **listenfd** - Listen on systemfd sockets
- **open** - Auto-open browser on startup

### File Structure

```
crates/storybook/
├── src/
│   ├── main.rs              # Server, routes, hot-reload
│   └── stories/
│       ├── mod.rs
│       ├── button.rs        # Button story
│       └── recorder/
│           ├── mod.rs
│           ├── index.rs     # Recorder overview
│           ├── demo.rs      # Full demo
│           ├── molecules.rs
│           ├── organisms.rs
│           ├── sections.rs
│           ├── shared.rs    # Shared utilities
│           └── atoms/
│               ├── mod.rs
│               ├── index.rs  # Atoms overview
│               ├── shared.rs # Atom utilities
│               ├── status_dot.rs
│               ├── line_type_label.rs
│               ├── step_badge.rs
│               ├── timestamp_badge.rs
│               ├── call_id_badge.rs
│               ├── cost_badge.rs
│               ├── token_badge.rs
│               ├── latency_badge.rs
│               ├── attempt_badge.rs
│               ├── tid_badge.rs
│               ├── blob_ref.rs
│               ├── redacted_value.rs
│               └── result_arrow.rs
└── Cargo.toml
```

### Server Structure

The main server (`main.rs`) includes:

```rust
// Base layout with sidebar and hot-reload
fn base_layout(title: &str, active_story: &str, content: Markup) -> Markup

// Sidebar navigation with active state
fn sidebar_nav(active_story: &str) -> Markup

// WebSocket hot-reload endpoint
async fn ws_reload(req: HttpRequest, stream: web::Payload) -> impl Responder

// Story page handlers
async fn button_story_page() -> impl Responder
async fn recorder_index_page() -> impl Responder
async fn atoms_status_dot_page() -> impl Responder
// ... etc
```

Routes are registered in `main()`:

```rust
App::new()
    .route("/", web::get().to(index))
    .route("/stories/button", web::get().to(button_story_page))
    .route("/stories/recorder", web::get().to(recorder_index_page))
    .route("/stories/recorder/atoms", web::get().to(atoms_index_page))
    .route("/stories/recorder/atoms/status-dot", web::get().to(atoms_status_dot_page))
    // ... etc
    .route("/__ws_reload", web::get().to(ws_reload))
```

## Creating Stories

### Story Structure

A story is a function that returns `Markup`:

```rust
use maud::{Markup, html};
use ui::{Button, ButtonVariant, ButtonSize};

pub fn button_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Button"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "A button component with variants, sizes, and states."
        }

        // Sections showing different variants
        (section_title("Variants"))
        (section(row(html! {
            (item("Primary", Button::new("Primary").render()))
            (item("Secondary", Button::new("Secondary")
                .variant(ButtonVariant::Secondary)
                .render()))
            (item("Ghost", Button::new("Ghost")
                .variant(ButtonVariant::Ghost)
                .render()))
        })))

        // Code example
        (section_title("Usage"))
        (code_block(r#"Button::new("Submit").render()"#))
    }
}
```

### Helper Components

Stories use shared helpers for consistent layout:

```rust
// Section title
fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" {
            (title)
        }
    }
}

// Card wrapper
fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" {
            (content)
        }
    }
}

// Horizontal row
fn row(content: Markup) -> Markup {
    html! {
        div class="flex gap-4 items-center flex-wrap" {
            (content)
        }
    }
}

// Labeled item
fn item(label: &str, content: Markup) -> Markup {
    html! {
        div class="flex flex-col gap-2" {
            span class="text-xs text-muted-foreground" { (label) }
            (content)
        }
    }
}

// Code block with syntax
fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto" {
            code { (code) }
        }
    }
}
```

### Adding a New Story

1. **Create story file:**

```bash
touch crates/storybook/src/stories/my_component.rs
```

2. **Write story function:**

```rust
// crates/storybook/src/stories/my_component.rs
use maud::{Markup, html};
use ui::MyComponent;

pub fn my_component_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2" { "My Component" }
        p class="text-sm text-muted-foreground mb-6" {
            "Description of what this component does."
        }

        // Show variants
        (MyComponent::new("Example").render())
    }
}
```

3. **Register in stories/mod.rs:**

```rust
// crates/storybook/src/stories/mod.rs
pub mod button;
pub mod my_component;  // Add this
pub mod recorder;
```

4. **Add route handler in main.rs:**

```rust
// Import the story
use stories::my_component::my_component_story;

// Add handler function
async fn my_component_page() -> impl Responder {
    let content = my_component_story();
    let html = base_layout("My Component", "my-component", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}
```

5. **Add route in App:**

```rust
App::new()
    .route("/stories/my-component", web::get().to(my_component_page))
    // ... other routes
```

6. **Add sidebar link:**

```rust
fn sidebar_nav(active_story: &str) -> Markup {
    html! {
        // ... existing nav
        a href="/stories/my-component"
          class=(link_class("my-component")) {
            "My Component"
        }
    }
}
```

7. **Test:**

```bash
cargo run -p storybook
# Open http://localhost:3030/stories/my-component
```

## Hot-Reload Setup

### How It Works

1. **systemfd** - Creates socket, passes file descriptor to child
2. **cargo watch** - Watches for file changes, recompiles
3. **listenfd** - Server listens on inherited socket (no port conflict)
4. **WebSocket** - Client detects new server, refreshes page

### WebSocket Client Code

Injected into every page:

```javascript
(function() {
    var wasConnected = false;
    function connect() {
        var ws = new WebSocket('ws://' + location.host + '/__ws_reload');
        ws.onopen = function() {
            if (wasConnected) location.reload();
            wasConnected = true;
        };
        ws.onclose = function() {
            setTimeout(connect, 500);
        };
    }
    connect();
})();
```

**Flow:**
1. Page loads, WebSocket connects
2. File changes, `cargo watch` rebuilds
3. Server restarts, WebSocket closes
4. Client retries, reconnects to new server
5. Second connection detected → page reloads

### systemfd Installation

```bash
# Install if not already installed
cargo install systemfd cargo-watch

# Run with hot-reload
systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'
```

## Story Patterns

### Component Variants

Show all variants with labels:

```rust
(section_title("Variants"))
(section(row(html! {
    @for variant in &[Variant::A, Variant::B, Variant::C] {
        (item(&format!("{:?}", variant),
              Component::new("Example").variant(*variant).render()))
    }
})))
```

### Component States

Compare states side-by-side:

```rust
(section_title("States"))
(section(row(html! {
    (item("Default", Component::new("Normal").render()))
    (item("Hover", Component::new("Hover").class("hover").render()))
    (item("Disabled", Component::new("Disabled").disabled(true).render()))
    (item("Active", Component::new("Active").active(true).render()))
})))
```

### Code Examples

Embed copy-paste ready code:

```rust
(section_title("Usage"))
(code_block(r#"use ui::Component;

// Basic
Component::new("Example").render()

// With options
Component::new("Example")
    .variant(Variant::Primary)
    .size(Size::Large)
    .render()"#))
```

### Interactive Examples

Show component in context:

```rust
(section_title("In Context"))
(section(html! {
    div class="p-8 bg-background" {
        div class="max-w-lg mx-auto" {
            h1 class="text-2xl mb-4" { "Page Title" }
            p class="text-muted-foreground mb-4" {
                "Some description text."
            }
            (Component::new("Call to Action").render())
        }
    }
}))
```

## Recorder Stories

### Atoms Index

Shows all atomic components:

```rust
pub fn atoms_index_story() -> Markup {
    html! {
        h1 { "Atoms" }
        p { "Smallest UI primitives used in session log rendering." }

        (section_title("Status Indicators"))
        (section(row(html! {
            (status_dot(StatusState::Success))
            (status_dot(StatusState::Error))
            (status_dot(StatusState::Pending))
        })))

        // ... more sections
    }
}
```

### Individual Atom Stories

One story per atom with all variants:

```rust
pub fn status_dot_story() -> Markup {
    html! {
        h1 { "Status Dot" }
        p { "Color-coded status indicator (3 states)." }

        (section_title("States"))
        (section(row(html! {
            (item("Success", status_dot(StatusState::Success)))
            (item("Error", status_dot(StatusState::Error)))
            (item("Pending", status_dot(StatusState::Pending)))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::recorder::atoms::{status_dot, StatusState};

status_dot(StatusState::Success)"#))
    }
}
```

### Demo Story

Full session viewer combining all components:

```rust
pub fn recorder_demo_story() -> Markup {
    html! {
        h1 { "Demo" }
        p { "Full session viewer using all recorder components." }

        // Session header
        (session_header("sess_001", "claude-opus-4-5", "main", "abc123", Some("auto")))

        // Session lines
        div class="space-y-2 mt-8" {
            (user_line(1, Some(1), Some("00:00:00"), "Fix the auth bug"))
            (agent_line(2, Some(2), Some("00:00:05"), "I'll check the module.", Some(100), Some(50)))
            (tool_line(3, Some(3), Some("00:00:10"), "Read", "src/auth.rs", Some("[186 lines]"), Some("call_1"), Some(1234)))
            // ... more lines
        }

        // Session stats
        (session_stats(15, 3, 8, 10, 5000, 2000, 300))
    }
}
```

## Styling

Storybook uses the same Tailwind theme as the main UI:

- **Semantic tokens** - `bg-background`, `text-foreground`, etc.
- **Sharp corners** - No border-radius
- **Monospace fonts** - Berkeley Mono stack
- **Dark mode** - Default color scheme

### Story Card Styling

Standard card wrapper:

```rust
div class="p-4 border border-border bg-card mb-4" {
    // Content
}
```

### Code Block Styling

Monospace code display:

```rust
pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
    code { (code_string) }
}
```

### Grid Layouts

Responsive component grids:

```rust
div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3" {
    @for component in components {
        (component.render())
    }
}
```

## Development Workflow

### Typical Workflow

1. **Start hot-reload server:**
   ```bash
   systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'
   ```

2. **Open browser:**
   ```
   http://localhost:3030
   ```

3. **Edit component in `crates/ui/src/`:**
   ```rust
   // Make changes to Button, etc.
   ```

4. **Browser auto-refreshes** showing changes

5. **Edit story in `crates/storybook/src/stories/`:**
   ```rust
   // Add new variants, examples
   ```

6. **Browser auto-refreshes** with new story content

### Testing Components

1. Navigate to component story
2. Test different variants visually
3. Check responsive behavior (resize browser)
4. Verify styling with semantic tokens
5. Copy code examples to verify usage

### Component Development Cycle

```
Edit component → Hot-reload → Visual check → Iterate
     ↓                                         ↑
     └─────────── Repeat until perfect ────────┘
```

## Deployment

Storybook is a **development tool only**. It's not meant for production deployment.

For component documentation in production, consider:
- Generating static HTML from stories
- Screenshot automation for visual testing
- Embedding examples in main documentation site

## Troubleshooting

### Hot-reload not working

**Symptom:** Changes don't refresh browser

**Fix:**
```bash
# Make sure systemfd and cargo-watch are installed
cargo install systemfd cargo-watch

# Run with correct command
systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'
```

### Port already in use

**Symptom:** `Address already in use (os error 98)`

**Fix:**
```bash
# Kill process on port 3030
lsof -ti:3030 | xargs kill -9

# Or use different port
systemfd --no-pid -s http::3031 -- cargo watch -x 'run -p storybook'
```

### Component not rendering

**Symptom:** Blank page or missing component

**Fix:**
1. Check component is imported: `use ui::Component;`
2. Verify story function is public: `pub fn component_story()`
3. Check route is registered in `App::new()`
4. Look for compile errors in terminal

### Sidebar link not highlighting

**Symptom:** Active story not highlighted in sidebar

**Fix:**

Check `active_story` parameter matches route name:

```rust
// In handler
let html = base_layout("Title", "my-component", content);
//                                ^^^^^^^^^^^^
//                                Must match sidebar link name

// In sidebar_nav
class=(link_class("my-component"))
//                 ^^^^^^^^^^^^
//                 Must match active_story parameter
```

## Performance

Server performance metrics:

- **Initial compile:** 5-10 seconds
- **Hot-reload rebuild:** 1-3 seconds
- **Page render:** <10ms
- **WebSocket overhead:** <1ms per ping

The hot-reload setup provides near-instant feedback during development.

## Future Work

- [ ] Component search/filter in sidebar
- [ ] Props editor for interactive testing
- [ ] Screenshot capture for visual regression
- [ ] Export stories as static HTML
- [ ] Keyboard shortcuts (j/k navigation)
- [ ] Dark/light mode toggle
- [ ] Component size/performance metrics
- [ ] A11y testing integration
- [ ] Diff view for component changes

## Related Documentation

- **UI Components**: `crates/ui/README.md`
- **Recorder Format**: `crates/recorder/README.md`
- **Desktop Shell**: `crates/desktop/README.md`

## License

Same as the OpenAgents workspace (MIT).
