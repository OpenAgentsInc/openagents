import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TargetedSiteCampaignMetricCampaignNotFound,
  TargetedSiteCampaignMetricProspectNotFound,
  TargetedSiteCampaignMetricValidationError,
  projectTargetedSiteCampaignMetrics,
  publicTargetedSiteCampaignMetricsProjection,
  recordTargetedSiteCampaignMetricEvent,
} from './targeted-site-campaign-metrics'

type Campaign = Readonly<{ archived_at: string | null; id: string }>
type Prospect = Readonly<{
  archived_at: string | null
  campaign_id: string
  id: string
}>
type MetricEvent = Readonly<{
  archived_at: string | null
  campaign_id: string
  cost_cents: number
  created_at: string
  event_kind: string
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string | null
  occurred_at: string
  prospect_id: string | null
  public_ref: string | null
  quantity: number
  related_event_id: string | null
  source_ref: string
}>

class MetricsStore {
  campaigns: Array<Campaign> = [
    { archived_at: null, id: 'targeted_site_campaign_1' },
    { archived_at: '2026-06-05T20:00:00.000Z', id: 'targeted_site_campaign_old' },
  ]
  events: Array<MetricEvent> = []
  prospects: Array<Prospect> = [
    {
      archived_at: null,
      campaign_id: 'targeted_site_campaign_1',
      id: 'targeted_site_prospect_1',
    },
    {
      archived_at: null,
      campaign_id: 'targeted_site_campaign_other',
      id: 'targeted_site_prospect_other',
    },
  ]
}

const runtime = {
  makeEventId: () => 'targeted_site_campaign_metric_generated',
  nowIso: () => '2026-06-05T20:10:00.000Z',
}

class MetricsStatement {
  values: ReadonlyArray<unknown> = []

  constructor(
    readonly query: string,
    private readonly store: MetricsStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): MetricsStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM targeted_site_campaign_metric_events')) {
      if (this.query.includes('idempotency_key = ?')) {
        const idempotencyKey = String(this.values[0])
        const event = this.store.events.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        )

        return Promise.resolve(event === undefined ? null : (event as T))
      }

      return Promise.resolve(this.aggregate() as T)
    }

    if (this.query.includes('FROM targeted_site_campaigns')) {
      const campaignId = String(this.values[0])
      const campaign = this.store.campaigns.find(
        item => item.id === campaignId && item.archived_at === null,
      )

      return Promise.resolve(
        campaign === undefined ? null : ({ id: campaign.id } as T),
      )
    }

    if (this.query.includes('FROM targeted_site_prospects')) {
      const prospectId = String(this.values[0])
      const campaignId = String(this.values[1])
      const prospect = this.store.prospects.find(
        item =>
          item.id === prospectId &&
          item.campaign_id === campaignId &&
          item.archived_at === null,
      )

      return Promise.resolve(
        prospect === undefined ? null : ({ id: prospect.id } as T),
      )
    }

    return Promise.resolve(null)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO targeted_site_campaign_metric_events')) {
      const idempotencyKey = String(this.values[1])
      const existing = this.store.events.some(
        item => item.idempotency_key === idempotencyKey,
      )

      if (!existing) {
        this.store.events.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          cost_cents: Number(this.values[7]),
          created_at: String(this.values[13]),
          event_kind: String(this.values[5]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[11]),
          normalized_domain:
            this.values[4] === null ? null : String(this.values[4]),
          occurred_at: String(this.values[12]),
          prospect_id: this.values[3] === null ? null : String(this.values[3]),
          public_ref: this.values[8] === null ? null : String(this.values[8]),
          quantity: Number(this.values[6]),
          related_event_id:
            this.values[10] === null ? null : String(this.values[10]),
          source_ref: String(this.values[9]),
        })
      }
    }

    return Promise.resolve({ success: true } as unknown as D1Result<T>)
  }

  private aggregate(): Record<string, unknown> {
    const campaignId = String(this.values[1])
    const events = this.store.events.filter(
      event => event.campaign_id === campaignId && event.archived_at === null,
    )
    const countKind = (kind: string): number =>
      events
        .filter(event => event.event_kind === kind)
        .reduce((sum, event) => sum + event.quantity, 0)

    return {
      accepted_outcome_count: countKind('accepted_outcome'),
      blocked_count: countKind('blocked'),
      bounce_count: countKind('email_bounced'),
      campaign_id: campaignId,
      complaint_count: countKind('complaint'),
      conversion_count: countKind('customer_converted'),
      event_count: events.length,
      latest_event_at:
        events.length === 0
          ? null
          : events
              .map(event => event.occurred_at)
              .sort()
              .at(-1) ?? null,
      meeting_count: countKind('meeting_booked'),
      preview_count: countKind('preview_generated'),
      refund_count: countKind('refund'),
      reply_count: countKind('email_replied'),
      sent_count: countKind('outreach_sent'),
      suppressed_count: countKind('suppressed'),
      total_capture_cost_cents: events
        .filter(event => event.event_kind === 'capture_cost')
        .reduce((sum, event) => sum + event.cost_cents, 0),
    }
  }
}

