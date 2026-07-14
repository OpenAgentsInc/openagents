// VP1 inference usage measurement.
//
// The gateway retains receipt-first provider token evidence, but VP1 has no
// charge, credit-debit, or serving-payout authority. The old constructor and
// dependency names remain as migration-compatible seams; none of those
// dependencies are consulted and no ledger mutation is possible here.
import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import { type ServingReceipt } from './openagents-network-adapter'
import { type FundingKind } from './pricing'
import { type InferenceUsage } from './provider-adapter'

export type MeteringContext = Readonly<{
  accountRef: string
  requestedModel: string
  servedModel: string
  adapterId: string
  usage: InferenceUsage
  streamed: boolean
  fundingKind: FundingKind
  requestId: string
  batch?: boolean | undefined
  servingReceipt?: ServingReceipt | undefined
}>

export type MeteringOutcome = Readonly<{
  byok?: boolean | undefined
  metered: false
  receiptRef: null
  zeroCharge?: boolean | undefined
  failureReason?:
    | 'insufficient_credit'
    | 'metering_storage_failed'
    | undefined
  paymentMode: 'no-spend'
  payoutClaimAllowed: false
  settlementState: 'not_applicable'
}>

export type MeteringHook = (
  context: MeteringContext,
) => Effect.Effect<MeteringOutcome>

const noSpendOutcome = (): MeteringOutcome => ({
  metered: false,
  paymentMode: 'no-spend',
  payoutClaimAllowed: false,
  receiptRef: null,
  settlementState: 'not_applicable',
})

const measureUsage = (context: MeteringContext) =>
  Effect.gen(function* () {
    yield* Effect.logInfo(
      workerLogEntry('inference.usage.measured_no_spend', {
        accountRef: context.accountRef,
        adapterId: context.adapterId,
        completionTokens: context.usage.completionTokens,
        promptTokens: context.usage.promptTokens,
        requestedModel: context.requestedModel,
        requestId: context.requestId,
        servedModel: context.servedModel,
        streamed: context.streamed,
        totalTokens: context.usage.totalTokens,
      }),
    )
    return noSpendOutcome()
  })

export const stubMeteringHook: MeteringHook = measureUsage

// Compatibility-only input. Keeping this permissive lets deployment wiring be
// deleted in a later graph slice without preserving any payment capability.
export type LedgerMeteringDeps = Readonly<{
  ledgerDb?: unknown
  nowIso?: (() => string) | undefined
  usdToMsat?: ((chargeUsd: number, fundingKind: FundingKind) => number) | undefined
  recordServingPayout?: unknown
  servingPayoutGate?: unknown
  servingRevenueAsset?: unknown
  recordCreditBalanceProjection?: unknown
}>

export const makeLedgerMeteringHook = (
  _deps: LedgerMeteringDeps,
): MeteringHook => measureUsage

// Historical/read-only reference helpers remain stable so stored receipts and
// context refs can still be dereferenced. They do not create a charge.
export const inferenceChargeIdempotencyKey = (requestId: string): string =>
  `inference:charge:${requestId}`

export const inferenceChargeReceiptRef = (requestId: string): string =>
  `receipt.inference.charge.${requestId}`

export {
  inferenceChargeContextRef,
  parseInferenceChargeContextRef,
} from './inference-charge-context'

export { DEFAULT_BTC_USD, usdToMsatCeil } from './usd-msat-conversion'
