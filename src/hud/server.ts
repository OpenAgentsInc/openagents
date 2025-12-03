/**
 * HUD WebSocket Server
 *
 * Runs inside the Electrobun mainview process.
 * Receives events from agent processes via WebSocket and
 * provides them to the UI for rendering.
 *
 * Usage (in mainview/index.ts):
 *   import { HudServer } from "../hud/server.js";
 *
 *   const server = new HudServer();
 *   server.onMessage((msg) => {
 *     // Update UI based on message
 *     console.log("Received:", msg);
 *   });
 */

import type { HudMessage } from "./protocol.js";
import { HUD_WS_PORT, parseHudMessage } from "./protocol.js";

export type MessageHandler = (message: HudMessage) => void;
export type ConnectionHandler = () => void;

export interface HudServerOptions {
  /** Port to listen on (default: 4242) */
  port?: number;
  /** Whether to enable verbose logging (default: false) */
  verbose?: boolean;
}

export class HudServer {
  private server: ReturnType<typeof Bun.serve> | null = null;
  private readonly port: number;
  private readonly verbose: boolean;

  private messageHandlers: MessageHandler[] = [];
  private connectHandlers: ConnectionHandler[] = [];
  private disconnectHandlers: ConnectionHandler[] = [];

  private clients: Set<unknown> = new Set();
  private messageHistory: HudMessage[] = [];
  private readonly maxHistorySize = 100;

  constructor(options: HudServerOptions = {}) {
    this.port = options.port ?? HUD_WS_PORT;
    this.verbose = options.verbose ?? false;
  }

  /**
   * Start the WebSocket server.
   */
  start(): void {
    if (this.server) return;

    this.log(`Starting HUD WebSocket server on port ${this.port}`);

    this.server = Bun.serve({
      port: this.port,
      fetch: (req, server) => {
        // Upgrade HTTP request to WebSocket
        if (server.upgrade(req)) {
          return; // upgraded successfully
        }
        return new Response("HUD WebSocket Server", { status: 200 });
      },
      websocket: {
        open: (ws) => {
          this.clients.add(ws);
          this.log(`Client connected (total: ${this.clients.size})`);

          // Send recent message history to newly connected client
          for (const msg of this.messageHistory) {
            ws.send(JSON.stringify(msg));
          }

          for (const handler of this.connectHandlers) {
            try {
              handler();
            } catch (e) {
              console.error("[HudServer] Connect handler error:", e);
            }
          }
        },
        message: (ws, message) => {
          const data = typeof message === "string" ? message : message.toString();
          const parsed = parseHudMessage(data);

          if (parsed) {
            // Store in history
            this.messageHistory.push(parsed);
            if (this.messageHistory.length > this.maxHistorySize) {
              this.messageHistory.shift();
            }

            // Notify handlers
            for (const handler of this.messageHandlers) {
              try {
                handler(parsed);
              } catch (e) {
                console.error("[HudServer] Message handler error:", e);
              }
            }
          }
        },
        close: (ws) => {
          this.clients.delete(ws);
          this.log(`Client disconnected (total: ${this.clients.size})`);

          for (const handler of this.disconnectHandlers) {
            try {
              handler();
            } catch (e) {
              console.error("[HudServer] Disconnect handler error:", e);
            }
          }
        },
      },
    });

    this.log("HUD WebSocket server started");
  }

  /**
   * Stop the WebSocket server.
   */
  stop(): void {
    if (this.server) {
      this.server.stop();
      this.server = null;
      this.clients.clear();
      this.log("HUD WebSocket server stopped");
    }
  }

  /**
   * Register a handler for incoming messages.
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
   * Get the number of connected clients.
   */
  getClientCount(): number {
    return this.clients.size;
  }

  /**
   * Get recent message history.
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
    return this.server !== null;
  }

  private log(msg: string): void {
    if (this.verbose) {
      console.log(`[HudServer] ${msg}`);
    }
  }
}

// ============================================================================
// Singleton for Convenience
// ============================================================================

let defaultServer: HudServer | null = null;

/**
 * Get or create the default HudServer singleton.
 */
export const getHudServer = (options?: HudServerOptions): HudServer => {
  if (!defaultServer) {
    defaultServer = new HudServer(options);
  }
  return defaultServer;
};

/**
 * Start the default server.
 */
export const startHudServer = (options?: HudServerOptions): HudServer => {
  const server = getHudServer(options);
  server.start();
  return server;
};

/**
 * Stop the default server.
 */
export const stopHudServer = (): void => {
  if (defaultServer) {
    defaultServer.stop();
    defaultServer = null;
  }
};
