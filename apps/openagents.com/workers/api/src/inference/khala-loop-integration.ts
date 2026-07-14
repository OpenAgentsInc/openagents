// VP1 Khala serving integration.
//
// Serving and parity receipts remain useful evidence. Payment gates, payout
// decisions, dry-run money-shaped receipts, and settlement dispatch have been
// retired. Compatibility constructors below are deliberately no-op so stale
// wiring cannot regain spend authority.
import { Effect } from 'effect'

import { workerLogEntry } from '../observability'
import {
  type NetworkServedResult,
  type ServingReceipt,
} from './openagents-network-adapter'
import {
  type PylonAdmissionDecision,
  type PylonServingSnapshot,
  decidePylonAdmission,
} from './khala-pylon-admission'
import {
  type PsionicFabricServeConfig,
  type PsionicServeTransport,
  dispatchPsionicServe,
} from './psionic-fabric-serve'
import { type InferenceRequest } from './provider-adapter'

export type PylonServeTransport = PsionicServeTransport
export type PylonServeTransportBuilder = (input: Readonly<{
  pylonNodeRef: string
}>) => PylonServeTransport

export const fabricConfigForPylon = (
  transport: PylonServeTransport,
): PsionicFabricServeConfig => ({ transport })

export const KhalaLoopArmingEnvKey = 'OPENAGENTS_KHALA_LOOP_ARMED'
export type KhalaLoopArming = Readonly<{ loopArmed: boolean }>
export const disabledKhalaLoopArming = { loopArmed: false } as const

// VP1 cannot be armed by environment configuration.
export const readKhalaLoopArming = (
  _env: Readonly<Record<string, unknown>>,
): KhalaLoopArming => disabledKhalaLoopArming

export type KhalaSettlementRecords = Readonly<{
  amountSats?: number
  settlementReceiptRef?: string
  [key: string]: unknown
}>

export type DryRunSettlementLedger = Readonly<{
  readReceiptByRef?: unknown
  recordReceipt?: unknown
}>

export type KhalaSettlementDispatch = (input: Readonly<{
  contributorRef: string
  settlement: KhalaSettlementRecords
}>) => Effect.Effect<void, unknown>

export const makeDryRunSettlementDispatch = (
  _ledger: DryRunSettlementLedger,
): KhalaSettlementDispatch => () => Effect.void

export type KhalaLoopDispatchSelectorInput = Readonly<{
  arming?: unknown
  dryRunDispatch?: KhalaSettlementDispatch
  readGate?: unknown
  realDispatch?: KhalaSettlementDispatch
  settlementRunRef?: string
}>

// Both former branches are retired. In particular, this never calls the
// supplied real or dry-run money-shaped dispatch.
export const makeKhalaLoopSettlementDispatch = (
  _input: KhalaLoopDispatchSelectorInput,
): KhalaSettlementDispatch => () => Effect.void

export const makeKhalaLoopSettlementSink = (
  _input: Readonly<{ arming?: unknown; settlementDeps?: unknown }>,
): ((decision: unknown, receipt: ServingReceipt) => Effect.Effect<void>) =>
  () => Effect.void

export type KhalaLoopRevenueAsset = 'bitcoin' | 'usd'

export type KhalaLoopOutcome = Readonly<{
  admission: PylonAdmissionDecision | null
  admittedAndServed: boolean
  receipt: ServingReceipt | null
  served: NetworkServedResult | null
  decision: null
  settlement: null
  forwardedToSettlement: false
  paymentMode: 'no-spend'
  payoutClaimAllowed: false
  settlementState: 'not_applicable'
}>

export type KhalaLoopConfig = Readonly<{
  transport: PylonServeTransport
  arming?: unknown
  settlementDeps?: unknown
  contributorCutMsat?: number
  revenueAsset?: KhalaLoopRevenueAsset
  payoutGate?: unknown
  resaleRefs?: unknown
  admission?: Readonly<{
    snapshot: PylonServingSnapshot
    requiredCapabilityRef: string
    nowMs: number
    heartbeatTtlMs?: number | undefined
  }> | undefined
}>

const noSpendFields = {
  decision: null,
  forwardedToSettlement: false,
  paymentMode: 'no-spend',
  payoutClaimAllowed: false,
  settlement: null,
  settlementState: 'not_applicable',
} as const

export const runKhalaLoopOnce = (
  config: KhalaLoopConfig,
  request: InferenceRequest,
): Effect.Effect<KhalaLoopOutcome, unknown> =>
  Effect.gen(function* () {
    const admission =
      config.admission === undefined
        ? null
        : decidePylonAdmission({
            nowMs: config.admission.nowMs,
            requiredCapabilityRef: config.admission.requiredCapabilityRef,
            snapshot: config.admission.snapshot,
            ...(config.admission.heartbeatTtlMs === undefined
              ? {}
              : { heartbeatTtlMs: config.admission.heartbeatTtlMs }),
          })

    if (admission !== null && !admission.admitted) {
      return {
        admission,
        admittedAndServed: false,
        receipt: null,
        served: null,
        ...noSpendFields,
      }
    }

    const served = yield* dispatchPsionicServe(
      fabricConfigForPylon(config.transport),
    )(request)

    yield* Effect.logInfo(
      workerLogEntry('inference.khala_loop.served_no_spend', {
        parityVerified: served.receipt.parityVerified,
        servingRunRef: served.receipt.servingRunRef,
        stageCount: served.receipt.stages.length,
      }),
    )

    return {
      admission,
      admittedAndServed: true,
      receipt: served.receipt,
      served,
      ...noSpendFields,
    }
  })

// Compatibility helper: VP1 has no contributor payout cut.
export const servingContributorCutMsat = (_marginMsat: number): number => 0
