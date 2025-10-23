import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'

export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string }
export type SessionItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
export type SessionResponse = { title: string; items: SessionItem[] }

type SessionsState = {
  history: HistoryItem[]
  loadingHistory: boolean
  historyLoadedAt?: number
  session: Record<string, SessionResponse | undefined>
  loadingSession: Record<string, boolean>
  loadHistory: (baseWsUrl: string) => Promise<void>
  loadSession: (baseWsUrl: string, id: string, path?: string) => Promise<SessionResponse | undefined>
}

const HISTORY_KEY = '@openagents/sessions-history-v1'

function wsToHttpBase(wsUrl: string): string {
  try {
    const u = new URL(wsUrl)
    const proto = u.protocol === 'wss:' ? 'https:' : 'http:'
    const origin = `${proto}//${u.host}`
    return origin
  } catch {
    return 'http://localhost:8787'
  }
}

export const useSessions = create<SessionsState>((set, get) => ({
  history: [],
  loadingHistory: false,
  historyLoadedAt: undefined,
  session: {},
  loadingSession: {},
  loadHistory: async (wsUrl: string) => {
    const base = wsToHttpBase(wsUrl)
    set({ loadingHistory: true })
    try {
      // hydrate cache first
      try {
        const raw = await AsyncStorage.getItem(HISTORY_KEY)
        if (raw) {
          const cached = JSON.parse(raw) as { items?: HistoryItem[] }
          if (Array.isArray(cached?.items)) set({ history: cached.items })
        }
      } catch {}
      const res = await fetch(`${base}/history`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = await res.json()
      const items = Array.isArray(json.items) ? (json.items as HistoryItem[]) : []
      set({ history: items, historyLoadedAt: Date.now() })
      try { await AsyncStorage.setItem(HISTORY_KEY, JSON.stringify({ items })) } catch {}
    } finally {
      set({ loadingHistory: false })
    }
  },
  loadSession: async (wsUrl: string, id: string, path?: string) => {
    const base = wsToHttpBase(wsUrl)
    const key = id
    const cur = get().session[key]
    if (cur) return cur
    set({ loadingSession: { ...get().loadingSession, [key]: true } })
    try {
      const url = new URL(`${base}/session`)
      url.searchParams.set('id', id)
      if (path) url.searchParams.set('path', path)
      const res = await fetch(url.toString())
      if (!res.ok) return undefined
      const json = (await res.json()) as SessionResponse
      set({ session: { ...get().session, [key]: json } })
      return json
    } finally {
      const { [key]: _, ...rest } = get().loadingSession
      set({ loadingSession: rest })
    }
  },
}))

