// Metering-hook seam for the inference gateway (EPIC #5474, #5477).
//
// This is the single typed point where #5477 (credits, metering & billing)
// decrements credits from the provider `usage` object — receipt-first, never an
// estimate (gateway business doc §4; INVARIANTS.md "Canonical Token Usage
// Ledger"). #5476 shipped a no-op/log stub so the route worked end-to-end and
// the seam had a stable shape; #5477 ships the live ledger implementation here
// (`makeLedgerMeteringHook`) and leaves the stub in place for tests and for the
// inert (flag-off) path.
//
// The hook receives the authenticated account, the served model + adapter, the
// funding kind, and the real provider usage AFTER a completion finishes (or, for
// streams, after the terminal usage frame arrives). It computes the charge with
// the pure pricing engine (`priceRequest`, #5478) and decrements the account's
// credit balance through the existing PayIn-shaped credit ledger
// (`payments-ledger.ts`, `agent_balances`). It moves money ONLY through that
// ledger: balances change by atomic increment/decrement, the
// `CHECK (balance_msat >= 0)` constraint makes an over-charge fail the whole D1
// batch (never goes negative), and the `idempotency_key UNIQUE` constraint makes
// the decrement idempotent per request (never double-charges on retry/replay).
//
// Returning a typed receipt-ref keeps the route's response honest about whether
// metering is live (stub => `metered: false`; live => `metered: true` + a
// public-safe receipt ref). The hook NEVER surfaces raw amounts, destinations,
// or payment material — only public-safe refs and token counts (observability).
import { Effect } from 'effect'

import {
  type MdkPayoutModeGateProjection,
  hostedMdkDirectPayoutDisabledGate,
} from '../mdk-payout-mode-gate'
import { workerLogEntry } from '../observability'
import {
  type PayInPlan,
  createPayInStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from '../payments-ledger'
import { currentIsoTimestamp } from '../runtime-primitives'
import { type ServingReceipt } from './openagents-network-adapter'
import { inferenceChargeContextRef } from './inference-charge-context'
import { type FundingKind, priceRequest } from './pricing'
import { type InferenceUsage } from './provider-adapter'
import {
  type ServingNodePayoutDecision,
  type ServingRevenueAsset,
  decideServingNodePayout,
  servingContributorCutMsat,
} from './serving-node-payout'
import { usdToMsatCeil } from './usd-msat-conversion'

class InferenceMeteringPersistenceError extends Error {
  readonly _tag = 'InferenceMeteringPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'InferenceMeteringPersistenceError'
  }
}

const inferenceMeteringPersistenceError = (error: unknown) =>
  new InferenceMeteringPersistenceError(error)

// Context handed to the metering hook when a request completes.
export type MeteringContext = Readonly<{
  // Authenticated account ref (e.g. "agent:<id>"), the principal whose balance
  // #5477 decrements.
  accountRef: string
  // The model alias the customer requested.
  requestedModel: string
  // The provider-native model actually served. Pricing keys on the served model
  // (the lane that actually incurred cost), falling back to the requested alias.
  servedModel: string
  // The adapter id that served the request (provider-capacity attribution).
  adapterId: string
  // Receipt-first usage from the provider response.
  usage: InferenceUsage
  // Whether the request was streamed.
  streamed: boolean
  // How the account funds its balance (card | bitcoin). Threaded from the route
  // so the Bitcoin funding discount in `priceRequest` applies. Defaults to card.
  fundingKind: FundingKind
  // Stable per-request id used to build the idempotency key so a retried/replayed
  // settle for the SAME request never double-charges. The route passes its
  // response id (one id per served completion).
  requestId: string
  // True for a batch request (Fireworks batch = −50% both directions). Optional;
  // routing/adapters set it once batch lands. Defaults false.
  batch?: boolean | undefined
  // The serving-fabric receipt (#5483), present ONLY when the OpenAgents serving
  // fabric (`openagents-network`) served this request. It is the apportionment +
  // parity proof the serving-node payout split (#5484) settles against. Absent for
  // every Vertex / Fireworks / passthrough request, and absent today because the
  // fabric lane is inert — the seam is wired-but-dormant so the money plumbing is
  // already shaped to receive serving payouts as the Psionic fabric lands.
  servingReceipt?: ServingReceipt | undefined
}>

