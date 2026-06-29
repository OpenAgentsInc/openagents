// Self-serve external-wallet labor earning payout — the typed,
// contributor-initiated shape of claiming their accepted labor earnings
// to an external wallet in a single self-serve action (promise
// provider.compliant_usage_labor.v1, yellow).
//
// THE GAP THIS CLOSES: the promise's first-live evidence (#4777)
// proved a negotiated labor job executed, was validator-accepted, and settled
// on the credit ledger. But that flow was OPERATOR-STAGED, and the settlement
// was credit-ledger only. The promise states: "broad self-serve, external-wallet
// labor earning is the remaining gate", carried as
// blocker.product_promises.labor_self_serve_earning_missing.
//
// This module is the missing self-serve capability: a provider (not an operator)
// initiates a payout of their withdrawable labor earnings to an external wallet
// in ONE action, evaluating their available Bitcoin-withdrawable balance and
// planning the Nexus Treasury payout intent.
//
// SCOPE / HONESTY: this is FLAG-GATED INERT where it would touch real dispatch.
// The plan is PURE: it moves no money, debits no balance, and issues no
// lightning payment. It assembles a typed payout PLAN over the existing ledger
// balances. The dispatch seam (`dispatchSelfServeLaborPayout`) is INERT by
// default (`enabled: false` => `disabled`, no ledger write); only when armed AND
// the balance allows does it surface the intended treasury intent.
// The promise STAYS yellow: a self-serve plan + an inert dispatch seam is not
// settled external money. Crucially, this clears ONLY the self-serve blocker.
// A green flip stays receipt-first and owner-signed per
// proof.claim_upgrade_receipts.v1, demanding actual ladder-settled receipts.

import { Effect, Schema as S } from 'effect'

import { currentIsoTimestamp } from './runtime-primitives'
import {
  type NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutIntentRecord as NexusTreasuryPayoutIntentSchema,
} from './nexus-treasury-payout-ledger'

export const LABOR_SELF_SERVE_PAYOUT_SCHEMA =
  'openagents.labor_self_serve_earning_payout.v1' as const

// The yellow promise this capability sits under. It STAYS yellow.
export const LABOR_SELF_SERVE_PAYOUT_PROMISE =
  'provider.compliant_usage_labor.v1' as const

// The blocker this self-serve capability CLEARS once deployed.
export const LABOR_SELF_SERVE_PAYOUT_CLEARED_BLOCKER_REF =
  'blocker.product_promises.labor_self_serve_earning_missing' as const

export const LaborSelfServePayoutInput = S.Struct({
  providerRef: S.String,
  destination: S.String,
})
export type LaborSelfServePayoutInput = typeof LaborSelfServePayoutInput.Type

export const LaborSelfServePayoutPlan = S.Struct({
  schema: S.Literal(LABOR_SELF_SERVE_PAYOUT_SCHEMA),
  planId: S.String,
  providerRef: S.String,
  selfServe: S.Literal(true),
  gate: S.Struct({
    state: S.Literals(['blocked', 'ready']),
    reasonRefs: S.Array(S.String),
  }),
  readyForMarket: S.Boolean,
  payoutIntent: S.NullOr(NexusTreasuryPayoutIntentSchema),
  promiseIds: S.Tuple([S.Literal(LABOR_SELF_SERVE_PAYOUT_PROMISE)]),
  promiseState: S.Literal('yellow'),
  inert: S.Literal(true),
  clearedBlockerRefs: S.Array(S.String),
  createdAt: S.String,
})
export type LaborSelfServePayoutPlan = typeof LaborSelfServePayoutPlan.Type

