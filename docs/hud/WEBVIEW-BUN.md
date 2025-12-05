# webview-bun

Bun bindings for the [webview](https://github.com/webview/webview/) library - a tiny cross-platform library for creating web-based desktop GUIs.

**Source:** https://github.com/tr1ckydev/webview-bun

## How It Works

webview-bun uses Bun's FFI (Foreign Function Interface) to call native webview functions. The native library uses:
- **macOS**: WebKit (WKWebView)
- **Windows**: Microsoft Edge WebView2
- **Linux**: WebKitGTK

The FFI bindings are in `src/ffi.ts` which loads the platform-specific shared library:
- `libwebview.dylib` (macOS)
- `libwebview.dll` (Windows)
- `libwebview-{arch}.so` (Linux)

## Key Limitation: about:blank Origin Blocks WebSocket

**CRITICAL**: When using `setHTML()`, WebKit loads the content with an `about:blank` origin.
This "opaque origin" blocks WebSocket connections to `localhost`.

### What Works
```typescript
// Navigate to localhost HTTP - WORKS (gives real origin)
webview.navigate("http://localhost:8080");

// This page can then connect WebSocket to localhost - WORKS
// const ws = new WebSocket("ws://localhost:8080/ws")
```

### What Doesn't Work
```typescript
// setHTML creates about:blank origin - WebSocket BLOCKED
webview.setHTML(`<html><body>Hello</body></html>`);
// Any WebSocket("ws://localhost:*") from this page will fail
```

### Our Solution
Serve HTML via HTTP and navigate to it:

```typescript
// Start HTTP server to serve static files
const server = Bun.serve({
  port: 8080,
  fetch(req) {
    // Serve index.html, index.css, index.js from src/mainview/
    return serveStatic(req);
  },
  websocket: {
    // Handle WebSocket connections
  }
});

// Navigate to the HTTP server (gives real origin)
webview.navigate("http://localhost:8080/");
```

## API Reference

### Creating a Window

```typescript
import { Webview, SizeHint } from "webview-bun";

const webview = new Webview();

// Optional: enable dev tools (first param)
const webview = new Webview(true);

// Optional: set initial size
const webview = new Webview(false, {
  width: 1200,
  height: 800,
  hint: SizeHint.NONE  // NONE, MIN, MAX, or FIXED
});
```

### Window Properties

```typescript
// Set window title
webview.title = "My App";

// Set window size
webview.size = { width: 800, height: 600, hint: SizeHint.NONE };
```

### Loading Content

```typescript
// Load HTML directly (RECOMMENDED for our use case)
webview.setHTML(`<html><body>Hello</body></html>`);

// Navigate to URL (doesn't work with localhost on macOS!)
webview.navigate("https://example.com");
```

### JavaScript Injection

```typescript
// Inject code that runs before window.onload on every page load
webview.init(`
  console.log('Page loading...');
  window.myGlobal = 42;
`);

// Evaluate code immediately (async, result ignored)
webview.eval(`console.log('Hello from bun!')`);
```

### Binding Functions (Bun <-> Webview Communication)

```typescript
// Bind a function callable from webview JavaScript
webview.bind("myFunction", (arg1, arg2) => {
  console.log("[Bun]", arg1, arg2);
  return { result: "from bun" };  // Returned to JS as Promise
});

// In webview JavaScript:
// const result = await myFunction("hello", 123);
```

### Running the Event Loop

```typescript
// Blocks until window is closed, then auto-destroys
webview.run();

// Manual cleanup (called automatically by run())
webview.destroy();
```

## Complete Example (OpenAgents Pattern)

```typescript
import { Webview, SizeHint } from "webview-bun";
import { join } from "node:path";

const MAINVIEW_DIR = join(import.meta.dir, "../mainview");
const HTTP_PORT = 8080;

// Start HTTP server to serve static files + WebSocket
const server = Bun.serve({
  port: HTTP_PORT,
  async fetch(req, server) {
    const url = new URL(req.url);

    // WebSocket upgrade
    if (url.pathname === "/ws" && server.upgrade(req)) {
      return; // Upgraded to WebSocket
    }

    // Serve static files from mainview dir
    const path = url.pathname === "/" ? "/index.html" : url.pathname;
    const file = Bun.file(join(MAINVIEW_DIR, path));
    if (await file.exists()) {
      return new Response(file);
    }
    return new Response("Not found", { status: 404 });
  },
  websocket: {
    message(ws, msg) { /* handle messages */ },
    open(ws) { console.log("WebSocket connected"); },
    close(ws) { console.log("WebSocket disconnected"); },
  },
});

const webview = new Webview();

// Debug: inject error handler
webview.init(`
  window.onerror = (msg, url, line) => console.error('[JS ERROR]', msg, url, line);
`);

// Bind logging function (useful for debugging)
webview.bind("bunLog", (...args) => console.log("[Webview]", ...args));

webview.title = "OpenAgents";
webview.size = { width: 1200, height: 800, hint: SizeHint.NONE };

// Navigate to HTTP server - gives page real origin for WebSocket
webview.navigate(`http://localhost:${HTTP_PORT}/`);

console.log("[Desktop] Opening window...");
webview.run();
console.log("[Desktop] Window closed");
server.stop();
```

## Building Executables

```bash
# Compile to single executable
bun build --compile --minify src/desktop/main.ts --outfile openagents

# Cross-compile for other platforms
bun build --compile --target=bun-windows-x64 --minify src/desktop/main.ts --outfile openagents.exe
bun build --compile --target=bun-linux-x64 --minify src/desktop/main.ts --outfile openagents-linux
```

### macOS: Hide Terminal Window
Add `.app` extension to output:
```bash
bun build --compile --minify src/desktop/main.ts --outfile openagents.app
```

## Using Bun.serve() with Webview

**CRITICAL**: `webview.run()` blocks Bun's event loop. You MUST run the server in a Worker:

```typescript
// main.ts
const worker = new Worker("./server-worker.ts", {
  env: { STATIC_DIR: "./src/mainview", HTTP_PORT: "8080" }
});
await new Promise(r => setTimeout(r, 500)); // Wait for server

const webview = new Webview();
webview.navigate("http://localhost:8080/"); // Use navigate, not setHTML!
webview.run();
worker.terminate();
```

```typescript
// server-worker.ts
Bun.serve({
  port: parseInt(process.env.HTTP_PORT || "8080", 10),
  async fetch(req, server) {
    const url = new URL(req.url);
    if (url.pathname === "/ws" && server.upgrade(req)) return;

    // Serve static files
    const file = Bun.file(process.env.STATIC_DIR + url.pathname);
    if (await file.exists()) return new Response(file);
    return new Response("Not found", { status: 404 });
  },
  websocket: {
    open(ws) { console.log("Client connected"); },
    message(ws, msg) { /* handle */ },
    close(ws) { console.log("Client disconnected"); }
  }
});
setInterval(() => {}, 1000); // Keep worker alive
```

## Debugging Tips

1. **White screen?** The server might not be responding. Make sure it's in a Worker!

2. **JS not running?** Inject debug code:
   ```typescript
   webview.init(`console.log('[DEBUG] Page loaded')`);
   ```

3. **Need console output?** Bind a log function:
   ```typescript
   webview.bind("bunLog", (...args) => console.log("[Webview]", ...args));
   // In JS: bunLog("message")
   ```

4. **Build the JS bundle first:**
   ```bash
   bun build src/mainview/index.ts --target browser --outdir src/mainview
   ```

## Platform Notes

### macOS
- WebKit is built-in, no deps needed
- Needs `cmake` and `ninja` for building from source
- Use `.app` extension for compiled binaries to hide terminal

### Windows
- Requires Microsoft Edge WebView2 runtime
- Pre-installed on Windows 11, manual install on older versions
- Use `scripts/hidecmd.bat` to hide terminal on compiled exe

### Linux
- Requires GTK 4 and WebKitGTK 6:
  - Debian: `sudo apt install libgtk-4-1 libwebkitgtk-6.0-4`
  - Arch: `sudo pacman -S gtk4 webkitgtk-6.0`
  - Fedora: `sudo dnf install gtk4 webkitgtk6.0`

## File Structure in OpenAgents

```
src/desktop/
  main.ts        # Entry point - creates webview, loads content
  server.ts      # HTTP + WebSocket server (for HUD)
  protocol.ts    # Socket message types
  handlers.ts    # Request handlers

src/mainview/
  index.html     # Main HTML
  index.css      # Styles
  index.ts       # Source TypeScript
  index.js       # Bundled JS (bun build output)
  socket-client.ts  # WebSocket client for server communication
```

## OpenAgents Desktop Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              DESKTOP APP                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐                      ┌─────────────────────────────┐  │
│  │   webview-bun   │      setHTML()       │         UI (HTML)           │  │
│  │  (native win)   │ ────────────────────→│                             │  │
│  │                 │                      │  - index.html (inlined)     │  │
│  │  main.ts        │                      │  - index.css (inlined)      │  │
│  │                 │                      │  - index.js  (inlined)      │  │
│  └─────────────────┘                      │                             │  │
│                                           │  ┌───────────────────────┐  │  │
│                                           │  │    SocketClient       │  │  │
│                                           │  │  (socket-client.ts)   │  │  │
│                                           │  └───────────┬───────────┘  │  │
│                                           └──────────────│──────────────┘  │
│                                                          │                  │
│                                                          │ WebSocket        │
│                                                          │ ws://localhost:8080/ws
│                                                          │                  │
│  ┌───────────────────────────────────────────────────────▼───────────────┐  │
│  │                        DesktopServer                                  │  │
│  │                         (server.ts)                                   │  │
│  │                                                                       │  │
│  │   ┌─────────────────────────┐    ┌─────────────────────────────────┐ │  │
│  │   │   HTTP Server :8080     │    │   HUD Server :4242              │ │  │
│  │   │                         │    │                                 │ │  │
│  │   │  - Static files         │    │  - Agent connections            │ │  │
│  │   │  - WebSocket upgrade    │    │  - HUD message ingestion        │ │  │
│  │   │  - RPC handling         │    │                                 │ │  │
│  │   └─────────────────────────┘    └─────────────────────────────────┘ │  │
│  │                                                                       │  │
│  │   ┌─────────────────────────────────────────────────────────────────┐│  │
│  │   │                    handleRequest()                              ││  │
│  │   │                     (handlers.ts)                               ││  │
│  │   │                                                                 ││  │
│  │   │  - loadTBSuite      - loadRecentTBRuns                         ││  │
│  │   │  - startTBRun       - loadTBRunDetails                         ││  │
│  │   │  - stopTBRun                                                   ││  │
│  │   └─────────────────────────────────────────────────────────────────┘│  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                      ▲
                                      │ WebSocket :4242
                                      │
┌─────────────────────────────────────┴─────────────────────────────────────┐
│                              EXTERNAL AGENTS                              │
│                                                                           │
│   MechaCoder, Claude Code, TB Runners, etc.                              │
│   Connect to ws://localhost:4242 to send HUD messages                    │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### Data Flow

**1. Content Loading (startup)**
```
main.ts → reads HTML/CSS/JS files → inlines them → webview.setHTML()
```

**2. RPC Requests (UI → Backend)**
```
UI button click
  → SocketClient.request("request:loadTBSuite", { suitePath })
  → WebSocket to DesktopServer
  → handleRequest() in handlers.ts
  → Response back via WebSocket
  → SocketClient resolves Promise
```

**3. HUD Events (Agents → UI)**
```
Agent connects to :4242
  → Sends HudMessage (JSON)
  → DesktopServer receives
  → Broadcasts to all UI clients via :8080 WebSocket
  → SocketClient.onMessage() fires
  → UI updates
```

### Why This Architecture?

1. **setHTML() bypasses WebKit restriction** - Can't navigate to localhost HTTP, but can load HTML directly

2. **WebSocket works from setHTML() content** - Even though HTTP navigation is blocked, WebSocket connections TO localhost work fine

3. **Single server handles everything** - DesktopServer provides both HTTP (for dev/debugging) and WebSocket (for RPC + HUD)

4. **Agents connect separately** - HUD port 4242 is dedicated to agent connections, keeping concerns separated

5. **No webview.bind() for RPC** - WebSocket is more flexible, supports correlation IDs, and doesn't require Bun-specific APIs in the frontend

### Socket Protocol

**Requests** (UI → Backend):
```typescript
{
  type: "request:loadTBSuite",
  correlationId: "abc123",
  suitePath: "path/to/suite.json"
}
```

**Responses** (Backend → UI):
```typescript
{
  type: "response:loadTBSuite",
  correlationId: "abc123",
  success: true,
  data: { name: "...", tasks: [...] }
}
```

**HUD Events** (Agents → UI):
```typescript
{
  type: "task:start",
  runId: "tb-123",
  taskId: "task-1",
  timestamp: "2024-..."
}
```

## UI Views

The HUD has two view modes accessible via the toggle buttons or keyboard shortcuts:

### MC View (MechaCoder) - Ctrl+1

Displays ready tasks from `.openagents/tasks.jsonl`:

- **Ready Tasks Widget**: Shows tasks that are ready to work on (status=open, no blocking deps)
- Tasks sorted by priority (P0-P4) and age
- Displays: Priority badge, Task ID, Title, Type, Labels
- Up to 15 tasks shown with overflow indicator
- Auto-loads when switching to MC view

**RPC Request:**
```typescript
{
  type: "request:loadReadyTasks",
  correlationId: "abc123",
  limit: 20  // optional
}
```

**Response:**
```typescript
{
  type: "response:loadReadyTasks",
  correlationId: "abc123",
  success: true,
  data: [{
    id: "oa-abc123",
    title: "Fix bug in parser",
    description: "...",
    status: "open",
    priority: 1,
    type: "bug",
    labels: ["parser", "urgent"],
    createdAt: "2024-...",
    updatedAt: "2024-..."
  }, ...]
}
```

### TB View (Terminal-Bench) - Ctrl+2

Displays Terminal-Bench run history and controls:

- **TB Controls Panel**: Load suite, start/stop runs, random task
- **Run History Flow**: Visual tree of past runs with pass/fail rates
- **Category Tree**: Hierarchical task view during runs
- **Output Viewer**: Live streaming output from running tasks
- **Comparison Widget**: Compare current run against a baseline (Shift+click)

**Keyboard Shortcuts (TB View):**
- `Ctrl+L` - Load suite
- `Ctrl+T` - Start run
- `Ctrl+R` - Start random task
- `Ctrl+X` - Stop run
- `Ctrl+B` - Clear baseline comparison
- `Shift+Click` on run node - Set as comparison baseline

## Lessons Learned

1. **Use `navigate()` to localhost HTTP** - This gives the page a real origin so WebSocket works.
   `setHTML()` creates `about:blank` origin which blocks WebSocket to localhost.

2. **Build with `--format iife`** - ES module exports (`export {}`) don't work in inline scripts:
   ```bash
   bun build src/mainview/index.ts --target browser --format iife --outdir src/mainview
   ```

3. **addEventListener works, inline onclick doesn't** - WebKit CSP blocks inline event handlers:
   ```html
   <!-- DOESN'T WORK -->
   <button onclick="doThing()">Click</button>

   <!-- WORKS -->
   <button id="btn">Click</button>
   <script>document.getElementById('btn').addEventListener('click', doThing)</script>
   ```

4. **localStorage/sessionStorage works with real origin** - When using `navigate()` to localhost,
   storage APIs work normally. They only fail with `about:blank` origin (setHTML).

5. **WebSocket needs real origin** - WebSocket connections to localhost are blocked from `about:blank`.
   Use `navigate("http://localhost:PORT/")` to give the page a real origin.

6. **Debug with bunLog binding** - Bind a function to get console output in terminal:
   ```typescript
   // In main.ts
   webview.bind("bunLog", (...args) => console.log("[Webview]", ...args));

   // In frontend JS
   window.bunLog?.("Debug message");
   ```

7. **Serve static files via HTTP** - The HTTP server serves HTML, CSS, JS from src/mainview/.
   WebSocket is handled on the same port via upgrade.

8. **CRITICAL: Run server in a Worker** - `webview.run()` blocks Bun's event loop completely.
   If you run the HTTP server in the main thread, it won't respond while the webview is open!
   ```typescript
   // server-worker.ts
   import { createDesktopServer } from "./server.js";
   const server = createDesktopServer({
     staticDir: process.env.STATIC_DIR!,
     httpPort: parseInt(process.env.HTTP_PORT || "8080", 10),
     verbose: true,
   });
   setInterval(() => {}, 1000); // Keep worker alive
   ```

   ```typescript
   // main.ts
   const worker = new Worker("./server-worker.ts", {
     env: { STATIC_DIR: MAINVIEW_DIR, HTTP_PORT: "8080" },
   });
   await new Promise(r => setTimeout(r, 500)); // Wait for server to start

   webview.navigate("http://localhost:8080/");
   webview.run(); // This blocks, but Worker keeps serving

   worker.terminate(); // Cleanup on exit
   ```

9. **No `type="module"` with IIFE bundles** - If building with `--format iife`, use plain `<script>`:
   ```html
   <!-- WRONG with IIFE bundle -->
   <script type="module" src="index.js"></script>

   <!-- CORRECT with IIFE bundle -->
   <script src="index.js"></script>
   ```
