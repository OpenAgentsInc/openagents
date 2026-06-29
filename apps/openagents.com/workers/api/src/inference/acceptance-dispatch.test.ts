// Full-loop proof for the async acceptance-verification dispatch (EPIC #6017).
//
// PROVES THE WHOLE CHAIN against the COMMITTED crossy-road fixtures, end to end:
//
//   gateway enqueues a job  ->  node-side harness runs the REAL headless runner
//   (Playwright/chromium)   ->  posts the verdict to the authenticated callback
//                           ->  the callback BACKFILLS the verification receipt.
//
//   - broken fixture  => verdict `failed` (per-test) => receipt backfilled to
//                        `failed`, scalarReward < 1.
//   - fixed fixture   => verdict `test_passed`        => receipt backfilled to
//                        `test_passed`, verified, scalarReward 1.
//
// Plus the unit-level guarantees the loop relies on: enqueue is INERT when the flag
// is off; the callback REJECTS unauthenticated/forged verdicts; backfill is
// idempotent. Requires a real headless chromium (Playwright); a single browser is
// reused across cases.
//
// Run with:
//   bun run --cwd apps/openagents.com/workers/api test -- src/inference/acceptance-dispatch
//   (chromium installed via `bunx playwright install chromium`)

import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { chromium, type Browser } from 'playwright'
import { Effect } from 'effect'

import {
  type AcceptanceJobMessage,
  type AcceptanceJobQueue,
  authenticateVerdictCallback,
  backfillVerdictIntoVerification,
  enqueueAcceptanceJob,
  isAcceptanceDispatchEnabled,
  makeInMemoryKhalaVerificationStore,
} from './acceptance-dispatch'
import { handleAcceptanceVerdictCallback } from './acceptance-verdict-callback-routes'
import {
  type RunnerTransport,
  runAcceptanceJob,
  type VerdictCallbackPayload,
} from './acceptance-runner/harness'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import { CROSSY_ROAD_BROKEN_HTML } from './acceptance-runner/fixtures/crossy-road-broken.html'
import { CROSSY_ROAD_FIXED_HTML } from './acceptance-runner/fixtures/crossy-road-fixed.html'

let browser: Browser

beforeAll(async () => {
  browser = await chromium.launch({ headless: true })
}, 60_000)

afterAll(async () => {
  await browser.close().catch(() => undefined)
})

const CALLBACK_TOKEN = 'test-runner-callback-token'

// Wire the full loop against in-memory seams: a fake queue records the enqueued job;
// the harness transport resolves the artifact from a fixture map and posts the verdict
// straight into the verdict-callback ROUTE (with the real bearer token), which
// backfills the in-memory verification store. This is the production data flow with
// only the infra boundaries faked.
const runFullLoop = async (
  fixtureHtml: string,
): Promise<{
  enqueuedMessage: AcceptanceJobMessage
  callbackStatus: number
  callbackBody: Record<string, unknown>
  storeRequestId: string
}> => {
  const store = makeInMemoryKhalaVerificationStore(() => '2026-06-22T00:00:00.000Z')
  const requestId = 'chatcmpl-loop-1'
  const artifactRef = 'r2://artifacts/loop-1.html'

  // (1) GATEWAY: enqueue the verification job (flag ON, fake queue).
  const sent: AcceptanceJobMessage[] = []
  const queue: AcceptanceJobQueue = {
    send: async message => {
      sent.push(message)
    },
  }
  const enqueueOutcome = await Effect.runPromise(
    enqueueAcceptanceJob({
      artifactRef,
      enabled: true,
      meteringReceiptRef: 'receipt.inference.charge.loop-1',
      queue,
      requestId,
      servedModel: 'served/crossy',
      spec: crossyRoadAcceptanceSpec(),
      worker: 'pylon-worker-1',
    }),
  )
  expect(enqueueOutcome.enqueued).toBe(true)
  expect(sent).toHaveLength(1)
  const enqueuedMessage = sent[0]!

  // (2) NODE-SIDE HARNESS: resolve the artifact + run the REAL headless runner, then
  //     post the verdict into the verdict-callback ROUTE with the bearer token.
  let callbackStatus = 0
  let callbackBody: Record<string, unknown> = {}
  const transport: RunnerTransport = {
    resolveArtifact: async ref => {
      expect(ref).toBe(artifactRef)
      return fixtureHtml
    },
    postVerdict: async (payload: VerdictCallbackPayload) => {
      const request = new Request(
        'https://openagents.com/v1/inference/acceptance-verdicts',
        {
          body: JSON.stringify(payload),
          headers: {
            authorization: `Bearer ${CALLBACK_TOKEN}`,
            'content-type': 'application/json',
          },
          method: 'POST',
        },
      )
      // (3) GATEWAY CALLBACK: authenticate + backfill the receipt.
      const response = await Effect.runPromise(
        handleAcceptanceVerdictCallback(request, {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso: () => '2026-06-22T00:00:01.000Z',
          store,
        }),
      )
      callbackStatus = response.status
      callbackBody = (await response.json()) as Record<string, unknown>
      if (!response.ok) {
        throw new Error(`callback_failed: ${response.status}`)
      }
    },
  }

  const result = await runAcceptanceJob(transport, enqueuedMessage, { browser })
  expect(result.delivered).toBe(true)

  // (4) The store row is now backfilled from EXECUTION.
  const record = await Effect.runPromise(store.read(requestId))
  expect(record).not.toBeNull()

  return {
    callbackBody,
    callbackStatus,
    enqueuedMessage,
    storeRequestId: requestId,
  }
}

