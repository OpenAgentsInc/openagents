/**
 * Desktop Server Worker
 *
 * Runs the HTTP + WebSocket server in a separate worker so it doesn't
 * get blocked by webview.run() in the main thread.
 */

// Debug: log immediately to see if worker starts at all
console.log("[Worker] ===== SERVER WORKER STARTING =====");

import { createDesktopServer } from "./server.js";
import { log } from "./logger.js";
import { isTBRunComplete, type TBRunHistoryMessage, type DevReloadMessage } from "../hud/protocol.js";
import { loadRecentTBRuns } from "./handlers.js";
import { setATIFHudSender } from "../atif/hud-emitter.js";
import { watch } from "node:fs";
import { join, dirname } from "node:path";

console.log("[Worker] Imports complete");

// Wrap everything in try-catch to see startup errors
try {
  log("Worker", "Starting server worker...");
} catch (e) {
  console.error("[Worker] Failed to log:", e);
}

// Get config from parent thread
const staticDir = process.env.STATIC_DIR;
const httpPort = parseInt(process.env.HTTP_PORT || "8080", 10);

console.log("[Worker] staticDir:", staticDir);
console.log("[Worker] httpPort:", httpPort);

if (!staticDir) {
  console.error("[Worker] STATIC_DIR not set!");
  throw new Error("STATIC_DIR environment variable not set");
}

let server: ReturnType<typeof createDesktopServer>;
try {
  server = createDesktopServer({
    staticDir,
    httpPort,
    verbose: true,
  });
  log("Worker", `Server running on http://localhost:${server.getHttpPort()}`);
} catch (e) {
  console.error("[Worker] Failed to create server:", e);
  throw e;
}

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
let isRebuilding = false;

const triggerReload = (changedFile: string) => {
  if (reloadTimeout) clearTimeout(reloadTimeout);
  reloadTimeout = setTimeout(async () => {
    if (isRebuilding) return; // Skip if already rebuilding
    isRebuilding = true;

    try {
      log("Worker", `HMR: File changed: ${changedFile}`);
      log("Worker", `HMR: Rebuilding effuse-main.js...`);

      // Rebuild the frontend bundle
      const entrypoint = join(staticDir, "effuse-main.ts");
      const outfile = join(staticDir, "effuse-main.js");

      const result = await Bun.build({
        entrypoints: [entrypoint],
        outdir: staticDir,
        target: "browser",
        format: "iife",
        minify: false,
      });

      if (!result.success) {
        log("Worker", `HMR: Build failed:`);
        for (const msg of result.logs) {
          log("Worker", `  ${msg}`);
        }
        return;
      }

      log("Worker", `HMR: Rebuild complete, sending reload signal`);
      const message: DevReloadMessage = { type: "dev_reload", changedFile };
      server.sendHudMessage(message);
    } catch (err) {
      log("Worker", `HMR: Rebuild error: ${err}`);
    } finally {
      isRebuilding = false;
      reloadTimeout = null;
    }
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
