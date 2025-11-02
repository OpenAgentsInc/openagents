import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { TinyvexClient } from 'tinyvex/client'
import { WsTransport } from 'tinyvex/client/WsTransport'
import type { ThreadSummaryTs, MessageRowTs } from 'tricoder/types'
import { ThreadListItem, ChatMessageBubble } from '@openagentsinc/core'
import { View, Text } from 'react-native'
import { Ionicons } from '@expo/vector-icons'

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
  const [lastByThread, setLastByThread] = useState<Record<string, string>>({})

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
                const rows = obj.rows as MessageRowTs[]
                const tid = String(obj.thread_id || '')
                // Only update main chat when rows belong to the selected thread
                if (tid && tid === selectedThread) setMessages(filterFinalMessages(rows))
                if (tid) updateLastFromRows(tid, rows)
              } else if (obj.type === 'tinyvex.query_result' && obj.name === 'messages.list' && Array.isArray(obj.rows)) {
                const rows = obj.rows as MessageRowTs[]
                const tid = String(obj.thread_id || '')
                if (tid && tid === selectedThread) setMessages(filterFinalMessages(rows))
                if (tid) updateLastFromRows(tid, rows)
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
    const finals = rows.filter((r) => String(r.kind || '') === 'message' && !!r.role)
    const byKey = new Map<string, MessageRowTs>()
    const keyOf = (r: MessageRowTs) => String(r.item_id || `${r.seq ?? ''}:${r.ts}:${String(r.text || '').slice(0, 120)}`)
    for (const r of finals) {
      const k = keyOf(r)
      const prev = byKey.get(k)
      if (!prev || Number(r.ts) >= Number(prev.ts)) byKey.set(k, r)
    }
    return Array.from(byKey.values()).sort((a, b) => Number(a.ts) - Number(b.ts))
  }

  function updateLastFromRows(threadId: string, rows: MessageRowTs[]) {
    try {
      const finals = rows.filter((r) => String(r.kind || '') === 'message' && !!r.role)
      if (!finals.length) return
      const last = finals[finals.length - 1]
      setLastByThread((prev) => ({ ...prev, [threadId]: String(last.text || '') }))
    } catch {}
  }

  function handleThreads(rows: ThreadSummaryTs[]) {
    setThreads(rows)
    // Choose most recent Codex thread, fallback to most recent
    const providerRows = rows.filter((r) => {
      const s = String(r.source || '')
      return s === 'codex' || s === 'claude_code'
    })
    const sorted = (xs: ThreadSummaryTs[]) => xs.slice().sort((a, b) => Number(b.updated_at ?? 0) - Number(a.updated_at ?? 0))
    const top = sorted(providerRows).slice(0, 10)
    // Query last message for the top threads
    for (const thr of top) {
      try { clientRef.current?.queryMessages(String(thr.id), 1) } catch {}
    }
    const pick = (top[0] || sorted(rows)[0]) as ThreadSummaryTs | undefined
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

  function formatRelative(ts: number): string {
    const now = Date.now()
    const diff = Math.max(0, now - ts)
    const sec = Math.floor(diff / 1000)
    if (sec < 5) return 'just now'
    if (sec < 60) return `${sec} seconds ago`
    const min = Math.floor(sec / 60)
    if (min < 60) return `${min} minute${min === 1 ? '' : 's'} ago`
    const hr = Math.floor(min / 60)
    if (hr < 24) return `${hr} hour${hr === 1 ? '' : 's'} ago`
    const day = Math.floor(hr / 24)
    if (day < 7) return `${day} day${day === 1 ? '' : 's'} ago`
    const week = Math.floor(day / 7)
    if (week < 5) return `${week} week${week === 1 ? '' : 's'} ago`
    const month = Math.floor(day / 30)
    if (month < 12) return `${month} month${month === 1 ? '' : 's'} ago`
    const year = Math.floor(day / 365)
    return `${year} year${year === 1 ? '' : 's'} ago`
  }

  function computeSnippet(lastText: string, fallbackTitle?: string | null): string {
    const raw = String(lastText || '')
    const cleaned = raw
      .replace(/```[\s\S]*?```/g, '')
      .replace(/`([^`]*)`/g, '$1')
      .replace(/^#+\s*/gm, '')
      .replace(/\*\*([^*]+)\*\*/g, '$1')
      .replace(/\*([^*]+)\*/g, '$1')
      .replace(/\[(.*?)\]\([^)]*\)/g, '$1')
      .replace(/\s+/g, ' ')
      .trim()
    const base = cleaned || (fallbackTitle ? String(fallbackTitle) : '') || 'Thread'
    const maxLen = 48
    return base.length > maxLen ? `${base.slice(0, maxLen - 1)}…` : base
  }

  return (
    <main className="container">
      <div style={{ display: 'flex', gap: 16, alignItems: 'stretch', justifyContent: 'stretch', width: '100%', margin: '16px 0 0', flex: 1, minHeight: 0 }}>
        {/* Sidebar with recent chats and compact raw feed */}
        <div style={{ width: 320, minWidth: 260, display: 'flex', flexDirection: 'column', gap: 12, height: '100%', minHeight: 0, flexShrink: 0 }}>
          {/* Connection panel */}
          <div style={{ border: '1px solid var(--border)', borderRadius: 4, background: '#0e0f10', padding: 10 }}>
            <div style={{ display: 'flex', gap: 8 }}>
              <input id="host-input" placeholder="host:port" value={host} onChange={(e) => setHost(e.currentTarget.value)} style={{ flex: 1 }} />
              {!connected ? (
                <button onClick={connect}>Connect</button>
              ) : (
                <button onClick={disconnect}>Disconnect</button>
              )}
            </div>
            <div style={{ textAlign: 'left', marginTop: 6 }}>
              <div style={{ fontSize: 11, color: 'var(--tertiary)' }}>wsUrl: <span style={{ color: 'var(--secondary)' }}>{wsBase || '—'}</span></div>
              <div style={{ fontSize: 11, color: 'var(--tertiary)' }}>Status: <span style={{ color: 'var(--secondary)' }}>{status}</span></div>
            </div>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', minHeight: 0, flex: '0 0 auto' }}>
            <h3 style={{ margin: 0, marginBottom: 6, textAlign: 'left', flexShrink: 0 }}>Recent chats</h3>
            <div style={{ border: '1px solid var(--border)', borderRadius: 4, background: '#0e0f10', flex: '0 0 auto', overflowY: 'auto', maxHeight: 200 }}>
              {threads
                .filter((r) => ['codex', 'claude_code'].includes(String(r.source || '')))
                .sort((a, b) => Number(b.updated_at ?? 0) - Number(a.updated_at ?? 0))
                .slice(0, 10)
                .map((r) => {
                  const tid = String(r.id)
                  const provider = String(r.source || '')
                  const active = selectedThread === tid
                  const last = (lastByThread[tid] || '').trim()
                  const title = computeSnippet(last, r.title)
                  const updatedAt = typeof r.last_message_ts === 'number' && r.last_message_ts > 0 ? r.last_message_ts : Number(r.updated_at || 0)
                  const tsText = updatedAt ? formatRelative(updatedAt) : ''
                  const providerBadge = provider ? (
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                      {provider === 'claude_code' ? <Ionicons name="flash-outline" size={12} color="#62666d" /> : provider === 'codex' ? <Ionicons name="code-slash" size={12} color="#62666d" /> : null}
                      <Text style={{ color: '#62666d', fontFamily: 'Berkeley Mono', fontSize: 12 }}>{provider === 'claude_code' ? 'Claude Code' : provider === 'codex' ? 'Codex' : provider}</Text>
                    </View>
                  ) : null
                  const meta = (
                    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                      {!!tsText && (
                        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
                          <Ionicons name="time-outline" size={12} color="#62666d" />
                          <Text numberOfLines={1} style={{ color: '#62666d', fontFamily: 'Berkeley Mono', fontSize: 12 }}>{tsText}</Text>
                        </View>
                      )}
                      {providerBadge ? (
                        <>
                          {!!tsText && (
                            <Text style={{ color: '#62666d', fontFamily: 'Berkeley Mono', fontSize: 12 }}>•</Text>
                          )}
                          {providerBadge}
                        </>
                      ) : null}
                    </View>
                  )
                  return (
                    <div key={tid} style={{ borderBottom: '1px solid var(--border)', background: active ? '#111216' : 'transparent' }}>
                      <ThreadListItem
                        title={title}
                        meta={meta}
                        timestamp={updatedAt}
                        onPress={() => {
                          setSelectedThread(tid)
                          try { clientRef.current?.subscribeThread(tid); clientRef.current?.["queryHistory"]?.(tid) } catch {}
                        }}
                        testID={`drawer-thread-${tid}`}
                      />
                    </div>
                  )
                })}
              {threads.filter((r) => ['codex', 'claude_code'].includes(String(r.source || ''))).length === 0 ? (
                <div style={{ padding: 12, color: 'var(--tertiary)', fontSize: 12, textAlign: 'left' }}>No recent Codex/Claude chats.</div>
              ) : null}
            </div>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
            <h3 style={{ margin: 0, marginBottom: 6, textAlign: 'left' }}>Raw events</h3>
            <div ref={logsContainerRef} style={{ border: '1px solid var(--border)', borderRadius: 4, background: '#0e0f10', flex: '0 0 auto', maxHeight: 200, overflowY: 'auto', padding: 10 }}>
              <pre style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word', color: '#d0d6e0', margin: 0, fontSize: 11, lineHeight: '15px' }}>
                {logs.slice(-10).join('\n')}
              </pre>
            </div>
          </div>
        </div>
        <div style={{ flex: 2, minWidth: 0, display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
          <div ref={chatContainerRef} style={{ border: '1px solid var(--border)', padding: 12, borderRadius: 4, background: '#0e0f10', flex: 1, overflowY: 'auto', minHeight: 0 }}>
            {selectedThread ? null : <p style={{ color: 'var(--tertiary)' }}>No threads yet…</p>}
            {messages.map((m, idx) => (
              <ChatMessageBubble key={`${m.id}-${idx}`} role={String(m.role || '').toLowerCase() === 'assistant' ? 'assistant' : 'user'} text={String(m.text || '')} />
            ))}
          </div>
        </div>
      </div>
    </main>
  );
}

export default App;
