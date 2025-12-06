# Plan: Unified Effuse Testing Framework v2

## Core Principle

**Test as close as possible to the actual app.**

The app uses `webview-bun` (WebKit on macOS, Edge WebView2 on Windows, WebKitGTK on Linux). The testing framework MUST use the same engine, not Chrome/CDP.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Test Definition Layer                            │
│                                                                      │
│   TypeScript Tests              JSON Test Cases                      │
│   (Effect.gen)                  (LLM-writable)                       │
└──────────────┬───────────────────────┬──────────────────────────────┘
               │                       │
               v                       v
┌──────────────────────────────────────────────────────────────────────┐
│                       TestRunner Service                              │
│  - Compiles JSON to test scripts                                      │
│  - Manages test execution (in-process or subprocess)                  │
│  - Reports results                                                    │
└──────────────────────────────────────────────────────────────────────┘
               │
      ┌────────┴─────────┐
      v                  v
┌───────────────┐  ┌──────────────────────────────────────────────────┐
│  Happy-DOM    │  │              webview-bun Layer                    │
│  Layer        │  │                                                   │
│  (in-process) │  │  ┌──────────────────────────────────────────┐    │
│  (~10ms/test) │  │  │         Test Subprocess                  │    │
│               │  │  │                                          │    │
│  For:         │  │  │  1. Bun spawns test subprocess           │    │
│  - Fast TDD   │  │  │  2. Subprocess creates Webview           │    │
│  - CI/quick   │  │  │  3. Load widget + test HTML              │    │
│  - Unit tests │  │  │  4. Tests run IN the webview             │    │
│               │  │  │  5. Results via webview.bind()           │    │
│               │  │  │  6. window.close() terminates            │    │
│               │  │  │  7. Subprocess exits with JSON           │    │
│               │  │  │                                          │    │
│               │  │  └──────────────────────────────────────────┘    │
│               │  │                                                   │
│               │  │  For:                                             │
│               │  │  - Real browser behavior                          │
│               │  │  - Visual accuracy                                │
│               │  │  - WebSocket integration                          │
│               │  │  - Pre-release validation                         │
│               │  │                                                   │
│               │  │  (~200-500ms/test)                                │
└───────────────┘  └──────────────────────────────────────────────────┘
```

---

## Why webview-bun, Not Chrome/CDP

| Aspect | Chrome/CDP | webview-bun |
|--------|-----------|-------------|
| **Engine** | Blink/V8 | WebKit/JavaScriptCore (macOS) |
| **Same as app?** | NO | YES |
| **Rendering** | Different | Identical |
| **CSS behavior** | Different | Identical |
| **JS engine** | Different | Identical |
| **WebSocket** | Different stack | Same stack |
| **Dependencies** | Requires Chrome | Uses system webview |

Testing in Chrome would give false confidence. Bugs specific to WebKit would be missed. The testing framework MUST use the same engine as production.

---

## Part 1: webview-bun Test Layer

### The Blocking Problem

`webview.run()` blocks Bun's event loop. This is the same challenge the desktop app faces (solved with Workers).

**Solution**: Run each test in a subprocess.

```
┌─────────────────────┐         ┌─────────────────────────────────┐
│   Test Runner       │         │      Test Subprocess            │
│   (Parent Process)  │         │                                 │
│                     │ spawn   │  ┌───────────────────────────┐  │
│  1. Compile test    │────────▶│  │    webview-bun            │  │
│  2. Spawn subprocess│         │  │                           │  │
│  3. Parse stdout    │◀────────│  │  - setHTML(testHTML)      │  │
│  4. Return results  │  JSON   │  │  - bind("reportResults")  │  │
│                     │         │  │  - run() [blocks]         │  │
│                     │         │  │  - window.close() exits   │  │
│                     │         │  └───────────────────────────┘  │
└─────────────────────┘         └─────────────────────────────────┘
```

### Test HTML Structure

```html
<!DOCTYPE html>
<html>
<head>
  <style>/* Widget styles */</style>
