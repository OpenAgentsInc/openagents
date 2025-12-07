/**
 * Desktop Server Worker
 *
 * Runs the HTTP + WebSocket server in a separate worker so it doesn't
 * get blocked by webview.run() in the main thread.
 */

import { createDesktopServer } from "./server.js";
import { log } from "./logger.js";
import { isTBRunComplete, type TBRunHistoryMessage, type DevReloadMessage } from "../hud/protocol.js";
import { loadRecentTBRuns } from "./handlers.js";
import { setATIFHudSender } from "../atif/hud-emitter.js";
import { watch } from "node:fs";
import { join, dirname } from "node:path";

// Get config from parent thread
const staticDir = process.env.STATIC_DIR!;
const httpPort = parseInt(process.env.HTTP_PORT || "8080", 10);

const server = createDesktopServer({
  staticDir,
  httpPort,
  verbose: true,
});

log("Worker", `Server running on http://localhost:${server.getHttpPort()}`);

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

// ============================================================================
// Hot Reload - File Watching
// ============================================================================

let reloadTimeout: ReturnType<typeof setTimeout> | null = null;

const triggerReload = (changedFile: string) => {
  if (reloadTimeout) clearTimeout(reloadTimeout);
  reloadTimeout = setTimeout(() => {
    log("Worker", `File changed: ${changedFile}, sending reload signal`);
    const message: DevReloadMessage = { type: "dev_reload", changedFile };
    server.sendHudMessage(message);
    reloadTimeout = null;
  }, 100); // 100ms debounce
};

// Watch src/mainview/ and src/effuse/ directories for changes
const projectRoot = dirname(staticDir); // Go up from src/mainview to src
const watchDirs = [
  staticDir, // src/mainview/
  join(projectRoot, "effuse"), // src/effuse/
];

for (const dir of watchDirs) {
  try {
    watch(dir, { recursive: true }, (eventType, filename) => {
      if (filename && /\.(ts|css|html)$/.test(filename)) {
        triggerReload(join(dir, filename));
      }
    });
    log("Worker", `HMR: Watching ${dir} for changes`);
  } catch (err) {
    log("Worker", `HMR: Could not watch ${dir}: ${err}`);
  }
}

// Keep worker alive
setInterval(() => {}, 1000);
