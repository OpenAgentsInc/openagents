# ui

Maud/HTMX/Tailwind server-rendered UI components for OpenAgents. Sharp corners, semantic colors, monospace everything.

## Overview

The `ui` crate provides a component library for building server-rendered HTML interfaces using:

- **Maud** - Type-safe HTML templating with Rust
- **Tailwind CSS** - Utility-first styling via Play CDN
- **HTMX** - (Coming soon) Hypermedia-driven interactivity
- **Semantic Color Tokens** - Dark mode theme with CSS custom properties
- **Atomic Design** - Components organized as atoms → molecules → organisms → sections

Design philosophy:
- **Server-rendered**: No client-side JavaScript frameworks
- **Tailwind-first**: All styling via utility classes
- **Sharp corners**: Zero border-radius anywhere (enforced by git hooks)
- **Semantic colors**: Use `bg-background`, never `bg-zinc-900`
- **Monospace fonts**: Vera Mono / system monospace stack

## Quick Start

### Basic Usage

```rust
use ui::{Button, ButtonVariant, base_document};
use maud::html;

// Full page with Tailwind
let page = base_document("My App", html! {
    div class="p-8" {
        h1 class="text-2xl font-bold mb-4" { "Hello World" }
        (Button::new("Click me")
            .variant(ButtonVariant::Primary)
            .render())
    }
});
```

### Custom Layout

```rust
use ui::{TAILWIND_CDN, TAILWIND_THEME};
use maud::{DOCTYPE, html, PreEscaped};

html! {
    (DOCTYPE)
    html lang="en" {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { "Custom Layout" }
            script { (PreEscaped(TAILWIND_CDN)) }
            style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
        }
        body class="bg-background text-foreground font-mono" {
            // Your content
        }
    }
}
```

## Components

### Button

Interactive button with variants and sizes.

```rust
use ui::{Button, ButtonVariant, ButtonSize};

// Basic
Button::new("Submit").render()

// With variant
Button::new("Save")
    .variant(ButtonVariant::Primary)
    .render()

// Full options
Button::new("Delete")
    .variant(ButtonVariant::Secondary)
    .size(ButtonSize::Large)
    .disabled(false)
    .render()
```

**Variants:**
- `Primary` - Main action (`bg-primary text-primary-foreground`)
- `Secondary` - Secondary action (`bg-secondary text-secondary-foreground`)
- `Ghost` - Subtle/text button (`bg-transparent text-muted-foreground`)

**Sizes:**
- `Small` - `px-2 py-1 text-xs`
- `Default` - `px-4 py-2 text-sm`
- `Large` - `px-6 py-3 text-base`

### Base Document

Full HTML page with Tailwind setup.

```rust
use ui::base_document;
use maud::html;

let page = base_document("Page Title", html! {
    main class="container mx-auto p-8" {
        h1 class="text-3xl font-bold" { "Welcome" }
        p class="text-muted-foreground mt-4" {
            "Server-rendered with Tailwind"
        }
    }
});
```

Includes:
- Tailwind Play CDN (vendored inline)
- Custom dark theme with semantic tokens
- Monospace font stack
- Viewport meta tags

## Recorder Components

Session log visualization components using Atomic Design:

### Atoms

Smallest UI building blocks:

```rust
use ui::recorder::atoms::*;

// Status indicator
status_dot(StatusState::Success)

// Line type label
line_type_label(LineType::Tool)

// Metadata badges
step_badge(42)
timestamp_badge_wall("2025-12-19T10:30:00Z")
timestamp_badge_elapsed("00:15:30")
call_id_badge("call_1", CallType::Tool)
token_badge(1500, "in")
latency_badge(1234)
attempt_badge("2/3")
tid_badge("thread_42")

// Special values
blob_ref("sha256=abc123...", 1024)
redacted_value("password")
result_arrow("[ok]")
```

**Available Atoms (14):**
- `status_dot` - Color-coded status indicator (3 states)
- `line_type_label` - Line type with color accent (14 types)
- `step_badge` - Sequential step number
- `timestamp_badge_wall` - ISO timestamp display
- `timestamp_badge_elapsed` - Elapsed time (HH:MM:SS)
- `call_id_badge` - Tool/MCP/subagent call ID
- `token_badge` - Token count (in/out/cached)
- `latency_badge` - Operation latency in ms
- `attempt_badge` - Retry attempt (N/M format)
- `tid_badge` - Thread/trace ID
- `blob_ref` - Binary blob reference
- `redacted_value` - Redacted sensitive data
- `result_arrow` - Unicode → with result
- `cost_badge` - Cost in credits/sats

### Molecules

Composed atom groups:

