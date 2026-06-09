import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsSiteMdkWebhookSource,
  digestRefForSiteMdkWebhookBody,
  verifyOpenAgentsSiteMdkWebhook,
} from './site-mdk-webhooks'

const encoder = new TextEncoder()

const hex = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const base64 = (bytes: ArrayBuffer): string =>
  btoa(String.fromCharCode(...new Uint8Array(bytes)))

const sign = async (secret: string, payload: string): Promise<ArrayBuffer> => {
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )

  return crypto.subtle.sign('HMAC', key, encoder.encode(payload))
}

const body = JSON.stringify({
  createdAt: '2026-06-07T14:01:00.000Z',
  data: {
    checkout: {
      id: 'checkout_live_123',
      status: 'PAYMENT_RECEIVED',
    },
  },
  id: 'evt_checkout_completed_123',
  type: 'checkout.completed',
})

const verifyWith = async (
  source: OpenAgentsSiteMdkWebhookSource,
  headers: Headers,
) =>
  verifyOpenAgentsSiteMdkWebhook({
    body,
    config: {
      bindingRef: `webhook_binding.openagents.hosted_mdk.${source}`,
      secret: 'test-webhook-secret',
      source,
    },
    headers,
    nowIso: '2026-06-07T14:02:00.000Z',
  })

describe('OpenAgents Site MDK webhooks', () => {
  test('verifies dashboard Standard Webhooks and projects safe event refs', async () => {
    const digest = await sign(
      'test-webhook-secret',
      `evt_standard_123.1780831200.${body}`,
    )
    const result = await verifyWith(
      'dashboard_standard_webhooks',
      new Headers({
        'webhook-id': 'evt_standard_123',
        'webhook-signature': `v1,${base64(digest)}`,
        'webhook-timestamp': '1780831200',
      }),
    )

    expect(result).toMatchObject({
      _tag: 'Verified',
      event: {
        checkoutRef: 'mdk_checkout.checkout_live_123',
        checkoutStatus: 'payment_received',
        eventKind: 'payment_received',
        providerEventRef:
          'provider_event.mdk.dashboard_standard_webhooks.evt_standard_123',
        signatureBindingRef:
          'webhook_binding.openagents.hosted_mdk.dashboard_standard_webhooks',
      },
    })
    expect(
      result._tag === 'Verified'
        ? result.event.eventBodyDigestRef
        : 'invalid',
    ).toBe(await digestRefForSiteMdkWebhookBody(
      'dashboard_standard_webhooks',
      body,
    ))
    expect(JSON.stringify(result)).not.toMatch(
      /(test-webhook-secret|lnbc|payment_preimage|mnemonic)/i,
    )
  })

  test('verifies daemon invoice HMAC signatures', async () => {
    const digest = await sign('test-webhook-secret', `1780831200.${body}`)
    const result = await verifyWith(
      'daemon_invoice_hmac',
      new Headers({
        'x-mdk-signature': hex(digest),
        'x-mdk-timestamp': '1780831200',
      }),
    )

    expect(result).toMatchObject({
      _tag: 'Verified',
      event: {
        checkoutRef: 'mdk_checkout.checkout_live_123',
        checkoutStatus: 'payment_received',
        signatureBindingRef:
          'webhook_binding.openagents.hosted_mdk.daemon_invoice_hmac',
      },
    })
  })

  test('verifies SDK node-control webhooks and rejects bad signatures', async () => {
    const verified = await verifyWith(
      'sdk_node_control',
      new Headers({
        'x-moneydevkit-webhook-secret': 'test-webhook-secret',
      }),
    )
    const rejected = await verifyWith(
      'sdk_node_control',
      new Headers({
        'x-moneydevkit-webhook-secret': 'wrong-secret',
      }),
    )

    expect(verified._tag).toBe('Verified')
    expect(rejected).toEqual({
      _tag: 'Invalid',
      reason: 'invalid_signature',
    })
  })
})
