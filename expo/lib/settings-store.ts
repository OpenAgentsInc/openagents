import { create } from 'zustand/traditional'
import { persist } from 'zustand/middleware'
import { persistStorage } from './persist-storage'

export type Approvals = 'never' | 'on-request' | 'on-failure'

type SettingsState = {
  bridgeHost: string
  bridgeCode: string
  bridgeToken: string
  bridgeAutoReconnect: boolean
  readOnly: boolean
  networkEnabled: boolean
  approvals: Approvals
  attachPreface: boolean
  agentProvider: 'codex' | 'claude_code'
  updatesAutoPoll: boolean
  lastRoute: string
  // Bridge sync preferences (persisted):
  // - syncEnabled controls the Codex sessions watcher (inbound sync)
  // - syncTwoWay controls the provider-native writer (outbound sync)
  syncEnabled: boolean
  syncTwoWay: boolean
  setBridgeHost: (v: string) => void
  setBridgeCode: (v: string) => void
  setBridgeToken: (v: string) => void
  setBridgeAutoReconnect: (v: boolean) => void
  setReadOnly: (v: boolean) => void
  setNetworkEnabled: (v: boolean) => void
  setApprovals: (v: Approvals) => void
  setAttachPreface: (v: boolean) => void
  setAgentProvider: (v: 'codex' | 'claude_code') => void
  setUpdatesAutoPoll: (v: boolean) => void
  setLastRoute: (v: string) => void
  setSyncEnabled: (v: boolean) => void
  setSyncTwoWay: (v: boolean) => void
}

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      // No default host; user must paste a Bridge Code or set host manually
      bridgeHost: '',
      bridgeCode: '',
      bridgeToken: '',
      // Do not auto-reconnect until the user explicitly presses Connect
      bridgeAutoReconnect: false,
      readOnly: false,
      networkEnabled: true,
      approvals: 'never',
      attachPreface: true,
      agentProvider: 'codex',
      updatesAutoPoll: false,
      lastRoute: '',
      // Default to watcher ON and two-way OFF unless user overrides.
      syncEnabled: true,
      syncTwoWay: false,
      setBridgeHost: (v) => set({ bridgeHost: sanitizeHostInput(v) }),
      setBridgeCode: (v) => set({ bridgeCode: v }),
      setBridgeToken: (v) => set({ bridgeToken: v }),
      setBridgeAutoReconnect: (v) => set({ bridgeAutoReconnect: v }),
      setReadOnly: (v) => set({ readOnly: v }),
      setNetworkEnabled: (v) => set({ networkEnabled: v }),
      setApprovals: (v) => set({ approvals: v }),
      setAttachPreface: (v) => set({ attachPreface: v }),
      setAgentProvider: (v) => set({ agentProvider: v }),
      setUpdatesAutoPoll: (v) => set({ updatesAutoPoll: v }),
      setLastRoute: (v) => set({ lastRoute: v }),
      setSyncEnabled: (v) => set({ syncEnabled: v }),
      setSyncTwoWay: (v) => set({ syncTwoWay: v }),
    }),
    {
      name: '@openagents/settings-v4',
      version: 4,
      storage: persistStorage(),
      // Migrate and sanitize legacy values (remove any bore/localhost defaults, clear bad hosts)
      migrate: (persisted: any, from) => {
        try {
          const obj = (persisted && typeof persisted === 'object') ? { ...persisted } : persisted
          if (!obj || typeof obj !== 'object') return persisted
          const host = String(obj.bridgeHost || '').trim()
          // Always start with explicit user connect; turn off auto-reconnect on migration
          obj.bridgeAutoReconnect = false
          // Drop legacy convexUrl and related fields
          if ('convexUrl' in obj) delete obj.convexUrl
          if (/\bbore(\.pub)?\b/.test(host) || host.startsWith('ws://bore') || host.includes('bore.pub') || host.startsWith('localhost:') || host.startsWith('127.0.0.1:')) {
            obj.bridgeHost = ''
            obj.bridgeCode = ''
            obj.bridgeToken = ''
          }
          // Sanitize any malformed or duplicated host strings stored previously
          if (obj.bridgeHost && typeof obj.bridgeHost === 'string') {
            obj.bridgeHost = sanitizeHostInput(String(obj.bridgeHost))
          }
          return obj
        } catch {
          return persisted
        }
      },
    }
  )
)

// Best-effort sanitize for Bridge Host input to prevent accidental concatenation
// or protocol fragments. Keeps only the first plausible host:port, strips ws://,
// http(s)://, trailing / and /ws suffix.
function sanitizeHostInput(raw: string): string {
  try {
    const val = String(raw || '').trim()
    if (!val) return ''
    const m = val.match(/((?:[a-zA-Z0-9.-]+|\d{1,3}(?:\.\d{1,3}){3}):\d{2,5})/)
    const basis = m ? m[1] : val
    return basis
      .replace(/^ws:\/\//i, '')
      .replace(/^wss:\/\//i, '')
      .replace(/^http:\/\//i, '')
      .replace(/^https:\/\//i, '')
      .replace(/\/$/, '')
      .replace(/\/ws$/i, '')
      .replace(/\/$/, '')
  } catch {
    return ''
  }
}
