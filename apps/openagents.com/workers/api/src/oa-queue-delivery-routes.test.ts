import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OA_QUEUE_DELIVER_PATH,
  handleOaQueueDeliver,
} from './oa-queue-delivery-routes'

const deliverRequest = (body: unknown): Request =>
  new Request(`https://openagents.com${OA_QUEUE_DELIVER_PATH}`, {
    body: JSON.stringify(body),
    method: 'POST',
  })

const run = (
  request: Request,
  deps: Readonly<{
    authorized?: boolean
    dispatch?: (body: unknown) => Promise<void>
  }> = {},
): Promise<Response> =>
  Effect.runPromise(
    handleOaQueueDeliver(request, {
      dispatch: deps.dispatch ?? (async () => {}),
      requireOperator: async () => deps.authorized ?? true,
    }),
  )

describe('oa-queue delivery route (CFG-7 #8522)', () => {
  test('rejects non-POST', async () => {
    const response = await run(
      new Request(`https://openagents.com${OA_QUEUE_DELIVER_PATH}`),
    )
    expect(response.status).toBe(405)
  })

  test('rejects unauthorized callers before reading the body', async () => {
    const response = await run(
      deliverRequest({
        attempts: 1,
        jobId: 'job_1',
        payload: '{}',
        topic: 'openagents-event-ledger-ingest',
      }),
      { authorized: false },
    )
    expect(response.status).toBe(401)
  })

  test('rejects malformed delivery envelopes with 400', async () => {
    const response = await run(deliverRequest({ nope: true }))
    expect(response.status).toBe(400)
  })

  test('reports unparseable job payloads as permanent 422', async () => {
    const response = await run(
      deliverRequest({
        attempts: 1,
        jobId: 'job_1',
        payload: 'not json {',
        topic: 'openagents-event-ledger-ingest',
      }),
    )
    expect(response.status).toBe(422)
    await expect(response.json()).resolves.toMatchObject({
      error: 'payload_not_json',
    })
  })

  test('dispatches the decoded payload and acks with 200', async () => {
    const dispatched: Array<unknown> = []
    const response = await run(
      deliverRequest({
        attempts: 2,
        jobId: 'job_7',
        payload: JSON.stringify({
          schemaVersion: 'openagents.event_ledger_ingest.v1',
          value: 42,
        }),
        topic: 'openagents-event-ledger-ingest',
      }),
      {
        dispatch: async body => {
          dispatched.push(body)
        },
      },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      jobId: 'job_7',
      ok: true,
      topic: 'openagents-event-ledger-ingest',
    })
    expect(dispatched).toEqual([
      { schemaVersion: 'openagents.event_ledger_ingest.v1', value: 42 },
    ])
  })

  test('maps dispatch failures to 500 so the pump nacks', async () => {
    const response = await run(
      deliverRequest({
        attempts: 1,
        jobId: 'job_9',
        payload: '{"schemaVersion":"openagents.adjutant_enrichment_job.v1"}',
        topic: 'openagents-adjutant-enrichment-jobs',
      }),
      {
        dispatch: async () => {
          throw new Error('D1 DB is overloaded')
        },
      },
    )

    expect(response.status).toBe(500)
    await expect(response.json()).resolves.toMatchObject({
      error: 'dispatch_failed',
      ok: false,
    })
  })
})
