/**
 * Desktop Server
 *
 * Unified HTTP + WebSocket server for the webview-bun desktop app.
 *
 * Architecture (unified):
 * - Single port (8080) for everything
 * - UI clients connect for request/response + receiving HUD events
 * - Agents connect to send HUD events (which get broadcast to UI clients)
 * - No separate HUD port needed
 *
 * Handles:
 * - Static file serving for the UI
 * - Unified socket protocol (events + requests/responses)
 * - HUD messages from agents (broadcast to UI clients)
 */

import { join } from "node:path";
import type { HudMessage, TBRunHistoryMessage } from "../hud/protocol.js";
import {
  parseSocketMessage,
  isSocketRequest,
  isHudEvent,
  serializeSocketMessage,
  DESKTOP_HTTP_PORT,
  DESKTOP_WS_PATH,
} from "./protocol.js";
import { handleRequest, loadRecentTBRuns } from "./handlers.js";
import { error as logError, log as logWithColor } from "./logger.js";

// ============================================================================
// Types
// ============================================================================

export type MessageHandler = (message: HudMessage) => void;
export type ConnectionHandler = (clientId: string) => void;

export interface DesktopServerOptions {
  /** HTTP port for static files and WebSocket (default: 8080) */
  httpPort?: number;
  /** Directory to serve static files from */
  staticDir: string;
  /** Enable verbose logging */
  verbose?: boolean;
}

interface WebSocketClient {
  id: string;
  ws: unknown;
  isAgent: boolean; // true if sending HUD events only, false if UI client (also receives)
}

// ============================================================================
// Desktop Server Class
// ============================================================================

export class DesktopServer {
  private httpServer: ReturnType<typeof Bun.serve> | null = null;

  private readonly httpPort: number;
  private readonly staticDir: string;
  private readonly verbose: boolean;

  private clients: Map<string, WebSocketClient> = new Map();
  private messageHandlers: MessageHandler[] = [];
  private connectHandlers: ConnectionHandler[] = [];
  private disconnectHandlers: ConnectionHandler[] = [];

  private messageHistory: HudMessage[] = [];
  private readonly maxHistorySize = 100;

  constructor(options: DesktopServerOptions) {
    this.httpPort = options.httpPort ?? DESKTOP_HTTP_PORT;
    this.staticDir = options.staticDir;
    this.verbose = options.verbose ?? false;
  }

  /**
   * Start the unified server.
   */
  start(): void {
    this.startHttpServer();
  }

  /**
   * Stop the server.
   */
  stop(): void {
    if (this.httpServer) {
      this.httpServer.stop();
      this.httpServer = null;
    }
    this.clients.clear();
    this.log("Server stopped");
  }

