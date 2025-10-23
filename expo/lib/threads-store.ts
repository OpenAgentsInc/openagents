import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { appLog } from '@/lib/app-log'
import { useBridge } from '@/providers/ws'

export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean }
export type ThreadItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string }

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

const HISTORY_KEY = '@openagents/threads-history-v1'

export const useThreads = create<ThreadsState>((set, get) => ({
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
      try {
        const raw = await AsyncStorage.getItem(HISTORY_KEY)
        if (raw) {
          const cached = JSON.parse(raw) as { items?: HistoryItem[] }
          if (Array.isArray(cached?.items)) set({ history: cached.items })
        }
      } catch {}
      appLog('history.fetch.start', { via: 'ws' })
      const items = await requestHistory()
      set({ history: items, historyLoadedAt: Date.now(), historyError: null })
      appLog('history.fetch.success', { count: items.length })
      try { await AsyncStorage.setItem(HISTORY_KEY, JSON.stringify({ items })) } catch {}
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
}))
