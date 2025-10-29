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