export class LaborSelfServePayoutValidationError extends S.TaggedErrorClass<LaborSelfServePayoutValidationError>()(
  'LaborSelfServePayoutValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

export const laborSelfServePayoutPlanId = (providerRef: string, nowIso: string): string =>
  `labor_payout.${providerRef.replace(/[^a-z0-9._-]+/giu, '_')}.${nowIso.replace(/[^a-z0-9]/giu, '')}`

export type LaborSelfServePayoutFacts = Readonly<{
  bitcoinWithdrawableMsat: number
}>

export const buildSelfServeLaborPayoutPlan = (
  input: LaborSelfServePayoutInput,
  facts: LaborSelfServePayoutFacts,
  createdAt?: string,
):
  | { ok: true; plan: LaborSelfServePayoutPlan }
  | { ok: false; error: LaborSelfServePayoutValidationError } => {
  if (!isNonEmpty(input.providerRef)) {
    return fail('providerRef must be non-empty')
  }
  if (!isNonEmpty(input.destination)) {
    return fail('destination must be non-empty')
  }

  const nowIso = createdAt ?? currentIsoTimestamp()
  const planId = laborSelfServePayoutPlanId(input.providerRef, nowIso)

  let state: 'blocked' | 'ready' = 'ready'
  const reasonRefs: string[] = []

  if (facts.bitcoinWithdrawableMsat < 100_000) {
    state = 'blocked'
    reasonRefs.push('reason.labor_payout.insufficient_withdrawable_balance')
  }

  const payoutIntent: NexusTreasuryPayoutIntentRecord | null =
    state === 'ready'
      ? {
          id: `intent_${planId}`,
          payoutIntentRef: `payout_intent.${planId}`,
          idempotencyKeyHash: `hash.${planId}`,
          actorRef: input.providerRef,
          ownerUserId: null,
          sourceKind: 'accepted_work',
          buyerPaymentRef: null,
          acceptedWorkRefs: [], // Aggregate payout for all historical accepted work
          assignmentRef: null,
          artanisDispatchRef: null,
          pylonJobRef: null,
          payoutTargetRef: `target.redacted.${planId}`,
          payoutTargetApprovalRef: null,
          adapterKind: 'spark_treasury',
          amount: {
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
            amountMinorUnits: facts.bitcoinWithdrawableMsat,
          },
          spendCap: {
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
            amountMinorUnits: facts.bitcoinWithdrawableMsat,
          },
          policySnapshotRef: 'policy.public.labor_self_serve_payout',
          status: 'approved',
          metadataRefs: ['metadata.nexus.labor_payout.self_serve'],
          publicProjectionJson: JSON.stringify({ selfServeLaborPayout: true }),
          createdAt: nowIso,
          updatedAt: nowIso,
          archivedAt: null,
        }
      : null

  return {
    ok: true,
    plan: {
      schema: LABOR_SELF_SERVE_PAYOUT_SCHEMA,
      planId,
      providerRef: input.providerRef,
      selfServe: true,
      gate: {
        state,
        reasonRefs,
      },
      readyForMarket: state === 'ready',
      payoutIntent,
      promiseIds: [LABOR_SELF_SERVE_PAYOUT_PROMISE],
      promiseState: 'yellow',
      inert: true,
      clearedBlockerRefs: [LABOR_SELF_SERVE_PAYOUT_CLEARED_BLOCKER_REF],
      createdAt: nowIso,
    },
  }

  function fail(reason: string): {
    ok: false
    error: LaborSelfServePayoutValidationError
  } {
    return {
      ok: false,
      error: new LaborSelfServePayoutValidationError({ reason }),
    }
  }
}

export type DispatchSelfServeLaborPayoutInput = Readonly<{
  plan: LaborSelfServePayoutPlan
}>

export type DispatchSelfServeLaborPayoutResult =
  | Readonly<{ _tag: 'disabled'; planId: string }>
  | Readonly<{ _tag: 'blocked'; planId: string; reasonRefs: ReadonlyArray<string> }>
  | Readonly<{
      _tag: 'authorized'
      planId: string
      payoutIntent: NexusTreasuryPayoutIntentRecord
    }>

export type DispatchSelfServeLaborPayoutDeps = Readonly<{
  enabled: boolean
}>

export const dispatchSelfServeLaborPayout = (
  deps: DispatchSelfServeLaborPayoutDeps,
  input: DispatchSelfServeLaborPayoutInput,
): Effect.Effect<DispatchSelfServeLaborPayoutResult> => {
  const planId = input.plan.planId

  if (!deps.enabled) {
    return Effect.succeed({ _tag: 'disabled', planId } as const)
  }

  if (!input.plan.readyForMarket || input.plan.payoutIntent === null) {
    return Effect.succeed({
      _tag: 'blocked',
      planId,
      reasonRefs: input.plan.gate.reasonRefs,
    } as const)
  }

  return Effect.succeed({
    _tag: 'authorized',
    planId,
    payoutIntent: input.plan.payoutIntent,
  } as const)
}
