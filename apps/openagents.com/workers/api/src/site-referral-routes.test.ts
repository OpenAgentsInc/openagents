import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeSiteReferralRoutes } from './site-referral-routes'

type SourceRow = Readonly<{
  archived_at: string | null
  campaign_ref: string | null
  created_at: string
  id: string
  policy_state: string
  public_slug: string
  public_source_ref: string
  referrer_user_id: string
  site_id: string
  site_version_id: string | null
  source_label: string | null
  updated_at: string
}>

type InviteRow = Readonly<{
  archived_at: string | null
  audience_path: string
  created_at: string
  expires_at: string | null
  id: string
  policy_state: string
  public_invite_ref: string
  referral_source_id: string
  scope: string
  token_hash: string
  updated_at: string
}>

type AttributionRow = Readonly<{
  archived_at: string | null
  capture_path: string
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  id: string
  policy_state: string
  public_invite_ref: string | null
  public_source_ref: string
  referral_invite_id: string | null
  referral_source_id: string
  target: string
  updated_at: string
}>

type Store = Readonly<{
  attributions: Array<AttributionRow>
  invites: Array<InviteRow>
  sources: Array<SourceRow>
}>

class MemoryStatement {
  private readonly values: ReadonlyArray<unknown>

  constructor(
    private readonly store: Store,
    private readonly query: string,
    values: ReadonlyArray<unknown> = [],
  ) {
    this.values = values
  }

  bind(...values: ReadonlyArray<unknown>): MemoryStatement {
    return new MemoryStatement(this.store, this.query, values)
  }

