import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { TinyvexClient } from 'tinyvex/client'
import { WsTransport } from 'tinyvex/client/WsTransport'
import type { ThreadSummaryTs, MessageRowTs } from 'tricoder/types'

function App() {
  // Bridge connection inputs
  const [host, setHost] = useState<string>(() => '127.0.0.1:8787')
  const [token, setToken] = useState<string>('')
  const [status, setStatus] = useState<'idle'|'connecting'|'open'|'closed'|'error'>('idle')
  const [connected, setConnected] = useState<boolean>(false)
  const [logs, setLogs] = useState<string[]>([])
  const [threads, setThreads] = useState<ThreadSummaryTs[]>([])
  const [selectedThread, setSelectedThread] = useState<string>('')
  const [messages, setMessages] = useState<MessageRowTs[]>([])
  const clientRef = useRef<TinyvexClient | null>(null)
  const unsubRef = useRef<(() => void) | null>(null)
  const lastMsgReqRef = useRef<number>(0)
  const autoDetectedRef = useRef<boolean>(false)
  const chatContainerRef = useRef<HTMLDivElement | null>(null)
  const logsContainerRef = useRef<HTMLDivElement | null>(null)

  const wsUrl = useMemo(() => {
    const scheme = 'ws'
    const qp = token ? `?token=${encodeURIComponent(token)}` : ''
    return host ? `${scheme}://${host}/ws${qp}` : ''
  }, [host, token])
  const wsBase = useMemo(() => (host ? `ws://${host}/ws` : ''), [host])

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

  // Auto-detect local bridge port: probe a small range and pick the highest responsive port
  useEffect(() => {
    if (!token) return; // wait until token is loaded to avoid auth failures
    if (autoDetectedRef.current) return; // only run once
    // Only auto-detect when host is unset or default
    const defaultHost = '127.0.0.1:8787'
    if (host && host !== defaultHost) return
    let cancelled = false
    const ports = Array.from({ length: 12 }, (_, i) => 8787 + i) // 8787..8798
    const probe = (port: number) => new Promise<number | null>((resolve) => {
      try {
        const url = `ws://127.0.0.1:${port}/ws?token=${encodeURIComponent(token)}`
        const ws = new WebSocket(url)
        let done = false
        const finish = (ok: boolean) => {
          if (done) return; done = true
          try { ws.close() } catch {}
          resolve(ok ? port : null)
        }
        const to = setTimeout(() => finish(false), 700)
        ws.onopen = () => { clearTimeout(to); finish(true) }
        ws.onerror = () => { clearTimeout(to); finish(false) }
        ws.onclose = () => {/* no-op */}
      } catch { resolve(null) }
    })
    ;(async () => {
      const results = await Promise.all(ports.map(probe))
      if (cancelled) return
      const okPorts = results.filter((p): p is number => typeof p === 'number')
      if (okPorts.length > 0) {
        const best = Math.max(...okPorts)
        autoDetectedRef.current = true
        setHost(`127.0.0.1:${best}`)
      }
    })()
    return () => { cancelled = true }
  }, [token, host])

  // Auto-connect once token and host are known and we are not connected
  useEffect(() => {
    if (!token || !host) return
    if (connected) return
    connect()
  // intentionally omit connect from deps to avoid recreating
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, host, connected])

  const connect = () => {
    try { disconnect() } catch {}
    if (!wsUrl) return
    const t = new WsTransport({ url: wsUrl })
    setStatus('connecting')
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
              const next = [...prev, line]
              return next.length > 200 ? next.slice(next.length - 200) : next
            })
            const obj: any = evt
            if (obj && typeof obj === 'object') {
              if (obj.type === 'tinyvex.snapshot' && obj.stream === 'threads' && Array.isArray(obj.rows)) {
                handleThreads(obj.rows as ThreadSummaryTs[])
              } else if (obj.type === 'tinyvex.query_result' && obj.name === 'threads.list' && Array.isArray(obj.rows)) {
                handleThreads(obj.rows as ThreadSummaryTs[])
              } else if (obj.type === 'tinyvex.update' && obj.stream === 'threads') {
                // throttle requery
                const now = Date.now(); if (now - lastMsgReqRef.current > 300) { lastMsgReqRef.current = now; clientRef.current?.queryThreads(20) }
              } else if (obj.type === 'tinyvex.snapshot' && obj.stream === 'messages' && Array.isArray(obj.rows)) {
                setMessages(filterFinalMessages(obj.rows as MessageRowTs[]))
              } else if (obj.type === 'tinyvex.query_result' && obj.name === 'messages.list' && Array.isArray(obj.rows)) {
                setMessages(filterFinalMessages(obj.rows as MessageRowTs[]))
              } else if (obj.type === 'tinyvex.update' && obj.stream === 'messages' && selectedThread) {
                const now = Date.now(); if (now - lastMsgReqRef.current > 300) { lastMsgReqRef.current = now; clientRef.current?.["queryHistory"]?.(selectedThread) }
              }
            }
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

  function filterFinalMessages(rows: MessageRowTs[]): MessageRowTs[] {
    return rows
      .filter((r) => String(r.kind || '') === 'message' && !!r.role)
      .sort((a, b) => Number(a.ts) - Number(b.ts))
  }

  function handleThreads(rows: ThreadSummaryTs[]) {
    setThreads(rows)
    // Choose most recent Codex thread, fallback to most recent
    const codex = rows.filter((r) => String(r.source || '') === 'codex')
    const sorted = (xs: ThreadSummaryTs[]) => xs.slice().sort((a, b) => Number(b.updated_at ?? 0) - Number(a.updated_at ?? 0))
    const pick = (sorted(codex)[0] || sorted(rows)[0]) as ThreadSummaryTs | undefined
    const tid = pick?.id ? String(pick.id) : ''
    if (tid && tid !== selectedThread) {
      setSelectedThread(tid)
      try {
        clientRef.current?.subscribeThread(tid)
        clientRef.current?.["queryHistory"]?.(tid)
      } catch {}
    }
  }

  // Keep chat and logs scrolled to bottom as new items arrive
  useEffect(() => {
    try {
      const el = chatContainerRef.current
      if (el) el.scrollTop = el.scrollHeight
    } catch {}
  }, [messages])
  useEffect(() => {
    try {
      const el = logsContainerRef.current
      if (el) el.scrollTop = el.scrollHeight
    } catch {}
  }, [logs])

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
      <div style={{ display: 'flex', gap: 16, alignItems: 'stretch', justifyContent: 'center', maxWidth: 1200, margin: '16px auto', width: '100%' }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <h3>Latest Codex Chat</h3>
          <div ref={chatContainerRef} style={{ border: '1px solid var(--border)', padding: 12, borderRadius: 4, background: '#0e0f10', height: '70vh', overflowY: 'auto' }}>
            {selectedThread ? null : <p style={{ color: 'var(--tertiary)' }}>No threads yet…</p>}
            {messages.map((m, idx) => (
              <div key={`${m.id}-${idx}`} style={{ display: 'flex', justifyContent: m.role === 'assistant' ? 'flex-start' : 'flex-end', padding: '6px 0' }}>
                <div style={{
                  maxWidth: 680,
                  padding: '10px 12px',
                  borderRadius: 8,
                  background: m.role === 'assistant' ? '#121317' : '#1b1d22',
                  border: '1px solid var(--border)',
                  color: 'var(--foreground)'
                }}>
                  <div style={{ fontSize: 12, color: 'var(--tertiary)', marginBottom: 4 }}>{m.role}</div>
                  <div style={{ whiteSpace: 'pre-wrap' }}>{String(m.text || '')}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <h3>Raw events</h3>
          <div ref={logsContainerRef} style={{ border: '1px solid var(--border)', borderRadius: 4, background: '#0e0f10', height: '70vh', overflowY: 'auto', padding: 12 }}>
            <pre style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word', color: '#d0d6e0', margin: 0 }}>
              {logs.join('\n')}
            </pre>
          </div>
        </div>
      </div>
    </main>
  );
}

export default App;
