import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { appLog } from '@/lib/app-log'
import { useBridge } from '@/providers/ws'

export type ThreadItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean; tail?: ThreadItem[] }
export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string; resume_id?: string; partial?: boolean }

const MAX_HISTORY_CACHE = 80; // cap persisted history to keep hydration fast

type ThreadsState = {
  history: HistoryItem[]
  loadingHistory: boolean
  historyLoadedAt?: number
  historyError?: string | null
  lastHistoryUrl?: string
  thread: Record<string, ThreadResponse | undefined>
  loadingThread: Record<string, boolean>
  rehydrated: boolean
  // Ephemeral mapping of live thread_id (UUID) -> projectId set when a thread starts
  threadProject: Record<string, string | undefined>
  setThreadProject: (threadId: string, projectId: string) => void
  primeThread: (id: string, preview: ThreadResponse) => void
  loadHistory: (requestHistory: (params?: { limit?: number; since_mtime?: number }) => Promise<HistoryItem[]>) => Promise<void>
  loadThread: (requestThread: (id: string, path?: string) => Promise<ThreadResponse | undefined>, id: string, path?: string) => Promise<ThreadResponse | undefined>
}

export const useThreads = create<ThreadsState>()(
  persist(
    (set, get) => ({
      history: [],
      loadingHistory: false,
      historyLoadedAt: undefined,
      historyError: null,
      lastHistoryUrl: undefined,
      thread: {},
      loadingThread: {},
      rehydrated: false,
      threadProject: {},
      setThreadProject: (threadId: string, projectId: string) => {
        const cur = get().threadProject
        set({ threadProject: { ...cur, [threadId]: projectId } })
      },
      primeThread: (id: string, preview: ThreadResponse) => {
        set({ thread: { ...get().thread, [id]: { ...preview, partial: true } } })
      },
      loadHistory: async (requestHistory: (params?: { limit?: number; since_mtime?: number }) => Promise<HistoryItem[]>) => {
    set({ loadingHistory: true })
    try {
          const state = get()
          const existing = (state.history || []).slice(0, MAX_HISTORY_CACHE)
          const latestMtime = existing.reduce((acc, it)=> Math.max(acc, it.mtime || 0), 0)
          const params = latestMtime ? { since_mtime: latestMtime, limit: 50 } : { limit: 50 }
          appLog('history.fetch.start', { via: 'ws', params })
          const delta = await requestHistory(params)
          let next = existing.slice()
          if (Array.isArray(delta) && delta.length > 0) {
            const seen = new Set(next.map(i=>i.id))
            for (const it of delta) { if (!seen.has(it.id)) { next.push(it); seen.add(it.id) } }
            next.sort((a,b)=> (b.mtime - a.mtime))
          }
          if (next.length > MAX_HISTORY_CACHE) {
            next = next.slice(0, MAX_HISTORY_CACHE)
          }
          if (next.length === 0 && existing.length === 0) {
            // Fallback initial fetch without since_mtime if cache is empty and delta was empty
            const full = await requestHistory({ limit: 50 })
            next = Array.isArray(full) ? full : []
            if (next.length > MAX_HISTORY_CACHE) {
              next = next.slice(0, MAX_HISTORY_CACHE)
            }
          }
          set({ history: next, historyLoadedAt: Date.now(), historyError: null })
          appLog('history.fetch.success', { count: next.length, delta: delta?.length ?? 0 })
      } catch (e: unknown) {
      const msg = String((e as Error)?.message ?? e)
      set({ historyError: msg })
      appLog('history.fetch.error', { error: msg }, 'error')
    } finally {
      set({ loadingHistory: false })
    }
  },
      loadThread: async (requestThread: (id: string, path?: string) => Promise<ThreadResponse | undefined>, id: string, path?: string) => {
        const key = id
        const cur = get().thread[key]
        // Fetch if missing or if we only have a partial preview
        if (cur && !cur.partial) return cur
        set({ loadingThread: { ...get().loadingThread, [key]: true } })
        try {
          appLog('thread.fetch.start', { via: 'ws', id, path })
          const json = await requestThread(id, path)
          if (json) {
            set({ thread: { ...get().thread, [key]: json } })
            appLog('thread.fetch.success', { id, items: json?.items?.length ?? 0 })
          }
          return json ?? undefined
        } finally {
          const { [key]: _, ...rest } = get().loadingThread
          set({ loadingThread: rest })
        }
      },
    }),
    {
      name: '@openagents/threads-v1',
      version: 1,
      skipHydration: true,
      partialize: (state) => ({
        history: (state.history || []).slice(0, MAX_HISTORY_CACHE),
        historyLoadedAt: state.historyLoadedAt,
        threadProject: state.threadProject,
      }),
      storage: createJSONStorage(() => AsyncStorage),
      onRehydrateStorage: () => (state, error) => {
        try {
          useThreads.setState({ rehydrated: true })
        } catch {}
      },
      migrate: (persisted: unknown) => {
        try {
          if (persisted && typeof persisted === 'object') {
            const next: Record<string, unknown> = { ...(persisted as Record<string, unknown>) };
            if (Array.isArray(next.items) && !Array.isArray(next.history)) {
              (next as any).history = next.items;
              delete next.items;
            }
            if (Array.isArray(next.history) && next.history.length > MAX_HISTORY_CACHE) {
              (next as any).history = next.history.slice(0, MAX_HISTORY_CACHE);
            }
            if ((next as any).thread && typeof (next as any).thread === 'object') {
              (next as any).thread = {};
            }
            return next;
          }
        } catch {}
        return persisted;
      },
    }
  )
)

let threadsRehydratePromise: Promise<void> | null = null;

export function ensureThreadsRehydrated(): Promise<void> {
  if (!threadsRehydratePromise) {
    threadsRehydratePromise = (async () => {
      try {
        await useThreads.persist.rehydrate?.();
      } catch {
        useThreads.setState({ rehydrated: true });
      }
      if (!useThreads.getState().rehydrated) {
        useThreads.setState({ rehydrated: true });
      }
    })();
  }
  return threadsRehydratePromise;
}
