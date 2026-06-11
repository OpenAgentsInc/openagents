import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotApertureContractUnsafe,
  AutopilotMissionWorkOrderLink,
  AutopilotPlacementExplanation,
  AutopilotWritebackPlan,
  assertAutopilotMissionWorkOrderLinksAreOneToOne,
  planAutopilotWorkOrderWriteback,
  projectAutopilotPlacementExplanation,
  validateAutopilotMissionWorkOrderLink,
} from './autopilot-aperture-contracts'
import { evaluateCodingAutopilotRepoPlacement } from './coding-autopilot-repo-placement'
import { resolveGitHubWritebackAuthority } from './github-writeback-authority'

const nowIso = '2026-06-11T22:10:00.000Z'

const missionWorkOrderLink = (
  input: Partial<AutopilotMissionWorkOrderLink> = {},
): AutopilotMissionWorkOrderLink => ({
  artifactRefs: ['artifact.diff_summary.aperture_1'],
  briefingRefs: ['briefing.mission.aperture_1'],
  canonicalRef: 'mission_work_order.aperture_1',
  continuationRefs: ['continuation.cross_runner.aperture_1'],
  createdAtIso: '2026-06-11T22:00:00.000Z',
  dataScopeRef: 'data_scope.repo_openagents_docs',
  decisionActionRefs: ['decision_action.review_pr.aperture_1'],
  frontDoor: 'agent_api',
  id: 'mission_work_order_link.aperture_1',
  missionRef: 'mission.aperture_1',
  placementRecordRef: 'repo_placement.aperture_1',
  receiptRefs: ['receipt.work_order.accepted.aperture_1'],
  status: 'waiting_for_review',
  updatedAtIso: '2026-06-11T22:05:00.000Z',
  workOrderRef: 'work_order.aperture_1',
  ...input,
})

const publicPlacementRecord = () =>
  evaluateCodingAutopilotRepoPlacement({
    customerGrantRefs: [],
    dataClassification: 'public',
    evaluatedAtIso: '2026-06-11T22:00:00.000Z',
    evidenceRefs: ['evidence.repo_placement.aperture_1'],
    id: 'repo_placement.aperture_1',
    missionRef: 'mission.aperture_1',
    operatorApprovalRefs: [],
    providerGrantRefs: [],
    publicProofProjectionRefs: ['public_proof.repo_placement.aperture_1'],
    repoRef: 'repo.github.OpenAgentsInc.openagents',
    runnerBackendKind: 'cloudflare_container',
    runnerWorkloadTrust: 'low',
    trustTier: 'public',
    workroomRefs: ['workroom.aperture_1'],
  })

