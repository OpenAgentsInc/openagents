import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeInMemoryKhalaVerificationStore } from './acceptance-dispatch'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import { handleAcceptanceVerdictCallback } from './acceptance-verdict-callback-routes'

const CALLBACK_TOKEN = 'runner-callback-token-test'

describe('VP1 verdict callback', () => {
  test('backfills verification but never invokes a legacy settlement sink', async () => {
    let settlementInvoked = false
    const checks = crossyRoadAcceptanceSpec().checks
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        new Request(
          'https://openagents.com/v1/inference/acceptance-verdicts',
          {
            body: JSON.stringify({
              requestId: 'req.no-spend',
              schemaVersion: 'openagents.inference.acceptance_verdict.v1',
              servedModel: 'openagents/khala-code',
              verdict: {
                checks: checks.map(id => ({ detail: 'ok', id, passed: true })),
                consoleErrors: [],
                executed: true,
                failedChecks: [],
                kind: 'crossy_road_single_html',
                pageErrors: [],
                passedChecks: [...checks],
                rubricRef: crossyRoadAcceptanceSpec().rubricRef,
                scalarReward: 1,
                verified: true,
              },
              worker: 'pylon-worker-1',
            }),
            headers: {
              authorization: `Bearer ${CALLBACK_TOKEN}`,
              'content-type': 'application/json',
            },
            method: 'POST',
          },
        ),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso: () => '2026-07-14T00:00:00.000Z',
          settlement: () => {
            settlementInvoked = true
            return Effect.succeed({
              settled: true,
              settledParties: [],
              settlementReceiptRefs: [],
            })
          },
          store: makeInMemoryKhalaVerificationStore(),
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toMatchObject({
      backfilled: true,
      paymentMode: 'no-spend',
      payoutClaimAllowed: false,
      settlementState: 'not_applicable',
      verified: true,
    })
    expect(settlementInvoked).toBe(false)
  })
})