const metricsDb = (store: MetricsStore): D1Database =>
  ({
    prepare: (query: string) =>
      new MetricsStatement(query, store) as unknown as D1PreparedStatement,
  }) as unknown as D1Database

describe('targeted Site campaign metrics', () => {
  test('records idempotent metric events and aggregates campaign projection', async () => {
    const store = new MetricsStore()
    const db = metricsDb(store)
    const first = await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          costCents: 375,
          eventKind: 'capture_cost',
          id: 'targeted_site_campaign_metric_1',
          idempotencyKey: 'metric:capture:1',
          normalizedDomain: 'example.com',
          prospectId: 'targeted_site_prospect_1',
          publicRef: 'capture_run_ref_1',
          sourceRef: 'capture_policy_allowed_1',
        },
        runtime,
      ),
    )
    const replay = await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'capture_cost',
          idempotencyKey: 'metric:capture:1',
          sourceRef: 'capture_policy_allowed_1',
        },
        runtime,
      ),
    )

    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'preview_generated',
          id: 'targeted_site_campaign_metric_2',
          idempotencyKey: 'metric:preview:1',
          quantity: 2,
          sourceRef: 'preview_generation_ref_1',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'outreach_sent',
          id: 'targeted_site_campaign_metric_3',
          idempotencyKey: 'metric:sent:1',
          sourceRef: 'outreach_email_dispatch_1',
        },
        runtime,
      ),
    )

    const metrics = await Effect.runPromise(
      projectTargetedSiteCampaignMetrics(db, 'targeted_site_campaign_1'),
    )

    expect(first.id).toBe(replay.id)
    expect(store.events).toHaveLength(3)
    expect(metrics.totalCaptureCostCents).toBe(375)
    expect(metrics.previewCount).toBe(2)
    expect(metrics.sentCount).toBe(1)
    expect(publicTargetedSiteCampaignMetricsProjection(metrics)).not.toHaveProperty(
      'eventCount',
    )
  })

  test('records conversion lifecycle metrics with refund and complaint linkage', async () => {
    const store = new MetricsStore()
    const db = metricsDb(store)
    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'customer_converted',
          id: 'targeted_site_campaign_metric_conversion',
          idempotencyKey: 'metric:conversion:1',
          sourceRef: 'customer_conversion_ref_1',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'accepted_outcome',
          id: 'targeted_site_campaign_metric_outcome',
          idempotencyKey: 'metric:outcome:1',
          sourceRef: 'accepted_outcome_ref_1',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'refund',
          id: 'targeted_site_campaign_metric_refund',
          idempotencyKey: 'metric:refund:1',
          relatedEventId: 'targeted_site_campaign_metric_conversion',
          sourceRef: 'refund_ref_1',
        },
        runtime,
      ),
    )
    await Effect.runPromise(
      recordTargetedSiteCampaignMetricEvent(
        db,
        {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'complaint',
          id: 'targeted_site_campaign_metric_complaint',
          idempotencyKey: 'metric:complaint:1',
          relatedEventId: 'targeted_site_campaign_metric_outcome',
          sourceRef: 'complaint_ref_1',
        },
        runtime,
      ),
    )
    const metrics = await Effect.runPromise(
      projectTargetedSiteCampaignMetrics(db, 'targeted_site_campaign_1'),
    )

    expect(metrics.conversionCount).toBe(1)
    expect(metrics.acceptedOutcomeCount).toBe(1)
    expect(metrics.refundCount).toBe(1)
    expect(metrics.complaintCount).toBe(1)
  })

  test('rejects unsafe material and invalid campaign or prospect refs', async () => {
    const db = metricsDb(new MetricsStore())

    await expect(
      Effect.runPromise(
        recordTargetedSiteCampaignMetricEvent(db, {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'email_bounced',
          idempotencyKey: 'metric:bounce:secret',
          metadata: { provider_payload: 'raw email body' },
          sourceRef: 'resend_webhook_bounce_1',
        }),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteCampaignMetricValidationError)

    await expect(
      Effect.runPromise(
        recordTargetedSiteCampaignMetricEvent(db, {
          campaignId: 'targeted_site_campaign_old',
          eventKind: 'outreach_sent',
          idempotencyKey: 'metric:old:1',
          sourceRef: 'dispatch_ref_1',
        }),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteCampaignMetricCampaignNotFound)

    await expect(
      Effect.runPromise(
        recordTargetedSiteCampaignMetricEvent(db, {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'outreach_sent',
          idempotencyKey: 'metric:wrong-prospect:1',
          prospectId: 'targeted_site_prospect_other',
          sourceRef: 'dispatch_ref_2',
        }),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteCampaignMetricProspectNotFound)
  })

  test('requires refund and complaint metrics to link a related event', async () => {
    const db = metricsDb(new MetricsStore())

    await expect(
      Effect.runPromise(
        recordTargetedSiteCampaignMetricEvent(db, {
          campaignId: 'targeted_site_campaign_1',
          eventKind: 'refund',
          idempotencyKey: 'metric:refund:missing-related',
          sourceRef: 'refund_ref_2',
        }),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteCampaignMetricValidationError)
  })
})
