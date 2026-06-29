import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeOpenAgentsNetworkAdapter,
  OPENAGENTS_NETWORK_ADAPTER_ID,
} from './openagents-network-adapter'
import {
  dispatchPsionicServe,
  FABRIC_MALFORMED_RESPONSE_KIND,
  FABRIC_PARITY_UNVERIFIED_KIND,
  FABRIC_PARITY_UNVERIFIED_REASON,
  FABRIC_SHARDED_UNSUPPORTED_KIND,
  makePsionicFabricAdapter,
  type PsionicServeRequest,
  type PsionicServeResponse,
  type PsionicServeTransport,
} from './psionic-fabric-serve'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'

const runResult = <A>(effect: Effect.Effect<A, InferenceAdapterError>) =>
  Effect.runPromise(Effect.result(effect))

// The intended PRIMARY test worker is the guinea-pig Pylon (Khala M4, #6012):
// a live admitted node we control whose serve -> exact-parity receipt is the
// first loop we want, and whose Bitcoin/Spark payout Lane E settles. Its Spark
// receive address is payment material that lives ONLY in
// `.secrets/khala-test-payout.env` and is NEVER committed; the serving receipt
// identifies the node by a public-safe attribution ref (`ServingStage.nodeRef`),
// not by any wallet address. This ref is the stable handle the per-stage payout
// split keys on downstream.
const GUINEA_PIG_NODE_REF = 'pylon.khala-test-payout.primary'

// A small open model — the whole-small-model lane this wave serves end-to-end on
// one Pylon (shard-WAN large-model serving is deferred).
const SMALL_MODEL = 'qwen3-0p6b'

const request = (model = SMALL_MODEL): InferenceRequest => ({
  messages: [{ content: 'serve this on a whole small model', role: 'user' }],
  model,
  passthroughParams: { max_tokens: 64, temperature: 0 },
  stream: false,
})

// A LOCAL/FAKE Psionic serve: a whole-small-model serve on the guinea-pig Pylon
// that ran the same-engine reference greedy decode and PASSED exact-greedy
// parity. This is the first end-to-end loop we want to prove.
const parityPassingServe = (
  overrides: Partial<PsionicServeResponse> = {},
): PsionicServeResponse => ({
  content: 'served by the guinea-pig Pylon, parity-verified',
  finishReason: 'stop',
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: SMALL_MODEL,
  servingRunRef: 'serve.run.guinea-pig.0001',
  stages: [
    { layerEnd: 24, layerStart: 0, nodeRef: GUINEA_PIG_NODE_REF, role: 'stage' },
  ],
  usage: { completionTokens: 7, promptTokens: 9, totalTokens: 16 },
  ...overrides,
})

const fakeServe =
  (response: PsionicServeResponse | string): PsionicServeTransport =>
  () =>
    Effect.succeed(response)

