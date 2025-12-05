/**
 * Desktop Socket Client
 *
 * WebSocket client for the mainview that communicates with the desktop server
 * using the unified socket protocol. Replaces Electrobun RPC.
 *
 * Features:
 * - Request/response with correlation IDs and timeouts
 * - HUD message event handling
 * - Auto-reconnect with exponential backoff
 * - Message queueing when disconnected
 */

import type { HudMessage } from "../hud/protocol.js";
import type {
  SocketRequest,
  SocketResponse,
  TBSuiteInfo,
  TBRunHistoryItem,
  TBRunDetails,
  MCTask,
} from "../desktop/protocol.js";
import {
  generateCorrelationId,
  parseSocketMessage,
  serializeSocketMessage,
  isSocketResponse,
  isHudEvent,
  DESKTOP_HTTP_PORT,
  DESKTOP_WS_PATH,
} from "../desktop/protocol.js";

// ============================================================================
// Types
// ============================================================================

export type HudMessageHandler = (message: HudMessage) => void;
export type ConnectionHandler = () => void;

export interface SocketClientOptions {
  /** WebSocket URL (default: ws://localhost:8080/ws) */
  url?: string;
  /** Request timeout in ms (default: 10000) */
  requestTimeout?: number;
  /** Enable auto-reconnect (default: true) */
  autoReconnect?: boolean;
  /** Max reconnect attempts (default: 10) */
  maxReconnectAttempts?: number;
  /** Enable verbose logging (default: false) */
  verbose?: boolean;
}

interface PendingRequest {
  resolve: (response: SocketResponse) => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
}

// ============================================================================
// Socket Client Class
// ============================================================================

export class SocketClient {
  private ws: WebSocket | null = null;
  private readonly url: string;
  private readonly requestTimeout: number;
  private readonly autoReconnect: boolean;
  private readonly maxReconnectAttempts: number;
  private readonly verbose: boolean;

  private pendingRequests: Map<string, PendingRequest> = new Map();
  private messageQueue: string[] = [];
  private messageHandlers: HudMessageHandler[] = [];
  private connectHandlers: ConnectionHandler[] = [];
  private disconnectHandlers: ConnectionHandler[] = [];

  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private isConnecting = false;

  constructor(options: SocketClientOptions = {}) {
    this.url = options.url ?? `ws://localhost:${DESKTOP_HTTP_PORT}${DESKTOP_WS_PATH}`;
    this.requestTimeout = options.requestTimeout ?? 10000;
    this.autoReconnect = options.autoReconnect ?? true;
    this.maxReconnectAttempts = options.maxReconnectAttempts ?? 10;
    this.verbose = options.verbose ?? false;
  }

