// Abuse / KYC / rate-limit fair-share controls for the OpenAgents inference
// gateway (EPIC #5474, child #5486).
//
// A funded-credits, OpenAI-compatible API is a classic abuse target: one
// customer can starve the shared Vertex quota (or our Fireworks adaptive
// limits), prompt-inject for free compute, or pay by card and charge back after
// burning credits (gateway business doc §7 "Abuse / KYC"). This module is the
// ONE typed home for the bounded, ENFORCEABLE parts of that defense, plus the
// honest policy-only seams flagged as such.
//
// Boundaries (kept deliberately narrow, matching the rest of the inference dir):
//   - PURE + deterministic. Every decision here is a pure function of typed
//     inputs (counts, caps, levels, signals) — no Effect runtime, no IO, no
//     clock, no env. The ONE exception is `clawbackInferenceCredits`, an
//     Effect-returning hook that moves money: it reuses the EXISTING PayIn-shaped
//     credit ledger (`payments-ledger.ts`) exactly like the metering hook
//     (#5477), never inventing a parallel money path.
//   - INERT-safe. These gates only bind when the gateway flag is on AND the
//     Worker wires a non-default config. The route/metering defaults leave every
//     gate OPEN (allow) so the flag-off path is byte-for-byte unchanged.
//   - PUBLIC-SAFE. No prompts, no payment material, no raw amounts beyond the
//     msat the ledger already exposes. Decisions carry stable refs + bounded
//     counters only.
//
// Reuse: the per-window request/spend bounds are seeded from the existing
// `AgentRateLimitPolicy` (src/agent-rate-limit-policy.ts) so the gateway shares
// the same default window the rest of the agent surface advertises, rather than
// inventing a parallel limit. The clawback path reuses `inferenceChargePayInPlan`
// / `payments-ledger` (the same atomic, constraint-guarded, idempotent ledger the
// metering hook charges through), so a clawback is just a typed inverse debit.

import { Effect, Schema as S } from 'effect'

import { workerLogEntry } from '../observability'
import {
  createPayInStatements,
  type PayInPlan,
  runLedgerStatements,
} from '../payments-ledger'
import { currentIsoTimestamp } from '../runtime-primitives'
import { AgentRateLimitPolicy } from '../agent-rate-limit-policy'

class InferenceClawbackPersistenceError extends Error {
  readonly _tag = 'InferenceClawbackPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'InferenceClawbackPersistenceError'
  }
}

const inferenceClawbackPersistenceError = (error: unknown) =>
  new InferenceClawbackPersistenceError(error)

// ----------------------------------------------------------------------------
// 1. Per-customer rate / fair-share limits
// ----------------------------------------------------------------------------

// Fair-share bounds for one account over one rolling window. Defaults are seeded
// from the shared `AgentRateLimitPolicy` so the gateway does not advertise a
// parallel window: `maxRequests` mirrors the policy `limit`, `windowSeconds`
// mirrors the policy window. `maxTokens` (output of the window) is the
// fair-share knob that keeps a single customer from draining the shared Vertex
// quota even within the request count — one slow customer streaming millions of
// tokens is the real starvation risk, not request count alone (doc §7).
export type FairShareLimits = Readonly<{
  maxRequests: number
  maxTokens: number
  windowSeconds: number
}>

export const DEFAULT_FAIR_SHARE_LIMITS: FairShareLimits = {
  // Per-window request ceiling, shared with the agent rate-limit policy.
  maxRequests: AgentRateLimitPolicy.limit,
  // Per-window TOKEN ceiling. Bounds the shared-quota draw of a single account
  // regardless of request count. A generous default (tunable per launch); the
  // point is that it EXISTS and is enforceable, not the exact number.
  maxTokens: 2_000_000,
  windowSeconds: AgentRateLimitPolicy.windowSeconds,
}

// What the account has already consumed in the CURRENT window. The Worker reads
// this from a per-account window counter (D1/KV) keyed by account + window
// bucket; tests inject it directly. This module never owns the counter store —
// it owns the DECISION given the counts.
export type FairShareUsage = Readonly<{
  requestsInWindow: number
  tokensInWindow: number
}>

export const FairShareDecisionStatus = S.Literals([
  'allow',
  'request_rate_exceeded',
  'token_fair_share_exceeded',
])
export type FairShareDecisionStatus = typeof FairShareDecisionStatus.Type

