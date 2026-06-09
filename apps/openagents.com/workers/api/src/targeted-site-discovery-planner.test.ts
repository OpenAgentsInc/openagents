import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ExaClientShape, ExaSearchInput, ExaSearchResult } from './exa'
import {
  buildTargetedSiteDiscoveryPlan,
  runTargetedSiteDiscoveryPlan,
  sourceCardsFromExaResults,
} from './targeted-site-discovery-planner'

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

class DiscoveryProspectStore {
  prospects: Array<ProspectRow> = []
}

class DiscoveryProspectStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: DiscoveryProspectStore,
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
              company_name: this.values[1] as string | null,
              discovery_confidence: Number(this.values[7]),
              metadata_json: String(this.values[11]),
              origin_url: this.values[0] as string | null,
              site_name: this.values[2] as string | null,
              source_ref: String(this.values[6]),
              updated_at: String(this.values[12]),
            }
          : row,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
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

const prospectDb = (store: DiscoveryProspectStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new DiscoveryProspectStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const fakeExa = (
  results: ReadonlyArray<ExaSearchResult>,
  captured: Array<ExaSearchInput> = [],
): ExaClientShape => ({
  getContents: () =>
    Effect.succeed({
      results: [],
    }),
  search: input => {
    captured.push(input)

    return Effect.succeed({
      requestId: 'exa_request_targeted_sites',
      results: [...results],
    })
  },
})

const basePlanInput = {
  campaignId: 'targeted_site_campaign_texas_energy',
  idempotencyKeyPrefix: 'targeted-site-prospect:texas-energy',
  maxResults: 50,
  qualitySignals: ['stale design', 'weak CTA', 'slow mobile'],
  sourceRunRef: 'targeted_site_discovery_run_1',
  vertical: 'energy infrastructure',
  geography: 'Texas',
} as const

describe('targeted Site discovery planner', () => {
  test('builds a bounded Exa company search plan from campaign criteria', () => {
    const plan = buildTargetedSiteDiscoveryPlan(basePlanInput)

    expect(plan.maxResults).toBe(25)
    expect(plan.dryRun).toBe(true)
    expect(plan.exaSearch).toMatchObject({
      category: 'company',
      numResults: 25,
      type: 'auto',
    })
    expect(plan.exaSearch.query).toContain('energy infrastructure')
    expect(plan.exaSearch.query).toContain('Texas')
    expect(plan.exaSearch.query).toContain('stale design')
  })

  test('normalizes Exa results into deduped public-safe source cards', () => {
    const plan = buildTargetedSiteDiscoveryPlan({
      ...basePlanInput,
      maxResults: 3,
    })
    const cards = sourceCardsFromExaResults(plan, [
      {
        contents: {
          highlights: ['A public company website with weak mobile CTAs.'],
        },
        score: 0.91,
        title: 'OTEC Example',
        url: 'https://www.otec.example/',
      },
      {
        score: 0.5,
        title: 'Duplicate OTEC',
        url: 'https://otec.example/about',
      },
      {
        score: 1,
        title: 'Bearer gho_secret',
        url: 'https://unsafe.example/',
      },
      {
        score: 0.2,
        title: 'Raw contact page',
        url: 'https://sales@example.com/',
      },
    ])

    expect(cards).toEqual([
      expect.objectContaining({
        confidence: 0.91,
        domain: 'otec.example',
        prospectIdempotencyKey:
          'targeted-site-prospect:texas-energy:otec.example',
        title: 'OTEC Example',
      }),
      expect.objectContaining({
        domain: 'unsafe.example',
        title: null,
      }),
    ])
    expect(JSON.stringify(cards)).not.toMatch(/gho_secret|sales@example.com/)
  })

  test('runs in dry-run mode without writing prospects', async () => {
    const captured: Array<ExaSearchInput> = []
    const store = new DiscoveryProspectStore()
    const result = await Effect.runPromise(
      runTargetedSiteDiscoveryPlan(
        prospectDb(store),
        fakeExa(
          [
            {
              score: 0.7,
              title: 'Grid Services',
              url: 'https://grid.example/',
            },
          ],
          captured,
        ),
        {
          ...basePlanInput,
          dryRun: true,
          maxResults: 2,
        },
      ),
    )

    expect(captured[0]).toMatchObject({
      category: 'company',
      numResults: 2,
      type: 'auto',
    })
    expect(result.persistedProspects).toHaveLength(0)
    expect(result.sourceCards).toHaveLength(1)
    expect(store.prospects).toHaveLength(0)
  })

  test('persists discovered prospects through campaign-domain dedupe', async () => {
    const store = new DiscoveryProspectStore()
    const db = prospectDb(store)
    const exa = fakeExa([
      {
        score: 0.8,
        title: 'Grid Services',
        url: 'https://grid.example/',
      },
      {
        score: 0.6,
        title: 'Ocean Cooling',
        url: 'https://www.ocean-cooling.example/',
      },
    ])

    const first = await Effect.runPromise(
      runTargetedSiteDiscoveryPlan(db, exa, {
        ...basePlanInput,
        dryRun: false,
        maxResults: 2,
      }),
    )
    const second = await Effect.runPromise(
      runTargetedSiteDiscoveryPlan(db, exa, {
        ...basePlanInput,
        dryRun: false,
        maxResults: 2,
      }),
    )

    expect(first.persistedProspects).toHaveLength(2)
    expect(second.persistedProspects).toHaveLength(2)
    expect(store.prospects).toHaveLength(2)
    expect(store.prospects.map(row => row.normalized_domain)).toEqual([
      'grid.example',
      'ocean-cooling.example',
    ])
  })
})
