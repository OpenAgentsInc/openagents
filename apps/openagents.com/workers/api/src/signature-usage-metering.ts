// Signature usage metering — the capability that PRODUCES the public-safe
// usage evidence the signature-marketplace revenue gate consumes
// (EPIC #5523 / DE-6 #5529; promise marketplace.signature_monetization.v1, red).
//
// THE GAP THIS CLOSES: `signature-marketplace-revenue-gate.ts` reaches its
// `metered` state only when it is handed `usageEventRefs`,
// `usageIdempotencyRefs`, and `exactUsageSubjectRefs`. Until now nothing in the
// repo derived those refs — there was no metering record path — which is
// exactly `blocker.product_promises.signature_usage_metering_missing`. This
// module records idempotent, exact-subject-bound signature usage events and
// derives the three public-safe ref families the gate needs.
//
// SCOPE / HONESTY: PURE and INERT. It moves no money, runs no signature, reads
// no wallet, writes no payout, and settles nothing. Metering is the `metered`
// rung of the gate's state ladder — strictly BEFORE pricing/attribution/payout/
// settlement. The promise marketplace.signature_monetization.v1 STAYS `red`:
// `blocker.product_promises.signature_settlement_missing` is untouched and
// owner-gated. Nothing here flips it green; a green flip is receipt-first and
// owner-signed per proof.claim_upgrade_receipts.v1.
//
// PUBLIC-SAFE BY CONSTRUCTION: the derived refs reuse the revenue gate's own
// public-safe ref discipline — they carry no raw usage payload, no prompt, no
// customer / provider / wallet / payment / payout / private-repo / secret / raw
// timestamp material. Callers supply only neutral, bounded identifiers
// (a program-signature subject ref, a package ref, an idempotency token).

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type SignatureMarketplaceRevenueGate,
  type SignatureMarketplaceRevenueGateInput,
  projectSignatureMarketplaceRevenueGate,
} from './signature-marketplace-revenue-gate'

export const SIGNATURE_USAGE_METERING_SCHEMA =
  'openagents.signature_usage_metering.v1' as const

export const SIGNATURE_MONETIZATION_PROMISE =
  'marketplace.signature_monetization.v1' as const

// The metering surface clears the metering blocker only; settlement stays
// owner-gated. Both are surfaced so the projection is honest about what remains.
export const SIGNATURE_USAGE_METERING_BLOCKER =
  'blocker.product_promises.signature_usage_metering_missing' as const
export const SIGNATURE_SETTLEMENT_BLOCKER =
  'blocker.product_promises.signature_settlement_missing' as const

// A bounded, neutral ref token: same alphabet the revenue gate's safeRefPattern
// accepts, so every ref we derive round-trips through the gate's guards.
const SAFE_TOKEN_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,200}$/

// A focused semantic deny-list mirroring the revenue gate's intent: a metering
// record must never form around money/secret/customer/path/timestamp material,
// even when it is syntactically a bounded token. The derived refs are also
// re-checked by the gate's own (broader) guard at projection time; this just
// fails fast at record time so unsafe material never enters the store.
const UNSAFE_TOKEN_PATTERN =
  /(@|\/users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer|email|gho_|ghp_|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mnemonic|oauth|payment|payout|preimage|private|provider|raw[_-]|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet)/i
const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}t\d{2}:\d{2}:\d{2}/i

export class SignatureUsageMeteringError extends S.TaggedErrorClass<SignatureUsageMeteringError>()(
  'SignatureUsageMeteringError',
  {
    reason: S.String,
  },
) {}

/**
 * One recorded signature-usage event. INERT: a record that a verified program
 * signature was used once, bound to an exact subject and deduped by an
 * idempotency key. Carries NO usage payload, prompt, output, price, or money.
 */
