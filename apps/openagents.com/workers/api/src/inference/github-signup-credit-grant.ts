// $10 GitHub-account-keyed signup credit grant (MM-D1, #8478, epic #8467).
//
// THE GAP this closes: the mobile-only MVP promises "$10 free credit ($10 per
// GitHub account)" on first sign-in. Neither existing grant matches: Pool A's
// `$10 trial` (billing.ts, INITIAL_TRIAL_CREDIT_CENTS) is a lazily-granted USD
// balance keyed to user_id (Autopilot-oriented, not inference-spendable
// directly); Pool C's free-allowance (inference-free-allowance.ts) is keyed to
// verified X owner-claims and only EATS metered charges rather than crediting
// a spendable balance. This module grants real, spendable msat directly into
// Pool B (`agent_balances.balance_msat` — what the inference gateway actually
// charges via metering-hook.ts), keyed to the GitHub account id, exactly once.
//
// REUSES THE EXISTING BRIDGE PRIMITIVE. Rather than inventing a new ledger
// shape, this calls the SAME low-level `usdCreditGrantStatements` builder the
// card-funded USD->msat bridge uses (usd-credit-bridge.ts): one
// `pay_in_type = 'usd_credit_grant'` row, a guarded `balance_msat` +
// `usd_credit_msat` credit, and an audit leg — all in ONE atomic Postgres
// transaction on the credits ledger (CFG-4, #8519). The
// difference from `fundInferenceFromCredit` is what funds the grant: that
// function DEBITS an existing USD `billing_ledger_entries` balance (a card
// purchase converting into spendable credit); this one MINTS new promotional
// credit with no offsetting USD debit (there is no purchase to convert from
// on a brand-new signup).
//
// RL-3 ASSET BOUNDARY (INVARIANTS "Credit<->Bitcoin Asset Boundary"): the
// granted msat is tagged USD-origin (`agent_balances.usd_credit_msat`, the
// SAME column the card bridge tags), so the Lightning sweep (`tips-sweep.ts`)
// excludes it from the sweepable/withdrawable amount — it can never leak into
// real Bitcoin. Because this credit was never actually purchased, the shared
// `validateAssetBoundary` guard is asserted with `revenueAsset: 'free'`
// (promotional, not fiat-purchased) rather than `'usd'`; both tags forbid a
// Bitcoin-denominated share, so the enforced behavior is identical either way
// — 'free' is simply the more honest label for this specific value.
//
// IDEMPOTENCY / ANTI-ABUSE (the "floor" the issue asks for):
//   1. Exactly once per GitHub account id, forever, even under a race. The
//      grant ref is `signup:github:<githubUserId>`, which becomes BOTH the
//      pay_ins UNIQUE idempotency key (via usdCreditGrantIdempotencyKey) AND
//      the UNIQUE `github_user_id` column on the dedicated metadata table
//      (written right after the ledger transaction — see the non-atomic-seam
//      note in `grantGithubSignupCredit`). A retried/raced call for the same
//      GitHub id can insert at most once on either surface — the guarded
//      balance UPDATE (WHERE NOT EXISTS the audit leg) makes a losing racer's
//      credit a true no-op, so the account is credited exactly once no matter
//      how many concurrent callers ask.
//   2. A GitHub-account-age heuristic gate: an account created within
//      `MIN_GITHUB_ACCOUNT_AGE_SECONDS` of "now" is too new to trust (the
//      cheap-to-script farm-account case) and gets a typed `grant_deferred`
//      outcome instead of a silent denial — a caller (or an operator) can
//      retry once the account ages past the floor. Env-overridable without a
//      deploy (mirrors `resolveFreeKeyMintCap`'s pattern). GitHub's `/user`
//      response not carrying `created_at` (should not happen in practice) is
//      NOT penalized — an unknown age falls through to eligible, since this
//      is a defense-in-depth heuristic, not the sole gate.
//   3. A per-IP-hash, per-UTC-day mint cap on DISTINCT GitHub accounts,
//      mirroring the existing `inference_free_key_mints` shape
//      (inference-free-tier-key.ts) in a dedicated
//      `github_signup_credit_ip_mints` table (migration 0304) rather than
//      reusing that table directly (different semantic entity: this counts
//      signup-credit grants, not free-tier key mints). The raw IP is hashed
//      (SHA-256) by the caller before it ever reaches this module.
//   4. Device attestation (DeviceCheck/Play Integrity) is an explicit
//      fast-follow per the issue, not implemented here.
//
// QUERYABLE PER USER (for #8480's balance/history UI): every grant is a row in
// `github_signup_credit_grants` keyed by `user_id`, independent of parsing
// `pay_ins.context_ref`. `readGithubSignupCreditGrantsForUser` is the reader.
//
// CLAWBACK: `clawbackInferenceCredits` (inference-abuse-controls.ts) already
// debits `agent_balances.balance_msat` for any `accountRef` regardless of how
// it was funded — no special-casing needed for this grant to be clawed back.

