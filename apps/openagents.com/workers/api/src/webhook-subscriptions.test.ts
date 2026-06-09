import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  WebhookDeliveryProjection,
  WebhookEventProjection,
  ProgramRunReceiptWebhookProjection,
  WebhookSubscriptionProjection,
  WebhookSubscriptionUnsafe,
  exampleProgramRunReceiptWebhook,
  exampleWebhookDelivery,
  exampleWebhookEvent,
  exampleWebhookSubscription,
  programRunReceiptWebhookAuthorityIsContractOnly,
  programRunReceiptWebhookIdempotencyRef,
  programRunReceiptWebhookReplayKey,
  projectProgramRunReceiptWebhook,
  projectWebhookDelivery,
  projectWebhookEvent,
  projectWebhookSubscription,
  webhookDeliveryCanRetry,
  webhookDeliveryIdempotencyKey,
  webhookEventReplayKeyForDelivery,
  webhookProjectionHasPrivateMaterial,
} from './webhook-subscriptions'

const nowIso = '2026-06-06T22:30:00.000Z'

describe('webhook subscription contracts', () => {
  test('projects active subscriptions with secret-binding refs only for operators', () => {
    const publicProjection = projectWebhookSubscription(
      exampleWebhookSubscription(),
      'public',
      nowIso,
    )
    const operatorProjection = projectWebhookSubscription(
      exampleWebhookSubscription(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(WebhookSubscriptionProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      authMode: 'owner_grant',
      eventFamilies: ['receipt', 'site_revision', 'workroom'],
      maxAttempts: 3,
      retryPolicy: 'exponential',
      secretBindingRef: null,
      status: 'active',
      statusLabel: 'Active',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(operatorProjection.secretBindingRef).toBe(
      'secret_binding.webhook.customer_ops',
    )
    expect(JSON.stringify(publicProjection)).not.toContain(
      '2026-06-06T22:25:00.000Z',
    )
    expect(webhookProjectionHasPrivateMaterial(publicProjection)).toBe(false)
  })

  test('projects event and delivery records with replay-safe keys and friendly time labels', () => {
    const eventProjection = projectWebhookEvent(
      exampleWebhookEvent(),
      'customer',
      nowIso,
    )
    const deliveryProjection = projectWebhookDelivery(
      exampleWebhookDelivery(),
      'customer',
      nowIso,
    )

    expect(S.decodeUnknownSync(WebhookEventProjection)(eventProjection))
      .toEqual(eventProjection)
    expect(S.decodeUnknownSync(WebhookDeliveryProjection)(deliveryProjection))
      .toEqual(deliveryProjection)
    expect(eventProjection).toMatchObject({
      family: 'site_revision',
      occurredAtDisplay: '4 minutes ago',
      status: 'ready',
      statusLabel: 'Ready',
    })
    expect(deliveryProjection).toMatchObject({
      canRetry: true,
      failureClass: 'timeout',
      failureClassLabel: 'Timeout',
      nextAttemptAtDisplay: 'Just now',
      state: 'retry_scheduled',
      stateLabel: 'Retry scheduled',
    })
    expect(webhookDeliveryCanRetry(exampleWebhookDelivery())).toBe(true)
    expect(webhookEventReplayKeyForDelivery(
      'webhook.sub.customer_ops',
      'event.webhook.site_revision_ready',
    )).toBe(exampleWebhookDelivery().replayKey)
    expect(webhookDeliveryIdempotencyKey(
      'webhook.sub.customer_ops',
      'event.webhook.site_revision_ready',
      1,
    )).toBe(exampleWebhookDelivery().idempotencyKey)
  })

  test('keeps non-retriable delivery failures from retrying', () => {
    const base = exampleWebhookDelivery()

    ;[
      { ...base, failureClass: 'policy_denied' as const },
      { ...base, failureClass: 'redaction_failed' as const },
      { ...base, failureClass: 'secret_missing' as const },
      { ...base, attempt: 3 },
      { ...base, retryPolicy: 'none' as const },
      { ...base, state: 'delivered' as const },
    ].forEach(delivery => {
      expect(webhookDeliveryCanRetry(delivery)).toBe(false)
      expect(projectWebhookDelivery(delivery, 'operator', nowIso).canRetry)
        .toBe(false)
    })
  })

  test('rejects raw webhook secrets, payloads, provider responses, tokens, customer data, wallet/payment material, and raw runner logs', () => {
    const subscription = exampleWebhookSubscription()
    const event = exampleWebhookEvent()
    const delivery = exampleWebhookDelivery()

    ;[
      () =>
        projectWebhookSubscription({
          ...subscription,
          endpointRef: 'endpoint.raw_webhook_payload',
        }, 'operator', nowIso),
      () =>
        projectWebhookSubscription({
          ...subscription,
          secretBindingRef: 'webhook_secret.raw_value',
        }, 'operator', nowIso),
      () =>
        projectWebhookSubscription({
          ...subscription,
          ownerRef: 'customer_email_ben@example.com',
        }, 'operator', nowIso),
      () =>
        projectWebhookEvent({
          ...event,
          payloadDigestRef: 'raw_provider_response.body',
        }, 'operator', nowIso),
      () =>
        projectWebhookEvent({
          ...event,
          sourceAuthorityRefs: ['bearer token_secret'],
        }, 'operator', nowIso),
      () =>
        projectWebhookDelivery({
          ...delivery,
          failureSummaryRef: 'raw_runner_log.failure',
        }, 'operator', nowIso),
      () =>
        projectWebhookDelivery({
          ...delivery,
          deliveryReceiptRefs: ['payment_hash.raw_123'],
        }, 'operator', nowIso),
      () =>
        projectWebhookDelivery({
          ...delivery,
          endpointRef: 'wallet_state.local_node',
        }, 'operator', nowIso),
    ].forEach(action => {
      expect(action).toThrow(WebhookSubscriptionUnsafe)
    })
  })

  test('requires event families and non-negative attempts', () => {
    expect(() =>
      projectWebhookSubscription({
        ...exampleWebhookSubscription(),
        eventFamilies: [],
      }, 'operator', nowIso),
    ).toThrow(WebhookSubscriptionUnsafe)
    expect(() =>
      projectWebhookDelivery({
        ...exampleWebhookDelivery(),
        attempt: -1,
      }, 'operator', nowIso),
    ).toThrow(WebhookSubscriptionUnsafe)
  })
})

describe('Program Run and receipt webhook subscription contracts', () => {
  test('projects focused subscriptions with lifecycle phase separation and no-send authority', () => {
    const contract = exampleProgramRunReceiptWebhook()
    const publicProjection = projectProgramRunReceiptWebhook(
      contract,
      'public',
      nowIso,
    )
    const operatorProjection = projectProgramRunReceiptWebhook(
      contract,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(ProgramRunReceiptWebhookProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      authEscalationAllowed: false,
      createdAtDisplay: '15 minutes ago',
      deliveryAttemptRecorded: true,
      deliveryPreparationRecorded: true,
      deliveryQueueEnqueueAllowed: false,
      eventFamilies: ['program_run', 'receipt'],
      eventSelectionRecorded: true,
      externalWebhookCallAllowed: false,
      lifecycleLabel: 'Receipt recorded',
      lifecycleState: 'receipt_recorded',
      paymentMutationAllowed: false,
      programRunMutationAllowed: false,
      receiptMutationAllowed: false,
      receiptRecorded: true,
      replayWindowRecorded: true,
      retryScheduled: true,
      revoked: false,
      scopedAuthRefs: [],
      secretMaterialAllowed: false,
      status: 'active',
      statusLabel: 'Active',
      subscriptionRegistrationRecorded: true,
      updatedAtDisplay: '5 minutes ago',
    })
    expect(publicProjection.endpointRefs).toEqual([
      'endpoint.public.partner_receipts',
    ])
    expect(publicProjection.subscriberRefs).toEqual([
      'subscriber.public.partner_receipts',
    ])
    expect(publicProjection.operatorDiagnosticRefs).toEqual([])
    expect(operatorProjection.scopedAuthRefs).toEqual([
      'auth.private.owner_grant.customer_ops',
    ])
    expect(operatorProjection.operatorDiagnosticRefs).toEqual([
      'diagnostic.operator.webhook.program_run_receipt',
    ])
    expect(programRunReceiptWebhookAuthorityIsContractOnly(
      contract.authority,
    )).toBe(true)
    expect(webhookProjectionHasPrivateMaterial(publicProjection)).toBe(false)
    expect(JSON.stringify(publicProjection)).not.toContain(
      '2026-06-06T22:25:00.000Z',
    )
  })

  test('derives replay and idempotency refs for receipt lifecycle delivery', () => {
    expect(programRunReceiptWebhookReplayKey(
      'webhook.sub.program_run_receipt',
      'topic.receipt.lifecycle',
    )).toBe(
      'program_run_receipt_webhook.replay:webhook.sub.program_run_receipt:topic.receipt.lifecycle',
    )
    expect(programRunReceiptWebhookIdempotencyRef(
      'webhook.sub.program_run_receipt',
      'topic.receipt.lifecycle',
      'receipt_recorded',
    )).toBe(exampleProgramRunReceiptWebhook().idempotencyRefs[0])
  })

  test('tracks retry, replay, and revocation state without granting delivery authority', () => {
    const revoked = projectProgramRunReceiptWebhook(
      {
        ...exampleProgramRunReceiptWebhook(),
        lifecycleState: 'revoked',
        revocationRefs: ['revocation.webhook.owner_disabled'],
        status: 'revoked',
      },
      'customer',
      nowIso,
    )

    expect(revoked).toMatchObject({
      deliveryQueueEnqueueAllowed: false,
      externalWebhookCallAllowed: false,
      lifecycleLabel: 'Revoked',
      replayWindowRecorded: true,
      retryScheduled: true,
      revoked: true,
      statusLabel: 'Revoked',
    })
  })

  test('rejects non Program Run/receipt families, negative attempts, external-send authority, and unsafe refs', () => {
    const contract = exampleProgramRunReceiptWebhook()

    ;[
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          eventFamilies: ['site_revision'],
        }, 'operator', nowIso),
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          maxAttempts: -1,
        }, 'operator', nowIso),
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          authority: {
            ...contract.authority,
            noExternalWebhookCall: false,
          },
        }, 'operator', nowIso),
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          endpointRefs: ['https://customer.example.com/webhook?token=abc'],
        }, 'operator', nowIso),
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          deliveryAttemptRefs: ['raw_webhook_payload.full_body'],
        }, 'operator', nowIso),
      () =>
        projectProgramRunReceiptWebhook({
          ...contract,
          operatorDiagnosticRefs: ['raw_runner_log.webhook_dispatch'],
        }, 'operator', nowIso),
    ].forEach(action => {
      expect(action).toThrow(WebhookSubscriptionUnsafe)
    })
  })
})
