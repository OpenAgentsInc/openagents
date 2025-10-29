import React, { createContext, useCallback, useContext, useMemo, useRef, useState, useEffect } from 'react';
import { appLog } from '@/lib/app-log';
import { useSettings, type Approvals as StoreApprovals } from '@/lib/settings-store'
import { isDevEnv } from '@/lib/env'
import { parseSessionNotification } from '@/lib/acp/validation'

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
  wsLastClose: { code?: number; reason?: string } | null;
  send: (payload: string | ArrayBuffer | Blob) => boolean;
  setOnMessage: (fn: MsgHandler) => void;
  addSubscriber: (fn: MsgHandler) => () => void;
  // WS request helpers
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
  const [connecting, setConnecting] = useState(false);
  const [wsLastClose, setWsLastClose] = useState<{ code?: number; reason?: string } | null>(null);
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
  const bridgeToken = useSettings((s) => s.bridgeToken)

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
  const isSecureHost = React.useMemo(() => {
    try {
      if (!effectiveHost) return false;
      const parts = effectiveHost.split(':');
      const port = parts.length > 1 ? parts[1] : '';
      if (port === '443') return true;
      // Heuristic: public domains should use TLS
      if (/\.(openagents|cloudflare|vercel|netlify|aws|googleapis)\./i.test(effectiveHost)) return true;
      return false;
    } catch { return false }
  }, [effectiveHost]);

  const disconnect = useCallback(() => {
    setAutoReconnect(false);
    try { wsRef.current?.close(); } catch {}
    wsRef.current = null;
    setConnected(false);
    setConnecting(false);
  }, []);

  const connect = useCallback(() => {
    setAutoReconnect(true);
    // Clear any prior WS close error as a new manual attempt begins
    try { setWsLastClose(null) } catch {}
    // Read latest values from the settings store to avoid stale closures
    const rawHost = useSettings.getState().bridgeHost;
    const tokenNow = useSettings.getState().bridgeToken;
    const host = sanitizeHost(String(rawHost || ''));
    if (!host) return;
    // Compute secure scheme for this host now
    const parts = host.split(':');
    const port = parts.length > 1 ? parts[1] : '';
    const secure = port === '443' || /\.(openagents|cloudflare|vercel|netlify|aws|googleapis)\./i.test(host);
    try { wsRef.current?.close(); } catch {}
    try {
      const tokenPart = (() => {
        const t = String(tokenNow || '').trim();
        return t ? `?token=${encodeURIComponent(t)}` : '';
      })();
      const scheme = secure ? 'wss' : 'ws';
      const httpScheme = secure ? 'https' : 'http';
      const wsUrl = `${scheme}://${host}/ws${tokenPart}`;
      const httpBase = `${httpScheme}://${host}`;
      appLog('bridge.connect', { wsUrl, httpBase });
      try { console.log('[bridge.ws] connect', wsUrl) } catch {}
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      setConnecting(true);
      ws.onopen = () => {
        // Clear any previous connection error on successful open
        try { setWsLastClose(null) } catch {}
        setConnected(true);
        setConnecting(false);
        appLog('bridge.open');
        try { console.log('[bridge.ws] open') } catch {}
      };
      ws.onmessage = (evt) => {
        const data = typeof evt.data === 'string' ? evt.data : String(evt.data);
        try {
          const buf = pendingBufferRef.current;
          buf.push(data);
          if (buf.length > 800) buf.splice(0, buf.length - 800);
        } catch {}
        // Debug: surface ACP and bridge events in the console for diagnostics
        try {
          const s = String(data || '').trim();
          if (s.startsWith('{')) {
            const obj = JSON.parse(s);
            if (obj && typeof obj === 'object') {
              const t = String(obj.type || '');
              if (t === 'bridge.acp') {
                const rawNotif = obj.notification || {};
                const parsed = parseSessionNotification(rawNotif);
                const sid = (rawNotif && (rawNotif.sessionId || rawNotif.session_id)) || undefined;
                const kind = rawNotif?.update?.sessionUpdate;
                // eslint-disable-next-line no-console
                console.log('[ws.in][bridge.acp]', { sessionId: sid, kind, valid: parsed.ok });
                if (!parsed.ok && isDevEnv()) {
                  // eslint-disable-next-line no-console
                  console.log('[ws.in][bridge.acp][invalid]', { error: String((parsed as any).error), raw: rawNotif });
                }
              } else if (t && /^bridge\./.test(t)) {
                // eslint-disable-next-line no-console
                console.log('[ws.in]', t);
              }
            }
          }
        } catch {}
        try { onMessageRef.current?.(data); } catch {}
        try { subsRef.current.forEach((fn) => { try { if (fn) fn(data) } catch {} }); } catch {}
      };
      ws.onerror = (_evt: any) => { setConnecting(false); };
      ws.onclose = (evt: any) => {
        try {
          const code = Number((evt && evt.code) || 0) || undefined;
          const reason = String((evt && evt.reason) || '').trim() || undefined;
          setWsLastClose({ code, reason });
        } catch {}
        setConnected(false);
        setConnecting(false);
        appLog('bridge.close');
      };
    } catch (e: any) {
      appLog('bridge.connect.error', { error: String(e?.message ?? e) }, 'error');
      try { console.log('[bridge.ws] connect.error', String(e?.message ?? e)) } catch {}
    }
  }, [sanitizeHost, setAutoReconnect]);

  // Helper: wait until the WebSocket is OPEN (or time out)
  const awaitConnected = useCallback(async (_timeoutMs: number = 8000) => {
    // Do not auto-connect; only report current state
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) return;
    throw new Error('ws not connected');
  }, []);

  // Disable auto-connect on mount; user must press Connect explicitly
  // (left intentionally blank)

  // Disable periodic auto-reconnect; connections are manual
  // (left intentionally blank)

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
      wsLastClose,
      send,
      setOnMessage,
      addSubscriber,
      setClearLogHandler,
      clearLog,
      
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
