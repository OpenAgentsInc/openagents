import { describe, expect, test } from 'vitest'

import {
  buildWhiteLabelOperatorProvisioningProjection,
} from './white-label-operator-provisioning'

const baseInput = () => ({
  clientWorkrooms: [
    {
      surface: 'customer' as const,
      teamId: 'team_agency_operator',
      workroomId: 'client_a_launch',
    },
  ],
  generatedAt: '2026-07-03T00:00:00.000Z',
  hostname: {
    hostname: 'Client-Portal.Example.com.',
    status: 'active' as const,
    teamId: 'team_agency_operator',
  },
  operatorTenantRef: 'tenant.agency_operator.opaque',
  payoutLedger: {
    policyRefs: ['policy.partner_payout.white_label_operator.v1'],
    receiptRefs: ['receipt.partner_payout.hosted_mdk.opaque_001'],
    settlementState: 'settled' as const,
  },
  theme: {
    tokenSetRef: 'theme.token_set.white_label.opaque_001',
    tokens: [
      {
        token: '--oa-color-accent',
        valueRef: 'theme.value.accent.opaque',
      },
      {
        token: '--oa-radius-card',
        valueRef: 'theme.value.radius.opaque',
      },
    ],
  },
})

describe('buildWhiteLabelOperatorProvisioningProjection', () => {
  test('marks a white-label operator client provisioned only when every BF-8.3 receipt is present', () => {
    const projection = buildWhiteLabelOperatorProvisioningProjection(baseInput())

    expect(projection.status).toBe('provisioned')
    expect(projection.blockerRefs).toEqual([])
    expect(projection.hostname).toBe('client-portal.example.com')
    expect(projection.clientWorkroomRefs).toEqual([
      'workroom.customer.client_a_launch',
    ])
    expect(projection.themeTokenRefs).toContain(
      '--oa-color-accent=theme.value.accent.opaque',
    )
    expect(projection.payoutReceiptRefs).toEqual([
      'receipt.partner_payout.hosted_mdk.opaque_001',
    ])
    expect(projection.sourceRefs).toContain('docs/fable/ROADMAP_BIZ.md#BF-8.3')
  })

  test('keeps the projection blocked until hostname, workroom, theme, and settled payout evidence all exist', () => {
    const projection = buildWhiteLabelOperatorProvisioningProjection({
      ...baseInput(),
      clientWorkrooms: [
        {
          surface: 'customer',
          teamId: 'team_other',
          workroomId: 'client_a_launch',
        },
      ],
      hostname: {
        hostname: 'client-portal.example.com',
        status: 'verified',
        teamId: 'team_agency_operator',
      },
      payoutLedger: {
        policyRefs: [],
        receiptRefs: ['receipt.partner_payout.hosted_mdk.opaque_001'],
        settlementState: 'pending',
      },
      theme: {
        tokenSetRef: 'theme.token_set.white_label.opaque_001',
        tokens: [{ token: '--not-product-css', valueRef: 'raw.#ffffff' }],
      },
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'blocker.white_label.hostname_not_active',
      'blocker.white_label.theme_token_evidence_missing',
      'blocker.white_label.client_workroom_missing',
      'blocker.white_label.payout_policy_missing',
      'blocker.white_label.settled_payout_receipt_missing',
    ])
  })

  test('redacts unsafe refs and hostnames from the public-safe readout', () => {
    const projection = buildWhiteLabelOperatorProvisioningProjection({
      ...baseInput(),
      hostname: {
        hostname: 'localhost',
        status: 'active',
        teamId: 'team_agency_operator',
      },
      operatorTenantRef: 'tenant.private.customer_email',
      payoutLedger: {
        policyRefs: ['policy.partner_payout.white_label_operator.v1'],
        receiptRefs: [
          'receipt.partner_payout.hosted_mdk.opaque_001',
          'receipt.partner_payout.raw_payment_hash_secret',
        ],
        settlementState: 'settled',
      },
    })

    expect(projection.status).toBe('blocked')
    expect(projection.hostname).toBe('redacted.hostname')
    expect(projection.operatorTenantRef).toBe('redacted.operator_tenant')
    expect(projection.payoutReceiptRefs).toEqual([
      'receipt.partner_payout.hosted_mdk.opaque_001',
    ])
    expect(projection.blockerRefs).toContain(
      'blocker.white_label.hostname_not_public_safe',
    )
    expect(projection.blockerRefs).toContain(
      'blocker.white_label.operator_tenant_ref_not_public_safe',
    )
  })
})