import { Effect, Schema as S } from 'effect'

import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from '../asset-bitcoin-boundary'
import { currentIsoTimestamp } from '../runtime-primitives'
import { workerLogEntry } from '../observability'
import { runLedgerStatements } from '../payments-ledger'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import {
  agentRefForUser,
  usdCreditGrantIdempotencyKey,
  usdCreditGrantReceiptRef,
  usdCreditGrantStatements,
} from './usd-credit-bridge'
import { parsePositiveIntEnv } from './inference-free-tier-key'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

// ----------------------------------------------------------------------------
// Tunables
// ----------------------------------------------------------------------------

// The signup grant amount: $10.00, matching the owner-stated MVP promise
// verbatim ("$10 free credit ... $10 per GitHub account").
export const GITHUB_SIGNUP_CREDIT_GRANT_CENTS = 1000 as const

// Minimum GitHub account age (seconds) before the signup grant is trusted.
// !! TUNABLE: a floor this low mainly blocks trivial same-second scripted
// account creation; it intentionally does not penalize a real user whose
// GitHub account happens to be young. Env-overridable without a deploy.
export const MIN_GITHUB_ACCOUNT_AGE_SECONDS = 60 * 60 // 1 hour
export const MIN_GITHUB_ACCOUNT_AGE_SECONDS_ENV_KEY =
  'GITHUB_SIGNUP_CREDIT_MIN_ACCOUNT_AGE_SECONDS' as const

// Per-IP-hash, per-UTC-day ceiling on distinct GitHub accounts granted a
// signup credit. !! TUNABLE, env-overridable without a deploy (mirrors
// FREE_KEY_MAX_MINTS_PER_IP_PER_DAY's pattern).
export const GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY = 20 as const
export const GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY_ENV_KEY =
  'GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY' as const

export const resolveGithubSignupCreditMinAccountAgeSeconds = (
  env: Readonly<{ GITHUB_SIGNUP_CREDIT_MIN_ACCOUNT_AGE_SECONDS?: unknown }>,
): number =>
  parsePositiveIntEnv(
    env.GITHUB_SIGNUP_CREDIT_MIN_ACCOUNT_AGE_SECONDS,
    MIN_GITHUB_ACCOUNT_AGE_SECONDS,
  )

export const resolveGithubSignupCreditIpMintCap = (
  env: Readonly<{ GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY?: unknown }>,
): number =>
  parsePositiveIntEnv(
    env.GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY,
    GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY,
  )

// ----------------------------------------------------------------------------
// Stable refs
// ----------------------------------------------------------------------------

export const githubSignupCreditGrantRef = (githubUserId: string): string =>
  `signup:github:${githubUserId}`

export const githubSignupCreditContextRef = (githubUserId: string): string =>
  `github-signup:${githubUserId}`

// ----------------------------------------------------------------------------
// Pure decisions
// ----------------------------------------------------------------------------

export type GithubAccountAgeDecision = Readonly<{
  eligible: boolean
  ageSeconds: number | null
  reasonRef: string
}>

export const GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_AGE_UNKNOWN =
  'reason.github_signup_credit.account_age_unknown' as const
export const GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_TOO_NEW =
  'reason.github_signup_credit.account_too_new' as const
export const GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_AGE_OK =
  'reason.github_signup_credit.account_age_ok' as const

