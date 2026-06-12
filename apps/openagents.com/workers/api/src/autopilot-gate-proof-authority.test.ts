import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotGateProofDecision,
  AutopilotGateProofDecisionInput,
  AutopilotGateProofUnsafe,
  AutopilotGateSmokeReceiptAuthority,
  evaluateAutopilotGateProofDecision,
  validateAutopilotGateSmokeReceiptAuthority,
} from './autopilot-gate-proof-authority'
import { PackAReceiptRecord } from './autopilot-pack-a-ledger'

const nowIso = '2026-06-11T22:55:00.000Z'

const receipt = (input: Partial<PackAReceiptRecord> = {}): PackAReceiptRecord =>
  new PackAReceiptRecord({
    artifactRefs: ['artifact.gate.smoke.public_1'],
    createdAt: nowIso,
    idempotencyKey: 'idempotency.gate.receipt.1',
    kind: 'smoke_passed',
    previousReceiptRefs: [],
    receiptRef: 'receipt.gate.smoke_passed.1',
    subjectRef: 'proof.gate.m10.1',
    ...input,
  })

const smokeAuthority = (
  input: Partial<AutopilotGateSmokeReceiptAuthority> = {},
): AutopilotGateSmokeReceiptAuthority =>
  new AutopilotGateSmokeReceiptAuthority({
    acceptedWorkAuthority: false,
    artifactRefs: ['artifact.gate.smoke.public_1'],
    authorityRef: 'authority.gate.smoke.m10.1',
    claimKind: 'm10_overnight_unattended',
    generatedAt: nowIso,
    issueRefs: ['issue.public.openagents.4768'],
    payoutAuthority: false,
    publicClaimAuthority: false,
    receipt: receipt(),
    sourceCommitRefs: ['commit.openagents.323526346'],
    verifierRefs: ['verifier.gate.pack_a_receipt_check'],
    ...input,
  })

const decisionInput = (
  input: Partial<AutopilotGateProofDecisionInput> = {},
): AutopilotGateProofDecisionInput =>
  new AutopilotGateProofDecisionInput({
    acceptedDeferredIssueRefs: [],
    blockingIssueRefs: [],
    claimKind: 'm10_overnight_unattended',
    closedIssueRefs: [
      'issue.public.openagents.4814',
      'issue.public.openagents.4815',
      'issue.public.openagents.4816',
      'issue.public.openagents.4818',
      'issue.public.openagents.4819',
      'issue.public.openagents.4820',
      'issue.public.openagents.4821',
      'issue.public.openagents.4822',
      'issue.public.openagents.4823',
    ],
    generatedAt: nowIso,
    liveEvidenceRefs: ['evidence.live.overnight_unattended.1'],
    receipts: [
      receipt({ kind: 'schedule_fired', receiptRef: 'receipt.schedule_fired' }),
      receipt({ kind: 'task_completed', receiptRef: 'receipt.task_completed' }),
      receipt({
        kind: 'notification_delivered',
        receiptRef: 'receipt.notification_delivered',
      }),
      receipt({
        kind: 'review_recorded',
        receiptRef: 'receipt.review_recorded',
      }),
      receipt({
        kind: 'verification_passed',
        receiptRef: 'receipt.verification_passed',
      }),
      receipt({
        kind: 'delivery_recorded',
        receiptRef: 'receipt.delivery_recorded',
      }),
      receipt({ kind: 'smoke_passed', receiptRef: 'receipt.smoke_passed' }),
    ],
    requiredIssueRefs: [
      'issue.public.openagents.4814',
      'issue.public.openagents.4815',
      'issue.public.openagents.4816',
      'issue.public.openagents.4818',
      'issue.public.openagents.4819',
      'issue.public.openagents.4820',
      'issue.public.openagents.4821',
      'issue.public.openagents.4822',
      'issue.public.openagents.4823',
    ],
    requiredLiveEvidenceRefs: ['evidence.live.overnight_unattended.1'],
    smokeAuthorities: [smokeAuthority()],
    sourceCommitRefs: ['commit.openagents.323526346'],
    ...input,
  })