</head>
<body>
  <div id="effuse-test-root"></div>

  <!-- Compiled widget bundle (IIFE) -->
  <script>${widgetBundle}</script>

  <!-- Test harness -->
  <script>
    const __results = [];

    // Assertion helpers
    const assert = {
      eq: (a, b, msg) => { if (a !== b) throw new Error(msg || `${a} !== ${b}`); },
      contains: (s, sub) => { if (!s.includes(sub)) throw new Error(`"${s}" !contains "${sub}"`); },
      visible: (sel) => {
        const el = document.querySelector(sel);
        if (!el) throw new Error(`${sel} not found`);
        const s = getComputedStyle(el);
        if (s.display === 'none' || s.visibility === 'hidden')
          throw new Error(`${sel} not visible`);
      },
      hidden: (sel) => {
        const el = document.querySelector(sel);
        if (el) {
          const s = getComputedStyle(el);
          if (s.display !== 'none' && s.visibility !== 'hidden')
            throw new Error(`${sel} is visible`);
        }
      },
      count: (sel, n) => {
        const els = document.querySelectorAll(sel);
        if (els.length !== n) throw new Error(`${sel}: ${els.length} !== ${n}`);
      },
    };

    const $ = (sel) => document.querySelector(sel);
    const $$ = (sel) => [...document.querySelectorAll(sel)];
    const sleep = (ms) => new Promise(r => setTimeout(r, ms));

    async function runTests() {
      try {
        // === COMPILED TEST STEPS ===
        ${compiledTestSteps}
        // === END TEST STEPS ===

        __results.push({ pass: true });
      } catch (e) {
        __results.push({ pass: false, error: e.message, stack: e.stack });
      }

      // Report to Bun and close
      window.reportResults(JSON.stringify(__results));
      setTimeout(() => window.close(), 50);
    }

    document.readyState === 'loading'
      ? document.addEventListener('DOMContentLoaded', runTests)
      : runTests();
  </script>
</body>
</html>
```

### Subprocess Runner

```typescript
// src/effuse/testing/webview/runner.ts
import { Webview } from "webview-bun";

const testHTML = Bun.env.EFFUSE_TEST_HTML!;
const headed = Bun.env.EFFUSE_HEADED === "1";
const timeout = parseInt(Bun.env.EFFUSE_TIMEOUT || "30000");

let results = "[]";

const webview = new Webview(headed); // devtools if headed

webview.bind("reportResults", (r: string) => {
  results = r;
});

webview.setHTML(testHTML);

// Failsafe timeout
const timer = setTimeout(() => {
  console.log(JSON.stringify([{ pass: false, error: "Test timeout" }]));
  process.exit(1);
}, timeout);

webview.run();

clearTimeout(timer);
console.log(results);
```

### Test Execution from Effect

```typescript
// src/effuse/testing/webview/execute.ts
export const executeWebviewTest = (
  testHTML: string,
  options?: { headed?: boolean; timeout?: number }
): Effect.Effect<TestResult[], TestError> =>
  Effect.gen(function* () {
    const proc = Bun.spawn(
      ["bun", "run", resolve(__dirname, "runner.ts")],
      {
        stdout: "pipe",
        stderr: "pipe",
        env: {
          ...process.env,
          EFFUSE_TEST_HTML: testHTML,
          EFFUSE_HEADED: options?.headed ? "1" : "0",
          EFFUSE_TIMEOUT: String(options?.timeout ?? 30000),
        },
      }
    );

    const [stdout, stderr] = yield* Effect.all([
      Effect.promise(() => new Response(proc.stdout).text()),
      Effect.promise(() => new Response(proc.stderr).text()),
    ]);

    const exitCode = yield* Effect.promise(() => proc.exited);

    if (exitCode !== 0) {
      return yield* Effect.fail(
        new TestError({
          reason: "action_failed",
          message: `Webview test failed: ${stderr}`,
        })
      );
    }

    try {
      return JSON.parse(stdout) as TestResult[];
    } catch {
      return yield* Effect.fail(
        new TestError({
          reason: "action_failed",
          message: `Invalid test output: ${stdout}`,
        })
      );
    }
  });
