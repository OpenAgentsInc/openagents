The hardest-core Electron optimization is to stop treating it as “a website in a desktop wrapper.” Treat it as a **latency-sensitive, multi-process native application whose UI happens to use Chromium**.

The priority order is usually:

1. **Show a usable window almost immediately**
2. **Keep the main process nearly empty**
3. **Minimize renderer JavaScript before first interaction**
4. **Move sustained computation off the renderer**
5. **Avoid moving large data through Electron IPC**
6. **Render only what is visible**
7. **Measure cold starts on bad hardware**

Electron’s own guidance emphasizes profiling, delaying unnecessary work, avoiding blocked main/renderer processes, bundling, and removing unnecessary dependencies and network requests. ([Electron][1])

# 1. Build a two-phase startup

Your initial startup path should do almost nothing:

```text
process starts
→ acquire single-instance lock
→ disable unused native menu
→ app ready
→ create hidden window
→ load tiny local shell
→ first paint
→ show window
→ asynchronously initialize everything else
```

The initial renderer should contain only enough code to display:

* Window chrome
* Navigation skeleton
* Last-known workspace
* Placeholder content
* Input controls
* A lightweight loading state

Do **not** initialize these before first paint:

* Auto-update checks
* Telemetry upload
* Git repository scanning
* Extension discovery
* Search indexes
* Database migrations beyond the minimum required
* Authentication refreshes
* Cloud synchronization
* AI/model initialization
* Syntax highlighting
* Markdown engines
* Monaco
* Large icon libraries
* Settings panels
* Background services that the first screen does not need

Electron specifically recommends allocating resources just in time rather than eagerly and notes that module loading can be especially expensive on Windows. ([Electron][1])

A practical startup implementation:

```ts
import { app, BrowserWindow, Menu } from "electron";
import path from "node:path";

Menu.setApplicationMenu(null);

let mainWindow: BrowserWindow | undefined;

async function createWindow(): Promise<void> {
  mainWindow = new BrowserWindow({
    show: false,
    backgroundColor: "#0b0d10",
    width: 1280,
    height: 800,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });

  mainWindow.once("ready-to-show", () => {
    mainWindow?.show();
  });

  await mainWindow.loadFile("renderer/index.html");

  // Do not await this.
  void initializeAfterFirstPaint();
}

async function initializeAfterFirstPaint(): Promise<void> {
  await new Promise<void>((resolve) => setImmediate(resolve));

  const [
    { startBackgroundSync },
    { initializeUpdater },
    { warmApplicationServices },
  ] = await Promise.all([
    import("./services/sync.js"),
    import("./services/updater.js"),
    import("./services/application-services.js"),
  ]);

  void startBackgroundSync();
  void initializeUpdater();
  void warmApplicationServices();
}

void app.whenReady().then(createWindow);
```

Do not blindly wait for every service before showing the window. The distinction is:

* **Window visible**
* **Window interactive**
* **Primary data available**
* **Application fully warmed**

Instrument those as separate milestones.

# 2. Make the startup bundle microscopic

The app’s initial bundle should not include the entire application merely because tree-shaking theoretically exists.

Create explicit entry-point layers:

```text
main-bootstrap
preload-bootstrap
renderer-shell
renderer-primary-route
renderer-secondary-routes
workers
utility-processes
```

For the first renderer chunk, aim for something like:

* No charting library
* No editor
* No document parser
* No syntax grammar
* No giant date library
* No full icon package
* No admin UI
* No settings UI
* No rarely used modal code

Use route- and feature-level dynamic imports:

```ts
const MonacoEditor = lazy(() => import("./editor/MonacoEditor"));
const AnalyticsPanel = lazy(() => import("./analytics/AnalyticsPanel"));
const Settings = lazy(() => import("./settings/Settings"));
```

But do not create hundreds of tiny chunks. On desktop, excessive file operations can become its own startup cost, especially under Windows Defender or other endpoint security software.

A good production shape is often:

* One very small boot chunk
* One primary-screen chunk
* Several coarse feature chunks
* Separate workers

Electron’s documentation recommends bundling because repeated module resolution and `require()` operations add startup overhead. ([Electron][1])

## Audit dependency initialization cost

Package size alone is not enough. Measure:

* Parse time
* Compilation time
* Top-level execution
* Files opened
* JSON parsed
* Transitive dependencies
* Memory retained after import

For Node-side dependencies:

```bash
node --cpu-prof --heap-prof -e "require('suspect-package')"
```

