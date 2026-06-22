// OpenAgents decentralized serving-fabric provider adapter for the inference
// gateway (EPIC #5474, child #5483).
//
// This is the THIRD supply lane (docs/inference/2026-06-19-decentralized-serving-
// shard-wan.md §2): our own Pylon serving fabric, alongside the Vertex quota
// (#5480) and partner passthrough (#5481) lanes. It is the ONLY lane whose margin
// can fan back to contributors, because we — not a third party — own the compute.
//
// Honest scope (the doc is explicit, §7 "Honest gaps"):
//   - The execution substrate (whole-small-model on one Pylon near-term, and
//     shard-WAN large-model serving across many Pylons) is owned by PSIONIC and is
//     mostly PLANNED / partly hardware-blocked.
//   - This adapter therefore ships as a typed, tested, INERT lane by default: with
//     no live fabric dispatch wired, every call returns a typed, NON-retryable
//     `network_dispatch_unavailable` error with the stable reason "network dispatch
//     pending Psionic fabric". It NEVER fabricates a served completion.
//   - The seam is real: `makeOpenAgentsNetworkAdapter` accepts an injectable
//     `dispatch` (the fabric ask-plan/serve/consume-receipt interface to Psionic).
//     When a dispatch is supplied (tests today; a real fabric client as Psionic's
//     phases land), the adapter returns the normalized `InferenceResult` AND a
//     typed SERVING RECEIPT (`ServingReceipt`) that names which node(s) served
//     which layer-block(s). The receipt is the dereferenceable proof the per-stage
//     payout split (#5484) settles against, and carries the exact-greedy parity
//     result that is the non-negotiable payment gate (doc §3a).
//
// Boundaries (doc §5, the Psionic boundary): this adapter is PRODUCT-layer routing
// to the fabric. It consumes a plan + receipt; it never reaches into Psionic
// execution, never holds money authority, and never prices or pays — pricing
// (#5478), metering (#5477), and payout (#5484) consume the receipt downstream.

import { Effect } from 'effect'

import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceUsage,
} from './provider-adapter'

// The canonical adapter id for the serving-fabric lane. `model-router.ts` maps
// the `openagents-network` lane to this id; index.ts registers the adapter under
// exactly this id (the inert lane is simply skipped at dispatch time when it has
// no live fabric dispatch seam wired).
export const OPENAGENTS_NETWORK_ADAPTER_ID = 'openagents-network'

// Stable, neutral failure-classification kind for the inert lane (no fabric
// dispatch wired yet). Non-retryable: routing must NOT overflow-retry the fabric
// when it is structurally unavailable — it falls through to the next viable lane
// (Vertex / passthrough) instead.
export const NETWORK_DISPATCH_UNAVAILABLE_KIND = 'network_dispatch_unavailable'
export const NETWORK_DISPATCH_PENDING_REASON =
  'network dispatch pending Psionic fabric'

// ----------------------------------------------------------------------------
// Serving receipt — the per-stage proof the payout split consumes (#5484)
// ----------------------------------------------------------------------------

// The verifiable mode a serving plan declares. Psionic's non-negotiable
// acceptance gate is EXACT-GREEDY PARITY (doc §3a): a sharded/speculative run
// must produce tokens identical to the same-engine reference greedy decode.
//   - `exact_greedy_parity`: a same-engine reference greedy decode was run and
//     compared; the run is born-verified against a checkable outcome.
//   - `none`: no parity / no feasible same-engine reference. The run can still be
//     SERVED but is flagged; payout policy (#5484) decides whether to pay against
//     weaker evidence, defaulting to PAY ONLY against a checkable outcome.
export type ServingParityMode = 'exact_greedy_parity' | 'none'

// One pipeline stage of a served request. For a whole-model Pylon this is a
// single stage holding all layers; for a shard-WAN run there are N stages, each
// holding a contiguous transformer layer-block (doc §1, §3b).
export type ServingStage = Readonly<{
  // Public-safe node id of the serving Pylon (the payout recipient party). Never
  // a wallet address or payment material — an attribution ref only.
  nodeRef: string
  // Inclusive-exclusive contiguous layer range this stage held, e.g. [0, 12).
  // For a whole-model single-node serve this spans the whole model.
  layerStart: number
  layerEnd: number
  // Distinct pipeline role for non-layer work that still earns under the plan
  // (doc §3c: coordinator token selection, draft proposal). `stage` is the
  // ordinary per-layer-block worker.
  role: 'stage' | 'coordinator' | 'draft'
}>

