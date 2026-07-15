import { describe, expect, test } from 'vitest'

import {
  runnerBackendReadinessCheck,
  runnerWorkloadTrustFromSelector,
} from './runner-backend-readiness'

describe('runner backend readiness', () => {
  test('reports Google Cloud as the sole ready runtime', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      gcloudControlStatus: 'ok',
      workloadTrust: 'sensitive',
    })

    expect(check).toMatchObject({
      message: 'Google Cloud runner and callback paths are ready.',
      status: 'ok',
    })
    expect(check.details).toMatchObject({
      lanes: { google_cloud: { ready: true, role: 'sole_runtime' } },
    })
  })

  test('blocks when Google Cloud control is unavailable', () => {
    expect(
      runnerBackendReadinessCheck({
        callbackStatus: 'ok',
        gcloudControlStatus: 'blocked',
        workloadTrust: 'low',
      }).status,
    ).toBe('blocked')
  })

  test('parses bounded workload trust selector fields', () => {
    expect(
      runnerWorkloadTrustFromSelector({ runnerWorkloadTrust: 'sensitive' }),
    ).toBe('sensitive')
    expect(runnerWorkloadTrustFromSelector({ workloadTrust: 'medium' })).toBe(
      'medium',
    )
    expect(runnerWorkloadTrustFromSelector({ siteTrustLevel: 'unknown' })).toBe(
      'low',
    )
  })
})
