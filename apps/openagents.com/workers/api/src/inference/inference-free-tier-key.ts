// Khala FREE API MODE (issue #6228, EPIC #5474). The self-serve FREE tier: a
// FREE API key (`POST /api/keys/free`) plus a FREE inference lane that calls the
// single public model `openagents/khala` (own-infra GPT-OSS / Gemini Flash) with
// NO funded balance, within a per-key daily quota. Beyond the quota — or for
// premium lanes — credits/budget are still required (the existing balance + 402
// path), so paid Khala behavior for funded keys is unchanged.
//
// WHY THIS IS A KEY-TIER, NOT A NEW MODEL. There is exactly one public model
// `openagents/khala`; `khala-mini`/`khala-code` are not public products. "Free"
// is a property of the KEY/account + the lane, mirroring the two existing
// balance-gate bypasses (`inference-free-allowance.ts` — owner-keyed Sybil pool
// on Gemini; `inference-operator-exemption.ts` — owner-keyed operator credit on
// own-infra). This module adds the THIRD bypass: a per-KEY free tier on the
// public Khala lane, the one anyone can self-serve. It reuses the SAME agent
// bearer-token auth (a free key is a normal `oa_agent_` credential), the SAME
// balance-gate seam shape (`checkFreeTier`), and the SAME metering-hook decorator
// shape (`withFreeTierKhala` records a zero-debit free receipt + accrues quota).
//
// HARD GUARDRAILS:
//   - Own-infra / non-premium ONLY. The free lane is `openagents/khala` (which
//     classifies `open`, routed to own GPT-OSS/Gemini). A PREMIUM model
//     (`claude` / `unknown`) is NEVER free here — it still hits the normal
//     balance + premium-grant gates, so a free key can never rack up real
//     third-party cost. We reuse `isPremiumModel` (model-router classes) so the
//     premium set stays single-source.
//   - Quota-bounded. A free key has a per-UTC-day request count + served-token
//     ceiling. Over-quota requests fall through to the normal balance gate (402),
//     so free is genuinely free WITHIN the quota and paid beyond it.
//   - Mint is abuse-bounded. The self-serve mint endpoint is per-IP-hash,
//     per-day rate-limited (no unbounded key minting) and never echoes a raw IP.
//
// PUBLIC-SAFE. Every table this touches carries account refs, IP-hash refs,
// model ids, bounded integer counters, and timestamps only — never prompts,
// completions, wallet/payment material, raw tokens, raw IPs, or secrets. The
// decorator never fails the customer's inference call: an accrual error logs a
// public-safe diagnostic and falls through to normal metering rather than
// breaking.
//
// INERT until INFERENCE_FREE_TIER_ENABLED is on AND the gateway is enabled.

import { Effect } from 'effect'

import type {
  InferenceEntitlementsGateReads,
  InferenceEntitlementsMirror,
} from '../inference-entitlements-store'
import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import { isPremiumModel } from './inference-premium-allowlist'
import { isKhalaModel } from './pricing'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'

class FreeTierPersistenceError extends Error {
  readonly _tag = 'FreeTierPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'FreeTierPersistenceError'
  }
}

const freeTierPersistenceError = (error: unknown) =>
  new FreeTierPersistenceError(error)

// ----------------------------------------------------------------------------
// Env flag (fail-closed, default OFF)
// ----------------------------------------------------------------------------

export const INFERENCE_FREE_TIER_ENABLED_ENV_KEY =
  'INFERENCE_FREE_TIER_ENABLED' as const

const ON_TOKENS = new Set(['1', 'on', 'true', 'yes'])

// Fail-closed flag read. Absent / non-string / any non-on value => disabled.
export const isFreeTierEnabled = (value: unknown): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())

// ----------------------------------------------------------------------------
// Tunable quota constants (all free-tier thresholds in ONE place)
// ----------------------------------------------------------------------------

// Per-key, per-UTC-day FREE request ceiling. The last allowed request lands the
// counter AT the ceiling; the next one falls through to the normal balance gate.
// !! TUNABLE: issue #6228, raised on the cost model (issue #6232,
// docs/inference/2026-06-25-khala-cost-model-and-analytics.md). Env-overridable
// via FREE_TIER_MAX_REQUESTS_PER_DAY without a deploy.
export const FREE_TIER_MAX_REQUESTS_PER_DAY = 2_000 as const

