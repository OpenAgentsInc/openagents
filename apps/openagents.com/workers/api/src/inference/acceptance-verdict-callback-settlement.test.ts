// Khala M3 — the verdict callback fires accepted-outcome settlement on the FIRST
// verified backfill, surfaces `settled:true` + receipt refs, and is at-most-once
// (#6011, EPIC #6017). The settlement sink is MOCKED here (no money path exercised at
// this layer — the engine has its own fail-closed tests); this proves the callback
// WIRES the trigger correctly: verified+executed => settle, idempotent replay => no
// re-fire, unverified => no settle.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeInMemoryKhalaVerificationStore,
} from './acceptance-dispatch'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import {
  type AcceptedOutcomeSettlementSink,
  handleAcceptanceVerdictCallback,
} from './acceptance-verdict-callback-routes'
import { type KhalaAcceptedOutcome } from './khala-accepted-outcome-settlement'

const CALLBACK_TOKEN = 'runner-callback-token-test'
const nowIso = () => '2026-06-22T18:00:00.000Z'

const verdictBody = (
  requestId: string,
  overrides: { verified?: boolean } = {},
) => ({
  requestId,
  schemaVersion: 'openagents.inference.acceptance_verdict.v1' as const,
  servedModel: 'openagents/khala-code',
  verdict: {
    checks: crossyRoadAcceptanceSpec().checks.map(id => ({
      detail: 'ok',
      id,
      passed: overrides.verified !== false,
    })),
    consoleErrors: [] as string[],
    executed: true as const,
    failedChecks:
      overrides.verified === false ? [...crossyRoadAcceptanceSpec().checks] : [],
    kind: 'crossy_road_single_html' as const,
    pageErrors: [] as string[],
    passedChecks:
      overrides.verified === false ? [] : [...crossyRoadAcceptanceSpec().checks],
    rubricRef: crossyRoadAcceptanceSpec().rubricRef,
    scalarReward: overrides.verified === false ? 0 : 1,
    verified: overrides.verified !== false,
  },
  worker: 'pylon-worker-1',
})

const makeRequest = (body: unknown) =>
  new Request('https://openagents.com/v1/inference/acceptance-verdicts', {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${CALLBACK_TOKEN}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

// A capturing mock settlement sink: records the outcomes it was asked to settle and
// returns a settled summary naming both parties. Proves the callback fired settlement
// with the right party refs without exercising the real money path.
const capturingSink = () => {
  const calls: KhalaAcceptedOutcome[] = []
  const sink: AcceptedOutcomeSettlementSink = outcome =>
    Effect.sync(() => {
      calls.push(outcome)
      return {
        settled: true,
        settledParties: ['serving_worker', 'validator'],
        settlementReceiptRefs: [
          `receipt.nexus.khala_serving_settlement.${outcome.workerRef}`,
          `receipt.nexus.khala_serving_settlement.${outcome.validatorRef}`,
        ],
      }
    })
  return { calls, sink }
}

describe('verdict callback -> accepted-outcome settlement wiring (#6011)', () => {
  test('a VERIFIED+EXECUTED first backfill fires settlement and surfaces settled:true', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const { calls, sink } = capturingSink()

    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(makeRequest(verdictBody('req_1')), {
        callbackToken: CALLBACK_TOKEN,
        enabled: true,
        nowIso,
        settlement: sink,
        store,
      }),
    )
    expect(response.status).toBe(200)
    const json = (await response.json()) as Record<string, unknown>
    expect(json.backfilled).toBe(true)
    expect(json.verified).toBe(true)
    // settled:true + per-party receipt refs are surfaced in the response.
    expect(json.settled).toBe(true)
    expect(json.settledParties).toEqual(['serving_worker', 'validator'])
    expect((json.settlementReceiptRefs as string[]).length).toBe(2)

    // The sink fired ONCE with the worker = the serving worker and validator = the
    // independent verifier worker id (a distinct party).
    expect(calls).toHaveLength(1)
    expect(calls[0]!.workerRef).toBe('pylon-worker-1')
    expect(calls[0]!.validatorRef).toBe('khala-code-crossy-road-verifier')
    expect(calls[0]!.verified).toBe(true)
    expect(calls[0]!.executed).toBe(true)
  })

  test('a redelivered callback is at-most-once: settlement does NOT re-fire', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const { calls, sink } = capturingSink()
    const deps = {
      callbackToken: CALLBACK_TOKEN,
      enabled: true,
      nowIso,
      settlement: sink,
      store,
    }

    await Effect.runPromise(
      handleAcceptanceVerdictCallback(makeRequest(verdictBody('req_2')), deps),
    )
    expect(calls).toHaveLength(1)

    // Redeliver the SAME verdict: the backfill is now idempotent (already executed), so
    // `backfilled:false` and the settlement sink is NOT fired again.
    const second = await Effect.runPromise(
      handleAcceptanceVerdictCallback(makeRequest(verdictBody('req_2')), deps),
    )
    const secondJson = (await second.json()) as Record<string, unknown>
    expect(secondJson.backfilled).toBe(false)
    expect(calls).toHaveLength(1)
  })

  test('a FAILED (unverified) outcome does NOT fire settlement', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const { calls, sink } = capturingSink()

    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest(verdictBody('req_3', { verified: false })),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso,
          settlement: sink,
          store,
        },
      ),
    )
    const json = (await response.json()) as Record<string, unknown>
    expect(json.backfilled).toBe(true)
    expect(json.verified).toBe(false)
    // No settlement fired for a failed outcome: the sink is gated on verified+executed,
    // so it is never called and no `settled` key is surfaced.
    expect('settled' in json).toBe(false)
    expect(calls).toHaveLength(0)
  })

  test('with NO settlement sink wired (inert default), the callback still backfills', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(makeRequest(verdictBody('req_4')), {
        callbackToken: CALLBACK_TOKEN,
        enabled: true,
        nowIso,
        store,
      }),
    )
    const json = (await response.json()) as Record<string, unknown>
    expect(json.backfilled).toBe(true)
    expect(json.verified).toBe(true)
    // No `settled` key at all when no sink is wired (the honest pre-arm default).
    expect('settled' in json).toBe(false)
  })
})
