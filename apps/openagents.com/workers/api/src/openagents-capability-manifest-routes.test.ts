import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OpenAgentsCapabilityManifestEndpoint } from './openagents-capability-manifest'
import { handleOpenAgentsCapabilityManifestApi } from './openagents-capability-manifest-routes'

const runRoute = (method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsCapabilityManifestApi(
      new Request(
        `https://openagents.com${OpenAgentsCapabilityManifestEndpoint}`,
        { method },
      ),
    ),
  )

describe('OpenAgents capability manifest route', () => {
  test('serves active capabilities without advertising retired graphs', async () => {
    const response = await runRoute()
    const body = (await response.json()) as {
      actions: ReadonlyArray<Record<string, unknown>>
      authModes: ReadonlyArray<Record<string, unknown>>
      caveats: ReadonlyArray<string>
      docs: Record<string, string>
      resources: ReadonlyArray<Record<string, unknown>>
      schemaVersion: string
      service: Record<string, string>
    }
    const entries = [...body.actions, ...body.resources]
    const advertisedEntryText = entries
      .map(entry => `${String(entry.id ?? '')} ${String(entry.href ?? '')}`)
      .join('\n')

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.schemaVersion).toBe('openagents.capabilities.v1')
    expect(body.docs).not.toHaveProperty('liveSiteSource')
    expect(body.docs).not.toHaveProperty('sitesPlan')
    expect(entries.length).toBeGreaterThan(20)
    expect(advertisedEntryText).not.toMatch(
      /(?:billing|checkout|credits?|payments?|wallet|payout|settlement|treasury|tips?|markets?|sites?)(?:\/|\b)/i,
    )
    expect(body.caveats.join('\n')).toContain(
      'No retired paid or credit-gated capacity becomes free capacity',
    )
    expect(containsProviderSecretMaterial(JSON.stringify(body))).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await runRoute('POST')

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    await expect(response.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
  })
})
