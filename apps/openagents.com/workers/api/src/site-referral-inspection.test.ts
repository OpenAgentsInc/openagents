import { Effect } from 'effect'
import { describe, expect, test, vi } from 'vitest'

import {
  readOperatorSiteReferralInspection,
  readSiteReferralOwnerOverview,
} from './site-referral-inspection'
import { makeSiteReferralInspectionRoutes } from './site-referral-inspection-routes'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type SourceMetricsRow = Readonly<{
  agent_claim_count: number
  capped_policy_count: number
  campaign_ref: string | null
  capture_count: number
  claimed_capture_count: number
  disputed_capture_count: number
  disputed_policy_count: number
  expired_capture_count: number
  held_policy_count: number
  latest_capture_at: string | null
  latest_verified_at: string | null
  linked_order_count: number
  operator_override_count: number
  paid_workflow_count: number
  pending_capture_count: number
  policy_state: string
  public_slug: string
  public_source_ref: string
  referral_source_id: string
  referrer_user_id: string
  reversed_policy_count: number
  site_id: string
  site_owner_user_id: string
  site_slug: string
  site_title: string
  source_label: string | null
  verified_user_count: number
}>

type AttributionInspectionRow = Readonly<{
  capture_path: string
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  linked_order_count: number
  policy_state: string
  public_invite_ref: string | null
  public_source_ref: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  site_id: string
  site_slug: string
  site_title: string
  target: string
  updated_at: string
}>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

class SiteReferralInspectionStore {
  queries: Array<Readonly<{ query: string; values: ReadonlyArray<unknown> }>> =
    []
  sourceRows: Array<SourceMetricsRow> = [
    {
      agent_claim_count: 1,
      capped_policy_count: 1,
      campaign_ref: 'otc',
      capture_count: 3,
      claimed_capture_count: 2,
      disputed_capture_count: 0,
      disputed_policy_count: 1,
      expired_capture_count: 0,
      held_policy_count: 1,
      latest_capture_at: '2026-06-05T12:00:00.000Z',
      latest_verified_at: '2026-06-05T12:05:00.000Z',
      linked_order_count: 1,
      operator_override_count: 1,
      paid_workflow_count: 1,
      pending_capture_count: 1,
      policy_state: 'active',
      public_slug: 'otec',
      public_source_ref: 'src_otec',
      referral_source_id: 'site_referral_source_otec',
      referrer_user_id: 'github:owner',
      reversed_policy_count: 1,
      site_id: 'site_project_otec',
      site_owner_user_id: 'github:owner',
      site_slug: 'otec',
      site_title: 'OTEC Floating Datacenter',
      source_label: 'Bearer gho_should_not_render',
      verified_user_count: 2,
    },
    {
      agent_claim_count: 0,
      capped_policy_count: 0,
      campaign_ref: null,
      capture_count: 1,
      claimed_capture_count: 1,
      disputed_capture_count: 0,
      disputed_policy_count: 0,
      expired_capture_count: 0,
      held_policy_count: 0,
      latest_capture_at: '2026-06-05T13:00:00.000Z',
      latest_verified_at: '2026-06-05T13:05:00.000Z',
      linked_order_count: 1,
      operator_override_count: 0,
      paid_workflow_count: 0,
      pending_capture_count: 0,
      policy_state: 'active',
      public_slug: 'other',
      public_source_ref: 'src_other',
      referral_source_id: 'site_referral_source_other',
      referrer_user_id: 'github:other',
      reversed_policy_count: 0,
      site_id: 'site_project_other',
      site_owner_user_id: 'github:other',
      site_slug: 'other',
      site_title: 'Other Site',
      source_label: 'Other Site',
      verified_user_count: 1,
    },
  ]
  attributionRows: Array<AttributionInspectionRow> = [
    {
      capture_path: 'human',
      claimed_user_id: 'github:referred',
      created_at: '2026-06-05T12:00:00.000Z',
      expires_at: '2026-07-05T12:00:00.000Z',
      first_verified_at: '2026-06-05T12:05:00.000Z',
      linked_order_count: 1,
      policy_state: 'claimed',
      public_invite_ref: null,
      public_source_ref: 'src_otec',
      referral_attribution_id: 'referral_attribution_otec',
      referral_invite_id: null,
      referral_source_id: 'site_referral_source_otec',
      site_id: 'site_project_otec',
      site_slug: 'otec',
      site_title: 'OTEC Floating Datacenter',
      target: 'order',
      updated_at: '2026-06-05T12:05:00.000Z',
    },
    {
      capture_path: 'human',
      claimed_user_id: null,
      created_at: '2026-06-05T12:10:00.000Z',
      expires_at: '2026-07-05T12:10:00.000Z',
      first_verified_at: null,
      linked_order_count: 0,
      policy_state: 'pending',
      public_invite_ref: null,
      public_source_ref: 'src_otec',
      referral_attribution_id: 'referral_attribution_pending',
      referral_invite_id: null,
      referral_source_id: 'site_referral_source_otec',
      site_id: 'site_project_otec',
      site_slug: 'otec',
      site_title: 'OTEC Floating Datacenter',
      target: 'order',
      updated_at: '2026-06-05T12:10:00.000Z',
    },
  ]
}

class SiteReferralInspectionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SiteReferralInspectionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    this.store.queries.push({ query: this.query, values: this.values })

    if (this.query.includes('FROM site_referral_sources')) {
      const limit = Number(this.values.at(-1) ?? 100)
      const rows = this.query.includes('referrer_user_id = ?')
        ? this.store.sourceRows.filter(
            row => row.referrer_user_id === String(this.values[0]),
          )
        : this.store.sourceRows

      return Promise.resolve({
        results: rows.slice(0, limit) as unknown as ReadonlyArray<T>,
        success: true,
      } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM referral_attributions')) {
      const limit = Number(this.values[0] ?? 100)
      const rows = this.query.includes("policy_state = 'claimed'")
        ? this.store.attributionRows.filter(
            row =>
              row.policy_state === 'claimed' &&
              row.first_verified_at !== null,
          )
        : this.store.attributionRows

      return Promise.resolve({
        results: rows.slice(0, limit) as unknown as ReadonlyArray<T>,
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const siteReferralInspectionDb = (
  store: SiteReferralInspectionStore,
): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new SiteReferralInspectionStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const makeRoutes = (
  session: TestSession | null,
  requireBrowserSession = () => Promise.resolve(session ?? undefined),
) =>
  makeSiteReferralInspectionRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'admin@openagents.com',
    requireBrowserSession,
  })

const runRoute = (
  request: Request,
  session: TestSession | null,
  store: SiteReferralInspectionStore,
) =>
  Effect.runPromise(
    makeRoutes(session).routeSiteReferralInspectionRequest(
      request,
      { OPENAGENTS_DB: siteReferralInspectionDb(store) },
      executionContext(),
    )!,
  )

describe('Site referral inspection projections', () => {
  test('returns owner-safe aggregate metrics without unsafe source labels', async () => {
    const overview = await readSiteReferralOwnerOverview(
      siteReferralInspectionDb(new SiteReferralInspectionStore()),
      'github:owner',
    )

    expect(overview.sources).toHaveLength(1)
    expect(overview.sources[0]).toMatchObject({
      referralSourceId: 'site_referral_source_otec',
      rewardGate: {
        attributionCaptured: true,
        bitcoinWithdrawalCopyAllowed: false,
        payoutPending: false,
        rewardEligible: false,
        settled: false,
        state: 'blocked_by_policy',
      },
      siteTitle: 'OTEC Floating Datacenter',
      sourceLabel: null,
    })
    expect(overview.sources[0]!.rewardGate.blockerRefs).toEqual([
      'blocker.public.site_referral.cap_exceeded',
      'blocker.public.site_referral.chargeback_refund_or_clawback',
      'blocker.public.site_referral.dispute_hold',
      'blocker.public.site_referral.operator_review',
      'blocker.public.site_referral.policy_held',
    ])
    expect(overview.totals).toMatchObject({
      cappedPolicyCount: 1,
      captureCount: 3,
      disputedPolicyCount: 1,
      heldPolicyCount: 1,
      linkedOrderCount: 1,
      operatorOverrideCount: 1,
      paidWorkflowCount: 1,
      reversedPolicyCount: 1,
      verifiedUserCount: 2,
    })
    expect(JSON.stringify(overview)).not.toMatch(/gho_should_not_render|email/i)
  })

  test('returns operator inspection refs without private contact data', async () => {
    const inspection = await readOperatorSiteReferralInspection(
      siteReferralInspectionDb(new SiteReferralInspectionStore()),
    )

    expect(inspection.sources).toHaveLength(2)
    expect(inspection.sources[0]).toMatchObject({
      rewardGate: {
        bitcoinWithdrawalCopyAllowed: false,
        payoutPending: false,
        rewardEligible: false,
        settled: false,
        state: 'blocked_by_policy',
      },
    })
    expect(inspection.sources[1]).toMatchObject({
      rewardGate: {
        attributionCaptured: true,
        bitcoinWithdrawalCopyAllowed: false,
        payoutPending: false,
        rewardEligible: false,
        settled: false,
        state: 'attribution_only',
      },
    })
    expect(inspection.attributions).toEqual([
      expect.objectContaining({
        claimedUserId: 'github:referred',
        linkedOrderCount: 1,
        referralAttributionId: 'referral_attribution_otec',
      }),
      expect.objectContaining({
        claimedUserId: null,
        linkedOrderCount: 0,
        referralAttributionId: 'referral_attribution_pending',
      }),
    ])
    expect(JSON.stringify(inspection)).not.toMatch(/@|gho_should_not_render/)
  })
})

describe('Site referral inspection routes', () => {
  test('rejects unsupported methods before session lookup', async () => {
    const requireBrowserSession = vi.fn(() =>
      Promise.resolve({
        user: {
          email: 'admin@openagents.com',
          userId: 'github:admin',
        },
      }),
    )
    const response = await Effect.runPromise(
      makeRoutes(null, requireBrowserSession).routeSiteReferralInspectionRequest(
        new Request('https://openagents.com/api/sites/referrals/overview', {
          method: 'POST',
        }),
        {
          OPENAGENTS_DB: siteReferralInspectionDb(
            new SiteReferralInspectionStore(),
          ),
        },
        executionContext(),
      )!,
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    await expect(response.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
    expect(requireBrowserSession).not.toHaveBeenCalled()
  })

  test('requires a browser session for owner overview', async () => {
    const response = await runRoute(
      new Request('https://openagents.com/api/sites/referrals/overview'),
      null,
      new SiteReferralInspectionStore(),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('non-owner sessions do not receive another owner source metrics', async () => {
    const response = await runRoute(
      new Request('https://openagents.com/api/sites/referrals/overview'),
      {
        user: {
          email: 'other@example.com',
          userId: 'github:other',
        },
      },
      new SiteReferralInspectionStore(),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      referralOverview: { sources: Array<{ siteId: string }> }
    }

    expect(body.referralOverview.sources).toHaveLength(1)
    expect(body.referralOverview.sources[0]!.siteId).toBe('site_project_other')
    expect(JSON.stringify(body)).not.toContain('site_project_otec')
  })

  test('operator inspection requires an admin session', async () => {
    const response = await runRoute(
      new Request('https://openagents.com/api/operator/sites/referrals'),
      {
        user: {
          email: 'ben@example.com',
          userId: 'github:ben',
        },
      },
      new SiteReferralInspectionStore(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('operator inspection returns source and attribution state for admins', async () => {
    const store = new SiteReferralInspectionStore()
    const response = await runRoute(
      new Request(
        'https://openagents.com/api/operator/sites/referrals?limit=1',
      ),
      {
        user: {
          email: 'admin@openagents.com',
          userId: 'github:admin',
        },
      },
      store,
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    const body = (await response.json()) as {
      referralInspection: {
        attributions: Array<{
          policyState: string
          publicSourceRef: string
          target: string
        }>
        sources: Array<unknown>
      }
    }

    expect(body.referralInspection.sources).toHaveLength(1)
    expect(body.referralInspection.sources[0]).toMatchObject({
      rewardGate: {
        bitcoinWithdrawalCopyAllowed: false,
        payoutPending: false,
        rewardEligible: false,
        settled: false,
        state: 'blocked_by_policy',
      },
    })
    expect(body.referralInspection.attributions).toHaveLength(1)
    expect(body.referralInspection.attributions[0]).toMatchObject({
      policyState: 'claimed',
      publicSourceRef: 'src_otec',
      target: 'order',
    })
    expect(JSON.stringify(body)).not.toMatch(/email|gho_should_not_render/)
  })

  test('operator consumed attribution query is admin-gated and claimed-only', async () => {
    const forbidden = await runRoute(
      new Request(
        'https://openagents.com/api/operator/sites/referrals/consumed',
      ),
      {
        user: {
          email: 'ben@example.com',
          userId: 'github:ben',
        },
      },
      new SiteReferralInspectionStore(),
    )

    expect(forbidden.status).toBe(403)

    const store = new SiteReferralInspectionStore()
    const response = await runRoute(
      new Request(
        'https://openagents.com/api/operator/sites/referrals/consumed?limit=10',
      ),
      {
        user: {
          email: 'admin@openagents.com',
          userId: 'github:admin',
        },
      },
      store,
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      consumedAttributions: {
        attributions: Array<{
          firstVerifiedAt: string | null
          policyState: string
          referralAttributionId: string
        }>
      }
    }

    expect(body.consumedAttributions.attributions).toEqual([
      expect.objectContaining({
        firstVerifiedAt: '2026-06-05T12:05:00.000Z',
        policyState: 'claimed',
        referralAttributionId: 'referral_attribution_otec',
      }),
    ])
    expect(JSON.stringify(body)).not.toContain('referral_attribution_pending')
    expect(
      store.queries.some(query =>
        query.query.includes("policy_state = 'claimed'"),
      ),
    ).toBe(true)
  })
})
