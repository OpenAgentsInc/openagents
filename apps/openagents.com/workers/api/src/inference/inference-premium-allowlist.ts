// Premium-model owner-grant allowlist for the inference gateway (EPIC #5474,
// free-tier enablement §2).
//
// Premium models (Claude Opus/Sonnet/Haiku, GPT, and any unknown/partner
// passthrough model) are OWNER-GRANT ONLY: a request for a premium model is
// allowed only when the requesting account's resolved OWNER identity is on an
// owner-controlled allowlist (`inference_premium_allowlist`, migration 0210). A
// non-allowlisted owner requesting a premium model is DENIED with a clear,
// actionable message rather than silently metered. The free/cheap tiers
// (Gemini Flash, the Fireworks open models) are NEVER premium-gated — they are
// the default path and the free tier.
//
// The allowlist keys on the SAME owner identity the free pool uses
// (`owner:<ownerUserId>` for a verified claim, `account:<accountRef>` for an
// unclaimed account — see inference-owner-identity.ts), so granting one owner
// covers ALL of that owner's accounts/autopilots, and an unclaimed account can
// never be premium-eligible (its synthetic account key is not granted by the
// owner-controlled admin surface, which grants verified owner keys only).
//
// SHAPE: a pure decision (`decidePremiumModelAccess`) + a route gate seam
// (`makePremiumAccessGate`) the chat-completions route calls AFTER auth and
// BEFORE provider dispatch, plus the owner/admin store (`grant` / `revoke` /
// `isAllowed`) the admin surface drives. Bounded model-class classification
// (reusing the router's `classifyModel`), never an intent parser.

import { Effect } from 'effect'

import type {
  InferenceEntitlementsGateReads,
  InferenceEntitlementsMirror,
} from '../inference-entitlements-store'

import { currentIsoTimestamp } from '../runtime-primitives'
import { classifyModel, type ModelClass } from './model-router'
import {
  type VerifiedOwnerIdentityResolver,
  isVerifiedOwnerKey,
  resolveOwnerKey,
} from './inference-owner-identity'

class PremiumAllowlistPersistenceError extends Error {
  readonly _tag = 'PremiumAllowlistPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'PremiumAllowlistPersistenceError'
  }
}

const premiumAllowlistPersistenceError = (error: unknown) =>
  new PremiumAllowlistPersistenceError(error)

// ----------------------------------------------------------------------------
// Premium classification (bounded model-class set)
// ----------------------------------------------------------------------------

// Model classes that are PREMIUM (owner-grant only). Claude (frontier, our
// best-margin Vertex lane), plus the `unknown` class (partner passthrough —
// GPT and anything else we hold no owned quota for). The free/cheap tiers —
// `gemini` (free default) and `open` (managed Fireworks open models) — are NOT
// premium. !! TUNABLE: free-tier enablement §2.
export const PREMIUM_MODEL_CLASSES: ReadonlyArray<ModelClass> = [
  'claude',
  'unknown',
]

export const isPremiumModel = (model: string): boolean =>
  PREMIUM_MODEL_CLASSES.includes(classifyModel(model))

// ----------------------------------------------------------------------------
// Pure decision
// ----------------------------------------------------------------------------

export type PremiumAccessDecision = Readonly<{
  // True when the request may proceed (non-premium model, OR a premium model
  // whose owner is allowlisted). False => the route denies with the message.
  allowed: boolean
  // Whether the requested model is in a premium class (diagnostics).
  premium: boolean
  // Stable, neutral reason ref (observability / client routing).
  reasonRef: string
  // A clear, actionable human message when denied; empty when allowed.
  message: string
}>

// Stable reason refs (neutral, no payment material).
export const PREMIUM_REASON_ALLOWED_NON_PREMIUM =
  'reason.inference_premium.non_premium_model' as const
export const PREMIUM_REASON_ALLOWED_GRANTED =
  'reason.inference_premium.owner_allowlisted' as const
export const PREMIUM_REASON_DENIED_NOT_ALLOWLISTED =
  'reason.inference_premium.owner_not_allowlisted' as const

// The actionable denial message. Tells the owner exactly how to unblock:
// premium models require an owner grant; the default free model is always
// available. !! Copy is a product-claim surface; keep it neutral + actionable.
export const premiumDeniedMessage = (model: string): string =>
  `Model "${model}" is a premium model and requires an owner grant. ` +
  `Ask the account owner to add this owner identity to the premium allowlist, ` +
  `or use the default free model "gemini-3.5-flash".`

// Decide whether a request for `model` is allowed given whether the resolved
// owner is allowlisted. Pure. Non-premium models are always allowed; premium
// models require `ownerAllowlisted === true`.
export const decidePremiumModelAccess = (
  input: Readonly<{
    model: string
    ownerAllowlisted: boolean
  }>,
): PremiumAccessDecision => {
  if (!isPremiumModel(input.model)) {
    return {
      allowed: true,
      message: '',
      premium: false,
      reasonRef: PREMIUM_REASON_ALLOWED_NON_PREMIUM,
    }
  }
  if (input.ownerAllowlisted) {
    return {
      allowed: true,
      message: '',
      premium: true,
      reasonRef: PREMIUM_REASON_ALLOWED_GRANTED,
    }
  }
  return {
    allowed: false,
    message: premiumDeniedMessage(input.model),
    premium: true,
    reasonRef: PREMIUM_REASON_DENIED_NOT_ALLOWLISTED,
  }
}

// ----------------------------------------------------------------------------
// Allowlist store (owner/admin-controlled)
// ----------------------------------------------------------------------------

// Default grant scope. Reserves room for per-model-class grants later; today a
// row grants the whole premium tier.
export const DEFAULT_PREMIUM_SCOPE = 'all_premium' as const