// Outcome of a metering attempt. The stub returns `metered: false`; the live
// implementation returns `metered: true` with a real ledger receipt ref once
// credits are decremented (or `metered: true` + `zeroCharge` when usage rounded
// to a zero charge, e.g. an empty/no-token completion).
export type MeteringOutcome = Readonly<{
  metered: boolean
  // Public-safe ledger/usage receipt ref when metering is live; null for the
  // stub. Never a raw amount, destination, or payment material here.
  receiptRef: string | null
  noDebitReason?: 'caller_byok' | 'operator_exempt_or_unmetered' | undefined
  // True when the live hook ran but the priced charge rounded to zero msat (no
  // billable tokens) so no ledger row was written. Distinguishes "metered, $0"
  // from "not metered" without exposing the amount.
  zeroCharge?: boolean
}>

// The metering-hook contract. #5477 provides the live implementation.
export type MeteringHook = (
  context: MeteringContext,
) => Effect.Effect<MeteringOutcome>

// No-op stub (kept from #5476). Logs the usage it WOULD meter (public-safe:
// account ref, model, adapter, token counts only — no prompts, no payment
// material) and reports `metered: false`. Used for the inert (flag-off) path and
// as a default in tests that do not exercise the ledger.
export const stubMeteringHook: MeteringHook = (context: MeteringContext) =>
  Effect.gen(function* () {
    // Public-safe, bounded diagnostic only (token counts + refs, never prompt
    // or response content), through the redacted observability helper.
    yield* Effect.logInfo(
      workerLogEntry('inference.metering.stub', {
        accountRef: context.accountRef,
        adapterId: context.adapterId,
        completionTokens: context.usage.completionTokens,
        fundingKind: context.fundingKind,
        promptTokens: context.usage.promptTokens,
        requestedModel: context.requestedModel,
        servedModel: context.servedModel,
        streamed: context.streamed,
        totalTokens: context.usage.totalTokens,
      }),
    )
    return { metered: false, receiptRef: null } satisfies MeteringOutcome
  })

// ----------------------------------------------------------------------------
// Live ledger metering (#5477)
// ----------------------------------------------------------------------------

// USD -> msat conversion lives in the shared single-source module
// (`usd-msat-conversion.ts`, #5497) so the metering charge and the USD->msat
// credit bridge convert at the IDENTICAL rate with identical rounding. The
// pricing module stays currency-pure in USD and defers msat conversion to that
// module; the `agent_balances` ledger is msat-denominated, so a USD charge must
// land as msat to decrement the same balance the route's gate reads. Re-exported
// here so the existing metering importers/tests that read `DEFAULT_BTC_USD` /
// `usdToMsatCeil` from this module keep working unchanged.
export { DEFAULT_BTC_USD, usdToMsatCeil } from './usd-msat-conversion'