  first<T>(): Promise<T | null> {
    if (this.query.includes('FROM referral_attributions')) {
      const [id, nowIso] = this.values
      return Promise.resolve(
        (this.store.attributions.find(
          row =>
            row.id === id &&
            row.policy_state === 'pending' &&
            row.archived_at === null &&
            row.expires_at > String(nowIso),
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM site_referral_sources')) {
      const [value] = this.values

      return Promise.resolve(
        (this.store.sources.find(
          row => row.public_source_ref === value || row.id === value,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM referral_invites')) {
      const [publicInviteRef] = this.values

      return Promise.resolve(
        (this.store.invites.find(
          row => row.public_invite_ref === publicInviteRef,
        ) as T | undefined) ?? null,
      )
    }

    return Promise.resolve(null)
  }

  run(): Promise<D1Result> {
    if (this.query.includes('INSERT INTO referral_attributions')) {
      const [
        id,
        referralSourceId,
        referralInviteId,
        publicSourceRef,
        publicInviteRef,
        capturePath,
        target,
        policyState,
        firstVerifiedAt,
        claimedUserId,
        expiresAt,
        createdAt,
        updatedAt,
        archivedAt,
      ] = this.values

      this.store.attributions.push({
        archived_at: archivedAt as string | null,
        capture_path: String(capturePath),
        claimed_user_id: claimedUserId as string | null,
        created_at: String(createdAt),
        expires_at: String(expiresAt),
        first_verified_at: firstVerifiedAt as string | null,
        id: String(id),
        policy_state: String(policyState),
        public_invite_ref: publicInviteRef as string | null,
        public_source_ref: String(publicSourceRef),
        referral_invite_id: referralInviteId as string | null,
        referral_source_id: String(referralSourceId),
        target: String(target),
        updated_at: String(updatedAt),
      })
    }

    return Promise.resolve({ success: true } as D1Result)
  }
}

const db = (store: Store): D1Database =>
  ({
    prepare: (query: string) => new MemoryStatement(store, query),
  }) as unknown as D1Database

const now = '2026-06-05T20:00:00.000Z'

// The route under test compares `expires_at` against the REAL wall-clock
// time (`currentIsoTimestamp()` in site-referral-routes.ts), not the fixed
// `now` fixture above. A hardcoded future literal here silently rots into a
// "genuinely expired" fixture once the calendar catches up (root cause of a
// prior 302→410 regression), so compute a far-future timestamp at test-run
// time instead of hardcoding one.
const farFutureIso = new Date(
  Date.now() + 1000 * 60 * 60 * 24 * 365 * 5,
).toISOString()

const source = (overrides: Partial<SourceRow> = {}): SourceRow => ({
  archived_at: null,
  campaign_ref: 'first-sites',
  created_at: now,
  id: 'site_referral_source_otec',
  policy_state: 'active',
  public_slug: 'otec',
  public_source_ref: 'site_ref_otec_ben',
  referrer_user_id: 'github:14167547',
  site_id: 'site_project_otec',
  site_version_id: 'site_version_2',
  source_label: 'OTEC public Site',
  updated_at: now,
  ...overrides,
})

const invite = (overrides: Partial<InviteRow> = {}): InviteRow => ({
  archived_at: null,
  audience_path: 'agent',
  created_at: now,
  expires_at: farFutureIso,
  id: 'referral_invite_otec_agent',
  policy_state: 'active',
  public_invite_ref: 'invite_otec_agent',
  referral_source_id: 'site_referral_source_otec',
  scope: 'agent_claim',
  token_hash: 'sha256:server-side-hash-only',
  updated_at: now,
  ...overrides,
})

const route = async (
  store: Store,
  url: string,
  cookie?: string,
): Promise<Response> => {
  const request = new Request(
    url,
    cookie === undefined ? undefined : { headers: { cookie } },
  )
  const routed = makeSiteReferralRoutes().routeSiteReferralRequest(request, {
    OPENAGENTS_DB: db(store),
  })

  if (routed === undefined) {
    throw new Error('Expected Site referral route to match.')
  }

  return Effect.runPromise(routed)
}

describe('Site referral capture routes', () => {
  test('captures a source referral and redirects to a clean order URL', async () => {
    const store = {
      attributions: [],
      invites: [],
      sources: [source()],
    } satisfies Store

    const response = await route(
      store,
      'https://openagents.com/r/site/site_ref_otec_ben?target=order&ref=secret',
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/order')
    expect(response.headers.get('location')).not.toContain('ref=')
    expect(response.headers.get('set-cookie')).toContain(
      'oa_pending_referral_attribution=referral_attribution_',
    )
    expect(store.attributions).toHaveLength(1)
    expect(store.attributions[0]).toMatchObject({
      capture_path: 'human',
      policy_state: 'pending',
      public_invite_ref: null,
      public_source_ref: 'site_ref_otec_ben',
      target: 'order',
    })
  })

  test('captures an agent invite without exposing token hashes', async () => {
    const store = {
      attributions: [],
      invites: [invite()],
      sources: [source()],
    } satisfies Store

    const response = await route(
      store,
      'https://openagents.com/r/invite/invite_otec_agent',
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/AGENTS.md')
    expect(JSON.stringify(store.attributions)).not.toContain('token_hash')
    expect(JSON.stringify(store.attributions)).not.toContain(
      'server-side-hash-only',
    )
    expect(store.attributions[0]).toMatchObject({
      capture_path: 'agent',
      public_invite_ref: 'invite_otec_agent',
      target: 'agent_claim',
    })
  })

  test('replaces a valid pending cookie with the latest capture', async () => {
    const store = {
      attributions: [
        {
          archived_at: null,
          capture_path: 'human',
          claimed_user_id: null,
          created_at: now,
          expires_at: farFutureIso,
          first_verified_at: null,
          id: 'referral_attribution_existing',
          policy_state: 'pending',
          public_invite_ref: null,
          public_source_ref: 'site_ref_first',
          referral_invite_id: null,
          referral_source_id: 'site_referral_source_first',
          target: 'home',
          updated_at: now,
        },
      ],
      invites: [],
      sources: [source()],
    } satisfies Store

    const response = await route(
      store,
      'https://openagents.com/r/site/site_ref_otec_ben?target=order',
      'oa_pending_referral_attribution=referral_attribution_existing',
    )

    expect(response.status).toBe(302)
    expect(response.headers.get('location')).toBe('/order')
    expect(response.headers.get('set-cookie')).toContain(
      'oa_pending_referral_attribution=referral_attribution_',
    )
    expect(response.headers.get('set-cookie')).not.toContain(
      'referral_attribution_existing',
    )
    expect(store.attributions).toHaveLength(2)
    expect(store.attributions[0]).toMatchObject({
      id: 'referral_attribution_existing',
      policy_state: 'pending',
    })
    expect(store.attributions[1]).toMatchObject({
      public_source_ref: 'site_ref_otec_ben',
      target: 'order',
    })
  })

  test('fails safely for expired or disabled referrals', async () => {
    const expiredStore = {
      attributions: [],
      invites: [
        invite({
          expires_at: '2026-01-05T20:00:00.000Z',
        }),
      ],
      sources: [source()],
    } satisfies Store

    const expired = await route(
      expiredStore,
      'https://openagents.com/r/invite/invite_otec_agent',
    )

    expect(expired.status).toBe(410)
    await expect(expired.json()).resolves.toEqual({
      error: 'referral_invite_unavailable',
      reason: 'expired',
    })

    const disabledStore = {
      attributions: [],
      invites: [],
      sources: [source({ policy_state: 'disabled' })],
    } satisfies Store

    const disabled = await route(
      disabledStore,
      'https://openagents.com/r/site/site_ref_otec_ben',
    )

    expect(disabled.status).toBe(410)
    await expect(disabled.json()).resolves.toEqual({
      error: 'referral_source_unavailable',
      reason: 'disabled',
    })
  })
})
