/**
 * Effuse Webview Testing Module
 *
 * Real browser testing using webview-bun.
 * Tests run in the same engine as production (WebKit/Edge WebView2/WebKitGTK).
 */

export { executeWebviewTest, executeAndAssert, type TestResult } from "./execute.js"
export { generateTestHTML, generateSimpleTest, type TestHTMLOptions } from "./html-template.js"
