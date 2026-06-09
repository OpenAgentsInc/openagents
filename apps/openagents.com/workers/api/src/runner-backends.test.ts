import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsRunnerBackendRecord,
  OpenAgentsRunnerBackendProjection,
  OpenAgentsRunnerBackendRecord as OpenAgentsRunnerBackendRecordSchema,
  openAgentsRunnerBackendProjectionHasPrivateMaterial,
  projectOpenAgentsRunnerBackend,
} from './runner-backends'

const backend = (
  overrides: Partial<OpenAgentsRunnerBackendRecord> = {},
): OpenAgentsRunnerBackendRecord =>
  S.decodeUnknownSync(OpenAgentsRunnerBackendRecordSchema)({
    artifactRefs: ['artifact.container.preview_manifest'],
    backendKind: 'cloudflare_container',
    capacityRefs: ['capacity.container.low_medium_trust_available'],
    configured: false,
    costRefs: ['cost.container.metered.gated'],
    dispatchStatus: 'blocked',
    displayNameRef: 'runner.cloudflare_container',
    enabled: false,
    healthRefs: ['health.container.disabled_by_default'],
    id: 'runner_backend.cloudflare_container',
    lifecycleEventRefs: ['runner_event.container.blocked.not_enabled'],
    operatorDiagnosticRefs: [
      'diagnostic.container.policy_not_approved',
      'provider_account.redacted_capacity_ref',
    ],
    policyRefs: ['policy.runner.container.disabled_by_default'],
    publicSummaryRef: 'summary.runner.container_gated',
    receiptRefs: ['receipt.runner.container_schema_seed'],
    trustLevel: 'medium',
    ...overrides,
  })

describe('OpenAgents runner backend schemas and projections', () => {
  test('decodes the cloudflare_container backend schema', () => {
    const record = backend()

    expect(record.backendKind).toBe('cloudflare_container')
    expect(record.dispatchStatus).toBe('blocked')
  })

  test('supports SHC and reference backend kinds without changing dispatch policy', () => {
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerBackendRecordSchema)(
        backend({
          backendKind: 'shc_vm',
          id: 'runner_backend.shc_primary',
          trustLevel: 'sensitive',
        }),
      ).backendKind,
    ).toBe('shc_vm')
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerBackendRecordSchema)(
        backend({
          backendKind: 'gcloud_vm',
          id: 'runner_backend.gcloud_reference',
          trustLevel: 'sensitive',
        }),
      ).backendKind,
    ).toBe('gcloud_vm')
  })

  test('redacts customer projection refs and hides operator diagnostics', () => {
    const projection = projectOpenAgentsRunnerBackend(
      backend({
        artifactRefs: [
          'artifact.safe_manifest',
          'source_archive.private_bundle',
        ],
        healthRefs: [
          'health.safe',
          'raw_runner_log_private',
          'callback_token_abc',
        ],
        operatorDiagnosticRefs: ['diagnostic.operator_only'],
        receiptRefs: ['receipt.safe', 'provider_token_secret'],
      }),
      'customer',
    )

    expect(S.decodeUnknownSync(OpenAgentsRunnerBackendProjection)(projection))
      .toEqual(projection)
    expect(projection.artifactRefs).toEqual(['artifact.safe_manifest'])
    expect(projection.healthRefs).toEqual(['health.safe'])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.receiptRefs).toEqual(['receipt.safe'])
    expect(openAgentsRunnerBackendProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps operator-safe diagnostics while rejecting raw secrets', () => {
    const projection = projectOpenAgentsRunnerBackend(
      backend({
        operatorDiagnosticRefs: [
          'provider_account.redacted_capacity_ref',
          'diagnostic.container.cold_start_p95_redacted',
          'sk-secret-value',
          'runner_log.raw',
        ],
      }),
      'operator',
    )

    expect(projection.operatorDiagnosticRefs).toEqual([
      'provider_account.redacted_capacity_ref',
      'diagnostic.container.cold_start_p95_redacted',
    ])
    expect(openAgentsRunnerBackendProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })
})
