import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

interface AgentState {
  githubToken: string
  agentPurpose: string
  setGithubToken: (token: string) => void
  setAgentPurpose: (purpose: string) => void
  clearAgentData: () => void
}

// Simple Zustand store with SSR safety
const useAgentStore = create<AgentState>()(
  persist(
    (set) => ({
      githubToken: '',
      agentPurpose: '',
      setGithubToken: (token: string) => set({ githubToken: token }),
      setAgentPurpose: (purpose: string) => set({ agentPurpose: purpose }),
      clearAgentData: () => set({ githubToken: '', agentPurpose: '' }),
    }),
    {
      name: 'openagents-storage',
      storage: createJSONStorage(() => {
        // Provide fallback for SSR
        if (typeof window === 'undefined') {
          return {
            getItem: () => '',
            setItem: () => {},
            removeItem: () => {},
          }
        }
        return sessionStorage
      }),
      // Only persist agentPurpose, not the token
      partialize: (state) => ({ agentPurpose: state.agentPurpose }),
    },
  ),
)

export { useAgentStore }