Electron explicitly recommends this sort of profiling for determining the real import cost of dependencies. ([Electron][1])

Look aggressively for modules that:

* Read package metadata at import time
* Scan directories at import time
* Load huge JSON files
* Initialize locale tables
* Detect hundreds of environment cases
* Import every language grammar
* Register global plugins
* Instantiate clients immediately

Replace heavy packages with five lines of direct platform code when feasible.

# 3. Keep the main process as a control plane

The main process should manage:

* Windows
* Menus
* Tray
* OS integration
* Process lifecycle
* Permission boundaries
* Routing messages to appropriate services

It should not perform:

* Parsing
* Indexing
* Image manipulation
* Compression
* Repository analysis
* Search
* Large database queries
* Encryption of large payloads
* AI inference
* Long synchronous filesystem operations

The main process houses critical UI responsibilities; blocking it can freeze interactions across the application. Electron recommends asynchronous I/O and moving CPU-heavy work to workers or separate processes. ([Electron][1])

For isolated Node workloads, use:

* `utilityProcess.fork()`
* `worker_threads`
* A persistent daemon process
* A native service, for truly serious applications

Electron has a first-class `utilityProcess` API for spawning child processes with Node.js and MessagePort support. ([Electron][2])

A useful architecture:

```text
Main process
  ├── Window management
  ├── Native integrations
  └── Service broker

Renderer
  ├── UI state
  └── Interaction handling

Utility process: database
Utility process: search/indexing
Utility process: Git/filesystem
Utility process: synchronization
Worker: parsing
Worker: image processing
```

Why multiple services rather than one huge background process?

* Independent crash containment
* Better profiling
* Easier priority management
* Less accidental event-loop interference
* Work can be started only when needed

Do not overdo it, though. Every process has a real memory cost because Electron inherits Chromium’s multi-process architecture. Each window or web content can involve another renderer. ([Electron][3])

# 4. Eliminate synchronous IPC

Never use synchronous IPC on a hot path.

Avoid:

```ts
ipcRenderer.sendSync(...)
```

Also avoid pretending asynchronous IPC is free. This can still be expensive:

```ts
const giantResult = await window.api.getEntireDatabase();
```

IPC can incur:

* Serialization
* Copying
* Main-process scheduling
* Renderer scheduling
* Garbage collection
* State reconciliation
* React rerenders

Prefer coarse commands with incremental results:

```ts
await window.api.search.start({
  query,
  limit: 100,
});
```

Then stream batches:

```text
results: 1–20
results: 21–40
results: 41–60
```

For high-volume channels, prefer:

* `MessageChannelMain`
* `MessagePort`
* Transferable `ArrayBuffer`s
* Shared memory where the design justifies it
* File-backed exchange for very large payloads
* Local sockets to a persistent native daemon

Avoid repeatedly serializing a 50 MB object graph through IPC.

A strong design is to send:

```ts
{
  entityIds: ["a", "b", "c"],
  revision: 418
}
```

rather than:

```ts
{
  entireApplicationState: { /* tens of megabytes */ }
}
```

# 5. Do not hydrate the whole app

For React, the biggest Electron renderer failure mode is often:

```text
load enormous JS bundle
→ execute framework
→ create every provider
→ deserialize all persisted state
→ mount entire application tree
→ rerender repeatedly while services connect
```

Instead:

* Render the shell immediately
* Restore only the active workspace
* Fetch visible data
* Subscribe only to active regions
* Mount expensive providers within routes that need them
* Keep server/service state outside global React context
* Do not put rapidly changing data in root-level context

Bad:

```tsx
<AppProvider value={entireApplicationState}>
  <Everything />
</AppProvider>
```

Better:

```tsx
<AppShell>
  <WorkspaceRoute workspaceId={activeWorkspaceId} />
</AppShell>
```

Use selector-based subscriptions:

```ts
const selectedItem = useStore(
  (state) => state.itemsById[selectedId],
  shallow,
);
```

Do not subscribe a component to a 20,000-item array when it needs one record.

# 6. Virtualize everything

Never render thousands of elements merely because Chromium technically can.

Virtualize:

* Lists
* Trees
* Tables
* Logs
* Chat histories
* File explorers
* Search results
* Timeline events
* Command output
* Diff lines
* Editor decorations

The real problem is not only DOM node count. Large trees increase:

* Style calculation
* Layout
* Paint
* Accessibility tree work
* Event listener overhead
* Framework reconciliation
* Garbage collection pressure

