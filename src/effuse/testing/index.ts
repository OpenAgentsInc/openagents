/**
 * Effuse Testing Module
 *
 * Effect-native testing utilities for Effuse widgets.
 *
 * @example
 * ```typescript
 * import { test, expect } from "bun:test"
 * import { Effect } from "effect"
 * import { makeHappyDomLayer, TestBrowserTag, TestHarnessTag } from "./effuse/testing"
 * import { MyWidget } from "./widgets/my-widget"
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
 */

// Error types
export { TestError, type WaitOptions } from "./errors.js"

// Service interfaces
export { type TestBrowser, TestBrowserTag } from "./browser.js"
export { type TestHarness, type WidgetHandle, TestHarnessTag } from "./harness.js"

// Layer implementations
export { makeHappyDomLayer, type HappyDomLayerResult } from "./layers/happy-dom.js"
