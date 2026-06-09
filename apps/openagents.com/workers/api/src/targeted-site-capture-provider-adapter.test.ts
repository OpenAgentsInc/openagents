import { describe, expect, test } from 'vitest'

import type { TargetedSiteCapturePolicyEventRecord } from './targeted-site-capture-policy'
import {
  TargetedSiteCaptureProviderAdapterValidationError,
  listTargetedSiteCaptureProviderAdapterRunsByCampaign,
  listTargetedSiteCaptureProviderAdapterRunsByDomain,
  listTargetedSiteCaptureProviderAdapterRunsByProspect,
  operatorTargetedSiteCaptureProviderAdapterProjection,
  publicTargetedSiteCaptureProviderAdapterProjection,
  recordTargetedSiteCaptureProviderAdapterRun,
  type RecordTargetedSiteCaptureProviderAdapterRunInput,
} from './targeted-site-capture-provider-adapter'

type StoredProviderRun = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  cost_ref: string | null
  created_at: string
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  output_pack_ref: string | null
  paid_escalation_ref: string | null
  prospect_id: string | null
  provider_kind:
    | 'first_party_worker'
    | 'browser_run'
    | 'firecrawl'
    | 'browserless'
    | 'browserbase'
    | 'apify'
    | 'container'
  provider_receipt_ref: string | null
  provider_request_ref: string | null
  reason:
    | 'first_party_default'
    | 'static_insufficient'
    | 'rendered_insufficient'
    | 'paid_escalation_approved'
    | 'benchmark_quality_check'
    | 'cost_not_approved'
    | 'provider_unavailable'
    | 'provider_error'
    | 'manual_review'
    | 'policy_not_fetchable'
    | 'bot_protection_or_login'
  rendered_capture_run_id: string | null
  requested_at: string
  state:
    | 'requested'
    | 'approved_fallback'
    | 'benchmark'
    | 'denied'
    | 'failed'
    | 'partial'
    | 'succeeded'
    | 'manual_review'
    | 'archived'
  static_capture_run_id: string | null
  usage_ref: string | null
}>

class ProviderRunStore {
  rows: Array<StoredProviderRun> = []
}

class ProviderRunStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ProviderRunStore,
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
      this.query.includes(
        'INSERT OR IGNORE INTO targeted_site_capture_provider_adapter_runs',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          capture_policy_event_id: String(this.values[5]),
          completed_at: this.values[19] as string | null,
          cost_ref: this.values[16] as string | null,
          created_at: String(this.values[20]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[17]),
          normalized_domain: String(this.values[4]),
          output_pack_ref: this.values[14] as string | null,
          paid_escalation_ref: this.values[11] as string | null,
          prospect_id: this.values[3] as string | null,
          provider_kind: this.values[8] as StoredProviderRun['provider_kind'],
          provider_receipt_ref: this.values[13] as string | null,
          provider_request_ref: this.values[12] as string | null,
          reason: this.values[10] as StoredProviderRun['reason'],
          rendered_capture_run_id: this.values[7] as string | null,
          requested_at: String(this.values[18]),
          state: this.values[9] as StoredProviderRun['state'],
          static_capture_run_id: this.values[6] as string | null,
          usage_ref: this.values[15] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_capture_provider_adapter_runs')) {
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

const providerRunDb = (store: ProviderRunStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ProviderRunStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const allowedPolicy = {
  archivedAt: null,
  campaignId: 'targeted_site_campaign_texas_energy',
  createdAt: '2026-06-05T20:00:00.000Z',
  customerAuthorityRef: null,
  decidedAt: '2026-06-05T20:00:00.000Z',
  decision: 'allowed',
  fetchable: true,
  id: 'targeted_site_capture_policy_otec_allowed',
  idempotencyKey: 'targeted-site-capture-policy:otec:allowed',
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

const paidPolicy = {
  ...allowedPolicy,
  decision: 'paid_escalation',
  id: 'targeted_site_capture_policy_otec_paid',
  paidEscalationRef: 'paid_provider_capture_ref_1',
  reason: 'paid_provider_required',
} satisfies TargetedSiteCapturePolicyEventRecord

const blockedPolicy = {
  ...allowedPolicy,
  decision: 'blocked',
  fetchable: false,
  id: 'targeted_site_capture_policy_otec_blocked',
  reason: 'bot_protection_or_login',
} satisfies TargetedSiteCapturePolicyEventRecord

const baseInput = {
  capturePolicyEvent: allowedPolicy,
  id: 'targeted_site_capture_provider_run_1',
  idempotencyKey: 'targeted-site-capture-provider:otec:1',
  providerKind: 'browser_run',
  renderedCaptureRunId: 'targeted_site_rendered_capture_otec_1',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
} satisfies RecordTargetedSiteCaptureProviderAdapterRunInput

describe('targeted Site capture provider adapter boundary', () => {
  test('records idempotent first-party or Browser Run adapter runs', async () => {
    const store = new ProviderRunStore()
    const db = providerRunDb(store)
    const first = await recordTargetedSiteCaptureProviderAdapterRun(db, {
      ...baseInput,
      completedAt: '2026-06-05T22:00:00.000Z',
      costRef: 'browser_run_cost_ref_1',
      outputPackRef: 'r2:targeted-site-captures/otec/browser-run-pack.json',
      providerReceiptRef: 'browser_run_receipt_ref_1',
      providerRequestRef: 'browser_run_request_ref_1',
      usageRef: 'browser_run_usage_ref_1',
    })
    const replay = await recordTargetedSiteCaptureProviderAdapterRun(db, {
      ...baseInput,
      id: 'targeted_site_capture_provider_replay',
      providerKind: 'first_party_worker',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      providerKind: 'browser_run',
      reason: 'benchmark_quality_check',
      state: 'benchmark',
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteCaptureProviderAdapterRunsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteCaptureProviderAdapterRunsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteCaptureProviderAdapterRunsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('requires fetchable policy for any provider run', async () => {
    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        capturePolicyEvent: blockedPolicy,
        idempotencyKey: 'targeted-site-capture-provider:blocked-policy',
      }),
    ).rejects.toMatchObject({
      reason:
        'provider adapter runs require an explicit allowed or paid-escalation capture policy event.',
    })
  })

  test('requires paid escalation for paid provider fallback success', async () => {
    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:paid-missing',
        providerKind: 'firecrawl',
        state: 'approved_fallback',
      }),
    ).rejects.toMatchObject({
      reason:
        'paid provider fallback requires explicit paid-escalation policy evidence.',
    })

    const run = await recordTargetedSiteCaptureProviderAdapterRun(
      providerRunDb(new ProviderRunStore()),
      {
        ...baseInput,
        capturePolicyEvent: paidPolicy,
        idempotencyKey: 'targeted-site-capture-provider:paid-ok',
        outputPackRef: 'firecrawl_output_pack_ref_1',
        providerKind: 'firecrawl',
        providerReceiptRef: 'firecrawl_receipt_ref_1',
        providerRequestRef: 'firecrawl_request_ref_1',
        reason: 'paid_escalation_approved',
        state: 'approved_fallback',
      },
    )

    expect(run).toMatchObject({
      paidEscalationRef: 'paid_provider_capture_ref_1',
      providerKind: 'firecrawl',
      reason: 'paid_escalation_approved',
      state: 'approved_fallback',
    })
  })

  test('records denied and failed provider states without raw payloads', async () => {
    const denied = await recordTargetedSiteCaptureProviderAdapterRun(
      providerRunDb(new ProviderRunStore()),
      {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:denied',
        providerKind: 'container',
        reason: 'cost_not_approved',
        state: 'denied',
      },
    )

    expect(denied).toMatchObject({
      providerKind: 'container',
      reason: 'cost_not_approved',
      state: 'denied',
    })
  })

  test('redacts public and operator projections', async () => {
    const run = await recordTargetedSiteCaptureProviderAdapterRun(
      providerRunDb(new ProviderRunStore()),
      {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:redaction',
        metadata: { internal: 'quality benchmark' },
        outputPackRef: 'browser_run_output_pack_ref_1',
        providerReceiptRef: 'browser_run_receipt_ref_1',
        providerRequestRef: 'browser_run_request_ref_1',
      },
    )

    const publicProjection =
      publicTargetedSiteCaptureProviderAdapterProjection(run)
    const operatorProjection =
      operatorTargetedSiteCaptureProviderAdapterProjection(run)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      completedAt: null,
      normalizedDomain: 'otec.example',
      outputAvailable: true,
      prospectId: 'targeted_site_prospect_otec',
      providerKind: 'browser_run',
      requestedAt: run.requestedAt,
      state: 'benchmark',
    })
    expect(publicProjection).not.toHaveProperty('providerRequestRef')
    expect(publicProjection).not.toHaveProperty('providerReceiptRef')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      hasMetadata: true,
      outputPackRef: 'browser_run_output_pack_ref_1',
      providerReceiptRef: 'browser_run_receipt_ref_1',
      providerRequestRef: 'browser_run_request_ref_1',
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('rejects raw provider payloads, contact data, payment material, and bypass instructions', async () => {
    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:unsafe:1',
        providerRequestRef: 'provider_payload_raw_1',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteCaptureProviderAdapterValidationError)

    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:unsafe:2',
        metadata: { contact: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteCaptureProviderAdapterValidationError)

    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        costRef: 'lnbc_private_invoice',
        idempotencyKey: 'targeted-site-capture-provider:unsafe:3',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteCaptureProviderAdapterValidationError)

    await expect(
      recordTargetedSiteCaptureProviderAdapterRun(providerRunDb(new ProviderRunStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-provider:unsafe:4',
        outputPackRef: 'captcha_bypass_pack',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteCaptureProviderAdapterValidationError)
  })
})
