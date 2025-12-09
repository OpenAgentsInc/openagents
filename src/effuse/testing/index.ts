/**
 * Effuse Testing Module
 *
 * Effect-native testing utilities for Effuse widgets.
 *
 * Two testing modes:
 * - **Happy-DOM** (default): Fast, in-process (~10ms/test). Good for TDD and CI.
 * - **webview-bun**: Real browser engine, same as production (~200-500ms/test).
 *
 * @example Happy-DOM (fast)
 * ```typescript
 * import { test, expect } from "bun:test"
 * import { Effect } from "effect"
 * import { makeHappyDomLayer, TestBrowserTag, TestHarnessTag } from "./effuse/testing"
 *
 * test("widget responds to events", async () => {
 *   await Effect.runPromise(
 *     Effect.scoped(
 *       Effect.gen(function* () {
 *         const { layer } = yield* makeHappyDomLayer()
 *         const harness = yield* TestHarnessTag
 *         const browser = yield* TestBrowserTag
 *
 *         const handle = yield* harness.mount(MyWidget)
 *         yield* handle.emit({ type: "buttonClick" })
 *         yield* browser.expectText(".status", "clicked")
 *       }).pipe(Effect.provide(layer))
 *     )
 *   )
 * })
 * ```
 *
 * @example webview-bun (real browser)
 * ```typescript
 * import { test, expect } from "bun:test"
 * import { Effect } from "effect"
 * import { executeWebviewTest, generateTestHTML } from "./effuse/testing"
 *
 * test("widget in real browser", async () => {
 *   const html = generateTestHTML({
 *     widgetBundle: myWidgetCode,
 *     testSteps: `
 *       assert.exists('.widget');
 *       assert.text('.widget', 'Hello');
 *     `,
 *   })
 *
 *   const results = await Effect.runPromise(executeWebviewTest(html))
 *   expect(results[0].pass).toBe(true)
 * })
 * ```
 */

// Error types
export {
  TestError,
  WebviewTestError,
  type WaitOptions,
  type WebviewTestOptions,
} from "./errors.js"

// Service interfaces
export { type TestBrowser, TestBrowserTag } from "./browser.js"
export { type TestHarness, type ComponentHandle, TestHarnessTag } from "./harness.js"

// Layer implementations
export { makeHappyDomLayer, type HappyDomLayerResult } from "./layers/happy-dom.js"
export {
  WebviewTestLayer,
  runWebviewTest,
  setWidgetBundle,
  setStyles,
} from "./layers/webview.js"

// Webview test utilities
export {
  executeWebviewTest,
  executeAndAssert,
  type TestResult,
} from "./webview/index.js"
export { generateTestHTML, generateSimpleTest, type TestHTMLOptions } from "./webview/index.js"
