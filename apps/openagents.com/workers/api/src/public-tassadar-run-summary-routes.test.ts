import { describe, expect, it } from 'vitest'

import {
  DEFAULT_TASSADAR_RUN_REF,
  PublicTassadarRunSummarySchemaVersion,
  buildPublicTassadarRunSummaryEnvelopeForRequest,
} from './public-tassadar-run-summary-routes'

// Minimal fake store; tests never hit D1. readRun defaults to "not found".
const fakeStore = (overrides: Record<string, unknown> = {}) =>
  ({
    readRun: async () => undefined,
    listWindowsForRun: async () => [],
    listWindowLeasesForRun: async () => [],
    listVerificationChallengesForRun: async () => [],
    ...overrides,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  }) as any

const now = () => '2026-06-16T12:00:00.000Z'
const req = (url = 'https://openagents.com/api/public/tassadar-run-summary') =>
  new Request(url)

describe('buildPublicTassadarRunSummaryEnvelopeForRequest (public read, #5114)', () => {
  it('returns an honest idle envelope when the run is not found (receipt-first)', async () => {
    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makeStore: () => fakeStore(),
        now,
      },
    )
    expect(body.schemaVersion).toBe(PublicTassadarRunSummarySchemaVersion)
    expect(body.runRef).toBe(DEFAULT_TASSADAR_RUN_REF)
    expect(body.runState).toBe('planned')
    expect((body.emptyState as { idle: boolean }).idle).toBe(true)
    expect(body.metrics).toEqual({})
    expect(body.generatedAt).toBe('2026-06-16T12:00:00.000Z')
    expect((body.staleness as { composition?: unknown }).composition).toBe(
      'live_at_read',
    )
    expect(
      (body.staleness as { maxStalenessSeconds?: unknown }).maxStalenessSeconds,
    ).toBe(0)
  })

  it('honors the ?run= query param when choosing which run to read', async () => {
    let asked = ''
    await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(
        'https://openagents.com/api/public/tassadar-run-summary?run=run.custom.test',
      ),
      {} as never,
      {
        makeStore: () =>
          fakeStore({
            readRun: async (ref: string) => {
              asked = ref
              return undefined
            },
          }),
        now,
      },
    )
    expect(asked).toBe('run.custom.test')
  })

  it('requires no admin auth to build the public read envelope', async () => {
    const body = await buildPublicTassadarRunSummaryEnvelopeForRequest(
      req(),
      {} as never,
      {
        makeStore: () => fakeStore(),
        now,
      },
    )
    expect(body.runRef).toBe(DEFAULT_TASSADAR_RUN_REF)
  })
})
