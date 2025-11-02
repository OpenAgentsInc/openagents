import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { TinyvexClient } from 'tinyvex/client'
import { WsTransport } from 'tinyvex/client/WsTransport'

function App() {
  // Bridge connection inputs
  const [host, setHost] = useState<string>(() => '127.0.0.1:8787')
  const [token, setToken] = useState<string>('')
  const [status, setStatus] = useState<'idle'|'connecting'|'open'|'closed'|'error'>('idle')
  const [connected, setConnected] = useState<boolean>(false)
  const [logs, setLogs] = useState<string[]>([])
  const clientRef = useRef<TinyvexClient | null>(null)
  const unsubRef = useRef<(() => void) | null>(null)

  const wsUrl = useMemo(() => {
    const scheme = 'ws'
    const qp = token ? `?token=${encodeURIComponent(token)}` : ''
    return host ? `${scheme}://${host}/ws${qp}` : ''
  }, [host, token])
  const wsBase = useMemo(() => (host ? `ws://${host}/ws` : ''), [host])

  const connect = () => {
    try { disconnect() } catch {}
    if (!wsUrl) return
    const t = new WsTransport({ url: wsUrl })
    setStatus(t.status())
    const c = new TinyvexClient(t)
    clientRef.current = c
    ;(async () => {
      try {
        await t.connect()
        setStatus(t.status())
        setConnected(true)
        // Subscribe to threads to exercise the path
        c.subscribeThreads();
        c.queryThreads(20);
        // Capture raw events to a bounded log
        unsubRef.current = c.onRaw((evt: unknown) => {
          try {
            const line = JSON.stringify(evt)
            setLogs((prev) => {
              const next = [line, ...prev]
              return next.slice(0, 200)
            })
          } catch {}
        })
      } catch {
        setStatus('error')
        setConnected(false)
      }
    })()
  }
  const disconnect = () => {
    try { unsubRef.current?.() } catch {}
    unsubRef.current = null
    try { clientRef.current = null } catch {}
    setConnected(false)
    setStatus('closed')
  }

  return (
    <main className="container">
      <h1>OpenAgents — Bridge</h1>
      <div className="row" style={{ gap: 8 }}>
        <input id="host-input" placeholder="host:port (e.g., 127.0.0.1:8787)" value={host} onChange={(e) => setHost(e.currentTarget.value)} />
        {!connected ? (
          <button onClick={connect}>Connect</button>
        ) : (
          <button onClick={disconnect}>Disconnect</button>
        )}
      </div>
      <p>wsUrl: {wsBase || '—'}</p>
      <p>Status: {status}</p>
      <div style={{ textAlign: 'left', maxWidth: 960, margin: '0 auto' }}>
        <h3>Raw events</h3>
        <pre style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word', background: '#0e0f10', color: '#d0d6e0', padding: 12, borderRadius: 4, border: '1px solid #23252a', minHeight: 240 }}>
          {logs.join('\n')}
        </pre>
      </div>
    </main>
  );
}

export default App;
  // Fetch token in the background from ~/.openagents/bridge.json and prefill state
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const t = await invoke<string | null>('get_bridge_token')
        if (!cancelled && t) setToken(t)
      } catch {}
    })()
    return () => { cancelled = true }
  }, [])
