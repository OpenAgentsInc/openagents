import { Platform } from 'react-native'

export function isWeb(): boolean {
  try { return Platform.OS === 'web' } catch { return false }
}

export function isMobile(): boolean {
  try { return Platform.OS === 'ios' || Platform.OS === 'android' } catch { return false }
}

export function isDesktop(): boolean {
  // Treat all web as desktop; Tauri detection optional
  if (isWeb()) return true
  return false
}

export function isTauri(): boolean {
  if (!isWeb()) return false
  try { return typeof window !== 'undefined' && !!(window as any).__TAURI__ } catch { return false }
}

