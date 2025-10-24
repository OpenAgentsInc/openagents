import React from 'react'
import { useBridge } from '@/providers/ws'
import { useQuery, useMutation } from 'convex/react'
import AsyncStorage from '@react-native-async-storage/async-storage'

// Backfills recent Codex history into Convex if the target thread has no messages yet.
export function ConvexBackfillOnce() {
  const ws = useBridge()
  const threads = (useQuery as any)('threads:list', {}) as any[] | null | undefined
  const upsertThread = (useMutation as any)('threads:upsertFromStream') as (args: { threadId: string; title?: string; projectId?: string }) => Promise<any>
  const createMessage = (useMutation as any)('messages:create') as (args: { threadId: string; role: string; text: string; ts?: number }) => Promise<any>

  const ranRef = React.useRef(false)

  React.useEffect(() => {
    if (ranRef.current) return
    if (threads === null || threads === undefined) return
    ranRef.current = true
    ;(async () => {
      try {
        const doneKey = 'oa_convex_backfill_v1_done'
        const flag = await AsyncStorage.getItem(doneKey)
        if (flag === '1') return
        const hist = await ws.requestHistory({ limit: 20 })
        for (const h of hist) {
          try {
            const thr = await ws.requestThread(h.id, h.path)
            if (!thr) continue
            const threadId = String(thr.resume_id || h.id)
            await upsertThread({ threadId, title: thr.title || 'Thread' }).catch(()=>{})
            const items = Array.isArray(thr.items) ? thr.items : []
            for (const it of items) {
              try {
                if (it.kind !== 'message') continue
                const role = it.role === 'user' ? 'user' : 'assistant'
                const text = String(it.text || '')
                if (!text.trim()) continue
                await createMessage({ threadId, role, text, ts: typeof it.ts === 'number' ? it.ts * 1000 : undefined }).catch(()=>{})
              } catch {}
            }
          } catch {}
        }
        await AsyncStorage.setItem(doneKey, '1')
      } catch {}
    })()
  }, [threads])

  return null
}
