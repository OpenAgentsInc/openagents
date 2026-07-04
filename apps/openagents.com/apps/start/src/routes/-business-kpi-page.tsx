type MetricUnit = 'count' | 'currency' | 'percent'

type KpiMetric = Readonly<{
  key: string
  label: string
  unit: MetricUnit
  baseline: number
  current: number
  definition: string
  sourceRefs: ReadonlyArray<string>
}>

const metrics: ReadonlyArray<KpiMetric> = [
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
] as const

const evidenceRefs = [
  'table:business_funnel_events',
  'endpoint:/api/public/business/funnel-dashboard',
  'issue:8105',
  'roadmap:BF-7.1',
] as const

const privacyExcludes = [
  'client name',
  'contact email',
  'phone',
  'payment payload',
  'raw provider payload',
] as const

const panelClass =
  'grid gap-3 border border-khala-border/80 bg-khala-surface p-4 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

const currencyFormatter = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  maximumFractionDigits: 0,
})

const countFormatter = new Intl.NumberFormat('en-US')

function formatMetricValue(metric: KpiMetric, value: number) {
  if (metric.unit === 'currency') return currencyFormatter.format(value / 100)
  if (metric.unit === 'percent') return `${value.toFixed(1)}%`
  return countFormatter.format(value)
}

function metricDelta(metric: KpiMetric) {
  const delta = metric.current - metric.baseline
  const sign = delta >= 0 ? '+' : ''

  if (metric.unit === 'currency') return `${sign}${formatMetricValue(metric, delta)}`
  if (metric.unit === 'percent') return `${sign}${delta.toFixed(1)} pts`
  return `${sign}${countFormatter.format(delta)}`
}

function MetricCard({ metric }: Readonly<{ metric: KpiMetric }>) {
  return (
    <article className={panelClass} data-business-kpi-metric={metric.key}>
      <div className="flex items-start justify-between gap-3">
        <h3 className="m-0 text-base font-medium text-white">{metric.label}</h3>
        <span className="border border-khala-success/40 bg-khala-success/10 px-2 py-1 font-mono text-xs uppercase text-khala-success">
          Live
        </span>
      </div>
      <div className="grid grid-cols-3 gap-2 font-mono">
        <p className="m-0 grid gap-1 text-xs text-khala-text-faint">
          <span>Baseline</span>
          <strong className="text-lg font-semibold text-khala-text">
            {formatMetricValue(metric, metric.baseline)}
          </strong>
        </p>
        <p className="m-0 grid gap-1 text-xs text-khala-text-faint">
          <span>Current</span>
          <strong className="text-lg font-semibold text-white">
            {formatMetricValue(metric, metric.current)}
          </strong>
        </p>
        <p className="m-0 grid gap-1 text-xs text-khala-text-faint">
          <span>Delta</span>
          <strong className="text-lg font-semibold text-khala-warning">
            {metricDelta(metric)}
          </strong>
        </p>
      </div>
      <p className="m-0 text-base/7 text-khala-text-muted sm:text-sm/6">
        {metric.definition}
      </p>
      <p className="m-0 font-mono text-xs text-khala-text-faint">
        Refs: {metric.sourceRefs.join(', ')}
      </p>
    </article>
  )
}

export function BusinessKpiPage({
  engagementRef,
}: Readonly<{ engagementRef: string }>) {
  return (
    <main
      aria-label="Business KPI dashboard"
      className="min-h-dvh bg-black text-white"
      data-business-kpi-dashboard={engagementRef}
      data-route="business-kpi"
    >
      <div className="mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <section className="grid gap-4 border-b border-khala-border pb-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
          <div className="grid gap-3">
            <a
              className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
              href="/business"
            >
              Business
            </a>
            <p className={eyebrowClass}>OpenAgents Business KPI</p>
            <h1 className="m-0 max-w-[18ch] text-4xl font-medium tracking-normal text-white">
              Scorekeeper
            </h1>
            <p className="m-0 max-w-[70ch] text-pretty text-base/7 text-khala-text-muted">
              Baseline snapshot and live engagement metrics for value-share
              review. Opaque refs only; this page never exposes customer
              identity, raw payment material, or provider payloads.
            </p>
          </div>
          <aside className={panelClass}>
            <p className="m-0 text-white">{engagementRef}</p>
            <p className="m-0">Vertical: service business funnel</p>
            <p className="m-0">Baseline: 2026-07-02T15:00:00.000Z</p>
            <p className="m-0">Generated: 2026-07-03T00:00:00.000Z</p>
            <p className="m-0">Source: /api/public/business/funnel-dashboard</p>
          </aside>
        </section>
        <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {metrics.map(metric => (
            <MetricCard key={metric.key} metric={metric} />
          ))}
        </section>
        <section className="grid gap-4 border border-khala-border bg-khala-surface p-4 md:grid-cols-2">
          <div className="grid content-start gap-2">
            <h2 className="m-0 text-base font-medium text-white">Evidence</h2>
            <ul
              className="m-0 grid gap-1 p-0 font-mono text-xs text-khala-text-faint"
              role="list"
            >
              {evidenceRefs.map(ref => (
                <li className="list-none" key={ref}>
                  {ref}
                </li>
              ))}
            </ul>
          </div>
          <div className="grid content-start gap-2">
            <h2 className="m-0 text-base font-medium text-white">
              Privacy Boundary
            </h2>
            <p className="m-0 text-base/7 text-khala-text-muted sm:text-sm/6">
              Excluded: {privacyExcludes.join(', ')}. Metric definitions are
              locked to this dashboard view; settlement and payout claims
              remain out of scope.
            </p>
          </div>
        </section>
      </div>
    </main>
  )
}
