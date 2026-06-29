import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotRepoPlacementProjection,
  CodingAutopilotRepoPlacementRecord,
  CodingAutopilotRepoPlacementRequest,
  CodingAutopilotRepoPlacementUnsafe,
  codingAutopilotRepoPlacementProjectionHasPrivateMaterial,
  evaluateCodingAutopilotRepoPlacement,
  projectCodingAutopilotRepoPlacement,
} from './coding-autopilot-repo-placement'

const baseRequest = (
  input: Partial<CodingAutopilotRepoPlacementRequest> = {},
): CodingAutopilotRepoPlacementRequest => ({
  customerGrantRefs: [],
  dataClassification: 'public',
  evaluatedAtIso: '2026-06-06T21:00:00.000Z',
  evidenceRefs: ['evidence.repo_placement'],
  id: 'repo_placement.public_site',
  missionRef: 'mission.otec_revision_4',
  operatorApprovalRefs: [],
  providerGrantRefs: [],
  publicProofProjectionRefs: ['public_proof.repo_placement.public_site'],
  repoRef: 'repo.OpenAgentsInc.otec_public',
  runnerBackendKind: 'cloudflare_container',
  runnerWorkloadTrust: 'low',
  trustTier: 'public',
  workroomRefs: ['workroom.otec_site_revision_4'],
  ...input,
})

describe('Coding on Autopilot repo placement', () => {
  test('allows public repos on low-trust backends and permits public proof claims', () => {
    const record = evaluateCodingAutopilotRepoPlacement(baseRequest())
    const publicProjection = projectCodingAutopilotRepoPlacement(record, 'public')

    expect(S.decodeUnknownSync(CodingAutopilotRepoPlacementRequest)(baseRequest()))
      .toEqual(baseRequest())
    expect(S.decodeUnknownSync(CodingAutopilotRepoPlacementRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(CodingAutopilotRepoPlacementProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(record).toMatchObject({
      decision: 'eligible',
      eligible: true,
      publicClaimAllowed: true,
      trustTier: 'public',
    })
    expect(publicProjection.repoRef).toBe('repo.OpenAgentsInc.otec_public')
    expect(publicProjection.workroomRefs).toEqual([])
    expect(codingAutopilotRepoPlacementProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('requires customer grants for private and sensitive repos without public claims', () => {
    const record = evaluateCodingAutopilotRepoPlacement(baseRequest({
      dataClassification: 'private',
      repoRef: 'repo.customer_private_app',
      trustTier: 'private',
    }))
    const customerProjection = projectCodingAutopilotRepoPlacement(
      record,
      'customer',
    )

    expect(record).toMatchObject({
      decision: 'needs_customer_grant',
      eligible: false,
      publicClaimAllowed: false,
    })
    expect(record.blockerRefs).toEqual([
      'blocker.repo_placement.customer_grant_required',
    ])
    expect(customerProjection.repoRef).toBe('repo.redacted')
    expect(customerProjection.customerSafeBlockedReasonRefs).toEqual([
      'reason.repo_placement.customer_connection_needed',
    ])
  })

  test('blocks legal/payment/regulated trust tiers from non-SHC backends and gates payment-sensitive placement', () => {
    const legalRecord = evaluateCodingAutopilotRepoPlacement(baseRequest({
      customerGrantRefs: ['customer_grant.repo_access'],
      dataClassification: 'legal_sensitive',
      operatorApprovalRefs: ['operator_approval.legal_review'],
      runnerBackendKind: 'cloudflare_container',
      trustTier: 'legal_sensitive',
    }))
    const paymentNeedsProvider = evaluateCodingAutopilotRepoPlacement(baseRequest({
      customerGrantRefs: ['customer_grant.repo_access'],
      dataClassification: 'payment_private',
      operatorApprovalRefs: ['operator_approval.payment_review'],
      runnerBackendKind: 'shc_vm',
      runnerWorkloadTrust: 'sensitive',
      trustTier: 'payment_sensitive',
    }))
    const paymentEligible = evaluateCodingAutopilotRepoPlacement(baseRequest({
      customerGrantRefs: ['customer_grant.repo_access'],
      dataClassification: 'payment_private',
      operatorApprovalRefs: ['operator_approval.payment_review'],
      providerGrantRefs: ['provider_grant.runner_account'],
      runnerBackendKind: 'shc_vm',
      runnerWorkloadTrust: 'sensitive',
      trustTier: 'payment_sensitive',
    }))

    expect(legalRecord).toMatchObject({
      decision: 'blocked',
      eligible: false,
      publicClaimAllowed: false,
    })
    expect(legalRecord.blockerRefs).toEqual([
      'blocker.repo_placement.backend_not_allowed_for_trust_tier',
    ])
    expect(paymentNeedsProvider.decision).toBe('needs_provider_grant')
    expect(paymentEligible).toMatchObject({
      decision: 'eligible',
      eligible: true,
      publicClaimAllowed: false,
    })
  })

  test('blocks unknown and secret-bearing contexts', () => {
    expect(evaluateCodingAutopilotRepoPlacement(baseRequest({
      trustTier: 'unknown',
    })).decision).toBe('blocked')
    expect(evaluateCodingAutopilotRepoPlacement(baseRequest({
      dataClassification: 'secret_bearing',
    })).decision).toBe('blocked')
  })

  test('rejects unsafe repo, grant, provider, runner, and customer refs', () => {
    expect(() =>
      evaluateCodingAutopilotRepoPlacement(baseRequest({
        repoRef: 'private_repo:https://github.com/customer/private-repo',
      })),
    ).toThrow(CodingAutopilotRepoPlacementUnsafe)
    expect(() =>
      evaluateCodingAutopilotRepoPlacement(baseRequest({
        providerGrantRefs: ['provider_token:abc'],
      })),
    ).toThrow(CodingAutopilotRepoPlacementUnsafe)
    expect(() =>
      evaluateCodingAutopilotRepoPlacement(baseRequest({
        evidenceRefs: ['raw_runner_payload:mission'],
      })),
    ).toThrow(CodingAutopilotRepoPlacementUnsafe)
    expect(() =>
      evaluateCodingAutopilotRepoPlacement(baseRequest({
        customerGrantRefs: ['ben@example.com'],
      })),
    ).toThrow(CodingAutopilotRepoPlacementUnsafe)
  })
})
