// Psionic fabric serve dispatch for the OpenAgents serving-fabric lane
// (EPIC #5474, #5483; Khala M4, #6012 / EPIC #6017; design:
// docs/inference/2026-06-19-decentralized-serving-shard-wan.md §1, §3, §5, §8).
//
// This is the CONCRETE `NetworkFabricDispatch` the inert
// `openagents-network-adapter.ts` seam was built to receive. It speaks the
// product-layer view of the Psionic serve wire contract:
//
//   ask-plan -> execute -> consume an EXACT-PARITY receipt
//
// against a single admitted Pylon serving a WHOLE SMALL MODEL (the near-term,
// low-risk lane; doc §1 / §6 "Build order"). Shard-WAN large-model serving is
// DEFERRED entirely (doc §7 "Honest gaps": that path is Psionic-planned /
// partly hardware-blocked) — this module typed-refuses any serve plan that
// declares more than one stage.
//
// THE NON-NEGOTIABLE GATE (doc §3a): payment clears against a CHECKABLE
// outcome, not a self-report. Psionic's acceptance gate is EXACT-GREEDY PARITY:
// a served run must produce tokens identical to the same-engine reference
// greedy decode. This module enforces "NO PARITY -> NO SUCCESS" at the dispatch
// layer: a serve response that does not carry `exact_greedy_parity` with
// `parityVerified: true` is a typed NON-RETRYABLE failure, never a fabricated
// success. The downstream payout gate (serving-node-payout.ts GATE 1) re-checks
// parity independently; this is the first, earliest fail-closed point.
//
// BOUNDARIES (doc §5, the Psionic boundary): this is a PRODUCT-layer dispatch.
// It asks the fabric for a plan + offered posture, consumes the served result +
// receipt, and never reaches into Psionic execution, never holds money
// authority, and never prices or pays. Pricing (#5478), metering (#5477), and
// the per-stage payout split (#5484) consume the receipt downstream.
//
// TRANSPORT INJECTION: the serve transport is injectable so tests prove the
// dispatch against a LOCAL or FAKE Psionic serve, and a real Psionic
// `psionic-serve` HTTP endpoint can be wired later with no contract change. The
// live fabric is NOT wired in this repo (owner-gated, Psionic-planned); this
// module never fakes a LIVE serve — it only consumes whatever serve a wired
// transport returns.

import { parseJsonRecord, recordFromUnknown } from '../json-boundary'
import {
  makeOpenAgentsNetworkAdapter,
  type NetworkFabricDispatch,
  type NetworkServedResult,
  OPENAGENTS_NETWORK_ADAPTER_ID,
  type ServingReceipt,
  type ServingStage,
} from './openagents-network-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
  type InferenceResult,
  type InferenceUsage,
} from './provider-adapter'

import { Effect } from 'effect'

// ----------------------------------------------------------------------------
// Stable, neutral failure-classification kinds + reason refs (public-safe)
// ----------------------------------------------------------------------------

// Stable adapter id used on every typed failure from this dispatch — the same
// id the lane is registered under, so routing/metrics attribute the failure to
// the serving-fabric lane.
const FABRIC_ADAPTER_ID = OPENAGENTS_NETWORK_ADAPTER_ID

// A serve response that does not carry a verified exact-greedy parity result.
// NON-retryable: a missing/failed parity gate is not a transient fault — the
// run is structurally unpayable, so routing must overflow to the next viable
// lane (Vertex / passthrough) rather than back off against the fabric.
export const FABRIC_PARITY_UNVERIFIED_KIND = 'fabric_parity_unverified'
export const FABRIC_PARITY_UNVERIFIED_REASON =
  'fabric serve did not return a verified exact-greedy parity receipt'

// A serve plan that declares a sharded (multi-stage) run. DEFERRED this wave
// (whole-small-model only). NON-retryable: shard-WAN is not wired, so a sharded
// plan must typed-refuse and fall back, never be served unverified.
export const FABRIC_SHARDED_UNSUPPORTED_KIND = 'fabric_sharded_unsupported'
export const FABRIC_SHARDED_UNSUPPORTED_REASON =
  'shard-WAN multi-stage serving is deferred; whole-small-model only this wave'

// The serve transport returned a malformed / unparseable response. NON-retryable:
// a malformed serve is a contract fault, not a transient one; metering must
// never settle on a fabricated/estimated usage, so this fails closed.
export const FABRIC_MALFORMED_RESPONSE_KIND = 'malformed_response'

