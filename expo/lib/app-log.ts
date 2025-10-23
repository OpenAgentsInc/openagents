import { create } from 'zustand'

export type AppLogLevel = 'info' | 'warn' | 'error'
export type AppLogItem = { ts: number; level: AppLogLevel; event: string; details?: any }

type AppLogState = {
  logs: AppLogItem[]
  add: (level: AppLogLevel, event: string, details?: any) => void
  clear: () => void
}

export const useAppLogStore = create<AppLogState>((set, get) => ({
  logs: [],
  add: (level, event, details) => {
    const item: AppLogItem = { ts: Date.now(), level, event, details }
    const next = [...get().logs, item]
    // Keep last 500
    if (next.length > 500) next.splice(0, next.length - 500)
    set({ logs: next })
  },
  clear: () => set({ logs: [] }),
}))

export function appLog(event: string, details?: any, level: AppLogLevel = 'info') {
  try { useAppLogStore.getState().add(level, event, details) } catch {}
}