describe('psionic-fabric-serve dispatch — whole-small-model + exact-parity gate', () => {
  test('serves the guinea-pig Pylon and consumes its exact-parity receipt', async () => {
    let captured: PsionicServeRequest | undefined
    const transport: PsionicServeTransport = serveRequest => {
      captured = serveRequest
      return Effect.succeed(parityPassingServe())
    }
    const dispatch = dispatchPsionicServe({ transport })

    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      // The completion result rides through receipt-first.
      expect(outcome.success.result.content).toContain('guinea-pig')
      expect(outcome.success.result.usage.totalTokens).toBe(16)
      expect(outcome.success.result.servedModel).toBe(SMALL_MODEL)
      // The serving receipt names the guinea-pig node and its (whole-model)
      // layer block — the apportionment + parity input the payout split keys on.
      expect(outcome.success.receipt.servingRunRef).toBe(
        'serve.run.guinea-pig.0001',
      )
      expect(outcome.success.receipt.sharded).toBe(false)
      expect(outcome.success.receipt.stages).toHaveLength(1)
      expect(outcome.success.receipt.stages[0]!.nodeRef).toBe(
        GUINEA_PIG_NODE_REF,
      )
      expect(outcome.success.receipt.parityMode).toBe('exact_greedy_parity')
      expect(outcome.success.receipt.parityVerified).toBe(true)
    }
    // The gateway asked the fabric for the exact-greedy-parity posture.
    expect(captured?.requireExactGreedyParity).toBe(true)
    expect(captured?.model).toBe(SMALL_MODEL)
  })

  test('NO PARITY -> NO SUCCESS: parityVerified:false typed-fails non-retryably', async () => {
    const dispatch = dispatchPsionicServe({
      transport: fakeServe(parityPassingServe({ parityVerified: false })),
    })
    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(FABRIC_PARITY_UNVERIFIED_KIND)
      expect(outcome.failure.reason).toBe(FABRIC_PARITY_UNVERIFIED_REASON)
      // NON-retryable: a structurally unpayable run overflows to the next lane.
      expect(outcome.failure.retryable).toBe(false)
      expect(outcome.failure.adapterId).toBe(OPENAGENTS_NETWORK_ADAPTER_ID)
    }
  })

  test('NO PARITY -> NO SUCCESS: parityMode:none typed-fails (served but unverified)', async () => {
    const dispatch = dispatchPsionicServe({
      transport: fakeServe(
        parityPassingServe({ parityMode: 'none', parityVerified: true }),
      ),
    })
    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(FABRIC_PARITY_UNVERIFIED_KIND)
    }
  })

  test('shard-WAN multi-stage plan is REFUSED (deferred this wave)', async () => {
    const dispatch = dispatchPsionicServe({
      transport: fakeServe(
        parityPassingServe({
          stages: [
            { layerEnd: 12, layerStart: 0, nodeRef: 'pylon.a', role: 'stage' },
            { layerEnd: 24, layerStart: 12, nodeRef: 'pylon.b', role: 'stage' },
          ],
        }),
      ),
    })
    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(FABRIC_SHARDED_UNSUPPORTED_KIND)
      expect(outcome.failure.retryable).toBe(false)
    }
  })

  test('a malformed serve response fails closed (never serves garbage)', async () => {
    const dispatch = dispatchPsionicServe({
      transport: fakeServe('{"content":"missing usage and stages"}'),
    })
    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(FABRIC_MALFORMED_RESPONSE_KIND)
      expect(outcome.failure.retryable).toBe(false)
    }
  })

  test('accepts a JSON-string serve response (real HTTP serve shape)', async () => {
    const dispatch = dispatchPsionicServe({
      transport: fakeServe(JSON.stringify(parityPassingServe())),
    })
    const outcome = await runResult(dispatch(request()))
    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      expect(outcome.success.receipt.parityVerified).toBe(true)
    }
  })

  test('a transport refusal/fault surfaces verbatim (already typed)', async () => {
    const transport: PsionicServeTransport = () =>
      Effect.fail(
        new InferenceAdapterError({
          adapterId: OPENAGENTS_NETWORK_ADAPTER_ID,
          kind: 'service_overloaded',
          reason: 'guinea-pig Pylon is shedding load',
          retryable: true,
        }),
      )
    const outcome = await runResult(dispatchPsionicServe({ transport })(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('service_overloaded')
      expect(outcome.failure.retryable).toBe(true)
    }
  })
})

describe('psionic-fabric-serve adapter — registration-ready behind the seam', () => {
  test('makePsionicFabricAdapter is the openagents-network lane adapter with a live dispatch', async () => {
    const adapter = makePsionicFabricAdapter({
      transport: fakeServe(parityPassingServe()),
    })
    expect(adapter.id).toBe(OPENAGENTS_NETWORK_ADAPTER_ID)

    // complete returns the parity-verified completion.
    const complete = await runResult(adapter.complete(request()))
    expect(complete._tag).toBe('Success')
    if (complete._tag === 'Success') {
      expect(complete.success.content).toContain('guinea-pig')
      expect(complete.success.usage.totalTokens).toBe(16)
    }

    // stream maps the served result into a content frame + terminal usage frame
    // (the receipt-first usage metering settles from).
    const stream = await runResult(adapter.stream(request()))
    expect(stream._tag).toBe('Success')
    if (stream._tag === 'Success') {
      expect(stream.success).toHaveLength(2)
      expect(stream.success[0]!.contentDelta).toContain('guinea-pig')
      expect(stream.success[1]!.usage?.totalTokens).toBe(16)
      expect(stream.success[1]!.finishReason).toBe('stop')
    }
  })

  test('dispatch preserves paid-traffic verification from a Psionic HTTP response', async () => {
    const transport: PsionicServeTransport = () =>
      Effect.succeed(
        JSON.stringify({
          ...parityPassingServe(),
          paidTrafficVerification: {
            blockerRefs: [],
            canaryPassed: true,
            parityPassed: true,
            payoutEligible: true,
            replayPassed: true,
          },
        }),
      )
    const outcome = await runResult(dispatchPsionicServe({ transport })(request()))
    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      expect(outcome.success.receipt.paidTrafficVerification).toEqual({
        blockerRefs: [],
        canaryPassed: true,
        parityPassed: true,
        payoutEligible: true,
        replayPassed: true,
      })
    }
  })

  test('the adapter enforces the parity gate through the complete() path too', async () => {
    const adapter = makePsionicFabricAdapter({
      transport: fakeServe(parityPassingServe({ parityVerified: false })),
    })
    const outcome = await runResult(adapter.complete(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(FABRIC_PARITY_UNVERIFIED_KIND)
    }
  })

  // Sanity: the inert default network adapter (no dispatch) is unchanged — it
  // still typed-fails, so registering THIS live adapter is the only thing that
  // activates the lane.
  test('the inert default network adapter still typed-fails (no fabricated serve)', async () => {
    const inert = makeOpenAgentsNetworkAdapter()
    const outcome = await runResult(inert.complete(request()))
    expect(outcome._tag).toBe('Failure')
  })
})
