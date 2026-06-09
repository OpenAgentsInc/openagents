import { describe, expect, test } from 'vitest'

import type { TargetedSiteCapturePolicyEventRecord } from './targeted-site-capture-policy'
import {
  TargetedSiteStaticCaptureValidationError,
  listTargetedSiteStaticCaptureRunsByCampaign,
  listTargetedSiteStaticCaptureRunsByDomain,
  listTargetedSiteStaticCaptureRunsByProspect,
  normalizeTargetedSiteStaticCaptureUrl,
  operatorTargetedSiteStaticCaptureProjection,
  publicTargetedSiteStaticCaptureProjection,
  recordTargetedSiteStaticCaptureRun,
  type RecordTargetedSiteStaticCaptureRunInput,
} from './targeted-site-static-capture'

type StoredStaticCaptureRun = Readonly<{
  archived_at: string | null
  asset_refs_json: string
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  created_at: string
  homepage_ref: string | null
  homepage_url: string
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  page_refs_json: string
  prospect_id: string | null
  reason:
    | 'policy_fetchable'
    | 'policy_not_fetchable'
    | 'homepage_fetched'
    | 'partial_pages'
    | 'network_error'
    | 'invalid_url'
    | 'cross_origin_url'
    | 'response_too_large'
    | 'unsupported_content_type'
    | 'robots_changed'
    | 'manual_review'
    | 'source_pack_ready'
  response_summary_json: string
  robots_ref: string | null
  sitemap_ref: string | null
  source_hash: string | null
  source_pack_ref: string | null
  started_at: string
  state:
    | 'planned'
    | 'succeeded'
    | 'partial'
    | 'failed'
    | 'blocked'
    | 'manual_review'
    | 'archived'
}>

class StaticCaptureStore {
  rows: Array<StoredStaticCaptureRun> = []
}

class StaticCaptureStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: StaticCaptureStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.rows.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('INSERT OR IGNORE INTO targeted_site_static_capture_runs')
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          asset_refs_json: String(this.values[15]),
          campaign_id: String(this.values[2]),
          capture_policy_event_id: String(this.values[5]),
          completed_at: this.values[19] as string | null,
          created_at: String(this.values[20]),
          homepage_ref: this.values[9] as string | null,
          homepage_url: String(this.values[8]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[17]),
          normalized_domain: String(this.values[4]),
          page_refs_json: String(this.values[14]),
          prospect_id: this.values[3] as string | null,
          reason: this.values[7] as StoredStaticCaptureRun['reason'],
          response_summary_json: String(this.values[16]),
          robots_ref: this.values[10] as string | null,
          sitemap_ref: this.values[11] as string | null,
          source_hash: this.values[13] as string | null,
          source_pack_ref: this.values[12] as string | null,
          started_at: String(this.values[18]),
          state: this.values[6] as StoredStaticCaptureRun['state'],
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_static_capture_runs')) {
      const value = String(this.values[0])
      const key = this.query.includes('campaign_id = ?')
        ? 'campaign_id'
        : this.query.includes('prospect_id = ?')
          ? 'prospect_id'
          : 'normalized_domain'
      const limit = Number(this.values[1] ?? 100)
      const rows = this.store.rows
        .filter(row => row.archived_at === null && row[key] === value)
        .slice(0, limit)

      return Promise.resolve({
        results: rows as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }
}

