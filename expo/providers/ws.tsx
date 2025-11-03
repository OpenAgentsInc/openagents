import React, { createContext, useCallback, useContext, useMemo, useRef, useState, useEffect } from 'react';
import { appLog } from '@/lib/app-log';
import { useSettings, type Approvals as StoreApprovals } from '@/lib/settings-store'
import { isDevEnv, devBridgeHost, devBridgeToken } from '@/lib/env'
import { parseSessionNotification } from '@/lib/acp/validation'
import { usePairingStore } from '@openagentsinc/core'

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
  requestProjects: () => Promise<unknown[]>;
  requestSkills: () => Promise<unknown[]>;
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

// Build a stable, TDZ-free preview string for outbound payloads.
// Uses only function-scoped `var` and avoids nested blocks to work around
// WebKit/Safari JIT quirks for block-scoped bindings in tight callsites.
function payloadPreview(payload: string | ArrayBuffer | Blob): string {
  var preview = '';
  if (typeof payload === 'string') {
    var s = payload;
    if (s.length > 160) {
      return s.slice(0, 160) + '…';
    }
    return s;
  }
  if (typeof ArrayBuffer !== 'undefined') {
    if (payload instanceof ArrayBuffer) {
      var len = payload.byteLength;
      return '[' + String(len) + ' bytes]';
    }
    // Covers typed arrays (ArrayBufferView)
    if (typeof ArrayBuffer.isView === 'function' && ArrayBuffer.isView(payload)) {
      var view = payload as ArrayBufferView;
      return '[' + String(view.byteLength) + ' bytes]';
    }
  }
  if (typeof Blob !== 'undefined' && payload instanceof Blob) {
    return '[' + String(payload.size) + ' bytes]';
  }
  return preview;
}

