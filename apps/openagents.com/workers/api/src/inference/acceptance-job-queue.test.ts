// Unit proof for the out-of-Worker acceptance-runner PULL queue (EPIC #6017).
//
// Covers the queue store lease semantics (enqueue idempotency, claim oldest-first,
// expired-lease re-claim, ack delivered/retryable) and the authenticated lease/ack
// routes (INERT gate, fail-closed auth, 204 idle, 200 lease, ack round-trip). No
// chromium — this is the Worker-safe pull side only.

import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'

import { AcceptanceJobMessage } from './acceptance-dispatch'
import { crossyRoadAcceptanceSpec } from './acceptance-spec'
import { acceptanceJobSpecFromSpec } from './acceptance-dispatch'
import { makeInMemoryAcceptanceJobQueueStore } from './acceptance-job-queue-store'
import {
  handleAcceptanceJobAck,
  handleAcceptanceJobLease,
} from './acceptance-job-lease-routes'

const job = (requestId: string): AcceptanceJobMessage =>
  AcceptanceJobMessage.make({
    artifactRef: `r2://artifacts/${requestId}.html`,
    meteringReceiptRef: null,
    requestId,
    schemaVersion: 'openagents.inference.acceptance_job.v1',
    servedModel: 'openagents/khala-code',
    spec: acceptanceJobSpecFromSpec(crossyRoadAcceptanceSpec()),
    worker: 'khala-code-crossy-road-verifier',
  })

const run = <A>(eff: Effect.Effect<A>): Promise<A> => Effect.runPromise(eff)

const TOKEN = 'test-runner-token'
let leaseCounter = 0
const newLeaseId = () => `lease-${(leaseCounter += 1)}`

describe('acceptance job queue store', () => {
  test('enqueue is idempotent per request id', async () => {
    const store = makeInMemoryAcceptanceJobQueueStore()
    await run(store.enqueue(job('r1')))
    await run(store.enqueue(job('r1')))
    const a = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'l1', nowIso: new Date().toISOString() }),
    )
    const b = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'l2', nowIso: new Date().toISOString() }),
    )
    expect(a?.message.requestId).toBe('r1')
    // Only ONE row existed despite the duplicate enqueue.
    expect(b).toBeNull()
  })

  test('leases oldest pending first, then idles', async () => {
    const store = makeInMemoryAcceptanceJobQueueStore()
    await run(store.enqueue(job('older')))
    await new Promise(r => setTimeout(r, 5))
    await run(store.enqueue(job('newer')))
    const first = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'l1', nowIso: new Date().toISOString() }),
    )
    const second = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'l2', nowIso: new Date().toISOString() }),
    )
    const third = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'l3', nowIso: new Date().toISOString() }),
    )
    expect(first?.message.requestId).toBe('older')
    expect(second?.message.requestId).toBe('newer')
    expect(third).toBeNull()
  })

  test('an EXPIRED lease becomes re-claimable; a live lease does not', async () => {
    const store = makeInMemoryAcceptanceJobQueueStore()
    await run(store.enqueue(job('r1')))
    const t0 = '2026-06-22T00:00:00.000Z'
    const leased = await run(
      store.lease({ leaseTtlMs: 60_000, newLeaseId: 'l1', nowIso: t0 }),
    )
    expect(leased?.leaseId).toBe('l1')
    // Still within TTL: not re-claimable.
    const stillLeased = await run(
      store.lease({ leaseTtlMs: 60_000, newLeaseId: 'l2', nowIso: '2026-06-22T00:00:30.000Z' }),
    )
    expect(stillLeased).toBeNull()
    // After TTL: re-claimable (a crashed runner's job).
    const reclaimed = await run(
      store.lease({ leaseTtlMs: 60_000, newLeaseId: 'l3', nowIso: '2026-06-22T00:02:00.000Z' }),
    )
    expect(reclaimed?.leaseId).toBe('l3')
    expect(reclaimed?.message.requestId).toBe('r1')
  })

  test('ack delivered removes the job; ack retryable returns it to pending', async () => {
    const store = makeInMemoryAcceptanceJobQueueStore()
    await run(store.enqueue(job('deliver')))
    await run(store.enqueue(job('retry')))

    const a = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'la', nowIso: new Date().toISOString() }),
    )
    await run(store.ack({ delivered: true, leaseId: a!.leaseId, nowIso: new Date().toISOString() }))

    const b = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'lb', nowIso: new Date().toISOString() }),
    )
    await run(store.ack({ delivered: false, leaseId: b!.leaseId, nowIso: new Date().toISOString() }))

    // 'deliver' is gone; 'retry' is pending again and re-leasable.
    const next = await run(
      store.lease({ leaseTtlMs: 1000, newLeaseId: 'lc', nowIso: new Date().toISOString() }),
    )
    expect(next?.message.requestId).toBe('retry')
  })
})

