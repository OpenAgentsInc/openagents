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
  connecting: boolean;
  connect: () => void;
  disconnect: () => void;
  send: (payload: string | ArrayBuffer | Blob) => boolean;
  setOnMessage: (fn: MsgHandler) => void;
  addSubscriber: (fn: MsgHandler) => () => void;
  // WS request helpers (deprecated in Convex-only flow)
  requestProjects: () => Promise<any[]>;
  requestConvexStatus: () => Promise<{ healthy: boolean; url: string; db: string; tables: string[] }>;
  createConvexDemo: () => Promise<{ healthy: boolean; url: string; db: string; tables: string[] }>;
  createConvexThreads: () => Promise<{ healthy: boolean; url: string; db: string; tables: string[] }>;
  createConvexDemoThread: () => Promise<{ healthy: boolean; url: string; db: string; tables: string[] }>;
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
  const [connecting, setConnecting] = useState(false);
  const autoReconnect = useSettings((s) => s.bridgeAutoReconnect)
  const setAutoReconnect = useSettings((s) => s.setBridgeAutoReconnect)
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
      if (!val) return '';
      const stripped = val
        .replace(/^ws:\/\//i, '')
        .replace(/^wss:\/\//i, '')
        .replace(/^http:\/\//i, '')
        .replace(/^https:\/\//i, '')
        .replace(/\/$/, '')
        .replace(/\/ws$/i, '')
        .replace(/\/$/, '');
      return stripped || '';
    } catch {
      return '';
    }
  }, []);

  const effectiveHost = sanitizeHost(bridgeHost);

  const disconnect = useCallback(() => {
    setAutoReconnect(false);
    try { wsRef.current?.close(); } catch {}
    wsRef.current = null;
    setConnected(false);
    setConnecting(false);
  }, []);

  const connect = useCallback(() => {
    setAutoReconnect(true);
    if (!effectiveHost) {
      // No host configured; do not attempt to connect
      return;
    }
    try { wsRef.current?.close(); } catch {}
    try {
      const wsUrl = `ws://${effectiveHost}/ws`;
      const httpBase = `http://${effectiveHost}`;
      appLog('bridge.connect', { wsUrl, httpBase });
      // Keep connection logs minimal
      try { console.log('[bridge.ws] connect', wsUrl) } catch {}
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      setConnecting(true);
      ws.onopen = () => {
        setConnected(true);
        setConnecting(false);
        appLog('bridge.open');
        try { console.log('[bridge.ws] open') } catch {}
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
      ws.onerror = (_evt: any) => {
        // Suppress noisy error prints; UI shows disconnected state
        setConnecting(false);
      };
      ws.onclose = () => {
        setConnected(false);
        setConnecting(false);
        appLog('bridge.close');
      };
    } catch (e: any) {
      // Suppress connection error logs in the feed.
      appLog('bridge.connect.error', { error: String(e?.message ?? e) }, 'error');
      try { console.log('[bridge.ws] connect.error', String(e?.message ?? e)) } catch {}
    }
  }, [effectiveHost]);

  // Helper: wait until the WebSocket is OPEN (or time out)
  const awaitConnected = useCallback(async (timeoutMs: number = 8000) => {
    if (!effectiveHost) throw new Error('ws host not configured');
    if (!autoReconnect) throw new Error('ws auto-reconnect paused');
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
  }, [connect, effectiveHost, autoReconnect]);

  // Try a single auto-connect after hydration using the saved URL.
  useEffect(() => {
    if (!hydrated) return;
    if (autoTriedRef.current) return;
    if (connected || wsRef.current) return;
    if (!effectiveHost) return;
    if (!autoReconnect) return;
    autoTriedRef.current = true;
    // Best-effort: ignore errors; UI still has manual Connect.
    connect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, bridgeHost, effectiveHost, autoReconnect]);

  // Auto-reconnect loop: every 3s when disconnected, try to connect.
  useEffect(() => {
    if (!hydrated) return;
    const interval = setInterval(() => {
      try {
        const ws = wsRef.current;
        if (connected) return;
        if (!effectiveHost) return; // do not auto-reconnect when host is cleared
        if (!autoReconnect) return; // paused by user via disconnect
        // Don't stomp an in-flight connection
        if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
        connect();
      } catch {}
    }, 3000);
    return () => clearInterval(interval);
  }, [connected, connect, hydrated, effectiveHost, autoReconnect]);

  useEffect(() => () => { try { wsRef.current?.close(); } catch {} }, []);

  const send = useCallback((payload: string | ArrayBuffer | Blob) => {
    const ws = wsRef.current;
    const preview = (() => {
      try {
        const s = typeof payload === 'string' ? payload : String(payload as any);
        return s.length > 160 ? s.slice(0, 160) + '…' : s;
      } catch { return '' }
    })();
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      try { appLog('ws.send.skipped', { preview, reason: 'not_open' }); } catch {}
      return false;
    }
    try { ws.send(payload as any); appLog('ws.send', { preview }); } catch (e: any) { appLog('ws.send.error', { error: String(e?.message ?? e) }, 'error'); return false; }
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

  // Removed history/thread helpers; Convex-only UI uses subscriptions directly

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

  const requestConvexStatus = useCallback(async (): Promise<any> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 8000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try { const obj = JSON.parse(s); if (obj?.type === 'bridge.convex_status') { done = true; clearTimeout(timer); unsub(); resolve(obj); } } catch {}
      });
      const ok = send(JSON.stringify({ control: 'convex.status' }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const createConvexDemo = useCallback(async (): Promise<any> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 10000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try { const obj = JSON.parse(s); if (obj?.type === 'bridge.convex_status') { done = true; clearTimeout(timer); unsub(); resolve(obj); } } catch {}
      });
      const ok = send(JSON.stringify({ control: 'convex.create_demo' }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const createConvexThreads = useCallback(async (): Promise<any> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 10000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try { const obj = JSON.parse(s); if (obj?.type === 'bridge.convex_status') { done = true; clearTimeout(timer); unsub(); resolve(obj); } } catch {}
      });
      const ok = send(JSON.stringify({ control: 'convex.create_threads' }));
      if (!ok) { clearTimeout(timer); unsub(); reject(new Error('ws not connected')); }
    });
  }, [addSubscriber, send, awaitConnected]);

  const createConvexDemoThread = useCallback(async (): Promise<any> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise((resolve, reject) => {
      let done = false;
      const timer = setTimeout(() => { if (!done) { done = true; reject(new Error('timeout')); unsub(); } }, 10000);
      const unsub = addSubscriber((line) => {
        if (done) return; const s = String(line || '').trim(); if (!s.startsWith('{')) return;
        try { const obj = JSON.parse(s); if (obj?.type === 'bridge.convex_status') { done = true; clearTimeout(timer); unsub(); resolve(obj); } } catch {}
      });
      const ok = send(JSON.stringify({ control: 'convex.create_demo_thread' }));
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
      wsUrl: effectiveHost ? `ws://${effectiveHost}/ws` : '',
      httpBase: effectiveHost ? `http://${effectiveHost}` : '',
      connected,
      connecting,
      connect,
      disconnect,
      send,
      setOnMessage,
      addSubscriber,
      setClearLogHandler,
      clearLog,
      
      requestProjects,
      requestConvexStatus,
      createConvexDemo,
      createConvexThreads,
      createConvexDemoThread,
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
    [bridgeHost, effectiveHost, connected, connecting, connect, disconnect, send, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, resumeNextId]
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