```

---

## Part 2: WebSocket Integration Testing

Test the full stack: widget → WebSocket → server → widget update.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Test Subprocess                                   │
│                                                                      │
│  ┌─────────────────┐        WebSocket        ┌──────────────────┐  │
│  │    Webview      │◀──────────────────────▶│   Test Server    │  │
│  │                 │   ws://localhost:PORT   │   (in Worker)    │  │
│  │  Widget under   │                         │                  │  │
│  │  test renders   │                         │  HTTP inject API │  │
│  │  and reacts to  │                         │  for test steps  │  │
│  │  WS messages    │                         │                  │  │
│  └─────────────────┘                         └──────────────────┘  │
│                                                      ▲              │
│                                                      │ HTTP         │
│                                                      │              │
│  ┌───────────────────────────────────────────────────┴────────────┐ │
│  │                    Test Orchestrator                           │ │
│  │                                                                │ │
│  │  1. Start test server in Worker                                │ │
│  │  2. Generate test HTML with WS connection                      │ │
│  │  3. Create webview, navigate to test server                    │ │
│  │  4. Inject messages via HTTP API                               │ │
│  │  5. Assert on DOM via test harness                             │ │
│  │  6. Collect results                                            │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Test Server (Worker)

```typescript
// src/effuse/testing/webview/test-server-worker.ts
const PORT = parseInt(Bun.env.TEST_PORT || "0");

const server = Bun.serve({
  port: PORT,

  async fetch(req, server) {
    const url = new URL(req.url);

    // WebSocket upgrade
    if (url.pathname === "/ws") {
      if (server.upgrade(req)) return;
      return new Response("Upgrade failed", { status: 400 });
    }

    // Inject message API
    if (url.pathname === "/inject" && req.method === "POST") {
      const msg = await req.json();
      // Broadcast to connected clients
      server.publish("test", JSON.stringify(msg));
      return Response.json({ ok: true });
    }

    // Serve test HTML
    if (url.pathname === "/") {
      return new Response(Bun.env.TEST_HTML, {
        headers: { "Content-Type": "text/html" },
      });
    }

    return new Response("Not found", { status: 404 });
  },

  websocket: {
    open(ws) {
      ws.subscribe("test");
    },
    message(ws, msg) {
      // Record messages from widget if needed
    },
  },
});

// Report port to parent
postMessage({ type: "ready", port: server.port });
```

### Integration Test Flow

```typescript
// Effect program for integration test
const runIntegrationTest = (
  widgetHTML: string,
  steps: IntegrationStep[]
): Effect.Effect<TestResult[], TestError> =>
  Effect.gen(function* () {
    // Start test server
    const worker = new Worker("./test-server-worker.ts");
    const port = yield* waitForWorkerReady(worker);

    // Generate test HTML with WS connection
    const testHTML = generateIntegrationTestHTML(widgetHTML, port);

    // Execute in webview subprocess
    const results = yield* executeWebviewTest(testHTML);

    worker.terminate();
    return results;
  });
```

---

## Part 3: JSON Test DSL

### Schema (Effect Schema)

```typescript
// src/effuse/testing/json/schema.ts
import * as S from "@effect/schema/Schema";

const InjectStep = S.Struct({
  op: S.Literal("inject"),
  message: S.Unknown,
});

const InjectSequenceStep = S.Struct({
  op: S.Literal("injectSequence"),
  messages: S.Array(S.Unknown),
  delayMs: S.optional(S.Number),
});

const AssertTextStep = S.Struct({
  op: S.Literal("assertText"),
  selector: S.String,
  text: S.String,
});

const AssertVisibleStep = S.Struct({
  op: S.Literal("assertVisible"),
  selector: S.String,
});

const AssertHiddenStep = S.Struct({
  op: S.Literal("assertHidden"),
  selector: S.String,
});

const AssertCountStep = S.Struct({
  op: S.Literal("assertCount"),
  selector: S.String,
  count: S.Number,
});

const ClickStep = S.Struct({
  op: S.Literal("click"),
  selector: S.String,
});

const TypeStep = S.Struct({
  op: S.Literal("type"),
  selector: S.String,
  text: S.String,
});

const WaitStep = S.Struct({
  op: S.Literal("wait"),
  selector: S.String,
  timeout: S.optional(S.Number),
});

const WaitForTextStep = S.Struct({
  op: S.Literal("waitForText"),
  selector: S.String,
  text: S.String,
  timeout: S.optional(S.Number),
});

const SleepStep = S.Struct({
  op: S.Literal("sleep"),
  ms: S.Number,
});

export const TestStep = S.Union(
  InjectStep,
  InjectSequenceStep,
  AssertTextStep,
  AssertVisibleStep,
  AssertHiddenStep,
  AssertCountStep,
  ClickStep,
  TypeStep,
  WaitStep,
  WaitForTextStep,
  SleepStep
);