describe('acceptance-dispatch — full loop against committed fixtures', () => {
  test('FIXED fixture: job -> runner -> verdict test_passed -> receipt backfilled, reward 1', async () => {
    const out = await runFullLoop(CROSSY_ROAD_FIXED_HTML)

    expect(out.enqueuedMessage.schemaVersion).toBe(
      'openagents.inference.acceptance_job.v1',
    )
    expect(out.callbackStatus).toBe(200)
    expect(out.callbackBody.verdict).toBe('test_passed')
    expect(out.callbackBody.verified).toBe(true)
    expect(out.callbackBody.scalarReward).toBe(1)
    expect(out.callbackBody.backfilled).toBe(true)
    expect(out.callbackBody.failedChecks).toEqual([])
  }, 90_000)

  test('BROKEN fixture: job -> runner -> verdict failed (per-test) -> receipt backfilled failed, low reward', async () => {
    const out = await runFullLoop(CROSSY_ROAD_BROKEN_HTML)

    expect(out.callbackStatus).toBe(200)
    expect(out.callbackBody.verdict).toBe('failed')
    expect(out.callbackBody.verified).toBe(false)
    expect(out.callbackBody.scalarReward).toBeLessThan(1)
    expect(out.callbackBody.backfilled).toBe(true)
    // The four caught bugs surface as per-test failures.
    const failed = out.callbackBody.failedChecks as ReadonlyArray<string>
    expect(failed).toContain('loads_without_errors')
    expect(failed).toContain('play_starts_game')
  }, 90_000)
})

describe('acceptance-dispatch — enqueue is inert by default', () => {
  test('flag OFF: nothing is enqueued; the message shape is still derivable', async () => {
    const sent: AcceptanceJobMessage[] = []
    const queue: AcceptanceJobQueue = {
      send: async m => {
        sent.push(m)
      },
    }
    const outcome = await Effect.runPromise(
      enqueueAcceptanceJob({
        artifactRef: 'r2://x.html',
        enabled: false,
        queue,
        requestId: 'chatcmpl-inert',
        servedModel: 'm',
        spec: crossyRoadAcceptanceSpec(),
        worker: 'w',
      }),
    )
    expect(outcome.enqueued).toBe(false)
    expect(sent).toHaveLength(0)
    expect(outcome.message.requestId).toBe('chatcmpl-inert')
  })

  test('flag parsing fails closed', () => {
    expect(isAcceptanceDispatchEnabled('on')).toBe(true)
    expect(isAcceptanceDispatchEnabled('true')).toBe(true)
    expect(isAcceptanceDispatchEnabled('1')).toBe(true)
    expect(isAcceptanceDispatchEnabled('')).toBe(false)
    expect(isAcceptanceDispatchEnabled(undefined)).toBe(false)
    expect(isAcceptanceDispatchEnabled('armed-ish')).toBe(false)
  })
})