describe('acceptance job lease/ack routes', () => {
  const baseDeps = () => ({
    callbackToken: TOKEN,
    enabled: true,
    newLeaseId,
    nowIso: () => new Date().toISOString(),
    store: makeInMemoryAcceptanceJobQueueStore(),
  })

  const get = (token?: string) =>
    new Request('https://x/v1/inference/acceptance-jobs/lease', {
      headers: token === undefined ? {} : { authorization: `Bearer ${token}` },
      method: 'GET',
    })

  test('INERT: 404 when the gateway is disabled', async () => {
    const res = await run(
      handleAcceptanceJobLease(get(TOKEN), { ...baseDeps(), enabled: false }),
    )
    expect(res.status).toBe(404)
  })

  test('fail-closed: 401 without the token, 401 with a wrong token', async () => {
    const deps = baseDeps()
    expect((await run(handleAcceptanceJobLease(get(), deps))).status).toBe(401)
    expect((await run(handleAcceptanceJobLease(get('nope'), deps))).status).toBe(401)
  })

  test('closed: 401 when no callback token is configured at all', async () => {
    const deps = { ...baseDeps(), callbackToken: undefined }
    expect((await run(handleAcceptanceJobLease(get(TOKEN), deps))).status).toBe(401)
  })

  test('204 when the queue is empty (idle)', async () => {
    const res = await run(handleAcceptanceJobLease(get(TOKEN), baseDeps()))
    expect(res.status).toBe(204)
  })

  test('200 leases a job, ack delivered removes it, then idle 204', async () => {
    const deps = baseDeps()
    await run(deps.store.enqueue(job('r-route')))

    const leaseRes = await run(handleAcceptanceJobLease(get(TOKEN), deps))
    expect(leaseRes.status).toBe(200)
    const leaseBody = (await leaseRes.json()) as {
      leaseId: string
      job: { requestId: string; schemaVersion: string }
    }
    expect(leaseBody.job.requestId).toBe('r-route')
    expect(leaseBody.job.schemaVersion).toBe(
      'openagents.inference.acceptance_job.v1',
    )

    const ackReq = new Request('https://x/v1/inference/acceptance-jobs/ack', {
      body: JSON.stringify({ delivered: true, leaseId: leaseBody.leaseId }),
      headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' },
      method: 'POST',
    })
    const ackRes = await run(handleAcceptanceJobAck(ackReq, deps))
    expect(ackRes.status).toBe(200)

    // Job removed: next lease idles.
    expect((await run(handleAcceptanceJobLease(get(TOKEN), deps))).status).toBe(204)
  })

  test('ack rejects a malformed body 400', async () => {
    const deps = baseDeps()
    const bad = new Request('https://x/v1/inference/acceptance-jobs/ack', {
      body: JSON.stringify({ nope: true }),
      headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' },
      method: 'POST',
    })
    expect((await run(handleAcceptanceJobAck(bad, deps))).status).toBe(400)
  })
})
