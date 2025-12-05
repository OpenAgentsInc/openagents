/**
 * Desktop entry point using webview-bun
 *
 * This replaces the Electrobun-based entry point with a simpler architecture:
 * - DesktopServer for HTTP (static files) and WebSocket (unified protocol)
 * - webview-bun for native window
 * - No RPC layer - all communication via WebSocket with request/response
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

// Optional: log HUD messages
server.onMessage((msg) => {
  console.log(`[HUD] ${msg.type}`);
});

// ============================================================================
// Native Window via webview-bun
// ============================================================================

const webview = new Webview();

// Debug: inject code to log page loading
webview.init(`
  console.log('[DEBUG] Webview init script running');
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

// Load content and embed directly - WebKit blocks localhost HTTP
const htmlFile = Bun.file(join(MAINVIEW_DIR, "index.html"));
const cssFile = Bun.file(join(MAINVIEW_DIR, "index.css"));
const jsFile = Bun.file(join(MAINVIEW_DIR, "index.js"));

const [html, css, js] = await Promise.all([
  htmlFile.text(),
  cssFile.text(),
  jsFile.text(),
]);

// Inject CSS and JS inline
const inlinedHtml = html
  .replace('<link rel="stylesheet" href="index.css">', `<style>${css}</style>`)
  .replace('<script type="module" src="index.js"></script>', `<script>${js}</script>`);

console.log(`[Desktop] Loaded: HTML=${html.length}b, CSS=${css.length}b, JS=${js.length}b`);
webview.setHTML(inlinedHtml);

console.log("[Desktop] Opening webview window...");

// This blocks until the window is closed
webview.run();

// Cleanup on exit
console.log("[Desktop] Window closed, shutting down...");
server.stop();
