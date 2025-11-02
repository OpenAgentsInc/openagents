import { router } from 'expo-router'

/**
 * Typed router wrapper to eliminate `as any` casts at call sites.
 * 
 * Expo Router has strict type checking on dynamic routes which requires
 * `as any` casts for template strings. This wrapper centralizes those casts
 * in one place rather than scattering them throughout the app.
 * 
 * Usage:
 *   import { typedRouter } from '@/lib/typed-router'
 *   typedRouter.push('/thread/new')
 *   typedRouter.push(`/thread/${id}`)
 */
export const typedRouter = {
  /**
   * Navigate to a route. Works with both static and dynamic paths.
   */
  push: (path: string) => {
    try {
      router.push(path as any)
    } catch (e) {
      // Silently fail - matches existing error handling pattern
    }
  },

  /**
   * Replace the current route with a new one.
   */
  replace: (path: string) => {
    try {
      router.replace(path as any)
    } catch (e) {
      // Silently fail - matches existing error handling pattern
    }
  },

  /**
   * Navigate back in the history.
   */
  back: () => {
    try {
      router.back()
    } catch (e) {
      // Silently fail
    }
  },

  /**
   * Check if we can go back in the history.
   */
  canGoBack: () => {
    try {
      return router.canGoBack()
    } catch {
      return false
    }
  },
}