describe('Autopilot Aperture contracts', () => {
  test('validates one mission to one work-order linkage with shared refs', () => {
    const link = validateAutopilotMissionWorkOrderLink(
      missionWorkOrderLink({
        artifactRefs: [
          'artifact.diff_summary.aperture_1',
          'artifact.diff_summary.aperture_1',
          'artifact.test_run.aperture_1',
        ],
      }),
    )

    expect(S.decodeUnknownSync(AutopilotMissionWorkOrderLink)(link)).toEqual(
      link,
    )
    expect(link).toMatchObject({
      canonicalRef: 'mission_work_order.aperture_1',
      dataScopeRef: 'data_scope.repo_openagents_docs',
      placementRecordRef: 'repo_placement.aperture_1',
      workOrderRef: 'work_order.aperture_1',
    })
    expect(link.artifactRefs).toEqual([
      'artifact.diff_summary.aperture_1',
      'artifact.test_run.aperture_1',
    ])
    expect(() =>
      assertAutopilotMissionWorkOrderLinksAreOneToOne([
        link,
        missionWorkOrderLink({
          canonicalRef: 'mission_work_order.aperture_2',
          id: 'mission_work_order_link.aperture_2',
          missionRef: 'mission.aperture_2',
          workOrderRef: 'work_order.aperture_1',
        }),
      ]),
    ).toThrow(AutopilotApertureContractUnsafe)
  })

  test('projects placement explanations with generatedAt and redacts non-public repo refs', () => {
    const publicExplanation = projectAutopilotPlacementExplanation(
      publicPlacementRecord(),
      nowIso,
    )
    const privateExplanation = projectAutopilotPlacementExplanation(
      {
        ...publicPlacementRecord(),
        decision: 'needs_customer_grant',
        eligible: false,
        publicClaimAllowed: false,
        repoRef: 'repo.customer_private_product',
        trustTier: 'private',
        blockerRefs: ['blocker.repo_placement.customer_grant_required'],
        customerSafeBlockedReasonRefs: [
          'reason.repo_placement.customer_connection_needed',
        ],
        policyRefs: [
          'policy.repo_trust.private',
          'policy.repo_placement.cloudflare_container',
        ],
      },
      nowIso,
    )

    expect(
      S.decodeUnknownSync(AutopilotPlacementExplanation)(publicExplanation),
    ).toEqual(publicExplanation)
    expect(publicExplanation).toMatchObject({
      decision: 'eligible',
      generatedAt: nowIso,
      repoRef: 'repo.github.OpenAgentsInc.openagents',
      staleness: 'fresh_until_placement_or_scope_transition',
    })
    expect(privateExplanation.repoRef).toBe('repo.redacted')
    expect(privateExplanation.customerSafeReasonRefs).toEqual([
      'reason.repo_placement.customer_connection_needed',
    ])
  })

  test('plans PR draft writeback only behind explicit authority receipts', () => {
    const decision = resolveGitHubWritebackAuthority(
      {
        approval: {
          approvedAt: nowIso,
          source: 'customer_action',
        },
        assignmentId: 'assignment.aperture_1',
        connection: null,
        grant: null,
        operation: 'open_fork_pull_request',
        repository: {
          fullName: 'OpenAgentsInc/openagents',
          isPrivate: false,
        },
        softwareOrderId: 'work_order.aperture_1',
        userId: 'user.aperture',
      },
      nowIso,
    )
    const plan = planAutopilotWorkOrderWriteback({
      artifactRef: 'artifact.pr_draft.aperture_1',
      authorityDecision: decision,
      authorityReceiptRefs: ['authority_receipt.github_writeback.aperture_1'],
      createdAtIso: nowIso,
      deliveryArtifactRefs: ['artifact.diff_summary.aperture_1'],
      id: 'artifact_record.pr_draft.aperture_1',
      missionRef: 'mission.aperture_1',
      operation: 'open_fork_pull_request',
      summaryRef: 'summary.pr_draft.aperture_1',
      updatedAtIso: nowIso,
      workOrderRef: 'work_order.aperture_1',
      workroomRefs: ['workroom.aperture_1'],
    })

    expect(S.decodeUnknownSync(AutopilotWritebackPlan)(plan)).toEqual(plan)
    expect(plan).toMatchObject({
      authorityDecision: 'allowed',
      blockerRefs: [],
      operation: 'open_fork_pull_request',
    })
    expect(plan.artifact).toMatchObject({
      artifactKind: 'pr_draft',
      authorityReceiptRefs: ['authority_receipt.github_writeback.aperture_1'],
      caveatRefs: ['caveat.pr_draft.human_merge_required'],
      status: 'draft',
      visibility: 'customer',
    })
  })

  test('returns blocked writeback artifacts for missing approval and rejects unsafe refs', () => {
    const decision = resolveGitHubWritebackAuthority(
      {
        approval: null,
        assignmentId: 'assignment.aperture_1',
        connection: null,
        grant: null,
        operation: 'open_pull_request',
        repository: {
          fullName: 'OpenAgentsInc/openagents',
          isPrivate: false,
        },
        softwareOrderId: 'work_order.aperture_1',
        userId: 'user.aperture',
      },
      nowIso,
    )
    const blocked = planAutopilotWorkOrderWriteback({
      artifactRef: 'artifact.pr_draft.blocked.aperture_1',
      authorityDecision: decision,
      authorityReceiptRefs: [
        'authority_receipt.github_writeback.blocked.aperture_1',
      ],
      createdAtIso: nowIso,
      deliveryArtifactRefs: [],
      id: 'artifact_record.pr_draft.blocked.aperture_1',
      missionRef: 'mission.aperture_1',
      operation: 'open_pull_request',
      summaryRef: 'summary.pr_draft.blocked.aperture_1',
      updatedAtIso: nowIso,
      workOrderRef: 'work_order.aperture_1',
      workroomRefs: ['workroom.aperture_1'],
    })

    expect(blocked).toMatchObject({
      authorityDecision: 'blocked',
      blockerRefs: ['blocker.github_writeback.explicit_approval_required'],
    })
    expect(blocked.artifact).toMatchObject({
      publicSafe: false,
      status: 'blocked',
      visibility: 'team',
    })
    expect(() =>
      validateAutopilotMissionWorkOrderLink(
        missionWorkOrderLink({
          artifactRefs: ['raw_patch.private_repo'],
        }),
      ),
    ).toThrow(AutopilotApertureContractUnsafe)
    expect(() =>
      planAutopilotWorkOrderWriteback({
        artifactRef: 'artifact.pr_draft.no_receipt',
        authorityDecision: {
          authorityMode: 'openagents_fork',
          connectionRef: null,
          customerMessage: 'Allowed',
          decision: 'allowed',
          grantRef: null,
          metadata: {
            approvalSource: 'customer_action',
            approvedAt: nowIso,
            authorityMode: 'openagents_fork',
            blockedReason: null,
            connectionRef: null,
            decision: 'allowed',
            grantRef: null,
            operation: 'open_fork_pull_request',
            repositoryFullName: 'OpenAgentsInc/openagents',
            repositoryPrivate: false,
          },
        },
        authorityReceiptRefs: [],
        createdAtIso: nowIso,
        deliveryArtifactRefs: ['artifact.diff_summary.aperture_1'],
        id: 'artifact_record.pr_draft.no_receipt',
        missionRef: 'mission.aperture_1',
        operation: 'open_fork_pull_request',
        summaryRef: 'summary.pr_draft.no_receipt',
        updatedAtIso: nowIso,
        workOrderRef: 'work_order.aperture_1',
        workroomRefs: ['workroom.aperture_1'],
      }),
    ).toThrow(AutopilotApertureContractUnsafe)
  })
})
