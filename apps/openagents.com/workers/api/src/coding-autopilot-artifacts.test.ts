import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotArtifactProjection,
  CodingAutopilotArtifactRecord,
  CodingAutopilotArtifactUnsafe,
  codingAutopilotArtifactProjectionHasPrivateMaterial,
  exampleCodingAutopilotArtifacts,
  projectCodingAutopilotArtifactRecord,
} from './coding-autopilot-artifacts'

const nowIso = '2026-06-06T21:05:00.000Z'

describe('Coding on Autopilot artifacts', () => {
  test('projects artifacts by visibility and audience', () => {
    const [publicDiff, customerTest, customerPr, teamBuild] =
      exampleCodingAutopilotArtifacts()
    const publicProjection = projectCodingAutopilotArtifactRecord(
      publicDiff!,
      'public',
      nowIso,
    )
    const publicCustomerArtifact = projectCodingAutopilotArtifactRecord(
      customerTest!,
      'public',
      nowIso,
    )
    const customerProjection = projectCodingAutopilotArtifactRecord(
      customerPr!,
      'customer',
      nowIso,
    )
    const teamProjection = projectCodingAutopilotArtifactRecord(
      teamBuild!,
      'team',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotArtifactRecord)(publicDiff))
      .toEqual(publicDiff)
    expect(S.decodeUnknownSync(CodingAutopilotArtifactProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      artifactKind: 'diff_summary',
      audience: 'public',
      createdAtDisplay: '35 minutes ago',
      status: 'ready',
      updatedAtDisplay: '5 minutes ago',
      visibility: 'public',
    })
    expect(publicProjection?.workroomRefs).toEqual([])
    expect(publicCustomerArtifact).toBe(null)
    expect(customerProjection?.authorityReceiptRefs).toEqual([
      'authority_receipt.github_writeback.otec_revision_4',
    ])
    expect(teamProjection?.artifactKind).toBe('build_log_summary')
    expect(teamProjection?.workroomRefs).toEqual([
      'workroom.otec_site_revision_4',
    ])
  })

  test('supports every required artifact kind', () => {
    const kinds = [
      'build_log_summary',
      'customer_note',
      'diff_summary',
      'fulfillment_receipt',
      'patch_ref',
      'pr_draft',
      'pr_url',
      'preview_url',
      'redaction_report',
      'rollback_note',
      'screenshot_ref',
      'test_run',
    ] as const
    const projections = kinds.map(artifactKind =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        artifactKind,
        artifactRef: `artifact.${artifactKind}.test`,
        authorityReceiptRefs: artifactKind === 'pr_draft' ||
          artifactKind === 'pr_url'
          ? ['authority_receipt.github_writeback.test']
          : [],
        evidenceRefs: ['evidence.artifact.test'],
        id: `artifact_${artifactKind}_test`,
        summaryRef: `summary.${artifactKind}.test`,
      }, 'customer', nowIso),
    )

    expect(projections.map(projection => projection?.artifactKind)).toEqual(kinds)
  })

  test('requires evidence for ready artifacts and authority receipts for PR artifacts', () => {
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        authorityReceiptRefs: [],
        evidenceRefs: [],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        artifactKind: 'pr_draft',
        authorityReceiptRefs: [],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        publicSafe: false,
        visibility: 'public',
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
  })

  test('does not expose raw timestamps and rejects unsafe refs', () => {
    const projection = projectCodingAutopilotArtifactRecord(
      exampleCodingAutopilotArtifacts()[0]!,
      'customer',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(codingAutopilotArtifactProjectionHasPrivateMaterial(projection!))
      .toBe(false)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        artifactRef: 'raw_build_log:mission',
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        artifactRef: 'raw_patch:private_repo',
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        sourceRefs: ['source_archive:repo'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        evidenceRefs: ['provider_token:abc'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
    expect(() =>
      projectCodingAutopilotArtifactRecord({
        ...exampleCodingAutopilotArtifacts()[0]!,
        caveatRefs: ['ben@example.com'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotArtifactUnsafe)
  })
})
