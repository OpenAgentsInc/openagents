import { describe, expect, it } from 'vitest'

import {
  DEFAULT_TASSADAR_RUN_REF,
  PublicTassadarRunSummarySchemaVersion,
  handlePublicTassadarRunSummary,
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
const req = (url = 'https://openagents.com/api/public/tassadar-run-summary') => new Request(url)

describe('handlePublicTassadarRunSummary (public read, #5114)', () => {
  it('returns an honest idle envelope when the run is not found (receipt-first)', async () => {
    const res = await handlePublicTassadarRunSummary(req(), {} as never, {
      makeStore: () => fakeStore(),
      now,
    })
    expect(res.status).toBe(200)
    expect(res.headers.get('cache-control')).toBe('no-store')
    const body = (await res.json()) as Record<string, unknown>
    expect(body.schemaVersion).toBe(PublicTassadarRunSummarySchemaVersion)
    expect(body.runRef).toBe(DEFAULT_TASSADAR_RUN_REF)
    expect(body.runState).toBe('planned')
    expect((body.emptyState as { idle: boolean }).idle).toBe(true)
    expect(body.metrics).toEqual({})
    expect(body.generatedAt).toBe('2026-06-16T12:00:00.000Z')
  })

  it('honors the ?run= query param when choosing which run to read', async () => {
    let asked = ''
    await handlePublicTassadarRunSummary(req('https://openagents.com/api/public/tassadar-run-summary?run=run.custom.test'), {} as never, {
      makeStore: () =>
        fakeStore({
          readRun: async (ref: string) => {
            asked = ref
            return undefined
          },
        }),
      now,
    })
    expect(asked).toBe('run.custom.test')
  })

  it('requires no admin auth — an unauthenticated request still returns 200', async () => {
    const res = await handlePublicTassadarRunSummary(req(), {} as never, {
      makeStore: () => fakeStore(),
      now,
    })
    expect(res.status).toBe(200)
  })
})
