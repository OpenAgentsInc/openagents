/**
 * HUD WebSocket Client
 *
 * Connects from the agent process to the Electrobun HUD WebSocket server.
 *
 * Features:
 * - Auto-reconnect with exponential backoff
 * - Message queue when disconnected (messages sent when reconnected)
 * - Silent failure if HUD not running (agent continues working)
 *
 * Usage:
 *   const client = new HudClient();
 *   client.send({ type: "session_start", sessionId: "...", timestamp: "..." });
 *   // ... later
 *   client.close();
 */

import type { HudMessage } from "./protocol.js";
import { HUD_WS_URL, serializeHudMessage } from "./protocol.js";

export interface HudClientOptions {
  /** WebSocket URL (default: ws://localhost:8080/ws) */
  url?: string;
  /** Max queue size before dropping oldest messages (default: 1000) */
  maxQueueSize?: number;
  /** Reconnect interval in ms (default: 2000) */
  reconnectInterval?: number;
  /** Max reconnect attempts before giving up (default: 10, 0 = infinite) */
  maxReconnectAttempts?: number;
  /** Whether to enable verbose logging (default: false) */
  verbose?: boolean;
}

type ConnectionState = "disconnected" | "connecting" | "connected";

export class HudClient {
  private ws: WebSocket | null = null;
  private readonly url: string;
  private readonly maxQueueSize: number;
  private readonly reconnectInterval: number;
  private readonly maxReconnectAttempts: number;
  private readonly verbose: boolean;

  private state: ConnectionState = "disconnected";
  private queue: HudMessage[] = [];
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionalClose = false;

  constructor(options: HudClientOptions = {}) {
    this.url = options.url ?? HUD_WS_URL;
    this.maxQueueSize = options.maxQueueSize ?? 1000;
    this.reconnectInterval = options.reconnectInterval ?? 2000;
    this.maxReconnectAttempts = options.maxReconnectAttempts ?? 10;
    this.verbose = options.verbose ?? false;

    // Attempt initial connection
    this.connect();
  }

  /**
   * Send a message to the HUD.
   * If disconnected, the message is queued and sent when reconnected.
   */
  send(message: HudMessage): void {
    if (this.state === "connected" && this.ws?.readyState === WebSocket.OPEN) {
      this.doSend(message);
    } else {
      this.enqueue(message);
    }
  }

  /**
   * Close the connection and stop reconnecting.
   */
  close(): void {
    this.intentionalClose = true;
    this.clearReconnectTimer();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.state = "disconnected";
  }

  /**
   * Get current connection state
   */
  getState(): ConnectionState {
    return this.state;
  }

  /**
   * Get current queue size
   */
  getQueueSize(): number {
    return this.queue.length;
  }

  // ============================================================================
  // Private Methods
  // ============================================================================

  private connect(): void {
    if (this.intentionalClose) return;
    if (this.state === "connecting" || this.state === "connected") return;

    this.state = "connecting";
    this.log("Connecting to HUD...");

    try {
      this.ws = new WebSocket(this.url);

      this.ws.onopen = () => {
        this.state = "connected";
        this.reconnectAttempts = 0;
        this.log("Connected to HUD");
        this.flushQueue();
      };

      this.ws.onclose = () => {
        this.state = "disconnected";
        this.ws = null;
        if (!this.intentionalClose) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = () => {
        // Error event is always followed by close event
        // Don't log here to avoid noise when HUD isn't running
      };
    } catch {
      // WebSocket constructor can throw if URL is invalid
      this.state = "disconnected";
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    if (this.intentionalClose) return;
    if (this.reconnectTimer) return;

    if (this.maxReconnectAttempts > 0 && this.reconnectAttempts >= this.maxReconnectAttempts) {
      this.log("Max reconnect attempts reached, giving up");
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectInterval * Math.min(this.reconnectAttempts, 5);
    this.log(`Scheduling reconnect in ${delay}ms (attempt ${this.reconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private doSend(message: HudMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    try {
      this.ws.send(serializeHudMessage(message));
    } catch (error) {
      // If send fails, queue the message for retry
      this.enqueue(message);
    }
  }

  private enqueue(message: HudMessage): void {
    if (this.queue.length >= this.maxQueueSize) {
      // Drop oldest message to make room
      this.queue.shift();
    }
    this.queue.push(message);
  }

  private flushQueue(): void {
    while (this.queue.length > 0 && this.state === "connected" && this.ws?.readyState === WebSocket.OPEN) {
      const message = this.queue.shift();
      if (message) {
        this.doSend(message);
      }
    }
  }

  private log(msg: string): void {
    if (this.verbose) {
      console.log(`[HudClient] ${msg}`);
    }
  }
}

// ============================================================================
// Singleton for Convenience
// ============================================================================

let defaultClient: HudClient | null = null;

/**
 * Get or create the default HudClient singleton.
 */
export const getHudClient = (options?: HudClientOptions): HudClient => {
  if (!defaultClient) {
    defaultClient = new HudClient(options);
  }
  return defaultClient;
};

/**
 * Send a message using the default client.
 */
export const sendToHud = (message: HudMessage): void => {
  getHudClient().send(message);
};

/**
 * Close the default client.
 */
export const closeHudClient = (): void => {
  if (defaultClient) {
    defaultClient.close();
    defaultClient = null;
  }
};
