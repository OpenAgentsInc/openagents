import { describe, expect, test } from 'vitest'
import { handlePublicLaborEarningsApi } from './labor-earnings-routes'
import { Effect } from 'effect'

describe('LaborEarningsRoutes', () => {
  test('returns 400 when providerRef is missing', async () => {
    const request = new Request('https://openagents.com/api/public/labor-earnings')
    const response = await Effect.runPromise(
      handlePublicLaborEarningsApi(request, { ledgerDb: null as never })
    )
    expect(response.status).toBe(400)
  })
})
