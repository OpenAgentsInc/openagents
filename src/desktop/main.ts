/**
 * Desktop entry point using webview-bun
 *
 * This replaces the Electrobun-based entry point with a simpler architecture:
 * - Bun.serve() for HTTP (static files) and WebSocket (HUD messages)
 * - webview-bun for native window
 * - No RPC layer - all communication via WebSocket
 */

import { Webview, SizeHint } from "webview-bun";
import { resolve, dirname, join } from "node:path";
import { HUD_WS_PORT, parseHudMessage } from "../hud/protocol.js";

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

// Ports
const HTTP_PORT = 8080;
const WS_PORT = HUD_WS_PORT; // 4242

// ============================================================================
// Static File Server + WebSocket Server
// ============================================================================

const hudClients = new Set<unknown>();

// Combined HTTP + WebSocket server
const server = Bun.serve({
  port: HTTP_PORT,

  async fetch(req, server) {
    const url = new URL(req.url);

    // WebSocket upgrade for /ws path
    if (url.pathname === "/ws") {
      if (server.upgrade(req)) {
        return;
      }
      return new Response("WebSocket upgrade failed", { status: 400 });
    }

    // Static file serving for mainview
    let filePath = url.pathname === "/" ? "/index.html" : url.pathname;
    const fullPath = join(MAINVIEW_DIR, filePath);

    // Security: ensure we stay within MAINVIEW_DIR
    if (!fullPath.startsWith(MAINVIEW_DIR)) {
      return new Response("Forbidden", { status: 403 });
    }

    const file = Bun.file(fullPath);
    if (await file.exists()) {
      // Determine content type
      const ext = filePath.split(".").pop() || "";
      const contentTypes: Record<string, string> = {
        html: "text/html",
        css: "text/css",
        js: "application/javascript",
        json: "application/json",
        woff: "font/woff",
        woff2: "font/woff2",
        svg: "image/svg+xml",
        png: "image/png",
        ico: "image/x-icon",
      };
      const contentType = contentTypes[ext] || "application/octet-stream";

      return new Response(file, {
        headers: { "Content-Type": contentType },
      });
    }

    return new Response("Not Found", { status: 404 });
  },

  websocket: {
    open(ws) {
      hudClients.add(ws);
      console.log(`[HUD] Client connected (total: ${hudClients.size})`);
    },
    message(ws, message) {
      const data = typeof message === "string" ? message : message.toString();
      const parsed = parseHudMessage(data);

      if (parsed) {
        // Broadcast to all connected clients (including the webview)
        for (const client of hudClients) {
          if (client !== ws) {
            try {
              (client as { send: (msg: string) => void }).send(JSON.stringify(parsed));
            } catch {
              // Client may have disconnected
            }
          }
        }
      }
    },
    close(ws) {
      hudClients.delete(ws);
      console.log(`[HUD] Client disconnected (total: ${hudClients.size})`);
    },
  },
});

console.log(`[Desktop] HTTP server: http://localhost:${HTTP_PORT}`);
console.log(`[Desktop] WebSocket: ws://localhost:${HTTP_PORT}/ws`);
console.log(`[Desktop] Project root: ${PROJECT_ROOT}`);
console.log(`[Desktop] Mainview dir: ${MAINVIEW_DIR}`);

// ============================================================================
// Separate HUD WebSocket Server (for agent connections on port 4242)
// ============================================================================

const hudServer = Bun.serve({
  port: WS_PORT,
  fetch(req, server) {
    if (server.upgrade(req)) {
      return;
    }
    return new Response("OpenAgents HUD WebSocket Server", { status: 200 });
  },
  websocket: {
    open(ws) {
      hudClients.add(ws);
      console.log(`[HUD] Agent connected on port ${WS_PORT} (total: ${hudClients.size})`);
    },
    message(ws, message) {
      const data = typeof message === "string" ? message : message.toString();
      const parsed = parseHudMessage(data);

      if (parsed) {
        // Broadcast to all connected clients
        for (const client of hudClients) {
          if (client !== ws) {
            try {
              (client as { send: (msg: string) => void }).send(JSON.stringify(parsed));
            } catch {
              // Client may have disconnected
            }
          }
        }
      }
    },
    close(ws) {
      hudClients.delete(ws);
      console.log(`[HUD] Agent disconnected (total: ${hudClients.size})`);
    },
  },
});

console.log(`[HUD] Agent WebSocket server: ws://localhost:${WS_PORT}`);

// ============================================================================
// Native Window via webview-bun
// ============================================================================

const webview = new Webview();

webview.title = "OpenAgents";
webview.size = { width: 1200, height: 800, hint: SizeHint.NONE };
webview.navigate(`http://localhost:${HTTP_PORT}`);

console.log("[Desktop] Opening webview window...");

// This blocks until the window is closed
webview.run();

// Cleanup on exit
console.log("[Desktop] Window closed, shutting down...");
server.stop();
hudServer.stop();
