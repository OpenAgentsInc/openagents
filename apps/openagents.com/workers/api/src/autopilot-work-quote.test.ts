import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
} from './autopilot-work-request'
import { makeAutopilotWorkQuote } from './autopilot-work-quote'

describe('Autopilot work quote service', () => {
  test('prices public free-slice work at zero from persisted request inputs', () => {
    const quote = makeAutopilotWorkQuote(
      OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    )

    expect(quote).toMatchObject({
      amountCents: 0,
      buyerPaymentMode: 'free_slice',
      currency: 'USD',
      freeSlice: true,
      paymentRequired: false,
      pricingVersion: 'openagents.autopilot_work_quote.v1',
      quoteRef:
        'quote.autopilot_work.client.example.20260609.001.0.openagents.autopilot_work_quote.v1',
    })
    expect(quote.lineItems).toEqual([
      {
        amountCents: 0,
        code: 'public_free_slice',
        descriptionRef: 'pricing.public_free_slice',
      },
    ])
  })

  test('prices paid privacy and runner requests deterministically', () => {
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['openagents_shc', 'cloud_sandbox'] as const,
        preferredRunnerKinds: ['openagents_shc'] as const,
        privacyTier: 'openagents_shc' as const,
        publicTraceAllowed: false,
        requiresSecretBroker: true,
      },
    }
    const first = makeAutopilotWorkQuote(request)
    const second = makeAutopilotWorkQuote(request)

    expect(first).toEqual(second)
    expect(first).toMatchObject({
      amountCents: 7600,
      buyerPaymentMode: 'l402',
      freeSlice: false,
      maxSpendCents: 2500,
      paymentRequired: true,
      quoteRef:
        'quote.autopilot_work.client.example.20260609.002.7600.openagents.autopilot_work_quote.v1',
    })
    expect(first.lineItems.map(item => item.code)).toEqual([
      'task_scope',
      'privacy_tier',
      'runner_class',
      'secret_broker',
      'private_trace',
    ])
  })

  test('prices Pylon-local requests from placement policy inputs', () => {
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        buyerPaymentMode: 'mdk_checkout' as const,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['requester_pylon'] as const,
        localOnlyAllowed: true,
        preferredRunnerKinds: ['requester_pylon'] as const,
        privacyTier: 'customer_local_pylon' as const,
        publicTraceAllowed: true,
        requiresSecretBroker: false,
      },
    }
    const quote = makeAutopilotWorkQuote(request)

    expect(quote).toMatchObject({
      amountCents: 4200,
      buyerPaymentMode: 'mdk_checkout',
      paymentRequired: true,
      quoteRef:
        'quote.autopilot_work.client.example.20260609.002.4200.openagents.autopilot_work_quote.v1',
    })
    expect(quote.lineItems.map(item => item.code)).toEqual([
      'task_scope',
      'privacy_tier',
      'runner_class',
      'local_only',
    ])
  })

  test('preserves an upstream persisted quote when present', () => {
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: 'quote.persisted.autopilot.123',
        quotedAmountCents: 4200,
      },
    }
    const quote = makeAutopilotWorkQuote(request)

    expect(quote).toMatchObject({
      amountCents: 4200,
      paymentRequired: true,
      quoteRef: 'quote.persisted.autopilot.123',
    })
    expect(quote.lineItems).toEqual([
      {
        amountCents: 4200,
        code: 'persisted_quote',
        descriptionRef: 'quote.persisted.autopilot.123',
      },
    ])
  })
})
