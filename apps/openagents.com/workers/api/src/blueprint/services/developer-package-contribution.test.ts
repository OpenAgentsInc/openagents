import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BlueprintDeveloperPackageContributionProjection,
  BlueprintDeveloperPackageContributionRecord,
} from '../schemas/developer-package-contribution'
import {
  BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
  blueprintDeveloperPackageContributionBlockerRefs,
  blueprintDeveloperPackageContributionCanEnterReleaseGate,
  blueprintDeveloperPackageContributionHasRuntimeAuthority,
  blueprintDeveloperPackageContributionProjectionHasPrivateMaterial,
  blueprintDeveloperPackageContributionRuntimeEffectDeniedRefs,
  projectBlueprintDeveloperPackageContribution,
} from './developer-package-contribution'

const contribution = (
  overrides: Partial<BlueprintDeveloperPackageContributionRecord> = {},
): BlueprintDeveloperPackageContributionRecord =>
  S.decodeUnknownSync(BlueprintDeveloperPackageContributionRecord)({
    authority: BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
    backendProjectionAdapterRefs: ['adapter.probe.apple_fm.blueprint_tools.v1'],
    capabilityFamily: 'program_signature',
    capabilitySummaryRef:
      'summary.developer_package.autopilot_continuation_review',
    contextPackageRefs: ['context_package.autopilot.site_review_sources'],
    contributorRefs: ['contributor.agent.openagents_smoke'],
    createdAt: '2026-06-06T00:00:00.000Z',
    dogfoodScopeRef: 'dogfood.autopilot.continue.candidate_only',
    id: 'developer_package.autopilot.continuation_review.v1',
    intendedProgramFamily: 'continuation',
    noProductionRuntimeAuthority: true,
    outcomeTemplateRefs: ['outcome_template.site_revision_review'],
    paymentAttributionRefs: ['payment_attribution.promoted_package.v1'],
    promotionRef: null,
    proposedModuleVersionRefs: [
      'module_version.autopilot.continue.candidate_3',
    ],
    proposedProgramSignatureRefs: ['program_signature.autopilot.continue.v3'],
    proposedProgramTypeRefs: ['program_type.autopilot.continue'],
    rejectionRef: null,
    releaseGateRefs: ['release_gate.autopilot.continue.v3'],
    requiredFixtureRefs: ['fixture.continuation.continue.v3'],
    retainedFailureRefs: ['failure.continuation.continue.v3.fixture_1'],
    reviewStatus: 'approved',
    riskClass: 'medium',
    selfPromotionAttempt: false,
    sourceRefs: ['source.developer_package.public_manifest'],
    status: 'approved_for_release_gate',
    toolPackageRefs: ['tool_package.probe.repo_read_tools.v1'],
    uiBindingRefs: ['ui_binding.decision_queue.continue_v3'],
    updatedAt: '2026-06-06T00:00:00.000Z',
    ...overrides,
  })