// Per-key, per-UTC-day FREE served-token ceiling (prompt + completion). Bounds a
// single free key's draw on the shared own-infra pool regardless of request
// count. Raised from 200k -> 2.5M on the cost model: the real Khala lane
// (Fireworks DeepSeek V4 Flash, $0.14 in / $0.28 out per Mtok) costs ~$0.60 to
// serve a fully-maxed 2.5M-token day, ~$0.12 at realistic utilization — a
// generous "try it" session bounded by cost (issue #6232).
// !! TUNABLE: env-overridable via FREE_TIER_MAX_TOKENS_PER_DAY without a deploy.
export const FREE_TIER_MAX_TOKENS_PER_DAY = 2_500_000 as const

// Env keys the owner can set to tune the free-tier quota WITHOUT a code deploy.
// A missing / non-positive / non-numeric value falls back to the constant above.
export const FREE_TIER_MAX_REQUESTS_PER_DAY_ENV_KEY =
  'FREE_TIER_MAX_REQUESTS_PER_DAY' as const
export const FREE_TIER_MAX_TOKENS_PER_DAY_ENV_KEY =
  'FREE_TIER_MAX_TOKENS_PER_DAY' as const

// Parse a positive-integer env override; any absent / non-numeric / <= 0 value
// returns the fallback. Bounded numeric parse, never an intent parser.
export const parsePositiveIntEnv = (
  value: unknown,
  fallback: number,
): number => {
  if (typeof value !== 'string') {
    return fallback
  }
  const parsed = Number.parseInt(value.trim(), 10)
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback
}

// Resolve the effective free-tier quota from the Worker env, falling back to the
// compiled defaults. This is the ONE place env overrides are read so the gate,
// the metering wrapper, the mint response, and the public catalog all agree.
export const resolveFreeTierQuota = (
  env: Readonly<{
    FREE_TIER_MAX_REQUESTS_PER_DAY?: unknown
    FREE_TIER_MAX_TOKENS_PER_DAY?: unknown
  }>,
): FreeTierQuota => ({
  maxRequestsPerDay: parsePositiveIntEnv(
    env.FREE_TIER_MAX_REQUESTS_PER_DAY,
    FREE_TIER_MAX_REQUESTS_PER_DAY,
  ),
  maxTokensPerDay: parsePositiveIntEnv(
    env.FREE_TIER_MAX_TOKENS_PER_DAY,
    FREE_TIER_MAX_TOKENS_PER_DAY,
  ),
})

// Per-IP-hash, per-UTC-day self-serve MINT ceiling so anonymous minting is
// bounded (no unbounded key minting). !! TUNABLE: issue #6228, raised 25 -> 200
// after the 2026-06-25 outage AAR: the responder could not mint a fresh key to
// test the recovering gateway because the cap was hit and was NOT env-overridable
// (docs/incidents/2026-06-25-khala-500-completions-outage-aar.md). 200/IP/day is
// still a hard abuse bound (the per-key daily token + request quota is the real
// spend bound), but it leaves ample headroom for ops/canary key provisioning.
// Env-overridable via FREE_KEY_MAX_MINTS_PER_IP_PER_DAY without a deploy.
export const FREE_KEY_MAX_MINTS_PER_IP_PER_DAY = 200 as const

// Env key the owner can set to tune the per-IP daily mint ceiling WITHOUT a code
// deploy. A missing / non-positive / non-numeric value falls back to the constant
// above. Read through the same bounded `parsePositiveIntEnv` numeric parse as the
// free-tier quota overrides — never an intent parser.
export const FREE_KEY_MAX_MINTS_PER_IP_PER_DAY_ENV_KEY =
  'FREE_KEY_MAX_MINTS_PER_IP_PER_DAY' as const

// Resolve the effective per-IP daily mint ceiling from the Worker env, falling
// back to the compiled default. The ONE place this override is read so the mint
// route gate and any catalog/diagnostic surface agree.
export const resolveFreeKeyMintCap = (
  env: Readonly<{ FREE_KEY_MAX_MINTS_PER_IP_PER_DAY?: unknown }>,
): number =>
  parsePositiveIntEnv(
    env.FREE_KEY_MAX_MINTS_PER_IP_PER_DAY,
    FREE_KEY_MAX_MINTS_PER_IP_PER_DAY,
  )

export type FreeTierQuota = Readonly<{
  maxRequestsPerDay: number
  maxTokensPerDay: number
}>

export const DEFAULT_FREE_TIER_QUOTA: FreeTierQuota = {
  maxRequestsPerDay: FREE_TIER_MAX_REQUESTS_PER_DAY,
  maxTokensPerDay: FREE_TIER_MAX_TOKENS_PER_DAY,
}

// ----------------------------------------------------------------------------
// UTC day bucket
// ----------------------------------------------------------------------------