// Decide whether a GitHub account is old enough to trust for the signup
// grant. A missing/unparseable `created_at` is NOT penalized (falls through
// eligible) — see the module header for why.
export const decideGithubAccountAge = (
  input: Readonly<{
    githubAccountCreatedAtIso: string | undefined
    nowIso: string
    minAccountAgeSeconds?: number | undefined
  }>,
): GithubAccountAgeDecision => {
  const createdAt = input.githubAccountCreatedAtIso
  if (createdAt === undefined || createdAt.trim() === '') {
    return {
      ageSeconds: null,
      eligible: true,
      reasonRef: GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_AGE_UNKNOWN,
    }
  }
  const createdAtMs = Date.parse(createdAt)
  const nowMs = Date.parse(input.nowIso)
  if (!Number.isFinite(createdAtMs) || !Number.isFinite(nowMs)) {
    return {
      ageSeconds: null,
      eligible: true,
      reasonRef: GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_AGE_UNKNOWN,
    }
  }
  const ageSeconds = Math.max(0, Math.floor((nowMs - createdAtMs) / 1000))
  const minAge = Math.max(0, Math.trunc(input.minAccountAgeSeconds ?? MIN_GITHUB_ACCOUNT_AGE_SECONDS))
  if (ageSeconds < minAge) {
    return {
      ageSeconds,
      eligible: false,
      reasonRef: GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_TOO_NEW,
    }
  }
  return {
    ageSeconds,
    eligible: true,
    reasonRef: GITHUB_SIGNUP_CREDIT_REASON_ACCOUNT_AGE_OK,
  }
}

export type IpMintCapDecision = Readonly<{
  allowed: boolean
  maxMintsPerDay: number
  mintsToday: number
  reasonRef: string
}>

export const GITHUB_SIGNUP_CREDIT_REASON_IP_CAP_OK =
  'reason.github_signup_credit.ip_mint_cap_ok' as const
export const GITHUB_SIGNUP_CREDIT_REASON_IP_CAP_EXCEEDED =
  'reason.github_signup_credit.ip_mint_cap_exceeded' as const

export const decideGithubSignupCreditIpMintCap = (
  input: Readonly<{
    mintsToday: number
    maxMintsPerDay?: number | undefined
  }>,
): IpMintCapDecision => {
  const max = Math.max(
    1,
    Math.trunc(input.maxMintsPerDay ?? GITHUB_SIGNUP_CREDIT_MAX_MINTS_PER_IP_PER_DAY),
  )
  const mints = Math.max(0, Math.trunc(input.mintsToday))
  if (mints >= max) {
    return {
      allowed: false,
      maxMintsPerDay: max,
      mintsToday: mints,
      reasonRef: GITHUB_SIGNUP_CREDIT_REASON_IP_CAP_EXCEEDED,
    }
  }
  return {
    allowed: true,
    maxMintsPerDay: max,
    mintsToday: mints,
    reasonRef: GITHUB_SIGNUP_CREDIT_REASON_IP_CAP_OK,
  }
}

// ----------------------------------------------------------------------------
// D1 reads/writes
// ----------------------------------------------------------------------------

type ExistingGrantRow = Readonly<{
  grant_ref: string
  amount_usd_cents: number
  amount_msat: number
  credit_receipt_ref: string
}>

export const readGithubSignupCreditGrant = async (
  db: D1Database,
  githubUserId: string,
): Promise<ExistingGrantRow | null> => {
  const row = await db
    .prepare(
      `SELECT grant_ref, amount_usd_cents, amount_msat, credit_receipt_ref
         FROM github_signup_credit_grants
        WHERE github_user_id = ?
        LIMIT 1`,
    )
    .bind(githubUserId)
    .first<ExistingGrantRow>()
  return row ?? null
}

// Per-user grant history (#8480 balance/history UI). Newest first.
export type GithubSignupCreditGrantRecord = Readonly<{
  grantRef: string
  githubUserId: string
  amountUsdCents: number
  amountMsat: number
  creditReceiptRef: string
  githubAccountCreatedAt: string | null
  createdAt: string
}>

type GrantHistoryRow = Readonly<{
  grant_ref: string
  github_user_id: string
  amount_usd_cents: number
  amount_msat: number
  credit_receipt_ref: string
  github_account_created_at: string | null
  created_at: string
}>