export const TestCase = S.Struct({
  name: S.String,
  widget: S.String,
  initialState: S.optional(S.Unknown),
  timeout: S.optional(S.Number),
  steps: S.Array(TestStep),
  tags: S.optional(S.Array(S.String)),
});

export const TestSuite = S.Struct({
  name: S.String,
  description: S.optional(S.String),
  fixtures: S.optional(S.Record({ key: S.String, value: S.Unknown })),
  tests: S.Array(TestCase),
});
```

### Step Compilation (JSON → JavaScript)

```typescript
// src/effuse/testing/json/compiler.ts
type CompiledStep = string; // JavaScript code

const compileStep = (step: TestStep): CompiledStep => {
  switch (step.op) {
    case "inject":
      return `await injectMessage(${JSON.stringify(step.message)});`;

    case "injectSequence":
      return `
        for (const msg of ${JSON.stringify(step.messages)}) {
          await injectMessage(msg);
          ${step.delayMs ? `await sleep(${step.delayMs});` : ""}
        }
      `;

    case "assertText":
      return `assert.contains($('${step.selector}').textContent, '${step.text}');`;

    case "assertVisible":
      return `assert.visible('${step.selector}');`;

    case "assertHidden":
      return `assert.hidden('${step.selector}');`;

    case "assertCount":
      return `assert.count('${step.selector}', ${step.count});`;

    case "click":
      return `$('${step.selector}').click();`;

    case "type":
      return `
        const input = $('${step.selector}');
        input.value = '${step.text}';
        input.dispatchEvent(new Event('input', { bubbles: true }));
      `;

    case "wait":
      return `await waitFor('${step.selector}', ${step.timeout ?? 5000});`;

    case "waitForText":
      return `await waitForText('${step.selector}', '${step.text}', ${step.timeout ?? 5000});`;

    case "sleep":
      return `await sleep(${step.ms});`;
  }
};

export const compileTestCase = (test: TestCase): string =>
  test.steps.map(compileStep).join("\n");
```

### Example JSON Test

```json
{
  "name": "TB Output Widget - Run Progress",
  "widget": "TBOutputWidget",
  "steps": [
    {
      "op": "inject",
      "message": {
        "type": "tb_run_start",
        "runId": "test-001",
        "suiteName": "example",
        "totalTasks": 3
      }
    },
    { "op": "assertVisible", "selector": ".tb-output" },
    { "op": "assertText", "selector": ".run-status", "text": "Running" },
    { "op": "sleep", "ms": 50 },
    {
      "op": "inject",
      "message": {
        "type": "tb_task_start",
        "runId": "test-001",
        "taskId": "task-1",
        "taskName": "First Task"
      }
    },
    { "op": "assertText", "selector": ".current-task", "text": "First Task" },
    {
      "op": "inject",
      "message": {
        "type": "tb_task_complete",
        "runId": "test-001",
        "taskId": "task-1",
        "outcome": "success"
      }
    },
    { "op": "assertCount", "selector": ".task-success", "count": 1 }
  ]
}
```

---

## Part 4: Unified TestBrowser Interface

Both Happy-DOM and webview-bun layers implement the same interface:

```typescript
// src/effuse/testing/browser.ts (existing, unchanged)
export interface TestBrowser {
  readonly query: <T extends Element>(selector: string) => Effect<T, TestError>;
  readonly queryOption: <T extends Element>(selector: string) => Effect<T | null>;
  readonly queryAll: <T extends Element>(selector: string) => Effect<T[]>;
  readonly click: (selector: string) => Effect<void, TestError>;
  readonly type: (selector: string, text: string) => Effect<void, TestError>;
  readonly clear: (selector: string) => Effect<void, TestError>;
  readonly getText: (selector: string) => Effect<string, TestError>;
  readonly expectText: (selector: string, text: string) => Effect<void, TestError>;
  readonly expectVisible: (selector: string) => Effect<void, TestError>;
  readonly expectHidden: (selector: string) => Effect<void, TestError>;
  readonly expectCount: (selector: string, count: number) => Effect<void, TestError>;
  readonly waitFor: (selector: string, options?: WaitOptions) => Effect<Element, TestError>;
  readonly waitForText: (selector: string, text: string, options?: WaitOptions) => Effect<void, TestError>;
}
```

### webview-bun Implementation

The webview layer compiles Effect operations to JavaScript, executes in the webview, and parses results:

```typescript
// src/effuse/testing/layers/webview.ts
const webviewBrowser: TestBrowser = {
  expectText: (selector, text) =>
    Effect.gen(function* () {
      const js = `assert.contains($('${selector}').textContent, '${text}');`;
      yield* executeInWebview(js);
    }),

  click: (selector) =>
    Effect.gen(function* () {
      const js = `$('${selector}').click();`;
      yield* executeInWebview(js);
    }),

  // ... etc
};
```

---

## File Structure

```
src/effuse/testing/
  index.ts                    # Public exports
  errors.ts                   # TestError, WebviewTestError
  browser.ts                  # TestBrowser interface (existing)
  harness.ts                  # TestHarness interface (existing)

  layers/
    happy-dom.ts              # Fast in-process (existing)
    webview.ts                # Real webview-bun (NEW)

  webview/
    index.ts                  # Exports
    runner.ts                 # Subprocess entry point
    execute.ts                # Effect wrapper for spawning
    html-template.ts          # Test HTML generation
    test-server-worker.ts     # Worker for WS integration tests

  json/
    index.ts                  # Exports
    schema.ts                 # Effect Schema for JSON tests
    compiler.ts               # Step → JavaScript compiler
    runner.ts                 # Suite execution
    bun-adapter.ts            # bun:test integration