// The serve transport itself faulted (network/transport). Retryable so routing
// may overflow to the next viable lane.
export const FABRIC_TRANSPORT_ERROR_KIND = 'transport_error'

const fabricError = (
  input: Readonly<{
    reason: string
    kind: string
    retryable?: boolean | undefined
    httpStatus?: number | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: FABRIC_ADAPTER_ID,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable ?? false,
  })

// ----------------------------------------------------------------------------
// The product-layer Psionic serve wire contract
// ----------------------------------------------------------------------------

// The serve REQUEST handed to the Psionic serve transport. This is the
// product-layer "ask-plan + execute" payload: the normalized model + messages
// plus the requested verifiable posture. A wired transport serializes this onto
// the real `psionic-serve` endpoint; a local/fake serve in tests consumes it
// directly. The gateway declares it REQUIRES exact-greedy parity — the fabric
// must serve verifiably or refuse (doc §3a).
export type PsionicServeRequest = Readonly<{
  // The model artifact to serve (the gateway alias / provider-native id).
  model: string
  // The normalized chat messages, OpenAI-Chat-Completions-shaped.
  messages: ReadonlyArray<Readonly<{ role: string; content: string }>>
  // Sampling params forwarded verbatim (temperature, max_tokens, ...). The
  // serve applies only the ones it supports; greedy decode is implied for the
  // parity reference regardless of these.
  passthroughParams: Readonly<Record<string, unknown>>
  // The gateway's required verifiable posture. Pinned to exact-greedy parity:
  // the serve must run the same-engine reference greedy decode and compare, or
  // the run does not clear the payment gate.
  requireExactGreedyParity: true
}>

// The serve RESPONSE the Psionic serve transport returns. Mirrors (at the
// product layer) the typed receipt shape Psionic emits
// (`psionic.serve.pipeline_sharded_run_receipt.v1`, doc §3b): which node served
// which layer-block, plus the exact-greedy parity result that is the payment
// gate. The transport may return this as a parsed object (a local/fake serve)
// or as a JSON string (a real HTTP serve); `dispatchPsionicServe` accepts both.
export type PsionicServeResponse = Readonly<{
  // The assistant completion content for the single choice.
  content: string
  // Provider-reported finish reason ("stop" / "length" / ...).
  finishReason: string
  // Receipt-first token usage (NEVER an estimate — the metering source of
  // truth).
  usage: InferenceUsage
  // The model artifact actually served.
  servedModel: string
  // Stable, public-safe serving-run id. One per served request; used to build
  // the idempotent payout key downstream so a replay never double-pays.
  servingRunRef: string
  // The stages that participated, in pipeline order. For whole-small-model
  // serving there is EXACTLY ONE stage holding the whole model.
  stages: ReadonlyArray<ServingStage>
  // Whether the serve declares it ran the same-engine reference greedy decode
  // and compared (the parity mode). Anything other than `exact_greedy_parity`
  // fails the gate.
  parityMode: 'exact_greedy_parity' | 'none'
  // Whether the declared parity check PASSED (token-identical to the reference
  // greedy decode). Only `exact_greedy_parity` && `true` clears the gate.
  parityVerified: boolean
  // Optional full paid-traffic verification bundle. Raw parity is not enough
  // for paid Pylon routing; the admitted adapter requires parity + canary +
  // replay + payout eligibility before the network lane can clear.
  paidTrafficVerification?: ServingReceipt['paidTrafficVerification'] | undefined
}>

// The injectable serve transport — the ask-plan / execute seam to Psionic. A
// test passes a LOCAL or FAKE serve; a real wiring passes an HTTP client that
// posts to `psionic-serve`. Returns the parsed response, a JSON string, or a
// typed adapter error (a retryable transport/overload fault, or a non-retryable
// refusal). The dispatch normalizes + parity-gates whatever it returns.
export type PsionicServeTransport = (
  request: PsionicServeRequest,
) => Effect.Effect<PsionicServeResponse | string, InferenceAdapterError>

// ----------------------------------------------------------------------------
// Parse + validate a serve response (fails closed on anything malformed)
// ----------------------------------------------------------------------------

const asInteger = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

