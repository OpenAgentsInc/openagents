import { describe, expect, test } from 'vitest'

import type { TargetedSiteCapturePolicyEventRecord } from './targeted-site-capture-policy'
import {
  TargetedSiteRenderedCaptureValidationError,
  listTargetedSiteRenderedCaptureRunsByCampaign,
  listTargetedSiteRenderedCaptureRunsByDomain,
  listTargetedSiteRenderedCaptureRunsByProspect,
  operatorTargetedSiteRenderedCaptureProjection,
  publicTargetedSiteRenderedCaptureProjection,
  recordTargetedSiteRenderedCaptureRun,
  type RecordTargetedSiteRenderedCaptureRunInput,
} from './targeted-site-rendered-capture'

type StoredRenderedCaptureRun = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  crawl_ref: string | null
  created_at: string
  device_ref: string | null
  id: string
  idempotency_key: string
  links_ref: string | null
  markdown_ref: string | null
  metadata_json: string
  normalized_domain: string
  prospect_id: string | null
  provider_ref: string
  reason:
    | 'policy_fetchable'
    | 'policy_not_fetchable'
    | 'static_capture_insufficient'
    | 'screenshot_ready'
    | 'rendered_source_ready'
    | 'crawl_ready'
    | 'usage_limit'
    | 'network_error'
    | 'provider_error'
    | 'bot_protection_or_login'
    | 'manual_review'
  rendered_html_ref: string | null
  screenshot_ref: string | null
  started_at: string
  state:
    | 'planned'
    | 'succeeded'
    | 'partial'
    | 'failed'
    | 'blocked'
    | 'manual_review'
    | 'archived'
  static_capture_run_id: string | null
  structured_json_ref: string | null
  target_url: string
  usage_summary_json: string
  viewport_ref: string | null
}>

class RenderedCaptureStore {
  rows: Array<StoredRenderedCaptureRun> = []
}

class RenderedCaptureStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: RenderedCaptureStore,
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
      this.query.includes('INSERT OR IGNORE INTO targeted_site_rendered_capture_runs')
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          capture_policy_event_id: String(this.values[5]),
          completed_at: this.values[22] as string | null,
          crawl_ref: this.values[16] as string | null,
          created_at: String(this.values[23]),
          device_ref: this.values[18] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          links_ref: this.values[14] as string | null,
          markdown_ref: this.values[13] as string | null,
          metadata_json: String(this.values[20]),
          normalized_domain: String(this.values[4]),
          prospect_id: this.values[3] as string | null,
          provider_ref: String(this.values[10]),
          reason: this.values[8] as StoredRenderedCaptureRun['reason'],
          rendered_html_ref: this.values[12] as string | null,
          screenshot_ref: this.values[11] as string | null,
          started_at: String(this.values[21]),
          state: this.values[7] as StoredRenderedCaptureRun['state'],
          static_capture_run_id: this.values[6] as string | null,
          structured_json_ref: this.values[15] as string | null,
          target_url: String(this.values[9]),
          usage_summary_json: String(this.values[19]),
          viewport_ref: this.values[17] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_rendered_capture_runs')) {
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

const renderedCaptureDb = (store: RenderedCaptureStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new RenderedCaptureStatement(query, store),
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
  decision: 'suppressed',
  fetchable: false,
  id: 'targeted_site_capture_policy_suppressed_1',
  reason: 'suppression_match',
} satisfies TargetedSiteCapturePolicyEventRecord

const baseInput = {
  capturePolicyEvent: fetchablePolicy,
  completedAt: '2026-06-05T21:05:00.000Z',
  crawlRef: 'browser_run_crawl_ref_1',
  deviceRef: 'device_desktop_chrome',
  id: 'targeted_site_rendered_capture_otec_1',
  idempotencyKey: 'targeted-site-rendered-capture:otec:1',
  linksRef: 'browser_run_links_ref_1',
  markdownRef: 'browser_run_markdown_ref_1',
  metadata: { runClass: 'rendered_static_escalation' },
  providerRef: 'browser_run',
  renderedHtmlRef: 'browser_run_html_ref_1',
  screenshotRef: 'browser_run_screenshot_ref_1',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
  structuredJsonRef: 'browser_run_json_ref_1',
  targetUrl: '/about#team',
  usageSummary: {
    browserMs: 12_345,
    bytes: 4_567_890,
    costRef: 'browser_run_cost_ref_1',
    estimatedCostCredits: 3.456789,
    pages: 7,
  },
  viewportRef: 'viewport_1440x1200',
} satisfies RecordTargetedSiteRenderedCaptureRunInput

