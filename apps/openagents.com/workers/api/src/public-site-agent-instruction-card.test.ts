import { describe, expect, test } from 'vitest'

import { publicSiteAgentInstructionCard } from './public-site-agent-instruction-card'

describe('public Site agent instruction cards', () => {
  test('builds copyable public-safe instructions for a deployed Site', () => {
    const card = publicSiteAgentInstructionCard({
      preset: 'proof_and_challenge',
      proofUrl: 'https://openagents.com/api/public/proof/otec',
      publicSourceRef: 'site_ref_otec_ben',
      siteSlug: 'ben-otec',
      siteTitle: 'Ben OTEC',
      siteUrl: 'https://sites.openagents.com/ben-otec',
    })
    const serialized = JSON.stringify(card)

    expect(card).toEqual(
      expect.objectContaining({
        title: 'Send your agent to this Site',
        preset: 'proof_and_challenge',
        siteSlug: 'ben-otec',
        siteTitle: 'Ben OTEC',
        siteUrl: 'https://sites.openagents.com/ben-otec',
        proofUrl: 'https://openagents.com/api/public/proof/otec',
        capabilityManifestUrl:
          'https://openagents.com/.well-known/openagents.json',
        openApiUrl: 'https://openagents.com/api/openapi.json',
        requiresOwnerClaimForMutation: true,
        referralCta: expect.objectContaining({
          referralJoinUrl:
            'https://openagents.com/r/site/site_ref_otec_ben?target=order',
          agentReferralJoinUrl:
            'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
        }),
      }),
    )
    expect(card?.allowedActions).toContain('inspect_public_proof')
    expect(card?.allowedActions).toContain('propose_site_improvement')
    expect(card?.copyableInstruction).toContain(
      'Read this instruction before taking action.',
    )
    expect(card?.copyableInstruction).toContain(
      'https://openagents.com/.well-known/openagents.json',
    )
    expect(card?.copyableInstruction).toContain(
      'https://openagents.com/api/openapi.json',
    )
    expect(card?.copyableInstruction).toContain(
      'https://openagents.com/api/public/proof/otec',
    )
    expect(card?.copyableInstruction).toContain(
      'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
    )
    expect(serialized).not.toContain('provider_account')
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('runner_payload')
    expect(serialized).not.toContain('source_archive')
  })

  test('degrades to public discovery when proof and deployment are absent', () => {
    const card = publicSiteAgentInstructionCard({
      preset: 'customer_site_safe',
      siteSlug: 'draft-site',
      siteTitle: 'Draft Site',
    })

    expect(card?.proofUrl).toBeNull()
    expect(card?.siteUrl).toBeNull()
    expect(card?.copyableInstruction).toContain(
      'No public proof URL is available yet',
    )
    expect(card?.copyableInstruction).toContain(
      'Do not claim a live deployment exists',
    )
    expect(card?.caveats).toContain(
      'No public proof URL is available yet; inspect public discovery documents only.',
    )
  })

  test('does not emit a card for disabled agent surfaces', () => {
    expect(
      publicSiteAgentInstructionCard({
        preset: 'none',
        siteSlug: 'private-site',
      }),
    ).toBeNull()
  })

  test('fails closed when card input contains secret-shaped material', () => {
    expect(
      publicSiteAgentInstructionCard({
        preset: 'openagents_network',
        siteTitle: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
      }),
    ).toBeNull()
  })
})