// The dereferenceable proof of who served what (doc §3b — mirrors the Psionic
// `psionic.serve.pipeline_sharded_run_receipt.v1` shape at the product layer).
// This is the apportionment input for the per-stage payout split (#5484).
export type ServingReceipt = Readonly<{
  // Stable serving-run id (public-safe). One receipt per served request; used to
  // build the idempotent payout key so a replayed settle never double-pays.
  servingRunRef: string
  // The model artifact actually served (provider-native id / alias).
  servedModel: string
  // Whether more than one node participated (a true shard-WAN split) vs a single
  // whole-model node. Derived from `stages` but carried explicitly for legibility.
  sharded: boolean
  // The stages that participated, in pipeline order. Always at least one.
  stages: ReadonlyArray<ServingStage>
  // The born-verified parity result (doc §3a) — the non-negotiable payment gate.
  parityMode: ServingParityMode
  // Whether the declared parity check PASSED (token-identical to the same-engine
  // reference greedy decode). Only `parityMode: 'exact_greedy_parity'` &&
  // `parityVerified: true` clears the strong payment gate in #5484.
  parityVerified: boolean
}>

// What a live fabric dispatch returns: the normalized completion result PLUS the
// serving receipt. The two are returned together so the route/metering/payout
// path always has the receipt that proves the served completion.
export type NetworkServedResult = Readonly<{
  result: InferenceResult
  receipt: ServingReceipt
}>

// The fabric dispatch seam (the ask-plan / serve / consume-receipt interface to
// Psionic, doc §5). Returns a served result + receipt, or a typed adapter error
// (a retryable transport/overload fault, or a non-retryable refusal — e.g. a
// large-model request with no hardware-backed receipt, which must typed-refuse
// and fall back to passthrough per doc §6 Phase 10).
export type NetworkFabricDispatch = (
  request: InferenceRequest,
) => Effect.Effect<NetworkServedResult, InferenceAdapterError>

export type OpenAgentsNetworkAdapterConfig = Readonly<{
  // Live fabric dispatch. Omitted today (Psionic fabric is planned) => the
  // adapter is fully INERT: every call typed-fails `network_dispatch_unavailable`
  // and the lane is skipped by routing in favor of Vertex / passthrough.
  dispatch?: NetworkFabricDispatch | undefined
}>

// Build the inert/typed-fail Effect for the no-fabric case. NON-retryable so
// routing overflows to the next viable lane rather than backing off against a
// structurally absent lane.
const dispatchUnavailable = (): Effect.Effect<never, InferenceAdapterError> =>
  Effect.fail(
    new InferenceAdapterError({
      adapterId: OPENAGENTS_NETWORK_ADAPTER_ID,
      kind: NETWORK_DISPATCH_UNAVAILABLE_KIND,
      reason: NETWORK_DISPATCH_PENDING_REASON,
      retryable: false,
    }),
  )

// Split a served result into the gateway's stream-chunk shape: one content frame
// then a terminal frame carrying the receipt-first usage (mirrors the passthrough
// adapter's stream mapping, so metering settles from real, receipt-first counts).
const toStreamChunks = (
  usage: InferenceUsage,
  content: string,
  finishReason: string,
  servedModel: string,
): ReadonlyArray<InferenceStreamChunk> => [
  { contentDelta: content },
  { contentDelta: '', finishReason, servedModel, usage },
]

// Build the serving-fabric adapter. Pure data + Effects; it touches the fabric
// only when `complete`/`stream` actually run AND a `dispatch` seam is wired, so
// registering it without a dispatch keeps it fully INERT.
export const makeOpenAgentsNetworkAdapter = (
  config: OpenAgentsNetworkAdapterConfig = {},
): InferenceProviderAdapter => ({
  id: OPENAGENTS_NETWORK_ADAPTER_ID,
  complete: (request: InferenceRequest) =>
    config.dispatch === undefined
      ? dispatchUnavailable()
      : config.dispatch(request).pipe(Effect.map(served => served.result)),
  // Streaming maps the (collected) served result into a content frame + terminal
  // usage frame. A future revision can stream true per-token deltas from the
  // coordinator without changing the contract; the receipt-first usage is what
  // metering + payout settle from either way.
  stream: (request: InferenceRequest) =>
    config.dispatch === undefined
      ? dispatchUnavailable()
      : config.dispatch(request).pipe(
          Effect.map(served =>
            toStreamChunks(
              served.result.usage,
              served.result.content,
              served.result.finishReason,
              served.result.servedModel,
            ),
          ),
        ),
})

// The INERT serving-fabric adapter (no fabric dispatch). It is NOT registered in
// index.ts today: routing's `dispatchWithOverflow` filters the lane plan to
// REGISTERED adapters, so leaving the `openagents-network` id unregistered cleanly
// skips the lane and overflows to passthrough — never a faked serve. This export
// exists so tests can assert the honest typed-fail (`network_dispatch_unavailable`)
// and so a future wiring can register `makeOpenAgentsNetworkAdapter({ dispatch })`
// to activate the lane with no routing change.
export const openAgentsNetworkAdapter: InferenceProviderAdapter =
  makeOpenAgentsNetworkAdapter()
