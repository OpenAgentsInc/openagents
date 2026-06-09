import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_FORUM_REWARD_SMOKE_RECORD_ONLY_AUTHORITY,
  ArtanisForumRewardSmokeProjection,
  ArtanisForumRewardSmokeRecord,
  ArtanisForumRewardSmokeUnsafe,
  artanisForumRewardSmokeProjectionHasPrivateMaterial,
  exampleArtanisForumRewardSmokeRecord,
  projectArtanisForumRewardSmoke,
} from './artanis-forum-reward-smoke'

const nowIso = '2026-06-07T02:10:00.000Z'

const smokeRecord = (
  overrides: Partial<ArtanisForumRewardSmokeRecord> = {},
): ArtanisForumRewardSmokeRecord =>
  S.decodeUnknownSync(ArtanisForumRewardSmokeRecord)({
    ...exampleArtanisForumRewardSmokeRecord(),
    ...overrides,
  })

describe('Artanis Forum reward smoke', () => {
  test('projects the two-agent reward smoke as simulation-only public evidence', () => {
    const projection = projectArtanisForumRewardSmoke(
      exampleArtanisForumRewardSmokeRecord(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisForumRewardSmokeProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkPayoutClaimAllowed: false,
      agentRef: 'agent_artanis',
      authority: ARTANIS_FORUM_REWARD_SMOKE_RECORD_ONLY_AUTHORITY,
      exchangeCount: 2,
      mode: 'simulation',
      modeLabel: 'Simulation only',
      providerSettlementClaimAllowed: false,
      updatedAtDisplay: '10 minutes ago',
      usedLiveBitcoin: false,
    })
    expect(projection.registeredAgentRefs).toEqual([
      'agent.public.alice',
      'agent.public.ben',
    ])
    expect(projection.exchangeRecords.map(exchange => [
      exchange.fromAgentRef,
      exchange.toAgentRef,
      exchange.receiptRef,
      exchange.amountValue,
      exchange.amountAsset,
    ])).toEqual([
      [
        'agent.public.alice',
        'agent.public.ben',
        'receipt.public.forum_reward_alice_to_ben',
        100,
        'sats',
      ],
      [
        'agent.public.ben',
        'agent.public.alice',
        'receipt.public.forum_reward_ben_to_alice',
        100,
        'sats',
      ],
    ])
    expect(projection.runReasonRefs).toEqual([
      'reason.public.deterministic_fake_bitcoin_simulation',
      'reason.public.no_concrete_spend_cap',
      'reason.public.no_owner_approved_named_wallet',
    ])
    expect(artanisForumRewardSmokeProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(JSON.stringify(projection)).not.toContain('lnbc')
    expect(JSON.stringify(projection)).not.toContain('preimage')
    expect(JSON.stringify(projection)).not.toContain('mnemonic')
    expect(JSON.stringify(projection)).not.toContain('private_key')
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('requires explicit owner-approved wallet authority, named wallet, and spend cap for live mode', () => {
    expect(() =>
      projectArtanisForumRewardSmoke(
        smokeRecord({
          mode: 'live_bitcoin',
          runReasonRefs: ['reason.public.owner_approved_live_bitcoin_smoke'],
          usedLiveBitcoin: true,
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisForumRewardSmokeUnsafe)

    const projection = projectArtanisForumRewardSmoke(
      smokeRecord({
        mode: 'live_bitcoin',
        namedWalletRefs: ['wallet.public.owner_approved_forum_reward_smoke'],
        runReasonRefs: ['reason.public.owner_approved_live_bitcoin_smoke'],
        spendCapRefs: ['spend_cap.public.forum_reward_smoke.200_sats'],
        usedLiveBitcoin: true,
        walletAuthorityRefs: [
          'authority.public.owner_approved_forum_reward_wallet',
        ],
      }),
      'operator',
      nowIso,
    )

    expect(projection).toMatchObject({
      mode: 'live_bitcoin',
      modeLabel: 'Live bitcoin recorded',
      usedLiveBitcoin: true,
      walletAuthorityRefs: [
        'authority.public.owner_approved_forum_reward_wallet',
      ],
    })
    expect(projection.authority)
      .toEqual(ARTANIS_FORUM_REWARD_SMOKE_RECORD_ONLY_AUTHORITY)
  })

  test('keeps ordinary Forum rewards separate from accepted-work payouts and provider settlement', () => {
    expect(() =>
      projectArtanisForumRewardSmoke(
        smokeRecord({
          acceptedWorkPayoutRefs: ['payout_row.public.accepted_work.forum'],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisForumRewardSmokeUnsafe)
    expect(() =>
      projectArtanisForumRewardSmoke(
        smokeRecord({
          providerSettlementRefs: ['settlement.public.provider.forum'],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ArtanisForumRewardSmokeUnsafe)
  })

  test('rejects raw payment, wallet, customer, provider, private repo, and timestamp material', () => {
    for (const unsafe of [
      smokeRecord({ receiptProjectionRefs: ['invoice.lnbc123'] }),
      smokeRecord({ caveatRefs: ['wallet.secret.seed'] }),
      smokeRecord({ runReasonRefs: ['customer_email_ben@example.com'] }),
      smokeRecord({ sourceRefs: ['provider_token.local'] }),
      smokeRecord({ sourceRefs: ['https://github.com/org/private-repo'] }),
      smokeRecord({ sourceRefs: ['source.public.2026-06-07T02:00:00.000Z'] }),
    ]) {
      expect(() =>
        projectArtanisForumRewardSmoke(unsafe, 'operator', nowIso),
      ).toThrow(ArtanisForumRewardSmokeUnsafe)
    }
  })
})