```rust
use ui::recorder::molecules::*;

// Line header with metadata
line_header(
    LineType::Tool,
    Some(5),                    // step
    Some("00:15:30"),           // elapsed
    Some("2025-12-19T10:30:00Z") // timestamp
)

// Metadata row
line_meta(
    Some("call_1"),  // call_id
    Some(1234),      // latency_ms
    Some("2/3"),     // attempt
    Some("thread_1") // tid
)

// Result display with arrow
result_display("[ok] 186 lines", Some(CallType::Tool))

// Cost tracking
cost_accumulator(total_tokens, total_cost_sats)

// Budget meter
budget_meter(used_sats, budget_sats, percentage)

// Mode indicator
mode_indicator("auto")

// Phase indicator
phase_indicator("implement")

// Metrics footer
metrics_footer(
    total_calls,
    total_tokens_in,
    total_tokens_out,
    total_cost_sats,
    duration_secs
)
```

**Available Molecules (8):**
- `line_header` - Type + step + timestamps
- `line_meta` - Call ID + latency + attempt + TID
- `result_display` - Arrow + result text
- `cost_accumulator` - Running total cost
- `budget_meter` - Budget usage bar
- `mode_indicator` - Operating mode badge
- `phase_indicator` - Planning phase badge
- `metrics_footer` - Session summary stats

### Organisms

Complete line renderers:

```rust
use ui::recorder::organisms::*;

// User message
user_line(
    line_number,
    step,
    timestamp,
    "Can you check the auth module?"
)

// Agent response
agent_line(
    line_number,
    step,
    timestamp,
    "I'll investigate the session validation logic.",
    Some(tokens_in),
    Some(tokens_out)
)

// Tool call
tool_line(
    line_number,
    step,
    timestamp,
    "Read",
    "file_path=src/auth.rs",
    Some("[186 lines]"),
    Some(call_id),
    Some(latency_ms)
)

// Lifecycle event
lifecycle_line(
    line_number,
    timestamp,
    "start",
    "id=sess_001 duration=1h"
)

// Phase marker
phase_line(line_number, timestamp, "explore")

// Question
question_line(
    line_number,
    timestamp,
    "Which database?",
    Some("[selected: PostgreSQL]")
)

// MCP call
mcp_line(
    line_number,
    timestamp,
    "github",
    "issues",
    "state=open",
    Some("[8 issues]"),
    Some(call_id)
)

// Subagent spawn
subagent_line(
    line_number,
    timestamp,
    "explore",
    "Find auth failures",
    Some("Found 3 issues"),
    Some(call_id)
)

// Recall/memory
recall_line(
    line_number,
    timestamp,
    "authentication bugs",
    Some("[2 matches]")
)

// Time markers
time_marker("2025-12-19 10:30:00")
hour_divider("10:00")
```

**Available Organisms (11):**
- `user_line` - User message with metadata
- `agent_line` - Agent response with token usage
- `tool_line` - Tool call with result and latency
- `lifecycle_line` - Session lifecycle events
- `phase_line` - Planning phase markers
- `question_line` - User question with answer
- `mcp_line` - MCP protocol call
- `subagent_line` - Subagent spawn with summary
- `recall_line` - Memory retrieval
- `time_marker` - Absolute time display
- `hour_divider` - Hour boundary separator

### Sections

Page-level layouts:

```rust
use ui::recorder::sections::*;

// Session header with metadata
session_header(
    "sess_001",
    "claude-opus-4-5",
    "main",
    "abc123",
    Some("auto")
)

// Session statistics panel
session_stats(
    total_lines,
    user_messages,
    agent_messages,
    tool_calls,
    total_tokens_in,
    total_tokens_out,
    duration_secs
)

// Tool call index (sidebar)
tool_index(vec![
    ("call_1", "Read", Some("[ok]")),
    ("call_2", "Edit", Some("[ok]")),
    ("call_3", "Bash", Some("[exit 0]")),
])

// Session navigation sidebar
session_sidebar(
    session_id,
    phases,
    tools_summary,
    cost_summary
)
```

**Available Sections (4):**
- `session_header` - Session metadata banner
- `session_stats` - Statistics dashboard
- `tool_index` - Tool call navigation
- `session_sidebar` - Full session navigation

## Color System

### Semantic Tokens

The theme defines CSS custom properties that map to Tailwind utilities:

| Token | CSS Variable | Use Case |
|-------|--------------|----------|
| `bg-background` | `--color-background` | Page background |
| `text-foreground` | `--color-foreground` | Primary text |
| `bg-card` | `--color-card` | Card backgrounds |
| `border-border` | `--color-border` | Borders |
| `bg-primary` | `--color-primary` | Primary actions |
| `text-primary-foreground` | `--color-primary-foreground` | Primary text |
| `bg-secondary` | `--color-secondary` | Secondary actions |
| `bg-muted` | `--color-muted` | Muted backgrounds |
| `text-muted-foreground` | `--color-muted-foreground` | Muted text |
| `bg-accent` | `--color-accent` | Hover states |
| `bg-destructive` | `--color-destructive` | Danger/delete |

