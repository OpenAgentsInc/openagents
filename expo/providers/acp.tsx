import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import type { SessionNotification } from '@/types/acp'
import { useBridge } from '@/providers/ws'
import { parseSessionNotification } from '@/lib/acp/validation'

type AcpCtx = {
  eventsForThread: (threadDocId: string) => SessionNotification[];
  sessionToThread: Record<string, string>;
}

const Ctx = createContext<AcpCtx | undefined>(undefined)

export function AcpProvider({ children }: { children: React.ReactNode }) {
  const { addSubscriber } = useBridge()
  const [eventsByThread, setEventsByThread] = useState<Record<string, SessionNotification[]>>({})
  const [sessionToThread, setSessionToThread] = useState<Record<string, string>>({})
  const sessionToThreadRef = useRef<Record<string, string>>({})
  const pendingBySessionRef = useRef<Record<string, SessionNotification[]>>({})

  const mapSession = useCallback((sessionId: string, threadDocId: string) => {
    sessionToThreadRef.current[sessionId] = threadDocId
    setSessionToThread({ ...sessionToThreadRef.current })
    // Flush any pending ACP updates for this session into the thread bucket
    const pend = pendingBySessionRef.current[sessionId]
    if (Array.isArray(pend) && pend.length > 0) {
      setEventsByThread((prev) => {
        const arr = prev[threadDocId] ? [...prev[threadDocId]] : []
        arr.push(...pend)
        // Clear
        pendingBySessionRef.current[sessionId] = []
        return { ...prev, [threadDocId]: arr }
      })
    }
  }, [])

  const addEventForSession = useCallback((n: SessionNotification) => {
    const sid = String((n as any).sessionId || '')
    const tdoc = sessionToThreadRef.current[sid]
    if (tdoc) {
      setEventsByThread((prev) => {
        const arr = prev[tdoc] ? [...prev[tdoc]] : []
        arr.push(n)
        return { ...prev, [tdoc]: arr }
      })
    } else {
      const pend = pendingBySessionRef.current[sid] || []
      pend.push(n)
      pendingBySessionRef.current[sid] = pend
    }
  }, [])

  useEffect(() => {
    const unsub = addSubscriber((line) => {
      const s = String(line || '').trim()
      if (!s.startsWith('{')) return
      try {
        const obj = JSON.parse(s)
        if (obj?.type === 'bridge.session_started' && obj.sessionId && obj.clientThreadDocId) {
          mapSession(String(obj.sessionId), String(obj.clientThreadDocId))
          return
        }
        if (obj?.type === 'bridge.acp' && obj.notification) {
          const parsed = parseSessionNotification(obj.notification)
          if (parsed.ok) addEventForSession(parsed.value as any)
        }
      } catch {}
    })
    return unsub
  }, [addSubscriber, addEventForSession, mapSession])

  const eventsForThread = useCallback((threadDocId: string) => eventsByThread[threadDocId] || [], [eventsByThread])
  const value = useMemo(() => ({ eventsForThread, sessionToThread }), [eventsForThread, sessionToThread])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useAcp() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useAcp must be used within AcpProvider')
  return ctx
}
