/**
 * Core WebSocket connection hook for tinyvex sync engine
 *
 * Provides persistent WebSocket connection with auto-reconnect,
 * message broadcasting, and connection state management.
 */

import { useEffect, useRef, useState, useCallback } from "react";

export interface TinyvexMessage {
  type: string;
  [key: string]: any;
}

export interface UseTinyvexWebSocketOptions {
  url?: string;
  autoConnect?: boolean;
  reconnectDelay?: number;
  onMessage?: (msg: TinyvexMessage) => void;
  onError?: (error: Event) => void;
}

export interface TinyvexWebSocketHandle {
  socket: WebSocket | null;
  connected: boolean;
  connecting: boolean;
  error: Event | null;
  send: (msg: object) => void;
  subscribe: (callback: (msg: TinyvexMessage) => void) => () => void;
  connect: () => void;
  disconnect: () => void;
}

/**
 * Hook to manage WebSocket connection to tinyvex server
 *
 * @example
 * ```tsx
 * const ws = useTinyvexWebSocket({
 *   url: 'ws://localhost:9099/ws',
 *   onMessage: (msg) => console.log('Received:', msg),
 * });
 *
 * // Send a query
 * ws.send({ control: 'tvx.query', name: 'threads.list', args: {} });
 * ```
 */
export function useTinyvexWebSocket(
  options: UseTinyvexWebSocketOptions = {}
): TinyvexWebSocketHandle {
  const {
    url = "ws://127.0.0.1:9099/ws",
    autoConnect = true,
    reconnectDelay = 2000,
    onMessage: globalOnMessage,
    onError: globalOnError,
  } = options;

  const [socket, setSocket] = useState<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<Event | null>(null);

  const subscribersRef = useRef<Set<(msg: TinyvexMessage) => void>>(new Set());
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const shouldConnectRef = useRef(autoConnect);

  const connect = useCallback(() => {
    if (socket?.readyState === WebSocket.OPEN || connecting) {
      return;
    }

    console.log(`[tinyvex-ws] Connecting to ${url}`);
    setConnecting(true);
    setError(null);

    const ws = new WebSocket(url);

    ws.onopen = () => {
      console.log("[tinyvex-ws] Connected");
      setSocket(ws);
      setConnected(true);
      setConnecting(false);
      setError(null);
    };

    ws.onclose = () => {
      console.log("[tinyvex-ws] Disconnected");
      setSocket(null);
      setConnected(false);
      setConnecting(false);

      // Auto-reconnect if should be connected
      if (shouldConnectRef.current) {
        console.log(`[tinyvex-ws] Reconnecting in ${reconnectDelay}ms`);
        reconnectTimeoutRef.current = setTimeout(() => {
          connect();
        }, reconnectDelay);
      }
    };

    ws.onerror = (evt) => {
      console.error("[tinyvex-ws] Error:", evt);
      setError(evt);
      setConnecting(false);
      globalOnError?.(evt);
    };

    ws.onmessage = (evt) => {
      try {
        const msg: TinyvexMessage = JSON.parse(evt.data);

        // Broadcast to global handler
        globalOnMessage?.(msg);

        // Broadcast to all subscribers
        subscribersRef.current.forEach((callback) => {
          try {
            callback(msg);
          } catch (err) {
            console.error("[tinyvex-ws] Subscriber error:", err);
          }
        });
      } catch (err) {
        console.error("[tinyvex-ws] Failed to parse message:", evt.data, err);
      }
    };

    setSocket(ws);
  }, [url, reconnectDelay, globalOnMessage, globalOnError, connecting, socket]);

  const disconnect = useCallback(() => {
    shouldConnectRef.current = false;
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
    }
    if (socket) {
      socket.close();
      setSocket(null);
      setConnected(false);
    }
  }, [socket]);

  const send = useCallback(
    (msg: object) => {
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        console.warn("[tinyvex-ws] Cannot send, socket not open:", msg);
        return;
      }
      socket.send(JSON.stringify(msg));
    },
    [socket]
  );

  const subscribe = useCallback(
    (callback: (msg: TinyvexMessage) => void) => {
      subscribersRef.current.add(callback);
      return () => {
        subscribersRef.current.delete(callback);
      };
    },
    []
  );

  // Auto-connect on mount if enabled
  useEffect(() => {
    if (autoConnect) {
      shouldConnectRef.current = true;
      connect();
    }

    return () => {
      shouldConnectRef.current = false;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (socket) {
        socket.close();
      }
    };
    // Only run on mount/unmount, not when callbacks change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    socket,
    connected,
    connecting,
    error,
    send,
    subscribe,
    connect,
    disconnect,
  };
}
