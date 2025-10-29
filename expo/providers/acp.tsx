import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react'
import type { SessionNotification } from '@/types/acp'
import { useBridge } from '@/providers/ws'
import { parseSessionNotification } from '@/lib/acp/validation'

type AcpCtx = {
  sessions: Record<string, SessionNotification[]>;
  add: (n: SessionNotification) => void;
}

const Ctx = createContext<AcpCtx | undefined>(undefined)

export function AcpProvider({ children }: { children: React.ReactNode }) {
  const { addSubscriber } = useBridge()
  const [sessions, setSessions] = useState<Record<string, SessionNotification[]>>({})

  const add = useCallback((n: SessionNotification) => {
    setSessions((prev) => {
      const id = String((n as any).sessionId || '')
      const key = id
      const arr = prev[key] ? [...prev[key]] : []
      arr.push(n)
      return { ...prev, [key]: arr }
    })
  }, [])

  useEffect(() => {
    const unsub = addSubscriber((line) => {
      const s = String(line || '').trim()
      if (!s.startsWith('{')) return
      try {
        const obj = JSON.parse(s)
        if (obj?.type === 'bridge.acp' && obj.notification) {
          const parsed = parseSessionNotification(obj.notification)
          if (parsed.ok) add(parsed.value as any)
        }
      } catch {}
    })
    return unsub
  }, [addSubscriber, add])

  const value = useMemo(() => ({ sessions, add }), [sessions, add])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useAcp() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useAcp must be used within AcpProvider')
  return ctx
}
