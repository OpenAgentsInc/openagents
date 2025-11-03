import { create } from 'zustand'

export type AgentProvider = 'codex' | 'claude_code'

type ThreadProviderState = {
  byThread: Record<string, AgentProvider>
  setProvider: (threadId: string, provider: AgentProvider) => void
}

export const useThreadProviders = create<ThreadProviderState>()((set) => ({
  byThread: {},
  setProvider: (threadId, provider) => set((s) => ({ byThread: { ...s.byThread, [threadId]: provider } })),
}))

