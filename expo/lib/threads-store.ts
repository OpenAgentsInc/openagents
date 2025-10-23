import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { appLog } from '@/lib/app-log'
import { useBridge } from '@/providers/ws'

export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean }
export type ThreadItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string; resume_id?: string }

type ThreadsState = {
  history: HistoryItem[]
  loadingHistory: boolean
  historyLoadedAt?: number
  historyError?: string | null
  lastHistoryUrl?: string
  thread: Record<string, ThreadResponse | undefined>
  loadingThread: Record<string, boolean>
  // Ephemeral mapping of live thread_id (UUID) -> projectId set when a thread starts
  threadProject: Record<string, string | undefined>
  setThreadProject: (threadId: string, projectId: string) => void
  loadHistory: (requestHistory: () => Promise<HistoryItem[]>) => Promise<void>
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
      threadProject: {},
      setThreadProject: (threadId: string, projectId: string) => {
        const cur = get().threadProject
        set({ threadProject: { ...cur, [threadId]: projectId } })
      },
      loadHistory: async (requestHistory: () => Promise<HistoryItem[]>) => {
        set({ loadingHistory: true })
        try {
          appLog('history.fetch.start', { via: 'ws' })
          const items = await requestHistory()
          set({ history: items, historyLoadedAt: Date.now(), historyError: null })
          appLog('history.fetch.success', { count: items.length })
        } catch (e: any) {
          const msg = String(e?.message ?? e)
          set({ historyError: msg })
          appLog('history.fetch.error', { error: msg }, 'error')
        } finally {
          set({ loadingHistory: false })
        }
      },
      loadThread: async (requestThread: (id: string, path?: string) => Promise<ThreadResponse | undefined>, id: string, path?: string) => {
        const key = id
        const cur = get().thread[key]
        if (cur) return cur
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
      partialize: (state) => ({
        history: state.history,
        historyLoadedAt: state.historyLoadedAt,
        thread: state.thread,
        threadProject: state.threadProject,
      }),
      storage: createJSONStorage(() => AsyncStorage),
      migrate: (persisted: any) => {
        // Legacy migration from HISTORY_KEY cache shape { items: HistoryItem[] }
        try {
          if (persisted && typeof persisted === 'object' && Array.isArray(persisted.items) && !Array.isArray(persisted.history)) {
            return { ...persisted, history: persisted.items, items: undefined }
          }
        } catch {}
        return persisted
      },
    }
  )
)
