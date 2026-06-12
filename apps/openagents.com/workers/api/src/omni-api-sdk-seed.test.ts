import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_API_SDK_SEED,
  OmniApiSdkSeed,
  OmniApiSdkSeedEndpoint,
  omniApiSdkSeed,
  omniApiSdkSeedHasRequiredSurfaces,
  omniApiSdkSeedIsPrivateDataSafe,
} from './omni-api-sdk-seed'
import { handleOmniApiSdkSeedApi } from './omni-api-sdk-seed-routes'

const runRoute = (method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOmniApiSdkSeedApi(
      new Request(`https://openagents.com${OmniApiSdkSeedEndpoint}`, {
        method,
      }),
    ),
  )

describe('Omni API SDK seed', () => {
  test('decodes the seed and covers required Omni surfaces', async () => {
    const seed = await Effect.runPromise(omniApiSdkSeed())

    expect(S.decodeUnknownSync(OmniApiSdkSeed)(seed)).toEqual(seed)
    expect(seed.schemaVersion).toBe('openagents.omni.sdk_seed.v1')
    expect(omniApiSdkSeedHasRequiredSurfaces(seed)).toBe(true)
    expect(seed.schemaCatalog.map(entry => entry.surface)).toEqual(
      expect.arrayContaining([
        'accepted_outcomes',
        'billing',
        'proof_bundles',
        'program_runs',
        'receipts',
        'webhooks',
        'workrooms',
      ]),
    )
    expect(seed.schemaCatalog).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          exportName: 'OmniWorkroomRecord',
          sourceModule: 'workers/api/src/omni-workrooms.ts',
        }),
        expect.objectContaining({
          exportName: 'OmniAcceptedOutcomeContractRecord',
          sourceModule: 'workers/api/src/omni-accepted-outcome-contracts.ts',
        }),
        expect.objectContaining({
          exportName: 'ProgramRunReceiptWebhookSubscriptionContract',
          status: 'contract_only',
        }),
      ]),
    )
  })

  test('classifies live, gated, contract-only, and planned routes without over-claiming authority', async () => {
    const seed = await Effect.runPromise(omniApiSdkSeed())

    expect(seed.routeCatalog).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          accessKind: 'public_read',
          operationId: 'getOmniApiSdkSeed',
          path: '/api/omni/sdk-seed',
          status: 'available',
        }),
        expect.objectContaining({
          accessKind: 'browser_session',
          method: 'POST',
          operationId: 'createOmniAgentRun',
          path: '/api/omni/agent-runs',
          status: 'available',
        }),
        expect.objectContaining({
          accessKind: 'browser_session',
          operationId: 'getBillingSummary',
          path: '/api/billing/summary',
          status: 'available',
        }),
        expect.objectContaining({
          accessKind: 'contract_only',
          operationId: 'createProgramRunReceiptWebhookSubscription',
          status: 'planned',
        }),
      ]),
    )
    expect(JSON.stringify(seed.routeCatalog)).toContain(
      'cannot send external webhooks',
    )
    expect(JSON.stringify(seed.routeCatalog)).toContain(
      'not available to public agents without a matching owner authority path',
    )
  })

  test('serves the seed as no-store public JSON and rejects unsafe methods', async () => {
    const response = await runRoute()
    const methodResponse = await runRoute('POST')
    const body = await response.json() as OmniApiSdkSeed

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(methodResponse.status).toBe(405)
    expect(body.schemaVersion).toBe('openagents.omni.sdk_seed.v1')
    expect(body.docs.openApi).toBe('https://openagents.com/api/openapi.json')
    expect(body.routeCatalog).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ operationId: 'getOmniApiSdkSeed' }),
      ]),
    )
  })

  test('does not include private payloads, provider secrets, raw payment evidence, or raw timestamps', async () => {
    const seed = await Effect.runPromise(omniApiSdkSeed())
    const serialized = JSON.stringify(seed)

    expect(seed).toEqual(OMNI_API_SDK_SEED)
    expect(omniApiSdkSeedIsPrivateDataSafe(seed)).toBe(true)
    expect(containsProviderSecretMaterial(serialized)).toBe(false)
    expect(serialized).not.toMatch(
      /(Bearer\s+[A-Za-z0-9._-]{8,}|ghp_[A-Za-z0-9_]+|gho_[A-Za-z0-9_]+|sk-[a-z0-9]|lnbc1|lntb1|lnbcrt1|preimage[:=]|payment[_-]?hash[:=]|github\.com\/[^:/]+\/private|[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/i,
    )
  })
})