describe('Autopilot Gate proof authority', () => {
  test('validates smoke receipt authority as evidence-only and public-safe', () => {
    const authority = validateAutopilotGateSmokeReceiptAuthority(
      smokeAuthority({
        artifactRefs: [
          'artifact.gate.smoke.public_1',
          'artifact.gate.smoke.public_1',
          'artifact.gate.redaction.public_1',
        ],
      }),
    )

    expect(
      S.decodeUnknownSync(AutopilotGateSmokeReceiptAuthority)(authority),
    ).toEqual(authority)
    expect(authority).toMatchObject({
      acceptedWorkAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
    })
    expect(authority.artifactRefs).toEqual([
      'artifact.gate.redaction.public_1',
      'artifact.gate.smoke.public_1',
    ])
  })

  test('rejects smoke authority records without smoke receipts or with unsafe refs', () => {
    expect(() =>
      validateAutopilotGateSmokeReceiptAuthority(
        smokeAuthority({
          receipt: receipt({ kind: 'task_completed' }),
        }),
      ),
    ).toThrow(AutopilotGateProofUnsafe)
    expect(() =>
      validateAutopilotGateSmokeReceiptAuthority(
        smokeAuthority({
          artifactRefs: ['artifact.raw_prompt.sk-secret'],
        }),
      ),
    ).toThrow(AutopilotGateProofUnsafe)
  })

  test('allows M10 closeout only when required receipts, child issues, live evidence, and smoke authority are present', () => {
    const decision = evaluateAutopilotGateProofDecision(decisionInput())

    expect(S.decodeUnknownSync(AutopilotGateProofDecision)(decision)).toEqual(
      decision,
    )
    expect(decision).toMatchObject({
      blockerRefs: [],
      closeAllowed: true,
      claimKind: 'm10_overnight_unattended',
      missingIssueRefs: [],
      missingLiveEvidenceRefs: [],
      missingReceiptKinds: [],
      status: 'ready_to_close',
    })
    expect(decision.requiredReceiptKinds).toEqual([
      'schedule_fired',
      'task_completed',
      'notification_delivered',
      'review_recorded',
      'verification_passed',
      'delivery_recorded',
    ])
  })

  test('blocks M10 when Pack A receipts, child issues, or live evidence are missing', () => {
    const decision = evaluateAutopilotGateProofDecision(
      decisionInput({
        closedIssueRefs: ['issue.public.openagents.4814'],
        liveEvidenceRefs: [],
        receipts: [
          receipt({
            kind: 'schedule_fired',
            receiptRef: 'receipt.schedule_fired',
          }),
          receipt({
            kind: 'task_completed',
            receiptRef: 'receipt.task_completed',
          }),
        ],
      }),
    )

    expect(decision.closeAllowed).toBe(false)
    expect(decision.status).toBe('deferred')
    expect(decision.missingReceiptKinds).toEqual([
      'notification_delivered',
      'review_recorded',
      'verification_passed',
      'delivery_recorded',
    ])
    expect(decision.missingIssueRefs).toContain('issue.public.openagents.4818')
    expect(decision.missingLiveEvidenceRefs).toEqual([
      'evidence.live.overnight_unattended.1',
    ])
  })

  test('allows M9 closeout with rotation receipts and live continuity evidence', () => {
    const decision = evaluateAutopilotGateProofDecision(
      decisionInput({
        claimKind: 'm9_live_rate_limit_rotation',
        closedIssueRefs: [],
        liveEvidenceRefs: ['evidence.live.rate_limit_rotation.1'],
        receipts: [
          receipt({
            artifactRefs: ['artifact.m9_rotation.ci_safe_smoke'],
            kind: 'smoke_passed',
            receiptRef: 'receipt.m9_rotation.smoke_passed',
            subjectRef: 'proof.m9_rotation.ci_safe',
          }),
          receipt({
            artifactRefs: ['artifact.m9_rotation.work_order_closeout'],
            kind: 'verification_passed',
            receiptRef: 'receipt.m9_rotation.verification_passed',
            subjectRef: 'proof.m9_rotation.continuity',
          }),
          receipt({
            artifactRefs: ['artifact.m9_rotation.live_route_failover'],
            kind: 'usage_threshold_crossed',
            receiptRef: 'receipt.m9_rotation.usage_threshold_crossed',
            subjectRef: 'proof.m9_rotation.route_rotation',
          }),
        ],
        requiredIssueRefs: [],
        requiredLiveEvidenceRefs: ['evidence.live.rate_limit_rotation.1'],
        smokeAuthorities: [],
        sourceCommitRefs: [
          'commit.openagents.9cd15c4a0',
          'commit.openagents.8b79ef2c2',
        ],
      }),
    )

    expect(decision).toMatchObject({
      blockerRefs: [],
      closeAllowed: true,
      claimKind: 'm9_live_rate_limit_rotation',
      liveEvidenceRefs: ['evidence.live.rate_limit_rotation.1'],
      missingLiveEvidenceRefs: [],
      missingReceiptKinds: [],
      status: 'ready_to_close',
    })
    expect(decision.requiredReceiptKinds).toEqual([
      'smoke_passed',
      'verification_passed',
      'usage_threshold_crossed',
    ])
  })

  test('keeps M14 and parent closeout deferred while live Gate dependencies remain open', () => {
    const m14 = evaluateAutopilotGateProofDecision(
      decisionInput({
        acceptedDeferredIssueRefs: ['issue.public.openagents.4786'],
        blockingIssueRefs: [
          'issue.public.openagents.4767',
          'issue.public.openagents.4771',
          'issue.public.openagents.4768',
        ],
        claimKind: 'm14_mvp_exit_review',
        liveEvidenceRefs: [],
        receipts: [
          receipt({ kind: 'smoke_passed', receiptRef: 'receipt.smoke_passed' }),
          receipt({
            kind: 'review_recorded',
            receiptRef: 'receipt.review_recorded',
          }),
          receipt({
            kind: 'acceptance_recorded',
            receiptRef: 'receipt.acceptance_recorded',
          }),
        ],
        requiredIssueRefs: [
          'issue.public.openagents.4767',
          'issue.public.openagents.4768',
          'issue.public.openagents.4773',
          'issue.public.openagents.4813',
        ],
        requiredLiveEvidenceRefs: [
          'evidence.live.rate_limit_rotation.1',
          'evidence.live.overnight_unattended.1',
        ],
        smokeAuthorities: [
          smokeAuthority({
            authorityRef: 'authority.gate.smoke.m14.1',
            claimKind: 'm14_mvp_exit_review',
          }),
        ],
      }),
    )

    expect(m14.closeAllowed).toBe(false)
    expect(m14.status).toBe('deferred')
    expect(m14.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.gate.issue_open.issue.public.openagents.4767',
        'blocker.gate.live_evidence_missing.evidence.live.rate_limit_rotation.1',
        'blocker.gate.receipt_missing.usage_budget_stop',
      ]),
    )

    const parent = evaluateAutopilotGateProofDecision(
      decisionInput({
        acceptedDeferredIssueRefs: [
          'issue.public.openagents.4749',
          'issue.public.openagents.4777',
          'issue.public.openagents.4781',
          'issue.public.openagents.4782',
          'issue.public.openagents.4783',
        ],
        blockingIssueRefs: ['issue.public.openagents.4772'],
        claimKind: 'autopilot_parent_closeout',
        requiredIssueRefs: ['issue.public.openagents.4772'],
        requiredLiveEvidenceRefs: [],
        smokeAuthorities: [],
      }),
    )

    expect(parent.closeAllowed).toBe(false)
    expect(parent.status).toBe('deferred')
    expect(parent.deferredIssueRefs).toEqual([
      'issue.public.openagents.4749',
      'issue.public.openagents.4777',
      'issue.public.openagents.4781',
      'issue.public.openagents.4782',
      'issue.public.openagents.4783',
    ])
  })

  test('records W3 as a separate blocked evaluation track, not an MVP gate', () => {
    const decision = evaluateAutopilotGateProofDecision(
      decisionInput({
        blockingIssueRefs: ['issue.public.openagents.4749'],
        claimKind: 'w3_student_program_evaluation',
        requiredIssueRefs: ['issue.public.openagents.4749'],
        requiredLiveEvidenceRefs: [
          'evidence.w3.four_baseline_report',
          'evidence.w3.hypothesis_verdicts',
        ],
        smokeAuthorities: [],
      }),
    )

    expect(decision.closeAllowed).toBe(false)
    expect(decision.status).toBe('deferred')
    expect(decision.blockerRefs).toContain(
      'blocker.gate.issue_open.issue.public.openagents.4749',
    )
    expect(decision.missingLiveEvidenceRefs).toEqual([
      'evidence.w3.four_baseline_report',
      'evidence.w3.hypothesis_verdicts',
    ])
  })
})
