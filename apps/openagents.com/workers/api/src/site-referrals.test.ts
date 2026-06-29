import { describe, expect, test } from 'vitest'

import {
  type ReferralInviteRecord,
  SiteReferralUnsafePayload,
  type SiteReferralSourceRecord,
  publicReferralInvite,
  publicSiteReferralSource,
} from './site-referrals'

const sourceRecord = {
  archivedAt: null,
  campaignRef: 'first-sites',
  createdAt: '2026-06-05T20:00:00.000Z',
  id: 'site_referral_source_otec',
  policyState: 'active',
  publicSlug: 'otec',
  publicSourceRef: 'site_ref_otec_ben',
  referrerUserId: 'github:14167547',
  siteId: 'site_project_otec',
  siteVersionId: 'site_version_2',
  sourceLabel: 'OTEC public Site',
  updatedAt: '2026-06-05T20:00:00.000Z',
} satisfies SiteReferralSourceRecord

const inviteRecord = {
  archivedAt: null,
  audiencePath: 'agent',
  createdAt: '2026-06-05T20:00:00.000Z',
  expiresAt: '2026-07-05T20:00:00.000Z',
  id: 'referral_invite_otec_agent',
  policyState: 'active',
  publicInviteRef: 'invite_otec_agent',
  referralSourceId: 'site_referral_source_otec',
  scope: 'agent_claim',
  tokenHash: 'sha256:server-side-hash-only',
  updatedAt: '2026-06-05T20:00:00.000Z',
} satisfies ReferralInviteRecord

describe('Site referral source and invite projections', () => {
  test('projects a public-safe Site referral source', () => {
    expect(publicSiteReferralSource(sourceRecord)).toEqual({
      campaignRef: 'first-sites',
      id: 'site_referral_source_otec',
      policyState: 'active',
      publicSlug: 'otec',
      publicSourceRef: 'site_ref_otec_ben',
      siteId: 'site_project_otec',
      siteVersionId: 'site_version_2',
      sourceLabel: 'OTEC public Site',
    })
  })

  test('projects public invite metadata without the token hash', () => {
    const projection = publicReferralInvite(inviteRecord)

    expect(projection).toEqual({
      audiencePath: 'agent',
      expiresAt: '2026-07-05T20:00:00.000Z',
      id: 'referral_invite_otec_agent',
      policyState: 'active',
      publicInviteRef: 'invite_otec_agent',
      referralSourceId: 'site_referral_source_otec',
      scope: 'agent_claim',
    })
    expect(JSON.stringify(projection)).not.toContain('tokenHash')
    expect(JSON.stringify(projection)).not.toContain('server-side-hash-only')
  })

  test('rejects secret-shaped public source refs', () => {
    expect(() =>
      publicSiteReferralSource({
        ...sourceRecord,
        publicSourceRef: 'xprv_referral_secret',
      }),
    ).toThrow(SiteReferralUnsafePayload)
  })

  test('rejects secret-shaped source labels before generated Sites receive them', () => {
    expect(() =>
      publicSiteReferralSource({
        ...sourceRecord,
        sourceLabel: 'mnemonic seed phrase goes here',
      }),
    ).toThrow(SiteReferralUnsafePayload)
  })

  test('rejects secret-shaped public invite refs', () => {
    expect(() =>
      publicReferralInvite({
        ...inviteRecord,
        publicInviteRef: 'webhook_secret_referral',
      }),
    ).toThrow(SiteReferralUnsafePayload)
  })
})
