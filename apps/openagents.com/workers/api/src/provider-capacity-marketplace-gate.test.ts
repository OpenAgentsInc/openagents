import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProviderCapacityMarketplaceGateUnsafe,
  ProviderCapacityMarketplaceProvider,
  projectProviderCapacityMarketplaceGate,
  providerCapacityMarketplaceGateHasPrivateMaterial,
} from './provider-capacity-marketplace-gate'

const chatGptEndToEndInput = {
  accountSchemaRefs: ['schema.public.provider_capacity.chatgpt_account_v1'],
  assignmentModeRefs: ['mode.public.provider_capacity.agentic_task_v1'],
  assignmentDispatchRefs: ['dispatch.public.provider_capacity.chatgpt_001'],
  assignmentReceiptRefs: ['receipt.public.provider_capacity.chatgpt_001'],
  connectorHealthRefs: ['health.public.provider_capacity.chatgpt_healthy'],
  meteringReceiptRefs: ['meter.public.provider_capacity.chatgpt_001'],
  pricingMode: 'agentic_work' as const,
  pricingPolicyRefs: ['pricing.public.provider_capacity.agentic_work_001'],
  provider: 'chatgpt_codex' as const,
  providerGrantRefs: ['grant.public.provider_capacity.chatgpt_001'],
  quotaEvidenceRefs: ['quota.public.provider_capacity.chatgpt_available'],
  routePolicyRefs: ['policy.public.provider_capacity.chatgpt_routes_001'],
  secretPolicyRefs: ['policy.public.provider_capacity.chatgpt_auth_material_boundary_v1'],
  tosBoundaryRefs: ['tos.public.provider_capacity.chatgpt_agentic_work_001'],
}