export type PremiumAllowlistGrant = Readonly<{
  ownerKey: string
  scope: string
  grantedBy: string | null
  note: string | null
}>

// Plain-async read of whether an owner key is on the premium allowlist. The
// route gate (a Promise-returning seam) uses this directly so no Effect->Promise
// bridge runs on the hot path. Returns false on any read error (fail-closed).
export const readOwnerAllowlisted = async (
  db: D1Database,
  ownerKey: string,
): Promise<boolean> => {
  try {
    const row = await db
      .prepare(
        `SELECT owner_key FROM inference_premium_allowlist WHERE owner_key = ? LIMIT 1`,
      )
      .bind(ownerKey)
      .first<{ owner_key: string }>()
    return row !== null
  } catch {
    return false
  }
}

// Whether an owner key is currently on the premium allowlist, as an Effect (for
// Effect-shaped callers / tests). Wraps `readOwnerAllowlisted`.
export const isOwnerAllowlisted = (
  db: D1Database,
  ownerKey: string,
): Effect.Effect<boolean> =>
  Effect.promise(() => readOwnerAllowlisted(db, ownerKey))

// Grant the premium tier to an owner key (idempotent upsert). The owner/admin
// surface calls this. Refuses to grant a non-verified (synthetic account) owner
// key — premium is owner-grant only, and an unclaimed account has no verified
// owner to grant. Returns whether the grant is now in place.
export const grantPremiumAccess = (
  db: D1Database,
  input: Readonly<{
    ownerKey: string
    grantedBy?: string | null | undefined
    note?: string | null | undefined
    scope?: string | undefined
    nowIso?: (() => string) | undefined
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Effect.Effect<boolean> =>
  Effect.gen(function* () {
    // Premium grants apply to VERIFIED owner identities only; a synthetic
    // account key (unclaimed) is never granted.
    if (!isVerifiedOwnerKey(input.ownerKey)) {
      return false
    }
    const nowIso = (input.nowIso ?? currentIsoTimestamp)()
    return yield* Effect.tryPromise({
      catch: premiumAllowlistPersistenceError,
      try: async () => {
        await db
          .prepare(
            `INSERT INTO inference_premium_allowlist
               (owner_key, scope, granted_by, note, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(owner_key) DO UPDATE SET
               scope = excluded.scope,
               granted_by = excluded.granted_by,
               note = excluded.note,
               updated_at = excluded.updated_at`,
          )
          .bind(
            input.ownerKey,
            input.scope ?? DEFAULT_PREMIUM_SCOPE,
            input.grantedBy ?? null,
            input.note ?? null,
            nowIso,
            nowIso,
          )
          .run()
        mirror?.([
          {
            kind: 'write',
            row: {
              created_at: nowIso,
              granted_by: input.grantedBy ?? null,
              note: input.note ?? null,
              owner_key: input.ownerKey,
              scope: input.scope ?? DEFAULT_PREMIUM_SCOPE,
              updated_at: nowIso,
            },
            table: 'inference_premium_allowlist',
          },
        ])
        return true
      },
    }).pipe(Effect.catch(() => Effect.succeed(false)))
  })

// Revoke an owner key's premium grant (idempotent). Returns whether the call
// completed without error (the key is absent afterward regardless).
export const revokePremiumAccess = (
  db: D1Database,
  ownerKey: string,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Effect.Effect<boolean> =>
  Effect.tryPromise({
    catch: premiumAllowlistPersistenceError,
    try: async () => {
      await db
        .prepare(`DELETE FROM inference_premium_allowlist WHERE owner_key = ?`)
        .bind(ownerKey)
        .run()
      mirror?.([
        {
          kind: 'delete_owner_grant',
          ownerKey,
          table: 'inference_premium_allowlist',
        },
      ])
      return true
    },
  }).pipe(Effect.catch(() => Effect.succeed(false)))

// ----------------------------------------------------------------------------
// Route gate seam
// ----------------------------------------------------------------------------

export type PremiumAccessGateDeps = Readonly<{
  db: D1Database
  resolveOwnerIdentity: VerifiedOwnerIdentityResolver
  // KS-8.9 (#8320): routed enforcement read (compare/postgres modes).
  // Absent => the untouched inline D1 read (zero added hot-path latency).
  gateReads?:
    | Pick<InferenceEntitlementsGateReads, 'premiumAllowlisted'>
    | undefined
}>

// A route-level premium gate: given an account ref + requested model, returns
// the premium-access decision the route enforces (deny non-allowlisted premium
// with the actionable message). Non-premium models short-circuit to allowed
// WITHOUT a DB read. Wired into the chat-completions route's `checkPremiumAccess`
// seam (open/no-op when unwired, e.g. the inert flag-off path).
export type PremiumAccessGate = (
  accountRef: string,
  model: string,
) => Promise<PremiumAccessDecision>

export const makePremiumAccessGate = (
  deps: PremiumAccessGateDeps,
): PremiumAccessGate => {
  return async (accountRef: string, model: string) => {
    // Non-premium models never touch the allowlist (the free/cheap default path).
    if (!isPremiumModel(model)) {
      return decidePremiumModelAccess({ model, ownerAllowlisted: false })
    }
    const identity = await deps.resolveOwnerIdentity(accountRef)
    const ownerKey = resolveOwnerKey(accountRef, identity)
    // Routed read stays FAIL-CLOSED like readOwnerAllowlisted: any error
    // resolves to not-allowlisted (deny premium).
    const ownerAllowlisted =
      deps.gateReads === undefined
        ? await readOwnerAllowlisted(deps.db, ownerKey)
        : await deps.gateReads.premiumAllowlisted(ownerKey).catch(() => false)
    return decidePremiumModelAccess({ model, ownerAllowlisted })
  }
}
