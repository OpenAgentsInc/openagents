# Autopilot GUI

Visual interface for OpenAgents Autopilot powered by the Claude Agent SDK.

## Overview

Autopilot GUI provides a native desktop application for monitoring and managing autonomous agent sessions. Instead of working purely from the command line, you get real-time visibility into what autopilot is doing, how much it's costing, and what context it has access to.

**Why use the GUI instead of CLI?**
- **Real-time monitoring** - See sessions as they run with live updates
- **Visual metrics** - Charts and stats for token usage, costs, success rates
- **Context inspection** - Understand what the agent can see
- **Permission management** - Visual interface for tool permissions
- **Session history** - Browse past runs with full details

## Features

### 📊 Real-Time Dashboard
- Live session list showing active and recent autopilot runs
- Quick stats: sessions today, success rate, total tokens/cost, avg duration
- WebSocket connection with pulsing "LIVE" indicator
- Auto-refresh when sessions start or complete
- Color-coded status indicators (✓ Complete, ✗ Failed, ⚠ Crashed)

### 🔍 Context Inspector
- View what the agent can see
- Git status with branch, modified files, recent commits
- Token usage gauge with breakdown by source
- Directory tree structure
- CLAUDE.md content display
- Current working directory

### 💬 Chat Interface
- Interactive chat with autopilot
- Real-time message streaming
- Tool call visualization
- Permission request handling

### 🔐 Permission Management
- Visual permission rules manager
- Create, view, and delete permission rules
- Control what tools autopilot can use

### 🔌 WebSocket Live Updates
- Real-time session events
- Auto-reconnect on connection loss
- Minimal latency for status updates

## Installation

### Build from Source

```bash
# From the workspace root
cargo build --release -p autopilot-gui

# The binary will be at:
# target/release/autopilot-gui
```

### Run

```bash
./target/release/autopilot-gui
```

The GUI will launch on `http://localhost:3847` in a native window.

## Usage

### Launch the Application

```bash
cargo run -p autopilot-gui
```

Or if built:

```bash
./target/release/autopilot-gui
```

### Navigate Views

The navigation bar at the top provides quick access to all views:
- **Dashboard** - Session monitoring and statistics
- **Chat** - Interactive agent interface
- **Context** - Inspect agent context and environment
- **Permissions** - Manage tool permissions

Click any link to navigate between views. The current page is highlighted with a blue underline.

### Monitor Sessions

The dashboard shows:

**Quick Stats** (5 cards at top):
- Sessions Today - count of sessions started today
- Success Rate (30d) - percentage of successfully completed sessions
- Total Tokens (30d) - cumulative token usage
- Total Cost (30d) - total spend in USD
- Avg Duration (30d) - average session length

**Recent Sessions Table**:
- Time - when the session started
- Model - which Claude model was used (Sonnet 4.5, Opus 4.5, etc.)
- Duration - how long the session ran
- Tokens - total input + output tokens
- Issues - number of issues completed
- Status - session outcome with color coding

The "● LIVE" indicator shows WebSocket connection status:
- Green "● LIVE" with pulsing animation - connected and receiving updates
- Red "DISCONNECTED" - connection lost, auto-reconnecting

### Inspect Context

The Context view shows what autopilot can see:

**Git Status Panel**:
- Current branch
- Modified, added, deleted files
- Recent commits with hash, message, author, timestamp

**Token Usage Panel**:
- Current usage vs max (200k tokens)
- Percentage gauge with color zones
- Breakdown by source (System Prompt, CLAUDE.md, Conversation History, Tool Results)

**CLAUDE.md Display**:
- Full content of project instructions
- Scrollable view

**Directory Tree**:
- Workspace structure
- Key folders (crates, docs, tests, .github)

### Manage Permissions

The Permissions view shows tool permission rules:
- View all configured rules
- Delete rules you no longer need
- See when rules were created

(Creating new rules is handled through the Chat interface when autopilot requests permission)

## Architecture

### Technology Stack

```
┌─────────────────────────────────────────────────────────┐
│                 Autopilot GUI                            │
├─────────────────────────────────────────────────────────┤
│  wry/tao native window (cross-platform)                 │
│       │                                                  │
│       ▼                                                  │
│  Actix-web server (localhost:3847)                      │
│       │                                                  │
│       ├── /                  → Dashboard (Maud + HTMX)  │
│       ├── /chat              → Chat interface           │
│       ├── /context           → Context inspector        │
│       ├── /permissions       → Permission manager       │
│       └── /ws                → WebSocket live updates   │
└─────────────────────────────────────────────────────────┘
```

**Key Components**:
- **wry** - Cross-platform WebView2 wrapper
- **tao** - Cross-platform window creation
- **Actix-web** - Fast async web framework
- **Maud** - Type-safe HTML templates
- **actix-ws** - WebSocket support
- **rusqlite** - Metrics database access

### Project Structure

