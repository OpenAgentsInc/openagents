import { describe, expect, test } from 'vitest'

import { projectDebtReceiptSettlement } from './debt-receipt-policy'
import {
  buildArtanisStudyingLaborProposal,
  handleArtanisStudyingContributionDelivery,
  projectArtanisStudyingContributionCorrectnessGate,
  runArtanisStudyingLaborRequestTick,
  studyingVerdictToDebtReceiptStudiedKnowledgeSource,
  type ArtanisStudyingContributionDelivery,
  type ArtanisStudyingContributionVerificationVerdict,
  type ArtanisStudyingContributionWorkRequest,
} from './artanis-studying-labor'

const workRequest = (
  overrides?: Partial<ArtanisStudyingContributionWorkRequest>,
): ArtanisStudyingContributionWorkRequest => ({
  budgetSats: 3_000,
  contributionKind: 'study_packet_and_graph',
  deadlineRef: 'deadline.public.study_labor.soon',
  graphRef: 'openagents_repo_studied_knowledge_graph.fixture',
  objectiveRef: 'objective.public.study_labor.openagents_s6',
  packetRef: 'openagents_repo_study_packet.fixture',
  repositoryRef: 'repo.public.github.OpenAgentsInc.openagents',
  title: 'Study openagents labor contribution',
  verificationCommandRef:
    'command.public.openagents.study_verification.s3_correctness',
  ...overrides,
})

const s3Verdict = (
  overrides?: Partial<ArtanisStudyingContributionVerificationVerdict>,
): ArtanisStudyingContributionVerificationVerdict => ({
  correctnessGatePassed: true,
  graphHash:
    'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
  graphRef: 'openagents_repo_studied_knowledge_graph.fixture',
  packetHash:
    'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
  packetRef: 'openagents_repo_study_packet.fixture',
  rejectedCount: 0,
  schemaRef: 'openagents.repo_studied_knowledge_verification.v0',
  sourceBoundary: 'public_refs_only',
  validatorReviewRefs: [],
  validatorReviewRequired: false,
  verificationHash:
    'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
  verificationRef:
    'openagents_repo_studied_knowledge_verification.fixture_passed',
  ...overrides,
})

const delivery = (
  overrides?: Partial<ArtanisStudyingContributionDelivery>,
): ArtanisStudyingContributionDelivery => ({
  acceptanceEventRef: 'nostr.event.' + 'd'.repeat(64),
  contributionKind: 'study_packet_and_graph',
  contributionRef: 'study_contribution.public.openagents.s6',
  graphRef: 'openagents_repo_studied_knowledge_graph.fixture',
  packetRef: 'openagents_repo_study_packet.fixture',
  providerActorRef: 'agent:study-provider',
  resultRef: 'result.public.study_labor.openagents.s6',
  s3Verification: s3Verdict(),
  verificationCommandRef:
    'command.public.openagents.study_verification.s3_correctness',
  workRequestId: 'work_request_study_s6',
  ...overrides,
})

