/**
 * Client-side (browser) API for the Aiur credits console (AIUR-2, #8500).
 * Every call hits Aiur's OWN same-origin proxy paths (`./admin-credits-proxy.ts`
 * on the server), which forwards to the main Worker with the signed-in
 * owner's bearer — this module never sees or needs a raw token.
 */
import {
  AIUR_ADMIN_CREDITS_BALANCE_PATH,
  AIUR_ADMIN_CREDITS_CLAWBACK_PATH,
  AIUR_ADMIN_CREDITS_GRANT_PATH,
  AIUR_ADMIN_CREDITS_HISTORY_PATH,
  AIUR_ADMIN_CREDITS_RECENT_GRANTS_PATH,
  AIUR_ADMIN_CREDITS_USERS_PATH,
} from '../admin-credits-proxy'

export type CreditsUserSummary = Readonly<{
  userId: string
  displayName: string
  primaryEmail: string | null
  githubLogin: string | null
  createdAt: string
  hasSignupCreditGrant: boolean
  hasAdminCreditGrant: boolean
  balanceUsdCents: number
}>

export type CreditsBalance = Readonly<{
  user: Readonly<{ userId: string; displayName: string; githubLogin: string | null }>
  balance: Readonly<{
    availableUsdCents: number
    balanceUsdCents: number
    availableMsat: number
    balanceMsat: number
    usdCreditMsat: number
    bitcoinWithdrawableMsat: number
  }>
}>

export type CreditsHistoryEntry = Readonly<{
  kind: 'admin_grant' | 'signup_grant'
  amountUsdCents: number
  reason: string
  receiptRef: string
  occurredAt: string
}>

export type CreditsHistory = Readonly<{
  user: Readonly<{ userId: string; displayName: string; githubLogin: string | null }>
  history: ReadonlyArray<CreditsHistoryEntry>
}>

export type RecentGrant = Readonly<{
  grantRef: string
  userId: string
  amountUsdCents: number
  reason: string
  grantedByUserId: string
  receiptRef: string
  createdAt: string
}>

export type CreditsApiError = Readonly<{
  ok: false
  status: number
  code: string
  messageSafe: string
}>

export type CreditsApiResult<T> = Readonly<{ ok: true; value: T }> | CreditsApiError

/** Mints a fresh client-side idempotency ref for one grant/clawback
 * ATTEMPT — retries of the SAME attempt (e.g. a network timeout retry) must
 * reuse the same ref; a NEW user action mints a new one. */
export const mintCreditsActionRef = (): string =>
  (globalThis.crypto ?? (globalThis as unknown as { crypto: Crypto }).crypto).randomUUID()

const parseJsonSafe = async (response: Response): Promise<Record<string, unknown>> => {
  try {
    const parsed: unknown = await response.json()
    return typeof parsed === 'object' && parsed !== null
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

const errorResult = (status: number, body: Record<string, unknown>): CreditsApiError => ({
  code: typeof body.code === 'string' ? body.code : typeof body.error === 'string' ? body.error : 'unknown',
  messageSafe:
    typeof body.messageSafe === 'string'
      ? body.messageSafe
      : typeof body.reason === 'string'
        ? body.reason
        : `Request failed (${status}).`,
  ok: false,
  status,
})

async function requestJson<T>(
  path: string,
  init?: RequestInit,
): Promise<CreditsApiResult<T>> {
  const response = await fetch(path, init)
  const body = await parseJsonSafe(response)
  if (!response.ok) return errorResult(response.status, body)
  return { ok: true, value: body as unknown as T }
}

export const fetchCreditsUsers = (
  query?: string,
): Promise<CreditsApiResult<{ ok: true; users: ReadonlyArray<CreditsUserSummary> }>> => {
  const url = new URL(AIUR_ADMIN_CREDITS_USERS_PATH, window.location.origin)
  if (query !== undefined && query.trim() !== '') url.searchParams.set('query', query.trim())
  return requestJson(url.pathname + url.search)
}

export const fetchCreditsBalance = (
  target: Readonly<{ userId?: string; githubLogin?: string }>,
): Promise<CreditsApiResult<CreditsBalance>> => {
  const url = new URL(AIUR_ADMIN_CREDITS_BALANCE_PATH, window.location.origin)
  if (target.userId !== undefined) url.searchParams.set('userId', target.userId)
  if (target.githubLogin !== undefined) url.searchParams.set('githubLogin', target.githubLogin)
  return requestJson(url.pathname + url.search)
}

export const fetchCreditsHistory = (
  target: Readonly<{ userId?: string; githubLogin?: string }>,
): Promise<CreditsApiResult<CreditsHistory>> => {
  const url = new URL(AIUR_ADMIN_CREDITS_HISTORY_PATH, window.location.origin)
  if (target.userId !== undefined) url.searchParams.set('userId', target.userId)
  if (target.githubLogin !== undefined) url.searchParams.set('githubLogin', target.githubLogin)
  return requestJson(url.pathname + url.search)
}

export const fetchRecentGrants = (): Promise<
  CreditsApiResult<{ ok: true; grants: ReadonlyArray<RecentGrant> }>
> => requestJson(AIUR_ADMIN_CREDITS_RECENT_GRANTS_PATH)

export type SubmitGrantOutcome = Readonly<{
  ok: true
  alreadyGranted: boolean
  grantedCents: number
  grantRef: string
  receiptRef: string
}>

export const submitCreditsGrant = (
  input: Readonly<{
    userId?: string
    githubLogin: string | undefined
    grantRef: string
    amountUsdCents: number
    reason: string
  }>,
): Promise<CreditsApiResult<SubmitGrantOutcome>> =>
  requestJson(AIUR_ADMIN_CREDITS_GRANT_PATH, {
    body: JSON.stringify(input),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

export type SubmitClawbackOutcome = Readonly<{
  ok: true
  clawedBack: boolean
  insufficientBalance: boolean
  receiptRef: string
}>

export const submitCreditsClawback = (
  input: Readonly<{
    userId?: string
    githubLogin: string | undefined
    clawbackRef: string
    amountUsdCents: number
    reason: string
  }>,
): Promise<CreditsApiResult<SubmitClawbackOutcome>> =>
  requestJson(AIUR_ADMIN_CREDITS_CLAWBACK_PATH, {
    body: JSON.stringify(input),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })
