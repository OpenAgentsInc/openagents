import { useEffect, useState } from 'react'

import { AIUR_ACCESS_PATH } from '@/auth/access-route'

export type AiurAccessUiState =
  | Readonly<{ kind: 'loading' }>
  | Readonly<{ kind: 'signed_out' }>
  | Readonly<{
      kind: 'denied'
      user: Readonly<{ login: string | undefined; name: string; avatarUrl: string }>
    }>
  | Readonly<{
      kind: 'owner'
      user: Readonly<{ login: string | undefined; name: string; avatarUrl: string }>
    }>

const fetchAccess = async (): Promise<AiurAccessUiState> => {
  try {
    const response = await fetch(AIUR_ACCESS_PATH, { method: 'GET' })
    if (!response.ok) return { kind: 'signed_out' }
    const body = (await response.json()) as { kind?: unknown }
    if (body.kind === 'owner' || body.kind === 'denied') {
      return body as AiurAccessUiState
    }
    return { kind: 'signed_out' }
  } catch {
    return { kind: 'signed_out' }
  }
}

export function useAiurAccess(): AiurAccessUiState {
  const [state, setState] = useState<AiurAccessUiState>({ kind: 'loading' })

  useEffect(() => {
    let cancelled = false
    void fetchAccess().then(result => {
      if (!cancelled) setState(result)
    })
    return () => {
      cancelled = true
    }
  }, [])

  return state
}