describe('Provider capacity marketplace gate', () => {
  test('keeps provider routing typed instead of accepting generic strings', () => {
    expect(
      S.decodeUnknownSync(ProviderCapacityMarketplaceProvider)('chatgpt_codex'),
    ).toBe('chatgpt_codex')
    expect(() =>
      S.decodeUnknownSync(ProviderCapacityMarketplaceProvider)('openai'),
    ).toThrow()
    expect(() =>
      S.decodeUnknownSync(ProviderCapacityMarketplaceProvider)('fake_provider'),
    ).toThrow()
  })

  test('keeps ChatGPT account connection separate from resale authorization', () => {
    const gate = projectProviderCapacityMarketplaceGate({
      pricingMode: 'agentic_work',
      provider: 'chatgpt_codex',
      providerGrantRefs: ['grant.public.provider_capacity.chatgpt_001'],
    })

    expect(gate).toMatchObject({
      assignmentDispatchAllowed: false,
      assignmentReceiptClaimAllowed: false,
      connectorState: 'configured',
      marketableCapacityCopyAllowed: false,
      paidSettlementClaimAllowed: false,
      provider: 'chatgpt_codex',
      sellableCapacityListed: false,
      state: 'blocked',
    })
    expect(gate.blockerRefs).toEqual([
      'blocker.public.provider_capacity.account_schema_missing',
      'blocker.public.provider_capacity.assignment_dispatch_missing',
      'blocker.public.provider_capacity.assignment_mode_missing',
      'blocker.public.provider_capacity.assignment_receipt_missing',
      'blocker.public.provider_capacity.connector_health_missing',
      'blocker.public.provider_capacity.metering_receipt_missing',
      'blocker.public.provider_capacity.pricing_policy_missing',
      'blocker.public.provider_capacity.quota_evidence_missing',
      'blocker.public.provider_capacity.route_policy_missing',
      'blocker.public.provider_capacity.secret_ref_policy_missing',
      'blocker.public.provider_capacity.settlement_receipt_missing',
      'blocker.public.provider_capacity.tos_boundary_missing',
    ])
    expect(gate.caveatRefs).toContain(
      'caveat.public.provider_capacity.provider_connection_is_not_resale_authorization',
    )
  })

  test('allows first-provider assignment receipts without claiming Bitcoin settlement', () => {
    const gate = projectProviderCapacityMarketplaceGate(chatGptEndToEndInput)

    expect(gate).toMatchObject({
      assignmentDispatchAllowed: true,
      assignmentReceiptClaimAllowed: true,
      connectorState: 'payable',
      marketableCapacityCopyAllowed: false,
      paidSettlementClaimAllowed: false,
      sellableCapacityListed: true,
      state: 'assignment_receipted',
    })
    expect(gate.blockerRefs).toEqual([
      'blocker.public.provider_capacity.settlement_receipt_missing',
    ])
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.provider_capacity.marketplace_monetization_blocked',
    ])
  })

  test('requires settlement receipts before provider capacity monetization copy', () => {
    const gate = projectProviderCapacityMarketplaceGate({
      ...chatGptEndToEndInput,
      settlementReceiptRefs: [
        'settlement.public.provider_capacity.chatgpt_001',
      ],
    })

    expect(gate).toMatchObject({
      assignmentDispatchAllowed: true,
      assignmentReceiptClaimAllowed: true,
      connectorState: 'settled',
      marketableCapacityCopyAllowed: true,
      paidSettlementClaimAllowed: true,
      sellableCapacityListed: true,
      state: 'settled',
    })
    expect(gate.publicCopyRefs).toEqual([
      'copy.public.provider_capacity.bitcoin_settlement_receipts_visible',
    ])
  })

  test('blocks base inference resale even when assignment evidence exists', () => {
    const gate = projectProviderCapacityMarketplaceGate({
      ...chatGptEndToEndInput,
      pricingMode: 'base_inference_resale',
      settlementReceiptRefs: [
        'settlement.public.provider_capacity.chatgpt_001',
      ],
    })

    expect(gate).toMatchObject({
      assignmentDispatchAllowed: false,
      assignmentReceiptClaimAllowed: false,
      connectorState: 'healthy',
      marketableCapacityCopyAllowed: false,
      paidSettlementClaimAllowed: false,
      sellableCapacityListed: false,
      state: 'blocked',
    })
    expect(gate.blockerRefs).toContain(
      'blocker.public.provider_capacity.base_inference_resale_not_authorized',
    )
  })

  test('labels Claude and Venice as planned or blocked unsupported providers', () => {
    expect(
      projectProviderCapacityMarketplaceGate({
        pricingMode: 'agentic_work',
        provider: 'claude',
      }),
    ).toMatchObject({
      connectorState: 'unsupported',
      marketableCapacityCopyAllowed: false,
      providerLabel: 'Claude',
      sellableCapacityListed: false,
      state: 'planned_unsupported',
    })

    expect(
      projectProviderCapacityMarketplaceGate({
        meteringReceiptRefs: ['meter.public.provider_capacity.venice_001'],
        pricingMode: 'agentic_work',
        provider: 'venice',
        providerGrantRefs: ['grant.public.provider_capacity.venice_001'],
      }),
    ).toMatchObject({
      assignmentDispatchAllowed: false,
      connectorState: 'unsupported',
      marketableCapacityCopyAllowed: false,
      providerLabel: 'Venice',
      sellableCapacityListed: false,
      state: 'blocked_unsupported',
    })
  })

  test('requires health and quota evidence before capacity can be listed as sellable', () => {
    const configured = projectProviderCapacityMarketplaceGate({
      accountSchemaRefs: ['schema.public.provider_capacity.chatgpt_account_v1'],
      pricingMode: 'agentic_work',
      provider: 'chatgpt_codex',
      providerGrantRefs: ['grant.public.provider_capacity.chatgpt_001'],
      secretPolicyRefs: [
        'policy.public.provider_capacity.chatgpt_auth_material_boundary_v1',
      ],
    })
    const healthy = projectProviderCapacityMarketplaceGate({
      accountSchemaRefs: ['schema.public.provider_capacity.chatgpt_account_v1'],
      connectorHealthRefs: ['health.public.provider_capacity.chatgpt_healthy'],
      pricingMode: 'agentic_work',
      provider: 'chatgpt_codex',
      providerGrantRefs: ['grant.public.provider_capacity.chatgpt_001'],
      quotaEvidenceRefs: ['quota.public.provider_capacity.chatgpt_available'],
      secretPolicyRefs: [
        'policy.public.provider_capacity.chatgpt_auth_material_boundary_v1',
      ],
    })
    const assignable = projectProviderCapacityMarketplaceGate({
      accountSchemaRefs: ['schema.public.provider_capacity.chatgpt_account_v1'],
      assignmentModeRefs: ['mode.public.provider_capacity.agentic_task_v1'],
      connectorHealthRefs: ['health.public.provider_capacity.chatgpt_healthy'],
      meteringReceiptRefs: ['meter.public.provider_capacity.chatgpt_001'],
      pricingMode: 'agentic_work',
      pricingPolicyRefs: ['pricing.public.provider_capacity.agentic_work_001'],
      provider: 'chatgpt_codex',
      providerGrantRefs: ['grant.public.provider_capacity.chatgpt_001'],
      quotaEvidenceRefs: ['quota.public.provider_capacity.chatgpt_available'],
      routePolicyRefs: ['policy.public.provider_capacity.chatgpt_routes_001'],
      secretPolicyRefs: [
        'policy.public.provider_capacity.chatgpt_auth_material_boundary_v1',
      ],
      tosBoundaryRefs: [
        'tos.public.provider_capacity.chatgpt_agentic_work_001',
      ],
    })

    expect(configured).toMatchObject({
      connectorState: 'configured',
      sellableCapacityListed: false,
    })
    expect(healthy).toMatchObject({
      connectorState: 'healthy',
      sellableCapacityListed: false,
    })
    expect(assignable).toMatchObject({
      assignmentDispatchAllowed: true,
      connectorState: 'assignable',
      sellableCapacityListed: true,
    })
  })

  test('rejects provider tokens, quota payloads, raw metering, payment, wallet, and timestamps', () => {
    for (const input of [
      {
        accountSchemaRefs: ['raw_provider_schema.payload'],
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
      },
      {
        meteringReceiptRefs: ['quota_payload.raw.100000_tokens'],
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
      },
      {
        connectorHealthRefs: ['api_key.sk-live-secret000000000'],
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
      },
      {
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
        routePolicyRefs: ['2026-06-08T12:00:00Z'],
      },
      {
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
        settlementReceiptRefs: ['payment_hash.provider_secret'],
      },
      {
        pricingMode: 'agentic_work' as const,
        provider: 'chatgpt_codex' as const,
        tosBoundaryRefs: ['wallet.private.capacity_market'],
      },
    ]) {
      expect(() => projectProviderCapacityMarketplaceGate(input)).toThrow(
        ProviderCapacityMarketplaceGateUnsafe,
      )
    }
  })

  test('keeps settled projection public-safe', () => {
    const gate = projectProviderCapacityMarketplaceGate({
      ...chatGptEndToEndInput,
      settlementReceiptRefs: [
        'settlement.public.provider_capacity.chatgpt_001',
      ],
    })
    const json = JSON.stringify(gate)

    expect(providerCapacityMarketplaceGateHasPrivateMaterial(gate)).toBe(false)
    expect(json).not.toMatch(
      /provider_account|auth_grant|quota_payload|access_token|wallet|preimage|lnbc|@/i,
    )
  })
})
