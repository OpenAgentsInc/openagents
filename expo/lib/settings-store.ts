import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type Approvals = 'never' | 'on-request' | 'on-failure'

type SettingsState = {
  bridgeHost: string
  readOnly: boolean
  networkEnabled: boolean
  approvals: Approvals
  attachPreface: boolean
  setBridgeHost: (v: string) => void
  setReadOnly: (v: boolean) => void
  setNetworkEnabled: (v: boolean) => void
  setApprovals: (v: Approvals) => void
  setAttachPreface: (v: boolean) => void
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      bridgeHost: 'localhost:8787',
      readOnly: false,
      networkEnabled: true,
      approvals: 'never',
      attachPreface: true,
      setBridgeHost: (v) => set({ bridgeHost: v }),
      setReadOnly: (v) => set({ readOnly: v }),
      setNetworkEnabled: (v) => set({ networkEnabled: v }),
      setApprovals: (v) => set({ approvals: v }),
      setAttachPreface: (v) => set({ attachPreface: v }),
    }),
    {
      name: '@openagents/settings-v1',
      version: 1,
      storage: createJSONStorage(() => AsyncStorage),
      // Migrate possible legacy payloads from providers/ws.tsx (stringified object)
      migrate: (persisted: any, _from) => {
        try {
          // If persisted is an object with keys from SettingsState, return as-is
          if (persisted && typeof persisted === 'object' && 'bridgeHost' in persisted) {
            return persisted
          }
        } catch {}
        return persisted
      },
    }
  )
)

