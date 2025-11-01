import React from 'react'

export function isDevEnv(): boolean {
  try {
    const v = String(process.env.EXPO_PUBLIC_ENV || '').trim().toLowerCase()
    return v === 'development'
  } catch { return false }
}

export function useIsDevEnv(): boolean {
  return React.useMemo(() => isDevEnv(), [])
}

export function devBridgeHost(): string | null {
  try {
    const h = String(process.env.EXPO_PUBLIC_BRIDGE_HOST || process.env.EXPO_PUBLIC_BRIDGE || '').trim()
    return h || null
  } catch { return null }
}

export function devBridgeToken(): string | null {
  try {
    const t = String(process.env.EXPO_PUBLIC_BRIDGE_TOKEN || '').trim()
    return t || null
  } catch { return null }
}
