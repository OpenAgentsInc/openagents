// Use native EventSource in browser
import { Transport } from "./transport";
import { JSONRPCMessage, JSONRPCMessageSchema } from "./types";

// Use the native EventSource type
type EventSourceType = typeof EventSource;
const BrowserEventSource: EventSourceType = globalThis.EventSource;

export class SseError extends Error {
  constructor(
    public readonly code: number | undefined,
    message: string | undefined,
    public readonly event: any,
  ) {
    super(`SSE error: ${message}`);
  }
}

/**
 * Configuration options for the `SSEClientTransport`.
 */
export type SSEClientTransportOptions = {
  /**
   * Customizes the initial SSE request to the server (the request that begins the stream).
   */
  eventSourceInit?: any;

  /**
   * Customizes recurring POST requests to the server.
   */
  requestInit?: RequestInit;
};

/**
 * Client transport for SSE: this will connect to a server using Server-Sent Events for receiving
 * messages and make separate POST requests for sending messages.
 */
export class SSEClientTransport implements Transport {
  private _eventSource?: EventSource;
  private _endpoint?: URL;
  private _abortController?: AbortController;
  private _url: URL;
  private _eventSourceInit?: any;
  private _requestInit?: RequestInit;

  onclose?: () => void;
  onerror?: (error: Error) => void;
  onmessage?: (message: JSONRPCMessage) => void;

  constructor(
    url: URL,
    opts?: SSEClientTransportOptions,
  ) {
    this._url = url;
    this._eventSourceInit = opts?.eventSourceInit;
    this._requestInit = opts?.requestInit;
  }

  private _start(): Promise<void> {
    return new Promise((resolve, reject) => {
      this._eventSource = new BrowserEventSource(
        this._url.href,
        this._eventSourceInit
      );
      this._abortController = new AbortController();

      this._eventSource.onerror = (event: any) => {
        const error = new SseError(event.code, event.message, event);
        reject(error);
        this.onerror?.(error);
      };

      this._eventSource.onopen = () => {
        // The connection is open, but we need to wait for the endpoint to be received.
      };

      this._eventSource.addEventListener("endpoint", (event: Event) => {
        const messageEvent = event as MessageEvent;

        try {
          this._endpoint = new URL(messageEvent.data, this._url);
          if (this._endpoint.origin !== this._url.origin) {
            throw new Error(
              `Endpoint origin does not match connection origin: ${this._endpoint.origin}`,
            );
          }
        } catch (error) {
          reject(error);
          this.onerror?.(error as Error);

          void this.close();
          return;
        }

        resolve();
      });

      this._eventSource.onmessage = (event: Event) => {
        const messageEvent = event as MessageEvent;
        let message: JSONRPCMessage;
        try {
          message = JSONRPCMessageSchema.parse(JSON.parse(messageEvent.data));
        } catch (error) {
          this.onerror?.(error as Error);
          return;
        }

        this.onmessage?.(message);
      };
    });
  }

  async start() {
    if (this._eventSource) {
      throw new Error(
        "SSEClientTransport already started! If using Client class, note that connect() calls start() automatically.",
      );
    }

    return await this._start();
  }

  async close(): Promise<void> {
    this._abortController?.abort();
    this._eventSource?.close();
    this.onclose?.();
  }

  async send(message: JSONRPCMessage): Promise<void> {
    if (!this._endpoint) {
      throw new Error("Not connected");
    }

    try {
      const headers = new Headers(this._requestInit?.headers);
      headers.set("content-type", "application/json");
      const init = {
        ...this._requestInit,
        method: "POST",
        headers,
        body: JSON.stringify(message),
        signal: this._abortController?.signal,
      };

      const response = await fetch(this._endpoint, init);
      if (!response.ok) {
        const text = await response.text().catch(() => null);
        throw new Error(
          `Error POSTing to endpoint (HTTP ${response.status}): ${text}`,
        );
      }
    } catch (error) {
      this.onerror?.(error as Error);
      throw error;
    }
  }
}
