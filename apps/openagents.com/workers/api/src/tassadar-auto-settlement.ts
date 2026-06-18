import { Array as Arr, Effect } from 'effect'

import type { NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import {
  type TassadarRunSettlementRecords,
  type TassadarRunSettlementRequest,
  buildTassadarRunSettlement,
} from './tassadar-run-settlement'
import {
  type TassadarRealSettlementGate,
  type TassadarSettlementAdapterDecision,
  decideTassadarDailyBudget,
  resolveTassadarSettlementAdapter,
  tassadarRealSettledSatsForDay,
  tassadarRealSettlementUtcDayKey,
} from './tassadar-run-settlement-gate'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

/**
 * Hands-off AUTO-STREAMING real settlement for a Verified fixture pair
 * (openagents #5309 + #5310). When the worker -> validator verdict path
 * finalizes an `exact_trace_replay` challenge to `Verified`, BOTH legs settle
 * automatically — the per-window rate of 5 sats to the worker AND 5 sats to the
 * validator — through the proven #5232 Spark treasury rail, with NO operator
 * POST.
 *
 * The whole orchestration is:
 *  - IDEMPOTENT: each leg derives a deterministic settlement receipt ref from
 *    the challenge + party, so a retry pays AT MOST ONCE per challenge per
 *    party (the underlying dispatch path also dedupes on idempotency keys).
 *  - FAIL-SOFT: it returns a structured outcome and NEVER throws into the
 *    caller. A blocked/failed settlement must never break verification or the
 *    heartbeat. The caller fires it fire-and-forget.
 *  - REDACTION-SAFE: it only ever moves the existing public-safe builder
 *    records; no raw `spark1…` address, invoice, preimage, or wallet material
 *    enters any projection (the destination resolver keeps raw material
 *    private, exactly as the admin path does).
 *  - BOUNDED: per-payout cap + cumulative daily cap + run allowlist all apply;
 *    once the daily budget is exhausted, further legs fall back to skip.
 *
 * The validator leg SKIPS cleanly (no error) when the validator has no
 * registered Spark payout target — the destination resolver fails closed and
 * the leg reports `skipped: 'no_payout_destination'`.
 */

// Per-window rate (openagents #5309/#5310, confirmed on the Spark forum
// thread): 5 sats to the worker and 5 sats to the validator per recorded
// Verified exact_trace_replay fixture pair.
export const TassadarPerWindowWorkerRewardSats = 5
export const TassadarPerWindowValidatorRewardSats = 5

export type TassadarAutoSettlementParty = 'worker' | 'validator'

const stableRefSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 180)

/**
 * Build the deterministic settlement request for one leg of a Verified pair.
 * The idempotency ref is keyed by the challenge ref + party so the worker and
 * validator legs are distinct receipts, each pays at most once, and a retry of
 * the same Verified pair never double-pays. All refs are public-safe.
 */
export const buildTassadarAutoSettlementRequest = (
  input: Readonly<{
    amountSats: number
    challengeRef: string
    leaseRef: string
    party: TassadarAutoSettlementParty
  }>,
): TassadarRunSettlementRequest => {
  const suffix = stableRefSuffix(`${input.challengeRef}.${input.party}`)

  return {
    adapterKind: 'spark_treasury',
    amountSats: input.amountSats,
    challengeRef: input.challengeRef,
    idempotencyRef: `idempotency.tassadar.autostream.${suffix}`,
    leaseRef: input.leaseRef,
    operatorApprovalRef: `operator_approval.tassadar.autostream.${input.party}`,
    payoutTargetApprovalRef: `payout_target_approval.tassadar.autostream.${suffix}`,
    payoutTargetRef: `payout_target.tassadar.autostream.${suffix}`,
  }
}

export type TassadarAutoSettlementLegSkipReason =
  | 'daily_budget_exhausted'
  | 'gate_not_authorized'
  | 'no_payout_destination'
  | 'settlement_failed'

export type TassadarAutoSettlementLegOutcome = Readonly<{
  amountSats: number
  // Eligibility classification from the gate decision, when the real branch was
  // reached (`allowlisted` | `run_scoped_streaming`), else null.
  eligibilitySource: 'allowlisted' | 'run_scoped_streaming' | null
  party: TassadarAutoSettlementParty
  // Public-safe remaining daily budget AFTER this leg (null in
  // per-payout-only mode). For observability only.
  remainingDailyBudgetSats: number | null
  // Present when the leg did NOT settle for real. Never an error throw.
  skipped: TassadarAutoSettlementLegSkipReason | null
  // True only when a real-bitcoin settlement receipt was confirmed + persisted
  // (or already existed idempotently) for this leg.
  settled: boolean
}>