```

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `EFFUSE_TEST_MODE` | `happy-dom` | `happy-dom` or `webview` |
| `EFFUSE_HEADED` | `0` | Show webview window for debugging |
| `EFFUSE_TIMEOUT` | `30000` | Test timeout in milliseconds |
| `EFFUSE_SLOW_MO` | `0` | Delay between steps (ms) |

---

## Package.json Scripts

```json
{
  "test:effuse": "bun test src/effuse/",
  "test:effuse:webview": "EFFUSE_TEST_MODE=webview bun test src/effuse/",
  "test:effuse:headed": "EFFUSE_TEST_MODE=webview EFFUSE_HEADED=1 bun test src/effuse/",
  "test:effuse:slow": "EFFUSE_TEST_MODE=webview EFFUSE_HEADED=1 EFFUSE_SLOW_MO=500 bun test src/effuse/",
  "test:json": "bun src/effuse/testing/json/cli.ts run tests/*.json"
}
```

---

## Implementation Phases

| Phase | Tasks | Lines |
|-------|-------|-------|
| **1. webview runner** | runner.ts, execute.ts, html-template.ts | ~300 |
| **2. webview layer** | layers/webview.ts implementing TestBrowser | ~250 |
| **3. WS integration** | test-server-worker.ts, integration helpers | ~200 |
| **4. JSON DSL** | schema.ts, compiler.ts, runner.ts | ~350 |
| **5. bun:test adapter** | bun-adapter.ts, widget registry | ~150 |
| **6. Tests** | Self-tests for the framework | ~250 |

**Total: ~1500 lines**

---

## Why This Approach Works

1. **Same Engine**: Tests run in WebKit (macOS), Edge WebView2 (Windows), WebKitGTK (Linux) - exactly what production uses.

2. **Real Rendering**: CSS, fonts, layout - all real, not simulated.

3. **Real WebSocket**: Tests actual WebSocket connection, not mocks.

4. **Subprocess Isolation**: Each test is isolated, `webview.run()` blocking is solved.

5. **Graceful Cleanup**: `window.close()` terminates webview, subprocess exits cleanly.

6. **Headed Debugging**: Set `EFFUSE_HEADED=1` to see the actual window during test runs.

7. **LLM-Writable**: JSON DSL lets LLMs generate test cases that compile to real browser tests.

8. **Unified Interface**: Same `TestBrowser` interface for Happy-DOM and webview - switch with env var.

---

## Critical Insight

The desktop app already solved the "webview.run() blocks" problem using Workers. We apply the same pattern: orchestration in parent process/worker, blocking webview in subprocess. This is battle-tested architecture from the actual app.

---

## Cleanup: Remove CDP Implementation

The following files should be deleted (they were created for the Chrome/CDP approach which is NOT what we want):

- `src/effuse/testing/cdp/client.ts`
- `src/effuse/testing/cdp/browser.ts`
- `src/effuse/testing/cdp/` directory

The `CDPError` type in `errors.ts` should be renamed or replaced with `WebviewTestError`.