// Deps for the live ledger metering hook.
export type LedgerMeteringDeps = Readonly<{
  // The openagents.com Worker D1 database (carries `agent_balances`, `pay_ins`,
  // `pay_in_legs`). The Worker passes `openAgentsDatabase(env)`.
  db: D1Database
  // ISO timestamp source for the ledger rows. Defaults to the runtime clock.
  nowIso?: () => string
  // USD -> msat conversion. Defaults to `usdToMsatCeil` at `DEFAULT_BTC_USD`.
  // Tests inject a fixed conversion; a live oracle injects a real one.
  usdToMsat?: (chargeUsd: number, fundingKind: FundingKind) => number
  // SERVING-NODE PAYOUT SINK (#5484) — the wired-but-dormant seam. When the
  // OpenAgents serving fabric served the request (`context.servingReceipt`
  // present), the hook computes the per-stage serving-payout DECISION (pure,
  // owner-armed-gated) AFTER the customer charge settles, and forwards it here.
  // Default UNDEFINED => the decision is computed and logged but nothing is
  // dispatched (no live payout). A live wiring passes a sink that records the
  // decision's PayIn-shaped legs through the revenue-loop spine — but only when
  // the decision is `armed` (owner-armed gate). The serving fabric is inert today,
  // so no receipt flows and this never fires; the seam is shaped to receive it.
  //
  // The sink also receives the serving RECEIPT that produced the decision, so a
  // Bitcoin/Spark settlement sink (`khala-verified-work-settlement.ts`, M3 #6011)
  // can read the served model + parity result it needs to settle real sats to the
  // serving node(s) over Spark. The receipt is the same one on `context` — passed
  // explicitly so the sink contract is self-contained.
  recordServingPayout?: (
    decision: ServingNodePayoutDecision,
    receipt: ServingReceipt,
  ) => Effect.Effect<void>
  // The owner-armed payout-mode gate (mdk-payout-mode-gate.ts) the serving-payout
  // decision consults. Defaults to a DISABLED gate (no live payout) so the seam
  // is inert unless the owner arms it explicitly.
  servingPayoutGate?: MdkPayoutModeGateProjection
  // The revenue asset of the served request for the RL-3 asset-boundary check.
  // Resolved from how the account funds (bitcoin funding => bitcoin revenue).
  // Defaults to mapping `fundingKind` (bitcoin -> 'bitcoin', card -> 'usd').
  servingRevenueAsset?: (fundingKind: FundingKind) => ServingRevenueAsset
}>

// Stable, public-safe idempotency key for an inference charge. One key per served
// request id, so a retried/replayed settle (same request) hits the
// `idempotency_key UNIQUE` constraint and is a no-op decrement — never a double
// charge. Neutral, contains no payment material.
export const inferenceChargeIdempotencyKey = (requestId: string): string =>
  `inference:charge:${requestId}`

// Public-safe receipt ref for an inference charge. Resolvable without exposing
// the idempotency key, amount, destination, or any payment material.
export const inferenceChargeReceiptRef = (requestId: string): string =>
  `receipt.inference.charge.${requestId}`

export {
  inferenceChargeContextRef,
  parseInferenceChargeContextRef,
} from './inference-charge-context'

// Build the debit-only PayIn plan for an inference charge: a single `adjustment`
// pay-in funded by one `in` balance leg from the account (which debits the
// account's `agent_balances` row, constraint-guarded), with no payout legs. This
// reuses the exact atomic credit-ledger discipline the rest of the Worker uses
// (one D1 batch = one transaction; balance moves by decrement; resulting balance
// captured on the leg for audit).
export const inferenceChargePayInPlan = (
  input: Readonly<{
    requestId: string
    accountRef: string
    costMsat: number
    contextRef: string
  }>,
): PayInPlan => ({
  contextRef: input.contextRef,
  costMsat: input.costMsat,
  genesisId: null,
  idempotencyKey: inferenceChargeIdempotencyKey(input.requestId),
  legs: [
    {
      amountMsat: input.costMsat,
      direction: 'in',
      externalRef: 'inference_charge',
      kind: 'balance',
      legId: `${input.requestId}:debit`,
      partyRef: input.accountRef,
    },
  ],
  // One pay-in per served request id (the UNIQUE idempotency key already
  // guarantees uniqueness; this keeps the primary key request-stable too).
  payInId: `inference:payin:${input.requestId}`,
  payInType: 'adjustment',
  payerRef: input.accountRef,
  publicReceiptRef: inferenceChargeReceiptRef(input.requestId),
  rung: null,
})

