import React, { createContext, useCallback, useContext, useMemo, useRef, useState, useEffect } from 'react';
import { appLog } from '@/lib/app-log';
import { useSettings, type Approvals as StoreApprovals } from '@/lib/settings-store'

type MsgHandler = ((text: string) => void) | null;

type Approvals = StoreApprovals;

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
  requestHistory: (params?: { limit?: number; since_mtime?: number }) => Promise<any[]>;
  requestThread: (id: string, path?: string) => Promise<any | undefined>;
  requestProjects: () => Promise<any[]>;
  requestSkills: () => Promise<any[]>;
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
  const bridgeHost = useSettings((s) => s.bridgeHost)
  const setBridgeHost = useSettings((s) => s.setBridgeHost)
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const onMessageRef = useRef<MsgHandler>(null);
  const pendingBufferRef = useRef<string[]>([]); // buffer chunks when no handler is attached
  const subsRef = useRef<Set<MsgHandler>>(new Set());
  const clearLogHandlerRef = useRef<(() => void) | null>(null);

  // Persisted settings via Zustand store
  const readOnly = useSettings((s) => s.readOnly)
  const setReadOnly = useSettings((s) => s.setReadOnly)
  const networkEnabled = useSettings((s) => s.networkEnabled)
  const setNetworkEnabled = useSettings((s) => s.setNetworkEnabled)
  const approvals = useSettings((s) => s.approvals)
  const setApprovals = useSettings((s) => s.setApprovals)
  const attachPreface = useSettings((s) => s.attachPreface)
  const setAttachPreface = useSettings((s) => s.setAttachPreface)
  const [resumeNextId, setResumeNextId] = useState<string | null>(null);

  // Hydration is handled by the Zustand settings store; treat as ready
  const [hydrated] = useState(true);
  const autoTriedRef = useRef(false);

  // Normalize any legacy or malformed host values (e.g., ws://host:port/ws)
  const sanitizeHost = useCallback((raw: string): string => {
    try {
      const val = String(raw || '').trim();
      if (!val) return 'localhost:8787';
      const stripped = val
        .replace(/^ws:\/\//i, '')
        .replace(/^wss:\/\//i, '')
        .replace(/^http:\/\//i, '')
        .replace(/^https:\/\//i, '')
        .replace(/\/$/, '')
        .replace(/\/ws$/i, '')
        .replace(/\/$/, '');
      return stripped || 'localhost:8787';
    } catch {
      return 'localhost:8787';
    }
  }, []);

  const effectiveHost = sanitizeHost(bridgeHost);

  const disconnect = useCallback(() => {
    try { wsRef.current?.close(); } catch {}
    wsRef.current = null;
    setConnected(false);
  }, []);

  const connect = useCallback(() => {
    try { wsRef.current?.close(); } catch {}
    try {
      const wsUrl = `ws://${effectiveHost}/ws`;
      const httpBase = `http://${effectiveHost}`;
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
  }, [effectiveHost]);

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
  }, [connected, connect, hydrated, effectiveHost]);

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
  const requestHistory = useCallback(async (params?: { limit?: number; since_mtime?: number }): Promise<any[]> => {
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
      const payload = { control: 'history', ...(params?.limit ? { limit: params.limit } : {}), ...(params?.since_mtime ? { since_mtime: params.since_mtime } : {}) } as any;
      const ok = send(JSON.stringify(payload));
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

  const requestProjects = useCallback(async (): Promise<any[]> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise<any[]>((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 15000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try {
          const obj = JSON.parse(s);
          if (obj?.type === 'bridge.projects' && Array.isArray(obj.items)) { done = true; clearTimeout(timer); unsub(); resolve(obj.items); }
        } catch {}
      });
      const ok = send(JSON.stringify({ control: 'projects' }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const requestSkills = useCallback(async (): Promise<any[]> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise<any[]>((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 15000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try {
          const obj = JSON.parse(s);
          if (obj?.type === 'bridge.skills' && Array.isArray(obj.items)) { done = true; clearTimeout(timer); unsub(); resolve(obj.items); }
        } catch {}
      });
      const ok = send(JSON.stringify({ control: 'skills' }));
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
      bridgeHost, // expose raw for UI editing; connect uses effectiveHost
      setBridgeHost,
      wsUrl: `ws://${effectiveHost}/ws`,
      httpBase: `http://${effectiveHost}`,
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
      requestProjects,
      requestSkills,
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
    [bridgeHost, effectiveHost, connected, connect, disconnect, send, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, resumeNextId]
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
