import { create } from 'zustand/traditional'
import { persist } from 'zustand/middleware'
import { persistStorage } from './persist-storage'

type DrawerState = {
  open: boolean
  setOpen: (v: boolean) => void
}

export const useDrawerStore = create<DrawerState>()(
  persist(
    (set) => ({
      open: false,
      setOpen: (v: boolean) => set({ open: v }),
    }),
    {
      name: '@openagents/drawer-v1',
      version: 1,
      storage: persistStorage(),
    }
  )
)
