/**
 * Effuse HMR State Registry
 *
 * Preserves component state across hot reloads using a global window object.
 * State is saved continuously via StateCell.changes stream and restored
 * when components remount after reload.
 */

declare global {
  interface Window {
    __EFFUSE_HMR__?: HMRRegistry
  }
}

interface HMRRegistry {
  components: Map<string, unknown>
  version: number
}

/**
 * Check if running in browser environment
 */
const isBrowser = typeof window !== "undefined"

/**
 * Get or create the HMR registry on window.
 * Returns null if not in browser environment.
 */
const getRegistry = (): HMRRegistry | null => {
  if (!isBrowser) return null
  if (!window.__EFFUSE_HMR__) {
    window.__EFFUSE_HMR__ = {
      components: new Map(),
      version: 0,
    }
  }
  return window.__EFFUSE_HMR__
}

/**
 * Save component state to the registry.
 * Called continuously via Stream.tap on state.changes.
 *
 * Uses structuredClone to ensure state is safely serializable
 * and won't have reference issues across reloads.
 *
 * No-op in non-browser environments (tests).
 */
export const saveComponentState = (componentId: string, state: unknown): void => {
  const registry = getRegistry()
  if (!registry) return // Not in browser, skip silently
  try {
    // Use structuredClone for deep copy and to ensure serializability
    registry.components.set(componentId, structuredClone(state))
  } catch (e) {
    // If structuredClone fails (non-serializable state), skip saving
    console.warn(`[Effuse HMR] Could not save state for "${componentId}":`, e)
  }
}

/**
 * Load and consume preserved component state.
 * Returns the state if available, then removes it from registry.
 *
 * One-time consumption ensures components don't accidentally restore
 * stale state on subsequent mounts.
 *
 * Returns undefined in non-browser environments.
 */
export const loadComponentState = <S>(componentId: string): S | undefined => {
  const registry = getRegistry()
  if (!registry) return undefined // Not in browser
  const state = registry.components.get(componentId) as S | undefined
  if (state !== undefined) {
    registry.components.delete(componentId)
    console.log(`[Effuse HMR] Restored state for "${componentId}"`)
  }
  return state
}

/**
 * Check if preserved state exists for a component.
 * Does not consume the state.
 *
 * Returns false in non-browser environments.
 */
export const hasComponentState = (componentId: string): boolean => {
  const registry = getRegistry()
  if (!registry) return false
  return registry.components.has(componentId)
}

/**
 * Clear all preserved state.
 * Useful for forcing fresh state on next reload.
 *
 * No-op in non-browser environments.
 */
export const clearAllState = (): void => {
  const registry = getRegistry()
  if (!registry) return
  registry.components.clear()
  console.log("[Effuse HMR] Cleared all preserved state")
}

/**
 * Get the HMR version number.
 * Incremented on each reload for debugging.
 *
 * Returns 0 in non-browser environments.
 */
export const getHMRVersion = (): number => {
  const registry = getRegistry()
  return registry?.version ?? 0
}

/**
 * Increment HMR version (call on reload).
 *
 * Returns 0 in non-browser environments.
 */
export const bumpHMRVersion = (): number => {
  const registry = getRegistry()
  if (!registry) return 0
  registry.version++
  return registry.version
}
