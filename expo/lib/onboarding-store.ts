import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

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
      storage: createJSONStorage(() => AsyncStorage),
      onRehydrateStorage: () => (state, error) => {
        try { useOnboarding.setState({ rehydrated: true }) } catch {}
      },
    }
  )
)

