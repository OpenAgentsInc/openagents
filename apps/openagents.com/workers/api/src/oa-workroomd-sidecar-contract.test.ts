import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsWorkroomdSessionProjection,
  OpenAgentsWorkroomdSessionRecord,
  OpenAgentsWorkroomdSessionUnsafe,
  openAgentsWorkroomdSessionCloseoutReady,
  openAgentsWorkroomdSessionHasOnlyGrantRefs,
  openAgentsWorkroomdSessionPreservesAuditEvidence,
  openAgentsWorkroomdSessionProjectionHasPrivateMaterial,
  projectOpenAgentsWorkroomdSession,
} from './oa-workroomd-sidecar-contract'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T02:00:00.000Z'

const sessionRecord = (
  overrides: Partial<OpenAgentsWorkroomdSessionRecord> = {},
): OpenAgentsWorkroomdSessionRecord =>
  S.decodeUnknownSync(OpenAgentsWorkroomdSessionRecord)({
    archiveState: 'active',
    artifactManifestRefs: ['artifact_manifest.otec.codex_turn_1'],
    artifactRefs: ['artifact.otec.diff_summary'],
    assignmentRef: 'assignment.otec.site_revision',
    auditEvidenceRefs: ['audit_evidence.otec.session_1'],
    backendKind: 'shc_vm',
    cancellationRefs: [],
    cancellationState: 'none',
    closeoutCaveatRefs: ['caveat.closeout.customer_review_needed'],
    closeoutReceiptRefs: [],
    closeoutState: 'pending',
    correlationRefs: ['correlation.otec.session_1'],
    createdAtIso: '2026-06-07T01:50:00.000Z',
    daemonRef: 'daemon.oa_workroomd.bertha',
    eventKinds: [
      'session_created',
      'assignment_received',
      'grant_refs_resolved',
      'turn_started',
      'artifact_manifest_recorded',
    ],
    failureReceiptRefs: [],
    grantRefs: [
      'auth_grant.codex.account_1',
      'github_write_grant.otec.repo',
    ],
    grantResolutionRefs: ['grant_resolution.refs_present'],
    grantResolutionState: 'refs_present',
    id: 'oa_workroomd_session.otec.1',
    idempotencyRefs: ['idempotency.otec.session_1'],
    lifecycleEventRefs: ['lifecycle.otec.turn_started'],
    nodeRef: 'oa_node.bertha',
    policyRefs: ['policy.oa_workroomd.no_raw_credentials'],
    publicArtifactRefs: ['artifact.public.otec.preview'],
    routeRefs: ['route.shc_vm.codex'],
    runtimeRef: 'runtime.codex_cli',
    sessionRef: 'session.otec.1',
    sourceAuthorityRefs: ['source_authority.order.customer_summary'],
    status: 'closeout_ready',
    trustLevel: 'medium',
    updatedAtIso: '2026-06-07T01:58:00.000Z',
    workroomRef: 'workroom.otec.public',
    workspaceRef: 'workspace.otec.session_1',
    ...overrides,
  })

describe('OpenAgents oa-workroomd sidecar contract', () => {
  test('projects session state while hiding grant refs from public audiences', () => {
    const record = sessionRecord()
    const projection = projectOpenAgentsWorkroomdSession(
      record,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OpenAgentsWorkroomdSessionProjection)(
      projection,
    )).toEqual(projection)
    expect(openAgentsWorkroomdSessionHasOnlyGrantRefs(record)).toBe(true)
    expect(openAgentsWorkroomdSessionCloseoutReady(record)).toBe(true)
    expect(projection.grantRefs).toEqual([])
    expect(projection.grantResolutionRefs).toEqual([])
    expect(projection.auditEvidencePreserved).toBe(true)
    expect(projection.createdAtDisplay).toBe('10 minutes ago')
    expect(projection.updatedAtDisplay).toBe('2 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsWorkroomdSessionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('operator projections can show safe grant refs without raw credentials', () => {
    const projection = projectOpenAgentsWorkroomdSession(
      sessionRecord(),
      'operator',
      nowIso,
    )

    expect(projection.grantRefs).toEqual([
      'auth_grant.codex.account_1',
      'github_write_grant.otec.repo',
    ])
    expect(projection.grantResolutionRefs).toEqual([
      'grant_resolution.refs_present',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('models cancellation, archive, destroy, and closeout without deleting audit evidence', () => {
    const cancelled = sessionRecord({
      archiveState: 'archived',
      cancellationRefs: ['cancel.otec.operator_request'],
      cancellationState: 'cancelled',
      closeoutReceiptRefs: ['receipt.closeout.cancelled'],
      closeoutState: 'emitted',
      eventKinds: [
        'cancellation_requested',
        'cancellation_acknowledged',
        'closeout_emitted',
        'archive_recorded',
      ],
      status: 'archived',
    })
    const destroyed = sessionRecord({
      archiveState: 'destroyed',
      closeoutReceiptRefs: ['receipt.closeout.destroyed'],
      closeoutState: 'emitted',
      eventKinds: ['closeout_emitted', 'archive_recorded', 'destroy_recorded'],
      status: 'destroyed',
    })

    expect(openAgentsWorkroomdSessionPreservesAuditEvidence(cancelled)).toBe(
      true,
    )
    expect(openAgentsWorkroomdSessionPreservesAuditEvidence(destroyed)).toBe(
      true,
    )
    expect(projectOpenAgentsWorkroomdSession(
      cancelled,
      'customer',
      nowIso,
    ).status).toBe('archived')
    expect(projectOpenAgentsWorkroomdSession(
      destroyed,
      'customer',
      nowIso,
    ).archiveState).toBe('destroyed')
  })

  test('rejects raw credentials, provider payloads, local paths, raw logs, private repos, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'raw auth', value: 'auth_content_json.raw' },
      { label: 'local path', value: '/Users/chris/work/private' },
    ]) {
      expect(() =>
        projectOpenAgentsWorkroomdSession(
          sessionRecord({ grantRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(OpenAgentsWorkroomdSessionUnsafe)
    }
  })
})
