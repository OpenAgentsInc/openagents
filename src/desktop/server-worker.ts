/**
 * Desktop Server Worker
 *
 * Runs the HTTP + WebSocket server in a separate worker so it doesn't
 * get blocked by webview.run() in the main thread.
 */

import { createDesktopServer } from "./server.js";
import { log } from "./logger.js";
import { isTBRunComplete, type TBRunHistoryMessage } from "../hud/protocol.js";
import { loadRecentTBRuns } from "./handlers.js";
import { setATIFHudSender } from "../atif/hud-emitter.js";

// Get config from parent thread
const staticDir = process.env.STATIC_DIR!;
const httpPort = parseInt(process.env.HTTP_PORT || "8080", 10);

const server = createDesktopServer({
  staticDir,
  httpPort,
  verbose: true,
});

log("Worker", `Server running on http://localhost:${server.getHttpPort()}`);
log("Worker", `HUD server on ws://localhost:${server.getHudPort()}`);

// Wire ATIF HUD emitter to desktop server WebSocket
setATIFHudSender((message) => {
  server.sendHudMessage(message);
});
log("Worker", "ATIF HUD emitter initialized");

const broadcastRunHistory = async (): Promise<void> => {
  const runs = await loadRecentTBRuns(20);
  const message: TBRunHistoryMessage = {
    type: "tb_run_history",
    runs,
  };
  server.sendHudMessage(message);
};

// Push TB run history updates when runs complete (no polling)
server.onMessage((message) => {
  if (isTBRunComplete(message)) {
    void broadcastRunHistory();
  }
});

// Seed initial history for connected clients
void broadcastRunHistory();

// Keep worker alive
setInterval(() => {}, 1000);