### Platform Accents

Fixed accent colors for specific meanings:

| Color | Hex | Use Case |
|-------|-----|----------|
| `text-green` | `#00A645` | Success, ok, pass |
| `text-red` | `#FF0000` | Error, fail, danger |
| `text-orange` | `#FF6600` | Warning, pending |
| `text-cyan` | `#00FFFF` | Info, links |
| `text-blue` | `#0000FF` | Primary accent |
| `text-magenta` | `#FF00FF` | Special states |
| `text-yellow` | `#FFBF00` | Highlights |

### Color Rules

**DO:**
```html
<div class="bg-background text-foreground">
<div class="bg-card border-border">
<span class="text-muted-foreground">
<button class="bg-primary text-primary-foreground">
<span class="text-green">Success</span>
```

**DON'T:**
```html
<!-- WRONG - rejected by pre-push hook -->
<div class="bg-zinc-900 text-zinc-100">
<div class="bg-gray-800 border-gray-700">
<span class="text-gray-500">
<span class="text-emerald-500">Success</span>
```

## Design Conventions

### Sharp Corners

Zero border-radius throughout the UI. Enforced by `.githooks/pre-push`:

**DO:**
```html
<div class="border border-border">
<button class="border border-primary">
<input class="border border-input">
```

**DON'T:**
```html
<!-- WRONG - rejected by pre-push hook -->
<div class="rounded-lg border">
<button class="rounded-md">
<input class="rounded">
```

### Typography

Monospace font stack: `'Vera Mono', ui-monospace, monospace`

```html
<body class="font-mono">
```

**Type Scale:**

| Class | Use Case |
|-------|----------|
| `text-xs` | Labels, metadata badges |
| `text-sm` | Body text, buttons |
| `text-base` | Large buttons |
| `text-lg` | Subheadings |
| `text-xl` | Section titles |
| `text-2xl` | Page titles |
| `text-3xl+` | Hero text |

### Spacing

Consistent spacing scale:

| Class | Usage |
|-------|-------|
| `gap-2` | Tight grouping (atoms) |
| `gap-4` | Standard spacing (molecules) |
| `gap-6` | Section separation |
| `p-2` | Compact padding |
| `p-4` | Standard padding |
| `p-8` | Page padding |
| `mb-2` | Tight margins |
| `mb-4` | Standard margins |
| `mb-8` | Section margins |

### Component Patterns

**Builder Pattern:**

```rust
pub struct Component {
    content: String,
    variant: Variant,
}

impl Component {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            variant: Variant::default(),
        }
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn render(self) -> Markup {
        // Render logic
    }
}
```

**Class Composition:**

```rust
let base = "inline-flex items-center";
let size = "px-4 py-2 text-sm";
let variant = "bg-primary text-primary-foreground";
let disabled = if self.disabled { "opacity-50" } else { "" };
let class = format!("{base} {size} {variant} {disabled}");

html! {
    button class=(class) { (self.label) }
}
```

## Creating Components

Follow the Atomic Design pattern:

### Atom Example

```rust
use maud::{Markup, html};

pub fn status_dot(state: StatusState) -> Markup {
    let color = match state {
        StatusState::Success => "bg-green",
        StatusState::Error => "bg-red",
        StatusState::Pending => "bg-orange",
    };

    html! {
        span class=(format!("inline-block w-2 h-2 {color}")) {}
    }
}
```

### Molecule Example

```rust
use maud::{Markup, html};
use crate::recorder::atoms::{step_badge, timestamp_badge_wall};

pub fn line_header(step: Option<u32>, timestamp: Option<&str>) -> Markup {
    html! {
        div class="flex items-center gap-2" {
            @if let Some(s) = step {
                (step_badge(s))
            }
            @if let Some(ts) = timestamp {
                (timestamp_badge_wall(ts))
            }
        }
    }
}
```

### Organism Example

```rust
use maud::{Markup, html};
use crate::recorder::molecules::{line_header, line_meta};

pub fn tool_line(
    line_number: usize,
    step: Option<u32>,
    timestamp: Option<&str>,
    tool_name: &str,
    args: &str,
    result: Option<&str>,
) -> Markup {
    html! {
        div class="flex flex-col gap-1 p-2 border-l-2 border-cyan" {
            (line_header(step, timestamp))
            div class="flex items-center gap-2" {
                span class="text-cyan" { (tool_name) }
                span class="text-muted-foreground" { (args) }
                @if let Some(r) = result {
                    span class="text-foreground" { "→ " (r) }
                }
            }
        }
    }
}
```

