// Shared credit-metering seam for sellable OpenAgents Cloud primitives
// (EPIC #5510: fine-tuning #5516, sandbox compute #5517).
//
// The fine-tuning and sandbox scaffolds each shipped with a NO-OP metering stub
// (`metered: false`). This module supplies the REAL, receipt-first credit-debit
// implementation those stubs become once a primitive reports real runtime usage,
// reusing the EXACT atomic credit-ledger discipline the inference gateway uses
// (`inference/metering-hook.ts` -> `payments-ledger.ts`):
//
//   - the charge is a single `adjustment` PayIn funded by one `in` balance leg
//     from the account, debiting its `agent_balances` row;
//   - the `CHECK (balance_msat >= 0)` constraint fails the whole D1 batch on an
//     over-charge (the balance never goes negative);
//   - the `idempotency_key UNIQUE` constraint makes the decrement idempotent per
//     charge id (a retried/replayed settle never double-charges).
//
// HONEST SCOPE: this is the metering SEAM, not a live billed product. The amount
// to charge is computed by an INJECTED pure pricing function from REAL runtime
// usage (never an estimate, never a hardcoded price) — the scaffolds default to
// the no-op stub, so nothing is billed until a primitive both (a) runs a real
// job/sandbox that reports usage and (b) is wired to a live pricing function by
// its EPIC. The promises `cloud.fine_tuning_service.v1` /
// `cloud.sandbox_compute_service.v1` STAY red; a green flip still requires a
// dereferenceable PAID receipt + owner sign-off per
// `proof.claim_upgrade_receipts.v1`.

import { Effect } from 'effect'

import type { BillingDomainMirror } from '../billing'
import { workerLogEntry } from '../observability'
import {
  createPayInStatements,
  markPayInPaidStatements,
  readAgentBalance,
  type PayInPlan,
  runLedgerStatements,
} from '../payments-ledger'
import { currentIsoTimestamp } from '../runtime-primitives'
import { msatToUsdCentsRound } from '../inference/usd-msat-conversion'

class CloudMeteringPersistenceError extends Error {
  readonly _tag = 'CloudMeteringPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'CloudMeteringPersistenceError'
  }
}

const cloudMeteringPersistenceError = (error: unknown) =>
  new CloudMeteringPersistenceError(error)

// A single billable charge for a cloud-primitive unit of work (a finished
// fine-tune job, a closed sandbox rental). `chargeMsat` is the receipt-first
// amount computed by the caller's pure pricing function from REAL runtime usage;
// it MUST be a non-negative integer msat. `chargeId` is the stable per-unit id
// (job id / sandbox id) that makes the debit idempotent.
export type CloudPrimitiveCharge = Readonly<{
  // The principal whose `agent_balances` row is debited (e.g. "agent:<id>").
  accountRef: string
  // Stable per-unit id (fine-tune job id, sandbox id). One charge per id.
  chargeId: string
  // Receipt-first charge in integer msat (>= 0). Zero => priced, no ledger row.
  chargeMsat: number
  // Public-safe primitive tag for the receipt/context refs and diagnostics.
  // e.g. 'cloud.fine_tuning.job', 'cloud.sandbox_compute.rental'.
  primitive: string
  // Public-safe adapter/runtime id that produced the usage (attribution).
  adapterId: string
}>

// Stable, public-safe idempotency key for a cloud-primitive charge. One key per
// (primitive, chargeId): a retried/replayed settle for the SAME unit hits the
// `idempotency_key UNIQUE` constraint and is a no-op debit. Contains no payment
// material.
export const cloudChargeIdempotencyKey = (
  primitive: string,
  chargeId: string,
): string => `${primitive}:charge:${chargeId}`

// Public-safe receipt ref for a cloud-primitive charge, resolvable without
// exposing the idempotency key, amount, destination, or any payment material.
export const cloudChargeReceiptRef = (
  primitive: string,
  chargeId: string,
): string => `receipt.${primitive}.charge.${chargeId}`

// Build the debit-only PayIn plan for a cloud-primitive charge: a single
// `adjustment` pay-in funded by one `in` balance leg from the account, with no
// payout legs. Identical discipline to `inferenceChargePayInPlan`.
export const cloudChargePayInPlan = (
  charge: CloudPrimitiveCharge,
): PayInPlan => ({
  contextRef: `${charge.primitive}:${charge.adapterId}`,
  costMsat: charge.chargeMsat,
  genesisId: null,
  idempotencyKey: cloudChargeIdempotencyKey(charge.primitive, charge.chargeId),
  legs: [
    {
      amountMsat: charge.chargeMsat,
      direction: 'in',
      externalRef: `${charge.primitive}_charge`,
      kind: 'balance',
      legId: `${charge.chargeId}:debit`,
      partyRef: charge.accountRef,
    },
  ],
  payInId: `${charge.primitive}:payin:${charge.chargeId}`,
  payInType: 'adjustment',
  payerRef: charge.accountRef,
  publicReceiptRef: cloudChargeReceiptRef(charge.primitive, charge.chargeId),
  rung: null,
})

// Outcome of a cloud-primitive metering attempt. Mirrors the inference
// `MeteringOutcome`: `metered: false` for the stub / a genuinely failed debit;
// `metered: true` + a public-safe receipt ref once credits are decremented (or
// `metered: true` + `zeroCharge` when the priced charge rounded to zero msat).
export type CloudMeteringOutcome = Readonly<{
  metered: boolean
  receiptRef: string | null
  zeroCharge?: boolean
  failureReason?: 'insufficient_credit' | 'metering_storage_failed'
}>

