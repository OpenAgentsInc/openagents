import { create } from 'zustand/traditional'
import { persist } from 'zustand/middleware'
import { persistStorage } from './persist-storage'

export type AgentProvider = 'codex' | 'claude_code'

type ThreadProviderState = {
  byThread: Record<string, AgentProvider>
  setProvider: (threadId: string, provider: AgentProvider) => void
  getProvider: (threadId: string) => AgentProvider | undefined
  clear: () => void
}

export const useThreadProviders = create<ThreadProviderState>()(
  persist(
    (set, get) => ({
      byThread: {},
      setProvider: (threadId, provider) => {
        const cur = get().byThread
        set({ byThread: { ...cur, [threadId]: provider } })
      },
      getProvider: (threadId) => get().byThread[threadId],
      clear: () => set({ byThread: {} }),
    }),
    {
      name: '@openagents/thread-provider-v1',
      version: 1,
      storage: persistStorage(),
      partialize: (s) => ({ byThread: s.byThread }),
    }
  )
)
