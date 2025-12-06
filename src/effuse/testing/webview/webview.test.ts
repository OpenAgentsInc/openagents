/**
 * Webview Testing Layer Tests
 *
 * Tests the webview-bun testing infrastructure.
 * These tests actually spawn webview subprocesses to verify the framework works.
 */

import { describe, test, expect } from "bun:test"
import { Effect } from "effect"
import { executeWebviewTest, executeAndAssert } from "./execute.js"
import { generateTestHTML, generateSimpleTest } from "./html-template.js"

describe("webview test infrastructure", () => {
  test("generateTestHTML creates valid HTML", () => {
    const html = generateTestHTML({
      testSteps: "assert.eq(1, 1);",
    })

    expect(html).toContain("<!DOCTYPE html>")
    expect(html).toContain("assert.eq(1, 1);")
    expect(html).toContain("__effuseTestResults")
    expect(html).toContain("reportResults")
  })

  test("generateSimpleTest creates minimal test HTML", () => {
    const html = generateSimpleTest("assert.truthy(true);")

    expect(html).toContain("assert.truthy(true);")
    expect(html).toContain("effuse-test-root")
  })

  test("HTML template includes assertion helpers", () => {
    const html = generateTestHTML({ testSteps: "" })

    // assert object contains these methods
    expect(html).toContain("eq: (a, b, msg)")
    expect(html).toContain("contains: (str, sub, msg)")
    expect(html).toContain("visible: (sel, msg)")
    expect(html).toContain("hidden: (sel, msg)")
    expect(html).toContain("count: (sel, expected, msg)")
    expect(html).toContain("text: (sel, expected, msg)")
  })

  test("HTML template includes DOM helpers", () => {
    const html = generateTestHTML({ testSteps: "" })

    expect(html).toContain("const $ = ")
    expect(html).toContain("const $$ = ")
    expect(html).toContain("const sleep = ")
    expect(html).toContain("const waitFor = ")
  })
})

describe("webview test execution", () => {
  // Skip actual webview tests in CI (no display)
  const isCI = !!process.env.CI

  test.skipIf(isCI)("executes simple passing test", async () => {
    const html = generateSimpleTest(`
      assert.eq(1 + 1, 2, 'Math works');
    `)

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(isCI)("executes simple failing test", async () => {
    const html = generateSimpleTest(`
      assert.eq(1, 2, 'This should fail');
    `)

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(false)
    expect(results[0].error).toContain("This should fail")
  })

  test.skipIf(isCI)("can interact with DOM", async () => {
    const html = generateTestHTML({
      testSteps: `
        // Create an element
        const div = document.createElement('div');
        div.id = 'test-div';
        div.textContent = 'Hello World';
        document.getElementById('effuse-test-root').appendChild(div);

        // Assert on it
        assert.exists('#test-div');
        assert.text('#test-div', 'Hello World');
      `,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(isCI)("visibility assertions work", async () => {
    const html = generateTestHTML({
      styles: `
        .hidden { display: none; }
        .visible { display: block; }
      `,
      testSteps: `
        const root = document.getElementById('effuse-test-root');

        const visible = document.createElement('div');
        visible.className = 'visible';
        visible.id = 'visible-el';
        visible.textContent = 'I am visible';
        root.appendChild(visible);

        const hidden = document.createElement('div');
        hidden.className = 'hidden';
        hidden.id = 'hidden-el';
        hidden.textContent = 'I am hidden';
        root.appendChild(hidden);

        assert.visible('#visible-el');
        assert.hidden('#hidden-el');
      `,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(isCI)("count assertions work", async () => {
    const html = generateTestHTML({
      testSteps: `
        const root = document.getElementById('effuse-test-root');

        for (let i = 0; i < 5; i++) {
          const item = document.createElement('div');
          item.className = 'list-item';
          root.appendChild(item);
        }

        assert.count('.list-item', 5);
      `,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(isCI)("async operations work", async () => {
    const html = generateTestHTML({
      testSteps: `
        const root = document.getElementById('effuse-test-root');

        // Create element after delay
        setTimeout(() => {
          const delayed = document.createElement('div');
          delayed.id = 'delayed-el';
          delayed.textContent = 'Appeared!';
          root.appendChild(delayed);
        }, 100);

        // Wait for it
        await waitFor('#delayed-el', 2000);
        assert.text('#delayed-el', 'Appeared!');
      `,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })

  test.skipIf(isCI)("executeAndAssert throws on failure", async () => {
    const html = generateSimpleTest(`
      assert.eq(1, 2, 'This fails');
    `)

    await expect(
      Effect.runPromise(executeAndAssert(html))
    ).rejects.toThrow()
  })
})

describe("webview test with widget bundle", () => {
  const isCI = !!process.env.CI

  test.skipIf(isCI)("can load and test widget code", async () => {
    const widgetBundle = `
      // Simple widget "bundle"
      window.MyWidget = {
        render: function(root) {
          root.innerHTML = '<div class="widget-title">My Widget</div>';
        }
      };
    `

    const html = generateTestHTML({
      widgetBundle,
      testSteps: `
        // Mount widget
        const root = document.getElementById('effuse-test-root');
        window.MyWidget.render(root);

        // Test it
        assert.exists('.widget-title');
        assert.text('.widget-title', 'My Widget');
      `,
    })

    const results = await Effect.runPromise(executeWebviewTest(html))

    expect(results).toHaveLength(1)
    expect(results[0].pass).toBe(true)
  })
})
