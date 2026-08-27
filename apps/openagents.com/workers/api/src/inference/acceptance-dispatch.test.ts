// Unit proof for the retired acceptance-dispatch compatibility surface.

import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import {
  type AcceptanceJobMessage,
  type AcceptanceJobQueue,
  authenticateVerdictCallback,
  backfillVerdictIntoVerification,
  enqueueAcceptanceJob,
  isAcceptanceDispatchEnabled,
  type KhalaVerificationRecord,
  type KhalaVerificationStore,
  makeMirroredKhalaVerificationStore,
} from './acceptance-dispatch'
import { handleAcceptanceVerdictCallback } from './acceptance-verdict-callback-routes'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import type { AgentRuntimeRemainderMirror } from '../agent-runtime-remainder-store'

const CALLBACK_TOKEN = 'test-runner-callback-token'

class MemoryAgentRuntimeRemainderMirror implements AgentRuntimeRemainderMirror {
  readonly calls: Array<{
    pkValues: ReadonlyArray<string>
    table: string
  }> = []

  mirrorRowsByPk = async (
    table: Parameters<AgentRuntimeRemainderMirror['mirrorRowsByPk']>[0],
    pkValues: ReadonlyArray<string>,
  ) => {
    this.calls.push({ pkValues, table })
  }
}

const makeMemoryKhalaVerificationStore = (
  nowIso: () => string = () => new Date().toISOString(),
): KhalaVerificationStore => {
  const rows = new Map<string, KhalaVerificationRecord>()
  return {
    read: requestId => Effect.sync(() => rows.get(requestId) ?? null),
    upsert: record =>
      Effect.sync(() => {
        rows.set(record.requestId, { ...record, updatedAt: nowIso() })
      }),
  }
}

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

  test('mirrored verification store mirrors verdict upserts by request id', async () => {
    const mirror = new MemoryAgentRuntimeRemainderMirror()
    const store = makeMirroredKhalaVerificationStore(
      makeMemoryKhalaVerificationStore(() => '2026-06-22T00:00:00.000Z'),
      mirror,
    )
    const record: KhalaVerificationRecord = {
      executed: true,
      failedChecks: [],
      passedChecks: [...crossyRoadAcceptanceSpec().checks],
      requestId: 'chatcmpl-mirror-verdict',
      rubricRef: crossyRoadAcceptanceSpec().rubricRef,
      scalarReward: 1,
      updatedAt: '2026-06-22T00:00:00.000Z',
      verification: 'test_passed',
      verificationReceiptRef:
        'receipt.inference.khala_code_acceptance.chatcmpl-mirror-verdict',
      verified: true,
      version: 1,
    }

    await Effect.runPromise(store.upsert(record))

    expect(mirror.calls).toEqual([
      {
        pkValues: ['chatcmpl-mirror-verdict'],
        table: 'khala_acceptance_verdicts',
      },
    ])
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
    const store = makeMemoryKhalaVerificationStore()
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
    const store = makeMemoryKhalaVerificationStore()
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
    const store = makeMemoryKhalaVerificationStore()
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
    const store = makeMemoryKhalaVerificationStore()
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
    const store = makeMemoryKhalaVerificationStore()
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
    const store = makeMemoryKhalaVerificationStore(() => 'now')
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
    const store = makeMemoryKhalaVerificationStore(() => 'now')
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
