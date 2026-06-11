import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotBridgeAutonomicCodingEvaluation,
  AutopilotBridgeAutonomicCodingEvaluationInput,
  AutopilotBridgeAutonomicCodingProposal,
  AutopilotBridgeContractUnsafe,
  AutopilotBridgeForumCodingOrderInput,
  AutopilotBridgeForumCodingOrderLink,
  AutopilotBridgeLifecycleReceipt,
  AutopilotBridgeMvpParityMatrix,
  AutopilotBridgeParityProjection,
  AutopilotBridgeParityRow,
  assertNewMvpSurfaceHasApiPeer,
  evaluateAutonomicCodingWorkProposal,
  planForumCodingOrderBridge,
  projectAutopilotBridgeParityMatrix,
  validateAutopilotBridgeParityMatrix,
} from './autopilot-bridge-contracts'

const nowIso = '2026-06-11T22:40:00.000Z'

const forumLifecycle = (): ReadonlyArray<AutopilotBridgeLifecycleReceipt> => [
  new AutopilotBridgeLifecycleReceipt({
    idempotencyKey: 'idempotency.forum_bridge.queued.1',
    kind: 'queued',
    receiptRef: 'receipt.forum_bridge.queued.1',
    topicPostRef: 'forum_post.work_request.queued.1',
  }),
  new AutopilotBridgeLifecycleReceipt({
    idempotencyKey: 'idempotency.forum_bridge.placed.1',
    kind: 'placed',
    receiptRef: 'receipt.forum_bridge.placed.1',
    topicPostRef: 'forum_post.work_request.placed.1',
  }),
  new AutopilotBridgeLifecycleReceipt({
    idempotencyKey: 'idempotency.forum_bridge.delivered.1',
    kind: 'delivered',
    receiptRef: 'receipt.forum_bridge.delivered.1',
    topicPostRef: 'forum_post.work_request.delivered.1',
  }),
  new AutopilotBridgeLifecycleReceipt({
    idempotencyKey: 'idempotency.forum_bridge.reviewed.1',
    kind: 'reviewed',
    receiptRef: 'receipt.forum_bridge.reviewed.1',
    topicPostRef: 'forum_post.work_request.reviewed.1',
  }),
]

const forumInput = (
  input: Partial<AutopilotBridgeForumCodingOrderInput> = {},
): AutopilotBridgeForumCodingOrderInput =>
  new AutopilotBridgeForumCodingOrderInput({
    budgetRef: 'budget.forum_coding_order.1',
    forumActionRef: 'forum_action.request_coding.1',
    generatedAt: nowIso,
    lifecycleReceipts: forumLifecycle(),
    missionRef: 'mission.forum_coding_order.1',
    paymentMode: 'operator_credit',
    requestingAgentRef: 'agent:orrery',
    threadRef: 'forum_thread.work_requests.1',
    workOrderRef: 'work_order.forum_coding_order.1',
    ...input,
  })

const codingProposal = (
  input: Partial<AutopilotBridgeAutonomicCodingProposal> = {},
): AutopilotBridgeAutonomicCodingProposal =>
  new AutopilotBridgeAutonomicCodingProposal({
    acceptanceCriteriaRefs: ['acceptance.autonomic_coding.bun_test'],
    action: 'request_coding_work',
    budgetSats: 2_000,
    objectiveRef: 'objective.public.autonomic_coding.fix_test',
    repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
    requestingAutonomicRef: 'agent:artanis',
    reviewPolicyRef: 'review_policy.validator_reexecution.bun_test',
    verificationCommandRef: 'command.public.pylon.labor.bun_test',
    ...input,
  })

const autonomicInput = (
  input: Partial<AutopilotBridgeAutonomicCodingEvaluationInput> = {},
): AutopilotBridgeAutonomicCodingEvaluationInput =>
  new AutopilotBridgeAutonomicCodingEvaluationInput({
    alreadyReservedThisTickMsat: 1_000_000,
    generatedAt: nowIso,
    operatorEnabled: true,
    paymentAuthorityRef: 'authority.autonomic_coding.operator_credit.1',
    perTickBudgetMsat: 5_000_000,
    proposal: codingProposal(),
    seededBalanceAvailableMsat: 10_000_000,
    tickRef: 'tick.artanis.2026_06_11_2240',
    ...input,
  })

