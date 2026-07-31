// Per-identity daily served-token ceiling for SELF-PROVISIONED Omega installs,
// applied to the hosted Omega lanes on `POST /api/v1/chat/completions`.
//
// WHY
// ---
// `chat-completions-routes.ts` grants PLATFORM capacity to any `openauth:`
// session that requests one of the three hosted Omega lane ids
// (`gpt-5.6-luna`, `gemini-3.6-flash`, `kimi-k3`): the metering hook on that
// branch is a hardcoded `metered: false, paymentMode: 'no-spend'`, the funding
// gate is bypassed, and the production deps supply no fair-share or spend-cap
// gate. For an OpenAuth human that is intended — they are a known account. For
// a SELF-PROVISIONED `nostr:` identity it was an unbounded draw on owner-funded
// inference, including `gpt-5.6-luna`, which is an OpenAI passthrough billed to
// the owner. Nostr keypairs are free to generate, so nothing upstream of this
// bounded it.
//
// An equivalent ceiling already existed for `nostr:` identities on the
// google-gemini provider-accounts proxy (`provider-account-service-routes.ts`),
// reading the same `token_usage_events` ledger. That ceiling covered ONE
// handler and ONE lane. This module is the same rule for the route that
// actually serves Omega turns, so the bound follows the identity rather than
// the handler.
//
// SCOPE: this gate inspects ONLY self-provisioned `nostr:` identities. Every
// GitHub, email, agent, and operator-admitted identity is returned `undefined`
// (proceed) without a ledger read, so no live workflow changes cost or latency.

import { isOmegaNostrSelfProvisionedUserId } from '../auth/omega-nostr-self-provision'

/** `chat-completions` session account refs are `openauth:<userId>`. */
const OPENAUTH_ACCOUNT_PREFIX = 'openauth:'

export const SELF_PROVISIONED_CEILING_ERROR =
  'free_tier_daily_token_ceiling_reached' as const

export type SelfProvisionedCeilingRefusal = Readonly<{
  error: typeof SELF_PROVISIONED_CEILING_ERROR
  retryAfterSeconds: number
  servedToday: number
  tokensPerDay: number
}>

/**
 * The self-provisioned user id behind a chat-completions account ref, or
 * `undefined` when the caller is not a self-provisioned identity.
 *
 * Deliberately strict: only `openauth:nostr:<pubkey>` qualifies. An agent
 * token (`agent:…`) or a GitHub/email OpenAuth session is never ceilinged here.
 */
export const selfProvisionedUserIdFromAccountRef = (
  accountRef: string,
): string | undefined => {
  if (!accountRef.startsWith(OPENAUTH_ACCOUNT_PREFIX)) return undefined
  const userId = accountRef.slice(OPENAUTH_ACCOUNT_PREFIX.length)
  return isOmegaNostrSelfProvisionedUserId(userId) ? userId : undefined
}

/** Seconds remaining until the next UTC day boundary, floored at 1. */
export const secondsUntilNextUtcDay = (nowMs: number): number => {
  const now = new Date(nowMs)
  const nextDay = Date.UTC(
    now.getUTCFullYear(),
    now.getUTCMonth(),
    now.getUTCDate() + 1,
  )
  return Math.max(1, Math.ceil((nextDay - nowMs) / 1_000))
}

/**
 * Decide whether a self-provisioned identity may draw on the hosted lanes.
 *
 * `servedToday` is the exact `token_usage_events` sum for the identity since
 * the UTC day boundary — the SAME ledger and window the google-gemini proxy
 * ceiling reads, so one identity cannot get two independent allowances by
 * alternating routes.
 *
 * The comparison is `>=`, so a ceiling of 0 refuses every request (the useful
 * "provision accounts but serve nobody" state) and a ceiling reached exactly is
 * refused rather than allowed to tip over.
 */
export const decideSelfProvisionedDailyCeiling = (
  input: Readonly<{
    nowMs: number
    servedToday: number
    tokensPerDay: number
  }>,
): SelfProvisionedCeilingRefusal | undefined =>
  input.servedToday >= input.tokensPerDay
    ? {
        error: SELF_PROVISIONED_CEILING_ERROR,
        retryAfterSeconds: secondsUntilNextUtcDay(input.nowMs),
        servedToday: input.servedToday,
        tokensPerDay: input.tokensPerDay,
      }
    : undefined

export type SelfProvisionedDailyCeilingDeps = Readonly<{
  /** Exact served-token total for `userId` since the UTC day boundary. */
  servedTokensToday: (userId: string) => Promise<number>
  tokensPerDay: () => number
  nowMs?: (() => number) | undefined
  /**
   * Is this `nostr:` identity OPERATOR-ADMITTED rather than free tier?
   *
   * The `nostr:` prefix alone cannot answer that: `admit-sarah-voice-npub.ts`
   * gives an admitted alpha member the same id form a self-provisioned install
   * gets. Without this, an admitted identity is metered as a stranger — which
   * on 2026-07-31 locked the owner out of every turn in his own app. See
   * `./admitted-identity`. Absent => every `nostr:` identity is treated as free
   * tier, which is the pre-existing (conflating) behavior.
   */
  isAdmittedIdentity?: ((userId: string) => Promise<boolean>) | undefined
}>

/**
 * Build the route gate. Non-self-provisioned callers short-circuit WITHOUT a
 * ledger read.
 *
 * FAIL-CLOSED: if the ledger read throws, a self-provisioned identity is
 * REFUSED rather than served. This matches the google-gemini proxy ceiling —
 * a spend path must not fall open during a database outage — and it only ever
 * affects self-provisioned identities, never a paying or owner-admitted one.
 */
export const makeSelfProvisionedDailyCeilingGate = (
  deps: SelfProvisionedDailyCeilingDeps,
) => {
  const nowMs = deps.nowMs ?? (() => Date.now())
  return async (
    accountRef: string,
  ): Promise<SelfProvisionedCeilingRefusal | undefined> => {
    const userId = selfProvisionedUserIdFromAccountRef(accountRef)
    if (userId === undefined) return undefined
    // An operator-admitted identity is not free tier and must not be metered as
    // free tier. Checked BEFORE the ledger read, so an admitted member costs
    // one indexed membership lookup instead of a ledger scan. The lookup
    // resolves `false` on failure, so a database problem falls THROUGH to the
    // ceiling rather than past it.
    if (
      deps.isAdmittedIdentity !== undefined &&
      (await deps.isAdmittedIdentity(userId))
    ) {
      return undefined
    }
    const tokensPerDay = deps.tokensPerDay()
    let servedToday: number
    try {
      servedToday = await deps.servedTokensToday(userId)
    } catch {
      return {
        error: SELF_PROVISIONED_CEILING_ERROR,
        retryAfterSeconds: secondsUntilNextUtcDay(nowMs()),
        servedToday: tokensPerDay,
        tokensPerDay,
      }
    }
    return decideSelfProvisionedDailyCeiling({
      nowMs: nowMs(),
      servedToday,
      tokensPerDay,
    })
  }
}
