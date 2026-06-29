import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { PublicClaimProjectionUnsafe } from './public-claim-projections'
import {
  R10PylonCampaignProjection,
  projectR10PylonCampaign,
  r10PylonCampaignInput,
  r10PylonCampaignProjectionHasPrivateMaterial,
} from './r10-pylon-campaign'

const nowIso = '2026-06-06T21:10:00.000Z'

describe('R10 Pylon campaign projection', () => {
  test('projects measured, verified, planned, modeled, blocked, and prohibited campaign claims', () => {
    const projection = projectR10PylonCampaign(
      r10PylonCampaignInput(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(R10PylonCampaignProjection)(projection))
      .toEqual(projection)
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.stateCounts).toEqual([
      { count: 1, state: 'blocked' },
      { count: 1, state: 'measured' },
      { count: 1, state: 'modeled' },
      { count: 2, state: 'planned' },
      { count: 1, state: 'prohibited' },
      { count: 1, state: 'verified' },
    ])
    expect(projection.entries.map(entry => [entry.area, entry.state.state]))
      .toEqual([
        ['artanis_public_surface', 'measured'],
        ['provider_registration', 'verified'],
        ['pylon_release', 'planned'],
        ['work_routing', 'planned'],
        ['accepted_work_accounting', 'modeled'],
        ['live_spend_authority', 'blocked'],
        ['bitcoin_settlement_claims', 'prohibited'],
      ])
  })

  test('keeps live wallet tipping and provider payout settlement honest', () => {
    const projection = projectR10PylonCampaign(
      r10PylonCampaignInput(),
      'public',
      nowIso,
    )
    const liveWallet = projection.entries.find(
      entry => entry.area === 'live_spend_authority',
    )
    const payoutSettlement = projection.entries.find(
      entry => entry.area === 'bitcoin_settlement_claims',
    )

    expect(liveWallet?.state.state).toBe('blocked')
    expect(liveWallet?.blockedByRefs).toEqual([
      'blocker.no_approved_live_spend_cap',
    ])
    expect(liveWallet?.participantActionRefs).toEqual([
      'participant_action.use_fake_bitcoin_simulation_until_approved',
    ])
    expect(payoutSettlement?.state.state).toBe('prohibited')
    expect(payoutSettlement?.copyRule.allowedPublicVerb).toBe('not_public')
  })

  test('does not expose raw timestamps or private material', () => {
    const projection = projectR10PylonCampaign(
      r10PylonCampaignInput(),
      'public',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:05:00.000Z')
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('raw_runner')
    expect(r10PylonCampaignProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('lowers settlement claims without settlement evidence', () => {
    const [firstEntry] = r10PylonCampaignInput().entries
    const projection = projectR10PylonCampaign({
      ...r10PylonCampaignInput(),
      entries: [
        {
          ...firstEntry!,
          area: 'bitcoin_settlement_claims',
          claimId: 'claim_r10_settlement_without_settlement_evidence',
          claimKind: 'provider_settlement',
          claimRef: 'claim.r10_pylon.settlement_without_settlement_evidence',
          desiredState: 'settled',
          evidenceRefs: ['receipt.accepted_work.non_settlement'],
          titleRef: 'title.r10.settlement_without_settlement_evidence',
        },
      ],
    }, 'public', nowIso)

    expect(projection.entries[0]?.desiredState).toBe('settled')
    expect(projection.entries[0]?.state.state).toBe('verified')
    expect(projection.entries[0]?.copyRule.allowedPublicVerb).toBe('verified')
  })

  test('rejects wallet, provider, runner, payment, customer, and raw timestamp material', () => {
    const [firstEntry] = r10PylonCampaignInput().entries

    expect(() =>
      projectR10PylonCampaign({
        ...r10PylonCampaignInput(),
        entries: [
          {
            ...firstEntry!,
            evidenceRefs: ['wallet_secret.local_test_key'],
          },
        ],
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectR10PylonCampaign({
        ...r10PylonCampaignInput(),
        entries: [
          {
            ...firstEntry!,
            sourceRefs: ['provider_account.codex_1'],
          },
        ],
      }, 'operator', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectR10PylonCampaign({
        ...r10PylonCampaignInput(),
        entries: [
          {
            ...firstEntry!,
            nextActionRefs: ['raw_runner_log.r10'],
          },
        ],
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectR10PylonCampaign({
        ...r10PylonCampaignInput(),
        entries: [
          {
            ...firstEntry!,
            participantActionRefs: ['ben@example.com'],
          },
        ],
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectR10PylonCampaign({
        ...r10PylonCampaignInput(),
        entries: [
          {
            ...firstEntry!,
            blockedByRefs: ['blocker.2026-06-06T21:05:00.000Z'],
          },
        ],
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
  })
})
