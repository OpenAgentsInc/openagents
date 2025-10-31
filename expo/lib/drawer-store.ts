import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

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
      storage: createJSONStorage(() => AsyncStorage),
    }
  )
)

