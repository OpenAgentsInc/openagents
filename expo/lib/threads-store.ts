import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'

export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean }
export type ThreadItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string }

type ThreadsState = {
  history: HistoryItem[]
  loadingHistory: boolean
  historyLoadedAt?: number
  thread: Record<string, ThreadResponse | undefined>
  loadingThread: Record<string, boolean>
  // Ephemeral mapping of live thread_id (UUID) -> projectId set when a thread starts
  threadProject: Record<string, string | undefined>
  setThreadProject: (threadId: string, projectId: string) => void
  loadHistory: (httpBase: string) => Promise<void>
  loadThread: (httpBase: string, id: string, path?: string) => Promise<ThreadResponse | undefined>
}

const HISTORY_KEY = '@openagents/threads-history-v1'

function wsToHttpBase(wsUrl: string): string {
  try {
    const u = new URL(wsUrl)
    const proto = u.protocol === 'wss:' ? 'https:' : 'http:'
    return `${proto}//${u.host}`
  } catch {
    return 'http://localhost:8787'
  }
}

export const useThreads = create<ThreadsState>((set, get) => ({
  history: [],
  loadingHistory: false,
  historyLoadedAt: undefined,
  thread: {},
  loadingThread: {},
  threadProject: {},
  setThreadProject: (threadId: string, projectId: string) => {
    const cur = get().threadProject
    set({ threadProject: { ...cur, [threadId]: projectId } })
  },
  loadHistory: async (httpBase: string) => {
    const base = httpBase
    set({ loadingHistory: true })
    try {
      try {
        const raw = await AsyncStorage.getItem(HISTORY_KEY)
        if (raw) {
          const cached = JSON.parse(raw) as { items?: HistoryItem[] }
          if (Array.isArray(cached?.items)) set({ history: cached.items })
        }
      } catch {}
      try {
        const url = `${base}/history`
        const res = await fetch(url)
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        const json = await res.json()
        const items = Array.isArray(json.items) ? (json.items as HistoryItem[]) : []
        set({ history: items, historyLoadedAt: Date.now() })
        try { await AsyncStorage.setItem(HISTORY_KEY, JSON.stringify({ items })) } catch {}
      } catch (e) {
        try { console.warn('[threads] history fetch failed', { base, error: String((e as any)?.message ?? e) }) } catch {}
      }
    } finally {
      set({ loadingHistory: false })
    }
  },
  loadThread: async (httpBase: string, id: string, path?: string) => {
    const base = httpBase
    const key = id
    const cur = get().thread[key]
    if (cur) return cur
    set({ loadingThread: { ...get().loadingThread, [key]: true } })
    try {
      const url = new URL(`${base}/thread`)
      url.searchParams.set('id', id)
      if (path) url.searchParams.set('path', path)
      const res = await fetch(url.toString())
      if (!res.ok) return undefined
      const json = (await res.json()) as ThreadResponse
      set({ thread: { ...get().thread, [key]: json } })
      return json
    } finally {
      const { [key]: _, ...rest } = get().loadingThread
      set({ loadingThread: rest })
    }
  },
}))
