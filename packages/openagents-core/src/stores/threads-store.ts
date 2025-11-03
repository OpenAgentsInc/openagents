import { create } from 'zustand'
import { persist } from 'zustand/middleware/persist'
import { universalJSONStorage } from './persist'

export type ThreadItem = { ts: number; kind: 'message'|'reason'|'cmd'; role?: 'assistant'|'user'; text: string }
export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean; tail?: ThreadItem[] }
export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string; resume_id?: string; partial?: boolean }

const MAX_HISTORY_CACHE = 80

type ThreadsState = {
  history: HistoryItem[]
  loadingHistory: boolean
  historyLoadedAt?: number
  historyError?: string | null
  lastHistoryUrl?: string
  thread: Record<string, ThreadResponse | undefined>
  loadingThread: Record<string, boolean>
  rehydrated: boolean
  threadProject: Record<string, string | undefined>
  setThreadProject: (threadId: string, projectId: string) => void
  primeThread: (id: string, preview: ThreadResponse) => void
  loadHistory: (requestHistory: (params?: { limit?: number; since_mtime?: number }) => Promise<HistoryItem[]>) => Promise<void>
  loadThread: (requestThread: (id: string, path?: string) => Promise<ThreadResponse | undefined>, id: string, path?: string) => Promise<ThreadResponse | undefined>
}

export const useThreads = create<ThreadsState>()(
  persist(
    (set, get) => ({
      history: [], loadingHistory: false, historyLoadedAt: undefined, historyError: null, lastHistoryUrl: undefined,
      thread: {}, loadingThread: {}, rehydrated: false,
      threadProject: {},
      setThreadProject: (threadId, projectId) => set((s) => ({ threadProject: { ...s.threadProject, [threadId]: projectId } })),
      primeThread: (id, preview) => set((s) => ({ thread: { ...s.thread, [id]: { ...preview, partial: true } } })),
      loadHistory: async (requestHistory) => {
        set({ loadingHistory: true })
        try {
          const existing = (get().history || []).slice(0, MAX_HISTORY_CACHE)
          const latestMtime = existing.reduce((acc, it)=> Math.max(acc, it.mtime || 0), 0)
          const params = latestMtime ? { since_mtime: latestMtime, limit: 50 } : { limit: 50 }
          const delta = await requestHistory(params)
          let next = existing.slice()
          if (Array.isArray(delta) && delta.length > 0) {
            const seen = new Set(next.map(i=>i.id)); for (const it of delta) { if (!seen.has(it.id)) { next.push(it); seen.add(it.id) } }
            next.sort((a,b)=> (b.mtime - a.mtime))
          }
          if (next.length > MAX_HISTORY_CACHE) next = next.slice(0, MAX_HISTORY_CACHE)
          if (next.length === 0 && existing.length === 0) {
            const full = await requestHistory({ limit: 50 }); next = Array.isArray(full) ? full : []
            if (next.length > MAX_HISTORY_CACHE) next = next.slice(0, MAX_HISTORY_CACHE)
          }
          set({ history: next, historyLoadedAt: Date.now(), historyError: null })
        } catch (e) {
          set({ historyError: String((e as Error)?.message ?? e) })
        } finally { set({ loadingHistory: false }) }
      },
      loadThread: async (requestThread, id, path) => {
        const key = id; const cur = get().thread[key]; if (cur && !cur.partial) return cur
        set({ loadingThread: { ...get().loadingThread, [key]: true } })
        try {
          const json = await requestThread(id, path)
          if (json) set({ thread: { ...get().thread, [key]: json } })
          return json ?? undefined
        } finally {
          const { [key]: _, ...rest } = get().loadingThread; set({ loadingThread: rest })
        }
      },
    }),
    {
      name: '@openagents/threads-v1', version: 1, skipHydration: true, storage: universalJSONStorage(),
      partialize: (state) => ({ history: (state.history || []).slice(0, MAX_HISTORY_CACHE), historyLoadedAt: state.historyLoadedAt, threadProject: state.threadProject }),
      onRehydrateStorage: () => () => { try { useThreads.setState({ rehydrated: true }) } catch {} },
      migrate: (persisted: unknown) => {
        try { if (persisted && typeof persisted === 'object') { const next: any = { ...(persisted as any) }; if (Array.isArray(next.items) && !next.history) { next.history = next.items; delete next.items } if (Array.isArray(next.history) && next.history.length > MAX_HISTORY_CACHE) { next.history = next.history.slice(0, MAX_HISTORY_CACHE) } if (next.thread && typeof next.thread === 'object') { next.thread = {} } return next } } catch {}
        return persisted
      },
    }
  )
)

let threadsRehydratePromise: Promise<void> | null = null
export function ensureThreadsRehydrated(): Promise<void> {
  if (!threadsRehydratePromise) {
    threadsRehydratePromise = (async () => { try { await (useThreads as any).persist?.rehydrate?.() } catch {} ; if (!useThreads.getState().rehydrated) { useThreads.setState({ rehydrated: true }) } })()
  }
  return threadsRehydratePromise
}
