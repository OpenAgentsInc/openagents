/**
 * Desktop entry point using webview-bun
 *
 * Architecture:
 * - webview-bun for native window
 * - HTTP server serves static files from src/mainview/
 * - WebSocket for RPC and HUD events (frontend ↔ DesktopServer)
 * - DesktopServer handles both UI clients and agent HUD connections
 *
 * IMPORTANT: We use navigate() to http://localhost:PORT instead of setHTML().
 * setHTML() creates an about:blank origin which blocks WebSocket connections.
 * navigate() to localhost gives the page a real origin so WebSocket works.
 */

import { Webview, SizeHint } from "webview-bun";
import { resolve, dirname, join } from "node:path";
import { DESKTOP_HTTP_PORT } from "./protocol.js";

// ============================================================================
// Project Root Resolution
// ============================================================================

function getProjectRoot(): string {
  const metaPath = dirname(import.meta.path);

  // If running from compiled binary, handle path resolution
  if (metaPath.includes(".app/Contents")) {
    const buildIndex = metaPath.indexOf("/build/");
    if (buildIndex !== -1) {
      return metaPath.slice(0, buildIndex);
    }
  }

  // Go up from src/desktop/ to project root
  return resolve(metaPath, "../..");
}

const PROJECT_ROOT = getProjectRoot();
const MAINVIEW_DIR = join(PROJECT_ROOT, "src/mainview");

// ============================================================================
// Desktop Server (runs in Worker to avoid blocking by webview.run())
// ============================================================================

console.log(`[Desktop] Project root: ${PROJECT_ROOT}`);
console.log(`[Desktop] Mainview dir: ${MAINVIEW_DIR}`);

// Start server in a Worker so it doesn't get blocked by webview.run()
const workerPath = join(import.meta.dir, "server-worker.ts");
const worker = new Worker(workerPath, {
  env: {
    STATIC_DIR: MAINVIEW_DIR,
    HTTP_PORT: String(DESKTOP_HTTP_PORT),
  },
});

// Give worker time to start
await new Promise((resolve) => setTimeout(resolve, 500));
console.log(`[Desktop] Server worker started`);

// ============================================================================
// Native Window via webview-bun
// ============================================================================

const webview = new Webview();

// Debug: inject error handler and HUD event listener
webview.init(`
  console.log('[OpenAgents] Webview initialized');
  window.onerror = function(msg, url, line) {
    console.error('[JS ERROR]', msg, 'at', url, line);
  };
`);

// Debug: bind a function to get logs from webview
webview.bind("bunLog", (...args: unknown[]) => {
  console.log("[Webview]", ...args);
});

webview.title = "OpenAgents";
webview.size = { width: 1200, height: 800, hint: SizeHint.NONE };

// Navigate to localhost HTTP server - this gives the page a real origin
// so WebSocket connections to localhost will work
const url = `http://localhost:${DESKTOP_HTTP_PORT}/`;
console.log(`[Desktop] Navigating to: ${url}`);
webview.navigate(url);

console.log("[Desktop] Opening webview window...");

// This blocks until the window is closed
webview.run();

// Cleanup on exit
console.log("[Desktop] Window closed, shutting down...");
worker.terminate();
