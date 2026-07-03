import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { BusinessKpiRoute } from '../route'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

type MetricUnit = 'count' | 'percent' | 'currency'

type KpiMetric = Readonly<{
  key: string
  label: string
  unit: MetricUnit
  baseline: number
  current: number
  definition: string
  sourceRefs: ReadonlyArray<string>
}>

type EngagementKpiSnapshot = Readonly<{
  engagementRef: string
  vertical: string
  generatedAt: string
  baselineCapturedAt: string
  currentWindow: string
  dataSource: string
  status: 'live'
  metrics: ReadonlyArray<KpiMetric>
  evidenceRefs: ReadonlyArray<string>
  privacyExcludes: ReadonlyArray<string>
}>

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const dashboard: EngagementKpiSnapshot = {
  engagementRef: 'engagement.public.vertical_pipeline_1',
  vertical: 'service business funnel',
  generatedAt: '2026-07-03T00:00:00.000Z',
  baselineCapturedAt: '2026-07-02T15:00:00.000Z',
  currentWindow: 'last_30_days',
  dataSource: '/api/public/business/funnel-dashboard',
  status: 'live',
  metrics: [
    {
      key: 'lead_volume',
      label: 'Lead volume',
      unit: 'count',
      baseline: 6,
      current: 18,
      definition: 'Qualified signup or booked-consult lead rows counted once.',
      sourceRefs: ['metric.business.lead_volume', 'roadmap:BF-1.4'],
    },
    {
      key: 'conversion',
      label: 'Conversion',
      unit: 'percent',
      baseline: 8.3,
      current: 16.7,
      definition: 'Paid or accepted first outcome divided by qualified leads.',
      sourceRefs: ['metric.business.conversion', 'roadmap:BF-7.1'],
    },
    {
      key: 'aov',
      label: 'AOV',
      unit: 'currency',
      baseline: 120000,
      current: 260000,
      definition: 'Average order value from receipted engagement revenue only.',
      sourceRefs: ['metric.business.aov', 'receipt.business.revenue.opaque'],
    },
    {
      key: 'revenue',
      label: 'Revenue',
      unit: 'currency',
      baseline: 720000,
      current: 4680000,
      definition: 'Gross receipted revenue attributed to this engagement window.',
      sourceRefs: ['metric.business.revenue', 'receipt.business.revenue.opaque'],
    },
    {
      key: 'consult_attach',
      label: 'Consult attach',
      unit: 'percent',
      baseline: 33.3,
      current: 55.6,
      definition: 'Qualified leads with a consult or scope call attached.',
      sourceRefs: ['metric.business.consult_attach', 'roadmap:BF-7.1'],
    },
  ],
  evidenceRefs: [
    'table:business_funnel_events',
    'endpoint:/api/public/business/funnel-dashboard',
    'issue:8105',
    'roadmap:BF-7.1',
  ],
  privacyExcludes: [
    'client name',
    'contact email',
    'phone',
    'payment payload',
    'raw provider payload',
  ],
}

const formatMetricValue = (metric: KpiMetric, value: number): string => {
  if (metric.unit === 'currency') {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      maximumFractionDigits: 0,
    }).format(value / 100)
  }

  if (metric.unit === 'percent') {
    return `${value.toFixed(1)}%`
  }

  return new Intl.NumberFormat('en-US').format(value)
}

const metricDelta = (metric: KpiMetric): string => {
  const delta = metric.current - metric.baseline
  const sign = delta >= 0 ? '+' : ''

  if (metric.unit === 'currency') {
    return `${sign}${formatMetricValue(metric, delta)}`
  }

  if (metric.unit === 'percent') {
    return `${sign}${delta.toFixed(1)} pts`
  }

  return `${sign}${new Intl.NumberFormat('en-US').format(delta)}`
}

