import { describe, expect, test } from 'vitest'

import {
  OA_JOB_MAX_ATTEMPTS,
  OA_JOB_TOPIC_ADJUTANT_ENRICHMENT,
  OA_JOB_TOPIC_EVENT_LEDGER_INGEST,
  OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA,
  makeOaJobEnqueueForEnv,
} from './oa-job-queue-producer'
import type { KhalaSyncPushSqlClient } from './khala-sync-push-routes'

type CapturedStatement = Readonly<{
  text: string
  values: ReadonlyArray<unknown>
}>

const makeFakeSqlClient = (
  captured: Array<CapturedStatement>,
  events: Array<string>,
  rows: ReadonlyArray<{ id: string }> = [{ id: 'job_fake_1' }],
): KhalaSyncPushSqlClient => ({
  end: async () => {
    events.push('end')
  },
  sql: (async (
    strings: TemplateStringsArray,
    ...values: ReadonlyArray<unknown>
  ) => {
    captured.push({ text: strings.join('$?'), values })
    return rows
  }) as unknown as KhalaSyncPushSqlClient['sql'],
})

describe('oa-job-queue-producer (CFG-7 #8522)', () => {
  test('returns undefined without a KHALA_SYNC_DB connection', () => {
    expect(makeOaJobEnqueueForEnv({})).toBeUndefined()
    expect(
      makeOaJobEnqueueForEnv({ KHALA_SYNC_DB: { connectionString: '' } }),
    ).toBeUndefined()
  })

  test('enqueue is a single INSERT into oa_infra_jobs matching the oa-infra backend semantics', async () => {
    const captured: Array<CapturedStatement> = []
    const events: Array<string> = []
    const enqueue = makeOaJobEnqueueForEnv(
      { KHALA_SYNC_DB: { connectionString: 'postgres://example/khala' } },
      {
        makeSqlClient: async connectionString => {
          events.push(`acquire:${connectionString}`)
          return makeFakeSqlClient(captured, events)
        },
      },
    )

    expect(enqueue).toBeDefined()

    const jobId = await enqueue!(
      OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA,
      '{"schemaVersion":"openagents.pylon.codex_raw_event_metadata_queue.v1"}',
    )

    expect(jobId).toBe('job_fake_1')
    // One connection acquired, one statement, connection always released.
    expect(events).toEqual(['acquire:postgres://example/khala', 'end'])
    expect(captured).toHaveLength(1)
    const statement = captured[0]!
    expect(statement.text).toContain('INSERT INTO oa_infra_jobs')
    expect(statement.text).toContain('RETURNING id')
    expect(statement.values).toEqual([
      OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA,
      '{"schemaVersion":"openagents.pylon.codex_raw_event_metadata_queue.v1"}',
      OA_JOB_MAX_ATTEMPTS,
    ])
  })

  test('releases the connection even when the INSERT fails', async () => {
    const events: Array<string> = []
    const enqueue = makeOaJobEnqueueForEnv(
      { KHALA_SYNC_DB: { connectionString: 'postgres://example/khala' } },
      {
        makeSqlClient: async () => ({
          end: async () => {
            events.push('end')
          },
          sql: (async () => {
            throw new Error('connection refused')
          }) as unknown as KhalaSyncPushSqlClient['sql'],
        }),
      },
    )

    await expect(
      enqueue!(OA_JOB_TOPIC_EVENT_LEDGER_INGEST, '{}'),
    ).rejects.toThrow('connection refused')
    expect(events).toEqual(['end'])
  })

  test('topics mirror the retired Cloudflare queue names 1:1', () => {
    expect(OA_JOB_TOPIC_ADJUTANT_ENRICHMENT).toBe(
      'openagents-adjutant-enrichment-jobs',
    )
    expect(OA_JOB_TOPIC_EVENT_LEDGER_INGEST).toBe(
      'openagents-event-ledger-ingest',
    )
    expect(OA_JOB_TOPIC_PYLON_CODEX_RAW_EVENT_METADATA).toBe(
      'openagents-pylon-codex-raw-event-metadata',
    )
    // First delivery + the retired consumers' max_retries: 3.
    expect(OA_JOB_MAX_ATTEMPTS).toBe(4)
  })
})
