import { describe, expect, test } from 'vitest'

import {
  TargetedSiteOutreachValidationError,
  createTargetedSiteCampaign,
  listTargetedSiteCampaignsByOperator,
  listTargetedSiteCampaignsByOwner,
  listTargetedSiteProspectsByCampaign,
  upsertTargetedSiteProspect,
} from './targeted-site-outreach'

type CampaignRow = Readonly<{
  archived_at: string | null
  budget_cap_ref: string | null
  created_at: string
  geography: string | null
  id: string
  metadata_json: string
  name: string
  operator_state: 'draft' | 'active' | 'paused' | 'reviewing' | 'completed' | 'archived'
  operator_user_id: string | null
  owner_user_id: string
  slug: string
  source_authority_ref: string
  suppression_policy_ref: string | null
  updated_at: string
  vertical: string | null
}>

type ProspectRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_state:
    | 'not_started'
    | 'policy_pending'
    | 'allowed'
    | 'blocked'
    | 'captured'
    | 'archived'
  company_name: string | null
  contact_refs_json: string
  created_at: string
  discovered_at: string
  discovery_confidence: number
  geography: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  origin_url: string | null
  review_state: 'pending' | 'ready' | 'approved' | 'skipped' | 'archived'
  site_name: string | null
  source_ref: string
  suppression_state: 'unknown' | 'clear' | 'suppressed' | 'manual_review'
  updated_at: string
  vertical: string | null
}>

class OutreachStore {
  campaigns: Array<CampaignRow> = []
  prospects: Array<ProspectRow> = []
}

class OutreachStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OutreachStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM targeted_site_campaigns')) {
      const slug = String(this.values[0])
      const row =
        this.store.campaigns.find(
          item => item.slug === slug && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.prospects.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('WHERE campaign_id = ?') &&
      this.query.includes('AND normalized_domain = ?')
    ) {
      const campaignId = String(this.values[0])
      const normalizedDomain = String(this.values[1])
      const row =
        this.store.prospects.find(
          item =>
            item.campaign_id === campaignId &&
            item.normalized_domain === normalizedDomain &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO targeted_site_campaigns')) {
      const slug = String(this.values[1])

      if (this.store.campaigns.every(row => row.slug !== slug)) {
        this.store.campaigns.push({
          archived_at: null,
          budget_cap_ref: this.values[8] as string | null,
          created_at: String(this.values[12]),
          geography: this.values[6] as string | null,
          id: String(this.values[0]),
          metadata_json: String(this.values[11]),
          name: String(this.values[2]),
          operator_state: this.values[10] as CampaignRow['operator_state'],
          operator_user_id: this.values[4] as string | null,
          owner_user_id: String(this.values[3]),
          slug,
          source_authority_ref: String(this.values[7]),
          suppression_policy_ref: this.values[9] as string | null,
          updated_at: String(this.values[13]),
          vertical: this.values[5] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO targeted_site_prospects')) {
      this.store.prospects.push({
        archived_at: null,
        campaign_id: String(this.values[1]),
        capture_state: this.values[13] as ProspectRow['capture_state'],
        company_name: this.values[5] as string | null,
        contact_refs_json: String(this.values[7]),
        created_at: String(this.values[17]),
        discovered_at: String(this.values[16]),
        discovery_confidence: Number(this.values[11]),
        geography: this.values[9] as string | null,
        id: String(this.values[0]),
        idempotency_key: String(this.values[2]),
        metadata_json: String(this.values[15]),
        normalized_domain: String(this.values[3]),
        origin_url: this.values[4] as string | null,
        review_state: this.values[14] as ProspectRow['review_state'],
        site_name: this.values[6] as string | null,
        source_ref: String(this.values[10]),
        suppression_state:
          this.values[12] as ProspectRow['suppression_state'],
        updated_at: String(this.values[18]),
        vertical: this.values[8] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE targeted_site_prospects')) {
      const campaignId = String(this.values[13])
      const normalizedDomain = String(this.values[14])

      this.store.prospects = this.store.prospects.map(row =>
        row.campaign_id === campaignId &&
        row.normalized_domain === normalizedDomain &&
        row.archived_at === null
          ? {
              ...row,
              capture_state: this.values[9] as ProspectRow['capture_state'],
              company_name: this.values[1] as string | null,
              contact_refs_json: String(this.values[3]),
              discovery_confidence: Number(this.values[7]),
              geography: this.values[5] as string | null,
              metadata_json: String(this.values[11]),
              origin_url: this.values[0] as string | null,
              review_state: this.values[10] as ProspectRow['review_state'],
              site_name: this.values[2] as string | null,
              source_ref: String(this.values[6]),
              suppression_state:
                this.values[8] as ProspectRow['suppression_state'],
              updated_at: String(this.values[12]),
              vertical: this.values[4] as string | null,
            }
          : row,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_campaigns')) {
      const userId = String(this.values[0])
      const rows = this.query.includes('owner_user_id = ?')
        ? this.store.campaigns.filter(row => row.owner_user_id === userId)
        : this.store.campaigns.filter(row => row.operator_user_id === userId)
      const limit = Number(this.values[1] ?? 100)

      return Promise.resolve({
        results: rows.slice(0, limit) as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('FROM targeted_site_prospects')) {
      const campaignId = String(this.values[0])
      const limit = Number(this.values[1] ?? 100)
      const rows = this.store.prospects.filter(
        row => row.campaign_id === campaignId && row.archived_at === null,
      )

      return Promise.resolve({
        results: rows.slice(0, limit) as unknown as ReadonlyArray<T>,
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

const outreachDb = (store: OutreachStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OutreachStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const campaignInput = {
  budgetCapRef: 'budget_cap_internal_sites_1',
  geography: 'Texas',
  id: 'targeted_site_campaign_texas_energy',
  metadata: { source: 'operator_seed' },
  name: 'Texas Energy Site Remakes',
  operatorState: 'active',
  operatorUserId: 'github:operator',
  ownerUserId: 'github:owner',
  slug: 'texas-energy-sites',
  sourceAuthorityRef: 'targeted_site_outreach.manual_seed',
  suppressionPolicyRef: 'suppression_policy_marketing_sites',
  vertical: 'energy',
} as const

describe('targeted Site outreach campaigns and prospects', () => {
  test('creates a campaign and lists it by owner and operator', async () => {
    const store = new OutreachStore()
    const db = outreachDb(store)
    const campaign = await createTargetedSiteCampaign(db, campaignInput)

    expect(campaign).toMatchObject({
      id: 'targeted_site_campaign_texas_energy',
      operatorState: 'active',
      ownerUserId: 'github:owner',
      slug: 'texas-energy-sites',
    })
    await expect(
      listTargetedSiteCampaignsByOwner(db, 'github:owner'),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteCampaignsByOperator(db, 'github:operator'),
    ).resolves.toHaveLength(1)
  })

  test('upserts prospects idempotently and dedupes by campaign domain', async () => {
    const store = new OutreachStore()
    const db = outreachDb(store)
    await createTargetedSiteCampaign(db, campaignInput)

    const first = await upsertTargetedSiteProspect(db, {
      campaignId: campaignInput.id,
      companyName: 'Ocean Thermal Systems',
      contactRefs: ['crm_contact_ref_otec'],
      discoveryConfidence: 0.77,
      geography: 'Hawaii',
      id: 'targeted_site_prospect_otec',
      idempotencyKey: 'targeted-site-prospect:texas-energy:otec:1',
      metadata: { exaResultRef: 'exa_result_ref_otec_1' },
      originUrl: 'https://www.otec.example/about?source=public',
      reviewState: 'ready',
      siteName: 'OTEC Example',
      sourceRef: 'exa_result_ref_otec_1',
      suppressionState: 'clear',
      targetDomain: 'www.otec.example',
      vertical: 'ocean infrastructure',
    })
    const replay = await upsertTargetedSiteProspect(db, {
      campaignId: campaignInput.id,
      companyName: 'Changed Name',
      discoveryConfidence: 1,
      idempotencyKey: 'targeted-site-prospect:texas-energy:otec:1',
      sourceRef: 'exa_result_ref_otec_2',
      targetDomain: 'otec.example',
    })
    const deduped = await upsertTargetedSiteProspect(db, {
      campaignId: campaignInput.id,
      captureState: 'policy_pending',
      companyName: 'Ocean Thermal Systems Updated',
      discoveryConfidence: 0.88,
      idempotencyKey: 'targeted-site-prospect:texas-energy:otec:2',
      sourceRef: 'manual_import_ref_otec',
      suppressionState: 'manual_review',
      targetDomain: 'https://otec.example/',
    })

    expect(first.normalizedDomain).toBe('otec.example')
    expect(replay).toEqual(first)
    expect(deduped).toMatchObject({
      captureState: 'policy_pending',
      companyName: 'Ocean Thermal Systems Updated',
      id: 'targeted_site_prospect_otec',
      idempotencyKey: 'targeted-site-prospect:texas-energy:otec:1',
      suppressionState: 'manual_review',
    })
    expect(store.prospects).toHaveLength(1)
    await expect(
      listTargetedSiteProspectsByCampaign(db, campaignInput.id),
    ).resolves.toEqual([
      expect.objectContaining({
        normalizedDomain: 'otec.example',
        suppressionState: 'manual_review',
      }),
    ])
  })

  test('rejects raw contact, provider, payment, wallet, and operator-note material', async () => {
    await expect(
      createTargetedSiteCampaign(outreachDb(new OutreachStore()), {
        ...campaignInput,
        slug: 'unsafe-campaign',
        metadata: { operatorNote: 'email ben@example.com directly' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteOutreachValidationError)

    await expect(
      upsertTargetedSiteProspect(outreachDb(new OutreachStore()), {
        campaignId: campaignInput.id,
        companyName: 'Bearer gho_secret',
        discoveryConfidence: 0.5,
        idempotencyKey: 'targeted-site-prospect:unsafe:1',
        sourceRef: 'exa_result_ref_safe',
        targetDomain: 'example.com',
      }),
    ).rejects.toMatchObject({
      reason:
        'companyName must not contain raw contact, provider, payment, wallet, or operator-note material.',
    })

    await expect(
      upsertTargetedSiteProspect(outreachDb(new OutreachStore()), {
        campaignId: campaignInput.id,
        contactRefs: ['ben@example.com'],
        discoveryConfidence: 0.5,
        idempotencyKey: 'targeted-site-prospect:unsafe:2',
        sourceRef: 'exa_result_ref_safe',
        targetDomain: 'example.com',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteOutreachValidationError)
  })
})