const metricRow = <Message>(metric: KpiMetric): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#010102] p-4',
      ),
      h.DataAttribute('business-kpi-metric', metric.key),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-start justify-between gap-3')],
        [
          h.h3(
            [Ui.className<Message>('m-0 text-sm font-medium text-[#f1efe8]')],
            [metric.label],
          ),
          h.span(
            [
              Ui.className<Message>(
                'border border-[#1f4d2b] bg-[#06140a] px-2 py-1 font-mono text-[0.6875rem] uppercase text-[#7fdc9b]',
              ),
            ],
            ['Live'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid grid-cols-3 gap-2 font-mono')],
        [
          h.p(
            [Ui.className<Message>('m-0 grid gap-1 text-xs text-white/45')],
            [
              h.span([], ['Baseline']),
              h.strong(
                [Ui.className<Message>('text-lg font-semibold text-white/85')],
                [formatMetricValue(metric, metric.baseline)],
              ),
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0 grid gap-1 text-xs text-white/45')],
            [
              h.span([], ['Current']),
              h.strong(
                [Ui.className<Message>('text-lg font-semibold text-white/90')],
                [formatMetricValue(metric, metric.current)],
              ),
            ],
          ),
          h.p(
            [Ui.className<Message>('m-0 grid gap-1 text-xs text-white/45')],
            [
              h.span([], ['Delta']),
              h.strong(
                [Ui.className<Message>('text-lg font-semibold text-[#ffb400]')],
                [metricDelta(metric)],
              ),
            ],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-sm leading-6 text-white/60')],
        [metric.definition],
      ),
      h.p(
        [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
        [`Refs: ${metric.sourceRefs.join(', ')}`],
      ),
    ],
  )
}

const viewDashboard = <Message>(snapshot: EngagementKpiSnapshot): Html => {
  const h = html<Message>()

  return h.main(
    [
      h.AriaLabel('Business KPI dashboard'),
      Ui.className<Message>('mx-auto grid w-[min(100%,1120px)] gap-6 px-4 py-8'),
      h.DataAttribute('business-kpi-dashboard', snapshot.engagementRef),
    ],
    [
      h.section(
        [
          Ui.className<Message>(
            'grid gap-4 border-b border-[#222] pb-6 lg:grid-cols-[minmax(0,1fr)_22rem]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-3')],
            [
              h.p(
                [Ui.className<Message>('m-0 font-mono text-sm text-white/40')],
                ['OpenAgents Business KPI'],
              ),
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[18ch] text-4xl font-medium tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Scorekeeper'],
              ),
              h.p(
                [Ui.className<Message>('m-0 max-w-[70ch] text-base/7 text-white/65')],
                [
                  'Baseline snapshot and live engagement metrics for value-share review. Opaque refs only; this page never exposes customer identity, raw payment material, or provider payloads.',
                ],
              ),
            ],
          ),
          h.aside(
            [
              Ui.className<Message>(
                'grid content-start gap-2 border border-[#222] bg-[#010102] p-4 font-mono text-xs text-white/50',
              ),
            ],
            [
              h.p([Ui.className<Message>('m-0 text-white/75')], [snapshot.engagementRef]),
              h.p([Ui.className<Message>('m-0')], [`Vertical: ${snapshot.vertical}`]),
              h.p([Ui.className<Message>('m-0')], [`Baseline: ${snapshot.baselineCapturedAt}`]),
              h.p([Ui.className<Message>('m-0')], [`Generated: ${snapshot.generatedAt}`]),
              h.p([Ui.className<Message>('m-0')], [`Source: ${snapshot.dataSource}`]),
            ],
          ),
        ],
      ),
      h.section(
        [Ui.className<Message>('grid gap-3 md:grid-cols-2 xl:grid-cols-3')],
        snapshot.metrics.map(metric => metricRow<Message>(metric)),
      ),
      h.section(
        [
          Ui.className<Message>(
            'grid gap-4 border border-[#222] bg-[#010102] p-4 md:grid-cols-2',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid content-start gap-2')],
            [
              h.h2(
                [Ui.className<Message>('m-0 text-base font-medium text-[#f1efe8]')],
                ['Evidence'],
              ),
              h.ul(
                [Ui.className<Message>('m-0 grid gap-1 p-0 font-mono text-xs text-white/50')],
                snapshot.evidenceRefs.map(ref =>
                  h.li([Ui.className<Message>('list-none')], [ref]),
                ),
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid content-start gap-2')],
            [
              h.h2(
                [Ui.className<Message>('m-0 text-base font-medium text-[#f1efe8]')],
                ['Privacy Boundary'],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-sm/6 text-white/60')],
                [
                  `Excluded: ${snapshot.privacyExcludes.join(', ')}. Metric definitions are locked to this dashboard view; settlement and payout claims remain out of scope.`,
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

export const title = (route: BusinessKpiRoute): string =>
  `KPI ${route.engagementRef} - OpenAgents`

export const view = <Message>(
  route: BusinessKpiRoute,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const snapshot: EngagementKpiSnapshot =
    route.engagementRef === dashboard.engagementRef
      ? dashboard
      : { ...dashboard, engagementRef: route.engagementRef }

  return html<Message>().div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), viewDashboard<Message>(snapshot)],
  )
}