export type FairShareDecision = Readonly<{
  status: FairShareDecisionStatus
  allowed: boolean
  // Bounded, public-safe counters echoed back so the route can set
  // RateLimit-* headers and the body can explain the rejection without leaking
  // anything sensitive.
  limit: number
  remainingRequests: number
  remainingTokens: number
  windowSeconds: number
  // HTTP status the route should map to (200 allow, 429 exceeded).
  statusCode: number
  reasonRef: string
}>

// Pure fair-share decision. Bounds BOTH requests/window and tokens/window for a
// single account so one customer cannot starve the shared pool. Request-rate is
// checked first (cheapest signal), then the token fair-share. Allowing the
// current request requires it to fit UNDER the ceiling (a request that would
// land exactly AT the request ceiling is the last allowed one).
export const decideFairShare = (
  input: Readonly<{
    usage: FairShareUsage
    limits?: FairShareLimits
  }>,
): FairShareDecision => {
  const limits = input.limits ?? DEFAULT_FAIR_SHARE_LIMITS
  const requests = Math.max(0, Math.trunc(input.usage.requestsInWindow))
  const tokens = Math.max(0, Math.trunc(input.usage.tokensInWindow))
  const remainingRequests = Math.max(0, limits.maxRequests - requests)
  const remainingTokens = Math.max(0, limits.maxTokens - tokens)

  // Request-rate first: the account has already issued >= the request ceiling in
  // this window, so this request would exceed it.
  if (requests >= limits.maxRequests) {
    return {
      allowed: false,
      limit: limits.maxRequests,
      reasonRef: 'reason.inference_abuse.request_rate_exceeded',
      remainingRequests: 0,
      remainingTokens,
      status: 'request_rate_exceeded',
      statusCode: 429,
      windowSeconds: limits.windowSeconds,
    }
  }

  // Token fair-share: the account has already drawn >= the token ceiling for the
  // shared pool this window.
  if (tokens >= limits.maxTokens) {
    return {
      allowed: false,
      limit: limits.maxRequests,
      reasonRef: 'reason.inference_abuse.token_fair_share_exceeded',
      remainingRequests,
      remainingTokens: 0,
      status: 'token_fair_share_exceeded',
      statusCode: 429,
      windowSeconds: limits.windowSeconds,
    }
  }

  return {
    allowed: true,
    limit: limits.maxRequests,
    reasonRef: 'reason.inference_abuse.allow',
    remainingRequests,
    remainingTokens,
    status: 'allow',
    statusCode: 200,
    windowSeconds: limits.windowSeconds,
  }
}

// ----------------------------------------------------------------------------
// 2. Spend caps (per-account, per-window) — distinct from the raw balance gate
// ----------------------------------------------------------------------------

// A configurable per-account spend ceiling for one window, in msat (the gateway
// ledger denomination). This is DISTINCT from the route's read-only balance
// gate: an account can be flush with credits yet still be capped at, say, $X/day
// of inference so a compromised key cannot drain the whole balance in minutes.
// `null` ceiling => no cap configured for the account (the default; gate open).
export type SpendCap = Readonly<{
  maxSpendMsatPerWindow: number | null
  windowSeconds: number
}>

export const DEFAULT_SPEND_CAP: SpendCap = {
  // No cap by default => INERT. The Worker supplies a real per-account cap when
  // the account (or owner) configures one.
  maxSpendMsatPerWindow: null,
  windowSeconds: AgentRateLimitPolicy.windowSeconds,
}

export const SpendCapDecisionStatus = S.Literals([
  'allow',
  'no_cap_configured',
  'spend_cap_exceeded',
])
export type SpendCapDecisionStatus = typeof SpendCapDecisionStatus.Type

export type SpendCapDecision = Readonly<{
  status: SpendCapDecisionStatus
  allowed: boolean
  capMsat: number | null
  spentMsatInWindow: number
  // The estimated incremental spend that was checked against the cap (0 at the
  // pre-flight gate when no estimate is available; the priced charge post-hoc).
  estimatedChargeMsat: number
  remainingMsat: number | null
  windowSeconds: number
  statusCode: number
  reasonRef: string
}>

