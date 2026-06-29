import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotAttentionEventRecord,
  AutopilotAttentionProjection,
  AutopilotCompanionProjectionInput,
  AutopilotCompanionProjectionRow,
  AutopilotPackASupervisionUnsafe,
  AutopilotPermissionRequest,
  AutopilotStructuredOutputEnvelope,
  decideAutopilotCompanionAction,
  decideAutopilotPermission,
  interactionModeContract,
  projectAutopilotAttention,
  projectAutopilotCompanionRow,
  selectAttentionDeliveryChannel,
  structuredOutputEnvelope,
} from './autopilot-pack-a-supervision'

const nowIso = '2026-06-11T22:40:00.000Z'

const attentionEvent = (
  input: Partial<AutopilotAttentionEventRecord> = {},
): AutopilotAttentionEventRecord =>
  new AutopilotAttentionEventRecord({
    artifactRefs: [],
    blockerRefs: [],
    createdAt: '2026-06-11T22:30:00.000Z',
    decisionRef: 'decision.autopilot.work_1.approve',
    dedupeKey: 'dedupe.work_1.approval',
    eventRef: 'attention.work_1.approval.1',
    foldsWith: [],
    invalidates: [],
    kind: 'waiting_for_approval',
    missionRef: 'mission.work_1',
    preferredChannel: 'email',
    priority: 'high',
    privacyClass: 'owner_summary',
    resolvedAt: null,
    runRef: 'run.work_1',
    safeSummaryRef: 'summary.approval_needed',
    timeoutAt: '2026-06-12T22:30:00.000Z',
    workOrderRef: 'autopilot_work_order.work_1',
    ...input,
  })

