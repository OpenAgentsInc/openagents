import { InternalLink } from '@/components/internal-link'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useEffect, useState } from 'react'

import {
  type AnalyticsResult,
  type AnalyticsSummary,
  type AnalyticsWindow,
  fetchAnalyticsSummary,
} from './-admin-analytics-fetch'

const numberFormat = new Intl.NumberFormat('en-US')
const windows: ReadonlyArray<AnalyticsWindow> = ['24h', '7d', '30d']
export const ADMIN_ANALYTICS_LOGIN_HREF = '/login?returnTo=%2Fadmin%2Fanalytics'

export const analyticsAuthRedirect = (
  result: AnalyticsResult | null,
): string | undefined =>
  result?.tag === 'unauthorized' ? ADMIN_ANALYTICS_LOGIN_HREF : undefined

export function AnalyticsDashboard({
  summary,
}: Readonly<{ summary: AnalyticsSummary }>) {
  return (
    <main
      className="grid min-h-[calc(100dvh-4.25rem)] content-start gap-5 bg-khala-void px-4 py-6 text-khala-text sm:px-6"
      data-route="admin-analytics"
    >
      <header className="flex flex-wrap items-end justify-between gap-3 border-b border-khala-border pb-4">
        <div className="grid gap-1">
          <p className="m-0 font-mono text-[0.625rem] font-semibold uppercase tracking-[0.14em] text-khala-text-faint">
            OpenAgents · admin
          </p>
          <h1 className="m-0 text-xl font-semibold">Website analytics</h1>
        </div>
        <InternalLink
          className="font-mono text-xs text-khala-energy-cyan underline underline-offset-4"
          href="/admin/operator"
        >
          Operator dashboard
        </InternalLink>
      </header>

      <section className="grid grid-cols-2 gap-3 sm:max-w-xl">
        <Stat label="Page views" value={summary.pageViews} />
        <Stat label="Named events" value={summary.totalNamedEvents} />
      </section>

      <section className="grid gap-4 lg:grid-cols-2">
        <AnalyticsTable
          empty="No page views in this window."
          heading="Top pages"
          rows={summary.topPages.map(row => [
            row.routeId,
            numberFormat.format(row.pageViews),
          ])}
        />
        <AnalyticsTable
          empty="No named events in this window."
          heading="Conversions"
          rows={summary.namedEvents.map(row => [
            row.name,
            numberFormat.format(row.count),
          ])}
        />
      </section>

      <AnalyticsTable
        empty="No daily activity in this window."
        heading="Daily activity"
        rows={summary.daily.map(row => [
          row.day,
          `${numberFormat.format(row.pageViews)} views · ${numberFormat.format(row.namedEvents)} events`,
        ])}
      />
    </main>
  )
}

function Stat({ label, value }: Readonly<{ label: string; value: number }>) {
  return (
    <Card className="rounded-none border-khala-border bg-khala-surface">
      <CardHeader className="pb-2">
        <CardTitle className="text-xs uppercase tracking-wide text-khala-text-faint">
          {label}
        </CardTitle>
      </CardHeader>
      <CardContent className="text-3xl font-semibold tabular-nums">
        {numberFormat.format(value)}
      </CardContent>
    </Card>
  )
}

function AnalyticsTable({
  empty,
  heading,
  rows,
}: Readonly<{
  empty: string
  heading: string
  rows: ReadonlyArray<readonly [string, string]>
}>) {
  return (
    <Card className="rounded-none border-khala-border bg-khala-surface">
      <CardHeader>
        <CardTitle>{heading}</CardTitle>
      </CardHeader>
      <CardContent>
        {rows.length === 0 ? (
          <p className="m-0 text-sm text-khala-text-faint">{empty}</p>
        ) : (
          <dl className="m-0 grid gap-2">
            {rows.map(([label, value]) => (
              <div
                className="flex items-center justify-between gap-4 border-t border-khala-border py-2 first:border-t-0"
                key={label}
              >
                <dt className="truncate font-mono text-xs text-khala-text-muted">
                  {label}
                </dt>
                <dd className="m-0 shrink-0 text-sm tabular-nums">{value}</dd>
              </div>
            ))}
          </dl>
        )}
      </CardContent>
    </Card>
  )
}

export function AdminAnalyticsPage() {
  const [window, setWindow] = useState<AnalyticsWindow>('7d')
  const [result, setResult] = useState<AnalyticsResult | null>(null)

  useEffect(() => {
    let active = true
    void fetchAnalyticsSummary(window).then(next => {
      if (active) setResult(next)
    })
    return () => {
      active = false
    }
  }, [window])

  useEffect(() => {
    const redirect = analyticsAuthRedirect(result)
    if (redirect !== undefined) globalThis.window.location.replace(redirect)
  }, [result])

  if (result === null) {
    return <AnalyticsStatus title="Loading analytics…" />
  }
  if (result.tag === 'unauthorized') {
    return <AnalyticsStatus title="Redirecting to Log In…" />
  }
  if (result.tag === 'forbidden') {
    return <AnalyticsStatus title="This account is not an approved admin." />
  }
  if (result.tag === 'failed') {
    return <AnalyticsStatus title="Analytics are unavailable." />
  }

  return (
    <>
      <div className="fixed right-4 top-[5.25rem] z-10 flex gap-1 sm:right-6">
        {windows.map(value => (
          <button
            aria-pressed={window === value}
            className="border border-khala-border bg-khala-surface px-3 py-2 font-mono text-xs text-khala-text aria-pressed:border-khala-energy-cyan aria-pressed:text-khala-energy-cyan"
            key={value}
            onClick={() => setWindow(value)}
            type="button"
          >
            {value}
          </button>
        ))}
      </div>
      <AnalyticsDashboard summary={result.summary} />
    </>
  )
}

function AnalyticsStatus({ title }: Readonly<{ title: string }>) {
  return (
    <main
      className="grid min-h-[calc(100dvh-4.25rem)] place-items-center bg-khala-void px-4 text-khala-text"
      data-route="admin-analytics"
    >
      <div className="grid max-w-md gap-3 text-center">
        <h1 className="m-0 text-xl font-semibold">{title}</h1>
        <InternalLink
          className="font-mono text-sm text-khala-energy-cyan underline underline-offset-4"
          href={ADMIN_ANALYTICS_LOGIN_HREF}
        >
          Open Log In
        </InternalLink>
      </div>
    </main>
  )
}