// Pure spend-cap decision. When a cap is configured, the request is rejected if
// the already-spent window total PLUS the (optional) estimated charge for THIS
// request would exceed the cap. With no estimate (the pre-flight gate, before a
// price exists) it checks the already-spent total against the cap so a customer
// that has already blown the cap is stopped before incurring more cost.
export const decideSpendCap = (
  input: Readonly<{
    cap: SpendCap
    spentMsatInWindow: number
    estimatedChargeMsat?: number
  }>,
): SpendCapDecision => {
  const cap = input.cap.maxSpendMsatPerWindow
  const windowSeconds = input.cap.windowSeconds
  const spent = Math.max(0, Math.trunc(input.spentMsatInWindow))
  const estimate = Math.max(0, Math.trunc(input.estimatedChargeMsat ?? 0))

  if (cap === null) {
    return {
      allowed: true,
      capMsat: null,
      estimatedChargeMsat: estimate,
      reasonRef: 'reason.inference_abuse.no_spend_cap',
      remainingMsat: null,
      spentMsatInWindow: spent,
      status: 'no_cap_configured',
      statusCode: 200,
      windowSeconds,
    }
  }

  const projected = spent + estimate
  const remainingMsat = Math.max(0, cap - spent)
  if (projected > cap) {
    return {
      allowed: false,
      capMsat: cap,
      estimatedChargeMsat: estimate,
      reasonRef: 'reason.inference_abuse.spend_cap_exceeded',
      remainingMsat,
      spentMsatInWindow: spent,
      status: 'spend_cap_exceeded',
      statusCode: 402,
      windowSeconds,
    }
  }

  return {
    allowed: true,
    capMsat: cap,
    estimatedChargeMsat: estimate,
    reasonRef: 'reason.inference_abuse.under_spend_cap',
    remainingMsat,
    spentMsatInWindow: spent,
    status: 'allow',
    statusCode: 200,
    windowSeconds,
  }
}

// ----------------------------------------------------------------------------
// 3. Light KYC for fiat (card) credit top-up
// ----------------------------------------------------------------------------

// Identity-verification level for an account. Reuses the EXISTING public
// identity surface concept (the X owner-claim flow, `agent-owner-claim-routes.ts`,
// whose approved claim is the only identity proof the Worker holds today):
//   - none      : no verified public identity claim on file
//   - light     : a verified public identity claim (e.g. an approved X owner
//                 claim) — the "light KYC" bar for fiat top-up
// We deliberately do NOT model a heavyweight document-KYC tier here; the gate is
// "light KYC" only, and the Worker maps an approved owner claim to `light`.
export const IdentityVerificationLevel = S.Literals(['none', 'light'])
export type IdentityVerificationLevel =
  typeof IdentityVerificationLevel.Type

// Funding rail of a credit TOP-UP (purchase), distinct from the per-request
// `FundingKind`: this is the rail the customer is buying credits THROUGH.
// Bitcoin top-up is self-custodial / non-chargebackable and is NEVER KYC-gated
// here (doc §7 "light KYC for fiat" only). Card (fiat) top-up above a threshold
// requires light KYC.
export const TopUpRail = S.Literals(['card', 'bitcoin'])
export type TopUpRail = typeof TopUpRail.Type

// Light-KYC threshold for fiat top-up. A card purchase whose cumulative funded
// amount (this purchase + already-funded-this-window) reaches/exceeds the
// threshold requires `light` identity verification. Below the threshold, and for
// ALL Bitcoin top-ups, the gate is open. Default threshold is a tunable config
// knob; the point is the gate exists and Bitcoin is exempt.
export const DEFAULT_FIAT_KYC_THRESHOLD_CENTS = 20_000 as const // $200

export const FiatKycGateStatus = S.Literals([
  'allow',
  'bitcoin_exempt',
  'under_threshold',
  'kyc_required',
])
export type FiatKycGateStatus = typeof FiatKycGateStatus.Type

export type FiatKycGateDecision = Readonly<{
  status: FiatKycGateStatus
  allowed: boolean
  rail: TopUpRail
  thresholdCents: number
  cumulativeCents: number
  requiredLevel: IdentityVerificationLevel
  currentLevel: IdentityVerificationLevel
  // HTTP status the card top-up path should map to (200 allow, 403 kyc_required).
  statusCode: number
  // Next action the customer can take to clear the gate.
  nextActionRef: string
  reasonRef: string
}>

