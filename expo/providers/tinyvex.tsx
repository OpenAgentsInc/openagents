import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useBridge } from './ws'
import { useThreadProviders } from '@/lib/thread-provider-store'
import { createPerKeyThrottle, createPerKeyDebounce } from '@/utils/throttle'

// Tunables for provider behavior. Keep these conservative to avoid WS bursts.
const MSG_QUERY_THROTTLE_MS = 350 // per-thread throttle window for messages.list
const THREADS_REFRESH_DEBOUNCE_MS = 400 // debounce for threads.list refresh on updates
const PREFETCH_TOP_THREADS = 10 // number of recent threads to warm on connect
const DEFAULT_THREAD_TAIL = 50 // number of most recent messages to fetch per thread

type ThreadsRow = { id: string; thread_id?: string; title: string; project_id?: string; resume_id?: string; created_at: number; updated_at: number }
type MessageRow = { id: number; thread_id: string; role?: string; kind: string; text?: string; item_id?: string; partial?: number; seq?: number; ts: number; created_at: number; updated_at?: number }

type TinyvexContextValue = {
  threads: ThreadsRow[];
  messagesByThread: Record<string, MessageRow[]>;
  toolCallsByThread: Record<string, any[]>;
  planTouched: Record<string, number>;
  stateTouched: Record<string, number>;
  subscribeThreads: () => void;
  subscribeMessages: (threadId: string) => void;
  queryThreads: (limit?: number) => void;
  queryMessages: (threadId: string, limit?: number) => void;
  queryToolCalls: (threadId: string, limit?: number) => void;
}

const TinyvexContext = createContext<TinyvexContextValue | undefined>(undefined)

/**
 * TinyvexProvider
 *
 * Centralizes Tinyvex bootstrap and live update handling:
 * - On WS connect, subscribe to `threads` and fetch an initial list.
 * - Prefetch a bounded number of message tails for top threads.
 * - Throttle per-thread `messages.list` re-queries when `tinyvex.update` fires
 *   during streaming to avoid flooding the bridge.
 * - Debounce `threads.list` refresh after `tinyvex.update(stream:"threads")`.
 */