describe('Blueprint developer package contributions', () => {
  test('decodes package contributions and projects release-gate readiness without runtime authority', () => {
    const record = contribution()
    const projection = projectBlueprintDeveloperPackageContribution(record)

    expect(
      S.decodeUnknownSync(BlueprintDeveloperPackageContributionProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(
      blueprintDeveloperPackageContributionHasRuntimeAuthority(record),
    ).toBe(false)
    expect(
      blueprintDeveloperPackageContributionCanEnterReleaseGate(record),
    ).toBe(true)
    expect(projection).toMatchObject({
      capabilityFamily: 'program_signature',
      nonAuthoritative: true,
      releaseGateReady: true,
    })
    expect(projection.contextPackageRefs).toEqual([
      'context_package.autopilot.site_review_sources',
    ])
    expect(projection.backendProjectionAdapterRefs).toEqual([
      'adapter.probe.apple_fm.blueprint_tools.v1',
    ])
    expect(projection.outcomeTemplateRefs).toEqual([
      'outcome_template.site_revision_review',
    ])
    expect(projection.toolPackageRefs).toEqual([
      'tool_package.probe.repo_read_tools.v1',
    ])
    expect(projection.uiBindingRefs).toEqual([
      'ui_binding.decision_queue.continue_v3',
    ])
  })

  test('denies deploy, spend, email, repository mutation, public posting, Site creation, and runtime dispatch', () => {
    const denied =
      blueprintDeveloperPackageContributionRuntimeEffectDeniedRefs(
        contribution(),
      )

    expect(denied).toEqual([
      'effect.execute',
      'effect.dispatch_runtime',
      'effect.deploy',
      'effect.spend',
      'effect.send_email',
      'effect.mutate_repository',
      'effect.post_publicly',
      'effect.create_site',
      'effect.change_public_claims',
    ])
  })

  test('blocks unreviewed, incomplete, already-promoted, rejected, and authoritative contributions', () => {
    const blocked = contribution({
      authority: {
        ...BLUEPRINT_DEVELOPER_PACKAGE_CONTRIBUTION_NO_AUTHORITY,
        canCreateSite: true,
        canDispatchRuntime: true,
        canMutateRepository: true,
        canPostPublicly: true,
      },
      backendProjectionAdapterRefs: [],
      contextPackageRefs: [],
      outcomeTemplateRefs: [],
      promotionRef: 'promotion.already_done',
      proposedModuleVersionRefs: [],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      rejectionRef: 'rejection.needs_safe_source',
      releaseGateRefs: [],
      requiredFixtureRefs: [],
      reviewStatus: 'pending',
      selfPromotionAttempt: true,
      status: 'submitted',
      toolPackageRefs: [],
      uiBindingRefs: [],
    })

    expect(
      blueprintDeveloperPackageContributionCanEnterReleaseGate(blocked),
    ).toBe(false)
    expect(blueprintDeveloperPackageContributionBlockerRefs(blocked)).toEqual([
      'blocker.developer_package_contribution.already_promoted',
      'blocker.developer_package_contribution.fixture_refs_missing',
      'blocker.developer_package_contribution.not_release_gate_ready',
      'blocker.developer_package_contribution.rejected',
      'blocker.developer_package_contribution.release_gate_refs_missing',
      'blocker.developer_package_contribution.review_not_approved',
      'blocker.developer_package_contribution.runtime_authority_present',
      'blocker.developer_package_contribution.self_promotion_attempt',
      'blocker.developer_package_contribution.target_ref_missing',
    ])
  })

  test('allows package-shaped release gates when context, outcome, tool, backend adapter, or UI package refs are the target', () => {
    const packageOnly = contribution({
      capabilityFamily: 'context_package',
      backendProjectionAdapterRefs: [],
      contextPackageRefs: ['context_package.autopilot.safe_context'],
      outcomeTemplateRefs: [],
      proposedModuleVersionRefs: [],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      toolPackageRefs: [],
      uiBindingRefs: [],
    })
    const toolPackageOnly = contribution({
      backendProjectionAdapterRefs: [],
      capabilityFamily: 'tool_package',
      contextPackageRefs: [],
      outcomeTemplateRefs: [],
      proposedModuleVersionRefs: [],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      toolPackageRefs: ['tool_package.probe.repo_read_tools.v1'],
      uiBindingRefs: [],
    })
    const adapterOnly = contribution({
      backendProjectionAdapterRefs: ['adapter.probe.apple_fm.tools.v1'],
      capabilityFamily: 'backend_projection_adapter',
      contextPackageRefs: [],
      outcomeTemplateRefs: [],
      proposedModuleVersionRefs: [],
      proposedProgramSignatureRefs: [],
      proposedProgramTypeRefs: [],
      toolPackageRefs: [],
      uiBindingRefs: [],
    })

    expect(
      blueprintDeveloperPackageContributionCanEnterReleaseGate(packageOnly),
    ).toBe(true)
    expect(
      blueprintDeveloperPackageContributionCanEnterReleaseGate(toolPackageOnly),
    ).toBe(true)
    expect(
      blueprintDeveloperPackageContributionCanEnterReleaseGate(adapterOnly),
    ).toBe(true)
    expect(
      projectBlueprintDeveloperPackageContribution(packageOnly),
    ).toMatchObject({
      capabilityFamily: 'context_package',
      releaseGateReady: true,
    })
  })

  test('redacts unsafe package refs from projections', () => {
    const projection = projectBlueprintDeveloperPackageContribution(
      contribution({
        backendProjectionAdapterRefs: ['provider_payload.raw'],
        contextPackageRefs: [
          'raw_prompt.private_context',
          'context_package.public_safe',
        ],
        contributorRefs: ['customer_email_ben@example.com'],
        outcomeTemplateRefs: ['source_archive.private_zip'],
        proposedProgramSignatureRefs: ['github.com/acme/private/repo'],
        sourceRefs: ['provider_payload.raw', 'source.public_safe'],
        toolPackageRefs: ['tool_package.public_safe', 'raw_runner.private'],
        uiBindingRefs: ['ui_binding.public_safe', 'token.hidden'],
      }),
    )

    expect(projection.backendProjectionAdapterRefs).toEqual([])
    expect(projection.contextPackageRefs).toEqual([
      'context_package.public_safe',
    ])
    expect(projection.contributorRefs).toEqual([])
    expect(projection.outcomeTemplateRefs).toEqual([])
    expect(projection.proposedProgramSignatureRefs).toEqual([])
    expect(projection.sourceRefs).toEqual(['source.public_safe'])
    expect(projection.toolPackageRefs).toEqual(['tool_package.public_safe'])
    expect(projection.uiBindingRefs).toEqual(['ui_binding.public_safe'])
    expect(
      blueprintDeveloperPackageContributionProjectionHasPrivateMaterial(
        projection,
      ),
    ).toBe(false)
  })
})
