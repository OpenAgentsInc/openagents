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

// Test: WebSocket connectivity from setHTML content
const html = _html;
const js = _js;

// Prepend error catching to see what fails
const testScript = `
  console.log('[TEST] Script starting...');
  window.bunLog('[TEST] bunLog available, JS is running!');
  window.onerror = function(msg, url, line, col, error) {
    window.bunLog('[JS ERROR] ' + msg + ' at line ' + line + ':' + col);
    return true;
  };
`;

// Wrap bundle in try-catch to get actual error details
const wrappedJs = `
try {
${js}
} catch(e) {
  window.bunLog('[JS CATCH] ' + e.name + ': ' + e.message);
  window.bunLog('[JS STACK] ' + e.stack);
}
`;

// Inject CSS and JS inline (addEventListener works, inline onclick doesn't)
const inlinedHtml = html
  .replace('<link rel="stylesheet" href="index.css">', `<style>${css}</style>`)
  .replace('<script type="module" src="index.js"></script>', `<script>${testScript}\n${wrappedJs}</script>`);

console.log(`[Desktop] Loaded: HTML=${html.length}b, CSS=${css.length}b, JS=${js.length}b`);
console.log(`[Desktop] Inlined HTML: ${inlinedHtml.length}b`);
webview.setHTML(inlinedHtml);

console.log("[Desktop] Opening webview window...");

// This blocks until the window is closed
webview.run();

// Cleanup on exit
console.log("[Desktop] Window closed, shutting down...");
server.stop();