Use fixed or estimated row heights where possible. Variable-height virtualization is more expensive and often produces layout feedback loops.

For extremely large text/data surfaces, consider drawing with:

* Canvas 2D
* WebGL
* WebGPU
* A purpose-built native/editor rendering engine

For example, a million-cell grid should generally not be a million DOM nodes.

# 7. Make rendering compositor-friendly

Aim to keep animation and scrolling on the compositor thread.

Prefer animation of:

* `transform`
* `opacity`

Avoid animating:

* `width`
* `height`
* `top`
* `left`
* `margin`
* Large blur radii
* Large shadows
* Complex filters
* Backgrounds that trigger massive repaints

Be cautious with:

```css
backdrop-filter: blur(40px);
filter: blur(...);
box-shadow: 0 0 100px ...;
```

These can be surprisingly expensive when used across large or frequently changing surfaces.

Use containment strategically:

```css
.panel {
  contain: layout paint style;
}

.virtual-row {
  contain: content;
}
```

For off-screen sections:

```css
.section {
  content-visibility: auto;
  contain-intrinsic-size: 800px;
}
```

`content-visibility` can be highly effective for document-like screens, although it should be tested carefully with accessibility, scroll anchoring, and measurement code.

Use `will-change` sparingly. Applying it everywhere can increase GPU memory and layer-management costs.

# 8. Stop layout thrashing

Do not interleave DOM reads and writes:

```ts
for (const item of items) {
  item.element.style.width = `${container.offsetWidth}px`;
}
```

That can repeatedly force style/layout.

Batch reads, then writes:

```ts
const width = container.getBoundingClientRect().width;

requestAnimationFrame(() => {
  for (const item of items) {
    item.element.style.width = `${width}px`;
  }
});
```

Watch for code that repeatedly reads:

* `offsetWidth`
* `offsetHeight`
* `getBoundingClientRect()`
* `scrollTop`
* Computed styles

after mutating styles or DOM structure.

Use `ResizeObserver` rather than polling measurements. But do not let observers trigger unlimited measure-update-measure loops.

# 9. Schedule work around frames

At 60 Hz, you have roughly 16.7 ms per frame, and your JavaScript does not own that whole budget.

Use:

* `requestAnimationFrame()` for visual writes
* `requestIdleCallback()` for genuinely low-priority tasks
* `scheduler.postTask()` where supported and appropriate
* Workers for actual CPU work
* Chunked processing for tasks that must stay in the renderer

Electron explicitly calls out `requestIdleCallback()` and Web Workers as primary tools for preserving renderer responsiveness. ([Electron][1])

Chunking example:

```ts
async function processInChunks<T>(
  items: readonly T[],
  processItem: (item: T) => void,
  budgetMs = 5,
): Promise<void> {
  let index = 0;

  while (index < items.length) {
    const deadline = performance.now() + budgetMs;

    while (index < items.length && performance.now() < deadline) {
      processItem(items[index]);
      index += 1;
    }

    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });
  }
}
```

That is inferior to a worker for heavy computation, but much better than one 400 ms long task.

# 10. Use workers properly

Move these out of the renderer:

* Markdown parsing
* Syntax parsing
* Diff calculation
* Compression
* Hashing
* Image resizing
* Search ranking
* Embeddings
* Large JSON transforms
* CSV parsing
* Document conversion
* Cryptography
* Tree traversal
* Data aggregation

But worker startup is not free. For frequently used work:

* Keep a worker pool alive
* Size it conservatively
* Prioritize interactive work over background jobs
* Avoid duplicating enormous lookup tables in every worker
* Transfer buffers instead of cloning them

```ts
worker.postMessage(
  { type: "parse", bytes: arrayBuffer },
  [arrayBuffer],
);
```

The transfer list prevents copying the underlying buffer.

For a major desktop product, one of the highest-leverage changes is often a **persistent service process** rather than constantly spawning workers.

# 11. Replace hot JavaScript with native or Wasm code

For true hotspot optimization, move stable computational kernels to:

* Rust via N-API
* C++ Node addons
* WebAssembly
* A standalone Rust/Go/C++ service

Candidates include:

* Search indexing
* Diff algorithms
* Compression
* Image/video processing
* Parsing
* Tokenization
* Database extensions
* Cryptographic operations
* Large-scale text transformations

Do this only after profiling. Crossing the JS/native boundary has overhead, and poorly designed native code can make performance worse.

A useful rule:

* Small calls repeated millions of times: bad boundary design
* One large typed buffer in, one large result out: good boundary design

