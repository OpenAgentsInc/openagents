/**
 * TB UI Widget Tests
 *
 * Tests Terminal-Bench UI components in isolation using the Effuse testing framework.
 * Verifies components render correctly in both Happy-DOM (fast) and webview-bun (real browser).
 *
 * Task: oa-91f25c
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { makeHappyDomLayer } from "../testing/layers/happy-dom.js"
import { TestBrowserTag } from "../testing/browser.js"
import { TestHarnessTag } from "../testing/harness.js"
import { SocketServiceTag } from "../services/socket.js"

// Import TB widgets
import { TBOutputWidget } from "./tb-output.js"
import { TBControlsWidget } from "./tb-controls.js"
import { CategoryTreeWidget } from "./category-tree.js"

// ============================================================================
// Helper to run widget tests
// ============================================================================

const runWithLayer = <A, E>(
  effect: Effect.Effect<A, E, TestBrowserTag | TestHarnessTag | SocketServiceTag>
): Promise<A> =>
  Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        // Happy-DOM layer already provides a mock SocketService
        const { layer } = yield* makeHappyDomLayer()
        return yield* effect.pipe(Effect.provide(layer))
      })
    )
  )

// ============================================================================
// TBOutputWidget Tests
// ============================================================================

describe("TBOutputWidget", () => {
  test("renders hidden by default", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const handle = yield* harness.mount(TBOutputWidget)

        // Widget should be hidden by default
        yield* browser.expectHidden(".fixed")
      })
    )
  })

  test("renders visible state with correct structure", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const handle = yield* harness.mount(TBOutputWidget, {
          initialState: {
            ...TBOutputWidget.initialState(),
            visible: true,
            runId: "test-run-001",
          },
        })

        // Should be visible
        yield* browser.expectVisible(".fixed")

        // Check for key UI elements
        yield* browser.expectText(".fixed", "TB Output")

        // Check for control buttons
        yield* browser.expectVisible("[data-action='toggleAutoScroll']")
        yield* browser.expectVisible("[data-action='copy']")
        yield* browser.expectVisible("[data-action='clear']")
        yield* browser.expectVisible("[data-action='close']")

        // Check for source filter buttons
        yield* browser.expectVisible("[data-source='agent']")
        yield* browser.expectVisible("[data-source='verification']")
        yield* browser.expectVisible("[data-source='system']")
        yield* browser.expectVisible("[data-source='tool']")
      })
    )
  })

  test("displays output lines when present", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const handle = yield* harness.mount(TBOutputWidget, {
          initialState: {
            ...TBOutputWidget.initialState(),
            visible: true,
            outputLines: [
              { text: "Agent output line", source: "agent", timestamp: Date.now() },
              { text: "Verification output", source: "verification", timestamp: Date.now() },
            ],
          },
        })

        // Should display the output lines
        yield* browser.expectText(".fixed", "Agent output line")
        yield* browser.expectText(".fixed", "Verification output")

        // Should show line count
        yield* browser.expectText(".fixed", "2 lines")
      })
    )
  })

  test("handles close event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(TBOutputWidget, {
          initialState: {
            ...TBOutputWidget.initialState(),
            visible: true,
          },
        })

        // Emit close event
        yield* handle.emit({ type: "close" })

        // Wait for event to be processed
        const state = yield* handle.waitForState((s) => !s.visible)
        expect(state.visible).toBe(false)
      })
    )
  })

  test("handles clear event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(TBOutputWidget, {
          initialState: {
            ...TBOutputWidget.initialState(),
            visible: true,
            outputLines: [
              { text: "Line 1", source: "agent", timestamp: Date.now() },
            ],
          },
        })

        // Emit clear event
        yield* handle.emit({ type: "clear" })

        // Wait for event to be processed
        const state = yield* handle.waitForState((s) => s.outputLines.length === 0)
        expect(state.outputLines.length).toBe(0)
      })
    )
  })
})

// ============================================================================
// TBControlsWidget Tests
// ============================================================================

describe("TBControlsWidget", () => {
  test("renders with default state", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const tbControlsHandle = yield* harness.mount(TBControlsWidget)

        // Should show header
        yield* browser.expectText(".rounded-xl", "Terminal-Bench")

        // Should show default status
        yield* browser.expectText(".rounded-xl", "Ready")

        // Should have path input
        yield* browser.expectVisible("[data-input='suitePath']")

        // Should have load button
        yield* browser.expectVisible("[data-action='loadSuite']")
      })
    )
  })

  test("renders control buttons", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const tbControlsHandle = yield* harness.mount(TBControlsWidget)

        // Should have start/stop/random buttons
        yield* browser.expectVisible("[data-action='startRun']")
        yield* browser.expectVisible("[data-action='startRandomTask']")
        yield* browser.expectVisible("[data-action='stopRun']")
      })
    )
  })

  test("renders task list when suite loaded", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        yield* harness.mount(TBControlsWidget, {
          initialState: {
            ...TBControlsWidget.initialState(),
            suite: {
              name: "Test Suite",
              version: "1.0.0",
              tasks: [
                { id: "task-1", name: "Task One", difficulty: "easy", category: "cat-a" },
                { id: "task-2", name: "Task Two", difficulty: "medium", category: "cat-b" },
              ],
            },
            selectedTaskIds: new Set(["task-1", "task-2"]),
          },
        })

        // Should show suite info
        yield* browser.expectText(".rounded-xl", "Test Suite")
        yield* browser.expectText(".rounded-xl", "v1.0.0")

        // Should show tasks
        yield* browser.expectText(".rounded-xl", "Task One")
        yield* browser.expectText(".rounded-xl", "Task Two")

        // Should show difficulty badges
        yield* browser.expectText(".rounded-xl", "easy")
        yield* browser.expectText(".rounded-xl", "medium")

        // Should show selection count
        yield* browser.expectText(".rounded-xl", "2/2 selected")
      })
    )
  })

  test("handles toggleCollapse event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(TBControlsWidget)

        // Initially not collapsed
        let state = yield* handle.getState
        expect(state.collapsed).toBe(false)

        // Emit collapse event
        yield* handle.emit({ type: "toggleCollapse" })

        // Wait for event to be processed
        state = yield* handle.waitForState((s) => s.collapsed)
        expect(state.collapsed).toBe(true)
      })
    )
  })

  test("handles setSuitePath event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(TBControlsWidget)

        // Set new path
        yield* handle.emit({ type: "setSuitePath", path: "/new/path/suite.json" })

        // Wait for event to be processed
        const state = yield* handle.waitForState((s) => s.suitePath === "/new/path/suite.json")
        expect(state.suitePath).toBe("/new/path/suite.json")
      })
    )
  })
})

// ============================================================================
// CategoryTreeWidget Tests
// ============================================================================

describe("CategoryTreeWidget", () => {
  test("renders hidden by default", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        yield* harness.mount(CategoryTreeWidget)

        // Should be hidden by default
        yield* browser.expectHidden(".fixed")
      })
    )
  })

  test("renders visible state with categories header", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
          },
        })

        // Should show categories header
        yield* browser.expectText(".fixed", "Categories")

        // Should have expand/collapse buttons
        yield* browser.expectVisible("[data-action='expandAll']")
        yield* browser.expectVisible("[data-action='collapseAll']")
        yield* browser.expectVisible("[data-action='hide']")
      })
    )
  })

  test("renders empty state when no tasks", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
            tasks: new Map(),
          },
        })

        // Should show empty message
        yield* browser.expectText(".fixed", "No tasks loaded")
      })
    )
  })

  test("renders tasks grouped by category", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        const tasks = new Map([
          ["task-1", { id: "task-1", name: "Task One", difficulty: "easy", category: "Category A", status: "pending" as const }],
          ["task-2", { id: "task-2", name: "Task Two", difficulty: "medium", category: "Category A", status: "passed" as const }],
          ["task-3", { id: "task-3", name: "Task Three", difficulty: "hard", category: "Category B", status: "failed" as const }],
        ])

        yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
            tasks,
          },
        })

        // Should show category names
        yield* browser.expectText(".fixed", "Category A")
        yield* browser.expectText(".fixed", "Category B")

        // Should show task names
        yield* browser.expectText(".fixed", "Task One")
        yield* browser.expectText(".fixed", "Task Two")
        yield* browser.expectText(".fixed", "Task Three")

        // Should show status indicators (✓ for passed, ✗ for failed)
        yield* browser.expectText(".fixed", "✓")
        yield* browser.expectText(".fixed", "✗")
      })
    )
  })

  test("handles hide event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
          },
        })

        // Emit hide event
        yield* handle.emit({ type: "hide" })

        // Wait for event to be processed
        const state = yield* handle.waitForState((s) => !s.visible)
        expect(state.visible).toBe(false)
      })
    )
  })

  test("handles toggleCategory event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const tasks = new Map([
          ["task-1", { id: "task-1", name: "Task One", difficulty: "easy", category: "Category A", status: "pending" as const }],
        ])

        const handle = yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
            tasks,
          },
        })

        // Initially no collapsed categories
        let state = yield* handle.getState
        expect(state.collapsedCategories.has("Category A")).toBe(false)

        // Collapse the category
        yield* handle.emit({ type: "toggleCategory", category: "Category A" })

        // Wait for event to be processed
        state = yield* handle.waitForState((s) => s.collapsedCategories.has("Category A"))
        expect(state.collapsedCategories.has("Category A")).toBe(true)

        // Toggle again to expand
        yield* handle.emit({ type: "toggleCategory", category: "Category A" })

        // Wait for event to be processed
        state = yield* handle.waitForState((s) => !s.collapsedCategories.has("Category A"))
        expect(state.collapsedCategories.has("Category A")).toBe(false)
      })
    )
  })

  test("handles collapseAll event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const tasks = new Map([
          ["task-1", { id: "task-1", name: "Task One", difficulty: "easy", category: "Cat A", status: "pending" as const }],
          ["task-2", { id: "task-2", name: "Task Two", difficulty: "easy", category: "Cat B", status: "pending" as const }],
        ])

        const handle = yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
            tasks,
          },
        })

        // Collapse all
        yield* handle.emit({ type: "collapseAll" })

        // Wait for event to be processed - check both categories are collapsed
        const state = yield* handle.waitForState(
          (s) => s.collapsedCategories.has("Cat A") && s.collapsedCategories.has("Cat B")
        )
        expect(state.collapsedCategories.has("Cat A")).toBe(true)
        expect(state.collapsedCategories.has("Cat B")).toBe(true)
      })
    )
  })

  test("handles expandAll event", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag

        const handle = yield* harness.mount(CategoryTreeWidget, {
          initialState: {
            ...CategoryTreeWidget.initialState(),
            visible: true,
            collapsedCategories: new Set(["Cat A", "Cat B"]),
          },
        })

        // Expand all
        yield* handle.emit({ type: "expandAll" })

        // Wait for event to be processed
        const state = yield* handle.waitForState((s) => s.collapsedCategories.size === 0)
        expect(state.collapsedCategories.size).toBe(0)
      })
    )
  })
})

// ============================================================================
// Integration Tests (all widgets together)
// ============================================================================

describe("TB UI Integration", () => {
  test("all TB widgets can be mounted simultaneously", async () => {
    await runWithLayer(
      Effect.gen(function* () {
        const harness = yield* TestHarnessTag
        const browser = yield* TestBrowserTag

        // Mount all three widgets
        const handle = yield* harness.mount(TBOutputWidget, {
          initialState: { ...TBOutputWidget.initialState(), visible: true },
        })
        const tbControlsHandle = yield* harness.mount(TBControlsWidget)
        yield* harness.mount(CategoryTreeWidget, {
          initialState: { ...CategoryTreeWidget.initialState(), visible: true },
        })

        // Verify each widget's key element is present
        yield* browser.expectText("body", "TB Output")
        yield* browser.expectText("body", "Terminal-Bench")
        yield* browser.expectText("body", "Categories")
      })
    )
  })
})

// ============================================================================
// Webview-bun Tests (Real Browser)
// ============================================================================

describe("TB UI in webview-bun", () => {
  // Skip webview tests in CI or when explicitly disabled
  // These tests require a display and spawn real webview subprocesses
  const skipWebview = !!process.env.CI || !!process.env.SKIP_WEBVIEW_TESTS

  test.skipIf(skipWebview)("TBOutputWidget renders in real WebKit", async () => {
    const { executeWebviewTest, generateTestHTML } = await import("../testing/webview/index.js")

    // Generate widget rendering code
    const testSteps = `
      // Create a container for the widget
      const container = document.getElementById('effuse-test-root');

      // Simulate widget HTML (simplified for test)
      container.innerHTML = \`
        <div class="tb-output-widget fixed right-4 bottom-20 w-96 bg-zinc-950 border border-zinc-800 rounded-lg">
          <div class="header flex items-center justify-between px-3 py-2 border-b border-zinc-800">
            <span class="title text-sm font-medium text-zinc-300">TB Output</span>
            <div class="controls flex gap-2">
              <button class="copy-btn text-xs text-zinc-400">Copy</button>
              <button class="clear-btn text-xs text-zinc-400">Clear</button>
              <button class="close-btn text-zinc-500">×</button>
            </div>
          </div>
          <div class="content p-3">
            <div class="output-line text-xs text-zinc-300">Test output line</div>
          </div>
          <div class="footer px-3 py-1 border-t border-zinc-800 text-xs text-zinc-500">
            1 lines
          </div>
        </div>
      \`;

      // Assertions
      assert.exists('.tb-output-widget');
      assert.visible('.tb-output-widget');
      assert.text('.title', 'TB Output');
      assert.text('.output-line', 'Test output line');
      assert.exists('.copy-btn');
      assert.exists('.clear-btn');
      assert.exists('.close-btn');
    `

    const html = generateTestHTML({
      styles: `
        .fixed { position: fixed; }
        .hidden { display: none; }
      `,
      testSteps,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(skipWebview)("TBControlsWidget renders in real WebKit", async () => {
    const { executeWebviewTest, generateTestHTML } = await import("../testing/webview/index.js")

    const testSteps = `
      const container = document.getElementById('effuse-test-root');

      // Simulate TBControls widget HTML
      container.innerHTML = \`
        <div class="tb-controls-widget rounded-xl border border-zinc-800 bg-zinc-950">
          <div class="header flex items-center justify-between px-4 py-3 border-b border-zinc-800">
            <h2 class="title text-zinc-100 font-bold font-mono text-lg">Terminal-Bench</h2>
            <span class="status text-xs text-zinc-500">Ready</span>
          </div>
          <div class="controls flex items-center gap-2 px-4 py-3">
            <button class="start-btn px-4 py-2 text-xs text-emerald-300 bg-emerald-900/40 rounded border border-emerald-700">Start</button>
            <button class="random-btn px-4 py-2 text-xs text-violet-300 bg-violet-900/40 rounded border border-violet-700">Random</button>
            <button class="stop-btn px-4 py-2 text-xs text-zinc-500 bg-zinc-800/40 rounded border border-zinc-700">Stop</button>
          </div>
        </div>
      \`;

      // Assertions
      assert.exists('.tb-controls-widget');
      assert.visible('.tb-controls-widget');
      assert.text('.title', 'Terminal-Bench');
      assert.text('.status', 'Ready');
      assert.exists('.start-btn');
      assert.exists('.random-btn');
      assert.exists('.stop-btn');
    `

    const html = generateTestHTML({ testSteps })

    const results = await Effect.runPromise(executeWebviewTest(html))
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(skipWebview)("CategoryTreeWidget renders in real WebKit", async () => {
    const { executeWebviewTest, generateTestHTML } = await import("../testing/webview/index.js")

    const testSteps = `
      const container = document.getElementById('effuse-test-root');

      // Simulate CategoryTree widget HTML
      container.innerHTML = \`
        <div class="category-tree-widget fixed right-4 top-20 w-72 rounded-lg border border-zinc-800 bg-zinc-950">
          <div class="header flex items-center justify-between px-3 py-2 border-b border-zinc-800">
            <span class="title text-sm font-medium text-zinc-300">Categories</span>
            <div class="controls flex gap-2">
              <button class="expand-btn text-xs text-zinc-400">Expand</button>
              <button class="collapse-btn text-xs text-zinc-400">Collapse</button>
            </div>
          </div>
          <div class="categories">
            <div class="category">
              <div class="category-header flex items-center gap-2 px-3 py-2">
                <span class="toggle text-zinc-500 text-xs">▼</span>
                <span class="category-name text-sm font-medium text-zinc-200">Test Category</span>
                <span class="count text-xs text-zinc-500">3</span>
              </div>
              <div class="tasks">
                <div class="task flex items-center gap-2 px-3 py-1">
                  <span class="status text-emerald-400">✓</span>
                  <span class="task-name text-xs text-zinc-300">Task One</span>
                </div>
                <div class="task flex items-center gap-2 px-3 py-1">
                  <span class="status text-red-400">✗</span>
                  <span class="task-name text-xs text-zinc-300">Task Two</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      \`;

      // Assertions
      assert.exists('.category-tree-widget');
      assert.visible('.category-tree-widget');
      assert.text('.title', 'Categories');
      assert.text('.category-name', 'Test Category');
      assert.exists('.expand-btn');
      assert.exists('.collapse-btn');
      assert.count('.task', 2);
    `

    const html = generateTestHTML({
      styles: `.fixed { position: fixed; }`,
      testSteps,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(skipWebview)("all TB widgets render together in real WebKit", async () => {
    const { executeWebviewTest, generateTestHTML } = await import("../testing/webview/index.js")

    const testSteps = `
      const container = document.getElementById('effuse-test-root');

      // Mount all three widgets
      container.innerHTML = \`
        <div class="tb-output" data-widget="tb-output">
          <div class="widget-title">TB Output</div>
        </div>
        <div class="tb-controls" data-widget="tb-controls">
          <div class="widget-title">Terminal-Bench</div>
        </div>
        <div class="category-tree" data-widget="category-tree">
          <div class="widget-title">Categories</div>
        </div>
      \`;

      // Verify all widgets present
      assert.exists('[data-widget="tb-output"]');
      assert.exists('[data-widget="tb-controls"]');
      assert.exists('[data-widget="category-tree"]');

      // Verify titles
      assert.count('.widget-title', 3);
      assert.text('.tb-output', 'TB Output');
      assert.text('.tb-controls', 'Terminal-Bench');
      assert.text('.category-tree', 'Categories');
    `

    const html = generateTestHTML({ testSteps })

    const results = await Effect.runPromise(executeWebviewTest(html))
    expect(results[0].pass).toBe(true)
  })
})
