import { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import type { ThreadSummaryTs } from 'tricoder/types'
import { TinyvexContext } from './Provider'

type ThreadsState = { threads: ThreadSummaryTs[] }

export function useTinyvexThreads(limit = 50): ThreadsState {
  const client = useContext(TinyvexContext)
  if (!client) throw new Error('TinyvexProvider missing')
  const [threads, setThreads] = useState<ThreadSummaryTs[]>([])
  const lastUpdateRef = useRef<number>(0)
  const requery = useCallback(() => {
    const now = Date.now()
    if (now - lastUpdateRef.current < 300) return // throttle
    lastUpdateRef.current = now
    try { client.queryThreads(limit) } catch {}
  }, [client, limit])

  useEffect(() => {
    const off = client.onRaw((evt: any) => {
      if (!evt || typeof evt !== 'object') return
      if (evt.type === 'tinyvex.snapshot' && evt.stream === 'threads' && Array.isArray(evt.rows)) {
        setThreads(evt.rows as ThreadSummaryTs[])
      } else if (evt.type === 'tinyvex.query_result' && evt.name === 'threads.list' && Array.isArray(evt.rows)) {
        setThreads(evt.rows as ThreadSummaryTs[])
      } else if (evt.type === 'tinyvex.update' && evt.stream === 'threads') {
        requery()
      }
    })
    client.subscribeThreads()
    client.queryThreads(limit)
    return () => { try { off() } catch {} }
  }, [client, limit, requery])

  const sorted = useMemo(() => {
    const copy = Array.isArray(threads) ? threads.slice() : []
    copy.sort((a, b) => Number(b.updated_at ?? 0) - Number(a.updated_at ?? 0))
    return copy
  }, [threads])

  return { threads: sorted }
}