## Storybook

View all components in isolation:

```bash
# Start Storybook server
cargo storybook

# Open browser
open http://localhost:3030
```

Navigate to `/stories/recorder/*` to see recorder component examples.

## Git Hooks

### Pre-push Hook

Located at `.githooks/pre-push`, enforces:

1. **No border-radius** - Rejects `rounded`, `rounded-*`, `border-radius`
2. **No raw colors** - Rejects `bg-zinc-*`, `text-gray-*`, etc.

**Setup:**

```bash
git config core.hooksPath .githooks
```

**Bypass (emergencies only):**

```bash
git push --no-verify
```

## File Organization

```
crates/ui/
├── src/
│   ├── lib.rs              # Exports, theme constants
│   ├── button.rs           # Button component
│   ├── colors.rs           # Color token reference
│   ├── layout.rs           # base_document
│   ├── static/
│   │   └── tailwind.js     # Vendored Tailwind CDN
│   └── recorder/
│       ├── mod.rs
│       ├── atoms/
│       │   ├── mod.rs
│       │   ├── status_dot.rs
│       │   ├── line_type_label.rs
│       │   ├── step_badge.rs
│       │   ├── timestamp_badge.rs
│       │   ├── call_id_badge.rs
│       │   ├── token_badge.rs
│       │   ├── latency_badge.rs
│       │   ├── attempt_badge.rs
│       │   ├── tid_badge.rs
│       │   ├── blob_ref.rs
│       │   ├── redacted_value.rs
│       │   ├── result_arrow.rs
│       │   └── cost_badge.rs
│       ├── molecules/
│       │   ├── mod.rs
│       │   ├── line_header.rs
│       │   ├── line_meta.rs
│       │   ├── result_display.rs
│       │   ├── cost_accumulator.rs
│       │   ├── budget_meter.rs
│       │   ├── mode_indicator.rs
│       │   ├── phase_indicator.rs
│       │   └── metrics_footer.rs
│       ├── organisms/
│       │   ├── mod.rs
│       │   ├── user_line.rs
│       │   ├── agent_line.rs
│       │   ├── tool_line.rs
│       │   ├── lifecycle_line.rs
│       │   ├── phase_line.rs
│       │   ├── question_line.rs
│       │   ├── mcp_line.rs
│       │   ├── subagent_line.rs
│       │   ├── recall_line.rs
│       │   ├── time_marker.rs
│       │   ├── hour_divider.rs
│       │   └── styles.rs
│       └── sections/
│           ├── mod.rs
│           ├── session_header.rs
│           ├── session_stats.rs
│           ├── tool_index.rs
│           └── session_sidebar.rs
└── docs/
    ├── README.md           # This file (in docs/)
    ├── colors.md           # Color system documentation
    ├── components.md       # Component usage guide
    └── conventions.md      # Design rules and patterns
```

## Tailwind Play CDN

The crate vendors the Tailwind Play CDN inline for offline use:

```rust
pub const TAILWIND_CDN: &str = include_str!("static/tailwind.js");
```

This enables:
- No network requests in production
- Offline development
- Consistent Tailwind version
- Custom `@theme` directive support

## Testing

Components render to HTML strings via Maud:

```rust
#[test]
fn test_button_render() {
    let html = Button::new("Click")
        .variant(ButtonVariant::Primary)
        .render()
        .into_string();

    assert!(html.contains("bg-primary"));
    assert!(html.contains("Click"));
}
```

## Performance

Server-rendered components are:
- **Zero JavaScript** - No hydration overhead
- **No VDOM** - Direct HTML generation
- **Streaming** - Can stream partial responses
- **Cacheable** - Static HTML can be cached at CDN

Typical render times:
- Simple atom: <1μs
- Complex organism: 10-50μs
- Full page: 100-500μs

## Related Documentation

- **Colors**: `crates/ui/docs/colors.md`
- **Components**: `crates/ui/docs/components.md`
- **Conventions**: `crates/ui/docs/conventions.md`
- **Storybook**: `crates/storybook/src/stories/`

## Future Work

- [ ] HTMX integration for hypermedia interactions
- [ ] Form components with validation
- [ ] Table component with sorting/filtering
- [ ] Modal/dialog component
- [ ] Notification/toast system
- [ ] Input components (text, select, checkbox)
- [ ] Dark/light mode toggle (currently dark only)
- [ ] Accessibility (ARIA) improvements
- [ ] Component snapshot testing

## License

Same as the OpenAgents workspace (MIT).
