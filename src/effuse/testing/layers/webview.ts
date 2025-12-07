/**
 * Webview TestBrowser Layer
 *
 * Implements TestBrowser interface using real webview-bun.
 * Tests run in the same browser engine as production.
 *
 * Unlike Happy-DOM which runs in-process, webview tests run in subprocesses.
 * This layer accumulates test steps and executes them all at once when
 * the test completes.
 */

import { Context, Effect, Layer, Ref, Scope } from "effect"
import type { TestBrowser } from "../browser.js"
import { TestBrowserTag } from "../browser.js"
import { TestError, type WaitOptions } from "../errors.js"
import { executeAndAssert } from "../webview/execute.js"
import { generateTestHTML } from "../webview/html-template.js"

/**
 * Accumulated test step in JavaScript code form.
 */
interface TestStep {
  js: string
}

/**
 * Internal state for webview test session.
 */
interface WebviewTestState {
  steps: TestStep[]
  styles: string
  widgetBundle: string | undefined
}

/**
 * Tag for webview test state.
 */
class WebviewTestStateTag extends Context.Tag("effuse/testing/WebviewTestState")<
  WebviewTestStateTag,
  Ref.Ref<WebviewTestState>
>() {}

/**
 * Add a test step to be executed.
 */
const addStep = (js: string): Effect.Effect<void, never, WebviewTestStateTag> =>
  Effect.gen(function* () {
    const stateRef = yield* WebviewTestStateTag
    yield* Ref.update(stateRef, (state) => ({
      ...state,
      steps: [...state.steps, { js }],
    }))
  })

/**
 * Escape string for JavaScript.
 */
const escapeJS = (s: string): string =>
  s.replace(/\\/g, "\\\\").replace(/'/g, "\\'").replace(/\n/g, "\\n")

/**
 * Create TestBrowser implementation for webview.
 *
 * This implementation accumulates JavaScript code for each operation.
 * The actual execution happens in a subprocess when runWebviewTest is called.
 */
const makeWebviewBrowser = (): Effect.Effect<
  TestBrowser,
  never,
  WebviewTestStateTag
> =>
  Effect.gen(function* () {
    // Capture stateRef once at creation time
    const stateRef = yield* WebviewTestStateTag

    // Helper to add step without requiring service
    const addStepLocal = (js: string): Effect.Effect<void> =>
      Ref.update(stateRef, (state) => ({
        ...state,
        steps: [...state.steps, { js }],
      }))

    const browser: TestBrowser = {
      // ─────────────────────────────────────────────────────────────────
      // Queries
      // ─────────────────────────────────────────────────────────────────

      query: <T extends Element = Element>(selector: string) =>
        addStepLocal(`
          const __el = $('${escapeJS(selector)}');
          if (!__el) throw new Error('Element not found: ${escapeJS(selector)}');
        `).pipe(Effect.as(null as unknown as T)),

      queryOption: <T extends Element = Element>(selector: string) =>
        addStepLocal(`$('${escapeJS(selector)}');`).pipe(
          Effect.as(null as T | null)
        ),

      queryAll: <T extends Element = Element>(selector: string) =>
        addStepLocal(`$$('${escapeJS(selector)}');`).pipe(Effect.as([] as T[])),

      // ─────────────────────────────────────────────────────────────────
      // Actions
      // ─────────────────────────────────────────────────────────────────

      click: (selector: string) =>
        addStep(`click('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "action_failed",
                message: `Failed to click: ${selector}`,
              })
          )
        ),

      type: (selector: string, text: string) =>
        addStep(`type('${escapeJS(selector)}', '${escapeJS(text)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "action_failed",
                message: `Failed to type into: ${selector}`,
              })
          )
        ),

      clear: (selector: string) =>
        addStep(`clear('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "action_failed",
                message: `Failed to clear: ${selector}`,
              })
          )
        ),

      check: (selector: string, checked = true) =>
        addStep(`check('${escapeJS(selector)}', ${checked});`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "action_failed",
                message: `Failed to check: ${selector}`,
              })
          )
        ),

      dispatchEvent: (selector: string, event: Event | string) => {
        const eventCode =
          typeof event === "string"
            ? `new Event('${escapeJS(event)}', { bubbles: true })`
            : `new Event('${event.type}', { bubbles: true })`
        return addStep(`
          const __el = $('${escapeJS(selector)}');
          if (!__el) throw new Error('Element not found: ${escapeJS(selector)}');
          __el.dispatchEvent(${eventCode});
        `).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "action_failed",
                message: `Failed to dispatch event on: ${selector}`,
              })
          )
        )
      },

      // ─────────────────────────────────────────────────────────────────
      // Inspection
      // ─────────────────────────────────────────────────────────────────

      getInnerHTML: (selector: string) =>
        addStep(`$html('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "element_not_found",
                message: `Element not found: ${selector}`,
              })
          ),
          Effect.as("")
        ),

      getText: (selector: string) =>
        addStep(`$text('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "element_not_found",
                message: `Element not found: ${selector}`,
              })
          ),
          Effect.as("")
        ),

      getAttribute: (selector: string, attribute: string) =>
        addStep(`
          const __el = $('${escapeJS(selector)}');
          if (__el) __el.getAttribute('${escapeJS(attribute)}');
        `).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "element_not_found",
                message: `Element not found: ${selector}`,
              })
          ),
          Effect.as(null as string | null)
        ),

      isVisible: (selector: string) =>
        addStep(`
          const __el = $('${escapeJS(selector)}');
          if (__el) {
            const __style = getComputedStyle(__el);
            __style.display !== 'none' && __style.visibility !== 'hidden';
          }
        `).pipe(Effect.as(true)),

      exists: (selector: string) =>
        addStep(`!!$('${escapeJS(selector)}');`).pipe(Effect.as(true)),

      // ─────────────────────────────────────────────────────────────────
      // Assertions
      // ─────────────────────────────────────────────────────────────────

      expectText: (selector: string, text: string) =>
        addStep(`assert.text('${escapeJS(selector)}', '${escapeJS(text)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to contain "${text}"`,
              })
          )
        ),

      expectVisible: (selector: string) =>
        addStep(`assert.visible('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to be visible`,
              })
          )
        ),

      expectHidden: (selector: string) =>
        addStep(`assert.hidden('${escapeJS(selector)}');`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" to be hidden`,
              })
          )
        ),

      expectCount: (selector: string, count: number) =>
        addStep(`assert.count('${escapeJS(selector)}', ${count});`).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "assertion_failed",
                message: `Expected ${count} elements matching "${selector}"`,
              })
          )
        ),

      expectAttribute: (selector: string, attribute: string, value: string) =>
        addStep(
          `assert.attr('${escapeJS(selector)}', '${escapeJS(attribute)}', '${escapeJS(value)}');`
        ).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "assertion_failed",
                message: `Expected "${selector}" attribute "${attribute}" to be "${value}"`,
              })
          )
        ),

      // ─────────────────────────────────────────────────────────────────
      // Waiting
      // ─────────────────────────────────────────────────────────────────

      waitFor: (selector: string, options?: WaitOptions) =>
        addStep(
          `await waitFor('${escapeJS(selector)}', ${options?.timeout ?? 5000});`
        ).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "timeout",
                message: `Timeout waiting for "${selector}"`,
              })
          ),
          Effect.as(null as unknown as Element)
        ),

      waitForHidden: (selector: string, options?: WaitOptions) =>
        addStep(
          `await waitForHidden('${escapeJS(selector)}', ${options?.timeout ?? 5000});`
        ).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "timeout",
                message: `Timeout waiting for "${selector}" to be hidden`,
              })
          )
        ),

      waitForText: (selector: string, text: string, options?: WaitOptions) =>
        addStep(
          `await waitForText('${escapeJS(selector)}', '${escapeJS(text)}', ${options?.timeout ?? 5000});`
        ).pipe(
          Effect.mapError(
            () =>
              new TestError({
                reason: "timeout",
                message: `Timeout waiting for "${text}" in "${selector}"`,
              })
          )
        ),
    }

    return browser
  })

