import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PublicClaimProjection,
  PublicClaimProjectionUnsafe,
  projectPublicClaimRecord,
  publicClaimProjectionHasPrivateMaterial,
  type PublicClaimProjectionRecord,
} from './public-claim-projections'

const baseRecord: PublicClaimProjectionRecord = {
  caveatRefs: ['caveat.public_claim.deployment_reviewed'],
  claimId: 'claim_otec_latest_deployment',
  claimKind: 'deployment',
  claimRef: 'claim.otec.latest_deployment',
  customerRefs: ['customer_ref.order_otec'],
  desiredState: 'verified',
  evidenceRefs: [
    'deployment:site_otec:v3',
    'https://sites.openagents.com/otec',
  ],
  operatorRefs: [
    'operator_ref.review_closeout_otec',
    'operator_ref.deployment_audit_otec',
  ],
  sourceRefs: ['source.transcript.230', 'source.site_order.otec'],
  subjectRef: 'site:otec',
  surface: 'site',
  teamRefs: ['team_ref.sites_review'],
  titleRef: 'title.otec.latest_deployment',
  updatedAt: '2026-06-06T16:00:00.000Z',
}

describe('public claim projections', () => {
  test('projects public, customer, team, and operator audiences with scoped refs', () => {
    const publicProjection = projectPublicClaimRecord(baseRecord, 'public')
    const customerProjection = projectPublicClaimRecord(baseRecord, 'customer')
    const teamProjection = projectPublicClaimRecord(baseRecord, 'team')
    const operatorProjection = projectPublicClaimRecord(baseRecord, 'operator')

    expect(S.decodeUnknownSync(PublicClaimProjection)(publicProjection))
      .toEqual(publicProjection)
    expect(publicProjection).toMatchObject({
      audience: 'public',
      claimId: baseRecord.claimId,
      copyRule: {
        allowedPublicVerb: 'verified',
        copyRuleRef: 'copy_rule.public_claim.verified',
      },
      state: {
        label: 'Verified',
        state: 'verified',
      },
    })
    expect(publicProjection.customerRefs).toEqual([])
    expect(publicProjection.teamRefs).toEqual([])
    expect(publicProjection.operatorRefs).toEqual([])
    expect(customerProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(customerProjection.teamRefs).toEqual([])
    expect(customerProjection.operatorRefs).toEqual([])
    expect(teamProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(teamProjection.teamRefs).toEqual(baseRecord.teamRefs)
    expect(teamProjection.operatorRefs).toEqual([])
    expect(operatorProjection.customerRefs).toEqual(baseRecord.customerRefs)
    expect(operatorProjection.teamRefs).toEqual(baseRecord.teamRefs)
    expect(operatorProjection.operatorRefs).toEqual(
      [...baseRecord.operatorRefs].sort(),
    )
    expect(publicClaimProjectionHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('uses claim-state lowering and copy rules when evidence is missing', () => {
    const projection = projectPublicClaimRecord({
      ...baseRecord,
      desiredState: 'verified',
      evidenceRefs: [],
    }, 'public')

    expect(projection.state).toMatchObject({
      label: 'Planned',
      state: 'planned',
    })
    expect(projection.copyRule).toMatchObject({
      allowedPublicVerb: 'planned',
      copyRuleRef: 'copy_rule.public_claim.planned',
    })
    expect(projection.state.caveats).toContain(
      'Requested verified claim was lowered to planned because required evidence is missing.',
    )
  })

  test('keeps blocked and prohibited states terminal across audiences', () => {
    expect(projectPublicClaimRecord({
      ...baseRecord,
      desiredState: 'blocked',
    }, 'public').state.state).toBe('blocked')
    expect(projectPublicClaimRecord({
      ...baseRecord,
      desiredState: 'prohibited',
    }, 'operator').copyRule).toMatchObject({
      allowedPublicVerb: 'not_public',
      copyRuleRef: 'copy_rule.public_claim.prohibited',
    })
  })

  test('rejects private workroom, provider, wallet, and raw payment refs', () => {
    expect(() =>
      projectPublicClaimRecord({
        ...baseRecord,
        sourceRefs: ['raw_runner_payload:abc'],
      }, 'public'),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectPublicClaimRecord({
        ...baseRecord,
        operatorRefs: ['provider_token:abc'],
      }, 'operator'),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectPublicClaimRecord({
        ...baseRecord,
        evidenceRefs: ['lnbc1rawinvoice'],
      }, 'public'),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectPublicClaimRecord({
        ...baseRecord,
        customerRefs: ['ben@example.com'],
      }, 'customer'),
    ).toThrow(PublicClaimProjectionUnsafe)
  })
})
