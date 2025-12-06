# Plan: Unified Effuse Testing Framework

## Overview

Build a unified, Effect-native testing framework for Effuse that:
1. Is 100% Effect-native (services, errors, resources, schemas)
2. Uses Bun natively (no Node.js/Playwright)
3. Supports headed and headless browser testing via CDP
4. Tests widgets in isolation with Happy-DOM (fast) or real browser (accurate)
5. Tests WebSocket flows end-to-end
6. Allows JSON-based declarative test cases (LLM-writable)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Test Definition Layer                         │
│                                                                   │
│   TypeScript Tests          JSON Test Cases                       │
│   (Effect.gen)              (LLM-generated)                       │
└──────────────┬─────────────────────┬────────────────────────────┘
               │                     │
               v                     v
┌──────────────────────────────────────────────────────────────────┐
│                     TestRunner Service                            │
│  - Compiles JSON to Effect programs                               │
│  - Orchestrates test execution                                    │
│  - Reports results                                                │
└──────────────────────────────────────────────────────────────────┘
               │
               v
┌──────────────────────────────────────────────────────────────────┐
│              Unified Test Context (Effect Layer)                  │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐           │
│  │ TestBrowser │  │ TestHarness  │  │ TestSocket     │           │
│  │ (existing)  │  │ (existing)   │  │ (new)          │           │
│  └─────────────┘  └──────────────┘  └────────────────┘           │
└──────────────────────────────────────────────────────────────────┘
               │
      ┌────────┴────────┐
      v                 v
┌───────────────┐ ┌───────────────┐
│  Happy-DOM    │ │   CDP Layer   │
│  Layer        │ │  (real browser│
│  (~10ms/test) │ │   ~200ms/test)│
│  (existing)   │ │   (NEW)       │
└───────────────┘ └───────────────┘
```

---

## Part 1: JSON Test Case DSL

### Schema Design (Effect Schema)

```typescript
// src/effuse/testing/json/schema.ts
const TestStep = S.Union(
  // Message injection
  S.Struct({ op: S.Literal("inject"), message: S.Unknown }),
  S.Struct({ op: S.Literal("injectSequence"), messages: S.Array(S.Unknown), delayMs: S.optional(S.Number) }),

  // DOM assertions
  S.Struct({ op: S.Literal("assertText"), selector: S.String, text: S.String }),
  S.Struct({ op: S.Literal("assertVisible"), selector: S.String }),
  S.Struct({ op: S.Literal("assertHidden"), selector: S.String }),
  S.Struct({ op: S.Literal("assertCount"), selector: S.String, count: S.Number }),

  // DOM actions
  S.Struct({ op: S.Literal("click"), selector: S.String }),
  S.Struct({ op: S.Literal("type"), selector: S.String, text: S.String }),

  // Widget actions
  S.Struct({ op: S.Literal("emit"), event: S.Unknown }),
  S.Struct({ op: S.Literal("setState"), state: S.Unknown }),

  // Waiting
  S.Struct({ op: S.Literal("wait"), selector: S.String, timeout: S.optional(S.Number) }),
  S.Struct({ op: S.Literal("waitForText"), selector: S.String, text: S.String }),
  S.Struct({ op: S.Literal("waitForState"), path: S.String, value: S.Unknown }),

  // Control
  S.Struct({ op: S.Literal("sleep"), ms: S.Number }),
)

const TestCase = S.Struct({
  name: S.String,
  widget: S.String,
  initialState: S.optional(S.Unknown),
  timeout: S.optional(S.Number),
  steps: S.Array(TestStep),
  tags: S.optional(S.Array(S.String)),
})

const TestSuite = S.Struct({
  name: S.String,
  fixtures: S.optional(S.Record({ key: S.String, value: S.Unknown })),
  tests: S.Array(TestCase),
})
```

### Example JSON Test (LLM-friendly)

```json
{
  "name": "TB run shows progress",
  "widget": "TBOutputWidget",
  "steps": [
    { "op": "inject", "message": { "type": "tb_run_start", "runId": "run-1" } },
    { "op": "sleep", "ms": 10 },
    { "op": "inject", "message": { "type": "tb_task_output", "runId": "run-1", "text": "Working..." } },
    { "op": "assertText", "selector": ".tb-output", "text": "Working..." }
  ]
}
```

---

## Part 2: CDP Browser Layer (Replace Playwright)

### Bun-Native Chrome Control

```typescript
// src/effuse/testing/cdp/browser.ts
interface BrowserController {
  launch: (options?: { headless?: boolean; slowMo?: number }) => Effect<string, BrowserError>
  close: () => Effect<void>
}

// Launch Chrome with Bun.spawn
const launchBrowser = () => Effect.gen(function* () {
  const chromePath = yield* findChrome()
  const port = yield* findFreePort()

  const proc = Bun.spawn([chromePath,
    `--remote-debugging-port=${port}`,
    "--headless=new",
    "--no-first-run",
    "--disable-extensions",
  ], { stderr: "pipe" })

  // Parse WebSocket URL from stderr
  const wsEndpoint = yield* parseWSEndpoint(proc.stderr)
  return wsEndpoint
})
```

### CDP Client (Native WebSocket)

```typescript
// src/effuse/testing/cdp/client.ts
interface CDPClient {
  send: <T>(method: string, params?: object) => Effect<T, CDPError>
  on: (method: string) => Stream<CDPEvent>
}

