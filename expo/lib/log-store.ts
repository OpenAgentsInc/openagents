import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type LogKind = 'md' | 'reason' | 'text' | 'json' | 'summary' | 'delta' | 'exec' | 'file' | 'search' | 'mcp' | 'todo' | 'cmd' | 'err' | 'turn' | 'thread' | 'item_lifecycle'

export type LogDetail = {
  id: number
  text: string
  kind: LogKind
  deemphasize?: boolean
  ts?: number
  // Optional pointer: for summary/preview items, link to the full JSON/detail entry.
  detailId?: number
}

type LogState = {
  logs: LogDetail[]
  add: (entry: LogDetail) => void
  clear: () => void
}

export const useLogStore = create<LogState>()(
  persist(
    (set, get) => ({
      logs: [],
      add: (entry) => {
        try {
          const txt = String(entry.text ?? '')
          if (txt.includes('exec_command_end')) return
        } catch {}
        const cur = get().logs
        const next = [...cur, entry]
        set({ logs: next })
      },
      clear: () => set({ logs: [] }),
    }),
    {
      name: '@openagents/logs-v2',
      version: 2,
      storage: createJSONStorage(() => AsyncStorage),
      migrate: (persisted: any, from) => {
        // Migrate v1: plain array of LogDetail
        try {
          if (Array.isArray(persisted)) return { logs: persisted as LogDetail[] }
        } catch {}
        return persisted
      },
      partialize: (s) => ({ logs: s.logs }),
    }
  )
)

// Back-compat API used by screens
export function putLog(detail: LogDetail) {
  useLogStore.getState().add(detail)
}

export function getLog(id: number): LogDetail | undefined {
  const arr = useLogStore.getState().logs
  return arr.find((d) => d.id === id)
}

// Snapshot for useSyncExternalStore; referentially stable until logs mutate
export function getAllLogs(): LogDetail[] {
  return useLogStore.getState().logs
}

export function isHydrated(): boolean {
  // Zustand persist hydrates automatically; treat as true
  return true
}

export async function loadLogs(): Promise<LogDetail[]> {
  try { await useLogStore.persist.rehydrate?.() } catch {}
  return useLogStore.getState().logs
}

export async function saveLogs(): Promise<void> {
  // No-op: persist handled by middleware
}

export async function flushLogs(): Promise<void> {
  // No-op
}

export async function clearLogs(): Promise<void> {
  useLogStore.getState().clear()
}
