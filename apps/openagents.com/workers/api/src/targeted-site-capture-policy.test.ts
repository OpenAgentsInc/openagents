import { describe, expect, test } from 'vitest'

import {
  TargetedSiteCapturePolicyValidationError,
  evaluateTargetedSiteCapturePolicy,
  isTargetedSiteCaptureFetchable,
  listTargetedSiteCapturePolicyEventsByCampaign,
  listTargetedSiteCapturePolicyEventsByDomain,
  listTargetedSiteCapturePolicyEventsByProspect,
  operatorTargetedSiteCapturePolicyProjection,
  publicTargetedSiteCapturePolicyProjection,
  recordTargetedSiteCapturePolicyEvent,
  type RecordTargetedSiteCapturePolicyEventInput,
} from './targeted-site-capture-policy'

type StoredCapturePolicyEvent = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  customer_authority_ref: string | null
  decided_at: string
  decision:
    | 'allowed'
    | 'disallowed'
    | 'blocked'
    | 'manual_review'
    | 'customer_owned'
    | 'suppressed'
    | 'paid_escalation'
  fetchable: number
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  operator_actor_user_id: string | null
  operator_note_ref: string | null
  paid_escalation_ref: string | null
  prospect_id: string | null
  reason:
    | 'robots_allowed'
    | 'robots_disallowed'
    | 'robots_unavailable'
    | 'sitemap_available'
    | 'suppression_match'
    | 'customer_owned_domain'
    | 'contact_suppressed'
    | 'operator_manual_review'
    | 'paid_provider_required'
    | 'bot_protection_or_login'
    | 'unsupported_scheme'
    | 'unsafe_domain'
    | 'policy_override'
  robots_ref: string | null
  sitemap_ref: string | null
  source_ref: string
  suppression_ref: string | null
}>

class CapturePolicyStore {
  rows: Array<StoredCapturePolicyEvent> = []
}

class CapturePolicyStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: CapturePolicyStore,
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
      this.query.includes('INSERT OR IGNORE INTO targeted_site_capture_policy_events')
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          created_at: String(this.values[18]),
          customer_authority_ref: this.values[12] as string | null,
          decided_at: String(this.values[17]),
          decision: this.values[6] as StoredCapturePolicyEvent['decision'],
          fetchable: Number(this.values[7]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[16]),
          normalized_domain: String(this.values[4]),
          operator_actor_user_id: this.values[14] as string | null,
          operator_note_ref: this.values[15] as string | null,
          paid_escalation_ref: this.values[13] as string | null,
          prospect_id: this.values[3] as string | null,
          reason: this.values[8] as StoredCapturePolicyEvent['reason'],
          robots_ref: this.values[9] as string | null,
          sitemap_ref: this.values[10] as string | null,
          source_ref: String(this.values[5]),
          suppression_ref: this.values[11] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_capture_policy_events')) {
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