describe('acceptance-verdict-callback — fail-closed auth + idempotent backfill', () => {
  const verdictBody = (requestId: string) => ({
    requestId,
    schemaVersion: 'openagents.inference.acceptance_verdict.v1' as const,
    servedModel: 'm',
    verdict: {
      checks: crossyRoadAcceptanceSpec().checks.map(id => ({
        detail: 'ok',
        id,
        passed: true,
      })),
      consoleErrors: [] as string[],
      executed: true as const,
      failedChecks: [] as string[],
      kind: 'crossy_road_single_html' as const,
      pageErrors: [] as string[],
      passedChecks: [...crossyRoadAcceptanceSpec().checks],
      rubricRef: crossyRoadAcceptanceSpec().rubricRef,
      scalarReward: 1,
      verified: true,
    },
    worker: 'w',
  })

  const makeRequest = (token: string | null, body: unknown) =>
    new Request('https://openagents.com/v1/inference/acceptance-verdicts', {
      body: JSON.stringify(body),
      headers: {
        ...(token === null ? {} : { authorization: `Bearer ${token}` }),
        'content-type': 'application/json',
      },
      method: 'POST',
    })

  test('rejects a missing bearer token (401) and writes nothing', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(makeRequest(null, verdictBody('r1')), {
        callbackToken: CALLBACK_TOKEN,
        enabled: true,
        nowIso: () => 'now',
        store,
      }),
    )
    expect(response.status).toBe(401)
    expect(await Effect.runPromise(store.read('r1'))).toBeNull()
  })

  test('rejects a forged/mismatched token (401)', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest('wrong-token', verdictBody('r2')),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso: () => 'now',
          store,
        },
      ),
    )
    expect(response.status).toBe(401)
  })

  test('closed when no token is configured (401), even with a bearer present', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest(CALLBACK_TOKEN, verdictBody('r3')),
        {
          callbackToken: undefined,
          enabled: true,
          nowIso: () => 'now',
          store,
        },
      ),
    )
    expect(response.status).toBe(401)
  })

  test('rejects a malformed body (400)', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest(CALLBACK_TOKEN, { schemaVersion: 'nope' }),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso: () => 'now',
          store,
        },
      ),
    )
    expect(response.status).toBe(400)
  })

  test('404 when the gateway flag is off', async () => {
    const store = makeInMemoryKhalaVerificationStore()
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest(CALLBACK_TOKEN, verdictBody('r4')),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: false,
          nowIso: () => 'now',
          store,
        },
      ),
    )
    expect(response.status).toBe(404)
  })

  test('backfill is idempotent: a redelivered executed verdict does not double-write', async () => {
    const store = makeInMemoryKhalaVerificationStore(() => 'now')
    const body = await import('./acceptance-dispatch').then(m =>
      m.AcceptanceVerdictCallbackBody.make(verdictBody('r5')),
    )

    const first = await Effect.runPromise(
      backfillVerdictIntoVerification({ nowIso: () => 'now', store }, body),
    )
    expect(first.backfilled).toBe(true)
    expect(first.record.version).toBe(1)
    expect(first.record.verification).toBe('test_passed')

    const second = await Effect.runPromise(
      backfillVerdictIntoVerification({ nowIso: () => 'now', store }, body),
    )
    expect(second.backfilled).toBe(false)
    expect(second.record.version).toBe(1)
  })

  test('a valid authenticated verdict backfills (200)', async () => {
    const store = makeInMemoryKhalaVerificationStore(() => 'now')
    const response = await Effect.runPromise(
      handleAcceptanceVerdictCallback(
        makeRequest(CALLBACK_TOKEN, verdictBody('r6')),
        {
          callbackToken: CALLBACK_TOKEN,
          enabled: true,
          nowIso: () => 'now',
          store,
        },
      ),
    )
    expect(response.status).toBe(200)
    const record = await Effect.runPromise(store.read('r6'))
    expect(record?.verified).toBe(true)
    expect(record?.executed).toBe(true)
  })

  test('constant-time bearer compare matches only on exact equality', () => {
    expect(
      authenticateVerdictCallback({
        authorizationHeader: `Bearer ${CALLBACK_TOKEN}`,
        configuredToken: CALLBACK_TOKEN,
      }),
    ).toBe(true)
    expect(
      authenticateVerdictCallback({
        authorizationHeader: `Bearer ${CALLBACK_TOKEN}x`,
        configuredToken: CALLBACK_TOKEN,
      }),
    ).toBe(false)
  })
})