export const SignatureUsageEvent = S.Struct({
  schema: S.Literal(SIGNATURE_USAGE_METERING_SCHEMA),
  /** Neutral signature/program subject the usage binds to (e.g. a package or
   *  program-signature ref). The "exact usage subject". */
  signatureSubjectRef: S.String,
  /** Neutral package ref the signature belongs to. */
  packageRef: S.String,
  /** Caller-supplied idempotency token; identical tokens collapse to one event. */
  idempotencyToken: S.String,
  /** Derived, public-safe ref families the revenue gate consumes. */
  usageEventRef: S.String,
  usageIdempotencyRef: S.String,
  exactUsageSubjectRef: S.String,
})
export type SignatureUsageEvent = typeof SignatureUsageEvent.Type

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const assertSafeToken = (label: string, value: string): string => {
  const trimmed = value.trim()
  if (!isNonEmpty(trimmed)) {
    throw new SignatureUsageMeteringError({
      reason: `${label} must be a non-empty token`,
    })
  }
  if (
    !SAFE_TOKEN_PATTERN.test(trimmed) ||
    UNSAFE_TOKEN_PATTERN.test(trimmed) ||
    RAW_TIMESTAMP_PATTERN.test(trimmed)
  ) {
    throw new SignatureUsageMeteringError({
      reason: `${label} must be a bounded, public-safe token (no payload, prompt, secret, wallet, payment, customer, or timestamp material)`,
    })
  }
  return trimmed
}

/**
 * Build one metering record from neutral inputs. PURE / validating. The three
 * derived ref families are deterministic functions of the bounded inputs, so
 * the same (subject, idempotency token) always produces the same refs — which
 * is what makes metering idempotent and replayable.
 */
export const recordSignatureUsage = (input: {
  signatureSubjectRef: string
  packageRef: string
  idempotencyToken: string
}):
  | { ok: true; event: SignatureUsageEvent }
  | { ok: false; error: SignatureUsageMeteringError } => {
  try {
    const signatureSubjectRef = assertSafeToken(
      'signatureSubjectRef',
      input.signatureSubjectRef,
    )
    const packageRef = assertSafeToken('packageRef', input.packageRef)
    const idempotencyToken = assertSafeToken(
      'idempotencyToken',
      input.idempotencyToken,
    )

    return {
      ok: true,
      event: {
        schema: SIGNATURE_USAGE_METERING_SCHEMA,
        signatureSubjectRef,
        packageRef,
        idempotencyToken,
        usageEventRef: `usage_event.public.signature_market.${signatureSubjectRef}.${idempotencyToken}`,
        usageIdempotencyRef: `usage_idempotency.public.signature_market.${idempotencyToken}`,
        exactUsageSubjectRef: `usage_subject.public.signature_market.${signatureSubjectRef}`,
      },
    }
  } catch (error) {
    if (error instanceof SignatureUsageMeteringError) {
      return { ok: false, error }
    }
    throw error
  }
}

/**
 * An idempotent in-memory metering store. Recording the same idempotency token
 * twice is a no-op (the metering invariant). Injected so the surface stays pure
 * and testable; the live Worker passes an empty store while INERT.
 */
export type SignatureUsageMeteringStore = {
  list: () => ReadonlyArray<SignatureUsageEvent>
}

export const emptySignatureUsageMeteringStore: SignatureUsageMeteringStore = {
  list: () => [],
}

/**
 * Build an in-memory store from a set of recorded events, collapsing duplicate
 * idempotency tokens to the first occurrence (idempotent metering).
 */
export const makeInMemorySignatureUsageMeteringStore = (
  events: ReadonlyArray<SignatureUsageEvent>,
): SignatureUsageMeteringStore => {
  const byToken = new Map<string, SignatureUsageEvent>()
  for (const event of events) {
    if (!byToken.has(event.idempotencyToken)) {
      byToken.set(event.idempotencyToken, event)
    }
  }
  const deduped = [...byToken.values()]
  return { list: () => deduped }
}

/** The distinct public-safe usage-event refs across the store. */
export const meteringUsageEventRefs = (
  store: SignatureUsageMeteringStore,
): ReadonlyArray<string> => [
  ...new Set(store.list().map(event => event.usageEventRef)),
]

/** The distinct public-safe usage-idempotency refs across the store. */
export const meteringUsageIdempotencyRefs = (
  store: SignatureUsageMeteringStore,
): ReadonlyArray<string> => [
  ...new Set(store.list().map(event => event.usageIdempotencyRef)),
]