  /**
   * Start HTTP server with static files and WebSocket for UI clients.
   */
  private startHttpServer(): void {
    if (this.httpServer) return;

    const self = this;

    this.httpServer = Bun.serve({
      port: this.httpPort,

      async fetch(req, server) {
        const url = new URL(req.url);

        // WebSocket upgrade for UI clients
        if (url.pathname === DESKTOP_WS_PATH) {
          const clientId = `ui-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
          if (server.upgrade(req, { data: { clientId, isAgent: false } })) {
            return;
          }
          return new Response("WebSocket upgrade failed", { status: 400 });
        }

        // Static file serving
        let filePath = url.pathname === "/" ? "/index.html" : url.pathname;
        const fullPath = join(self.staticDir, filePath);

        // Security: ensure we stay within staticDir
        if (!fullPath.startsWith(self.staticDir)) {
          return new Response("Forbidden", { status: 403 });
        }

        const file = Bun.file(fullPath);
        const ext = filePath.split(".").pop() || "";

        // If .js file requested but doesn't exist, check for .ts source
        if (ext === "js" && !(await file.exists())) {
          const tsPath = fullPath.replace(/\.js$/, ".ts");
          const tsFile = Bun.file(tsPath);
          if (await tsFile.exists()) {
            try {
              const result = await Bun.build({
                entrypoints: [tsPath],
                target: "browser",
                minify: false,
                format: "iife",
              });
              if (result.success && result.outputs.length > 0) {
                const text = await result.outputs[0].text();
                return new Response(text, {
                  headers: {
                    "Content-Type": "application/javascript",
                    "Cache-Control": "no-cache, no-store, must-revalidate",
                    "Pragma": "no-cache",
                    "Expires": "0",
                  },
                });
              }
            } catch (e) {
              logError("DesktopServer", "TypeScript build error:", e);
              return new Response(`Build error: ${e}`, { status: 500 });
            }
          }
        }

        if (await file.exists()) {
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
            headers: {
              "Content-Type": contentType,
              "Cache-Control": "no-cache, no-store, must-revalidate",
              "Pragma": "no-cache",
              "Expires": "0",
            },
          });
        }

        return new Response("Not Found", { status: 404 });
      },

      websocket: {
        open(ws) {
          const data = (ws as unknown as { data: { clientId: string; isAgent: boolean } }).data;
          self.clients.set(data.clientId, { id: data.clientId, ws, isAgent: false });
          self.log(`UI client connected: ${data.clientId} (total: ${self.clients.size})`);

          // Send message history to new UI client
          for (const msg of self.messageHistory) {
            (ws as unknown as { send: (msg: string) => void }).send(JSON.stringify(msg));
          }

          for (const handler of self.connectHandlers) {
            try {
              handler(data.clientId);
            } catch (e) {
              logError("DesktopServer", "Connect handler error:", e);
            }
          }

          // Push latest TB run history to all clients (includes the new one)
          void self.sendTBRunHistory();
        },

        async message(ws, message) {
          const data = typeof message === "string" ? message : message.toString();
          const parsed = parseSocketMessage(data);

          if (!parsed) {
            self.log(`Invalid message received: ${data.slice(0, 100)}`);
            return;
          }

          // Handle requests from UI clients via WebSocket RPC
          if (isSocketRequest(parsed)) {
            self.log(`Request received: ${parsed.type}`);
            const response = await handleRequest(parsed);
            (ws as unknown as { send: (msg: string) => void }).send(
              serializeSocketMessage(response)
            );
            return;
          }

          // Handle HUD events (forward to all UI clients)
          if (isHudEvent(parsed)) {
            self.handleHudMessage(parsed as HudMessage);
          }
        },

        close(ws) {
          const data = (ws as unknown as { data: { clientId: string } }).data;
          self.clients.delete(data.clientId);
          self.log(`UI client disconnected: ${data.clientId} (total: ${self.clients.size})`);

          for (const handler of self.disconnectHandlers) {
            try {
              handler(data.clientId);
            } catch (e) {
              logError("DesktopServer", "Disconnect handler error:", e);
            }
          }
        },
      },
    });

    this.log(`HTTP server started on port ${this.httpPort}`);
    this.log(`WebSocket endpoint: ws://localhost:${this.httpPort}${DESKTOP_WS_PATH}`);
    this.log(`Agents should connect to the same endpoint to send HUD events`);
  }

  /**
   * Handle a HUD message: store in history, notify handlers, broadcast to UI clients.
   */
  private handleHudMessage(message: HudMessage): void {
    // Store in history
    this.messageHistory.push(message);
    if (this.messageHistory.length > this.maxHistorySize) {
      this.messageHistory.shift();
    }

    // Notify handlers
    for (const handler of this.messageHandlers) {
      try {
        handler(message);
      } catch (e) {
        logError("DesktopServer", "Message handler error:", e);
      }
    }

    // Broadcast to all UI clients (non-agent)
    const serialized = JSON.stringify(message);
    for (const client of this.clients.values()) {
      if (!client.isAgent) {
        try {
          (client.ws as { send: (msg: string) => void }).send(serialized);
        } catch {
          // Client may have disconnected
        }
      }
    }
  }

  async sendTBRunHistory(): Promise<void> {
    try {
      const runs = await loadRecentTBRuns();
      const message: TBRunHistoryMessage = { type: "tb_run_history", runs };
      this.handleHudMessage(message);
    } catch (e) {
      logError("DesktopServer", "Failed to broadcast TB run history:", e);
    }
  }

  /**
   * Broadcast a HUD message originating from the desktop server itself.
   * Stored in history for new UI clients, just like agent-emitted events.
   */
  sendHudMessage(message: HudMessage): void {
    this.handleHudMessage(message);
  }

  /**
   * Register a handler for HUD messages.
   */
  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.push(handler);
    return () => {
      const idx = this.messageHandlers.indexOf(handler);
      if (idx >= 0) this.messageHandlers.splice(idx, 1);
    };
  }

  /**
   * Register a handler for client connections.
   */
  onConnect(handler: ConnectionHandler): () => void {
    this.connectHandlers.push(handler);
    return () => {
      const idx = this.connectHandlers.indexOf(handler);
      if (idx >= 0) this.connectHandlers.splice(idx, 1);
    };
  }

  /**
   * Register a handler for client disconnections.
   */
  onDisconnect(handler: ConnectionHandler): () => void {
    this.disconnectHandlers.push(handler);
    return () => {
      const idx = this.disconnectHandlers.indexOf(handler);
      if (idx >= 0) this.disconnectHandlers.splice(idx, 1);
    };
  }

  /**
   * Get total client count.
   */
  getClientCount(): number {
    return this.clients.size;
  }

  /**
   * Get agent count.
   */
  getAgentCount(): number {
    let count = 0;
    for (const client of this.clients.values()) {
      if (client.isAgent) count++;
    }
    return count;
  }

  /**
   * Get UI client count.
   */
  getUIClientCount(): number {
    return this.clients.size - this.getAgentCount();
  }

  /**
   * Get message history.
   */
  getMessageHistory(): readonly HudMessage[] {
    return this.messageHistory;
  }

  /**
   * Clear message history.
   */
  clearHistory(): void {
    this.messageHistory = [];
  }

  /**
   * Check if server is running.
   */
  isRunning(): boolean {
    return this.httpServer !== null;
  }

  /**
   * Get HTTP port.
   */
  getHttpPort(): number {
    return this.httpPort;
  }

  private log(msg: string): void {
    if (this.verbose) {
      logWithColor("DesktopServer", msg);
    }
  }
}

// ============================================================================
// Factory Function
// ============================================================================

/**
 * Create and start a desktop server.
 */
export function createDesktopServer(options: DesktopServerOptions): DesktopServer {
  const server = new DesktopServer(options);

  server.onMessage((message) => {
    if (message.type === "tb_run_complete") {
      void server.sendTBRunHistory();
    }
  });

  server.start();
  return server;
}
