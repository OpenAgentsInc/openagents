import { useCallback, useEffect, useState } from 'react'

import { KHALA_SYNC_WEB_SESSION_PATH as KHALA_SYNC_SESSION_PATH } from './-chat-sync-web-core'

/**
 * Client-side session status for the web Khala Sync proxy (#8413). The
 * bearer token itself never reaches this module or any browser JS — it is
 * read straight off the httpOnly cookie server-side by
 * `apps/openagents.com/apps/start/src/khala-sync-proxy.ts`. This hook only
 * asks the proxy "is a token cookie present and did it validate at sign-in
 * time", and posts a fresh `{ ownerUserId, token }` pair through the same
 * proxy for a REAL bootstrap-backed validation
 * (`validateKhalaSyncCredentials`) before the cookie is ever set.
 */

export type KhalaSyncSessionStatus = 'loading' | 'signed_out' | 'signed_in'

export type KhalaSyncSessionState = Readonly<{
  status: KhalaSyncSessionStatus
  ownerUserId: string | null
  signIn: (input: {
    ownerUserId: string
    token: string
  }) => Promise<{ ok: true } | { ok: false; messageSafe: string }>
  signOut: () => Promise<void>
}>

type SessionStatusResponse = Readonly<{
  signedIn: boolean
  ownerUserId: string | null
}>

const fetchSessionStatus = async (): Promise<SessionStatusResponse> => {
  const response = await fetch(KHALA_SYNC_SESSION_PATH, { method: 'GET' })
  if (!response.ok) return { signedIn: false, ownerUserId: null }
  const body: unknown = await response.json().catch(() => null)
  if (typeof body !== 'object' || body === null) return { signedIn: false, ownerUserId: null }
  const record = body as Record<string, unknown>
  return {
    signedIn: record.signedIn === true,
    ownerUserId: typeof record.ownerUserId === 'string' ? record.ownerUserId : null,
  }
}

export function useKhalaSyncSession(): KhalaSyncSessionState {
  const [status, setStatus] = useState<KhalaSyncSessionStatus>('loading')
  const [ownerUserId, setOwnerUserId] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void fetchSessionStatus().then(result => {
      if (cancelled) return
      setStatus(result.signedIn ? 'signed_in' : 'signed_out')
      setOwnerUserId(result.ownerUserId)
    })
    return () => {
      cancelled = true
    }
  }, [])

  const signIn = useCallback(
    async (input: { ownerUserId: string; token: string }) => {
      const trimmedOwnerUserId = input.ownerUserId.trim()
      const trimmedToken = input.token.trim()
      if (trimmedOwnerUserId === '' || trimmedToken === '') {
        return { ok: false as const, messageSafe: 'Owner user id and token are both required.' }
      }
      const response = await fetch(KHALA_SYNC_SESSION_PATH, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ ownerUserId: trimmedOwnerUserId, token: trimmedToken }),
      })
      const body: unknown = await response.json().catch(() => null)
      const ok =
        typeof body === 'object' && body !== null && (body as { ok?: unknown }).ok === true
      if (!ok) {
        const messageSafe =
          typeof body === 'object' && body !== null && 'messageSafe' in body
            ? String((body as { messageSafe: unknown }).messageSafe)
            : `sign-in failed (${response.status})`
        return { ok: false as const, messageSafe }
      }
      setStatus('signed_in')
      setOwnerUserId(trimmedOwnerUserId)
      return { ok: true as const }
    },
    [],
  )

  const signOut = useCallback(async () => {
    await fetch(KHALA_SYNC_SESSION_PATH, { method: 'DELETE' })
    setStatus('signed_out')
    setOwnerUserId(null)
  }, [])

  return { status, ownerUserId, signIn, signOut }
}
