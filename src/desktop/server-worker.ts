/**
 * Desktop Server Worker
 *
 * Runs the HTTP + WebSocket server in a separate worker so it doesn't
 * get blocked by webview.run() in the main thread.
 */

import { createDesktopServer } from "./server.js";

// Get config from parent thread
const staticDir = process.env.STATIC_DIR!;
const httpPort = parseInt(process.env.HTTP_PORT || "8080", 10);

const server = createDesktopServer({
  staticDir,
  httpPort,
  verbose: true,
});

console.log(`[Worker] Server running on http://localhost:${server.getHttpPort()}`);
console.log(`[Worker] HUD server on ws://localhost:${server.getHudPort()}`);

// Keep worker alive
setInterval(() => {}, 1000);