export function BridgeProvider({ children }: { children: React.ReactNode }) {
  const bridgeHost = useSettings((s) => s.bridgeHost)
  const setBridgeHost = useSettings((s) => s.setBridgeHost)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
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

  // Tauri invoker resolver: supports both v2 (core.invoke) and v1 (invoke/tauri.invoke)
  type TauriInvoke = (cmd: string, args?: unknown) => Promise<unknown>;
  const getTauriInvoke = React.useCallback((): TauriInvoke | null => {
    try {
      const anyWin: any = typeof window !== 'undefined' ? (window as any) : null;
      const tauri = anyWin && anyWin.__TAURI__ ? anyWin.__TAURI__ : null;
      if (!tauri) return null;
      const coreInvoke: TauriInvoke | null = (tauri.core && typeof tauri.core.invoke === 'function') ? (tauri.core.invoke.bind(tauri.core) as unknown as TauriInvoke) : null;
      // Some builds expose invoke at top-level or under `tauri` namespace
      const topInvoke: TauriInvoke | null = (typeof tauri.invoke === 'function') ? (tauri.invoke.bind(tauri) as unknown as TauriInvoke) : null;
      const nsInvoke: TauriInvoke | null = (tauri.tauri && typeof tauri.tauri.invoke === 'function') ? (tauri.tauri.invoke.bind(tauri.tauri) as unknown as TauriInvoke) : null;
      return coreInvoke || topInvoke || nsInvoke || null;
    } catch {
      return null;
    }
  }, []);

  // When running in Tauri, poll bridge_status logs and print new lines to the console.
  useEffect(() => {
    const invoke = getTauriInvoke();
    if (!invoke) return;
    let alive = true;
    const prevLenRef = { current: 0 } as { current: number };
    const tick = async () => {
      try {
        const s = await invoke('bridge_status');
        if (!alive || !s) return;
        const obj = s as { running?: boolean; bind?: string; logs?: string[] };
        if (Array.isArray(obj.logs)) {
          const start = Math.max(0, obj.logs.length - 200);
          const newLen = obj.logs.length;
          const prev = prevLenRef.current;
          if (newLen > prev) {
            for (let i = prev; i < newLen; i++) {
              const line = obj.logs[i];
              try { console.log('[bridge.sidecar]', line) } catch {}
            }
            prevLenRef.current = newLen;
          } else if (prev > newLen) {
            // logs truncated/rotated; reset pointer
            prevLenRef.current = newLen;
          }
        }
      } catch {}
    };
    const id = setInterval(tick, 1000);
    // Fire once immediately to surface initial status
    tick().catch(() => {});
    return () => { alive = false; clearInterval(id) };
  }, [getTauriInvoke]);

  // Normalize any legacy or malformed host values (e.g., ws://host:port/ws)
  const sanitizeHost = useCallback((raw: string): string => {
    try {
      const val = String(raw || '').trim();
      if (!val) return '';
      // If host text accidentally contains repeated host:port concatenations,
      // extract the first plausible host:port and discard the rest.
      const repeatedMatch = val.match(/((?:[a-zA-Z0-9.-]+|\d{1,3}(?:\.\d{1,3}){3}):\d{2,5})/);
      const basis = repeatedMatch ? repeatedMatch[1] : val;
      const stripped = basis
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
    // Persist sanitized host back into settings to keep UI clean
    try { if (host !== rawHost) useSettings.getState().setBridgeHost(host) } catch {}
    // Compute secure scheme for this host now
    const parts = host.split(':');
    const port = parts.length > 1 ? parts[1] : '';
    const secure = port === '443' || /\.(openagents|cloudflare|vercel|netlify|aws|googleapis)\./i.test(host);
    try { wsRef.current?.close(); } catch {}
    try {
      let tokenPart = '';
      try {
        const t = String(tokenNow || '').trim();
        tokenPart = t ? `?token=${encodeURIComponent(t)}` : '';
      } catch {}
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
        try { usePairingStore.getState().setDeeplinkPairing(false) } catch {}
        appLog('bridge.open');
        try { console.log('[bridge.ws] open') } catch {}
        // Reapply persisted sync preferences to the bridge so toggles survive restarts
        try {
          const s = useSettings.getState();
          const prefs = { syncEnabled: !!s.syncEnabled, syncTwoWay: !!s.syncTwoWay };
          try { ws.send(JSON.stringify({ control: 'sync.enable', enabled: prefs.syncEnabled })); } catch {}
          try { ws.send(JSON.stringify({ control: 'sync.two_way', enabled: prefs.syncTwoWay })); } catch {}
          // Ask for status after applying
          try { ws.send(JSON.stringify({ control: 'sync.status' })); } catch {}
        } catch {}
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
                  console.log('[ws.in][bridge.acp][invalid]', { raw: rawNotif });
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
      ws.onerror = (_evt: Event) => {
        try { console.log('[bridge.ws] error') } catch {}
        setConnecting(false);
      };
      ws.onclose = (evt: CloseEvent) => {
        try {
          const code = Number((evt && evt.code) || 0) || undefined;
          const reason = String((evt && evt.reason) || '').trim() || undefined;
          try { console.log('[bridge.ws] close', code, reason || '(no reason)') } catch {}
          setWsLastClose({ code, reason });
        } catch {}
        try { usePairingStore.getState().setDeeplinkPairing(false) } catch {}
        setConnected(false);
        setConnecting(false);
        appLog('bridge.close');
      };
      ws.onerror = (_evt: Event) => {
        try { usePairingStore.getState().setDeeplinkPairing(false) } catch {}
      };
    } catch (e: any) {
      appLog('bridge.connect.error', { error: String(e?.message ?? e) }, 'error');
      try { console.log('[bridge.ws] connect.error', String(e?.message ?? e)) } catch {}
    }
  }, [sanitizeHost, setAutoReconnect]);

  // If running inside Tauri, try to start the local bridge sidecar and set host/token.
  // This mirrors the desktop app behavior (ADR-0009). No-ops in browsers/mobile.
  useEffect(() => {
    (async () => {
      try {
        if (connected || connecting) return;
        // Avoid repeated starts
        if (autoTriedRef.current) return;
        const invoke = getTauriInvoke();
        if (!invoke) {
          try { console.info('[bridge.autostart] Tauri API not detected; skipping sidecar start', { origin: typeof window !== 'undefined' ? window.location?.origin : '(no window)' }) } catch {}
          return;
        }
        autoTriedRef.current = true;
        try { console.info('[bridge.autostart] detected Tauri environment; attempting sidecar start') } catch {}
        // Fetch token from ~/.openagents/bridge.json
        let tok: string | null = null;
        try {
          const resp = await invoke('get_bridge_token');
          tok = typeof resp === 'string' ? resp : null;
        } catch (e) { try { console.warn('[bridge.autostart] get_bridge_token failed', e) } catch {} }
        if (tok && typeof tok === 'string') {
          try { setBridgeToken(tok) } catch {}
          try { console.info('[bridge.autostart] token loaded from ~/.openagents/bridge.json') } catch {}
        }
        // Start (or connect to) a local sidecar bridge; returns host:port
        let host: string | null = null;
        try {
          const resp = await invoke('bridge_start', { bind: null, token: tok || null });
          host = typeof resp === 'string' ? resp : null;
        } catch (e) { try { console.warn('[bridge.autostart] bridge_start failed', e) } catch {} }
        // If token was missing before start, try to read it again (bridge may have generated it)
        if (!tok) {
          try {
            const retry = await invoke('get_bridge_token');
            const retryTok = typeof retry === 'string' ? retry : null;
            if (retryTok) {
              tok = retryTok;
              try { setBridgeToken(retryTok) } catch {}
              try { console.info('[bridge.autostart] token initialized by bridge') } catch {}
            }
          } catch {}
        }
        if (host && typeof host === 'string') {
          try { console.info('[bridge.autostart] sidecar running at', host) } catch {}
          try { setBridgeHost(host) } catch {}
          try { connect() } catch {}
        } else {
          try { console.warn('[bridge.autostart] no host returned from sidecar') } catch {}
        }
      } catch {}
    })();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected, connecting, setBridgeHost, setBridgeToken, connect, getTauriInvoke]);

  // Helper: wait until the WebSocket is OPEN (or time out)
  const awaitConnected = useCallback(async (_timeoutMs: number = 8000) => {
    // Do not auto-connect; only report current state
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) return;
    throw new Error('ws not connected');
  }, []);

  // Disable auto-connect on mount; user must press Connect explicitly
  // (left intentionally blank)
  // Try once on app start to reconnect using saved host/token if present.
  React.useEffect(() => {
    try {
      if (!autoTriedRef.current && effectiveHost) {
        autoTriedRef.current = true;
        const ws = wsRef.current;
        if (!ws || ws.readyState !== WebSocket.OPEN) {
          // Do not mark deeplinkPairing here; this is a silent auto-reconnect path.
          if (!connected && !connecting) { connect(); }
        }
      }
    } catch {}
    // Only run when host/connection state changes; guard ensures single attempt per app boot
  }, [effectiveHost, connected, connecting, connect]);

  // Do not force-close the WebSocket on unmount; some WebViews (WebKit) exhibit
  // TDZ quirks around inline cleanup handlers. Let the browser clean up.

  const send = useCallback((payload: string | ArrayBuffer | Blob) => {
    const ws = wsRef.current;
    const preview = payloadPreview(payload);
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      try { appLog('ws.send.skipped', { preview, reason: 'not_open' }); } catch {}
      return false;
    }
    try { ws.send(payload); appLog('ws.send', { preview }); } catch (e: unknown) { appLog('ws.send.error', { error: String((e as Error)?.message ?? e) }, 'error'); return false; }
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

  const requestProjects = useCallback(async (): Promise<unknown[]> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise<unknown[]>((resolve, reject) => {
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


  const requestSkills = useCallback(async (): Promise<unknown[]> => {
    await awaitConnected().catch((e) => { throw e });
    return new Promise<unknown[]>((resolve, reject) => {
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
      wsUrl: effectiveHost ? `${isSecureHost ? 'wss' : 'ws'}://${effectiveHost}/ws` : '',
      httpBase: effectiveHost ? `${isSecureHost ? 'https' : 'http'}://${effectiveHost}` : '',
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
    [bridgeHost, effectiveHost, isSecureHost, connected, connecting, connect, disconnect, send, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, resumeNextId]
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

// Dev-only auto-connect helper: if EXPO_PUBLIC_AUTO_CONNECT=1 and
// EXPO_PUBLIC_BRIDGE_HOST/TOKEN are provided at build time, attempt a
// one-shot connect on mount without user interaction. This is only
// active when `EXPO_PUBLIC_ENV=development`.
export function DevAutoConnectGate() {
  const { connect } = useBridge();
  const setBridgeHost = useSettings((s) => s.setBridgeHost)
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  const connected = useBridge().connected
  const connecting = useBridge().connecting
  useEffect(() => {
    try {
      const want = String(process.env.EXPO_PUBLIC_AUTO_CONNECT || '').trim() === '1'
      if (!want) return
      if (!isDevEnv()) return
      if (connected || connecting) return
      const host = devBridgeHost();
      const token = devBridgeToken();
      if (host && token) {
        try { setBridgeHost(host) } catch {}
        try { setBridgeToken(token) } catch {}
        try { connect() } catch {}
      }
    } catch {}
  }, [connected, connecting, connect, setBridgeHost, setBridgeToken])
  return null
}
