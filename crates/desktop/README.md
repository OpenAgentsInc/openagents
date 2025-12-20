# OpenAgents Desktop

Native desktop shell for OpenAgents, combining a native webview (wry/tao) with an embedded local web server (Actix) for building desktop applications with web technologies.

## Architecture

```
┌─────────────────────────────────────┐
│   Native Window (tao + wry)         │
│  ┌───────────────────────────────┐  │
│  │    WebView (Chromium-based)   │  │
│  │                                │  │
│  │   ┌─────────────────────────┐ │  │
│  │   │   HTML/CSS/JS UI        │ │  │
│  │   │   (Maud + Tailwind)     │ │  │
│  │   └─────────────────────────┘ │  │
│  │                                │  │
│  │   WebSocket Connection ──────────┼──┐
│  └───────────────────────────────┘  │  │
└─────────────────────────────────────┘  │
                                         │
┌────────────────────────────────────────┼───┐
│  Actix Web Server (127.0.0.1:random)  │   │
│                                            │
│  ┌──────────────┐  ┌──────────────────┐   │
│  │   Routes     │  │  WebSocket       │◄──┘
│  │              │  │  Broadcaster     │
│  │ /            │  │                  │
│  │ /autopilot   │  │  - Broadcast to  │
│  │ /increment   │  │    all clients   │
│  │ /events      │  │  - HTML fragment │
│  │ /ws          │  │    updates       │
│  └──────────────┘  └──────────────────┘
│                                            │
│  ┌──────────────────────────────────────┐ │
│  │   Views (Maud templates)             │ │
│  │   - home_page()                      │ │
│  │   - autopilot_page()                 │ │
│  │   - layout() with Tailwind CSS       │ │
│  └──────────────────────────────────────┘ │
└────────────────────────────────────────────┘
```

### Key Components

