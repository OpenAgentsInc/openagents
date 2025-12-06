/**
 * Webview Test Runner (Subprocess Entry Point)
 *
 * This script runs in a subprocess spawned by the test framework.
 * It creates a webview-bun instance, loads test HTML, runs tests,
 * and outputs results as JSON to stdout.
 *
 * Environment Variables:
 * - EFFUSE_TEST_HTML: The complete HTML to load in the webview
 * - EFFUSE_HEADED: "1" to show the webview window (for debugging)
 * - EFFUSE_TIMEOUT: Test timeout in milliseconds (default: 30000)
 */

import { Webview } from "webview-bun"

const testHTML = Bun.env.EFFUSE_TEST_HTML
if (!testHTML) {
  console.error(JSON.stringify([{ pass: false, error: "EFFUSE_TEST_HTML not set" }]))
  process.exit(1)
}

const headed = Bun.env.EFFUSE_HEADED === "1"
const timeout = parseInt(Bun.env.EFFUSE_TIMEOUT || "30000", 10)

let results = "[]"
let hasResults = false

// Create webview - pass true for devtools when headed
const webview = new Webview(headed)

// Bind the results reporter
webview.bind("reportResults", (r: string) => {
  results = r
  hasResults = true
})

// Inject test status checker that will close the window
webview.init(`
  // Check for test completion and close window
  const checkAndClose = () => {
    if (window.__effuseTestDone) {
      // Small delay to ensure results are reported
      setTimeout(() => window.close(), 50)
    } else {
      setTimeout(checkAndClose, 50)
    }
  }

  // Start checking after a brief delay
  setTimeout(checkAndClose, 100)
`)

// Load the test HTML
webview.setHTML(testHTML)

// Failsafe timeout - if tests don't complete, force exit
const timer = setTimeout(() => {
  if (!hasResults) {
    console.log(JSON.stringify([{ pass: false, error: `Test timeout after ${timeout}ms` }]))
    process.exit(1)
  }
}, timeout)

// Run the webview event loop (blocks until window closes)
webview.run()

// Cleanup
clearTimeout(timer)

// Output results to stdout for parent process to parse
console.log(results)
