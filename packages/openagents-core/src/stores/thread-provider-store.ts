import { create } from "zustand"
import { persist } from "zustand/middleware"
import { universalJSONStorage } from "./persist"

export type AgentProvider = 'codex' | 'claude_code'

type ThreadProviderState = {
  byThread: Record<string, AgentProvider>
  setProvider: (threadId: string, provider: AgentProvider) => void
}

export const useThreadProviders = create<ThreadProviderState>()(
  persist(
    (set) => ({
      byThread: {},
      setProvider: (threadId, provider) => set((s) => ({ byThread: { ...s.byThread, [threadId]: provider } })),
    }),
    {
      name: '@openagents/thread-provider-v1',
      version: 1,
      storage: universalJSONStorage(),
      partialize: (s) => ({ byThread: s.byThread }),
    }
  )
)