/** The distinct public-safe exact-usage-subject refs across the store. */
export const meteringExactUsageSubjectRefs = (
  store: SignatureUsageMeteringStore,
): ReadonlyArray<string> => [
  ...new Set(store.list().map(event => event.exactUsageSubjectRef)),
]

/**
 * The metering contribution to the revenue gate: only the three usage-evidence
 * ref families this capability is responsible for. Everything else
 * (attribution, pricing, rev-share, payout, settlement) stays the caller's /
 * owner's responsibility — this is the metering rung, nothing past it.
 */
export const meteringRevenueGateInput = (
  store: SignatureUsageMeteringStore,
): Pick<
  SignatureMarketplaceRevenueGateInput,
  'usageEventRefs' | 'usageIdempotencyRefs' | 'exactUsageSubjectRefs'
> => ({
  usageEventRefs: meteringUsageEventRefs(store),
  usageIdempotencyRefs: meteringUsageIdempotencyRefs(store),
  exactUsageSubjectRefs: meteringExactUsageSubjectRefs(store),
})

/**
 * Project the revenue-gate state reached by metering alone, given the package /
 * program-signature validation evidence the metering subject already carries.
 * This is the receipt-shaped check: with validation + metering refs present and
 * nothing past metering, the gate must report `state: 'metered'`.
 */
export const projectSignatureMeteringGate = (
  store: SignatureUsageMeteringStore,
  validationInput: Pick<
    SignatureMarketplaceRevenueGateInput,
    | 'activationRefs'
    | 'packagePublicationRefs'
    | 'packageValidationRefs'
    | 'packageRefs'
    | 'programSignatureRefs'
  >,
): SignatureMarketplaceRevenueGate =>
  projectSignatureMarketplaceRevenueGate({
    ...validationInput,
    ...meteringRevenueGateInput(store),
  })

/**
 * Staleness contract for the metering projection. Built fresh from the injected
 * store on every request, so it is `live_at_read` (maxStaleness 0).
 */
export const SignatureUsageMeteringStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['signature_usage_metering_changed'])

/**
 * Public-safe metering projection. Reports the recorded usage-evidence refs and
 * the gate state metering reaches, and stays honest: `inert: true`,
 * `promiseState: 'red'`, the settlement blocker still open. NO money, NO
 * settlement, NO live-revenue claim.
 */
export const projectSignatureUsageMetering = (
  store: SignatureUsageMeteringStore,
): {
  schema: typeof SIGNATURE_USAGE_METERING_SCHEMA
  promiseId: typeof SIGNATURE_MONETIZATION_PROMISE
  promiseState: 'red'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  meteredUsageEventCount: number
  usageEventRefs: ReadonlyArray<string>
  usageIdempotencyRefs: ReadonlyArray<string>
  exactUsageSubjectRefs: ReadonlyArray<string>
  clearsBlocker: typeof SIGNATURE_USAGE_METERING_BLOCKER
  remainingOwnerGatedBlocker: typeof SIGNATURE_SETTLEMENT_BLOCKER
} => {
  const usageEventRefs = meteringUsageEventRefs(store)
  return {
    schema: SIGNATURE_USAGE_METERING_SCHEMA,
    promiseId: SIGNATURE_MONETIZATION_PROMISE,
    // Honest: metering clears one blocker; the promise stays red until
    // settlement, which is owner-gated.
    promiseState: 'red',
    inert: true,
    generatedAt: currentIsoTimestamp(),
    maxStalenessSeconds: SignatureUsageMeteringStaleness.maxStalenessSeconds,
    staleness: SignatureUsageMeteringStaleness,
    meteredUsageEventCount: usageEventRefs.length,
    usageEventRefs,
    usageIdempotencyRefs: meteringUsageIdempotencyRefs(store),
    exactUsageSubjectRefs: meteringExactUsageSubjectRefs(store),
    clearsBlocker: SIGNATURE_USAGE_METERING_BLOCKER,
    remainingOwnerGatedBlocker: SIGNATURE_SETTLEMENT_BLOCKER,
  }
}
