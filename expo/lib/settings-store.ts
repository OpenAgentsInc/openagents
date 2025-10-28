import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type Approvals = 'never' | 'on-request' | 'on-failure'

type SettingsState = {
  bridgeHost: string
  bridgeCode: string
  convexUrl: string
  bridgeToken: string
  bridgeAutoReconnect: boolean
  readOnly: boolean
  networkEnabled: boolean
  approvals: Approvals
  attachPreface: boolean
  setBridgeHost: (v: string) => void
  setBridgeCode: (v: string) => void
  setConvexUrl: (v: string) => void
  setBridgeToken: (v: string) => void
  setBridgeAutoReconnect: (v: boolean) => void
  setReadOnly: (v: boolean) => void
  setNetworkEnabled: (v: boolean) => void
  setApprovals: (v: Approvals) => void
  setAttachPreface: (v: boolean) => void
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      bridgeHost: 'localhost:8787',
      bridgeCode: '',
      convexUrl: '',
      bridgeToken: '',
      bridgeAutoReconnect: true,
      readOnly: false,
      networkEnabled: true,
      approvals: 'never',
      attachPreface: true,
      setBridgeHost: (v) => set({ bridgeHost: v }),
      setBridgeCode: (v) => set({ bridgeCode: v }),
      setConvexUrl: (v) => set({ convexUrl: v }),
      setBridgeToken: (v) => set({ bridgeToken: v }),
      setBridgeAutoReconnect: (v) => set({ bridgeAutoReconnect: v }),
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
