import { create } from 'zustand'
import { persist } from 'zustand/middleware/persist'
import { persistStorage } from './persist-storage'

type OnboardingState = {
  completed: boolean
  rehydrated: boolean
  setCompleted: (v: boolean) => void
}

export const useOnboarding = create<OnboardingState>()(
  persist(
    (set) => ({
      completed: false,
      rehydrated: false,
      setCompleted: (v) => set({ completed: v }),
    }),
    {
      name: '@openagents/onboarding-v1',
      storage: persistStorage(),
      onRehydrateStorage: () => (state, error) => {
        try { useOnboarding.setState({ rehydrated: true }) } catch {}
      },
    }
  )
)
