import { describe, expect, test } from 'vitest'

import { publicSiteReferralCta } from './public-site-referral-cta'

describe('public Site referral CTA projection', () => {
  test('builds public-safe human and agent referral join links', () => {
    const cta = publicSiteReferralCta({
      publicSourceRef: 'site_ref_otec_ben',
      siteSlug: 'ben-otec',
      siteTitle: 'Ben OTEC',
    })
    const serialized = JSON.stringify(cta)

    expect(cta).toEqual(
      expect.objectContaining({
        title: 'Get your own OpenAgents Site',
        label: 'Start your Site request',
        siteSlug: 'ben-otec',
        siteTitle: 'Ben OTEC',
        openAgentsJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        referralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=order',
        agentReferralJoinUrl:
          'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
      }),
    )
    expect(cta?.copyableAgentInstruction).toContain(
      'https://openagents.com/r/site/site_ref_otec_ben?target=agent&path=agent',
    )
    expect(cta?.copyableAgentInstruction).toContain(
      'not a payout promise or an authorization grant',
    )
    expect(serialized).not.toContain('token_hash')
    expect(serialized).not.toContain('private_key')
    expect(serialized).not.toContain('webhook_secret')
    expect(serialized).not.toContain('provider_account')
  })

  test('fails closed for secret-shaped source refs or titles', () => {
    expect(
      publicSiteReferralCta({
        publicSourceRef: 'webhook_secret_ref',
        siteSlug: 'ben-otec',
      }),
    ).toBeNull()

    expect(
      publicSiteReferralCta({
        publicSourceRef: 'site_ref_otec_ben',
        siteTitle: 'Bearer gho_abcdefghijklmnopqrstuvwxyz',
      }),
    ).toBeNull()
  })
})
