/**
 * Singleton WebSocket connection for tinyvex
 *
 * Ensures only one WebSocket connection exists across all runtimes,
 * preventing duplicate connections during hot-reload.
 */

import { useEffect, useMemo, useState } from "react";
import { DEFAULT_TINYVEX_WS_URL } from "@/config/acp";
import type { TinyvexWebSocketHandle, TinyvexMessage } from "./useTinyvexWebSocket";

// Global singleton WebSocket instance
let globalSocket: WebSocket | null = null;
let globalConnected = false;
let globalConnecting = false;
let subscribers = new Set<(msg: TinyvexMessage) => void>();
let stateListeners = new Set<() => void>();
let refCount = 0;
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;

// Current WebSocket URL (can be dynamically changed for mobile discovery)
let currentWsUrl = DEFAULT_TINYVEX_WS_URL;

function notifyStateChange() {
  stateListeners.forEach((listener) => listener());
}

function connect(url?: string) {
  if (url) {
    currentWsUrl = url;
  }

  if (globalSocket?.readyState === WebSocket.OPEN || globalConnecting) {
    return;
  }

  console.log("[tinyvex-singleton] Connecting to", currentWsUrl);
  globalConnecting = true;
  notifyStateChange();

  const ws = new WebSocket(currentWsUrl);

  ws.onopen = () => {
    console.log("[tinyvex-singleton] Connected");
    globalSocket = ws;
    globalConnected = true;
    globalConnecting = false;
    notifyStateChange();
  };

  ws.onclose = () => {
    console.log("[tinyvex-singleton] Disconnected");
    globalSocket = null;
    globalConnected = false;
    globalConnecting = false;
    notifyStateChange();

    // Auto-reconnect if there are still subscribers
    if (refCount > 0) {
      console.log("[tinyvex-singleton] Reconnecting in 2s");
      reconnectTimeout = setTimeout(connect, 2000);
    }
  };

  ws.onerror = (evt) => {
    console.error("[tinyvex-singleton] Error:", evt);
    globalConnecting = false;
    notifyStateChange();
  };

  ws.onmessage = (evt) => {
    try {
      const msg: TinyvexMessage = JSON.parse(evt.data);
      subscribers.forEach((callback) => {
        try {
          callback(msg);
        } catch (err) {
          console.error("[tinyvex-singleton] Subscriber error:", err);
        }
      });
    } catch (err) {
      console.error("[tinyvex-singleton] Failed to parse message:", evt.data, err);
    }
  };
}

function disconnect() {
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
  }
  if (globalSocket) {
    globalSocket.close();
    globalSocket = null;
    globalConnected = false;
  }
}

function send(msg: object) {
  if (!globalSocket || globalSocket.readyState !== WebSocket.OPEN) {
    console.warn("[tinyvex-singleton] Cannot send, socket not open:", msg);
    return;
  }
  globalSocket.send(JSON.stringify(msg));
}

function subscribe(callback: (msg: TinyvexMessage) => void) {
  subscribers.add(callback);
  return () => {
    subscribers.delete(callback);
  };
}

/**
 * Set the WebSocket URL (useful for mobile server discovery)
 *
 * Call this before connecting to override the default URL.
 * If already connected, will disconnect and reconnect with the new URL.
 */
export function setWebSocketUrl(url: string) {
  if (currentWsUrl === url) {
    return; // No change
  }

  console.log("[tinyvex-singleton] Changing WebSocket URL from", currentWsUrl, "to", url);
  currentWsUrl = url;

  // If currently connected, reconnect with new URL
  if (globalSocket || globalConnecting) {
    disconnect();
    if (refCount > 0) {
      connect(url);
    }
  }
}

/**
 * Get the current WebSocket URL
 */
export function getWebSocketUrl(): string {
  return currentWsUrl;
}

/**
 * Hook that returns a shared WebSocket connection
 *
 * Uses reference counting to ensure the connection stays alive
 * as long as at least one component is using it.
 *
 * @param url Optional WebSocket URL (for mobile server discovery)
 */
export function useSharedTinyvexWebSocket(url?: string): TinyvexWebSocketHandle {
  const [, forceUpdate] = useState(0);

  useEffect(() => {
    // Update URL if provided
    if (url && url !== currentWsUrl) {
      setWebSocketUrl(url);
    }

    refCount++;
    console.log("[tinyvex-singleton] Mount, refCount:", refCount);

    if (refCount === 1) {
      connect(url);
    }

    // Listen for state changes and re-render
    const listener = () => forceUpdate((n) => n + 1);
    stateListeners.add(listener);

    return () => {
      stateListeners.delete(listener);
      refCount--;
      console.log("[tinyvex-singleton] Unmount, refCount:", refCount);

      if (refCount === 0) {
        disconnect();
        console.log("[tinyvex-singleton] Last subscriber, disconnecting");
      }
    };
  }, []);

  const handle: TinyvexWebSocketHandle = useMemo(
    () => ({
      socket: globalSocket,
      connected: globalConnected,
      connecting: globalConnecting,
      error: null,
      send,
      subscribe,
      connect,
      disconnect,
    }),
    [globalConnected, globalConnecting]
  );

  return handle;
}
