import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BlueprintSignatureContributionDraft,
  BlueprintSignatureContributionProjection,
} from '../schemas/signature-contribution'
import {
  BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
  blueprintSignatureContributionDraftBlockerRefs,
  blueprintSignatureContributionDraftCanEnterReleaseGate,
  blueprintSignatureContributionDraftHasRuntimeAuthority,
  blueprintSignatureContributionDraftRuntimeEffectDeniedRefs,
  blueprintSignatureContributionProjectionHasPrivateMaterial,
  projectBlueprintSignatureContributionDraft,
} from './signature-contribution'

const draft = (
  overrides: Partial<BlueprintSignatureContributionDraft> = {},
): BlueprintSignatureContributionDraft =>
  S.decodeUnknownSync(BlueprintSignatureContributionDraft)({
    authority: BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
    capabilitySummaryRef: 'summary.signature_contribution.continuation_review',
    contributorRefs: ['contributor.agent.openagents_smoke'],
    createdAt: '2026-06-06T00:00:00.000Z',
    id: 'signature_contribution.continuation_review.v1',
    intendedFamily: 'continuation',
    promotionRef: null,
    proposedModuleVersionRef:
      'module_version.autopilot.continue.candidate_2',
    proposedProgramSignatureRef:
      'program_signature.autopilot.continue.v2',
    proposedProgramTypeRef: 'program_type.autopilot.continue',
    rejectionRef: null,
    releaseGateRefs: ['release_gate.autopilot.continue.v2'],
    requiredFixtureRefs: ['fixture.continuation.continue.v2'],
    reviewStatus: 'approved',
    riskClass: 'medium',
    sourceRefs: ['source.marketplace.contributor_package'],
    status: 'approved_for_release_gate',
    updatedAt: '2026-06-06T00:00:00.000Z',
    ...overrides,
  })

describe('Blueprint Program Signature contribution drafts', () => {
  test('decodes approved-for-release-gate drafts without runtime authority', () => {
    const record = draft()
    const projection = projectBlueprintSignatureContributionDraft(record)

    expect(S.decodeUnknownSync(BlueprintSignatureContributionProjection)(
      projection,
    )).toEqual(projection)
    expect(blueprintSignatureContributionDraftHasRuntimeAuthority(record)).toBe(
      false,
    )
    expect(blueprintSignatureContributionDraftCanEnterReleaseGate(record)).toBe(
      true,
    )
    expect(projection.nonAuthoritative).toBe(true)
    expect(projection.authority).toEqual(
      BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
    )
  })

  test('denies every runtime effect for contribution drafts', () => {
    const denied = blueprintSignatureContributionDraftRuntimeEffectDeniedRefs(
      draft(),
    )

    expect(denied).toEqual([
      'effect.execute',
      'effect.mutate',
      'effect.deploy',
      'effect.spend',
      'effect.send_email',
      'effect.change_public_claims',
    ])
  })

  test('blocks unreviewed or incomplete contribution drafts from release gates', () => {
    const incomplete = draft({
      proposedModuleVersionRef: null,
      proposedProgramSignatureRef: null,
      releaseGateRefs: [],
      requiredFixtureRefs: [],
      reviewStatus: 'pending',
      status: 'submitted',
    })

    expect(blueprintSignatureContributionDraftCanEnterReleaseGate(incomplete))
      .toBe(false)
    expect(blueprintSignatureContributionDraftBlockerRefs(incomplete)).toEqual(
      expect.arrayContaining([
        'blocker.signature_contribution.review_not_approved',
        'blocker.signature_contribution.not_release_gate_ready',
        'blocker.signature_contribution.fixture_refs_missing',
        'blocker.signature_contribution.release_gate_refs_missing',
        'blocker.signature_contribution.target_ref_missing',
      ]),
    )
  })

  test('detects accidental runtime authority and keeps projections explicit', () => {
    const unsafeDraft = draft({
      authority: {
        ...BLUEPRINT_SIGNATURE_CONTRIBUTION_NO_AUTHORITY,
        canExecute: true,
      },
    })
    const projection = projectBlueprintSignatureContributionDraft(unsafeDraft)

    expect(blueprintSignatureContributionDraftHasRuntimeAuthority(unsafeDraft))
      .toBe(true)
    expect(blueprintSignatureContributionDraftCanEnterReleaseGate(unsafeDraft))
      .toBe(false)
    expect(blueprintSignatureContributionDraftBlockerRefs(unsafeDraft))
      .toContain('blocker.signature_contribution.runtime_authority_present')
    expect(projection.nonAuthoritative).toBe(false)
  })

  test('redacts unsafe contribution refs from projections', () => {
    const projection = projectBlueprintSignatureContributionDraft(
      draft({
        contributorRefs: ['customer_email_ben@example.com'],
        sourceRefs: [
          'raw_run_log_private',
          '2026-06-06T00:00:00.000Z',
          'source.public_safe',
        ],
      }),
    )

    expect(projection.contributorRefs).toEqual([])
    expect(projection.sourceRefs).toEqual(['source.public_safe'])
    expect(blueprintSignatureContributionProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })
})
