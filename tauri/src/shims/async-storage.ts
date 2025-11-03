// Web/Tauri shim for React Native AsyncStorage
// Provides the minimal API shape used by Zustand's createJSONStorage.
// Uses window.localStorage when available; otherwise falls back to an in-memory map.

type StorageLike = {
  getItem(key: string): Promise<string | null>
  setItem(key: string, value: string): Promise<void>
  removeItem(key: string): Promise<void>
}

const memory = new Map<string, string>()

const hasLocal = typeof window !== 'undefined' && !!window.localStorage

const storage: StorageLike = hasLocal
  ? {
      async getItem(key) {
        try { return window.localStorage.getItem(key) } catch { return null }
      },
      async setItem(key, value) {
        try { window.localStorage.setItem(key, value) } catch {}
      },
      async removeItem(key) {
        try { window.localStorage.removeItem(key) } catch {}
      },
    }
  : {
      async getItem(key) { return memory.has(key) ? memory.get(key)! : null },
      async setItem(key, value) { memory.set(key, value) },
      async removeItem(key) { memory.delete(key) },
    }

export default storage