1. **Native Window** (`main.rs`):
   - Built with `tao` (cross-platform window library)
   - Embeds a `wry` webview (uses platform's native webview)
   - Runs on main thread (required for macOS)

2. **Web Server** (`server.rs`):
   - Actix-web server on `127.0.0.1:0` (random port)
   - Runs in background thread with Tokio runtime
   - Serves HTML pages and WebSocket connections

3. **Views** (`views/`):
   - Server-side rendered with Maud (Rust HTML templates)
   - Styled with Tailwind CSS (Play CDN for development)
   - Real-time updates via WebSocket

4. **WebSocket Broadcaster** (`ws.rs`):
   - Broadcasts HTML fragments to all connected clients
   - Supports out-of-band (OOB) swaps for DOM updates
   - Built on Tokio broadcast channels

## Quick Start

### Building

```bash
cd crates/desktop
cargo build --release
```

### Running

```bash
cargo run --release
```

The app will:
1. Start the Actix server on a random port
2. Print `DESKTOP_PORT=<port>` to stdout
3. Open a native window with the webview
4. Load `http://127.0.0.1:<port>/` in the webview

### Development

For faster iteration with hot reload:

```bash
# Run with debug symbols and logging
RUST_LOG=desktop=debug cargo run
```

## Adding a New Page/Route

### 1. Create a View Module

Create `src/views/mypage.rs`:

```rust
use maud::{Markup, html};
use super::layout;

pub fn mypage() -> Markup {
    layout(
        "My Page Title",
        html! {
            div class="text-center" {
                h1 { "My Page" }
                p { "Content goes here" }
            }
        },
    )
}
```

### 2. Export from Views Module

Add to `src/views/mod.rs`:

```rust
mod mypage;
pub use mypage::mypage;
```

### 3. Add Route to Server

Add to `src/server.rs`:

```rust
use crate::views::mypage;

// In start_server():
.route("/mypage", web::get().to(mypage_handler))

// Add handler:
async fn mypage_handler() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(mypage().into_string())
}
```

Now navigate to `http://127.0.0.1:<port>/mypage` in the webview.

## WebSocket Real-Time Updates

### Broadcasting HTML Fragments

The WebSocket broadcaster allows server-side code to push HTML updates to all connected clients.

#### Example: Counter Update

1. **Server broadcasts fragment** (`server.rs`):
```rust
async fn increment(state: web::Data<AppState>) -> HttpResponse {
    let new_val = state.counter.fetch_add(1, Ordering::SeqCst) + 1;

    // Create HTML fragment
    let fragment = counter_fragment(new_val);

    // Broadcast to all clients
    state.broadcaster.broadcast(&fragment);

    HttpResponse::Ok().finish()
}
```

2. **Fragment with matching ID** (`views/home.rs`):
```rust
pub fn counter_fragment(count: u64) -> String {
    html! {
        div id="counter" class="text-6xl font-bold" {
            (count)
        }
    }
    .into_string()
}
```

3. **Client auto-updates** (`layout.rs`):
The WebSocket client in `layout()` automatically swaps elements by ID:
```javascript
ws.onmessage = function(e) {
    var tmp = document.createElement('div');
    tmp.innerHTML = e.data;
    var el = tmp.firstElementChild;
    if (el && el.id) {
        var target = document.getElementById(el.id);
        if (target) target.outerHTML = el.outerHTML;
    }
};
```

### Broadcasting from External Processes

The `/events` endpoint allows external processes (like autopilot) to push updates:

```bash
# Send HTML fragment via POST
curl -X POST http://127.0.0.1:<port>/events \
  -H "Content-Type: text/plain" \
  -d '<div id="status">Updated!</div>'
```

## Styling with Tailwind CSS

The desktop app uses Tailwind CSS via the Play CDN (defined in the `ui` crate):

### Custom Theme

The theme is defined in `crates/ui/src/lib.rs`:

```rust
pub const TAILWIND_THEME: &str = r#"
@layer base {
  :root {
    --background: 0 0% 100%;
    --foreground: 222.2 84% 4.9%;
    --green: 142.1 76.2% 36.3%;
    /* ... */
  }
}
"#;
```

### Using Theme Colors

```rust
html! {
    div class="bg-background text-foreground" {
        span class="text-green" { "Success!" }
        span class="text-red" { "Error!" }
    }
}
```

### Custom Components

The `ui` crate provides reusable components:

```rust
use ui::Button;

html! {
    (Button::new("Click me").render())
}
```

## Project Structure

```
crates/desktop/
├── src/
│   ├── main.rs           # Window + webview setup
│   ├── server.rs         # Actix routes and handlers
│   ├── ws.rs             # WebSocket broadcaster
│   ├── replay.rs         # Autopilot replay viewer
│   └── views/
│       ├── mod.rs        # View exports
│       ├── layout.rs     # Base HTML layout
│       ├── home.rs       # Home page with counter
│       ├── autopilot.rs  # Autopilot live viewer
│       ├── projects.rs   # Project management UI
│       └── sessions.rs   # Session list UI
├── Cargo.toml
└── README.md
```

## State Management

App state is shared via `AppState`:

```rust
pub struct AppState {
    pub counter: AtomicU64,
    pub broadcaster: Arc<WsBroadcaster>,
}
```

Access in handlers:

```rust
async fn my_handler(state: web::Data<AppState>) -> HttpResponse {
    let count = state.counter.load(Ordering::SeqCst);
    // ...
}
```

## Autopilot Integration

The desktop app includes an Autopilot viewer at `/autopilot`:

- **Live trajectory viewer**: Shows real-time updates from autopilot runs
- **Replay mode**: View historical trajectories from rlog files
- **WebSocket updates**: Receives HTML fragments from autopilot via `/events`

See `src/views/autopilot.rs` and `src/replay.rs` for implementation details.

## Building for Production

### Release Build

```bash
cargo build --release --bin openagents-desktop
```

Binary location: `target/release/openagents-desktop`

### Platform-Specific Notes

#### macOS

- Window/webview must run on main thread (enforced by tao)
- May need to sign the app for distribution
- Uses system WebKit for the webview

#### Linux

- Uses WebKitGTK for the webview
- Install dependencies: `sudo apt install libwebkit2gtk-4.0-dev`

#### Windows

- Uses WebView2 (Edge) for the webview
- Requires WebView2 runtime installed

## Debugging

### Enable Logging

```bash
RUST_LOG=desktop=debug,actix_web=debug cargo run
```

### WebView DevTools

To enable DevTools in the webview, modify `main.rs`:

```rust
let _webview = WebViewBuilder::new()
    .with_url(format!("http://127.0.0.1:{}/", port))
    .with_devtools(true)  // Add this line
    .build(&window)
    .expect("webview");
```

Then right-click in the webview and select "Inspect" (platform-dependent).

### Server Logs

The Actix server logs all requests when `RUST_LOG` includes `actix_web`:

```
[DEBUG actix_web::middleware::logger] 127.0.0.1 "GET /autopilot HTTP/1.1" 200
```

## Common Patterns

### POST with HTMX

```rust
// Handler
async fn submit_form(form: web::Form<MyForm>) -> HttpResponse {
    // Process form
    let result = process(&form);

    // Return HTML fragment
    HttpResponse::Ok()
        .content_type("text/html")
        .body(result_fragment(result))
}

// View
html! {
    form hx-post="/submit" hx-target="#result" {
        input type="text" name="field";
        button type="submit" { "Submit" }
    }
    div id="result" {}
}
```

### Async Background Tasks

```rust
async fn start_task(state: web::Data<AppState>) -> HttpResponse {
    let broadcaster = state.broadcaster.clone();

    tokio::spawn(async move {
        // Long-running task
        for i in 0..10 {
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Broadcast progress
            let fragment = format!("<div id='progress'>{}%</div>", i * 10);
            broadcaster.broadcast(&fragment);
        }
    });

    HttpResponse::Ok().finish()
}
```

## Testing

Currently no automated tests exist for the desktop crate. Potential test strategies:

1. **Unit tests**: Test view rendering logic
2. **Integration tests**: Test server routes with `actix_web::test`
3. **E2E tests**: Test full window + webview with headless browser

Example integration test structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_home_route() {
        let app = test::init_service(
            App::new().route("/", web::get().to(index))
        ).await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
    }
}
```

## Dependencies

- **wry** (0.50): Cross-platform webview library
- **tao** (0.33): Cross-platform window library
- **actix-web** (4): Web framework for Rust
- **actix-ws** (0.3): WebSocket support for Actix
- **maud** (0.26): Compile-time HTML templates
- **tokio** (1): Async runtime
- **ui**: Custom UI component library (sibling crate)

## Related Crates

- **ui**: Tailwind theme and reusable components
- **recorder**: Trajectory parsing and replay
- **autopilot**: Autonomous agent runner

## Troubleshooting

### Port Already in Use

The server binds to a random port (`127.0.0.1:0`), so port conflicts are rare. Check for other instances:

```bash
ps aux | grep openagents-desktop
```

### WebView Not Loading

Check server is running:
```bash
curl http://127.0.0.1:<port>/
```

Check logs:
```bash
RUST_LOG=desktop=trace cargo run
```

### WebSocket Disconnects

The WebSocket client automatically reconnects by reloading the page after 1 second:

```javascript
ws.onclose = function() {
    setTimeout(function() { location.reload(); }, 1000);
};
```

For production, implement proper reconnection logic with exponential backoff.

## Future Enhancements

- [ ] Add automated tests (unit, integration, E2E)
- [ ] Implement proper WebSocket reconnection with backoff
- [ ] Add session persistence across app restarts
- [ ] Bundle Tailwind CSS for offline use
- [ ] Add keyboard shortcuts and native menus
- [ ] Implement window state persistence (size, position)
- [ ] Add system tray integration
- [ ] Support multiple windows

## License

See workspace root for license information.
