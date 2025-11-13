/**
 * Singleton WebSocket connection for tinyvex
 *
 * Ensures only one WebSocket connection exists across all runtimes,
 * preventing duplicate connections during hot-reload.
 */

import { useEffect, useMemo, useState } from "react";
import { TINYVEX_WS_URL } from "@/config/acp";
import type { TinyvexWebSocketHandle, TinyvexMessage } from "./useTinyvexWebSocket";

// Global singleton WebSocket instance
let globalSocket: WebSocket | null = null;
let globalConnected = false;
let globalConnecting = false;
let subscribers = new Set<(msg: TinyvexMessage) => void>();
let stateListeners = new Set<() => void>();
let refCount = 0;
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;

function notifyStateChange() {
  stateListeners.forEach((listener) => listener());
}

function connect() {
  if (globalSocket?.readyState === WebSocket.OPEN || globalConnecting) {
    return;
  }

  console.log("[tinyvex-singleton] Connecting to", TINYVEX_WS_URL);
  globalConnecting = true;
  notifyStateChange();

  const ws = new WebSocket(TINYVEX_WS_URL);

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
 * Hook that returns a shared WebSocket connection
 *
 * Uses reference counting to ensure the connection stays alive
 * as long as at least one component is using it.
 */
export function useSharedTinyvexWebSocket(): TinyvexWebSocketHandle {
  const [, forceUpdate] = useState(0);

  useEffect(() => {
    refCount++;
    console.log("[tinyvex-singleton] Mount, refCount:", refCount);

    if (refCount === 1) {
      connect();
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
