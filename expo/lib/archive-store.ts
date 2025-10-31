import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

type ArchiveState = {
  archived: Record<string, { archivedAt: number }>
  archive: (id: string) => void
  unarchive: (id: string) => void
  isArchived: (id: string) => boolean
  list: () => string[]
  clearAll: () => void
}

export const useArchiveStore = create<ArchiveState>()(
  persist(
    (set, get) => ({
      archived: {},
      archive: (id: string) => {
        if (!id) return
        const cur = get().archived || {}
        set({ archived: { ...cur, [id]: { archivedAt: Date.now() } } })
      },
      unarchive: (id: string) => {
        if (!id) return
        const cur = { ...(get().archived || {}) }
        delete cur[id]
        set({ archived: cur })
      },
      isArchived: (id: string) => {
        if (!id) return false
        return !!get().archived?.[id]
      },
      list: () => {
        return Object.keys(get().archived || {})
      },
      clearAll: () => set({ archived: {} }),
    }),
    {
      name: '@openagents/archived-threads-v1',
      version: 1,
      storage: createJSONStorage(() => AsyncStorage),
      partialize: (s) => ({ archived: s.archived }),
    }
  )
)

export function isThreadArchived(id: string): boolean {
  try { return useArchiveStore.getState().isArchived(id) } catch { return false }
}