describe('Artanis studying labor requests', () => {
  test('builds a ref-only study packet and graph work request proposal', () => {
    const proposal = buildArtanisStudyingLaborProposal(workRequest())

    expect(proposal).toEqual({
      budgetSats: 3_000,
      deadlineRef: 'deadline.public.study_labor.soon',
      objectiveRef: 'objective.public.study_labor.openagents_s6',
      repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
      requiredCapabilityRefs: [
        'capability.openagents.study_contribution.ref_only_delivery',
        'capability.openagents.study_verification.s3_correctness',
        'capability.openagents.study_packet.contribute',
        'capability.openagents.studied_knowledge_graph.contribute',
      ],
      title: 'Study openagents labor contribution',
      verificationCommandRef:
        'command.public.openagents.study_verification.s3_correctness',
    })
  })

  test('keeps the default requester path off until operator enabled', async () => {
    let proposed = false
    const result = await runArtanisStudyingLaborRequestTick({
      alreadyReservedThisTickMsat: 0,
      artanisActorRef: 'agent:artanis',
      enabled: false,
      nowIso: '2026-06-18T00:00:00.000Z',
      perTickBudgetMsat: 10_000_000,
      proposeStudyingContribution: async () => {
        proposed = true
        return workRequest()
      },
      recordTickReceipt: async () => {},
      reserveEscrow: async () => ({
        ok: true,
        reserveReceiptRef: 'receipt.labor_escrow.reserve.study_s6',
      }),
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async () => ({
        jobEventId: 'e'.repeat(64),
        topicId: 'topic_study_s6',
        workRequestId: 'work_request_study_s6',
      }),
    })

    expect(result).toEqual({ kind: 'skipped', reason: 'config_disabled' })
    expect(proposed).toBe(false)
  })

  test('emits a bounded study request through labor requester and reserves escrow', async () => {
    const submitted: unknown[] = []
    const reserved: unknown[] = []
    const receipts: unknown[] = []
    const result = await runArtanisStudyingLaborRequestTick({
      alreadyReservedThisTickMsat: 1_000_000,
      artanisActorRef: 'agent:artanis',
      enabled: true,
      nowIso: '2026-06-18T00:00:00.000Z',
      perTickBudgetMsat: 6_000_000,
      proposeStudyingContribution: async () => workRequest(),
      recordTickReceipt: async input => {
        receipts.push(input)
      },
      reserveEscrow: async input => {
        reserved.push(input)
        return {
          ok: true,
          reserveReceiptRef: 'receipt.labor_escrow.reserve.study_s6',
        }
      },
      seedBalanceAvailableMsat: 10_000_000,
      submitWorkRequest: async input => {
        submitted.push(input)
        return {
          jobEventId: 'e'.repeat(64),
          topicId: 'topic_study_s6',
          workRequestId: 'work_request_study_s6',
        }
      },
    })

    expect(result).toMatchObject({
      budgetMsat: 3_000_000,
      kind: 'requested',
      reserveReceiptRef: 'receipt.labor_escrow.reserve.study_s6',
    })
    expect(submitted).toHaveLength(1)
    expect(submitted[0]).toMatchObject({
      objectiveRef: 'objective.public.study_labor.openagents_s6',
      requiredCapabilityRefs: expect.arrayContaining([
        'capability.openagents.study_verification.s3_correctness',
      ]),
    })
    expect(reserved).toEqual([
      {
        amountMsat: 3_000_000,
        jobEventId: 'e'.repeat(64),
        requesterActorRef: 'agent:artanis',
        workRequestId: 'work_request_study_s6',
      },
    ])
    expect(receipts).toMatchObject([
      {
        kind: 'request_labor_proposed',
        refs: expect.arrayContaining([
          'work_request.public.work_request_study_s6',
          'receipt.labor_escrow.reserve.study_s6',
        ]),
      },
    ])
  })
})

