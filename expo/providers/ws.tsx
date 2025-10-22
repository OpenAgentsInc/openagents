import React, { createContext, useCallback, useContext, useMemo, useRef, useState, useEffect } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';

type MsgHandler = ((text: string) => void) | null;

type Approvals = 'never' | 'on-request' | 'on-failure';

type WsContextValue = {
  wsUrl: string;
  setWsUrl: (v: string) => void;
  connected: boolean;
  connect: () => void;
  disconnect: () => void;
  send: (payload: string | ArrayBuffer | Blob) => boolean;
  setOnMessage: (fn: MsgHandler) => void;
  // Log controls (Console registers a handler; Settings can trigger)
  setClearLogHandler: (fn: (() => void) | null) => void;
  clearLog: () => void;
  // Permissions/preferences exposed to UI and sender
  readOnly: boolean;
  setReadOnly: (v: boolean) => void;
  networkEnabled: boolean;
  setNetworkEnabled: (v: boolean) => void;
  approvals: Approvals;
  setApprovals: (v: Approvals) => void;
  attachPreface: boolean;
  setAttachPreface: (v: boolean) => void;
};

const WsContext = createContext<WsContextValue | undefined>(undefined);

export function WsProvider({ children }: { children: React.ReactNode }) {
  const [wsUrl, setWsUrl] = useState('ws://localhost:8787/ws');
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const onMessageRef = useRef<MsgHandler>(null);
  const clearLogHandlerRef = useRef<(() => void) | null>(null);

  // Simple in-memory preferences (could persist with AsyncStorage)
  const [readOnly, setReadOnly] = useState(true);
  const [networkEnabled, setNetworkEnabled] = useState(false);
  const [approvals, setApprovals] = useState<Approvals>('never');
  const [attachPreface, setAttachPreface] = useState(true);

  // Persist & hydrate settings
  const SETTINGS_KEY = '@openagents/ws-settings';
  const hydratedRef = useRef(false);
  const [hydrated, setHydrated] = useState(false);
  const autoTriedRef = useRef(false);

  const disconnect = useCallback(() => {
    try { wsRef.current?.close(); } catch {}
    wsRef.current = null;
    setConnected(false);
  }, []);

  const connect = useCallback(() => {
    try { wsRef.current?.close(); } catch {}
    try {
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.onopen = () => {
        setConnected(true);
        // Suppress noisy connection status logs in the session feed.
      };
      ws.onmessage = (evt) => {
        const data = typeof evt.data === 'string' ? evt.data : String(evt.data);
        onMessageRef.current?.(data);
      };
      ws.onerror = (evt: any) => {
        // Keep errors out of the session feed; rely on header dot and dev console
      };
      ws.onclose = () => {
        setConnected(false);
      };
    } catch (e: any) {
      // Suppress connection error logs in the feed.
    }
  }, [wsUrl]);

  useEffect(() => {
    (async () => {
      try {
        const raw = await AsyncStorage.getItem(SETTINGS_KEY);
        if (raw) {
          const s = JSON.parse(raw);
          if (typeof s.wsUrl === 'string') setWsUrl(s.wsUrl);
          if (typeof s.readOnly === 'boolean') setReadOnly(s.readOnly);
          if (typeof s.networkEnabled === 'boolean') setNetworkEnabled(s.networkEnabled);
          if (s.approvals === 'never' || s.approvals === 'on-request' || s.approvals === 'on-failure') setApprovals(s.approvals);
          if (typeof s.attachPreface === 'boolean') setAttachPreface(s.attachPreface);
        }
      } catch {}
      hydratedRef.current = true;
      setHydrated(true);
    })();
  }, []);

  useEffect(() => {
    if (!hydratedRef.current) return;
    const payload = JSON.stringify({ wsUrl, readOnly, networkEnabled, approvals, attachPreface });
    AsyncStorage.setItem(SETTINGS_KEY, payload).catch(() => {});
  }, [wsUrl, readOnly, networkEnabled, approvals, attachPreface]);

  // Try a single auto-connect after hydration using the saved URL.
  useEffect(() => {
    if (!hydrated) return;
    if (autoTriedRef.current) return;
    if (connected || wsRef.current) return;
    autoTriedRef.current = true;
    // Best-effort: ignore errors; UI still has manual Connect.
    connect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, wsUrl]);

  // Auto-reconnect loop: every 3s when disconnected, try to connect.
  useEffect(() => {
    if (!hydrated) return;
    const interval = setInterval(() => {
      try {
        const ws = wsRef.current;
        if (connected) return;
        // Don't stomp an in-flight connection
        if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
        connect();
      } catch {}
    }, 3000);
    return () => clearInterval(interval);
  }, [connected, connect, hydrated, wsUrl]);

  useEffect(() => () => { try { wsRef.current?.close(); } catch {} }, []);

  const send = useCallback((payload: string | ArrayBuffer | Blob) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return false;
    ws.send(payload as any);
    return true;
  }, []);

  const setOnMessage = useCallback((fn: MsgHandler) => {
    onMessageRef.current = fn;
  }, []);

  const setClearLogHandler = useCallback((fn: (() => void) | null) => {
    clearLogHandlerRef.current = fn;
  }, []);

  const clearLog = useCallback(() => {
    try { clearLogHandlerRef.current?.(); } catch {}
  }, []);

  const value = useMemo(
    () => ({
      wsUrl,
      setWsUrl,
      connected,
      connect,
      disconnect,
      send,
      setOnMessage,
      setClearLogHandler,
      clearLog,
      readOnly,
      setReadOnly,
      networkEnabled,
      setNetworkEnabled,
      approvals,
      setApprovals,
      attachPreface,
      setAttachPreface,
    }),
    [wsUrl, connected, connect, disconnect, send, setOnMessage, readOnly, networkEnabled, approvals, attachPreface]
  );

  return <WsContext.Provider value={value}>{children}</WsContext.Provider>;
}

export function useWs() {
  const ctx = useContext(WsContext);
  if (!ctx) throw new Error('useWs must be used within WsProvider');
  return ctx;
}
