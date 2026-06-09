import { describe, expect, test } from 'vitest'

import { publicSiteAgentChallenges } from './public-site-agent-challenges'

describe('public Site agent challenges', () => {
  test('publishes an open public-safe proof improvement challenge', () => {
    const challenges = publicSiteAgentChallenges({
      proofUrl: 'https://openagents.com/api/public/proof/otec',
      siteSlug: 'ben-otec',
      siteTitle: 'Ben OTEC',
    })
    const [challenge] = challenges
    const serialized = JSON.stringify(challenge)

    expect(challenges).toHaveLength(1)
    expect(challenge).toEqual(
      expect.objectContaining({
        id: 'ben-otec-proof-copy-source-challenge',
        title: 'Improve public proof for Ben OTEC',
        status: 'open',
        challengeUrl:
          'https://openagents.com/api/public/proof/otec#agent-challenges',
        proofUrl: 'https://openagents.com/api/public/proof/otec',
        capabilityManifestUrl:
          'https://openagents.com/.well-known/openagents.json',
        openApiUrl: 'https://openagents.com/api/openapi.json',
        ownerClaimUrl: 'https://openagents.com/onboarding',
        fundingStatus: 'planned_not_live',
        acceptedOutcomeClaim: null,
      }),
    )
    expect(challenge?.contributionTypes).toEqual([
      'proof_inspection',
      'research_source',
      'copy_improvement',
    ])
    expect(challenge?.requiredEvidence).toContain(
      'Public URL and source title for any proposed source.',
    )
    expect(challenge?.fundingNote).toContain('not live')
    expect(challenge?.claimState.state).toBe('measured')
    expect(challenge?.claimState.caveats).toContain(
      'No accepted outcome, payment, reward, or settlement is claimed until a receipt exists.',
    )
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('runner_payload')
    expect(serialized).not.toContain('source_archive')
    expect(serialized).not.toContain('gho_')
  })

  test('fails closed when challenge inputs contain secret-shaped material', () => {
    expect(
      publicSiteAgentChallenges({
        proofUrl: 'https://openagents.com/api/public/proof/otec',
        siteTitle: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
      }),
    ).toEqual([])
  })
})
