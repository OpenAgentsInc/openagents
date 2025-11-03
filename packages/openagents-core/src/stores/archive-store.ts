import { create } from 'zustand'

type ArchiveState = {
  archived: Record<string, true>
  isArchived: (id: string) => boolean
  archive: (id: string) => void
  unarchive: (id: string) => void
}

export const useArchiveStore = create<ArchiveState>()((set, get) => ({
  archived: {},
  isArchived: (id) => Boolean(get().archived[id]),
  archive: (id) => set((s) => ({ archived: { ...s.archived, [id]: true } })),
  unarchive: (id) => set((s) => { const next = { ...s.archived }; delete next[id]; return { archived: next } }),
}))