describe('Artanis studying labor S3 correctness gate', () => {
  test('accepts and records delivered to settled lifecycle only after S3 passes', async () => {
    const lifecycle: unknown[] = []
    const releases: unknown[] = []
    const receipts: unknown[] = []
    const result = await handleArtanisStudyingContributionDelivery(delivery(), {
      recordLifecycle: async input => {
        lifecycle.push(input)
      },
      recordTickReceipt: async input => {
        receipts.push(input)
      },
      refundEscrow: async () => {
        throw new Error('passing S3 verdict should not refund')
      },
      releaseEscrow: async input => {
        releases.push(input)
        return {
          ok: true,
          releaseReceiptRef: 'receipt.labor_escrow.release.study_s6',
        }
      },
    })

    expect(result).toMatchObject({
      kind: 'settled',
      lifecycleKinds: ['delivered', 'accepted', 'settled'],
      releaseReceiptRef: 'receipt.labor_escrow.release.study_s6',
    })
    expect(result.correctnessGate).toMatchObject({
      blockerRefs: [],
      correctnessReceiptRefs: [
        'openagents_repo_studied_knowledge_verification.fixture_passed',
      ],
      releaseAllowed: true,
      status: 'accepted',
    })
    expect(releases).toEqual([
      {
        acceptanceEventRef: 'nostr.event.' + 'd'.repeat(64),
        providerActorRef: 'agent:study-provider',
        workRequestId: 'work_request_study_s6',
      },
    ])
    expect(lifecycle).toEqual([
      {
        lifecycleKind: 'delivered',
        receiptRef: 'result.public.study_labor.openagents.s6',
        workRequestId: 'work_request_study_s6',
      },
      {
        lifecycleKind: 'accepted',
        receiptRef: 'nostr.event.' + 'd'.repeat(64),
        workRequestId: 'work_request_study_s6',
      },
      {
        lifecycleKind: 'settled',
        receiptRef: 'receipt.labor_escrow.release.study_s6',
        workRequestId: 'work_request_study_s6',
      },
    ])
    expect(receipts).toMatchObject([
      {
        kind: 'request_labor_accepted',
        refs: expect.arrayContaining([
          'openagents_repo_studied_knowledge_verification.fixture_passed',
          'receipt.labor_escrow.release.study_s6',
        ]),
      },
    ])
  })

  test('refunds escrow and does not accept or settle when S3 rejects', async () => {
    const lifecycle: unknown[] = []
    const refunds: unknown[] = []
    const result = await handleArtanisStudyingContributionDelivery(
      delivery({
        s3Verification: s3Verdict({
          correctnessGatePassed: false,
          rejectedCount: 1,
          verificationRef:
            'openagents_repo_studied_knowledge_verification.fixture_rejected',
        }),
      }),
      {
        recordLifecycle: async input => {
          lifecycle.push(input)
        },
        recordTickReceipt: async () => {},
        refundEscrow: async input => {
          refunds.push(input)
          return {
            ok: true,
            refundReceiptRef: 'receipt.labor_escrow.refund.study_s6',
          }
        },
        releaseEscrow: async () => {
          throw new Error('failing S3 verdict should not release')
        },
      },
    )

    expect(result).toMatchObject({
      kind: 'rejected_refunded',
      lifecycleKinds: ['delivered'],
      refundReceiptRef: 'receipt.labor_escrow.refund.study_s6',
    })
    expect(result.correctnessGate).toMatchObject({
      releaseAllowed: false,
      status: 'rejected',
    })
    expect(result.correctnessGate.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.study_labor.s3_correctness_failed',
        'blocker.public.study_labor.s3_rejected_claims',
      ]),
    )
    expect(refunds).toEqual([
      {
        reasonRef: expect.stringContaining('blocker.public.study_labor.'),
        workRequestId: 'work_request_study_s6',
      },
    ])
    expect(lifecycle).toEqual([
      {
        lifecycleKind: 'delivered',
        receiptRef: 'result.public.study_labor.openagents.s6',
        workRequestId: 'work_request_study_s6',
      },
    ])
  })

  test('holds validator-review remainder out of escrow release', () => {
    const gate = projectArtanisStudyingContributionCorrectnessGate(
      delivery({
        s3Verification: s3Verdict({
          correctnessGatePassed: false,
          validatorReviewRefs: [
            'validator_review.public.study_labor.remainder_1',
          ],
          validatorReviewRequired: true,
        }),
      }),
    )

    expect(gate).toMatchObject({
      releaseAllowed: false,
      status: 'needs_validator_review',
      validatorReviewRefs: [
        'validator_review.public.study_labor.remainder_1',
      ],
    })
    expect(gate.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.study_labor.validator_review_required',
      ]),
    )
  })
})