export function TinyvexProvider({ children }: { children: React.ReactNode }) {
  const bridge = useBridge();
  const connected = bridge.connected;
  const [threads, setThreads] = useState<ThreadsRow[]>([])
  const [messagesByThread, setMessagesByThread] = useState<Record<string, MessageRow[]>>({})
  const [toolCallsByThread, setToolCallsByThread] = useState<Record<string, any[]>>({})
  const [planTouched, setPlanTouched] = useState<Record<string, number>>({})
  const [stateTouched, setStateTouched] = useState<Record<string, number>>({})

  // Helpers to resolve aliasing between a client thread doc id and the
  // canonical session id (resume_id). When the bridge restarts before the
  // alias map is re-established, the watcher may mirror only to the session id.
  // We compensate on the client by querying canonical ids and storing results
  // under the client doc key as well.
  const getResumeForId = useCallback((tid: string): string | null => {
    try {
      const row = (Array.isArray(threads) ? threads : []).find((r: any) => String((r?.id || r?.thread_id || r?.threadId || '')) === String(tid))
      const rid = row && (row as any).resume_id
      return rid ? String(rid) : null
    } catch { return null }
  }, [threads])
  const getAliasForCanonical = useCallback((canonicalId: string): string | null => {
    try {
      const row = (Array.isArray(threads) ? threads : []).find((r: any) => String((r?.resume_id || '')) === String(canonicalId))
      if (!row) return null
      return String((row as any)?.id || (row as any)?.thread_id || (row as any)?.threadId || '') || null
    } catch { return null }
  }, [threads])

  // Handle incoming bridge events. We only parse JSON objects and ignore
  // plaintext rows for safety/perf. This function mutates provider state and
  // may schedule follow-up queries via throttled/debounced helpers.
  const onMessage = useCallback((raw: string) => {
    if (!raw || raw[0] !== '{') return;
    let obj: any; try { obj = JSON.parse(raw) } catch { return }
    const t = String(obj?.type || '');
    if (t === 'tinyvex.snapshot') {
      if (obj.stream === 'threads' && Array.isArray(obj.rows)) {
        setThreads(obj.rows as ThreadsRow[])
        try {
          const setProvider = useThreadProviders.getState().setProvider
          for (const r of obj.rows as ThreadsRow[]) {
            const tid = String((r as any)?.id || (r as any)?.thread_id || (r as any)?.threadId || '')
            const src = String((r as any)?.source || '')
            if (tid) setProvider(tid, src === 'claude_code' ? 'claude_code' : 'codex')
          }
        } catch {}
      } else if (obj.stream === 'messages' && typeof obj.threadId === 'string' && Array.isArray(obj.rows)) {
        // Store under the reported id
        setMessagesByThread((prev) => ({ ...prev, [obj.threadId]: obj.rows as MessageRow[] }))
        // If these rows are for a canonical session id, also project them onto the
        // client doc id so the thread screen keyed by that id stays fresh.
        try {
          const alias = getAliasForCanonical(String(obj.threadId))
          if (alias && alias !== obj.threadId) {
            setMessagesByThread((prev) => ({ ...prev, [alias]: obj.rows as MessageRow[] }))
          }
        } catch {}
      }
    } else if (t === 'tinyvex.update') {
      if (obj.stream === 'messages' && typeof obj.threadId === 'string') {
        // Live message writes can emit many updates while streaming.
        // Throttle per thread to avoid storms during streaming.
        try { scheduleMsgQuery(obj.threadId) } catch {}
        // Also schedule a query for the canonical id if this was the client doc id,
        // or vice-versa schedule for the alias if we received a canonical update.
        try {
          const resume = getResumeForId(String(obj.threadId))
          if (resume && resume !== obj.threadId) { scheduleMsgQuery(resume) }
          const alias = getAliasForCanonical(String(obj.threadId))
          if (alias && alias !== obj.threadId) { scheduleMsgQuery(alias) }
        } catch {}
      } else if (obj.stream === 'threads') {
        // Prefer merging the provided row to avoid a full refresh
        const row = obj.row
        if (row && typeof row === 'object') {
          setThreads((prev) => {
            const tid = String((row?.id || row?.thread_id || row?.threadId || ''))
            if (!tid) return prev
            const next = Array.isArray(prev) ? [...prev] : []
            const idx = next.findIndex((r: any) => String((r?.id || r?.thread_id || r?.threadId || '')) === tid)
            if (idx >= 0) next.splice(idx, 1)
            next.unshift(row as ThreadsRow)
            return next
          })
          try {
            const setProvider = useThreadProviders.getState().setProvider
            const tid = String((row?.id || row?.thread_id || row?.threadId || ''))
            const src = String((row?.source || ''))
            if (tid) setProvider(tid, src === 'claude_code' ? 'claude_code' : 'codex')
          } catch {}
        } else {
          // Fallback: Debounce refreshes to avoid repeated full refreshes on bursts.
          try { scheduleThreadsRefresh() } catch {}
        }
      } else if (obj.stream === 'toolCalls' && typeof obj.threadId === 'string') {
        const tid: string = obj.threadId
        const tcid: string | undefined = typeof obj.toolCallId === 'string' ? obj.toolCallId : undefined
        if (tcid) {
          setToolCallsByThread((prev) => {
            const next = { ...prev }
            const list = Array.isArray(next[tid]) ? next[tid].slice() : []
            if (!list.includes(tcid)) list.push(tcid)
            next[tid] = list
            return next
          })
        }
      } else if (obj.stream === 'plan' && typeof obj.threadId === 'string') {
        const tid: string = obj.threadId
        setPlanTouched((prev) => ({ ...prev, [tid]: Date.now() }))
      } else if (obj.stream === 'state' && typeof obj.threadId === 'string') {
        const tid: string = obj.threadId
        setStateTouched((prev) => ({ ...prev, [tid]: Date.now() }))
      }
    } else if (t === 'tinyvex.query_result') {
      if (obj.name === 'threads.list' && Array.isArray(obj.rows)) {
        setThreads(obj.rows as ThreadsRow[])
        try {
          const setProvider = useThreadProviders.getState().setProvider
          for (const r of obj.rows as ThreadsRow[]) {
            const tid = String((r as any)?.id || (r as any)?.thread_id || (r as any)?.threadId || '')
            const src = String((r as any)?.source || '')
            if (tid) setProvider(tid, src === 'claude_code' ? 'claude_code' : 'codex')
          }
        } catch {}
      } else if (obj.name === 'messages.list' && typeof obj.threadId === 'string' && Array.isArray(obj.rows)) {
        setMessagesByThread((prev) => ({ ...prev, [obj.threadId]: obj.rows as MessageRow[] }))
      } else if (obj.name === 'toolCalls.list' && typeof obj.threadId === 'string' && Array.isArray(obj.rows)) {
        setToolCallsByThread((prev) => ({ ...prev, [obj.threadId]: obj.rows as any[] }))
      } else if (obj.name === 'threadsAndTails.list') {
        // cancel fallback to threads.list if pending
        try {
          if (bootstrapPendingRef.current) {
            bootstrapPendingRef.current = false
            const id = fallbackTimerRef.current
            if (id != null) { clearTimeout(id as any); fallbackTimerRef.current = null as any }
          }
        } catch {}
        const threadsRows = Array.isArray(obj.threads) ? (obj.threads as ThreadsRow[]) : []
        setThreads(threadsRows)
        try {
          const setProvider = useThreadProviders.getState().setProvider
          for (const r of threadsRows) {
            const tid = String((r as any)?.id || (r as any)?.thread_id || (r as any)?.threadId || '')
            const src = String((r as any)?.source || '')
            if (tid) setProvider(tid, src === 'claude_code' ? 'claude_code' : 'codex')
          }
        } catch {}
        const tails = Array.isArray(obj.tails) ? (obj.tails as any[]) : []
        setMessagesByThread((prev) => {
          const next = { ...prev }
          for (const t of tails) {
            const tid = String((t?.threadId) || '')
            const rows = Array.isArray(t?.rows) ? (t.rows as MessageRow[]) : []
            if (tid && rows.length) next[tid] = rows
          }
          return next
        })
      }
    }
  }, [getAliasForCanonical, getResumeForId])

  useEffect(() => bridge.addSubscriber(onMessage), [bridge, onMessage])

  // Throttlers and debouncers used for follow-up queries
  const scheduleMsgQuery = useMemo(() => {
    const throttle = createPerKeyThrottle(MSG_QUERY_THROTTLE_MS)
    return (threadId: string) => throttle(threadId, () => {
      try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'messages.list', args: { threadId, limit: DEFAULT_THREAD_TAIL } })) } catch {}
    })
  }, [bridge])
  const scheduleThreadsRefresh = useMemo(() => {
    const debounce = createPerKeyDebounce(THREADS_REFRESH_DEBOUNCE_MS)
    return () => debounce('threads', () => {
      try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit: 50 } })) } catch {}
    })
  }, [bridge])

  // Auto-subscribe and fetch when the bridge connects
  useEffect(() => {
    if (!connected) return;
    // Single bootstrap: subscribe + list
    try { bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'threads' })) } catch {}
    try {
      bootstrapPendingRef.current = true
      bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threadsAndTails.list', args: { limit: 50, perThreadTail: DEFAULT_THREAD_TAIL } }))
      const timer = setTimeout(() => {
        try {
          if (bootstrapPendingRef.current) {
            bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit: 50 } }))
            bootstrapPendingRef.current = false
          }
        } catch {}
      }, 1200)
      fallbackTimerRef.current = timer as any
    } catch {}
  }, [connected])

  // Prefetch messages for known threads so history opens instantly
  const prefetchRef = useRef<Set<string>>(new Set())
  // Track aggregated bootstrap status and fallback timer
  const bootstrapPendingRef = useRef<boolean>(false)
  const fallbackTimerRef = useRef<number | null>(null)
  useEffect(() => {
    if (!connected) return
    try {
      const seen = prefetchRef.current
      const arr = Array.isArray(threads) ? threads : []
      // Prefetch a bounded recent set (reduce connect burst). We keep
      // a `seen` set to avoid re-subscribing while the provider lives.
      const copy = arr.slice().sort((a: any, b: any) => {
        const at = (a?.updated_at ?? a?.updatedAt ?? a?.created_at ?? a?.createdAt ?? 0) as number
        const bt = (b?.updated_at ?? b?.updatedAt ?? b?.created_at ?? b?.createdAt ?? 0) as number
        return bt - at
      }).slice(0, PREFETCH_TOP_THREADS)
      for (const r of copy) {
        const tid = String((r as any)?.id || (r as any)?.thread_id || (r as any)?.threadId || '')
        if (!tid || seen.has(tid)) continue
        try { bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'messages', threadId: tid })) } catch {}
        try { scheduleMsgQuery(tid) } catch {}
        seen.add(tid)
      }
    } catch {}
  }, [threads, connected])

  const subscribeThreads = useCallback(() => {
    bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'threads' }))
  }, [bridge])
  const subscribeMessages = useCallback((threadId: string) => {
    try { bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'messages', threadId })) } catch {}
    try {
      const canonical = getResumeForId(threadId)
      if (canonical && canonical !== threadId) {
        bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'messages', threadId: canonical }))
      }
    } catch {}
  }, [bridge, getResumeForId])
  const queryThreads = useCallback((limit: number = 50) => {
    bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit } }))
  }, [bridge])
  const queryMessages = useCallback((threadId: string, limit: number = DEFAULT_THREAD_TAIL) => {
    try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'messages.list', args: { threadId, limit } })) } catch {}
    try {
      const canonical = getResumeForId(threadId)
      if (canonical && canonical !== threadId) {
        bridge.send(JSON.stringify({ control: 'tvx.query', name: 'messages.list', args: { threadId: canonical, limit } }))
      }
    } catch {}
  }, [bridge, getResumeForId])
  const queryToolCalls = useCallback((threadId: string, limit: number = 50) => {
    bridge.send(JSON.stringify({ control: 'tvx.query', name: 'toolCalls.list', args: { threadId, limit } }))
  }, [bridge])

  const value = useMemo(() => ({ threads, messagesByThread, subscribeThreads, subscribeMessages, queryThreads, queryMessages, queryToolCalls }), [threads, messagesByThread, subscribeThreads, subscribeMessages, queryThreads, queryMessages, queryToolCalls])
  const ctxValue = useMemo(() => ({
    threads,
    messagesByThread,
    toolCallsByThread,
    planTouched,
    stateTouched,
    subscribeThreads,
    subscribeMessages,
    queryThreads,
    queryMessages,
    queryToolCalls,
  }), [threads, messagesByThread, toolCallsByThread, planTouched, stateTouched, subscribeThreads, subscribeMessages, queryThreads, queryMessages, queryToolCalls])
  return <TinyvexContext.Provider value={ctxValue}>{children}</TinyvexContext.Provider>
}

export function useTinyvex() {
  const ctx = useContext(TinyvexContext)
  if (!ctx) throw new Error('useTinyvex must be used within TinyvexProvider')
  return ctx
}