Wasm is particularly useful when the workload is CPU-bound and can operate over contiguous memory. It does not inherently solve DOM, IPC, filesystem, or React problems.

# 12. Use a serious local data layer

Do not load the entire application database into renderer memory.

Use SQLite, RocksDB, LMDB, or another indexed local store depending on requirements. SQLite is usually the default answer.

Keep the database in a dedicated service or utility process. Expose purpose-built queries:

```ts
searchMessages({
  workspaceId,
  query,
  cursor,
  limit: 50,
});
```

Not:

```ts
getAllMessages();
```

For SQLite:

* Enable WAL mode where appropriate
* Add indexes based on actual query plans
* Batch writes in transactions
* Use prepared statements
* Avoid N+1 queries
* Avoid enormous JSON columns when queryable normalized data is needed
* Paginate with keyset/cursor pagination rather than deep offsets
* Run expensive maintenance outside interactive startup
* Consider FTS5 for text search

The fastest React update is the one caused by a query that returned 50 rows rather than 50,000.

# 13. Treat parsing persisted state as a startup hazard

A 30 MB JSON settings/session file can destroy cold startup through:

* Disk read
* UTF-8 decoding
* JSON parsing
* Object allocation
* GC
* Framework state propagation

Instead:

* Split persisted state by feature
* Read only boot-critical keys
* Use SQLite or a binary indexed format for large state
* Keep a compact startup manifest
* Store a precomputed “last visible screen” projection
* Version and migrate lazily
* Do not rewrite giant files for tiny changes

Example:

```text
boot.json                2 KB
window-state.json        1 KB
recent-workspaces.db
workspace-data.db
search-index.db
```

rather than:

```text
entire-app-state.json    87 MB
```

# 14. Make local assets genuinely local

Bundle assets that rarely change:

* Fonts
* Icons
* Base themes
* Syntax themes needed at startup
* Skeleton images
* Core configuration
* Default templates

Do not block rendering on a CDN, remote font, analytics endpoint, auth check, or configuration service.

Electron recommends avoiding unnecessary network requests and bundling static resources locally. ([Electron][1])

Also:

* Subset fonts
* Prefer WOFF2 where relevant
* Preload only the exact fonts used above the fold
* Avoid five weights of a large font family
* Use SVG sprites or targeted icon imports
* Decode large images asynchronously
* Produce thumbnails instead of rendering full-resolution originals
* Use `loading="lazy"` for noncritical images

# 15. Optimize packaging and filesystem access

Packaging matters more than most web developers expect.

## ASAR

ASAR can reduce the number of loose-file filesystem operations and simplify distribution, but:

* Native modules generally need unpacking
* Some libraries expect real filesystem paths
* Large frequently streamed resources may be better unpacked
* Poorly configured glob patterns can ship huge amounts of dead content

Audit the final artifact, not the source tree.

Remove:

* Source maps from production distributions unless intentionally shipped
* Tests
* Examples
* Storybook
* Documentation
* Unused locales
* Unused native binaries
* Development-only packages
* Duplicate dependency versions
* Build caches
* Source TypeScript
* Unneeded platform resources

On Windows, thousands of tiny files can be punished by antivirus scanning. Reducing file count and startup-time file opens can materially improve cold startup.

## Do not unpack everything

An overly broad `asarUnpack` pattern can turn the app back into thousands of loose files. Unpack only assets and binaries that truly need it.

# 16. Keep preload scripts tiny

Preload runs before renderer content and is therefore part of startup.

Bad preload:

```ts
import "./database";
import "./git";
import "./analytics";
import "./sync";
import "./filesystem";
```

Good preload:

```ts
import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("desktop", {
  commands: {
    execute: (name: string, args: unknown) =>
      ipcRenderer.invoke("command:execute", { name, args }),
  },
});
```

The preload should primarily expose a narrow capability bridge. Heavy modules belong elsewhere.

# 17. Reuse windows carefully

Creating a renderer process is expensive. For frequently reopened utility surfaces—command palette, quick capture, mini player—you can sometimes:

* Create the window after initial startup
* Keep it hidden when closed
* Reuse its renderer
* Refresh its data when shown

But hidden windows continue consuming memory, and sometimes CPU. This is a latency-versus-memory tradeoff.

Use a tiered approach:

* Keep one or two frequently used surfaces warm
* Destroy rare or heavy windows
* Do not preload every possible window
* Consider a single-window route architecture where UX permits it