describe('Autopilot Bridge contracts', () => {
  test('projects the MVP parity matrix with every browser surface mapped to an API peer', () => {
    const projection = projectAutopilotBridgeParityMatrix(
      AutopilotBridgeMvpParityMatrix,
      nowIso,
    )

    expect(
      S.decodeUnknownSync(AutopilotBridgeParityProjection)(projection),
    ).toEqual(projection)
    expect(projection.ready).toBe(true)
    expect(projection.blockedCapabilityRefs).toEqual([])
    expect(projection.rows.map(row => row.capability).sort()).toEqual([
      'account_pool_visibility',
      'artifact_receipts',
      'budget_usage_visibility',
      'decisions_review',
      'placement_pricing_visibility',
      'scheduling_continuation',
      'status_events',
      'submit_work',
    ])
    expect(projection.staleness.maxStalenessSeconds).toBe(0)
  })

  test('fails a new MVP browser surface without an API peer or explicit waiver', () => {
    expect(() =>
      assertNewMvpSurfaceHasApiPeer(
        new AutopilotBridgeParityRow({
          apiPeerRefs: [],
          capability: 'submit_work',
          proofRefs: ['issue.public.openagents.4773'],
          testRefs: ['workers/api/src/autopilot-bridge-contracts.test.ts'],
          waiver: null,
          webSurfaceRef: 'web.autopilot.new_submit_surface',
        }),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)

    expect(() =>
      validateAutopilotBridgeParityMatrix(
        AutopilotBridgeMvpParityMatrix.filter(
          row => row.capability !== 'artifact_receipts',
        ),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)
  })

  test('plans Forum to Autopilot coding order linkage with ref-only lifecycle posts', () => {
    const link = planForumCodingOrderBridge(
      forumInput({
        lifecycleReceipts: [
          ...forumLifecycle(),
          forumLifecycle()[0] as AutopilotBridgeLifecycleReceipt,
        ],
      }),
    )

    expect(
      S.decodeUnknownSync(AutopilotBridgeForumCodingOrderLink)(link),
    ).toEqual(link)
    expect(link).toMatchObject({
      forumActionRef: 'forum_action.request_coding.1',
      idempotencyKey:
        'idempotency.forum_coding_order.work_order.forum_coding_order.1',
      paymentMode: 'operator_credit',
      requestingAgentRef: 'agent:orrery',
      threadRef: 'forum_thread.work_requests.1',
      workOrderRef: 'work_order.forum_coding_order.1',
    })
    expect(link.lifecycleReceiptRefs).toEqual([
      'receipt.forum_bridge.delivered.1',
      'receipt.forum_bridge.placed.1',
      'receipt.forum_bridge.queued.1',
      'receipt.forum_bridge.reviewed.1',
    ])
    expect(link.topicPostRefs).toHaveLength(4)
  })

  test('blocks Forum coding orders from non-agent actors, partial lifecycle, or unsafe material', () => {
    expect(() =>
      planForumCodingOrderBridge(
        forumInput({ requestingAgentRef: 'user:browser_session' }),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)
    expect(() =>
      planForumCodingOrderBridge(
        forumInput({ lifecycleReceipts: forumLifecycle().slice(0, 3) }),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)
    expect(() =>
      planForumCodingOrderBridge(
        forumInput({ threadRef: 'forum_thread.raw_prompt.secret_token' }),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)
  })

  test('evaluates autonomic request_coding_work as a proposal behind operator, budget, payment, and review gates', () => {
    const evaluation = evaluateAutonomicCodingWorkProposal(autonomicInput())

    expect(
      S.decodeUnknownSync(AutopilotBridgeAutonomicCodingEvaluation)(evaluation),
    ).toEqual(evaluation)
    expect(evaluation).toMatchObject({
      blockerRefs: [],
      budgetMsat: 2_000_000,
      decision: 'proposed',
      paymentAuthorityRef: 'authority.autonomic_coding.operator_credit.1',
      proposedWorkOrderDraftRef:
        'work_order_draft.autonomic.tick.artanis.2026_06_11_2240',
      reserveIntentRef:
        'reserve_intent.autonomic_coding.tick.artanis.2026_06_11_2240',
    })
    expect(evaluation.reviewGateRefs).toEqual([
      'review_policy.validator_reexecution.bun_test',
      'verification_command.command.public.pylon.labor.bun_test',
      'gate.human_review.repo_authority',
    ])
  })

  test('keeps autonomic coding work default-off and refuses missing payment or over-budget proposals', () => {
    expect(
      evaluateAutonomicCodingWorkProposal(
        autonomicInput({ operatorEnabled: false }),
      ),
    ).toMatchObject({
      blockerRefs: ['blocker.autonomic_coding_work.operator_disabled'],
      decision: 'skipped',
      proposedWorkOrderDraftRef: null,
      reserveIntentRef: null,
    })
    expect(
      evaluateAutonomicCodingWorkProposal(
        autonomicInput({ paymentAuthorityRef: null }),
      ),
    ).toMatchObject({
      blockerRefs: ['blocker.autonomic_coding_work.payment_authority_missing'],
      decision: 'refused',
    })
    expect(
      evaluateAutonomicCodingWorkProposal(
        autonomicInput({
          perTickBudgetMsat: 2_000_000,
        }),
      ),
    ).toMatchObject({
      blockerRefs: ['refusal.artanis_labor_budget.per_tick_cap'],
      budgetMsat: 2_000_000,
      decision: 'refused',
    })
  })

  test('rejects unsafe autonomic coding proposal refs before any draft is created', () => {
    expect(() =>
      evaluateAutonomicCodingWorkProposal(
        autonomicInput({
          proposal: codingProposal({
            objectiveRef: 'objective.raw_prompt.sk-secret',
          }),
        }),
      ),
    ).toThrow(AutopilotBridgeContractUnsafe)
  })
})
