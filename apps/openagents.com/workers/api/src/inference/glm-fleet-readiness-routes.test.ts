import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleGlmFleetReadiness } from './glm-fleet-readiness-routes'

const env = {
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS: 'primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BASE_URL:
    'https://primary.private.example.test',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_BEARER_TOKEN: 'secret-primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_PREFLIGHT_REF:
    'preflight.hydralisk.glm.primary',
  HYDRALISK_GLM_52_REAP_504B_PRIMARY_RECEIPT_REF:
    'receipt.hydralisk.glm.primary',
} as const

const get = () =>
  new Request('https://openagents.com/v1/gateway/glm-fleet/readiness', {
    method: 'GET',
  })

describe('handleGlmFleetReadiness', () => {
  test('404s with inference_gateway_disabled when the gateway is off', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), { enabled: false, env }),
    )

    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })

  test('405s on non-GET requests', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(
        new Request('https://openagents.com/v1/gateway/glm-fleet/readiness', {
          method: 'POST',
        }),
        { enabled: true, env },
      ),
    )

    expect(response.status).toBe(405)
    expect(await response.json()).toEqual({ error: 'method_not_allowed' })
  })

  test('returns a no-store public-safe fleet projection', async () => {
    const response = await Effect.runPromise(
      handleGlmFleetReadiness(get(), { enabled: true, env }),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const text = await response.text()
    expect(text).toContain('replica.hydralisk.glm_52_reap_504b.primary')
    expect(text).toContain('unavailableReplicaCount')
    expect(text).not.toContain('private.example.test')
    expect(text).not.toContain('secret-primary')
    expect(text).not.toContain('PRIMARY_BASE_URL')
  })
})