Electron creates separate renderer processes for windows and web embeds, so uncontrolled window proliferation has meaningful overhead. ([Electron][3])

# 18. Freeze or reduce background activity

When a window is hidden or inactive:

* Pause animations
* Reduce polling frequency
* Stop video/canvas rendering
* Suspend observers not needed
* Pause expensive subscriptions
* Coalesce background updates
* Lower worker priority conceptually through your own scheduler
* Stop rendering logs that nobody can see

Do not globally disable background throttling unless you genuinely need continuous rendering; doing so can waste considerable CPU and battery.

For live systems, separate:

```text
data continues arriving
```

from:

```text
UI renders every event immediately
```

Aggregate and render at a reasonable cadence:

```ts
const pendingEvents: Event[] = [];

setInterval(() => {
  if (document.visibilityState !== "visible") return;
  flushVisibleUpdates(pendingEvents.splice(0));
}, 100);
```

# 19. Build a priority scheduler

A sophisticated Electron app should not let arbitrary subsystems race for resources.

Define work classes:

```ts
enum Priority {
  Immediate,
  UserBlocking,
  Visible,
  Background,
  Maintenance,
}
```

Examples:

* Keystroke processing: `Immediate`
* Opening a document: `UserBlocking`
* Syntax highlighting visible lines: `Visible`
* Indexing unopened files: `Background`
* Vacuuming a database: `Maintenance`

Limit concurrent background jobs and preempt or postpone them when interactive work arrives.

Without this, “async” background jobs can still saturate:

* CPU
* Disk
* Database locks
* Worker pool
* IPC
* Network
* Memory bandwidth

That makes the UI stutter even though the renderer itself looks innocent in a basic JavaScript profile.

# 20. Optimize perceived latency intentionally

Perceived performance is not fake performance. Make interactions acknowledge immediately.

For every user action:

```text
0–50 ms: visible acknowledgement
<100 ms: UI state change
100–500 ms: useful partial result
>500 ms: progressive result or status
```

Examples:

* Open the editor before syntax highlighting is ready
* Display cached data before synchronization completes
* Show the command palette before all commands are indexed
* Render plain text before Markdown decoration
* Display file names before metadata and thumbnails
* Optimistically update local UI before background persistence

Electron’s docs use VS Code’s approach as an example: show the file promptly, then add highlighting afterward. ([Electron][1])

# 21. Go beyond React when a surface demands it

React is usually fine for application structure. It is not necessarily the best rendering engine for every hot surface.

For critical paths, consider:

* Imperative DOM updates
* Fine-grained reactive frameworks
* Canvas/WebGL
* A purpose-built text editor
* Custom elements
* A separate optimized rendering subsystem

Examples where ordinary React trees often struggle:

* Terminal emulators
* Million-row logs
* Real-time topology visualizations
* Very large diff views
* Waveforms
* High-frequency telemetry
* Spreadsheets
* Node graphs

You can keep React as the shell while using a specialized renderer inside one component.

# 22. Profile the whole multi-process system

Renderer DevTools alone are insufficient for some Electron problems.

Measure:

* Main process CPU
* Renderer CPU
* GPU process
* Utility processes
* Disk I/O
* IPC frequency and payload size
* Memory per process
* Long tasks
* Layout/paint
* GPU rasterization
* Startup module loading
* Cold filesystem behavior

Electron recommends Chrome DevTools and, for advanced multi-process analysis, Chromium tracing. Its `contentTracing` API exposes Chromium tracing from Electron. ([Electron][1])

Instrument application milestones:

```ts
performance.mark("renderer-entry");

// ...
performance.mark("shell-mounted");

// ...
performance.mark("primary-data-ready");

// ...
performance.measure(
  "entry-to-shell",
  "renderer-entry",
  "shell-mounted",
);
```

Main-process milestones can use:

```ts
const startedAt = performance.now();

app.whenReady().then(() => {
  console.log("app-ready", performance.now() - startedAt);
});
```

More useful metrics:

```text
process launch → app ready
app ready → BrowserWindow created
BrowserWindow created → navigation committed
navigation committed → first paint
first paint → first interaction
first interaction → primary data visible
peak startup RSS
idle CPU after 30 seconds
p95 input-to-paint latency
long tasks over 50 ms
dropped frames during critical flows
```

# 23. Test cold, not just warm

Warm starts hide:

* Filesystem latency
* Antivirus scans
* V8 compilation
* OS cache effects
* Database page loading
* Font initialization
* GPU startup
* Network dependency mistakes

Test at least:

* First run after install
* First run after reboot
* Warm restart
* Large existing workspace
* Slow SSD
* 8 GB RAM
* Integrated GPU
* Battery mode
* Windows with Defender enabled
* Offline
* High-DPI display
* Multiple monitors

Do not optimize solely on a development Mac with a warm filesystem cache.

# 24. Establish hard budgets

Give every layer a budget. For example:

| Metric                    |                Aggressive target |
| ------------------------- | -------------------------------: |
| Visible window            |            under 300–500 ms warm |
| Shell interactive         |                under 700 ms warm |
| Cold shell interactive    | under 1.5 s on baseline hardware |
| Initial renderer JS       |      under 250–500 KB compressed |
| Long tasks during startup |         none over 50 ms, ideally |
| Input response            |      under 50 ms acknowledgement |
| UI frame work             |                    under 8–10 ms |
| Idle CPU                  |                 approximately 0% |
| Hidden-window CPU         |                 approximately 0% |
| Initial DOM               |       under a few thousand nodes |
| IPC request payload       |              normally KB, not MB |

The exact numbers depend on the app, but having no budget means every dependency can steal startup time.

# The most extreme architecture

For a truly performance-obsessed Electron application, I would use something like:

```text
Electron main
  Tiny, bundled, no application business logic
  Window and lifecycle control only

Preload
  Tiny typed capability bridge

Renderer shell
  Tiny locally loaded UI
  Immediate cached projection
  Virtualized/fine-grained rendering

Native application daemon
  Rust or another compiled language
  Database ownership
  Filesystem watchers
  Search indexes
  Git operations
  Parsing
  Compression
  Networking and synchronization

Communication
  MessagePorts/local sockets
  Binary/typed payloads
  Streaming results
  Cancellation
  Backpressure
  Revision IDs rather than whole-state transfers
```

In this design, Electron is essentially the **GPU-accelerated interface and OS integration layer**, while the actual application engine is a persistent native process.

That is close to the maximum-performance version of Electron without abandoning Electron altogether.

# What I would do first

For an existing slow app, the highest-return sequence is:

1. Record the complete cold-start trace.
2. Add marks for first window, first paint, first interaction, and primary content.
3. Generate a startup import-cost report for main, preload, and renderer.
4. Cut the renderer down to a shell plus active route.
5. Remove all top-level service initialization.
6. Move database, indexing, parsing, and Git/filesystem workloads into utility processes.
7. Replace giant IPC state transfers with paginated or streamed queries.
8. Virtualize every potentially large view.
9. Profile rerenders and root-level subscriptions.
10. Test the packaged production build on a mediocre Windows machine.

The biggest breakthroughs are rarely microscopic CSS changes. They are usually architectural: **less startup code, less shared state, less serialization, fewer DOM nodes, and harder separation between UI and computation.**

[1]: https://www.electronjs.org/docs/latest/tutorial/performance "Performance | Electron"
[2]: https://www.electronjs.org/docs/latest/api/utility-process "utilityProcess | Electron"
[3]: https://www.electronjs.org/docs/latest/tutorial/process-model "Process Model | Electron"

---

## Applied in OpenAgents Desktop (measure-constantly log)

These principles are operationalized for `apps/openagents-desktop/` in
`docs/fable/2026-07-11-desktop-startup-speed-audit.md`, with a repeatable
harness (`apps/openagents-desktop/scripts/startup-bench.ts`, run via
`bun run --cwd apps/openagents-desktop startup-bench`) that reports the
milestone chain (process start → whenReady → window → first paint → interactive
→ capability-ready) as median + p95, plus JSON receipts under
`apps/openagents-desktop/benchmarks/startup/`.

Landed 2026-07-11, proven with a drift-controlled interleaved A/B:

- **Minify all build artifacts** (§2 above; `scripts/build.ts`): renderer
  3.56 → 2.20 MB, main 2.22 → 1.12 MB, preload 1.34 → 0.64 MB. First paint
  **−19 ms (−4.3%)**, interactive **−17 ms (−3.3%)**, capability-ready
  **−19.8 ms (−3.8%)** median.
- **Window-first startup** (§1 above) was tested and **reverted** — it landed
  within run noise here because the `appWhenReady → windowCreated` gap is the
  `BrowserWindow` constructor cost, not the (near-zero, fresh-`userData`)
  service init. Recorded so it is not blindly re-tried.

Next reducible chunks: shell-before-data renderer ordering, lazy main-process
service construction, and renderer bundle-split (see the audit §6/§9).
