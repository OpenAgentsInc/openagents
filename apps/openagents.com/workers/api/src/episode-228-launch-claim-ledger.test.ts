import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { PublicClaimProjectionUnsafe } from './public-claim-projections'
import {
  OpenAgentsLaunchClaimLedgerInput,
  OpenAgentsLaunchClaimLedgerProjection,
  episode228LaunchClaimLedger,
  episode228LaunchClaimLedgerInput,
  projectOpenAgentsLaunchClaimLedger,
} from './episode-228-launch-claim-ledger'

describe('Episode 228 launch claim ledger', () => {
  test('projects Episode 228 launch claims with honest public states', () => {
    const ledger = episode228LaunchClaimLedger()

    expect(S.decodeUnknownSync(OpenAgentsLaunchClaimLedgerProjection)(ledger))
      .toEqual(ledger)
    expect(ledger).toMatchObject({
      audience: 'public',
      ledgerId: 'launch_claim_ledger_episode_228',
      launchRef: 'launch:episode_228_free_autopilot',
    })

    const statesById = new Map(
      ledger.entries.map(entry => [
        entry.claimId,
        entry.claimProjection.state.state,
      ]),
    )
    const copyById = new Map(
      ledger.entries.map(entry => [
        entry.claimId,
        entry.claimProjection.copyRule.allowedPublicVerb,
      ]),
    )

    expect(statesById.get('claim_episode_228_autopilot_beta_launch'))
      .toBe('verified')
    expect(statesById.get('claim_episode_228_limited_free_beta'))
      .toBe('verified')
    expect(statesById.get('claim_episode_228_public_traces_visible'))
      .toBe('measured')
    expect(statesById.get('claim_episode_228_private_repo_support'))
      .toBe('planned')
    expect(statesById.get('claim_episode_228_revenue_share_model'))
      .toBe('modeled')
    expect(statesById.get('claim_episode_228_accepted_work_payouts_settled'))
      .toBe('prohibited')
    expect(copyById.get('claim_episode_228_accepted_work_payouts_settled'))
      .toBe('not_public')
  })

  test('keeps public projections free of customer, team, and operator refs', () => {
    const ledger = episode228LaunchClaimLedger()

    expect(ledger.entries.every(entry =>
      entry.claimProjection.customerRefs.length === 0 &&
      entry.claimProjection.teamRefs.length === 0 &&
      entry.claimProjection.operatorRefs.length === 0
    )).toBe(true)
  })

  test('reuses the projection contract and lowers unsupported verified claims', () => {
    const ledger = projectOpenAgentsLaunchClaimLedger({
      entries: [
        {
          caveatRefs: ['caveat.launch.test_missing_evidence'],
          claimId: 'claim_episode_228_test_missing_evidence',
          claimKind: 'agent_challenge',
          claimRef: 'claim.episode_228.test_missing_evidence',
          desiredState: 'verified',
          evidenceRefs: [],
          sourceRefs: ['source.openagents.blog.free_autopilot'],
          subjectRef: 'autopilot:free_beta_launch',
          titleRef: 'title.episode_228.test_missing_evidence',
          updatedAt: '2026-06-06T21:00:00.000Z',
        },
      ],
      ledgerId: 'launch_claim_ledger_test',
      launchRef: 'launch:test',
      sourceRefs: ['source.openagents.blog.free_autopilot'],
      updatedAt: '2026-06-06T21:00:00.000Z',
    })

    expect(ledger.entries[0]?.claimProjection.state).toMatchObject({
      label: 'Planned',
      state: 'planned',
    })
    expect(ledger.entries[0]?.claimProjection.state.caveats).toContain(
      'Requested verified claim was lowered to planned because required evidence is missing.',
    )
  })

  test('rejects private, provider, runner, wallet, payment, and customer refs', () => {
    const base = episode228LaunchClaimLedgerInput()
    const decode = S.decodeUnknownSync(OpenAgentsLaunchClaimLedgerInput)

    expect(decode(base)).toEqual(base)
    expect(() =>
      projectOpenAgentsLaunchClaimLedger({
        ...base,
        sourceRefs: ['source.openagents.blog.free_autopilot', 'ben@example.com'],
      }),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectOpenAgentsLaunchClaimLedger({
        ...base,
        entries: [
          {
            ...base.entries[0]!,
            evidenceRefs: ['raw_runner_payload:episode_228'],
          },
        ],
      }),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectOpenAgentsLaunchClaimLedger({
        ...base,
        entries: [
          {
            ...base.entries[0]!,
            evidenceRefs: ['provider_token:episode_228'],
          },
        ],
      }),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectOpenAgentsLaunchClaimLedger({
        ...base,
        entries: [
          {
            ...base.entries[0]!,
            evidenceRefs: ['wallet_state:episode_228'],
          },
        ],
      }),
    ).toThrow(PublicClaimProjectionUnsafe)
    expect(() =>
      projectOpenAgentsLaunchClaimLedger({
        ...base,
        entries: [
          {
            ...base.entries[0]!,
            evidenceRefs: ['lnbc1rawinvoice'],
          },
        ],
      }),
    ).toThrow(PublicClaimProjectionUnsafe)
  })
})
