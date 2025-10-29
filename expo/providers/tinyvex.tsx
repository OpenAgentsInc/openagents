import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useBridge } from './ws'

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
