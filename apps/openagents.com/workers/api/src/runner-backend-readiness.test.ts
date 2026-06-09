import { describe, expect, test } from 'vitest'

import type { RunnerBackendConfig } from './config'
import {
  runnerBackendReadinessCheck,
  runnerWorkloadTrustFromSelector,
} from './runner-backend-readiness'

const baseConfig: RunnerBackendConfig = {
  automaticFailoverEnabled: false,
  cloudflareContainer: {
    allowedWorkloadTrusts: ['low', 'medium'],
    binding: {},
    configured: false,
    enabled: false,
    policyApproved: false,
    stagingSmokePassed: false,
  },
  gcloud: {
    referenceEnabled: false,
    sensitiveApproved: false,
  },
  policy: 'shc_primary_only',
}

const configuredContainer = (): RunnerBackendConfig['cloudflareContainer'] => ({
  allowedWorkloadTrusts: ['low', 'medium'],
  binding: {
    className: 'OpenAgentsSiteRunnerContainer',
    durableObjectBinding: 'SITE_RUNNER_CONTAINER',
    imageRef: './containers/site-runner/Dockerfile',
    instanceType: 'lite',
    maxInstances: 2,
  },
  configured: true,
  enabled: true,
  policyApproved: true,
  stagingSmokePassed: true,
})

describe('runner backend readiness', () => {
  test('keeps SHC primary-only readiness ok without enabling failover', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: baseConfig,
      shcControlStatus: 'ok',
      workloadTrust: 'low',
    })

    expect(check).toMatchObject({
      name: 'runner_backends',
      status: 'ok',
    })
    expect(check.details).toMatchObject({
      automaticFailover: {
        effective: false,
        requested: false,
      },
      lanes: {
        shc_primary: {
          ready: true,
        },
      },
      policy: 'shc_primary_only',
    })
  })

  test('marks low-trust work eligible for Container backup when smoke and approval pass', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: {
        ...baseConfig,
        automaticFailoverEnabled: true,
        cloudflareContainer: configuredContainer(),
        policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
      },
      shcControlStatus: 'ok',
      workloadTrust: 'medium',
    })

    expect(check.status).toBe('ok')
    expect(check.details).toMatchObject({
      automaticFailover: {
        effective: true,
        requested: true,
      },
      lanes: {
        cloudflare_container_backup: {
          eligibleForWorkload: true,
          ready: true,
        },
      },
      workloadTrust: 'medium',
    })
  })

  test('keeps sensitive work off Container backup and reports missing reference approval', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: {
        ...baseConfig,
        cloudflareContainer: configuredContainer(),
        gcloud: {
          referenceEnabled: true,
          sensitiveApproved: false,
        },
        policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
      },
      shcControlStatus: 'ok',
      workloadTrust: 'sensitive',
    })

    expect(check.status).toBe('warning')
    expect(check.details).toMatchObject({
      lanes: {
        cloudflare_container_backup: {
          eligibleForWorkload: false,
          ready: false,
        },
        gcloud_reference: {
          ready: false,
          sensitiveApproved: false,
        },
      },
    })
  })

  test('blocks automatic failover until all Container prerequisites exist', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: {
        ...baseConfig,
        automaticFailoverEnabled: true,
        cloudflareContainer: {
          ...configuredContainer(),
          enabled: true,
          policyApproved: false,
        },
        policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
      },
      shcControlStatus: 'ok',
      workloadTrust: 'low',
    })

    expect(check).toMatchObject({
      message:
        'Automatic runner failover requires Container enablement, binding configuration, staging smoke, and policy approval.',
      status: 'blocked',
    })
    expect(check.details).toMatchObject({
      automaticFailover: {
        effective: false,
        requested: true,
      },
    })
  })

  test.each([
    [
      'missing binding',
      {
        binding: {},
      },
    ],
    [
      'missing approval',
      {
        policyApproved: false,
      },
    ],
    [
      'failed smoke',
      {
        stagingSmokePassed: false,
      },
    ],
  ])('blocks Container automatic failover for %s', (_, override) => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: {
        ...baseConfig,
        automaticFailoverEnabled: true,
        cloudflareContainer: {
          ...configuredContainer(),
          ...override,
        },
        policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
      },
      shcControlStatus: 'ok',
      workloadTrust: 'low',
    })

    expect(check.status).toBe('blocked')
    expect(check.details).toMatchObject({
      automaticFailover: {
        effective: false,
        requested: true,
      },
      lanes: {
        cloudflare_container_backup: {
          ready: false,
        },
      },
    })
  })

  test('blocks Container readiness when workload trust is not allowed', () => {
    const check = runnerBackendReadinessCheck({
      callbackStatus: 'ok',
      config: {
        ...baseConfig,
        cloudflareContainer: configuredContainer(),
        policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
      },
      shcControlStatus: 'ok',
      workloadTrust: 'sensitive',
    })

    expect(check.status).toBe('warning')
    expect(check.details).toMatchObject({
      lanes: {
        cloudflare_container_backup: {
          allowedWorkloadTrusts: ['low', 'medium'],
          eligibleForWorkload: false,
          ready: false,
        },
      },
    })
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
