/**
 * Desktop entry point using webview-bun
 *
 * Architecture:
 * - webview-bun for native window (content loaded via setHTML)
 * - WebSocket for RPC and HUD events (frontend ↔ DesktopServer)
 * - DesktopServer handles both UI clients and agent HUD connections
 *
 * Note: We use setHTML() because WebKit blocks navigate() to localhost.
 * But WebSocket connections TO localhost work fine from setHTML() content.
 */

import { Webview, SizeHint } from "webview-bun";
import { resolve, dirname, join } from "node:path";
import { createDesktopServer } from "./server.js";
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
// Desktop Server
// ============================================================================

const server = createDesktopServer({
  staticDir: MAINVIEW_DIR,
  httpPort: DESKTOP_HTTP_PORT,
  verbose: true,
});

console.log(`[Desktop] HTTP server: http://localhost:${server.getHttpPort()}`);
console.log(`[Desktop] HUD server: ws://localhost:${server.getHudPort()}`);
console.log(`[Desktop] Project root: ${PROJECT_ROOT}`);
console.log(`[Desktop] Mainview dir: ${MAINVIEW_DIR}`);

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

// Note: RPC and HUD events are handled via WebSocket (SocketClient in frontend)
// The DesktopServer broadcasts HUD messages to all UI clients over WebSocket

webview.title = "OpenAgents";
webview.size = { width: 1200, height: 800, hint: SizeHint.NONE };

// Load content and embed directly - WebKit blocks localhost HTTP
const htmlFile = Bun.file(join(MAINVIEW_DIR, "index.html"));
const cssFile = Bun.file(join(MAINVIEW_DIR, "index.css"));
const jsFile = Bun.file(join(MAINVIEW_DIR, "index.js"));

const [_html, css, _js] = await Promise.all([
  htmlFile.text(),
  cssFile.text(),
  jsFile.text(),
]);

// Test: inline script with addEventListener (not onclick)
const testHtml = `
<!DOCTYPE html>
<html>
<head><style>${css}</style></head>
<body>
  <h1 id="test">If you see this, HTML loaded</h1>
  <button id="btn">Click me</button>
  <script>
    document.getElementById('test').textContent = 'JS IS WORKING!';
    document.getElementById('btn').addEventListener('click', function() {
      alert('Button clicked via addEventListener!');
      document.getElementById('test').textContent = 'BUTTON CLICKED!';
    });
    console.log('Script with addEventListener executed');
  </script>
</body>
</html>
`;

console.log(`[Desktop] Testing with addEventListener...`);
webview.setHTML(testHtml);

console.log("[Desktop] Opening webview window...");

// This blocks until the window is closed
webview.run();

// Cleanup on exit
console.log("[Desktop] Window closed, shutting down...");
server.stop();