export type TassadarAutoSettlementOutcome = Readonly<{
  legs: ReadonlyArray<TassadarAutoSettlementLegOutcome>
}>

// Minimal injected surface so this module stays unit-testable and reuses the
// proven real-dispatch path rather than rebuilding it. `dispatchRealSettlement`
// is the receipt-first, idempotent Spark dispatch (the same one the admin
// settlement route uses). It returns whether a real receipt now exists; it must
// fail-soft for the caller (any thrown/failed dispatch is caught here).
export type TassadarAutoSettlementDeps<Bindings> = Readonly<{
  ledger: NexusTreasuryPayoutLedgerStore
  resolvePayoutDestination: (
    contributorRef: string,
  ) => Promise<string | undefined>
  dispatchRealSettlement: (input: {
    contributorRef: string
    settlement: TassadarRunSettlementRecords
  }) => Effect.Effect<void, unknown>
  readGate: () => TassadarRealSettlementGate
  nowIso: string
  run: TrainingRunRecord
}>

const buildLegSettlement = (
  input: Readonly<{
    amountSats: number
    challenge: TrainingVerificationChallengeRecord
    lease: TrainingWindowLeaseRecord
    nowIso: string
    party: TassadarAutoSettlementParty
    run: TrainingRunRecord
  }>,
): TassadarRunSettlementRecords =>
  buildTassadarRunSettlement({
    challenge: input.challenge,
    lease: input.lease,
    nowIso: input.nowIso,
    request: buildTassadarAutoSettlementRequest({
      amountSats: input.amountSats,
      challengeRef: input.challenge.challengeRef,
      leaseRef: input.lease.leaseRef,
      party: input.party,
    }),
    run: input.run,
  })

/**
 * Settle one leg of a Verified pair for `contributorRef`. Fail-soft and
 * idempotent. Reads the running daily total (passed in) so multiple legs in one
 * pair share the same budget window. Returns the leg outcome and the real sats
 * that were actually settled (0 when skipped) so the caller can advance the
 * running daily total for the next leg.
 */
const settleLeg = <Bindings>(
  deps: TassadarAutoSettlementDeps<Bindings>,
  input: Readonly<{
    alreadySettledTodaySats: number
    amountSats: number
    challenge: TrainingVerificationChallengeRecord
    contributorRef: string
    lease: TrainingWindowLeaseRecord
    party: TassadarAutoSettlementParty
  }>,
): Effect.Effect<
  Readonly<{ outcome: TassadarAutoSettlementLegOutcome; settledSats: number }>
> =>
  Effect.gen(function* () {
    const gate = deps.readGate()
    const decision: TassadarSettlementAdapterDecision =
      resolveTassadarSettlementAdapter({
        amountSats: input.amountSats,
        contributorRef: input.contributorRef,
        gate,
        requestedAdapterKind: 'spark_treasury',
        trainingRunRef: deps.run.trainingRunRef,
      })

    if (!decision.realAuthorized) {
      return {
        outcome: {
          amountSats: input.amountSats,
          eligibilitySource: null,
          party: input.party,
          remainingDailyBudgetSats: null,
          settled: false,
          skipped: 'gate_not_authorized',
        },
        settledSats: 0,
      }
    }

    // Cumulative daily budget (fail-closed). Once the day's real total would
    // exceed the cap, this leg falls back to skip until the UTC day resets.
    const budget = decideTassadarDailyBudget({
      alreadySettledTodaySats: input.alreadySettledTodaySats,
      amountSats: input.amountSats,
      gate,
    })

    if (!budget.authorized) {
      return {
        outcome: {
          amountSats: input.amountSats,
          eligibilitySource: decision.eligibilitySource,
          party: input.party,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          settled: false,
          skipped: 'daily_budget_exhausted',
        },
        settledSats: 0,
      }
    }

    // The validator (and any run-scoped recipient) must have a registered Spark
    // payout target. Absent target -> skip cleanly (fail-closed), no error.
    const destination = yield* Effect.tryPromise({
      catch: () => undefined,
      try: () => deps.resolvePayoutDestination(input.contributorRef),
    }).pipe(Effect.orElseSucceed(() => undefined))

    if (destination === undefined || destination.trim() === '') {
      return {
        outcome: {
          amountSats: input.amountSats,
          eligibilitySource: decision.eligibilitySource,
          party: input.party,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          settled: false,
          skipped: 'no_payout_destination',
        },
        settledSats: 0,
      }
    }

    const settlement = buildLegSettlement({
      amountSats: input.amountSats,
      challenge: input.challenge,
      lease: input.lease,
      nowIso: deps.nowIso,
      party: input.party,
      run: deps.run,
    })

    // Receipt-first idempotent dispatch. Any failure is caught here so the
    // caller (verification/heartbeat) is never broken.
    const dispatchResult = yield* deps
      .dispatchRealSettlement({
        contributorRef: input.contributorRef,
        settlement,
      })
      .pipe(
        Effect.as('settled' as const),
        Effect.orElseSucceed(() => 'failed' as const),
      )

    if (dispatchResult === 'failed') {
      return {
        outcome: {
          amountSats: input.amountSats,
          eligibilitySource: decision.eligibilitySource,
          party: input.party,
          remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
          settled: false,
          skipped: 'settlement_failed',
        },
        settledSats: 0,
      }
    }

    return {
      outcome: {
        amountSats: input.amountSats,
        eligibilitySource: decision.eligibilitySource,
        party: input.party,
        remainingDailyBudgetSats: budget.remainingDailyBudgetSats,
        settled: true,
        skipped: null,
      },
      settledSats: input.amountSats,
    }
  })