// The UTC day bucket (YYYY-MM-DD) the free quota resets on. Derived from an ISO
// timestamp so tests can inject a fixed clock.
export const freeTierUsageDay = (nowIso: string): string =>
  nowIso.slice(0, 10)

// ----------------------------------------------------------------------------
// Free-lane eligibility (own-infra / non-premium public Khala ONLY)
// ----------------------------------------------------------------------------

export const FREE_TIER_REASON_ELIGIBLE =
  'reason.inference_free_tier.eligible' as const
export const FREE_TIER_REASON_NOT_FREE_LANE =
  'reason.inference_free_tier.not_free_lane' as const
export const FREE_TIER_REASON_PREMIUM_DENIED =
  'reason.inference_free_tier.premium_never_free' as const

// Whether a requested model is on the FREE lane: it must be the single public
// Khala model AND not a premium class (defense in depth — Khala classifies as
// `open`, never premium, so the premium check can only ever protect against a
// future reclassification). Bounded id/class check, never an intent parser.
export const isFreeTierLaneModel = (model: string): boolean =>
  isKhalaModel(model) && !isPremiumModel(model)

export type FreeTierLaneDecision = Readonly<{
  freeLane: boolean
  premium: boolean
  reasonRef: string
}>

export const decideFreeTierLane = (model: string): FreeTierLaneDecision => {
  // GUARDRAIL: a premium model is never free, even if it were ever aliased to
  // Khala. Premium is deny-by-default here.
  if (isPremiumModel(model)) {
    return {
      freeLane: false,
      premium: true,
      reasonRef: FREE_TIER_REASON_PREMIUM_DENIED,
    }
  }
  if (isKhalaModel(model)) {
    return {
      freeLane: true,
      premium: false,
      reasonRef: FREE_TIER_REASON_ELIGIBLE,
    }
  }
  return {
    freeLane: false,
    premium: false,
    reasonRef: FREE_TIER_REASON_NOT_FREE_LANE,
  }
}

// ----------------------------------------------------------------------------
// Pure quota decision
// ----------------------------------------------------------------------------

export type FreeTierUsage = Readonly<{
  requestsToday: number
  tokensToday: number
}>

export type FreeTierQuotaDecision = Readonly<{
  // True when THIS request fits under BOTH the daily request and token
  // ceilings. False => fall through to the normal balance gate (402).
  withinQuota: boolean
  maxRequestsPerDay: number
  maxTokensPerDay: number
  remainingRequests: number
  remainingTokens: number
  reasonRef: string
}>

export const FREE_TIER_QUOTA_REASON_WITHIN =
  'reason.inference_free_tier.within_quota' as const
export const FREE_TIER_QUOTA_REASON_REQUESTS_EXCEEDED =
  'reason.inference_free_tier.daily_requests_exceeded' as const
export const FREE_TIER_QUOTA_REASON_TOKENS_EXCEEDED =
  'reason.inference_free_tier.daily_tokens_exceeded' as const

// Decide whether a free key may take ANOTHER request today. Pure. A request is
// admitted only when the already-used request count is UNDER the request ceiling
// AND the already-used token count is UNDER the token ceiling. A key that has
// already issued >= the request ceiling, or already drawn >= the token ceiling,
// is over quota (falls through to the balance gate). Pre-flight has no per-
// request token estimate, so the token check is against the already-used total.
export const decideFreeTierQuota = (
  input: Readonly<{
    usage: FreeTierUsage
    quota?: FreeTierQuota | undefined
  }>,
): FreeTierQuotaDecision => {
  const quota = input.quota ?? DEFAULT_FREE_TIER_QUOTA
  const requests = Math.max(0, Math.trunc(input.usage.requestsToday))
  const tokens = Math.max(0, Math.trunc(input.usage.tokensToday))
  const remainingRequests = Math.max(0, quota.maxRequestsPerDay - requests)
  const remainingTokens = Math.max(0, quota.maxTokensPerDay - tokens)

  if (requests >= quota.maxRequestsPerDay) {
    return {
      maxRequestsPerDay: quota.maxRequestsPerDay,
      maxTokensPerDay: quota.maxTokensPerDay,
      reasonRef: FREE_TIER_QUOTA_REASON_REQUESTS_EXCEEDED,
      remainingRequests: 0,
      remainingTokens,
      withinQuota: false,
    }
  }
  if (tokens >= quota.maxTokensPerDay) {
    return {
      maxRequestsPerDay: quota.maxRequestsPerDay,
      maxTokensPerDay: quota.maxTokensPerDay,
      reasonRef: FREE_TIER_QUOTA_REASON_TOKENS_EXCEEDED,
      remainingRequests,
      remainingTokens: 0,
      withinQuota: false,
    }
  }
  return {
    maxRequestsPerDay: quota.maxRequestsPerDay,
    maxTokensPerDay: quota.maxTokensPerDay,
    reasonRef: FREE_TIER_QUOTA_REASON_WITHIN,
    remainingRequests,
    remainingTokens,
    withinQuota: true,
  }
}

