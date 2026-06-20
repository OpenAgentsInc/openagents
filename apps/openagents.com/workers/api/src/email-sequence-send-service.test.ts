import { describe, expect, test } from 'vitest'

import type { EmailLedgerSendResult } from './email'
import {
  type EmailSequenceSendPlan,
  type EmailSequenceSendRow,
  isEmailSequenceSendEnabled,
  makeEmailSequenceSendService,
  planEmailSequenceSend,
} from './email-sequence-send-service'

const baseRow: EmailSequenceSendRow = {
  campaignId: 'campaign-1',
  email: 'lead@example.com',
  enrollmentId: 'enrollment-1',
  idempotencyKey: 'idem-1',
  sendId: 'send-1',
  sourceAuthorityRef: 'operator.email_sequence_authoring.v1:welcome:send:step-1',
  stepId: 'step-1',
  stepKey: 'welcome',
  templateSlug: 'sequence.welcome.v1',
  displayName: 'Ada',
  userId: 'user-1',
}

describe('isEmailSequenceSendEnabled', () => {
  test('defaults to disabled (INERT) when unset or non-truthy', () => {
    expect(isEmailSequenceSendEnabled(undefined)).toBe(false)
    expect(isEmailSequenceSendEnabled('')).toBe(false)
    expect(isEmailSequenceSendEnabled('0')).toBe(false)
    expect(isEmailSequenceSendEnabled('false')).toBe(false)
    expect(isEmailSequenceSendEnabled('off')).toBe(false)
  })

  test('is enabled only for explicit truthy values', () => {
    expect(isEmailSequenceSendEnabled('1')).toBe(true)
    expect(isEmailSequenceSendEnabled('true')).toBe(true)
    expect(isEmailSequenceSendEnabled('yes')).toBe(true)
    expect(isEmailSequenceSendEnabled(' ON ')).toBe(true)
  })
})

describe('planEmailSequenceSend', () => {
  test('shapes the typed send-service plan from a send row', () => {
    const plan = planEmailSequenceSend(baseRow)

    expect(plan).toEqual({
      campaignId: 'campaign-1',
      displayName: 'Ada',
      enrollmentId: 'enrollment-1',
      idempotencyKey: 'idem-1',
      sendId: 'send-1',
      sourceAuthorityRef:
        'operator.email_sequence_authoring.v1:welcome:send:step-1',
      stepId: 'step-1',
      stepKey: 'welcome',
      templateSlug: 'sequence.welcome.v1',
      to: 'lead@example.com',
      userId: 'user-1',
    })
  })

  test('falls back to a neutral display name and null user', () => {
    const plan = planEmailSequenceSend({
      ...baseRow,
      displayName: '   ',
      userId: null,
    })

    expect(plan.displayName).toBe('there')
    expect(plan.userId).toBeNull()
  })
})

describe('makeEmailSequenceSendService (INERT default)', () => {
  test('never calls the sender when disabled, returns a dry-run', async () => {
    let calls = 0
    const service = makeEmailSequenceSendService({
      isEnabled: () => false,
      send: async () => {
        calls += 1
        return {
          emailMessageId: 'm',
          ok: true,
          providerMessageId: 'p',
        }
      },
    })

    const outcome = await service.dispatchSequenceSend(baseRow)

    expect(calls).toBe(0)
    expect(outcome.kind).toBe('dry_run')
    if (outcome.kind === 'dry_run') {
      expect(outcome.reason).toBe('send_disabled')
      expect(outcome.plan.to).toBe('lead@example.com')
    }
  })

  test('delegates to the injected sender only when armed (sent)', async () => {
    const seen: EmailSequenceSendPlan[] = []
    const service = makeEmailSequenceSendService({
      isEnabled: () => true,
      send: async plan => {
        seen.push(plan)
        return {
          emailMessageId: 'message-1',
          ok: true,
          providerMessageId: 'provider-1',
        } satisfies EmailLedgerSendResult
      },
    })

    const outcome = await service.dispatchSequenceSend(baseRow)

    expect(seen).toHaveLength(1)
    expect(seen[0]?.idempotencyKey).toBe('idem-1')
    expect(outcome.kind).toBe('sent')
    if (outcome.kind === 'sent') {
      expect(outcome.result.providerMessageId).toBe('provider-1')
    }
  })

  test('reports a failed outcome when an armed send fails', async () => {
    const service = makeEmailSequenceSendService({
      isEnabled: () => true,
      send: async () =>
        ({
          emailMessageId: 'message-1',
          errorMessage: 'vendor rejected',
          errorName: 'send_failed',
          ok: false,
        }) satisfies EmailLedgerSendResult,
    })

    const outcome = await service.dispatchSequenceSend(baseRow)

    expect(outcome.kind).toBe('failed')
    if (outcome.kind === 'failed') {
      expect(outcome.result.errorMessage).toBe('vendor rejected')
    }
  })
})