describe('targeted Site rendered capture', () => {
  test('records idempotent successful rendered capture behind fetchable policy', async () => {
    const store = new RenderedCaptureStore()
    const db = renderedCaptureDb(store)
    const first = await recordTargetedSiteRenderedCaptureRun(db, baseInput)
    const replay = await recordTargetedSiteRenderedCaptureRun(db, {
      ...baseInput,
      id: 'targeted_site_rendered_capture_replay',
      state: 'failed',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      capturePolicyEventId: 'targeted_site_capture_policy_otec_1',
      crawlRef: 'browser_run_crawl_ref_1',
      normalizedDomain: 'otec.example',
      reason: 'crawl_ready',
      screenshotRef: 'browser_run_screenshot_ref_1',
      state: 'succeeded',
      staticCaptureRunId: 'targeted_site_static_capture_otec_1',
      targetUrl: 'https://otec.example/about',
      usageSummary: {
        browserMs: 12345,
        bytes: 4567890,
        costRef: 'browser_run_cost_ref_1',
        estimatedCostCredits: 3.4568,
        pages: 7,
      },
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteRenderedCaptureRunsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRenderedCaptureRunsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRenderedCaptureRunsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('blocks rendered capture without fetchable policy', async () => {
    await expect(
      recordTargetedSiteRenderedCaptureRun(
        renderedCaptureDb(new RenderedCaptureStore()),
        {
          ...baseInput,
          capturePolicyEvent: blockedPolicy,
          idempotencyKey: 'targeted-site-rendered-capture:blocked-policy:1',
        },
      ),
    ).rejects.toMatchObject({
      reason:
        'rendered capture requires an explicit allowed or paid-escalation capture policy event.',
    })
  })

  test('records bot-protection as blocked only when no rendered output refs are present', async () => {
    const blocked = await recordTargetedSiteRenderedCaptureRun(
      renderedCaptureDb(new RenderedCaptureStore()),
      {
        capturePolicyEvent: fetchablePolicy,
        idempotencyKey: 'targeted-site-rendered-capture:bot-blocked:1',
        signals: { botProtectionOrLogin: true },
        targetUrl: '/',
      },
    )

    expect(blocked).toMatchObject({
      reason: 'bot_protection_or_login',
      state: 'blocked',
    })

    await expect(
      recordTargetedSiteRenderedCaptureRun(
        renderedCaptureDb(new RenderedCaptureStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-rendered-capture:bot-output:1',
          signals: { botProtectionOrLogin: true },
        },
      ),
    ).rejects.toMatchObject({
      reason: 'bot-protection or login-wall targets cannot record rendered output refs.',
    })
  })

  test('bounds usage summaries', async () => {
    const run = await recordTargetedSiteRenderedCaptureRun(
      renderedCaptureDb(new RenderedCaptureStore()),
      {
        ...baseInput,
        idempotencyKey: 'targeted-site-rendered-capture:usage-bounds:1',
        usageSummary: {
          browserMs: 9_999_999,
          bytes: 999_999_999,
          estimatedCostCredits: 99_999,
          pages: 999,
        },
      },
    )

    expect(run.usageSummary).toEqual({
      browserMs: 3600000,
      bytes: 50000000,
      costRef: null,
      estimatedCostCredits: 10000,
      pages: 100,
    })
  })

  test('redacts public and operator projections', async () => {
    const run = await recordTargetedSiteRenderedCaptureRun(
      renderedCaptureDb(new RenderedCaptureStore()),
      {
        ...baseInput,
        idempotencyKey: 'targeted-site-rendered-capture:redaction:1',
      },
    )
    const publicProjection = publicTargetedSiteRenderedCaptureProjection(run)
    const operatorProjection = operatorTargetedSiteRenderedCaptureProjection(run)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      completedAt: '2026-06-05T21:05:00.000Z',
      hasCrawl: true,
      hasMarkdown: true,
      hasScreenshot: true,
      normalizedDomain: 'otec.example',
      prospectId: 'targeted_site_prospect_otec',
      startedAt: run.startedAt,
      state: 'succeeded',
      targetUrl: 'https://otec.example/about',
    })
    expect(publicProjection).not.toHaveProperty('providerRef')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      capturePolicyEventId: 'targeted_site_capture_policy_otec_1',
      hasMetadata: true,
      providerRef: 'browser_run',
      usageSummary: {
        browserMs: 12345,
        bytes: 4567890,
        costRef: 'browser_run_cost_ref_1',
        estimatedCostCredits: 3.4568,
        pages: 7,
      },
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('rejects provider payloads, raw contact data, and bypass instructions', async () => {
    await expect(
      recordTargetedSiteRenderedCaptureRun(
        renderedCaptureDb(new RenderedCaptureStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-rendered-capture:unsafe:1',
          providerRef: 'provider_payload_private',
        },
      ),
    ).rejects.toBeInstanceOf(TargetedSiteRenderedCaptureValidationError)

    await expect(
      recordTargetedSiteRenderedCaptureRun(
        renderedCaptureDb(new RenderedCaptureStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-rendered-capture:unsafe:2',
          metadata: { contact: 'ben@example.com' },
        },
      ),
    ).rejects.toBeInstanceOf(TargetedSiteRenderedCaptureValidationError)

    await expect(
      recordTargetedSiteRenderedCaptureRun(
        renderedCaptureDb(new RenderedCaptureStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-rendered-capture:unsafe:3',
          screenshotRef: 'captcha_bypass_ref',
        },
      ),
    ).rejects.toBeInstanceOf(TargetedSiteRenderedCaptureValidationError)
  })
})
