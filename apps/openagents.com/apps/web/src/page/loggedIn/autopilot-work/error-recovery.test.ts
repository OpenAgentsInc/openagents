import { describe, expect, test } from 'vitest'

import type {
  AutopilotWorkEvent,
  AutopilotWorkProjection,
  AutopilotWorkState,
} from '../model'
import {
  buildForgeErrorRecoveryInput,
  projectForgeErrorRecovery,
} from './error-recovery'

const work = (
  state: AutopilotWorkState,
  overrides: Partial<AutopilotWorkProjection> = {},
): AutopilotWorkProjection =>
  ({
    accessRequestRefs: [],
    accessRequirements: [],
    assignmentIntents: [],
    buyerPaymentProofRef: null,
    clientRequestRef: 'client.public.work_1',
    createdAt: '2026-06-16T15:00:00.000Z',
    eventStreamRef: 'event-stream.public.work_1',
    executionCloseout: null,
    fallbackLeaseIntents: [],
    funding: {},
    generatedAt: '2026-06-16T16:00:00.000Z',
    idempotent: false,
    nextAction: {
      callerActionRefs: [],
      reasonRefs: [],
      retryAfterSeconds: null,
      state,
    },
    paymentChallenge: null,
    paymentChallengeRef: null,
    placementDecision: { selectedRunnerKind: 'requester_pylon' },
    placementPolicy: {},
    promiseRef: {
      blockerRefs: [],
      promiseId: 'autopilot.mission_briefing.v1',
      registryVersion: '2026-06-15.6',
    },
    pylonAssignmentIntents: [],
    quote: {},
    repositoryAuthorities: [],
    reviewDecision: null,
    state,
    statusUrlRef: 'status.public.work_1',
    taskRefs: ['task.public.work_1'],
    tasks: [],
    updatedAt: '2026-06-16T16:00:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

const event = (
  sequence: number,
  state: AutopilotWorkState,
  overrides: Partial<AutopilotWorkEvent> = {},
): AutopilotWorkEvent =>
  ({
    eventKind: state === 'scheduled' ? 'scheduled' : 'running',
    eventRef: `event.public.work_1.${sequence}`,
    occurredAt: `2026-06-16T15:0${sequence}:00.000Z`,
    publicSafe: true,
    sequence,
    state,
    taskRefs: ['task.public.work_1'],
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkEvent

describe('Forge error recovery projection', () => {
  test('projects explicit typed errors and recovery events without authority', () => {
    const view = projectForgeErrorRecovery({
      errors: [
        {
          category: 'ProviderRateLimited',
          diagnosticRef: 'diagnostic.public.work_1.rate_limit',
          errorRef: 'error.public.work_1.rate_limit',
          occurredAt: '2026-06-16T15:30:00.000Z',
          originServiceRef: 'adapter.openai.public',
          publicMessage: 'Provider retry is scheduled.',
          recoveryStrategy: 'backoff_retry',
          redactionClass: 'public',
          relatedRefs: [
            'task.public.work_1',
            'idempotency.public.work_1.provider_retry',
          ],
          retryability: 'retryable',
          severity: 'warning',
        },
      ],
      events: [
        {
          errorRef: 'error.public.work_1.rate_limit',
          eventRef: 'recovery-event.public.work_1.retry_scheduled',
          kind: 'recovery.retry_scheduled',
          occurredAt: '2026-06-16T15:31:00.000Z',
          publicSafe: true,
          receiptRefs: ['receipt.public.work_1.retry_policy'],
          recoveryStrategy: 'backoff_retry',
        },
      ],
      generatedAt: '2026-06-16T16:00:00.000Z',
      recoveryRef: 'error-recovery.public.work_1',
      workOrderRef: 'work_1',
    })

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        automaticRetryAuthority: false,
        deploymentAuthority: false,
        publicClaimAuthority: false,
        runtimeMutationAuthority: false,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      publicSafe: true,
      status: 'recovering',
      workOrderRef: 'work_1',
    })
    expect(view.errors[0]).toMatchObject({
      category: 'ProviderRateLimited',
      recoveryStrategy: 'backoff_retry',
      retryability: 'retryable',
    })
    expect(view.events[0]).toMatchObject({
      eventRef: 'recovery-event.public.work_1.retry_scheduled',
      kind: 'recovery.retry_scheduled',
    })
    expect(view.blockerRefs).toEqual([])
  })

  test('derives fail-closed blockers when invalid Runs lack recovery evidence', () => {
    const view = projectForgeErrorRecovery(
      buildForgeErrorRecoveryInput(work('invalid'), [event(1, 'invalid')]),
    )

    expect(view.status).toBe('failed_closed')
    expect(view.errors[0]?.category).toBe('InternalBug')
    expect(view.events[0]?.kind).toBe('run.failed_closed')
    expect(view.blockerRefs).toContain(
      'forge-error-recovery-blocker:work_1:missing-error-recovery-evidence',
    )
    expect(view.blockerRefs).toContain(
      'forge-error-recovery-blocker:work_1:terminal-fail-closed',
    )
  })

  test('blocks automatic backoff recovery without idempotency evidence', () => {
    const unsafeRetry = projectForgeErrorRecovery({
      errors: [
        {
          category: 'NetworkTransient',
          errorRef: 'error.public.work_1.network',
          recoveryStrategy: 'backoff_retry',
          relatedRefs: ['task.public.work_1'],
          retryability: 'retryable',
        },
      ],
      events: [
        {
          errorRef: 'error.public.work_1.network',
          eventRef: 'recovery-event.public.work_1.retry_scheduled',
          kind: 'recovery.retry_scheduled',
          occurredAt: '2026-06-16T15:31:00.000Z',
          publicSafe: true,
          recoveryStrategy: 'backoff_retry',
        },
      ],
      generatedAt: '2026-06-16T16:00:00.000Z',
      workOrderRef: 'work_1',
    })
    const guardedRetry = projectForgeErrorRecovery({
      errors: [
        {
          category: 'NetworkTransient',
          errorRef: 'error.public.work_1.network',
          recoveryStrategy: 'backoff_retry',
          relatedRefs: [
            'task.public.work_1',
            'idempotency.public.work_1.retry',
          ],
          retryability: 'retryable',
        },
      ],
      events: [
        {
          errorRef: 'error.public.work_1.network',
          eventRef: 'recovery-event.public.work_1.retry_scheduled',
          kind: 'recovery.retry_scheduled',
          occurredAt: '2026-06-16T15:31:00.000Z',
          publicSafe: true,
          recoveryStrategy: 'backoff_retry',
        },
      ],
      generatedAt: '2026-06-16T16:00:00.000Z',
      workOrderRef: 'work_1',
    })

    expect(unsafeRetry.status).toBe('blocked')
    expect(unsafeRetry.blockerRefs.some(ref =>
      ref.includes('mutation-retry-safety-unproven')
    )).toBe(true)
    expect(guardedRetry.status).toBe('recovering')
    expect(guardedRetry.blockerRefs.some(ref =>
      ref.includes('mutation-retry-safety-unproven')
    )).toBe(false)
  })

  test('omits unsafe private diagnostics before projection', () => {
    const view = projectForgeErrorRecovery({
      errors: [
        {
          category: 'ToolExecutionFailed',
          causeRef: 'raw stack trace at /Users/christopher/app.ts:1:1',
          diagnosticRef: '/Users/christopher/.codex/private.jsonl',
          errorRef: 'error.public.work_1.tool',
          originServiceRef: 'raw shell command $(cat ~/.ssh/id_rsa)',
          publicMessage: 'provider payload sk-private',
          relatedRefs: [
            'task.public.work_1',
            'raw prompt /Users/christopher/private.md',
          ],
          recoveryStrategy: 'structured_tool_error',
          retryability: 'conditional',
        },
      ],
      events: [
        {
          blockerRefs: [
            'recovery-blocker.public.safe',
            'shell log /Users/christopher/run.log',
          ],
          errorRef: 'error.public.work_1.tool',
          eventRef: 'recovery-event.public.work_1.structured_error',
          kind: 'error.recorded',
          occurredAt: '2026-06-16T15:31:00.000Z',
          publicSafe: true,
          receiptRefs: [
            'receipt.public.work_1.structured_error',
            'provider payload sk-private',
          ],
        },
      ],
      generatedAt: '2026-06-16T16:00:00.000Z',
      recoveryRef: 'error-recovery.public.work_1',
      workOrderRef: 'work_1',
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.errors).toHaveLength(1)
    expect(view.errors[0]?.relatedRefs).toEqual(['task.public.work_1'])
    expect(view.events[0]?.receiptRefs).toEqual([
      'receipt.public.work_1.structured_error',
    ])
    expect(view.blockerRefs).toContain(
      'forge-error-recovery-blocker:work_1:unsafe-error-recovery-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
    expect(payload).not.toContain('stack trace')
  })

  test('keeps healthy Runs clear when no recovery evidence is present', () => {
    const view = projectForgeErrorRecovery(
      buildForgeErrorRecoveryInput(work('delivered'), []),
    )

    expect(view.status).toBe('clear')
    expect(view.errors).toEqual([])
    expect(view.events).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })
})