// SA-3 (#5340): studied knowledge wired into the hygiene/refactoring debt-receipt
// lane.
const hygieneDebtReceiptInput = (
  source: ReturnType<typeof studyingVerdictToDebtReceiptStudiedKnowledgeSource>,
) => ({
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.hygiene.pass'],
  baselineMetricRefs: ['metric.public.debt_receipt.hygiene.baseline'],
  budgetCapSats: 10_000,
  fundingApprovalRefs: ['approval.public.debt_receipt.hygiene.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.hygiene.improved'],
  noNewEqualOrWorseDebtRefs: ['check.public.debt_receipt.hygiene.no_new_debt'],
  payableSats: 5_000,
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewerActorRef: 'actor.public.reviewer.trigger',
  reviewDecisionRefs: ['review.public.debt_receipt.hygiene.accepted'],
  scopeRefs: ['scope.public.debt_receipt.hygiene.module'],
  settlementApprovalRefs: ['approval.public.debt_receipt.hygiene.settlement'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5340'],
  stopConditionRefs: ['stop.public.debt_receipt.hygiene.retire_once'],
  studiedKnowledgeRequired: true,
  studiedKnowledgeSource: source,
  targetMetricRefs: ['metric.public.debt_receipt.hygiene.target'],
  verificationCommandRefs: ['command.public.debt_receipt.hygiene.bun_test'],
})

describe('Studied knowledge wired into the hygiene debt-receipt lane (#5340)', () => {
  test('a passing S3 studying verdict becomes a payable hygiene debt-receipt source', () => {
    const source = studyingVerdictToDebtReceiptStudiedKnowledgeSource(
      s3Verdict(),
    )

    expect(source).toMatchObject({
      correctnessGatePassed: true,
      graphRef: 'openagents_repo_studied_knowledge_graph.fixture',
      packetRef: 'openagents_repo_study_packet.fixture',
      rejectedCount: 0,
      schemaRef: 'openagents.repo_studied_knowledge_verification.v0',
      sourceBoundary: 'public_refs_only',
      validatorReviewRequired: false,
      verificationRef:
        'openagents_repo_studied_knowledge_verification.fixture_passed',
    })

    const projection = projectDebtReceiptSettlement(
      hygieneDebtReceiptInput(source),
    )

    expect(projection).toMatchObject({
      state: 'payable',
      studiedKnowledgeGatePassed: true,
      studiedKnowledgeRequired: true,
      workerPayoutEligible: true,
    })
    expect(projection.studiedKnowledgeSourceRefs).toContain(
      'openagents_repo_studied_knowledge_verification.fixture_passed',
    )
  })

  test('a rejected S3 studying verdict blocks the hygiene debt receipt from becoming payable', () => {
    const source = studyingVerdictToDebtReceiptStudiedKnowledgeSource(
      s3Verdict({
        correctnessGatePassed: false,
        rejectedCount: 2,
        verificationRef:
          'openagents_repo_studied_knowledge_verification.fixture_rejected',
      }),
    )

    const projection = projectDebtReceiptSettlement(
      hygieneDebtReceiptInput(source),
    )

    expect(projection).toMatchObject({
      studiedKnowledgeGatePassed: false,
      workerPayoutEligible: false,
    })
    expect(projection.state).not.toBe('payable')
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.debt_receipt.studied_knowledge_correctness_failed',
        'blocker.public.debt_receipt.studied_knowledge_rejected_claims',
      ]),
    )
  })

  test('a validator-review-required S3 studying verdict surfaces review refs and stays unpayable', () => {
    const source = studyingVerdictToDebtReceiptStudiedKnowledgeSource(
      s3Verdict({
        correctnessGatePassed: false,
        validatorReviewRefs: ['validator_review.public.study_labor.remainder_1'],
        validatorReviewRequired: true,
      }),
    )

    expect(source.validatorReviewRefs).toEqual([
      'validator_review.public.study_labor.remainder_1',
    ])

    const projection = projectDebtReceiptSettlement(
      hygieneDebtReceiptInput(source),
    )

    expect(projection.studiedKnowledgeGatePassed).toBe(false)
    expect(projection.workerPayoutEligible).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.studied_knowledge_validator_review_required',
    )
  })
})
