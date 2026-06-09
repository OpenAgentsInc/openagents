import { describe, expect, test } from 'vitest'

import {
  PublicClaimCopyRule,
  assertPublicClaimCopySafe,
  publicClaimCopyRuleForState,
  publicClaimStateProjection,
} from './public-claim-state'

describe('public claim state', () => {
  test('keeps planned claims without evidence honest', () => {
    expect(
      publicClaimStateProjection({
        desiredState: 'planned',
        kind: 'deployment',
      }),
    ).toEqual({
      caveats: ['This claim is planned and should not be read as completed.'],
      description: 'Intended work or capability that is not yet evidenced.',
      evidenceRefs: [],
      label: 'Planned',
      state: 'planned',
    })
  })

  test('lowers verified claims when required evidence is missing', () => {
    const projection = publicClaimStateProjection({
      desiredState: 'verified',
      kind: 'site_url',
    })

    expect(projection.state).toBe('planned')
    expect(projection.caveats).toContain(
      'Requested verified claim was lowered to planned because required evidence is missing.',
    )
  })

  test('allows verified claims with evidence refs', () => {
    const projection = publicClaimStateProjection({
      desiredState: 'verified',
      evidenceRefs: ['site:otec', 'https://sites.openagents.com/otec'],
      kind: 'site_url',
    })

    expect(projection).toMatchObject({
      evidenceRefs: ['https://sites.openagents.com/otec', 'site:otec'],
      label: 'Verified',
      state: 'verified',
    })
  })

  test('requires settlement evidence for settled claims', () => {
    expect(
      publicClaimStateProjection({
        desiredState: 'settled',
        evidenceRefs: ['receipt:usage:1'],
        kind: 'provider_settlement',
      }).state,
    ).toBe('verified')
    expect(
      publicClaimStateProjection({
        desiredState: 'settled',
        evidenceRefs: ['settlement:pylon:1'],
        kind: 'provider_settlement',
      }).state,
    ).toBe('settled')
  })

  test('supports blocked and prohibited terminal public claim states', () => {
    expect(
      publicClaimStateProjection({
        desiredState: 'blocked',
        evidenceRefs: ['receipt:deployment:1'],
        kind: 'deployment',
      }),
    ).toMatchObject({
      description: 'Waiting on missing evidence, approval, or reachable authority.',
      label: 'Blocked',
      state: 'blocked',
    })
    expect(
      publicClaimStateProjection({
        desiredState: 'prohibited',
        evidenceRefs: ['receipt:deployment:1'],
        kind: 'deployment',
      }),
    ).toMatchObject({
      description: 'This claim must not be made on public surfaces.',
      label: 'Prohibited',
      state: 'prohibited',
    })
  })

  test('exposes copy rules for each claim state', () => {
    expect(PublicClaimCopyRule.pipe).toBeDefined()
    expect(publicClaimCopyRuleForState('planned')).toMatchObject({
      allowedPublicVerb: 'planned',
      copyRuleRef: 'copy_rule.public_claim.planned',
      evidenceRequired: false,
      settlementEvidenceRequired: false,
    })
    expect(publicClaimCopyRuleForState('settled')).toMatchObject({
      allowedPublicVerb: 'settled',
      evidenceRequired: true,
      settlementEvidenceRequired: true,
    })
    expect(publicClaimCopyRuleForState('prohibited')).toMatchObject({
      allowedPublicVerb: 'not_public',
      copyRuleRef: 'copy_rule.public_claim.prohibited',
    })
  })

  test('rejects copy that overstates public proof or contains secrets', () => {
    expect(() =>
      assertPublicClaimCopySafe('Provider settlement is live for all agents.'),
    ).toThrow()
    expect(() =>
      assertPublicClaimCopySafe('Provider settlement is live for all agents.'),
    ).toThrow(expect.objectContaining({
      reason: 'Public claim copy overstates evidence or settlement state.',
    }))
    expect(() =>
      assertPublicClaimCopySafe('Use OPENCODE_AUTH_CONTENT to verify.'),
    ).toThrow(expect.objectContaining({
      reason: 'Public claim copy overstates evidence or settlement state.',
    }))
  })

  test('rejects evidence refs with private, provider, wallet, or payment material', () => {
    expect(() =>
      publicClaimStateProjection({
        desiredState: 'verified',
        evidenceRefs: ['ben@example.com'],
        kind: 'site_url',
      }),
    ).toThrow(expect.objectContaining({
      reason:
        'Public claim evidence ref contains private, secret, payment, wallet, provider, or customer material.',
    }))
    expect(() =>
      publicClaimStateProjection({
        desiredState: 'verified',
        evidenceRefs: ['raw_runner_payload:abc'],
        kind: 'site_url',
      }),
    ).toThrow(expect.objectContaining({
      reason:
        'Public claim evidence ref contains private, secret, payment, wallet, provider, or customer material.',
    }))
    expect(() =>
      publicClaimStateProjection({
        desiredState: 'verified',
        evidenceRefs: ['lnbc1rawinvoice'],
        kind: 'site_url',
      }),
    ).toThrow(expect.objectContaining({
      reason:
        'Public claim evidence ref contains private, secret, payment, wallet, provider, or customer material.',
    }))
  })
})