// Uses Bun's native WebSocket, JSON-RPC correlation
```

### TestBrowser Implementation

The CDP layer implements the same `TestBrowser` interface as Happy-DOM:

```typescript
// src/effuse/testing/layers/cdp.ts
const browser: TestBrowser = {
  click: (selector) => CDP.querySelector(client, selector).pipe(
    Effect.flatMap(nodeId => CDP.click(client, nodeId))
  ),
  expectText: (selector, text) => Effect.gen(function* () {
    const content = yield* CDP.getTextContent(client, selector)
    if (!content.includes(text)) yield* Effect.fail(new TestError(...))
  }),
  // ... same interface as Happy-DOM
}
```

---

## Part 3: Enhanced WebSocket Testing

### TestSocket Service (NEW)

```typescript
// src/effuse/testing/socket.ts
interface TestSocket {
  // Existing (from TestHarness)
  inject: (msg: HudMessage) => Effect<void>
  injectSequence: (msgs: HudMessage[], delayMs?: number) => Effect<void>

  // NEW: Capture and assert on messages
  messages: Stream<HudMessage>
  waitForMessage: <T extends HudMessage["type"]>(type: T, timeout?: number) => Effect<Extract<HudMessage, { type: T }>>
  captureMessages: <A>(action: Effect<A>) => Effect<{ result: A; messages: HudMessage[] }>
  expectMessageSequence: (types: HudMessage["type"][]) => Effect<void>
}
```

---

## Part 4: Test Runner

### JSON → Effect Compiler

```typescript
// src/effuse/testing/json/compiler.ts
const compileStep = (step: TestStep, ctx: Context): Effect<void, TestError> => {
  switch (step.op) {
    case "inject": return ctx.harness.injectMessage(step.message)
    case "assertText": return ctx.browser.expectText(step.selector, step.text)
    case "click": return ctx.browser.click(step.selector)
    case "emit": return ctx.handle.emit(step.event)
    case "sleep": return Effect.sleep(step.ms)
    // ...
  }
}

const compileTestCase = (test: TestCase): Effect<void, TestError> =>
  Effect.gen(function* () {
    const handle = yield* harness.mount(getWidget(test.widget), { initialState: test.initialState })
    for (const step of test.steps) {
      yield* compileStep(step, { handle, browser, harness })
    }
  })
```

### Bun Test Integration

```typescript
// src/effuse/testing/json/bun-adapter.ts
export const registerJsonSuite = (suite: TestSuite) => {
  describe(suite.name, () => {
    for (const test of suite.tests) {
      test(test.name, async () => {
        await Effect.runPromise(
          Effect.scoped(
            compileTestCase(test).pipe(Effect.provide(makeHappyDomLayer()))
          )
        )
      })
    }
  })
}
```

---

## File Structure

```
src/effuse/testing/
  index.ts                    # Public exports (update)
  errors.ts                   # Existing - add new error types
  browser.ts                  # Existing TestBrowser interface
  harness.ts                  # Existing TestHarness interface
  socket.ts                   # NEW: TestSocket service

  layers/
    happy-dom.ts              # Existing - enhance
    cdp.ts                    # NEW: CDP browser layer

  cdp/
    index.ts                  # NEW
    client.ts                 # NEW: CDP WebSocket client
    browser.ts                # NEW: Chrome process controller
    commands.ts               # NEW: CDP command wrappers

  json/
    index.ts                  # NEW
    schema.ts                 # NEW: Effect Schema definitions
    compiler.ts               # NEW: Step → Effect compiler
    runner.ts                 # NEW: Suite runner
    bun-adapter.ts            # NEW: bun:test integration
    widget-registry.ts        # NEW: Widget ID → Widget mapping
```

---

## Implementation Phases

| Phase | Tasks | Scope |
|-------|-------|-------|
| **1. JSON DSL** | Schema, compiler, runner, bun adapter | ~400 lines |
| **2. TestSocket** | Enhanced socket testing with Stream | ~150 lines |
| **3. CDP Core** | Client, browser controller, commands | ~500 lines |
| **4. CDP Layer** | TestBrowser implementation over CDP | ~300 lines |
| **5. Integration** | Widget registry, headed/headless modes | ~200 lines |
| **6. Tests + Docs** | Self-tests, documentation | ~300 lines |

**Total: ~1850 lines**

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HEADED` | `false` | Show browser window |
| `SLOWMO` | `0` | Delay between actions (ms) |
| `CHROME_PATH` | auto | Chrome executable path |
| `TEST_TIMEOUT` | `30000` | Default test timeout |

---

## Scripts

```json
{
  "test:effuse": "bun test src/effuse/",
  "test:effuse:headed": "HEADED=1 bun test src/effuse/",
  "test:effuse:slow": "HEADED=1 SLOWMO=500 bun test src/effuse/",
  "test:json": "bun src/effuse/testing/json/cli.ts run tests/*.json"
}
```

---

## Critical Files to Create

| File | Purpose | Priority |
|------|---------|----------|
| `src/effuse/testing/json/schema.ts` | Effect Schema for JSON tests | P0 |
| `src/effuse/testing/json/compiler.ts` | Step → Effect compiler | P0 |
| `src/effuse/testing/json/runner.ts` | Test execution | P0 |
| `src/effuse/testing/json/bun-adapter.ts` | bun:test integration | P0 |
| `src/effuse/testing/socket.ts` | Enhanced WebSocket testing | P1 |
| `src/effuse/testing/cdp/client.ts` | CDP WebSocket client | P1 |
| `src/effuse/testing/cdp/browser.ts` | Chrome process control | P1 |
| `src/effuse/testing/layers/cdp.ts` | CDP TestBrowser layer | P1 |

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/effuse/testing/index.ts` | Export new modules |
| `src/effuse/testing/errors.ts` | Add CDP/runner errors |
| `src/effuse/testing/layers/happy-dom.ts` | Integrate with new runner |
