import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisStandaloneClaimLedgerProjection,
  artanisStandaloneClaimLedgerProjectionHasPrivateMaterial,
  exampleArtanisStandaloneClaimLedger,
  projectArtanisStandaloneClaimLedger,
} from './artanis-standalone-claim-ledger'
import { PublicClaimProjectionUnsafe } from './public-claim-projections'

const nowIso = '2026-06-06T23:35:00.000Z'

describe('Artanis standalone claim ledger', () => {
  test('projects all standalone autonomy claim areas and public claim states', () => {
    const projection = projectArtanisStandaloneClaimLedger(
      exampleArtanisStandaloneClaimLedger(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisStandaloneClaimLedgerProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.agentRef).toBe('agent_artanis')
    expect(projection.ledgerRef).toBe(
      'ledger.public.artanis.standalone_autonomy',
    )
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.entries.map(entry => entry.area)).toEqual([
      'autonomous_loop',
      'operator_steering',
      'forum_communication',
      'pylon_campaign',
      'nexus_pylon_administration',
      'model_lab_stewardship',
      'work_routing',
      'spend_authority',
      'bitcoin_rewards',
      'accepted_work_payout',
      'settlement',
    ])
    expect(projection.stateCounts).toEqual([
      { count: 2, state: 'blocked' },
      { count: 2, state: 'measured' },
      { count: 1, state: 'modeled' },
      { count: 1, state: 'planned' },
      { count: 2, state: 'prohibited' },
      { count: 3, state: 'verified' },
    ])
  })

  test('keeps bitcoin reward and accepted-work payout claims honest', () => {
    const projection = projectArtanisStandaloneClaimLedger(
      exampleArtanisStandaloneClaimLedger(),
      'public',
      nowIso,
    )
    const rewards = projection.entries.find(
      entry => entry.area === 'bitcoin_rewards',
    )
    const acceptedWork = projection.entries.find(
      entry => entry.area === 'accepted_work_payout',
    )
    const settlement = projection.entries.find(
      entry => entry.area === 'settlement',
    )

    expect(rewards?.state.state).toBe('blocked')
    expect(rewards?.blockedByRefs).toEqual([
      'blocker.artanis.forum_reward_smoke_not_complete',
    ])
    expect(acceptedWork?.state.state).toBe('prohibited')
    expect(acceptedWork?.copyRule.allowedPublicVerb).toBe('not_public')
    expect(settlement?.state.state).toBe('prohibited')
    expect(settlement?.copyRule.allowedPublicVerb).toBe('not_public')
  })

  test('lowers standalone autonomy claims when required evidence is missing', () => {
    const input = exampleArtanisStandaloneClaimLedger()
    const projection = projectArtanisStandaloneClaimLedger({
      ...input,
      entries: input.entries.map(entry =>
        entry.area === 'operator_steering'
          ? {
              ...entry,
              desiredState: 'verified',
              evidenceRefs: [],
            }
          : entry
      ),
    }, 'public', nowIso)
    const operatorClaim = projection.entries.find(
      entry => entry.area === 'operator_steering',
    )

    expect(operatorClaim?.desiredState).toBe('verified')
    expect(operatorClaim?.state.state).toBe('planned')
    expect(operatorClaim?.state.caveats).toEqual(
      expect.arrayContaining([
        'Requested verified claim was lowered to planned because required evidence is missing.',
      ]),
    )
  })

  test('rejects missing required areas and non-Artanis identity', () => {
    const input = exampleArtanisStandaloneClaimLedger()

    expect(() =>
      projectArtanisStandaloneClaimLedger({
        ...input,
        entries: input.entries.filter(
          entry => entry.area !== 'forum_communication',
        ),
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)

    expect(() =>
      projectArtanisStandaloneClaimLedger({
        ...input,
        agentRef: 'agent_adjutant',
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
  })

  test('does not expose private material, raw timestamps, or unsafe Forum copy refs', () => {
    const projection = projectArtanisStandaloneClaimLedger(
      exampleArtanisStandaloneClaimLedger(),
      'public',
      nowIso,
    )
    const serialized = JSON.stringify(projection)
    const input = exampleArtanisStandaloneClaimLedger()

    expect(serialized).not.toContain('2026-06-06T23:30:00.000Z')
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('raw_runner')
    expect(artanisStandaloneClaimLedgerProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(() =>
      projectArtanisStandaloneClaimLedger({
        ...input,
        entries: input.entries.map(entry =>
          entry.area === 'forum_communication'
            ? {
                ...entry,
                forumCopyRefs: ['raw_runner_log.artanis_forum_copy'],
              }
            : entry
        ),
      }, 'public', nowIso),
    ).toThrow(PublicClaimProjectionUnsafe)
  })
})
