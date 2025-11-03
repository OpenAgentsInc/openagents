import { createJSONStorage } from 'zustand/middleware'

// Environment-agnostic storage helper for Zustand persist.
// - Web: window.localStorage
// - React Native: AsyncStorage (required lazily)
// - Fallback: in-memory shim
export function universalJSONStorage<T = unknown>() {
  if (typeof window !== 'undefined' && (window as any).localStorage) {
    return createJSONStorage<T>(() => window.localStorage as any)
  }
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const AsyncStorage = require('@react-native-async-storage/async-storage').default
    if (AsyncStorage) return createJSONStorage<T>(() => AsyncStorage)
  } catch {}
  const memory = {
    getItem: async (_key: string) => null as any,
    setItem: async (_key: string, _value: string) => {},
    removeItem: async (_key: string) => {},
  }
  return createJSONStorage<T>(() => memory as any)
}

