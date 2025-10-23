import React, { createContext, useCallback, useContext, useMemo, useRef, useState, useEffect } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { appLog } from '@/lib/app-log';

type MsgHandler = ((text: string) => void) | null;

type Approvals = 'never' | 'on-request' | 'on-failure';

type BridgeContextValue = {
  bridgeHost: string; // e.g., "localhost:8787" or "100.x.x.x:8787"
  setBridgeHost: (v: string) => void;
  wsUrl: string;   // derived: ws://<bridgeHost>/ws
  httpBase: string; // derived: http://<bridgeHost>
  connected: boolean;
  connect: () => void;
  disconnect: () => void;
  send: (payload: string | ArrayBuffer | Blob) => boolean;
  setOnMessage: (fn: MsgHandler) => void;
  addSubscriber: (fn: MsgHandler) => () => void;
  // WS request helpers (no HTTP)
  requestHistory: () => Promise<any[]>;
  requestThread: (id: string, path?: string) => Promise<any | undefined>;
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
  // Next-send resume hook: if set, projects provider will include { resume: <id> }
  resumeNextId: string | null;
  setResumeNextId: (id: string | null) => void;
};

const BridgeContext = createContext<BridgeContextValue | undefined>(undefined);

export function BridgeProvider({ children }: { children: React.ReactNode }) {
  const [bridgeHost, setBridgeHost] = useState('localhost:8787');
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const onMessageRef = useRef<MsgHandler>(null);
  const pendingBufferRef = useRef<string[]>([]); // buffer chunks when no handler is attached
  const subsRef = useRef<Set<MsgHandler>>(new Set());
  const clearLogHandlerRef = useRef<(() => void) | null>(null);

  // Simple in-memory preferences (could persist with AsyncStorage)
  const [readOnly, setReadOnly] = useState(false);
  const [networkEnabled, setNetworkEnabled] = useState(true);
  const [approvals, setApprovals] = useState<Approvals>('never');
  const [attachPreface, setAttachPreface] = useState(true);
  const [resumeNextId, setResumeNextId] = useState<string | null>(null);

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
      const wsUrl = `ws://${bridgeHost}/ws`;
      const httpBase = `http://${bridgeHost}`;
      appLog('bridge.connect', { wsUrl, httpBase });
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.onopen = () => {
        setConnected(true);
        appLog('bridge.open');
        // Suppress noisy connection status logs in the session feed.
      };
      ws.onmessage = (evt) => {
        const data = typeof evt.data === 'string' ? evt.data : String(evt.data);
        // Always buffer so re-attaching consumers can catch up.
        try {
          const buf = pendingBufferRef.current;
          buf.push(data);
          // Trim buffer to last 800 chunks to avoid unbounded growth
          if (buf.length > 800) buf.splice(0, buf.length - 800);
        } catch {}
        // Deliver to active consumer if present
        try { onMessageRef.current?.(data); } catch {}
        // Deliver to passive subscribers
        try { subsRef.current.forEach((fn) => { try { if (fn) fn(data) } catch {} }); } catch {}
      };
      ws.onerror = (evt: any) => {
        // Keep errors out of the session feed; rely on header dot and dev console
      };
      ws.onclose = () => {
        setConnected(false);
        appLog('bridge.close');
      };
    } catch (e: any) {
      // Suppress connection error logs in the feed.
      appLog('bridge.connect.error', { error: String(e?.message ?? e) }, 'error');
    }
  }, [bridgeHost]);

  // Helper: wait until the WebSocket is OPEN (or time out)
  const awaitConnected = useCallback(async (timeoutMs: number = 8000) => {
    const start = Date.now();
    // Kick a connect attempt if nothing is connected
    try { if (!wsRef.current || wsRef.current.readyState === WebSocket.CLOSED) connect(); } catch {}
    return new Promise<void>((resolve, reject) => {
      const tick = () => {
        try {
          const ws = wsRef.current;
          if (ws && ws.readyState === WebSocket.OPEN) { resolve(); return; }
        } catch {}
        if (Date.now() - start >= timeoutMs) { reject(new Error('ws not connected')); return; }
        setTimeout(tick, 150);
      };
      tick();
    });
  }, [connect]);

  useEffect(() => {
    (async () => {
      try {
        const raw = await AsyncStorage.getItem(SETTINGS_KEY);
        if (raw) {
          const s = JSON.parse(raw);
          if (typeof s.bridgeHost === 'string') setBridgeHost(s.bridgeHost);
          // Back-compat: migrate old wsUrl -> bridgeHost
          if (!s.bridgeHost && typeof s.wsUrl === 'string') {
            let migrated: string | null = null;
            try {
              const u = new URL(s.wsUrl);
              if (u.host) migrated = u.host;
            } catch {
              // Fallback: accept raw host:port or ws://host:port[/ws]
              try {
                const rawUrl = String(s.wsUrl).trim();
                const stripped = rawUrl
                  .replace(/^ws:\/\//i, '')
                  .replace(/^wss:\/\//i, '')
                  .replace(/^http:\/\//i, '')
                  .replace(/^https:\/\//i, '')
                  .replace(/\/ws$/i, '')
                  .replace(/\/$/, '');
                if (stripped.includes(':') || /^[\d.]+$/.test(stripped)) {
                  migrated = stripped;
                }
              } catch {}
            }
            if (migrated) setBridgeHost(migrated);
          }
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
    const payload = JSON.stringify({ bridgeHost, readOnly, networkEnabled, approvals, attachPreface });
    AsyncStorage.setItem(SETTINGS_KEY, payload).catch(() => {});
  }, [bridgeHost, readOnly, networkEnabled, approvals, attachPreface]);

  // Try a single auto-connect after hydration using the saved URL.
  useEffect(() => {
    if (!hydrated) return;
    if (autoTriedRef.current) return;
    if (connected || wsRef.current) return;
    autoTriedRef.current = true;
    // Best-effort: ignore errors; UI still has manual Connect.
    connect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, bridgeHost]);

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
  }, [connected, connect, hydrated, bridgeHost]);

  useEffect(() => () => { try { wsRef.current?.close(); } catch {} }, []);

  const send = useCallback((payload: string | ArrayBuffer | Blob) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return false;
    ws.send(payload as any);
    return true;
  }, []);

  const setOnMessage = useCallback((fn: MsgHandler) => {
    onMessageRef.current = fn;
    // If a consumer attached, synchronously flush any buffered chunks so UI state updates
    if (fn) {
      const buf = pendingBufferRef.current;
      if (buf.length > 0) {
        try {
          for (const chunk of buf) { try { fn(chunk) } catch {} }
        } finally {
          pendingBufferRef.current = [];
        }
      }
    }
  }, []);

  const addSubscriber = useCallback((fn: MsgHandler) => {
    subsRef.current.add(fn);
    return () => { try { subsRef.current.delete(fn) } catch {} };
  }, []);

  // WS helpers
  const requestHistory = useCallback(async (): Promise<any[]> => {
    await awaitConnected().catch((e) => { throw e });
    // Subscribe once
    return new Promise<any[]>((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 30000);
      const unsub = addSubscriber((line) => {
        if (done) return;
        const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try {
          const obj = JSON.parse(s);
          if (obj?.type === 'bridge.history' && Array.isArray(obj.items)) {
            done = true; clearTimeout(timer); unsub(); resolve(obj.items);
          }
        } catch {}
      });
      const ok = send(JSON.stringify({ control: 'history' }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const requestThread = useCallback(async (id: string, path?: string): Promise<any | undefined> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise<any | undefined>((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 20000);
      const unsub = addSubscriber((line) => {
        if (done) return;
        const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try {
          const obj = JSON.parse(s);
          if (obj?.type === 'bridge.thread' && obj?.thread) {
            done = true; clearTimeout(timer); unsub(); resolve(obj.thread);
          }
        } catch {}
      });
      const ok = send(JSON.stringify({ control: 'thread', id, path }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const setClearLogHandler = useCallback((fn: (() => void) | null) => {
    clearLogHandlerRef.current = fn;
  }, []);

  const clearLog = useCallback(() => {
    try { clearLogHandlerRef.current?.(); } catch {}
  }, []);

  const value = useMemo(
    () => ({
      bridgeHost,
      setBridgeHost,
      wsUrl: `ws://${bridgeHost}/ws`,
      httpBase: `http://${bridgeHost}`,
      connected,
      connect,
      disconnect,
      send,
      setOnMessage,
      addSubscriber,
      setClearLogHandler,
      clearLog,
      requestHistory,
      requestThread,
      readOnly,
      setReadOnly,
      networkEnabled,
      setNetworkEnabled,
      approvals,
      setApprovals,
      attachPreface,
      setAttachPreface,
      resumeNextId,
      setResumeNextId,
    }),
    [bridgeHost, connected, connect, disconnect, send, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, resumeNextId]
  );

  return <BridgeContext.Provider value={value}>{children}</BridgeContext.Provider>;
}

export function useBridge() {
  const ctx = useContext(BridgeContext);
  if (!ctx) throw new Error('useBridge must be used within BridgeProvider');
  return ctx;
}

// Back-compat exports (deprecated)
export const WsProvider = BridgeProvider;
export const useWs = useBridge;
