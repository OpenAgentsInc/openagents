import { create } from 'zustand'
import { persist } from 'zustand/middleware/persist'
import { universalJSONStorage } from './persist'

type DrawerState = { open: boolean; setOpen: (v: boolean) => void }

export const useDrawerStore = create<DrawerState>()(
  persist(
    (set) => ({ open: false, setOpen: (v) => set({ open: v }) }),
    { name: '@openagents/drawer-v1', version: 1, storage: universalJSONStorage() }
  )
)
