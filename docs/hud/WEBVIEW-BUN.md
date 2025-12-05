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

## Key Limitation: macOS WebKit Blocks localhost HTTP

**CRITICAL**: On macOS, WebKit blocks `navigate()` calls to `http://localhost:*` URLs. This is a security restriction in the underlying WKWebView.

### What Works
```typescript
// Direct HTML content - WORKS
webview.setHTML(`<html><body>Hello</body></html>`);

// Data URIs - WORKS
webview.navigate("data:text/html,<html><body>Hello</body></html>");
```

### What Doesn't Work
```typescript
// localhost HTTP - BLOCKED (shows white screen)
webview.navigate("http://localhost:8080");
webview.navigate("http://127.0.0.1:8080");
```

### Our Solution
Load files and embed them directly via `setHTML()`:

```typescript
const html = await Bun.file("index.html").text();
const css = await Bun.file("index.css").text();
const js = await Bun.file("index.js").text();

const inlined = html
  .replace('<link rel="stylesheet" href="index.css">', `<style>${css}</style>`)
  .replace('<script src="index.js"></script>', `<script>${js}</script>`);

webview.setHTML(inlined);
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

// Load and inline all content
const html = await Bun.file(join(MAINVIEW_DIR, "index.html")).text();
const css = await Bun.file(join(MAINVIEW_DIR, "index.css")).text();
const js = await Bun.file(join(MAINVIEW_DIR, "index.js")).text();

const inlined = html
  .replace('<link rel="stylesheet" href="index.css">', `<style>${css}</style>`)
  .replace('<script type="module" src="index.js"></script>', `<script>${js}</script>`);

const webview = new Webview();

// Debug: inject error handler
webview.init(`
  window.onerror = (msg, url, line) => console.error('[JS ERROR]', msg, url, line);
`);

// Bind logging function
webview.bind("bunLog", (...args) => console.log("[Webview]", ...args));

webview.title = "OpenAgents";
webview.size = { width: 1200, height: 800, hint: SizeHint.NONE };
webview.setHTML(inlined);

console.log("[Desktop] Opening window...");
webview.run();
console.log("[Desktop] Window closed");
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

If you need a server running alongside the webview (e.g., for WebSocket), use Workers:

```typescript
// index.ts
const worker = new Worker("./server-worker.ts");
const webview = new Webview();
webview.setHTML(html);  // Use setHTML, not navigate to localhost
webview.run();
worker.terminate();
```

```typescript
// server-worker.ts
Bun.serve({
  port: 4242,
  fetch(req, server) {
    if (server.upgrade(req)) return;
    return new Response("OK");
  },
  websocket: { /* ... */ }
});
```

## Debugging Tips

1. **White screen?** Check if you're using `navigate()` with localhost - use `setHTML()` instead

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

## Lessons Learned

1. **Always use `setHTML()` on macOS** - `navigate()` to localhost HTTP is blocked by WebKit

2. **Build with `--format iife`** - ES module exports (`export {}`) don't work in inline scripts:
   ```bash
   bun build src/mainview/index.ts --target browser --format iife --outdir src/mainview
   ```

3. **NO `type="module"` scripts** - Inline scripts must be plain `<script>`, not `<script type="module">`

4. **addEventListener works, inline onclick doesn't** - WebKit CSP blocks inline event handlers:
   ```html
   <!-- DOESN'T WORK -->
   <button onclick="doThing()">Click</button>

   <!-- WORKS -->
   <button id="btn">Click</button>
   <script>document.getElementById('btn').addEventListener('click', doThing)</script>
   ```

5. **localStorage/sessionStorage is BLOCKED** - In about:blank context (setHTML), storage APIs throw SecurityError:
   ```typescript
   // Wrap in try-catch
   let setting = "default";
   try {
     setting = localStorage.getItem("key") || "default";
   } catch {
     // Blocked in webview context
   }
   ```

6. **WebSocket connections work** - Despite HTTP navigation being blocked, WebSocket TO localhost works fine from setHTML content

7. **Debug with bunLog binding** - Bind a function to get console output in terminal:
   ```typescript
   // In main.ts
   webview.bind("bunLog", (...args) => console.log("[Webview]", ...args));

   // In frontend JS
   window.bunLog?.("Debug message");
   ```

8. **Wrap large bundles in try-catch** - Silent failures are hard to debug:
   ```typescript
   const wrappedJs = `
   try {
   ${js}
   } catch(e) {
     window.bunLog?.('[JS ERROR] ' + e.name + ': ' + e.message);
   }
   `;
   ```
