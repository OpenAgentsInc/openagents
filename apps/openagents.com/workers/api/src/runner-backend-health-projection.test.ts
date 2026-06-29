import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsRunnerBackendHealthSnapshot,
  OpenAgentsRunnerBackendHealthProjection,
  openAgentsRunnerBackendHealthProjectionHasPrivateMaterial,
  projectOpenAgentsRunnerBackendHealth,
} from './runner-backend-health-projection'

const snapshot = (
  overrides: Partial<OpenAgentsRunnerBackendHealthSnapshot> = {},
): OpenAgentsRunnerBackendHealthSnapshot => ({
  availability: 'degraded',
  backendKind: 'cloudflare_container',
  billingCaveatRefs: [
    'billing.container.metered_review_required',
    'billing.container.rate.5_cents_per_minute',
  ],
  capacityRefs: ['capacity.container.max_instances.2'],
  coldStartRefs: ['cold_start.container.p95.redacted'],
  configured: true,
  costTierRefs: ['cost.container.metered.gated'],
  enabled: true,
  gates: [
    {
      gateKind: 'enabled',
      operatorDiagnosticRef: 'diagnostic.container.enabled.true',
      passed: true,
      publicCaveatRef: 'caveat.container.enabled',
    },
    {
      gateKind: 'staging_smoke',
      operatorDiagnosticRef: 'diagnostic.container.staging_smoke.false',
      passed: false,
      publicCaveatRef: 'caveat.container.review_required',
    },
    {
      gateKind: 'operator_approval',
      operatorDiagnosticRef: 'diagnostic.container.approval.pending',
      passed: false,
      publicCaveatRef: 'caveat.container.operator_review_required',
    },
  ],
  healthRefs: ['health.container.degraded'],
  operatorDiagnosticRefs: [
    'diagnostic.container.queue_depth_ref.redacted',
    'raw_runner_log.full_text',
    'provider_token.raw',
  ],
  publicSummaryRef: 'summary.container.degraded_review_required',
  queueDepthRefs: ['queue.container.depth_bucket.low'],
  smokeRefs: ['smoke.container.pending'],
  ...overrides,
})

describe('runner backend health projection', () => {
  test('keeps public projection to safe high-level availability and caveats', () => {
    const projection = projectOpenAgentsRunnerBackendHealth(
      snapshot(),
      'public',
    )
    const text = JSON.stringify(projection)

    expect(S.decodeUnknownSync(OpenAgentsRunnerBackendHealthProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      audience: 'public',
      availability: 'degraded',
      backendKind: 'cloudflare_container',
      billingCaveatRefs: [
        'caveat.container.review_required',
        'caveat.container.operator_review_required',
      ],
      capacityRefs: [],
      coldStartRefs: [],
      configured: false,
      costTierRefs: [],
      enabled: false,
      gateRefs: [
        'caveat.container.review_required',
        'caveat.container.operator_review_required',
      ],
      healthRefs: [],
      operatorDiagnosticRefs: [],
      queueDepthRefs: [],
      smokeRefs: [],
    })
    expect(text).not.toContain('queue.container')
    expect(text).not.toContain('cold_start')
    expect(text).not.toContain('metered')
    expect(text).not.toContain('raw_runner_log')
    expect(text).not.toContain('provider_token')
    expect(text).not.toMatch(/failover/i)
    expect(openAgentsRunnerBackendHealthProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps customer projection aligned with public-safe caveats', () => {
    const projection = projectOpenAgentsRunnerBackendHealth(
      snapshot({ availability: 'blocked' }),
      'customer',
    )

    expect(projection.audience).toBe('customer')
    expect(projection.availability).toBe('blocked')
    expect(projection.billingCaveatRefs).toEqual([
      'caveat.container.review_required',
      'caveat.container.operator_review_required',
    ])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(openAgentsRunnerBackendHealthProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('operator projection includes redacted diagnostics, billing, capacity, health, and cost refs', () => {
    const projection = projectOpenAgentsRunnerBackendHealth(
      snapshot(),
      'operator',
    )

    expect(projection).toMatchObject({
      audience: 'operator',
      availability: 'degraded',
      billingCaveatRefs: [
        'billing.container.metered_review_required',
        'billing.container.rate.5_cents_per_minute',
      ],
      capacityRefs: ['capacity.container.max_instances.2'],
      coldStartRefs: ['cold_start.container.p95.redacted'],
      configured: true,
      costTierRefs: ['cost.container.metered.gated'],
      enabled: true,
      healthRefs: ['health.container.degraded'],
      operatorDiagnosticRefs: ['diagnostic.container.queue_depth_ref.redacted'],
      queueDepthRefs: ['queue.container.depth_bucket.low'],
      smokeRefs: ['smoke.container.pending'],
    })
    expect(projection.gateRefs).toEqual([
      'gate.runner_backend.enabled.passed',
      'gate.runner_backend.staging_smoke.blocked',
      'gate.runner_backend.operator_approval.blocked',
    ])
    expect(openAgentsRunnerBackendHealthProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('operator projection strips unsafe refs and does not include failover policy', () => {
    const projection = projectOpenAgentsRunnerBackendHealth(
      snapshot({
        billingCaveatRefs: [
          'billing.safe',
          'source_archive.private_bundle',
        ],
        capacityRefs: ['capacity.safe', 'sk-secret-value'],
        operatorDiagnosticRefs: [
          'diagnostic.safe',
          'runner_log.raw',
          'failover.policy.internal',
        ],
      }),
      'operator',
    )
    const text = JSON.stringify(projection)

    expect(projection.billingCaveatRefs).toEqual(['billing.safe'])
    expect(projection.capacityRefs).toEqual(['capacity.safe'])
    expect(projection.operatorDiagnosticRefs).toEqual(['diagnostic.safe'])
    expect(text).not.toMatch(/failover/i)
    expect(openAgentsRunnerBackendHealthProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })
})