// Pure light-KYC gate for a credit top-up. Bitcoin top-ups are always exempt.
// A card top-up is allowed when EITHER the cumulative funded amount is under the
// threshold OR the account is already at `light` verification. Otherwise it is
// blocked pending light KYC. Conservative: an unknown/`none` level above the
// threshold is blocked.
export const decideFiatKycGate = (
  input: Readonly<{
    rail: TopUpRail
    // This purchase's amount in USD cents.
    purchaseCents: number
    // Already funded by card in the rolling KYC window, USD cents. The
    // cumulative is (already + this purchase), so a customer cannot dodge the
    // gate by splitting one large card purchase into many small ones.
    priorCardCentsInWindow?: number
    currentLevel: IdentityVerificationLevel
    thresholdCents?: number
  }>,
): FiatKycGateDecision => {
  const thresholdCents = Math.max(
    0,
    Math.trunc(input.thresholdCents ?? DEFAULT_FIAT_KYC_THRESHOLD_CENTS),
  )
  const purchase = Math.max(0, Math.trunc(input.purchaseCents))
  const prior = Math.max(0, Math.trunc(input.priorCardCentsInWindow ?? 0))
  const cumulativeCents = prior + purchase

  // Bitcoin top-up: never KYC-gated here (non-chargebackable rail).
  if (input.rail === 'bitcoin') {
    return {
      allowed: true,
      cumulativeCents,
      currentLevel: input.currentLevel,
      nextActionRef: 'next.inference_abuse.proceed',
      rail: 'bitcoin',
      reasonRef: 'reason.inference_abuse.bitcoin_topup_exempt',
      requiredLevel: 'none',
      status: 'bitcoin_exempt',
      statusCode: 200,
      thresholdCents,
    }
  }

  // Card under the threshold: light KYC not yet required.
  if (cumulativeCents < thresholdCents) {
    return {
      allowed: true,
      cumulativeCents,
      currentLevel: input.currentLevel,
      nextActionRef: 'next.inference_abuse.proceed',
      rail: 'card',
      reasonRef: 'reason.inference_abuse.fiat_under_kyc_threshold',
      requiredLevel: 'none',
      status: 'under_threshold',
      statusCode: 200,
      thresholdCents,
    }
  }

  // Card at/over the threshold: requires light identity verification.
  if (input.currentLevel === 'light') {
    return {
      allowed: true,
      cumulativeCents,
      currentLevel: 'light',
      nextActionRef: 'next.inference_abuse.proceed',
      rail: 'card',
      reasonRef: 'reason.inference_abuse.fiat_kyc_satisfied',
      requiredLevel: 'light',
      status: 'allow',
      statusCode: 200,
      thresholdCents,
    }
  }

  return {
    allowed: false,
    cumulativeCents,
    currentLevel: input.currentLevel,
    nextActionRef: 'next.inference_abuse.complete_light_kyc',
    rail: 'card',
    reasonRef: 'reason.inference_abuse.fiat_kyc_required',
    requiredLevel: 'light',
    status: 'kyc_required',
    statusCode: 403,
    thresholdCents,
  }
}

// ----------------------------------------------------------------------------
// 4. Abuse-signal hook + chargeback/refund credit clawback
// ----------------------------------------------------------------------------

// A typed abuse signal raised for an account. The KINDS are the doc §7 threat
// model:
//   - prompt_injection_free_compute : a request crafted to extract free compute
//                                     (e.g. metering-evasion / jailbreak-for-tokens)
//   - velocity                      : abnormal request/spend velocity for the account
//   - chargeback                    : a card payment was disputed/charged back
//   - refund                        : a card payment was refunded
// `severity` is a bounded enum (no free-form score), and `disputedMsat` carries
// the credit amount implicated by a chargeback/refund so the clawback path knows
// how much to reverse. Public-safe: refs + counts only.
export const AbuseSignalKind = S.Literals([
  'prompt_injection_free_compute',
  'velocity',
  'chargeback',
  'refund',
])
export type AbuseSignalKind = typeof AbuseSignalKind.Type

export const AbuseSignalSeverity = S.Literals(['info', 'warn', 'critical'])
export type AbuseSignalSeverity = typeof AbuseSignalSeverity.Type

export type AbuseSignal = Readonly<{
  accountRef: string
  kind: AbuseSignalKind
  severity: AbuseSignalSeverity
  // Credit amount implicated by a chargeback/refund (msat). Drives the clawback
  // amount; 0 for signals that do not move money (prompt_injection / velocity).
  disputedMsat: number
  // Stable, public-safe reference to the originating event (e.g. a Stripe
  // dispute event id ref). NEVER raw payment material.
  sourceRef: string
}>