const capturePolicyDb = (store: CapturePolicyStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new CapturePolicyStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const baseInput = {
  campaignId: 'targeted_site_campaign_texas_energy',
  id: 'targeted_site_capture_policy_otec_1',
  idempotencyKey: 'targeted-site-capture-policy:otec:1',
  metadata: { evidenceRef: 'public_capture_policy_card_1' },
  normalizedDomain: 'otec.example',
  prospectId: 'targeted_site_prospect_otec',
  robotsRef: 'robots_ref_otec_1',
  robotsState: 'allowed',
  sitemapRef: 'sitemap_ref_otec_1',
  sitemapState: 'available',
  sourceRef: 'exa_result_ref_otec_1',
} satisfies RecordTargetedSiteCapturePolicyEventInput

describe('targeted Site capture policy', () => {
  test('classifies explicit allowed and paid-escalation decisions as fetchable only', () => {
    expect(evaluateTargetedSiteCapturePolicy(baseInput)).toEqual({
      decision: 'allowed',
      fetchable: true,
      reason: 'sitemap_available',
    })

    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        paidEscalationRef: 'paid_provider_capture_ref_1',
        robotsState: undefined,
        signals: { paidProviderRequired: true },
        sitemapState: undefined,
      }),
    ).toEqual({
      decision: 'paid_escalation',
      fetchable: true,
      reason: 'paid_provider_required',
    })
  })

  test('classifies disallowed, manual-review, blocked, suppressed, and customer-owned states as not fetchable', () => {
    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        robotsState: 'disallowed',
      }),
    ).toEqual({
      decision: 'disallowed',
      fetchable: false,
      reason: 'robots_disallowed',
    })

    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        robotsState: 'unavailable',
      }),
    ).toEqual({
      decision: 'manual_review',
      fetchable: false,
      reason: 'robots_unavailable',
    })

    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        signals: { botProtectionOrLogin: true },
      }),
    ).toEqual({
      decision: 'blocked',
      fetchable: false,
      reason: 'bot_protection_or_login',
    })

    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        suppressionRef: 'suppression_match_ref_1',
      }),
    ).toEqual({
      decision: 'suppressed',
      fetchable: false,
      reason: 'suppression_match',
    })

    expect(
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        customerAuthorityRef: 'customer_authority_domain_ref_1',
        signals: { customerOwnedDomain: true },
      }),
    ).toEqual({
      decision: 'customer_owned',
      fetchable: false,
      reason: 'customer_owned_domain',
    })
  })

  test('records idempotent policy decisions and lists them by campaign, prospect, and domain', async () => {
    const store = new CapturePolicyStore()
    const db = capturePolicyDb(store)

    const first = await recordTargetedSiteCapturePolicyEvent(db, baseInput)
    const replay = await recordTargetedSiteCapturePolicyEvent(db, {
      ...baseInput,
      id: 'targeted_site_capture_policy_replay',
      robotsState: 'disallowed',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      decision: 'allowed',
      fetchable: true,
      id: 'targeted_site_capture_policy_otec_1',
      normalizedDomain: 'otec.example',
      reason: 'sitemap_available',
    })
    expect(replay).toEqual(first)
    expect(isTargetedSiteCaptureFetchable(first)).toBe(true)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteCapturePolicyEventsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteCapturePolicyEventsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteCapturePolicyEventsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('redacts public and operator projections', async () => {
    const db = capturePolicyDb(new CapturePolicyStore())
    const event = await recordTargetedSiteCapturePolicyEvent(db, {
      ...baseInput,
      idempotencyKey: 'targeted-site-capture-policy:otec:redaction',
      operatorActorUserId: 'github:operator',
      operatorNoteRef: 'operator_note_ref_private_1',
      suppressionRef: 'suppression_match_ref_private_1',
    })

    const publicProjection = publicTargetedSiteCapturePolicyProjection(event)
    const operatorProjection = operatorTargetedSiteCapturePolicyProjection(event)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      decidedAt: event.decidedAt,
      decision: 'suppressed',
      fetchable: false,
      normalizedDomain: 'otec.example',
      prospectId: 'targeted_site_prospect_otec',
      sourceRef: 'exa_result_ref_otec_1',
    })
    expect(publicProjection).not.toHaveProperty('reason')
    expect(publicProjection).not.toHaveProperty('suppressionRef')
    expect(publicProjection).not.toHaveProperty('operatorNoteRef')
    expect(operatorProjection).toMatchObject({
      decision: 'suppressed',
      hasOperatorNoteRef: true,
      hasSuppressionRef: true,
      reason: 'suppression_match',
    })
    expect(operatorProjection).not.toHaveProperty('suppressionRef')
    expect(operatorProjection).not.toHaveProperty('operatorNoteRef')
  })

  test('rejects contact, provider, payment, wallet, and bypass material', async () => {
    await expect(
      recordTargetedSiteCapturePolicyEvent(capturePolicyDb(new CapturePolicyStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-capture-policy:unsafe:1',
        metadata: { note: 'email ben@example.com before capture' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteCapturePolicyValidationError)

    expect(() =>
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        sourceRef: 'provider_payload_abc',
      }),
    ).toThrow(TargetedSiteCapturePolicyValidationError)

    expect(() =>
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        paidEscalationRef: 'lnbc_private_invoice',
      }),
    ).toThrow(TargetedSiteCapturePolicyValidationError)

    expect(() =>
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        metadata: { instruction: 'use captcha bypass instructions' },
      }),
    ).toThrow(TargetedSiteCapturePolicyValidationError)
  })

  test('requires a paid escalation ref before paid-provider capture is fetchable', () => {
    expect(() =>
      evaluateTargetedSiteCapturePolicy({
        ...baseInput,
        signals: { paidProviderRequired: true },
      }),
    ).toThrow(TargetedSiteCapturePolicyValidationError)
  })
})