export type CloudMeteringDeps = Readonly<{
  db: D1Database
  nowIso?: () => string
  /**
   * KS-8.7 (#8318/#8337): optional fail-soft Postgres mirror
   * (`billingDomainMirrorFromEnv`) for the `pay_ins`/`pay_in_legs` rows this
   * charge creates — otherwise D1-only until the next backfill sweep
   * converges them.
   */
  mirror?: BillingDomainMirror | undefined
  // Issue #8505 (Part 2): fail-soft, best-effort per-user credit-balance
  // projection into Khala Sync (`scope.user.<userId>`) — same seam as
  // `inference/metering-hook.ts`'s `recordCreditBalanceProjection`. Called
  // AFTER a FRESH D1 charge commits, with the SAME idempotency key the D1
  // charge used. Optional; a deployment without the Khala Sync binding (or a
  // test) charges exactly as before.
  recordCreditBalanceProjection?: (
    event: Readonly<{
      accountRef: string
      idempotencyKey: string
      deltaUsdCents: number
      observedAt: string
    }>,
  ) => Promise<void>
}>

// Settle a single cloud-primitive charge against the credit ledger. Receipt-
// first, idempotent per (primitive, chargeId), and never goes negative (the
// ledger CHECK fails the batch if it would). Never throws: a duplicate
// idempotency key (already charged) reports metered; a balance-CHECK abort
// reports NOT metered (public-safe diagnostic, no amount/payment material).
export const settleCloudPrimitiveCharge = (
  deps: CloudMeteringDeps,
  charge: CloudPrimitiveCharge,
): Effect.Effect<CloudMeteringOutcome> =>
  Effect.gen(function* () {
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const receiptRef = cloudChargeReceiptRef(charge.primitive, charge.chargeId)

    // No billable usage => zero charge => no ledger row (cost_msat must be > 0).
    // Still "metered" (we priced it); just $0.
    if (charge.chargeMsat <= 0) {
      yield* Effect.logInfo(
        workerLogEntry(`${charge.primitive}.metering.zero`, {
          accountRef: charge.accountRef,
          adapterId: charge.adapterId,
          chargeId: charge.chargeId,
        }),
      )
      return {
        metered: true,
        receiptRef,
        zeroCharge: true,
      } satisfies CloudMeteringOutcome
    }

    const plan = cloudChargePayInPlan(charge)

    const settledAt = nowIso()
    const settle = yield* Effect.tryPromise({
      catch: cloudMeteringPersistenceError,
      try: () =>
        runLedgerStatements(deps.db, [
          ...createPayInStatements(plan, settledAt),
          // A cloud-primitive charge is a debit-only adjustment: the balance is
          // already decremented by the funding `in` leg above, with no external
          // forwarding. Mark it `paid` in the SAME atomic batch (empty payout
          // legs) so the settled debit is the public receipt's `paid` event —
          // identical discipline to the inference metering hook. Without this the
          // row would linger `pending` and no dereferenceable paid receipt could
          // exist for the charge.
          ...markPayInPaidStatements(
            { balancePayoutLegs: [], payInId: plan.payInId },
            settledAt,
          ),
        ], deps.mirror),
    }).pipe(
      Effect.map(() => ({ ok: true as const })),
      Effect.catch(() => Effect.succeed({ ok: false as const })),
    )

    if (settle.ok) {
      yield* Effect.logInfo(
        workerLogEntry(`${charge.primitive}.metering.charged`, {
          accountRef: charge.accountRef,
          adapterId: charge.adapterId,
          chargeId: charge.chargeId,
          costMsat: charge.chargeMsat,
        }),
      )
      // Issue #8505 (Part 2): best-effort live projection of the FRESH charge
      // into scope.user.<userId>, reusing the SAME idempotency key the D1
      // charge just used. Fail-soft by contract and never blocks/reverses
      // the D1 charge above, which already committed.
      if (deps.recordCreditBalanceProjection !== undefined) {
        yield* Effect.promise(() =>
          deps
            .recordCreditBalanceProjection!({
              accountRef: charge.accountRef,
              deltaUsdCents: -msatToUsdCentsRound(charge.chargeMsat),
              idempotencyKey: cloudChargeIdempotencyKey(charge.primitive, charge.chargeId),
              observedAt: settledAt,
            })
            .catch(() => undefined),
        )
      }
      return { metered: true, receiptRef } satisfies CloudMeteringOutcome
    }

    // The batch failed. Re-read: if the charge row already exists, this was an
    // idempotent duplicate (already charged) — report metered, no re-charge.
    const already = yield* Effect.tryPromise({
      catch: cloudMeteringPersistenceError,
      try: () =>
        deps.db
          .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ? LIMIT 1')
          .bind(cloudChargeIdempotencyKey(charge.primitive, charge.chargeId))
          .first(),
    }).pipe(Effect.catch(() => Effect.succeed(null)))

    if (already !== null) {
      return { metered: true, receiptRef } satisfies CloudMeteringOutcome
    }

    // Otherwise the decrement genuinely failed (e.g. balance CHECK abort: the
    // account could not cover the charge). We never go negative; report not
    // metered (public-safe diagnostic, no amount/destination/payment material).
    const balance = yield* Effect.tryPromise({
      catch: cloudMeteringPersistenceError,
      try: () => readAgentBalance(deps.db, charge.accountRef),
    }).pipe(Effect.catch(() => Effect.succeed(null)))
    const failureReason =
      (balance?.availableMsat ?? 0) < charge.chargeMsat
        ? 'insufficient_credit'
        : 'metering_storage_failed'
    yield* Effect.logInfo(
      workerLogEntry(`${charge.primitive}.metering.failed`, {
        accountRef: charge.accountRef,
        adapterId: charge.adapterId,
        chargeId: charge.chargeId,
        failureReason,
      }),
    )
    return {
      failureReason,
      metered: false,
      receiptRef: null,
    } satisfies CloudMeteringOutcome
  })
