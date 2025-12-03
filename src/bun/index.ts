import { BrowserWindow, BrowserView } from "electrobun/bun";
import type { HudMessage } from "../hud/protocol.js";
import { HUD_WS_PORT, parseHudMessage } from "../hud/protocol.js";

// ============================================================================
// RPC Schema for HUD Messages
// ============================================================================

interface HudRpcSchema {
  bun: {
    requests: {};
    messages: {};
  };
  webview: {
    requests: {};
    messages: {
      hudMessage: HudMessage;
    };
  };
}

// Define RPC with HUD message support
const rpc = BrowserView.defineRPC<HudRpcSchema>({
  handlers: {
    requests: {},
    messages: {},
  },
});

// Create the main application window
const mainWindow = new BrowserWindow({
  title: "OpenAgents",
  url: "views://mainview/index.html",
  frame: {
    width: 1200,
    height: 800,
    x: 200,
    y: 200,
  },
  rpc,
});

// ============================================================================
// HUD WebSocket Server
// ============================================================================

// Track connected clients
const hudClients = new Set<unknown>();

// Start WebSocket server for agent connections
const hudServer = Bun.serve({
  port: HUD_WS_PORT,
  fetch(req, server) {
    // Upgrade HTTP request to WebSocket
    if (server.upgrade(req)) {
      return;
    }
    return new Response("OpenAgents HUD WebSocket Server", { status: 200 });
  },
  websocket: {
    open(ws) {
      hudClients.add(ws);
      console.log(`[HUD] Agent connected (total: ${hudClients.size})`);
    },
    message(ws, message) {
      const data = typeof message === "string" ? message : message.toString();
      const parsed = parseHudMessage(data);

      if (parsed) {
        // Forward message to mainview via Electrobun RPC
        try {
          mainWindow.webview.rpc.send.hudMessage(parsed);
        } catch {
          // Webview may not be ready yet, silently ignore
        }
      }
    },
    close(ws) {
      hudClients.delete(ws);
      console.log(`[HUD] Agent disconnected (total: ${hudClients.size})`);
    },
  },
});

console.log(`[HUD] WebSocket server started on port ${HUD_WS_PORT}`);
console.log("Hello Electrobun app started!");

// Keep references to avoid GC
void mainWindow;
void hudServer;
