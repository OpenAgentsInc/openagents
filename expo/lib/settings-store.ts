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
      name: '@openagents/settings-v2',
      version: 2,
      storage: createJSONStorage(() => AsyncStorage),
      // Migrate possible legacy payloads from providers/ws.tsx (stringified object)
      migrate: (persisted: any, from) => {
        try {
          const obj = (persisted && typeof persisted === 'object') ? { ...persisted } : persisted
          if (!obj || typeof obj !== 'object') return persisted
          const host = String(obj.bridgeHost || '').trim()
          if (/\bbore(\.pub)?\b/.test(host) || host.startsWith('ws://bore') || host.includes('bore.pub')) {
            obj.bridgeHost = ''
            obj.bridgeCode = ''
            obj.bridgeToken = ''
            obj.bridgeAutoReconnect = false
          }
          return obj
        } catch {
          return persisted
        }
      },
    }
  )
)