const staticCaptureDb = (store: StaticCaptureStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new StaticCaptureStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const fetchablePolicy = {
  archivedAt: null,
  campaignId: 'targeted_site_campaign_texas_energy',
  createdAt: '2026-06-05T20:00:00.000Z',
  customerAuthorityRef: null,
  decidedAt: '2026-06-05T20:00:00.000Z',
  decision: 'allowed',
  fetchable: true,
  id: 'targeted_site_capture_policy_otec_1',
  idempotencyKey: 'targeted-site-capture-policy:otec:1',
  metadata: {},
  normalizedDomain: 'otec.example',
  operatorActorUserId: null,
  operatorNoteRef: null,
  paidEscalationRef: null,
  prospectId: 'targeted_site_prospect_otec',
  reason: 'sitemap_available',
  robotsRef: 'robots_ref_otec_1',
  sitemapRef: 'sitemap_ref_otec_1',
  sourceRef: 'exa_result_ref_otec_1',
  suppressionRef: null,
} satisfies TargetedSiteCapturePolicyEventRecord

const blockedPolicy = {
  ...fetchablePolicy,
  decision: 'blocked',
  fetchable: false,
  id: 'targeted_site_capture_policy_blocked_1',
  reason: 'bot_protection_or_login',
} satisfies TargetedSiteCapturePolicyEventRecord

const baseInput = {
  assetRefs: [
    {
      kind: 'image',
      ref: 'asset_ref_logo_1',
      sourceHash:
        'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      url: '/logo.png',
    },
  ],
  capturePolicyEvent: fetchablePolicy,
  completedAt: '2026-06-05T20:05:00.000Z',
  homepageRef: 'static_capture_homepage_ref_1',
  homepageUrl: 'https://www.otec.example/?utm=campaign#hero',
  id: 'targeted_site_static_capture_otec_1',
  idempotencyKey: 'targeted-site-static-capture:otec:1',
  metadata: { sourcePackVersion: 'v1' },
  pageRefs: [
    {
      ref: 'page_ref_about_1',
      sourceHash:
        'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      url: '/about#team',
    },
  ],
  responseSummary: {
    bytes: 12345,
    contentType: 'text/html; charset=utf-8',
    headersRef: 'headers_ref_homepage_1',
    status: 200,
  },
  sourceHash:
    'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
  sourcePackRef: 'r2:targeted-site-captures/otec/source-pack.json',
} satisfies RecordTargetedSiteStaticCaptureRunInput

describe('targeted Site static capture', () => {
  test('normalizes same-origin capture URLs and rejects cross-origin URLs', () => {
    expect(
      normalizeTargetedSiteStaticCaptureUrl('otec.example', '/about#team'),
    ).toBe('https://otec.example/about')
    expect(
      normalizeTargetedSiteStaticCaptureUrl(
        'otec.example',
        'https://www.otec.example/path?x=1#frag',
      ),
    ).toBe('https://www.otec.example/path?x=1')

    expect(() =>
      normalizeTargetedSiteStaticCaptureUrl(
        'otec.example',
        'https://other.example/path',
      ),
    ).toThrow(TargetedSiteStaticCaptureValidationError)
  })

  test('records idempotent successful capture runs behind fetchable policy', async () => {
    const store = new StaticCaptureStore()
    const db = staticCaptureDb(store)
    const first = await recordTargetedSiteStaticCaptureRun(db, baseInput)
    const replay = await recordTargetedSiteStaticCaptureRun(db, {
      ...baseInput,
      id: 'targeted_site_static_capture_replay',
      state: 'failed',
    })

    expect(first).toMatchObject({
      assetRefs: [
        expect.objectContaining({
          ref: 'asset_ref_logo_1',
          url: 'https://otec.example/logo.png',
        }),
      ],
      capturePolicyEventId: 'targeted_site_capture_policy_otec_1',
      homepageUrl: 'https://www.otec.example/?utm=campaign',
      pageRefs: [
        expect.objectContaining({
          ref: 'page_ref_about_1',
          url: 'https://otec.example/about',
        }),
      ],
      reason: 'source_pack_ready',
      sourcePackRef: 'r2:targeted-site-captures/otec/source-pack.json',
      state: 'succeeded',
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteStaticCaptureRunsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteStaticCaptureRunsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteStaticCaptureRunsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('blocks static capture without fetchable policy', async () => {
    await expect(
      recordTargetedSiteStaticCaptureRun(staticCaptureDb(new StaticCaptureStore()), {
        ...baseInput,
        capturePolicyEvent: blockedPolicy,
        idempotencyKey: 'targeted-site-static-capture:blocked:1',
      }),
    ).rejects.toMatchObject({
      reason:
        'static capture requires an explicit allowed or paid-escalation capture policy event.',
    })
  })

  test('bounds asset and page refs', async () => {
    const assets = Array.from({ length: 120 }, (_, index) => ({
      kind: 'other' as const,
      ref: `asset_ref_${index}`,
      url: `/asset-${index}.txt`,
    }))
    const pages = Array.from({ length: 40 }, (_, index) => ({
      ref: `page_ref_${index}`,
      url: `/page-${index}`,
    }))
    const run = await recordTargetedSiteStaticCaptureRun(
      staticCaptureDb(new StaticCaptureStore()),
      {
        ...baseInput,
        assetRefs: assets,
        idempotencyKey: 'targeted-site-static-capture:bounded:1',
        pageRefs: pages,
      },
    )

    expect(run.assetRefs).toHaveLength(100)
    expect(run.pageRefs).toHaveLength(25)
  })

  test('redacts public and operator projections', async () => {
    const run = await recordTargetedSiteStaticCaptureRun(
      staticCaptureDb(new StaticCaptureStore()),
      {
        ...baseInput,
        idempotencyKey: 'targeted-site-static-capture:redaction:1',
      },
    )
    const publicProjection = publicTargetedSiteStaticCaptureProjection(run)
    const operatorProjection = operatorTargetedSiteStaticCaptureProjection(run)

    expect(publicProjection).toEqual({
      assetCount: 1,
      campaignId: 'targeted_site_campaign_texas_energy',
      completedAt: '2026-06-05T20:05:00.000Z',
      homepageUrl: 'https://www.otec.example/?utm=campaign',
      normalizedDomain: 'otec.example',
      pageCount: 1,
      prospectId: 'targeted_site_prospect_otec',
      sourcePackRef: 'r2:targeted-site-captures/otec/source-pack.json',
      startedAt: run.startedAt,
      state: 'succeeded',
    })
    expect(publicProjection).not.toHaveProperty('assetRefs')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      capturePolicyEventId: 'targeted_site_capture_policy_otec_1',
      hasMetadata: true,
      reason: 'source_pack_ready',
      responseSummary: {
        bytes: 12345,
        contentType: 'text/html; charset=utf-8',
        headersRef: 'headers_ref_homepage_1',
        status: 200,
      },
    })
    expect(operatorProjection).not.toHaveProperty('assetRefs')
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('rejects private material in refs, metadata, and response summaries', async () => {
    await expect(
      recordTargetedSiteStaticCaptureRun(staticCaptureDb(new StaticCaptureStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-static-capture:unsafe:1',
        metadata: { contact: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteStaticCaptureValidationError)

    await expect(
      recordTargetedSiteStaticCaptureRun(staticCaptureDb(new StaticCaptureStore()), {
        ...baseInput,
        homepageRef: 'provider_payload_private',
        idempotencyKey: 'targeted-site-static-capture:unsafe:2',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteStaticCaptureValidationError)

    await expect(
      recordTargetedSiteStaticCaptureRun(staticCaptureDb(new StaticCaptureStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-static-capture:unsafe:3',
        responseSummary: {
          contentType: 'captcha bypass instructions',
          status: 200,
        },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteStaticCaptureValidationError)
  })
})