describe('Pack A Beacon supervision contracts', () => {
  test('dedupes current attention, clears invalidated waiting state, and records delivery failure receipts', () => {
    const projection = projectAutopilotAttention({
      deliveryReceipts: [
        {
          attemptedAt: nowIso,
          channel: 'email',
          deliveryRef: 'receipt.attention.work_1.email.failed',
          errorSummaryRef: 'delivery_error.email.provider_unavailable',
          eventRef: 'attention.work_1.approval.2',
          idempotencyKey: 'idempotency.attention.work_1.email',
          status: 'failed',
        },
      ],
      events: [
        attentionEvent({ eventRef: 'attention.work_1.approval.1' }),
        attentionEvent({
          createdAt: '2026-06-11T22:35:00.000Z',
          eventRef: 'attention.work_1.approval.2',
          safeSummaryRef: 'summary.approval_still_needed',
        }),
        attentionEvent({
          createdAt: '2026-06-11T22:36:00.000Z',
          dedupeKey: 'dedupe.work_1.resolved',
          eventRef: 'attention.work_1.resolved',
          invalidates: ['dedupe.work_1.approval'],
          kind: 'background_completed',
          safeSummaryRef: 'summary.completed',
        }),
      ],
      nowIso,
    })

    expect(() =>
      S.decodeUnknownSync(AutopilotAttentionProjection)(projection),
    ).not.toThrow()
    expect(projection.current).toHaveLength(1)
    expect(projection.current[0]).toMatchObject({
      eventRef: 'attention.work_1.resolved',
      waitingState: 'completion_available',
    })
    expect(projection.deliveryFailures).toEqual([
      'receipt.attention.work_1.email.failed',
    ])
    expect(projection.generatedAt).toBe(nowIso)
    expect(projection.staleness.maxStalenessSeconds).toBe(0)
  })

  test('selects fallback delivery channels without failing the underlying run', () => {
    expect(
      selectAttentionDeliveryChannel({
        disabledChannels: ['email'],
        event: attentionEvent(),
        fallbackChannels: ['terminal', 'in_app'],
      }),
    ).toBe('terminal')
    expect(
      selectAttentionDeliveryChannel({
        disabledChannels: ['email', 'terminal', 'in_app'],
        event: attentionEvent(),
        fallbackChannels: ['terminal', 'in_app'],
      }),
    ).toBe('disabled')
  })

  test('projects companion rows with generatedAt, caveats, action refs, and private artifact redaction', () => {
    const projection = projectAutopilotCompanionRow(
      new AutopilotCompanionProjectionInput({
        artifactRefs: [
          'artifact.public.work_1.closeout',
          'artifact.private.work_1.raw_patch',
        ],
        attentionItems: [
          projectAutopilotAttention({
            deliveryReceipts: [],
            events: [attentionEvent()],
            nowIso,
          }).current[0]!,
        ],
        budgetStatusRef: 'budget.autopilot.work_1.ok',
        caveatRefs: [],
        latestPublicProgressRef: 'progress.work_1.tests_running',
        missionRef: 'mission.work_1',
        runRef: 'run.work_1',
        status: 'waiting',
        updatedAt: nowIso,
        waitingDecisionRef: 'decision.autopilot.work_1.approve',
        workOrderRef: 'autopilot_work_order.work_1',
      }),
      nowIso,
    )

    expect(() =>
      S.decodeUnknownSync(AutopilotCompanionProjectionRow)(projection),
    ).not.toThrow()
    expect(projection.artifactRefs).toEqual(['artifact.public.work_1.closeout'])
    expect(projection.caveatRefs).toContain('caveat.private_artifacts_redacted')
    expect(projection.actionRefs).toEqual([
      'action.autopilot_work_order.work_1.cancel',
      'action.decision.autopilot.work_1.approve.answer',
      'action.decision.autopilot.work_1.approve.approve',
      'action.decision.autopilot.work_1.approve.deny',
    ])
    expect(projection.generatedAt).toBe(nowIso)
    expect(projection.staleness.rebuildsOn).toContain(
      'autopilot_attention_event_recorded',
    )
  })

  test('rejects companion actions for non-members and stale decisions with typed reasons', () => {
    const projection = projectAutopilotCompanionRow(
      new AutopilotCompanionProjectionInput({
        artifactRefs: [],
        attentionItems: [],
        budgetStatusRef: 'budget.autopilot.work_1.ok',
        caveatRefs: [],
        latestPublicProgressRef: null,
        missionRef: 'mission.work_1',
        runRef: 'run.work_1',
        status: 'waiting',
        updatedAt: nowIso,
        waitingDecisionRef: 'decision.autopilot.work_1.approve',
        workOrderRef: 'autopilot_work_order.work_1',
      }),
      nowIso,
    )

    expect(
      decideAutopilotCompanionAction({
        projection,
        request: {
          actionKind: 'approve',
          actorMembership: 'non_member',
          decisionRef: 'decision.autopilot.work_1.approve',
          idempotencyKey: 'idempotency.work_1.approve',
          requestedAt: nowIso,
          requestRef: 'companion_action.work_1.non_member',
          workOrderRef: 'autopilot_work_order.work_1',
        },
        stale: false,
      }),
    ).toMatchObject({
      decision: 'rejected',
      directEffectPermitted: false,
      reasonRef: 'blocker.autopilot_companion.non_member',
    })
    expect(
      decideAutopilotCompanionAction({
        projection,
        request: {
          actionKind: 'approve',
          actorMembership: 'member',
          decisionRef: 'decision.autopilot.work_1.approve',
          idempotencyKey: 'idempotency.work_1.approve',
          requestedAt: nowIso,
          requestRef: 'companion_action.work_1.stale',
          workOrderRef: 'autopilot_work_order.work_1',
        },
        stale: true,
      }).reasonRef,
    ).toBe('blocker.autopilot_companion.stale_decision')
  })

  test('makes permission decisions fail closed and routes background approvals to waiting state', () => {
    const request = new AutopilotPermissionRequest({
      actionKind: 'shell',
      allowRuleRefs: ['rule.allow.shell.tests'],
      askRuleRefs: [],
      background: true,
      classifierAvailable: true,
      denyRuleRefs: [],
      hardSafetyCheckRefs: [],
      mode: 'default',
      promptAvailable: false,
      remoteApprovalAvailable: true,
      requestRef: 'permission.work_1.shell',
      riskRef: 'risk.shell.workspace_write',
      runRef: 'run.work_1',
    })

    expect(decideAutopilotPermission(request)).toMatchObject({
      decision: 'ask',
      persistTo: 'decision_queue',
      source: 'remote_approval',
      waitingState: 'waiting_for_approval',
    })
    expect(
      decideAutopilotPermission(
        new AutopilotPermissionRequest({
          ...request,
          allowRuleRefs: ['rule.allow.shell.all'],
          denyRuleRefs: ['rule.deny.shell.deploy'],
        }),
      ),
    ).toMatchObject({
      decision: 'deny',
      decisionReasonRef: 'permission.deny_rule_matched',
    })
    expect(
      decideAutopilotPermission(
        new AutopilotPermissionRequest({
          ...request,
          allowRuleRefs: [],
          background: false,
          remoteApprovalAvailable: false,
        }),
      ),
    ).toMatchObject({
      decision: 'deny',
      decisionReasonRef: 'permission.prompt_unavailable_no_remote_resolver',
    })
  })

  test('declares non-interactive mode capabilities and stable structured output exit codes', () => {
    expect(interactionModeContract('json')).toMatchObject({
      approvals: false,
      headless: true,
      prompts: false,
    })
    expect(interactionModeContract('interactive_tui')).toMatchObject({
      approvals: true,
      headless: false,
      prompts: true,
    })

    const envelope = structuredOutputEnvelope({
      artifactRefs: ['artifact.public.work_1.closeout'],
      blockerRefs: ['blocker.permission.prompt_unavailable'],
      caveatRefs: ['caveat.remote_approval_not_configured'],
      decisionRefs: ['decision.autopilot.work_1.approve'],
      generatedAt: nowIso,
      mode: 'ci',
      receiptRefs: ['receipt.permission.work_1.denied'],
      status: 'blocked',
      taskRefs: ['task.work_1.tests'],
    })

    expect(() =>
      S.decodeUnknownSync(AutopilotStructuredOutputEnvelope)(envelope),
    ).not.toThrow()
    expect(envelope.exitCode).toBe(3)
    expect(envelope.generatedAt).toBe(nowIso)
    expect(() =>
      structuredOutputEnvelope({
        ...envelope,
        artifactRefs: ['/Users/operator/private/raw_prompt.txt'],
      }),
    ).toThrow(AutopilotPackASupervisionUnsafe)
  })
})