export const AbuseResponseAction = S.Literals([
  'observe',
  'freeze_account',
  'clawback_credits',
])
export type AbuseResponseAction = typeof AbuseResponseAction.Type

// The DECISION an abuse signal produces. Pure + deterministic. Chargeback/refund
// signals with a positive disputed amount produce a `clawback_credits` action
// (reverse the implicated credits) AND freeze the account; a critical
// prompt-injection/velocity signal freezes the account (policy-enforced); lesser
// signals are observe-only. `freeze` is a policy flag the Worker enforces at the
// auth/balance gate (see honest-scope note) — the ENFORCEABLE money move is the
// clawback.
export type AbuseResponseDecision = Readonly<{
  action: AbuseResponseAction
  // True when the account should be frozen (blocked from new inference) pending
  // review. POLICY-ONLY enforcement: the Worker must consult a freeze flag at the
  // gate; this module decides WHETHER to freeze, not the enforcement mechanism.
  freeze: boolean
  // The clawback amount in msat (0 unless a chargeback/refund implicates credits).
  clawbackMsat: number
  signalKind: AbuseSignalKind
  severity: AbuseSignalSeverity
  reasonRef: string
}>

export const decideAbuseResponse = (signal: AbuseSignal): AbuseResponseDecision => {
  const disputedMsat = Math.max(0, Math.trunc(signal.disputedMsat))

  // Chargeback / refund implicating credits => claw back + freeze. The funded
  // credits were paid for by a payment that is now reversed, so the credits are
  // not owed; freezing prevents further draw while the dispute resolves.
  if (
    (signal.kind === 'chargeback' || signal.kind === 'refund') &&
    disputedMsat > 0
  ) {
    return {
      action: 'clawback_credits',
      clawbackMsat: disputedMsat,
      freeze: true,
      reasonRef: `reason.inference_abuse.${signal.kind}_clawback`,
      severity: signal.severity,
      signalKind: signal.kind,
    }
  }

  // Critical prompt-injection / velocity => freeze pending review (no money move;
  // POLICY enforcement at the gate).
  if (signal.severity === 'critical') {
    return {
      action: 'freeze_account',
      clawbackMsat: 0,
      freeze: true,
      reasonRef: `reason.inference_abuse.${signal.kind}_freeze`,
      severity: signal.severity,
      signalKind: signal.kind,
    }
  }

  // Everything else: observe only.
  return {
    action: 'observe',
    clawbackMsat: 0,
    freeze: false,
    reasonRef: `reason.inference_abuse.${signal.kind}_observe`,
    severity: signal.severity,
    signalKind: signal.kind,
  }
}

// Stable, public-safe idempotency key for a credit clawback. One key per source
// event (the dispute/refund event ref), so replaying the SAME chargeback webhook
// hits the `idempotency_key UNIQUE` constraint and is a no-op — never a double
// clawback. Neutral; contains no payment material (the sourceRef must already be
// a public-safe ref, NOT raw event JSON).
export const inferenceClawbackIdempotencyKey = (sourceRef: string): string =>
  `inference:clawback:${sourceRef}`

// Public-safe receipt ref for a clawback, resolvable without exposing the
// idempotency key, amount, or any payment material.
export const inferenceClawbackReceiptRef = (sourceRef: string): string =>
  `receipt.inference.clawback.${sourceRef}`

// Build the debit-only PayIn plan for a credit clawback: a single `adjustment`
// pay-in funded by one `in` balance leg from the account (debiting its
// `agent_balances` row, constraint-guarded). This is the SAME atomic ledger
// discipline the inference CHARGE uses (`inferenceChargePayInPlan`): one D1 batch
// = one transaction; balance moves by decrement; the `CHECK (balance_msat >= 0)`
// constraint makes an over-clawback fail the whole batch (never goes negative),
// and the `idempotency_key UNIQUE` constraint makes the clawback idempotent per
// source event (never double-claws on webhook replay).
export const inferenceClawbackPayInPlan = (
  input: Readonly<{
    sourceRef: string
    accountRef: string
    clawbackMsat: number
    contextRef: string
  }>,
): PayInPlan => ({
  contextRef: input.contextRef,
  costMsat: input.clawbackMsat,
  genesisId: null,
  idempotencyKey: inferenceClawbackIdempotencyKey(input.sourceRef),
  legs: [
    {
      amountMsat: input.clawbackMsat,
      direction: 'in',
      externalRef: 'inference_clawback',
      kind: 'balance',
      legId: `${input.sourceRef}:clawback`,
      partyRef: input.accountRef,
    },
  ],
  payInId: `inference:clawback:${input.sourceRef}`,
  payInType: 'adjustment',
  payerRef: input.accountRef,
  publicReceiptRef: inferenceClawbackReceiptRef(input.sourceRef),
  rung: null,
})

