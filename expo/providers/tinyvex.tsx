import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useBridge } from './ws'
import { useThreadProviders } from '@/lib/thread-provider-store'

type ThreadsRow = { id: string; thread_id?: string; title: string; project_id?: string; resume_id?: string; created_at: number; updated_at: number }
type MessageRow = { id: number; thread_id: string; role?: string; kind: string; text?: string; item_id?: string; partial?: number; seq?: number; ts: number; created_at: number; updated_at?: number }

type TinyvexContextValue = {
  threads: ThreadsRow[];
  messagesByThread: Record<string, MessageRow[]>;
  subscribeThreads: () => void;
  subscribeMessages: (threadId: string) => void;
  queryThreads: (limit?: number) => void;
  queryMessages: (threadId: string, limit?: number) => void;
}

const TinyvexContext = createContext<TinyvexContextValue | undefined>(undefined)

export function TinyvexProvider({ children }: { children: React.ReactNode }) {
  const bridge = useBridge();
  const connected = bridge.connected;
  const [threads, setThreads] = useState<ThreadsRow[]>([])
  const [messagesByThread, setMessagesByThread] = useState<Record<string, MessageRow[]>>({})

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
        setMessagesByThread((prev) => ({ ...prev, [obj.threadId]: obj.rows as MessageRow[] }))
      }
    } else if (t === 'tinyvex.update') {
      if (obj.stream === 'messages' && typeof obj.threadId === 'string') {
        // For MVP, re-query the recent tail on updates to keep logic simple
        queryMessages(obj.threadId, 200)
      } else if (obj.stream === 'threads') {
        // Threads list changed: refresh top threads
        try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit: 50 } })) } catch {}
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
      }
    }
  }, [])

  useEffect(() => bridge.addSubscriber(onMessage), [bridge, onMessage])

  // Auto-subscribe and fetch when the bridge connects
  useEffect(() => {
    if (!connected) return;
    try { bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'threads' })) } catch {}
    try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit: 50 } })) } catch {}
  }, [connected])

  // Prefetch messages for known threads so history opens instantly
  const prefetchRef = useRef<Set<string>>(new Set())
  useEffect(() => {
    if (!connected) return
    try {
      const seen = prefetchRef.current
      const arr = Array.isArray(threads) ? threads : []
      // Prefetch up to 25 most recent threads
      const copy = arr.slice().sort((a: any, b: any) => {
        const at = (a?.updated_at ?? a?.updatedAt ?? a?.created_at ?? a?.createdAt ?? 0) as number
        const bt = (b?.updated_at ?? b?.updatedAt ?? b?.created_at ?? b?.createdAt ?? 0) as number
        return bt - at
      }).slice(0, 25)
      for (const r of copy) {
        const tid = String((r as any)?.id || (r as any)?.thread_id || (r as any)?.threadId || '')
        if (!tid || seen.has(tid)) continue
        try { bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'messages', threadId: tid })) } catch {}
        try { bridge.send(JSON.stringify({ control: 'tvx.query', name: 'messages.list', args: { threadId: tid, limit: 200 } })) } catch {}
        seen.add(tid)
      }
    } catch {}
  }, [threads, connected])

  const subscribeThreads = useCallback(() => {
    bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'threads' }))
  }, [bridge])
  const subscribeMessages = useCallback((threadId: string) => {
    bridge.send(JSON.stringify({ control: 'tvx.subscribe', stream: 'messages', threadId }))
  }, [bridge])
  const queryThreads = useCallback((limit: number = 50) => {
    bridge.send(JSON.stringify({ control: 'tvx.query', name: 'threads.list', args: { limit } }))
  }, [bridge])
  const queryMessages = useCallback((threadId: string, limit: number = 200) => {
    bridge.send(JSON.stringify({ control: 'tvx.query', name: 'messages.list', args: { threadId, limit } }))
  }, [bridge])

  const value = useMemo(() => ({ threads, messagesByThread, subscribeThreads, subscribeMessages, queryThreads, queryMessages }), [threads, messagesByThread, subscribeThreads, subscribeMessages, queryThreads, queryMessages])
  return <TinyvexContext.Provider value={value}>{children}</TinyvexContext.Provider>
}

export function useTinyvex() {
  const ctx = useContext(TinyvexContext)
  if (!ctx) throw new Error('useTinyvex must be used within TinyvexProvider')
  return ctx
}