/**
 * Auto-settle BOTH legs of a Verified `exact_trace_replay` fixture pair (#5309 +
 * #5310). Fires the worker leg (5 sats to `lease.pylonRef`) then the validator
 * leg (5 sats to the validator device's registered target). Fail-soft and
 * idempotent throughout: this never fails into the caller, so the verdict route
 * stays unaffected by any settlement outcome.
 *
 * No-ops (every leg `gate_not_authorized`) when the gate is OFF — the default
 * everywhere — so streaming is inert until the owner arms the gate.
 */
export const autoSettleVerifiedPair = <Bindings>(
  deps: TassadarAutoSettlementDeps<Bindings>,
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    lease: TrainingWindowLeaseRecord
    validatorContributorRef: string | undefined
  }>,
): Effect.Effect<TassadarAutoSettlementOutcome> =>
  Effect.gen(function* () {
    // Only Verified exact_trace_replay pairs are settleable. Anything else is a
    // clean no-op (no legs).
    if (
      input.challenge.state !== 'Verified' ||
      input.challenge.verificationClass !== 'exact_trace_replay'
    ) {
      return { legs: [] }
    }

    const workerContributorRef = input.lease.pylonRef.trim()
    const utcDayKey = tassadarRealSettlementUtcDayKey(deps.nowIso)

    // Read today's already-settled real total from the receipt ledger (the
    // receipt-first source of truth for the daily budget). Fail-soft to 0 so a
    // ledger read failure never breaks verification; the per-payout cap still
    // bounds each leg.
    const receipts = yield* Effect.tryPromise({
      catch: () => [],
      try: () => deps.ledger.listPaymentAuthorityReceipts(5000),
    }).pipe(Effect.orElseSucceed(() => []))
    const dayStartSettledSats = tassadarRealSettledSatsForDay(
      receipts,
      utcDayKey,
    )

    const legInputs: ReadonlyArray<
      Readonly<{
        amountSats: number
        contributorRef: string
        party: TassadarAutoSettlementParty
      }>
    > = [
      {
        amountSats: TassadarPerWindowWorkerRewardSats,
        contributorRef: workerContributorRef,
        party: 'worker',
      },
      ...(input.validatorContributorRef !== undefined &&
      input.validatorContributorRef.trim() !== '' &&
      input.validatorContributorRef.trim() !== workerContributorRef
        ? [
            {
              amountSats: TassadarPerWindowValidatorRewardSats,
              contributorRef: input.validatorContributorRef.trim(),
              party: 'validator' as const,
            },
          ]
        : []),
    ]

    // Sequentially settle legs, threading the running daily total so the worker
    // leg consumes budget before the validator leg is checked. The legs are
    // folded as a chain of Effects (no Effect.reduce in the v4 surface).
    type Acc = Readonly<{
      legs: ReadonlyArray<TassadarAutoSettlementLegOutcome>
      runningSettledSats: number
    }>
    const settled = yield* Arr.reduce(
      legInputs,
      Effect.succeed<Acc>({
        legs: [],
        runningSettledSats: dayStartSettledSats,
      }),
      (accEffect, leg) =>
        accEffect.pipe(
          Effect.flatMap(acc =>
            settleLeg(deps, {
              alreadySettledTodaySats: acc.runningSettledSats,
              amountSats: leg.amountSats,
              challenge: input.challenge,
              contributorRef: leg.contributorRef,
              lease: input.lease,
              party: leg.party,
            }).pipe(
              Effect.map(result => ({
                legs: [...acc.legs, result.outcome],
                runningSettledSats:
                  acc.runningSettledSats + result.settledSats,
              })),
            ),
          ),
        ),
    )

    return { legs: settled.legs }
  })
