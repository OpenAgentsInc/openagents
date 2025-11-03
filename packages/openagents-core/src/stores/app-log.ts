import { create } from 'zustand'

export type LogLevel = 'info' | 'warn' | 'error'
type LogItem = { id: string; ts: number; level: LogLevel; event: string; details?: unknown }

type AppLogState = {
  logs: LogItem[]
  add: (level: LogLevel, event: string, details?: unknown) => void
  clear: () => void
}

export const useAppLogStore = create<AppLogState>((set, get) => ({
  logs: [],
  add: (level, event, details) => set((s) => ({ logs: [...s.logs, { id: `${Date.now()}-${Math.random()}`, ts: Date.now(), level, event, details }] })),
  clear: () => set({ logs: [] }),
}))