```
crates/autopilot-gui/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library exports
│   │
│   ├── agent/               # Agent execution (future)
│   │   └── mod.rs
│   │
│   ├── server/              # Actix web server
│   │   ├── mod.rs
│   │   ├── routes.rs        # HTTP routes
│   │   ├── state.rs         # App state
│   │   └── ws.rs            # WebSocket handler
│   │
│   ├── sessions.rs          # Session data from metrics.db
│   │
│   ├── storage/             # Permission storage
│   │   ├── mod.rs
│   │   └── permissions.rs
│   │
│   ├── views/               # Maud templates
│   │   ├── mod.rs
│   │   ├── layout.rs        # Base layout + dashboard
│   │   ├── chat.rs          # Chat interface
│   │   ├── context.rs       # Context inspector
│   │   ├── permissions.rs   # Permission manager
│   │   └── components.rs    # Reusable components
│   │
│   └── window.rs            # Native window (wry/tao)
```

## Development

### Adding a New View

1. **Create the template** in `src/views/your_view.rs`:

```rust
use maud::{html, Markup};

pub fn your_view() -> Markup {
    html! {
        div class="container" {
            div class="card" {
                h2 { "Your View Title" }
                p { "Content goes here" }
            }
        }
    }
}
```

2. **Add the route** in `src/server/routes.rs`:

```rust
#[get("/your-view")]
async fn your_view() -> impl Responder {
    let html = layout::page_with_current(
        "Your View - Autopilot GUI",
        your_view::your_view(),
        Some("your-view")
    );
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
```

3. **Add navigation link** in `src/views/layout.rs`:

```rust
a href="/your-view" class={ @if current_page == Some("your-view") { "active" } } {
    "Your View"
}
```

### WebSocket Message Types

#### Client → Server

```json
{
  "type": "prompt",
  "text": "Your message here"
}
```

```json
{
  "type": "abort"
}
```

#### Server → Client

```json
{
  "type": "session_started",
  "session_id": "uuid",
  "timestamp": "ISO8601",
  "model": "claude-sonnet-4-5-20250929",
  "prompt": "Task description"
}
```

```json
{
  "type": "session_completed",
  "session_id": "uuid",
  "duration_seconds": 123.45,
  "final_status": "completed",
  "issues_completed": 2,
  "cost_usd": 0.15
}
```

```json
{
  "type": "stats_updated",
  "sessions_today": 5,
  "success_rate": 92.5,
  "total_tokens": 45000,
  "total_cost": 0.67,
  "avg_duration": 180.5
}
```

### Styling Conventions

**IMPORTANT: No border radius allowed in this codebase!**

The pre-push hook will reject commits with:
- `border-radius` CSS property
- `.rounded*()` method calls
- Any rounded corner styling

Use sharp corners for all UI elements:

```css
/* ✅ GOOD */
.card {
    border: 1px solid #3a3a3a;
    padding: 1.5rem;
}

/* ❌ BAD - Will be rejected */
.card {
    border: 1px solid #3a3a3a;
    border-radius: 8px;
    padding: 1.5rem;
}
```

**Color Palette**:
- Background: `#1a1a1a` (dark)
- Cards: `#2a2a2a`
- Borders: `#3a3a3a`
- Text: `#e0e0e0` (light gray)
- Accent: `#4a9eff` (blue)
- Success: `#7dff7d` (green)
- Error: `#ff7d7d` (red)
- Warning: `#ffd97d` (yellow)

**Typography**:
- Font: System fonts (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto`)
- Line height: `1.6`
- Monospace: For session IDs, timestamps, code

### Running Tests

```bash
# All tests
cargo test -p autopilot-gui

# Specific test
cargo test -p autopilot-gui test_sessions
```

### Building Docs

```bash
cargo doc -p autopilot-gui --no-deps --open
```

## Database

The GUI reads from `autopilot-metrics.db` to display session data. This database is created and maintained by autopilot runs.

**Schema** (sessions table):
```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt TEXT NOT NULL,
    duration_seconds REAL NOT NULL,
    tokens_in INTEGER NOT NULL,
    tokens_out INTEGER NOT NULL,
    tokens_cached INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    issues_claimed INTEGER NOT NULL,
    issues_completed INTEGER NOT NULL,
    tool_calls INTEGER NOT NULL,
    tool_errors INTEGER NOT NULL,
    final_status TEXT NOT NULL
);
```

If the database doesn't exist, the dashboard will show a "Loading..." message.

## Troubleshooting

### GUI won't start

Check if port 3847 is already in use:
```bash
lsof -i :3847
```

### Dashboard shows "Loading..."

The metrics database is missing or empty. Run autopilot at least once:
```bash
cargo autopilot run "Test task"
```

### WebSocket won't connect

Check browser console for errors. The GUI expects WebSocket at `ws://localhost:3847/ws`.

### Styling looks wrong

Make sure you're using the built-in styles. Don't add external CSS frameworks.

## Related Documentation

- [Autopilot README](../autopilot/README.md) - Main autopilot documentation
- [Claude Agent SDK](../claude-agent-sdk/README.md) - SDK documentation
- [Directive d-009](../../.openagents/directives/d-009.md) - GUI project directive

## License

MIT