// Outcome of a clawback attempt.
export type ClawbackOutcome = Readonly<{
  // True when the clawback ledger row was written (or was already present from a
  // prior idempotent replay).
  clawedBack: boolean
  receiptRef: string
  // True when the balance CHECK aborted the decrement (the account no longer had
  // enough credit to fully reverse — e.g. it already spent the disputed credits).
  // The Worker should escalate a partial/failed clawback to manual review.
  insufficientBalance: boolean
}>

export type ClawbackDeps = Readonly<{
  db: D1Database
  nowIso?: () => string
}>

// Effect-returning clawback hook (the ONE money-moving surface here). Decrements
// the disputed credits from the account's balance through the existing
// PayIn-shaped ledger, idempotent per source event, never going negative. Mirrors
// the metering hook's settle classification: a duplicate idempotency key is a
// no-op (already clawed back); a balance CHECK abort surfaces as
// `insufficientBalance` for manual escalation. Returns `clawedBack: false` with
// `clawbackMsat <= 0` (no-op) so callers can call it unconditionally on a decision.
export const clawbackInferenceCredits = (
  input: Readonly<{
    accountRef: string
    sourceRef: string
    clawbackMsat: number
    contextRef?: string
  }>,
  deps: ClawbackDeps,
): Effect.Effect<ClawbackOutcome> =>
  Effect.gen(function* () {
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const clawbackMsat = Math.max(0, Math.trunc(input.clawbackMsat))
    const receiptRef = inferenceClawbackReceiptRef(input.sourceRef)

    if (clawbackMsat <= 0) {
      return {
        clawedBack: false,
        insufficientBalance: false,
        receiptRef,
      } satisfies ClawbackOutcome
    }

    const plan = inferenceClawbackPayInPlan({
      accountRef: input.accountRef,
      clawbackMsat,
      contextRef: input.contextRef ?? 'inference:clawback',
      sourceRef: input.sourceRef,
    })

    const settle = yield* Effect.tryPromise({
      catch: inferenceClawbackPersistenceError,
      try: () =>
        runLedgerStatements(deps.db, createPayInStatements(plan, nowIso())),
    }).pipe(
      Effect.map(() => ({ ok: true as const })),
      Effect.catch(() => Effect.succeed({ ok: false as const })),
    )

    if (settle.ok) {
      yield* Effect.logInfo(
        workerLogEntry('inference.abuse.clawback.charged', {
          accountRef: input.accountRef,
          clawbackMsat,
          sourceRef: input.sourceRef,
        }),
      )
      return {
        clawedBack: true,
        insufficientBalance: false,
        receiptRef,
      } satisfies ClawbackOutcome
    }

    // Batch failed. Re-read: an existing clawback row means an idempotent replay
    // (already clawed back) — report success, no re-claw.
    const already = yield* Effect.tryPromise({
      catch: inferenceClawbackPersistenceError,
      try: () =>
        deps.db
          .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ? LIMIT 1')
          .bind(inferenceClawbackIdempotencyKey(input.sourceRef))
          .first(),
    }).pipe(Effect.catch(() => Effect.succeed(null)))

    if (already !== null) {
      return {
        clawedBack: true,
        insufficientBalance: false,
        receiptRef,
      } satisfies ClawbackOutcome
    }

    // Genuine failure: the balance CHECK aborted (account lacked the credits to
    // fully reverse). Never goes negative; surface for manual escalation.
    yield* Effect.logInfo(
      workerLogEntry('inference.abuse.clawback.insufficient_balance', {
        accountRef: input.accountRef,
        clawbackMsat,
        sourceRef: input.sourceRef,
      }),
    )
    return {
      clawedBack: false,
      insufficientBalance: true,
      receiptRef,
    } satisfies ClawbackOutcome
  })