// ----------------------------------------------------------------------------
// Free-tier key store (mark + read)
// ----------------------------------------------------------------------------

export const DEFAULT_FREE_TIER_SCOPE = 'free_khala_daily' as const

// Plain-async read of whether an account is on the free tier. The route gate (a
// Promise-returning seam) uses this directly. Returns false on any read error
// (fail-closed).
export const readAccountFreeTier = async (
  db: D1Database,
  accountRef: string,
): Promise<boolean> => {
  try {
    const row = await db
      .prepare(
        `SELECT account_ref FROM inference_free_tier_keys WHERE account_ref = ? LIMIT 1`,
      )
      .bind(accountRef)
      .first<{ account_ref: string }>()
    return row !== null
  } catch {
    return false
  }
}

// Plain-async mark of an account as a free-tier key (idempotent upsert), for
// callers already in a Promise context (e.g. the mint route handler) so no
// Effect->Promise bridge runs there. Returns whether the write succeeded.
export const markAccountFreeTierAsync = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    mintSource?: string | undefined
    note?: string | null | undefined
    scope?: string | undefined
    nowIso?: string | undefined
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror (optional; absent
  // => byte-identical D1-only behavior).
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<boolean> => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  try {
    await db
      .prepare(
        `INSERT INTO inference_free_tier_keys
           (account_ref, scope, mint_source, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(account_ref) DO UPDATE SET
           scope = excluded.scope,
           mint_source = excluded.mint_source,
           note = excluded.note,
           updated_at = excluded.updated_at`,
      )
      .bind(
        input.accountRef,
        input.scope ?? DEFAULT_FREE_TIER_SCOPE,
        input.mintSource ?? 'self_serve_anonymous',
        input.note ?? null,
        nowIso,
        nowIso,
      )
      .run()
    mirror?.([
      {
        kind: 'write',
        row: {
          account_ref: input.accountRef,
          created_at: nowIso,
          mint_source: input.mintSource ?? 'self_serve_anonymous',
          note: input.note ?? null,
          scope: input.scope ?? DEFAULT_FREE_TIER_SCOPE,
          updated_at: nowIso,
        },
        table: 'inference_free_tier_keys',
      },
    ])
    return true
  } catch {
    return false
  }
}

// Plain-async record of one mint against an IP-hash for the day (idempotent
// increment), for callers already in a Promise context. Returns whether the
// write succeeded.
export const recordFreeKeyMintAsync = async (
  db: D1Database,
  input: Readonly<{
    ipHash: string
    mintDay: string
    nowIso?: string | undefined
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<boolean> => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  try {
    await db
      .prepare(
        `INSERT INTO inference_free_key_mints
           (ip_hash, mint_day, mint_count, created_at, updated_at)
         VALUES (?, ?, 1, ?, ?)
         ON CONFLICT(ip_hash, mint_day) DO UPDATE SET
           mint_count = mint_count + 1,
           updated_at = excluded.updated_at`,
      )
      .bind(input.ipHash, input.mintDay, nowIso, nowIso)
      .run()
    mirror?.([
      {
        ipHash: input.ipHash,
        kind: 'increment_free_key_mint',
        mintDay: input.mintDay,
        nowIso,
      },
    ])
    return true
  } catch {
    return false
  }
}

// Mark an account as a free-tier key (idempotent upsert). Called by the
// self-serve mint endpoint after registering the underlying agent credential.
export const markAccountFreeTier = (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    mintSource?: string | undefined
    note?: string | null | undefined
    scope?: string | undefined
    nowIso?: (() => string) | undefined
  }>,
): Effect.Effect<boolean> =>
  Effect.gen(function* () {
    const nowIso = (input.nowIso ?? currentIsoTimestamp)()
    return yield* Effect.tryPromise({
      catch: freeTierPersistenceError,
      try: async () => {
        await db
          .prepare(
            `INSERT INTO inference_free_tier_keys
               (account_ref, scope, mint_source, note, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(account_ref) DO UPDATE SET
               scope = excluded.scope,
               mint_source = excluded.mint_source,
               note = excluded.note,
               updated_at = excluded.updated_at`,
          )
          .bind(
            input.accountRef,
            input.scope ?? DEFAULT_FREE_TIER_SCOPE,
            input.mintSource ?? 'self_serve_anonymous',
            input.note ?? null,
            nowIso,
            nowIso,
          )
          .run()
        return true
      },
    }).pipe(Effect.catch(() => Effect.succeed(false)))
  })