const parseUsage = (raw: unknown): InferenceUsage | undefined => {
  const record = recordFromUnknown(raw)
  if (record === undefined) return undefined
  const promptTokens = asInteger(record['promptTokens'])
  const completionTokens = asInteger(record['completionTokens'])
  const totalTokens = asInteger(record['totalTokens'])
  if (
    promptTokens === undefined ||
    completionTokens === undefined ||
    totalTokens === undefined ||
    promptTokens < 0 ||
    completionTokens < 0 ||
    totalTokens < 0
  ) {
    return undefined
  }
  const cached = asInteger(record['cachedPromptTokens'])
  return {
    completionTokens,
    promptTokens,
    totalTokens,
    ...(cached === undefined ? {} : { cachedPromptTokens: cached }),
  }
}

const parseStage = (raw: unknown): ServingStage | undefined => {
  const record = recordFromUnknown(raw)
  if (record === undefined) return undefined
  const nodeRef = record['nodeRef']
  const layerStart = asInteger(record['layerStart'])
  const layerEnd = asInteger(record['layerEnd'])
  const role = record['role']
  if (
    typeof nodeRef !== 'string' ||
    nodeRef.trim() === '' ||
    layerStart === undefined ||
    layerEnd === undefined ||
    (role !== 'stage' && role !== 'coordinator' && role !== 'draft')
  ) {
    return undefined
  }
  return { layerEnd, layerStart, nodeRef, role }
}

const parseStages = (raw: unknown): ReadonlyArray<ServingStage> | undefined => {
  if (!Array.isArray(raw) || raw.length === 0) return undefined
  const parsed = raw.map(parseStage)
  if (parsed.some(stage => stage === undefined)) return undefined
  return parsed as ReadonlyArray<ServingStage>
}

const parsePaidTrafficVerification = (
  raw: unknown,
): ServingReceipt['paidTrafficVerification'] | undefined => {
  if (raw === undefined) return undefined
  const record = recordFromUnknown(raw)
  if (record === undefined) return undefined
  const parityPassed = record['parityPassed']
  const canaryPassed = record['canaryPassed']
  const replayPassed = record['replayPassed']
  const payoutEligible = record['payoutEligible']
  const blockerRefs = record['blockerRefs']
  if (
    typeof parityPassed !== 'boolean' ||
    typeof canaryPassed !== 'boolean' ||
    typeof replayPassed !== 'boolean' ||
    typeof payoutEligible !== 'boolean' ||
    !Array.isArray(blockerRefs) ||
    blockerRefs.some(ref => typeof ref !== 'string')
  ) {
    return undefined
  }
  return {
    blockerRefs: blockerRefs as ReadonlyArray<string>,
    canaryPassed,
    parityPassed,
    payoutEligible,
    replayPassed,
  }
}

// Normalize whatever the transport returned (object or JSON string) into a
// validated `PsionicServeResponse`, or undefined when it is malformed. Fails
// closed: any missing/ill-typed load-bearing field yields undefined so the
// caller raises a typed malformed-response failure rather than serving garbage.
const normalizeServeResponse = (
  raw: PsionicServeResponse | string,
): PsionicServeResponse | undefined => {
  const record =
    typeof raw === 'string' ? parseJsonRecord(raw) : recordFromUnknown(raw)
  if (record === undefined) return undefined

  const content = record['content']
  const finishReason = record['finishReason']
  const servedModel = record['servedModel']
  const servingRunRef = record['servingRunRef']
  const parityMode = record['parityMode']
  const parityVerified = record['parityVerified']
  const usage = parseUsage(record['usage'])
  const stages = parseStages(record['stages'])
  const paidTrafficVerification = parsePaidTrafficVerification(
    record['paidTrafficVerification'],
  )

  if (
    typeof content !== 'string' ||
    typeof finishReason !== 'string' ||
    finishReason.trim() === '' ||
    typeof servedModel !== 'string' ||
    servedModel.trim() === '' ||
    typeof servingRunRef !== 'string' ||
    servingRunRef.trim() === '' ||
    (parityMode !== 'exact_greedy_parity' && parityMode !== 'none') ||
    typeof parityVerified !== 'boolean' ||
    usage === undefined ||
    stages === undefined
  ) {
    return undefined
  }

  return {
    content,
    finishReason,
    parityMode,
    parityVerified,
    ...(paidTrafficVerification === undefined
      ? {}
      : { paidTrafficVerification }),
    servedModel,
    servingRunRef,
    stages,
    usage,
  }
}

