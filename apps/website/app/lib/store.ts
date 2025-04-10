import { create } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'

// Define agent type
export interface Agent {
  id: string
  purpose: string
  createdAt: number // timestamp
}

interface AgentState {
  // Current form state
  githubToken: string
  agentPurpose: string
  
  // Agents collection
  agents: Agent[]
  
  // Actions
  setGithubToken: (token: string) => void
  setAgentPurpose: (purpose: string) => void
  addAgent: (agent: Agent) => void
  getAgent: (id: string) => Agent | undefined
  clearAgentData: () => void
}

// Simple Zustand store with SSR safety
const useAgentStore = create<AgentState>()(
  persist(
    (set, get) => ({
      githubToken: '',
      agentPurpose: '',
      agents: [],
      
      setGithubToken: (token: string) => set({ githubToken: token }),
      setAgentPurpose: (purpose: string) => set({ agentPurpose: purpose }),
      
      addAgent: (agent: Agent) => set(state => ({
        agents: [...state.agents, agent],
        // Clear form data after adding agent
        agentPurpose: '',
      })),
      
      getAgent: (id: string) => {
        return get().agents.find(agent => agent.id === id);
      },
      
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
      // Only persist agentPurpose and agents, not the token
      partialize: (state) => ({ 
        agentPurpose: state.agentPurpose,
        agents: state.agents 
      }),
    },
  ),
)

export { useAgentStore }