/**
 * Execute accumulated test steps in a real webview.
 */
export const runWebviewTest = (): Effect.Effect<
  void,
  TestError,
  WebviewTestStateTag
> =>
  Effect.gen(function* () {
    const stateRef = yield* WebviewTestStateTag
    const state = yield* Ref.get(stateRef)

    if (state.steps.length === 0) {
      return // No steps to run
    }

    // Compile steps into JavaScript
    const testSteps = state.steps.map((s) => s.js).join("\n")

    // Generate test HTML
    const html = generateTestHTML({
      ...(state.widgetBundle && { widgetBundle: state.widgetBundle }),
      styles: state.styles,
      testSteps,
    })

    // Execute in webview subprocess
    yield* executeAndAssert(html).pipe(
      Effect.mapError(
        (e) =>
          new TestError({
            reason: "action_failed",
            message: e.message,
          })
      )
    )
  })

/**
 * Create a webview test layer.
 *
 * Usage:
 * ```typescript
 * const test = Effect.gen(function* () {
 *   const browser = yield* TestBrowserTag
 *   yield* browser.expectText('.title', 'Hello')
 *   yield* browser.click('.button')
 *   yield* runWebviewTest() // Execute all steps
 * })
 *
 * await Effect.runPromise(
 *   test.pipe(Effect.provide(WebviewTestLayer))
 * )
 * ```
 */
export const WebviewTestLayer: Layer.Layer<
  TestBrowserTag | WebviewTestStateTag,
  never,
  Scope.Scope
> = Layer.effect(
  TestBrowserTag,
  Effect.gen(function* () {
    const stateRef = yield* Ref.make<WebviewTestState>({
      steps: [],
      styles: "",
      widgetBundle: undefined,
    })

    // Provide state to browser implementation
    const browser = yield* makeWebviewBrowser().pipe(
      Effect.provideService(WebviewTestStateTag, stateRef)
    )

    return browser
  })
).pipe(
  Layer.provideMerge(
    Layer.effect(
      WebviewTestStateTag,
      Ref.make<WebviewTestState>({
        steps: [],
        styles: "",
        widgetBundle: undefined,
      })
    )
  )
)

/**
 * Set the widget bundle code for the test.
 */
export const setWidgetBundle = (
  bundle: string
): Effect.Effect<void, never, WebviewTestStateTag> =>
  Effect.gen(function* () {
    const stateRef = yield* WebviewTestStateTag
    yield* Ref.update(stateRef, (state) => ({
      ...state,
      widgetBundle: bundle,
    }))
  })

/**
 * Set CSS styles for the test.
 */
export const setStyles = (
  styles: string
): Effect.Effect<void, never, WebviewTestStateTag> =>
  Effect.gen(function* () {
    const stateRef = yield* WebviewTestStateTag
    yield* Ref.update(stateRef, (state) => ({
      ...state,
      styles,
    }))
  })