  /**
   * Connect to the server.
   */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        resolve();
        return;
      }

      if (this.isConnecting) {
        // Wait for existing connection attempt
        const checkInterval = setInterval(() => {
          if (this.ws?.readyState === WebSocket.OPEN) {
            clearInterval(checkInterval);
            resolve();
          }
        }, 100);
        return;
      }

      this.isConnecting = true;
      this.log(`Connecting to ${this.url}`);
      // Also log to terminal via bunLog
      (window as any).bunLog?.(`[SocketClient] Connecting to ${this.url}`);

      try {
        this.ws = new WebSocket(this.url);
        (window as any).bunLog?.(`[SocketClient] WebSocket created, waiting for open...`);

        this.ws.onopen = () => {
          this.isConnecting = false;
          this.reconnectAttempts = 0;
          this.log("Connected");
          (window as any).bunLog?.(`[SocketClient] WebSocket OPEN!`);

          // Flush queued messages
          while (this.messageQueue.length > 0) {
            const msg = this.messageQueue.shift()!;
            this.ws?.send(msg);
          }

          // Notify handlers
          for (const handler of this.connectHandlers) {
            try {
              handler();
            } catch (e) {
              console.error("[SocketClient] Connect handler error:", e);
            }
          }

          resolve();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(event.data);
        };

        this.ws.onclose = () => {
          this.isConnecting = false;
          this.log("Disconnected");

          // Notify handlers
          for (const handler of this.disconnectHandlers) {
            try {
              handler();
            } catch (e) {
              console.error("[SocketClient] Disconnect handler error:", e);
            }
          }

          // Auto-reconnect
          if (this.autoReconnect && this.reconnectAttempts < this.maxReconnectAttempts) {
            this.scheduleReconnect();
          }
        };

        this.ws.onerror = (error) => {
          this.isConnecting = false;
          this.log(`Connection error: ${error}`);
          (window as any).bunLog?.(`[SocketClient] WebSocket ERROR:`, String(error));
          reject(new Error("WebSocket connection failed"));
        };
      } catch (e) {
        this.isConnecting = false;
        reject(e);
      }
    });
  }

  /**
   * Disconnect from the server.
   */
  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempts = this.maxReconnectAttempts; // Prevent auto-reconnect

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    // Reject all pending requests
    for (const [_id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(new Error("Client disconnected"));
    }
    this.pendingRequests.clear();
  }

  /**
   * Check if connected.
   */
  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Send a request and wait for response.
   */
  async request<T extends SocketResponse>(
    type: SocketRequest["type"],
    params: Omit<Extract<SocketRequest, { type: typeof type }>, "type" | "correlationId">
  ): Promise<T> {
    const correlationId = generateCorrelationId();
    const request = { type, correlationId, ...params } as SocketRequest;

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(correlationId);
        reject(new Error(`Request timeout: ${type}`));
      }, this.requestTimeout);

      this.pendingRequests.set(correlationId, {
        resolve: resolve as (response: SocketResponse) => void,
        reject,
        timeout,
      });

      const msg = serializeSocketMessage(request);

      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(msg);
      } else {
        // Queue for later
        this.messageQueue.push(msg);

        // Try to connect if not already
        if (!this.isConnecting && this.autoReconnect) {
          this.connect().catch(() => {});
        }
      }
    });
  }

  /**
   * Register a handler for HUD messages.
   */
  onMessage(handler: HudMessageHandler): () => void {
    this.messageHandlers.push(handler);
    return () => {
      const idx = this.messageHandlers.indexOf(handler);
      if (idx >= 0) this.messageHandlers.splice(idx, 1);
    };
  }

  /**
   * Register a handler for connection events.
   */
  onConnect(handler: ConnectionHandler): () => void {
    this.connectHandlers.push(handler);
    return () => {
      const idx = this.connectHandlers.indexOf(handler);
      if (idx >= 0) this.connectHandlers.splice(idx, 1);
    };
  }

  /**
   * Register a handler for disconnection events.
   */
  onDisconnect(handler: ConnectionHandler): () => void {
    this.disconnectHandlers.push(handler);
    return () => {
      const idx = this.disconnectHandlers.indexOf(handler);
      if (idx >= 0) this.disconnectHandlers.splice(idx, 1);
    };
  }

  // ============================================================================
  // Request Convenience Methods
  // ============================================================================

  /**
   * Load a TB suite file.
   */
  async loadTBSuite(suitePath: string): Promise<TBSuiteInfo> {
    const response = await this.request("request:loadTBSuite", { suitePath });
    if (!response.success) {
      throw new Error(response.error ?? "Failed to load suite");
    }
    return (response as Extract<SocketResponse, { type: "response:loadTBSuite" }>).data!;
  }

  /**
   * Start a TB run.
   */
  async startTBRun(options: {
    suitePath: string;
    taskIds?: string[];
    timeout?: number;
    maxTurns?: number;
    outputDir?: string;
  }): Promise<{ runId: string }> {
    const response = await this.request("request:startTBRun", options);
    if (!response.success) {
      throw new Error(response.error ?? "Failed to start run");
    }
    return (response as Extract<SocketResponse, { type: "response:startTBRun" }>).data!;
  }

  /**
   * Stop the active TB run.
   */
  async stopTBRun(): Promise<{ stopped: boolean }> {
    const response = await this.request("request:stopTBRun", {});
    if (!response.success) {
      throw new Error(response.error ?? "Failed to stop run");
    }
    return (response as Extract<SocketResponse, { type: "response:stopTBRun" }>).data!;
  }

  /**
   * Load recent TB run history.
   */
  async loadRecentTBRuns(count?: number): Promise<TBRunHistoryItem[]> {
    const response = await this.request("request:loadRecentTBRuns", { count });
    if (!response.success) {
      throw new Error(response.error ?? "Failed to load runs");
    }
    return (response as Extract<SocketResponse, { type: "response:loadRecentTBRuns" }>).data!;
  }

  /**
   * Load full TB run details.
   */
  async loadTBRunDetails(runId: string): Promise<TBRunDetails | null> {
    const response = await this.request("request:loadTBRunDetails", { runId });
    if (!response.success) {
      throw new Error(response.error ?? "Failed to load run details");
    }
    return (response as Extract<SocketResponse, { type: "response:loadTBRunDetails" }>).data ?? null;
  }

  /**
   * Load ready tasks from .openagents/tasks.jsonl
   */
  async loadReadyTasks(limit?: number): Promise<MCTask[]> {
    const response = await this.request("request:loadReadyTasks", { limit });
    if (!response.success) {
      throw new Error(response.error ?? "Failed to load ready tasks");
    }
    return (response as Extract<SocketResponse, { type: "response:loadReadyTasks" }>).data ?? [];
  }

  // ============================================================================
  // Private Methods
  // ============================================================================

  private handleMessage(data: string): void {
    const parsed = parseSocketMessage(data);
    if (!parsed) {
      this.log(`Invalid message: ${data.slice(0, 100)}`);
      return;
    }

    // Handle responses to pending requests
    if (isSocketResponse(parsed)) {
      const pending = this.pendingRequests.get(parsed.correlationId);
      if (pending) {
        clearTimeout(pending.timeout);
        this.pendingRequests.delete(parsed.correlationId);
        pending.resolve(parsed);
      }
      return;
    }

    // Handle HUD events
    if (isHudEvent(parsed)) {
      for (const handler of this.messageHandlers) {
        try {
          handler(parsed as HudMessage);
        } catch (e) {
          console.error("[SocketClient] Message handler error:", e);
        }
      }
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;

    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
    this.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnectAttempts++;
      this.connect().catch(() => {});
    }, delay);
  }

  private log(msg: string): void {
    if (this.verbose) {
      console.log(`[SocketClient] ${msg}`);
    }
  }
}

// ============================================================================
// Factory Function
// ============================================================================

let defaultClient: SocketClient | null = null;

/**
 * Get or create the default socket client.
 */
export function getSocketClient(options?: SocketClientOptions): SocketClient {
  if (!defaultClient) {
    defaultClient = new SocketClient(options);
  }
  return defaultClient;
}

/**
 * Create and connect a socket client.
 */
export async function createSocketClient(options?: SocketClientOptions): Promise<SocketClient> {
  const client = new SocketClient(options);
  await client.connect();
  return client;
}