// Live metering hook (#5477). Computes the charge from real provider usage via
// the pure pricing engine and decrements the account's credit balance through
// the existing PayIn-shaped ledger. Receipt-first, idempotent per request, and
// never goes negative (the ledger CHECK fails the batch if it would).
// Default revenue-asset resolver for the serving-payout RL-3 boundary check:
// Bitcoin-funded accounts produce Bitcoin revenue (the only asset that may fund a
// withdrawable Bitcoin serving share); card-funded accounts produce USD/credit
// revenue (no withdrawable Bitcoin share). Conservative by construction.
const defaultServingRevenueAsset = (
  fundingKind: FundingKind,
): ServingRevenueAsset => (fundingKind === 'bitcoin' ? 'bitcoin' : 'usd')

export const makeLedgerMeteringHook = (
  deps: LedgerMeteringDeps,
): MeteringHook => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const usdToMsat =
    deps.usdToMsat ?? ((chargeUsd: number) => usdToMsatCeil(chargeUsd))
  // The owner-armed serving-payout gate. Defaults DISABLED so serving payout is
  // inert unless the owner arms it (the first real dispatched payout is owner-armed).
  const servingPayoutGate =
    deps.servingPayoutGate ?? hostedMdkDirectPayoutDisabledGate()
  const servingRevenueAsset =
    deps.servingRevenueAsset ?? defaultServingRevenueAsset

  // SERVING-NODE PAYOUT (#5484). After the customer charge settles, if the
  // OpenAgents serving fabric served the request, compute the per-stage payout
  // DECISION from the receipt-first priced margin and the receipt, then forward it
  // to the (dormant-by-default) sink. PURE + owner-armed-gated: nothing dispatches
  // unless the decision is `armed` AND a sink is wired. Always logs a public-safe,
  // bounded diagnostic (refs + parity + node count only — never amounts split per
  // node, never payment material). Returns void; never fails the charge.
  const settleServingPayout = (
    context: MeteringContext,
    priced: ReturnType<typeof priceRequest>,
  ): Effect.Effect<void> =>
    Effect.gen(function* () {
      const receipt = context.servingReceipt
      if (receipt === undefined) {
        return
      }
      // Contributor cut from the request MARGIN (gross sell − our cost), converted
      // to msat with the same conversion the charge uses, then the published
      // serving-fabric contributor share.
      const marginMsat = usdToMsat(
        Math.max(0, priced.grossChargeUsd - priced.costUsd),
        context.fundingKind,
      )
      const decision = decideServingNodePayout({
        contributorCutMsat: servingContributorCutMsat(marginMsat),
        payoutGate: servingPayoutGate,
        receipt,
        revenueAsset: servingRevenueAsset(context.fundingKind),
      })
      yield* Effect.logInfo(
        workerLogEntry('inference.serving_payout.decided', {
          adapterId: context.adapterId,
          armed: decision.armed,
          blockerRefs: decision.blockerRefs.join(','),
          requestId: context.requestId,
          servingRunRef: decision.servingRunRef,
          stageCount: receipt.stages.length,
          totalMsat: decision.split.totalMsat,
        }),
      )
      // Forward to the dormant-by-default sink ONLY when armed. The sink owns the
      // actual ledger write/dispatch through the revenue-loop spine; the default
      // (undefined) path dispatches nothing — no live payout.
      if (decision.armed && deps.recordServingPayout !== undefined) {
        yield* deps.recordServingPayout(decision, receipt)
      }
    })

  return (context: MeteringContext) =>
    Effect.gen(function* () {
      // Receipt-first price from the REAL provider usage (never an estimate),
      // keyed on the served model so the lane that incurred cost is the lane we
      // price. Funding kind applies the Bitcoin discount when present.
      const priced = priceRequest({
        batch: context.batch ?? false,
        fundingKind: context.fundingKind,
        model: context.servedModel,
        usage: context.usage,
      })

      const costMsat = usdToMsat(priced.chargeUsd, context.fundingKind)
      const receiptRef = inferenceChargeReceiptRef(context.requestId)

      // No billable tokens => zero charge => no ledger row (cost_msat must be
      // > 0). Still "metered" (we priced it); just $0.
      if (costMsat <= 0) {
        yield* Effect.logInfo(
          workerLogEntry('inference.metering.zero', {
            accountRef: context.accountRef,
            adapterId: context.adapterId,
            fundingKind: context.fundingKind,
            requestId: context.requestId,
            servedModel: priced.model,
            totalTokens: context.usage.totalTokens,
          }),
        )
        return {
          metered: true,
          receiptRef,
          zeroCharge: true,
        } satisfies MeteringOutcome
      }

      const plan = inferenceChargePayInPlan({
        accountRef: context.accountRef,
        contextRef: inferenceChargeContextRef({
          adapterId: context.adapterId,
          requestedModel: context.requestedModel,
          servedModel: priced.model,
          totalTokens: context.usage.totalTokens,
        }),
        costMsat,
        requestId: context.requestId,
      })

      // Decrement through the existing ledger. Two guarded outcomes are EXPECTED
      // and not errors:
      //   - duplicate idempotency key (UNIQUE) => the same request already
      //     settled; treat as already-metered (idempotent no-op, no re-charge).
      //   - balance CHECK abort => the account lacked funds at settle time; the
      //     pre-gate should have caught it, but we never silently go negative.
      // Both surface as a caught batch failure; we classify by re-reading whether
      // the charge row already exists.
      const settledAt = nowIso()
      const settle = yield* Effect.tryPromise({
        catch: inferenceMeteringPersistenceError,
        try: () =>
          runLedgerStatements(deps.db, [
            ...createPayInStatements(plan, settledAt),
            // Inference charges have no external forwarding leg; a successful
            // balance debit is the settlement event the public receipt proves.
            ...markPayInPaidStatements(
              { balancePayoutLegs: [], payInId: plan.payInId },
              settledAt,
            ),
          ]),
      }).pipe(
        Effect.map(() => ({ ok: true as const })),
        Effect.catch(() => Effect.succeed({ ok: false as const })),
      )

      if (settle.ok) {
        yield* Effect.logInfo(
          workerLogEntry('inference.metering.charged', {
            accountRef: context.accountRef,
            adapterId: context.adapterId,
            costMsat,
            fundingKind: context.fundingKind,
            requestId: context.requestId,
            servedModel: priced.model,
            streamed: context.streamed,
            totalTokens: context.usage.totalTokens,
          }),
        )
        // Serving-node payout (#5484): compute + (dormant) forward the per-stage
        // split now that the customer charge has settled. Inert unless the fabric
        // served this request AND the owner has armed live payout.
        yield* settleServingPayout(context, priced)
        return { metered: true, receiptRef } satisfies MeteringOutcome
      }

      // The batch failed. Re-read: if the charge row already exists, this was an
      // idempotent duplicate (already charged) — report metered, no re-charge.
      const already = yield* Effect.tryPromise({
        catch: inferenceMeteringPersistenceError,
        try: () =>
          deps.db
            .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ? LIMIT 1')
            .bind(inferenceChargeIdempotencyKey(context.requestId))
            .first(),
      }).pipe(Effect.catch(() => Effect.succeed(null)))

      if (already !== null) {
        return { metered: true, receiptRef } satisfies MeteringOutcome
      }

      // Otherwise the decrement genuinely failed (e.g. balance CHECK abort: the
      // account could not cover the charge). We never go negative; report not
      // metered (public-safe diagnostic, no amount/destination/payment material).
      yield* Effect.logInfo(
        workerLogEntry('inference.metering.failed', {
          accountRef: context.accountRef,
          adapterId: context.adapterId,
          fundingKind: context.fundingKind,
          requestId: context.requestId,
          servedModel: priced.model,
        }),
      )
      return { metered: false, receiptRef: null } satisfies MeteringOutcome
    })
}