// ----------------------------------------------------------------------------
// Free-tier daily usage read + idempotent accrual
// ----------------------------------------------------------------------------

type FreeTierUsageRow = Readonly<{
  free_request_count: number
  free_total_tokens: number
}>

// Read the account's current free usage for the given UTC day. A missing row
// means $0/0 used today. Read-only and bounded.
export const readFreeTierUsage = async (
  db: D1Database,
  accountRef: string,
  usageDay: string,
): Promise<FreeTierUsage> => {
  const row = await db
    .prepare(
      `SELECT free_request_count, free_total_tokens
         FROM inference_free_tier_usage
        WHERE account_ref = ? AND usage_day = ?
        LIMIT 1`,
    )
    .bind(accountRef, usageDay)
    .first<FreeTierUsageRow>()
  return {
    requestsToday:
      typeof row?.free_request_count === 'number' ? row.free_request_count : 0,
    tokensToday:
      typeof row?.free_total_tokens === 'number' ? row.free_total_tokens : 0,
  }
}

// Accrue one free request (+ its served tokens) against the account's daily
// tally, idempotently. Writes the per-request event row (UNIQUE request_id) and
// increments the daily tally in ONE D1 batch. On a duplicate request id the
// UNIQUE constraint aborts the batch and we treat it as already-accrued (no
// double-count). Returns whether a NEW accrual was recorded.
const accrueFreeTierUsage = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    usageDay: string
    requestId: string
    servedModel: string
    totalTokens: number
    nowIso: string
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror. The mirror op is
  // EVENT-KEYED (request_id) so the Postgres tally can never double-count.
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<boolean> => {
  try {
    await db.batch([
      db
        .prepare(
          `INSERT INTO inference_free_tier_usage_events
             (request_id, account_ref, usage_day, served_model, total_tokens, created_at)
           VALUES (?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.requestId,
          input.accountRef,
          input.usageDay,
          input.servedModel,
          input.totalTokens,
          input.nowIso,
        ),
      db
        .prepare(
          `INSERT INTO inference_free_tier_usage
             (account_ref, usage_day, free_request_count, free_total_tokens, created_at, updated_at)
           VALUES (?, ?, 1, ?, ?, ?)
           ON CONFLICT(account_ref, usage_day) DO UPDATE SET
             free_request_count = free_request_count + 1,
             free_total_tokens = free_total_tokens + excluded.free_total_tokens,
             updated_at = excluded.updated_at`,
        )
        .bind(
          input.accountRef,
          input.usageDay,
          input.totalTokens,
          input.nowIso,
          input.nowIso,
        ),
    ])
    mirror?.([
      {
        event: {
          accountRef: input.accountRef,
          createdAt: input.nowIso,
          requestId: input.requestId,
          servedModel: input.servedModel,
          totalTokens: input.totalTokens,
          usageDay: input.usageDay,
        },
        kind: 'accrue_free_tier_usage',
      },
    ])
    return true
  } catch {
    // Duplicate request id => already accrued. Idempotent no-op.
    return false
  }
}

// ----------------------------------------------------------------------------
// Mint abuse guard (per-IP-hash, per-day)
// ----------------------------------------------------------------------------

export type FreeKeyMintGateDecision = Readonly<{
  allowed: boolean
  mintsToday: number
  maxMintsPerDay: number
  reasonRef: string
}>

export const FREE_KEY_MINT_REASON_ALLOWED =
  'reason.inference_free_key_mint.allowed' as const
export const FREE_KEY_MINT_REASON_RATE_LIMITED =
  'reason.inference_free_key_mint.daily_ip_limit_exceeded' as const

// Pure mint-rate decision: a new mint is allowed only when the IP's already-used
// mint count today is UNDER the daily ceiling.
export const decideFreeKeyMint = (
  input: Readonly<{
    mintsToday: number
    maxMintsPerDay?: number
  }>,
): FreeKeyMintGateDecision => {
  const max = Math.max(
    1,
    Math.trunc(input.maxMintsPerDay ?? FREE_KEY_MAX_MINTS_PER_IP_PER_DAY),
  )
  const mints = Math.max(0, Math.trunc(input.mintsToday))
  if (mints >= max) {
    return {
      allowed: false,
      maxMintsPerDay: max,
      mintsToday: mints,
      reasonRef: FREE_KEY_MINT_REASON_RATE_LIMITED,
    }
  }
  return {
    allowed: true,
    maxMintsPerDay: max,
    mintsToday: mints,
    reasonRef: FREE_KEY_MINT_REASON_ALLOWED,
  }
}

// Read how many free keys an IP-hash has minted today. Returns 0 on any read
// error so a transient DB blip cannot block minting forever (the per-mint write
// below is still bounded; this read is the pre-flight signal).
export const readFreeKeyMintsToday = async (
  db: D1Database,
  ipHash: string,
  mintDay: string,
): Promise<number> => {
  try {
    const row = await db
      .prepare(
        `SELECT mint_count FROM inference_free_key_mints
          WHERE ip_hash = ? AND mint_day = ? LIMIT 1`,
      )
      .bind(ipHash, mintDay)
      .first<{ mint_count: number }>()
    return typeof row?.mint_count === 'number' ? row.mint_count : 0
  } catch {
    return 0
  }
}

// Record one mint against an IP-hash for the day (idempotent increment). Returns
// whether the write succeeded; a failure is logged by the caller (the mint still
// proceeds — the pre-flight read is the primary bound).
export const recordFreeKeyMint = (
  db: D1Database,
  input: Readonly<{
    ipHash: string
    mintDay: string
    nowIso?: (() => string) | undefined
  }>,
): Effect.Effect<boolean> =>
  Effect.gen(function* () {
    const nowIso = (input.nowIso ?? currentIsoTimestamp)()
    return yield* Effect.tryPromise({
      catch: freeTierPersistenceError,
      try: async () => {
        await db
          .prepare(
            `INSERT INTO inference_free_key_mints
               (ip_hash, mint_day, mint_count, created_at, updated_at)
             VALUES (?, ?, 1, ?, ?)
             ON CONFLICT(ip_hash, mint_day) DO UPDATE SET
               mint_count = mint_count + 1,
               updated_at = excluded.updated_at`,
          )
          .bind(input.ipHash, input.mintDay, nowIso, nowIso)
          .run()
        return true
      },
    }).pipe(Effect.catch(() => Effect.succeed(false)))
  })

// ----------------------------------------------------------------------------
// Balance-gate seam (the free-tier bypass)
// ----------------------------------------------------------------------------

export type FreeTierGateDecision = Readonly<{
  // True when the request may BYPASS the balance gate: a free-tier key, on the
  // free Khala lane, within the daily quota. False => the gate is unchanged (the
  // request must clear the normal balance gate; paid-Khala behavior intact).
  free: boolean
  reasonRef: string
}>

export type FreeTierGateDeps = Readonly<{
  db: D1Database
  quota?: FreeTierQuota
  nowIso?: (() => string) | undefined
  // INTERNAL-ACCOUNT QUOTA EXEMPTION (#6232 / #6298). The env-configured
  // internal/ops account allowlist (`INFERENCE_INTERNAL_ACCOUNT_REFS`, the SAME
  // set the demand-attribution rule uses — see `inference-internal-account.ts`).
  // A free-tier key whose account is on this allowlist is treated as
  // quota-EXEMPT: it bypasses the balance gate as a free request on the free
  // Khala lane WITHOUT a per-UTC-day request/token quota check, so our own
  // sustained internal testing never hits the free-tier daily quota and never
  // falls through to the 402 on quota grounds. This is a QUOTA exemption ONLY:
  // a premium / non-Khala model still short-circuits to NOT-free, and an
  // EXTERNAL (non-allowlist) free key keeps the unchanged 2.5M-token /
  // 2,000-request daily limit. Default empty/undefined => pure no-op (external
  // behavior is byte-for-byte unchanged).
  internalAccountRefs?: ReadonlySet<string> | undefined
  // KS-8.9 (#8320): routed enforcement reads (compare/postgres modes).
  // Absent => the untouched inline D1 reads (zero added hot-path latency).
  gateReads?:
    | Pick<
        InferenceEntitlementsGateReads,
        'freeTierKeyExists' | 'freeTierUsage'
      >
    | undefined
}>

export const FREE_TIER_QUOTA_REASON_INTERNAL_EXEMPT =
  'reason.inference_free_tier.internal_account_quota_exempt' as const

// A route-level balance-gate bypass: given an account ref + requested model,
// returns whether the request may bypass the balance gate as a free-tier call.
// PREMIUM / non-Khala models short-circuit to NOT-free WITHOUT a quota read.
// Wired into the chat-completions `checkFreeTier` seam (open/no-op when unwired
// or flag-off). Fail-closed: any read error => not free (the 402 stands).
export type FreeTierGate = (
  accountRef: string,
  model: string,
) => Promise<FreeTierGateDecision>

export const makeFreeTierGate = (deps: FreeTierGateDeps): FreeTierGate => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const internalAccountRefs =
    deps.internalAccountRefs ?? new Set<string>()
  // KS-8.9 (#8320): route through the migration seam when wired; the
  // default stays the inline D1 reads.
  const readKeyMembership =
    deps.gateReads?.freeTierKeyExists ??
    ((ref: string) => readAccountFreeTier(deps.db, ref))
  const readUsage =
    deps.gateReads?.freeTierUsage ??
    ((ref: string, day: string) => readFreeTierUsage(deps.db, ref, day))
  return async (accountRef: string, model: string) => {
    const lane = decideFreeTierLane(model)
    if (!lane.freeLane) {
      return { free: false, reasonRef: lane.reasonRef }
    }
    try {
      const isFree = await readKeyMembership(accountRef)
      if (!isFree) {
        return {
          free: false,
          reasonRef: 'reason.inference_free_tier.account_not_free_tier',
        }
      }
      // INTERNAL-ACCOUNT QUOTA EXEMPTION. A free-tier key on the internal/ops
      // allowlist is free on the free Khala lane WITHOUT a daily-quota check, so
      // sustained internal testing never exhausts the per-key quota and never
      // falls through to the 402. Scoped to the explicit allowlist only; external
      // free keys still take the quota path below unchanged.
      if (internalAccountRefs.has(accountRef)) {
        return {
          free: true,
          reasonRef: FREE_TIER_QUOTA_REASON_INTERNAL_EXEMPT,
        }
      }
      const usageDay = freeTierUsageDay(nowIso())
      const usage = await readUsage(accountRef, usageDay)
      const quota = decideFreeTierQuota({ quota: deps.quota, usage })
      return {
        free: quota.withinQuota,
        reasonRef: quota.reasonRef,
      }
    } catch {
      // Read error: do NOT bypass the balance gate.
      return {
        free: false,
        reasonRef: 'reason.inference_free_tier.read_error',
      }
    }
  }
}

// ----------------------------------------------------------------------------
// Free-tier metering wrapper (zero-debit, receipt-first, no referral)
// ----------------------------------------------------------------------------

// Public-safe free-tier receipt ref. Resolvable without exposing any account id,
// amount, destination, or payment material. The `free_tier` infix marks the
// honest zero-debit free-mode accounting state.
export const freeTierReceiptRef = (requestId: string): string =>
  `receipt.inference.free_tier.${requestId}`

const freeTierOutcome = (requestId: string): MeteringOutcome => ({
  metered: false,
  receiptRef: freeTierReceiptRef(requestId),
})

export type FreeTierMeteringDeps = Readonly<{
  db: D1Database
  quota?: FreeTierQuota
  nowIso?: (() => string) | undefined
  // INTERNAL-ACCOUNT QUOTA EXEMPTION (#6232 / #6298). MUST match the gate's
  // `internalAccountRefs` so the bypass and the zero-debit accrual agree: an
  // internal/ops account on the allowlist is recorded as a zero-debit free
  // receipt on the free Khala lane regardless of the per-UTC-day quota (it still
  // accrues usage for visibility, it just never goes over-quota -> charge). An
  // external (non-allowlist) free key is unaffected. Default empty => no-op.
  internalAccountRefs?: ReadonlySet<string> | undefined
  // KS-8.9 (#8320): routed enforcement reads + fire-safe dual-write
  // mirror. Absent => untouched D1-only behavior.
  gateReads?:
    | Pick<
        InferenceEntitlementsGateReads,
        'freeTierKeyExists' | 'freeTierUsage'
      >
    | undefined
  mirror?: InferenceEntitlementsMirror | undefined
}>

/**
 * Wrap a metering hook so a FREE-TIER key's request for the free Khala lane,
 * WITHIN the daily quota, is recorded as a zero-debit free receipt (receipt-
 * first, NO credit decrement, NO referral) AND its usage is accrued against the
 * daily quota (idempotent per request). Otherwise it falls through to the inner
 * hook unchanged.
 *
 * GUARDRAILS:
 *   - A PREMIUM served model ALWAYS falls through to the inner hook (the normal
 *     ledger debit), even for a free-tier key — a free key never gets a premium
 *     model free.
 *   - A non-free-tier account, a non-Khala model, an OVER-quota request, or any
 *     DB error falls through to the inner hook (charge), so free is genuinely
 *     free only within the quota and never grants usage we could not account
 *     for. The customer's inference call is never failed by the bookkeeping.
 *
 * Wired alongside `withFreeAllowance` / `withOperatorCredit`. It runs AFTER
 * dispatch; the route's `checkFreeTier` balance-gate seam is what lets the
 * zero-balance free-tier request reach dispatch in the first place. The two
 * agree because they share the SAME store + quota + UTC-day bucket.
 */
export const withFreeTierKhala = (
  inner: MeteringHook,
  deps: FreeTierMeteringDeps,
): MeteringHook => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const internalAccountRefs = deps.internalAccountRefs ?? new Set<string>()
  return (context: MeteringContext) =>
    Effect.gen(function* () {
      // Only the free Khala lane is ever free here; premium / non-Khala meter.
      // Key on the SERVED model (the lane that actually incurred cost), matching
      // the ledger hook + the other decorators.
      if (!isFreeTierLaneModel(context.servedModel)) {
        return yield* inner(context)
      }

      const gated = yield* Effect.tryPromise({
        catch: freeTierPersistenceError,
        try: async () => {
          const isFree = await (deps.gateReads?.freeTierKeyExists ??
            ((ref: string) => readAccountFreeTier(deps.db, ref)))(
            context.accountRef,
          )
          if (!isFree) {
            return { accrued: false, free: false as const }
          }
          const now = nowIso()
          const usageDay = freeTierUsageDay(now)
          const usage = await (deps.gateReads?.freeTierUsage ??
            ((ref: string, day: string) =>
              readFreeTierUsage(deps.db, ref, day)))(
            context.accountRef,
            usageDay,
          )
          const quota = decideFreeTierQuota({ quota: deps.quota, usage })
          // INTERNAL-ACCOUNT QUOTA EXEMPTION. An internal/ops free-tier key is
          // free regardless of the daily quota (mirrors the gate); it still
          // accrues usage for visibility but never goes over-quota -> charge.
          const internalExempt = internalAccountRefs.has(context.accountRef)
          if (!internalExempt && !quota.withinQuota) {
            return { accrued: false, free: false as const }
          }
          const accrued = await accrueFreeTierUsage(
            deps.db,
            {
              accountRef: context.accountRef,
              nowIso: now,
              requestId: context.requestId,
              servedModel: context.servedModel,
              totalTokens: Math.max(0, Math.trunc(context.usage.totalTokens)),
              usageDay,
            },
            deps.mirror,
          )
          return { accrued, free: true as const }
        },
      }).pipe(
        Effect.catch(error =>
          Effect.gen(function* () {
            // Public-safe diagnostic only; never break the inference response.
            // On error meter normally (charge) rather than grant a free request
            // we could not account for.
            yield* Effect.logInfo(
              workerLogEntry('inference.free_tier.error', {
                accountRef: context.accountRef,
                adapterId: context.adapterId,
                reason: error.message,
                requestId: context.requestId,
                servedModel: context.servedModel,
              }),
            )
            return { accrued: false, free: false as const }
          }),
        ),
      )

      if (!gated.free) {
        // Not free-tier, over quota, or error: meter normally (decrement).
        return yield* inner(context)
      }

      // Free-tier within quota: record the zero-debit free receipt + accrual and
      // return WITHOUT calling the inner hook (no decrement, no referral). Log a
      // public-safe diagnostic (refs + token counts only).
      yield* Effect.logInfo(
        workerLogEntry('inference.free_tier.granted', {
          accountRef: context.accountRef,
          accrued: gated.accrued,
          adapterId: context.adapterId,
          requestId: context.requestId,
          servedModel: context.servedModel,
          totalTokens: context.usage.totalTokens,
        }),
      )
      return freeTierOutcome(context.requestId) satisfies MeteringOutcome
    })
}

// ----------------------------------------------------------------------------
// Self-serve mint (pure helpers; the Worker route wires registration + IP hash)
// ----------------------------------------------------------------------------

// The public-safe request body for `POST /api/keys/free`. Anonymous by default;
// an optional public-safe label (e.g. an app name) helps the owner attribute
// keys. NO email is required (the simplest safe option — abuse is bounded by the
// per-IP mint rate limit + the per-key quota), and an optional email, if sent,
// is only used as the registration display label, never verified or stored as
// PII beyond the agent record.
export type FreeKeyMintRequest = Readonly<{
  label?: string | null | undefined
}>

// Bound the display label so the minted credential name is sane.
export const FREE_KEY_LABEL_MAX_LENGTH = 80 as const

export const sanitizeFreeKeyLabel = (
  label: string | null | undefined,
): string => {
  const trimmed = (label ?? '').trim()
  if (trimmed === '') {
    return 'Free API key'
  }
  return trimmed.slice(0, FREE_KEY_LABEL_MAX_LENGTH)
}