export const readGithubSignupCreditGrantsForUser = async (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<GithubSignupCreditGrantRecord>> => {
  const result = await db
    .prepare(
      `SELECT grant_ref, github_user_id, amount_usd_cents, amount_msat,
              credit_receipt_ref, github_account_created_at, created_at
         FROM github_signup_credit_grants
        WHERE user_id = ?
        ORDER BY created_at DESC`,
    )
    .bind(userId)
    .all<GrantHistoryRow>()
  return result.results.map(row => ({
    amountMsat: row.amount_msat,
    amountUsdCents: row.amount_usd_cents,
    createdAt: row.created_at,
    creditReceiptRef: row.credit_receipt_ref,
    githubAccountCreatedAt: row.github_account_created_at,
    githubUserId: row.github_user_id,
    grantRef: row.grant_ref,
  }))
}

const readIpMintsToday = async (
  db: D1Database,
  ipHash: string,
  mintDay: string,
): Promise<number> => {
  try {
    const row = await db
      .prepare(
        `SELECT mint_count FROM github_signup_credit_ip_mints
          WHERE ip_hash = ? AND mint_day = ? LIMIT 1`,
      )
      .bind(ipHash, mintDay)
      .first<{ mint_count: number }>()
    return typeof row?.mint_count === 'number' ? row.mint_count : 0
  } catch {
    return 0
  }
}

const recordIpMint = async (
  db: D1Database,
  input: Readonly<{ ipHash: string; mintDay: string; nowIso: string }>,
): Promise<void> => {
  try {
    await db
      .prepare(
        `INSERT INTO github_signup_credit_ip_mints
           (ip_hash, mint_day, mint_count, created_at, updated_at)
         VALUES (?, ?, 1, ?, ?)
         ON CONFLICT(ip_hash, mint_day) DO UPDATE SET
           mint_count = mint_count + 1,
           updated_at = excluded.updated_at`,
      )
      .bind(input.ipHash, input.mintDay, input.nowIso, input.nowIso)
      .run()
  } catch {
    // Best-effort abuse-floor accounting; never blocks the already-granted
    // credit if this write fails.
  }
}

// The grant-tracking metadata insert (`github_signup_credit_grants`, a D1
// admin-domain table — CFG-4: NOT part of the Postgres credits ledger). Its
// UNIQUE github_user_id constraint is a second, independent idempotency guard
// alongside the pay_ins UNIQUE idempotency key; since the hard cutover the two
// no longer share one transaction (see the non-atomic-seam note in
// `grantGithubSignupCredit`).
const githubSignupCreditGrantMetadataStatement = (
  input: Readonly<{
    grantRef: string
    githubUserId: string
    userId: string
    accountRef: string
    amountUsdCents: number
    amountMsat: number
    creditReceiptRef: string
    githubAccountCreatedAtIso: string | undefined
    ipHash: string | undefined
  }>,
  nowIso: string,
): Readonly<{
  sql: string
  params: ReadonlyArray<string | number | null>
}> => ({
  params: [
    input.grantRef,
    input.githubUserId,
    input.userId,
    input.accountRef,
    input.amountUsdCents,
    input.amountMsat,
    input.creditReceiptRef,
    input.githubAccountCreatedAtIso ?? null,
    input.ipHash ?? null,
    nowIso,
  ],
  sql: `INSERT OR IGNORE INTO github_signup_credit_grants
          (grant_ref, github_user_id, user_id, account_ref, amount_usd_cents,
           amount_msat, credit_receipt_ref, github_account_created_at, ip_hash,
           created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
})

// ----------------------------------------------------------------------------
// The grant action
// ----------------------------------------------------------------------------

export class GithubSignupCreditGrantError extends S.TaggedErrorClass<GithubSignupCreditGrantError>()(
  'GithubSignupCreditGrantError',
  { cause: S.Defect },
) {}

export type GithubSignupCreditGrantOutcome =
  | Readonly<{
      ok: true
      alreadyGranted: boolean
      grantRef: string
      grantedCents: number
      grantedMsat: number
      receiptRef: string
    }>
  | Readonly<{
      ok: false
      reason: 'account_too_new'
      message: string
      ageSeconds: number | null
    }>
  | Readonly<{
      ok: false
      reason: 'ip_mint_cap_exceeded'
      message: string
    }>
  | Readonly<{
      ok: false
      reason: 'asset_boundary_violation'
      message: string
      reasonRef?: string
    }>

export type GithubSignupCreditGrantDeps = Readonly<{
  // The D1 database for the grant-tracking tables
  // (`github_signup_credit_grants`, `github_signup_credit_ip_mints`).
  db: D1Database
  // The credits-domain ledger (CFG-4, #8519: `pay_ins`/`pay_in_legs`/
  // `agent_balances` are Cloud SQL Postgres-authoritative).
  ledgerDb: PaymentsLedgerDb
  nowIso?: (() => string) | undefined
  minAccountAgeSeconds?: number | undefined
  maxMintsPerIpPerDay?: number | undefined
  // Issue #8505 (Part 2): fail-soft, best-effort per-user credit-balance
  // projection into Khala Sync (`scope.user.<userId>`) — same seam as the
  // inference/cloud metering hooks' `recordCreditBalanceProjection`. Called
  // AFTER a FRESH grant commits (never for an idempotent-duplicate replay),
  // with the SAME idempotency key the ledger grant used
  // (`usdCreditGrantIdempotencyKey`). Optional; a deployment without the
  // Khala Sync binding (or a test) grants exactly as before.
  recordCreditBalanceProjection?: (
    event: Readonly<{
      accountRef: string
      idempotencyKey: string
      deltaUsdCents: number
      observedAt: string
    }>,
  ) => Promise<void>
}>

// Grant the $10 signup credit for a GitHub account, idempotent forever on the
// GitHub user id. Call this on every GitHub login success (not just "first" —
// idempotency makes repeat calls free no-ops), passing the caller's IP hash
// and the GitHub account's `created_at` when available.
export const grantGithubSignupCredit = (
  input: Readonly<{
    userId: string
    githubUserId: string
    githubAccountCreatedAtIso?: string
    ipHash?: string
  }>,
  deps: GithubSignupCreditGrantDeps,
): Effect.Effect<GithubSignupCreditGrantOutcome, GithubSignupCreditGrantError> =>
  Effect.gen(function* () {
    const nowIso = (deps.nowIso ?? currentIsoTimestamp)()

    const existing = yield* Effect.tryPromise({
      catch: (cause: unknown) => new GithubSignupCreditGrantError({ cause }),
      try: () => readGithubSignupCreditGrant(deps.db, input.githubUserId),
    })
    if (existing !== null) {
      return {
        alreadyGranted: true,
        grantedCents: existing.amount_usd_cents,
        grantedMsat: existing.amount_msat,
        grantRef: existing.grant_ref,
        ok: true,
        receiptRef: existing.credit_receipt_ref,
      }
    }

    const ageDecision = decideGithubAccountAge({
      githubAccountCreatedAtIso: input.githubAccountCreatedAtIso,
      minAccountAgeSeconds: deps.minAccountAgeSeconds,
      nowIso,
    })
    if (!ageDecision.eligible) {
      yield* Effect.logInfo(
        workerLogEntry('inference.github_signup_credit.deferred_account_age', {
          ageSeconds: ageDecision.ageSeconds,
          reasonRef: ageDecision.reasonRef,
          userId: input.userId,
        }),
      )
      return {
        ageSeconds: ageDecision.ageSeconds,
        message:
          'GitHub account is too new to grant the signup credit yet; try again later.',
        ok: false,
        reason: 'account_too_new',
      }
    }

    // RL-3: this grant must never create a withdrawable Bitcoin liability.
    // Tagged 'free' (promotional, not fiat-purchased) — see module header.
    const revenueAsset: AssetBoundaryAsset = 'free'
    const violation = validateAssetBoundary({
      contributorAsset: revenueAsset,
      movement: 'spend',
      revenueAsset,
    })
    if (violation !== null) {
      yield* Effect.logInfo(
        workerLogEntry('inference.github_signup_credit.boundary_denied', {
          reasonRef: violation.reasonRef,
          userId: input.userId,
        }),
      )
      return {
        message: violation.reason,
        ok: false,
        reason: 'asset_boundary_violation',
        reasonRef: violation.reasonRef,
      }
    }

    const mintDay = nowIso.slice(0, 10)
    if (input.ipHash !== undefined) {
      const mintsToday = yield* Effect.tryPromise({
        catch: (cause: unknown) => new GithubSignupCreditGrantError({ cause }),
        try: () => readIpMintsToday(deps.db, input.ipHash!, mintDay),
      })
      const capDecision = decideGithubSignupCreditIpMintCap({
        maxMintsPerDay: deps.maxMintsPerIpPerDay,
        mintsToday,
      })
      if (!capDecision.allowed) {
        yield* Effect.logInfo(
          workerLogEntry('inference.github_signup_credit.deferred_ip_cap', {
            maxMintsPerDay: capDecision.maxMintsPerDay,
            mintsToday: capDecision.mintsToday,
            userId: input.userId,
          }),
        )
        return {
          message:
            'Too many signup credit grants from this network today; try again tomorrow.',
          ok: false,
          reason: 'ip_mint_cap_exceeded',
        }
      }
    }

    const grantRef = githubSignupCreditGrantRef(input.githubUserId)
    const accountRef = agentRefForUser(input.userId)
    const contextRef = githubSignupCreditContextRef(input.githubUserId)
    const grantMsat = usdCentsToMsatFloor(GITHUB_SIGNUP_CREDIT_GRANT_CENTS)

    // CFG-4 NON-ATOMIC SEAM: the msat grant (Postgres credits ledger) and the
    // grant-tracking metadata row (`github_signup_credit_grants`, D1) can no
    // longer share one transaction. Order: LEDGER FIRST, metadata second — if
    // the metadata write fails after the credit landed, a retry re-enters here
    // (the existing-grant read above still misses), the ledger grant replays
    // as an idempotent no-op (UNIQUE pay_ins idempotency key + the guarded
    // balance UPDATE), and the metadata insert lands. Metadata-first would be
    // WRONG: a stranded metadata row would answer `alreadyGranted` forever
    // without the credit ever existing.
    yield* Effect.tryPromise({
      catch: (cause: unknown) => new GithubSignupCreditGrantError({ cause }),
      try: () =>
        runLedgerStatements(
          deps.ledgerDb,
          usdCreditGrantStatements(
            { accountRef, contextRef, grantMsat, grantRef },
            nowIso,
          ),
        ),
    })

    const metadata = githubSignupCreditGrantMetadataStatement(
      {
        accountRef,
        amountMsat: grantMsat,
        amountUsdCents: GITHUB_SIGNUP_CREDIT_GRANT_CENTS,
        creditReceiptRef: usdCreditGrantReceiptRef(grantRef),
        githubAccountCreatedAtIso: input.githubAccountCreatedAtIso,
        githubUserId: input.githubUserId,
        grantRef,
        ipHash: input.ipHash,
        userId: input.userId,
      },
      nowIso,
    )
    yield* Effect.tryPromise({
      catch: (cause: unknown) => new GithubSignupCreditGrantError({ cause }),
      try: () =>
        deps.db.prepare(metadata.sql).bind(...metadata.params).run(),
    })

    if (input.ipHash !== undefined) {
      yield* Effect.tryPromise({
        catch: (cause: unknown) => new GithubSignupCreditGrantError({ cause }),
        try: () => recordIpMint(deps.db, { ipHash: input.ipHash!, mintDay, nowIso }),
      })
    }

    yield* Effect.logInfo(
      workerLogEntry('inference.github_signup_credit.granted', {
        grantedCents: GITHUB_SIGNUP_CREDIT_GRANT_CENTS,
        grantedMsat: grantMsat,
        grantRef,
        userId: input.userId,
      }),
    )

    // Issue #8505 (Part 2): best-effort live projection of the FRESH signup
    // grant into scope.user.<userId>, reusing the SAME idempotency key the
    // ledger grant just used. Fail-soft by contract and never blocks/reverses
    // the ledger grant above, which already committed.
    if (deps.recordCreditBalanceProjection !== undefined) {
      yield* Effect.promise(() =>
        deps
          .recordCreditBalanceProjection!({
            accountRef,
            deltaUsdCents: GITHUB_SIGNUP_CREDIT_GRANT_CENTS,
            idempotencyKey: usdCreditGrantIdempotencyKey(grantRef),
            observedAt: nowIso,
          })
          .catch(() => undefined),
      )
    }

    return {
      alreadyGranted: false,
      grantedCents: GITHUB_SIGNUP_CREDIT_GRANT_CENTS,
      grantedMsat: grantMsat,
      grantRef,
      ok: true,
      receiptRef: usdCreditGrantReceiptRef(grantRef),
    }
  })
