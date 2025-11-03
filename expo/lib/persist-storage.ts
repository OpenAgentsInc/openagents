import { Platform } from 'react-native'
import { createJSONStorage } from 'zustand/middleware'

// Provides a persist storage that is safe across native, web, and SSR.
// - Native: uses AsyncStorage (required lazily)
// - Web (browser): uses window.localStorage
// - SSR/Node: uses a no-op in-memory shim to avoid `window` access during import
export function persistStorage() {
  if (Platform.OS === 'web') {
    if (typeof window !== 'undefined' && (window as any).localStorage) {
      return createJSONStorage(() => window.localStorage)
    }
    const memory = {
      getItem: async (_key: string) => null as any,
      setItem: async (_key: string, _value: string) => {},
      removeItem: async (_key: string) => {},
    }
    return createJSONStorage(() => memory as any)
  }
  // Native platforms: require lazily to avoid touching it during SSR
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const AsyncStorage = require('@react-native-async-storage/async-storage').default
  return createJSONStorage(() => AsyncStorage)
}