// ----------------------------------------------------------------------------
// The dispatch (ask-plan -> execute -> consume exact-parity receipt)
// ----------------------------------------------------------------------------

export type PsionicFabricServeConfig = Readonly<{
  // The injectable serve transport (local/fake in tests, HTTP in a wiring).
  transport: PsionicServeTransport
}>

// Build the `NetworkServedResult` (completion result + serving receipt) from a
// validated, parity-passing serve response.
const toServedResult = (serve: PsionicServeResponse): NetworkServedResult => {
  const result: InferenceResult = {
    content: serve.content,
    finishReason: serve.finishReason,
    servedModel: serve.servedModel,
    usage: serve.usage,
  }
  const receipt: ServingReceipt = {
    parityMode: serve.parityMode,
    parityVerified: serve.parityVerified,
    ...(serve.paidTrafficVerification === undefined
      ? {}
      : { paidTrafficVerification: serve.paidTrafficVerification }),
    servedModel: serve.servedModel,
    // Whole-small-model: exactly one stage => not sharded. Carried explicitly
    // for legibility (doc §3b); >1 stage is refused before we ever get here.
    sharded: serve.stages.length > 1,
    servingRunRef: serve.servingRunRef,
    stages: serve.stages,
  }
  return { receipt, result }
}

// The concrete `NetworkFabricDispatch`. Asks the transport to serve the request
// with the exact-greedy-parity posture, then:
//   1. surfaces a transport refusal/fault verbatim (already typed),
//   2. fails closed on a malformed serve (NON-retryable malformed_response),
//   3. REFUSES a sharded multi-stage plan (deferred this wave; NON-retryable),
//   4. enforces NO PARITY -> NO SUCCESS (NON-retryable parity-unverified), and
//   5. only then returns the served result + receipt.
export const dispatchPsionicServe =
  (config: PsionicFabricServeConfig): NetworkFabricDispatch =>
  (request: InferenceRequest) =>
    Effect.gen(function* () {
      const raw = yield* config.transport({
        messages: request.messages,
        model: request.model,
        passthroughParams: request.passthroughParams,
        requireExactGreedyParity: true,
      })

      const serve = normalizeServeResponse(raw)
      if (serve === undefined) {
        return yield* Effect.fail(
          fabricError({
            kind: FABRIC_MALFORMED_RESPONSE_KIND,
            reason: 'fabric serve returned a malformed serve response',
            retryable: false,
          }),
        )
      }

      // Shard-WAN is deferred: whole-small-model only this wave. A multi-stage
      // plan is refused before any parity/result handling.
      if (serve.stages.length > 1) {
        return yield* Effect.fail(
          fabricError({
            kind: FABRIC_SHARDED_UNSUPPORTED_KIND,
            reason: FABRIC_SHARDED_UNSUPPORTED_REASON,
            retryable: false,
          }),
        )
      }

      // THE GATE: no verified exact-greedy parity -> no success. This is the
      // earliest fail-closed point; serving-node-payout.ts re-checks parity
      // independently before any payout.
      if (
        !(serve.parityMode === 'exact_greedy_parity' && serve.parityVerified)
      ) {
        return yield* Effect.fail(
          fabricError({
            kind: FABRIC_PARITY_UNVERIFIED_KIND,
            reason: FABRIC_PARITY_UNVERIFIED_REASON,
            retryable: false,
          }),
        )
      }

      return toServedResult(serve)
    })

// Build the serving-fabric adapter wired to a Psionic serve transport. This is
// the registration-ready factory: a wiring passes a configured transport (HTTP
// to `psionic-serve` once owner-gated), and tests pass a local/fake serve. The
// returned adapter is the `openagents-network` lane adapter with a LIVE dispatch
// (vs the inert default `openAgentsNetworkAdapter`).
export const makePsionicFabricAdapter = (config: PsionicFabricServeConfig) =>
  // Re-uses the adapter shape from openagents-network-adapter via its public
  // factory so the stream/complete mapping + receipt wiring stay in one place.
  // This module owns the concrete parity-gated dispatch; the network adapter
  // owns the InferenceProviderAdapter shape.
  makeOpenAgentsNetworkAdapter({ dispatch: dispatchPsionicServe(config) })
