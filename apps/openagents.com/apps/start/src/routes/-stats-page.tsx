import { PublicPageShell } from '@/components/public-page-shell'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import type * as React from 'react'
import { useEffect, useState } from 'react'

import {
  FORUM_LAUNCH_STATUS_URL,
  type ForumLaunchStatusSnapshot,
  type HistoryMetric,
  type Loadable,
  type MixSnapshot,
  STATS_PYLON_STATS_URL,
  type StatsPylonSnapshot,
  TOKENS_SERVED_CHANNEL_MIX_URL,
  TOKENS_SERVED_HISTORY_URL,
  TOKENS_SERVED_MODEL_MIX_URL,
  TOKENS_SERVED_URL,
  type TokensServedHistorySnapshot,
  type TokensServedSnapshot,
  accountingPanelValues,
  fetchPublicJson,
  forumPanelValues,
  historyBars,
  mixRows,
  nostrPanelValues,
  pylonPanelValues,
  toLoadable,
  tokensServedDisplay,
} from './-stats-data'

type RowTone = 'good' | 'muted' | 'warn'

const eyebrowClass =
  'm-0 text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint'
const panelTitleClass =
  'm-0 text-xs font-semibold uppercase leading-none text-khala-text'
const panelMetaClass = 'm-0 mt-1 text-xs leading-4 text-khala-text-faint'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-khala-border/60 py-2'
const rowLabelClass = 'min-w-0 text-xs font-medium leading-4 text-khala-text'
const rowDetailClass = 'mt-1 text-xs leading-4 text-khala-text-faint'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'
const panelClass = 'min-w-0 border-khala-border bg-khala-surface p-3 text-left'

const toneClass = (tone: RowTone = 'muted'): string =>
  tone === 'good'
    ? 'text-khala-energy-soft'
    : tone === 'warn'
      ? 'text-khala-warning'
      : 'text-khala-text-faint'

function StatusDot({ live }: Readonly<{ live: boolean }>) {
  return (
    <span
      className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${
        live
          ? 'bg-khala-energy shadow-[0_0_8px_rgba(58,123,255,0.85)]'
          : 'bg-khala-text-faint/60'
      }`}
      data-status={live ? 'live' : 'pending'}
    />
  )
}

function PanelHeader({
  meta,
  status,
  title,
}: Readonly<{ meta?: string; status?: string; title: string }>) {
  return (
    <div className="mb-2 flex min-w-0 flex-wrap items-start justify-between gap-2">
      <div className="min-w-0">
        <h2 className={panelTitleClass}>{title}</h2>
        {meta === undefined ? null : <p className={panelMetaClass}>{meta}</p>}
      </div>
      {status === undefined ? null : <Badge>{status}</Badge>}
    </div>
  )
}

function MetricRow({
  detail,
  label,
  tone,
  value,
}: Readonly<{ detail: string; label: string; tone?: RowTone; value: string }>) {
  return (
    <div className={rowClass}>
      <div className="min-w-0">
        <div className={rowLabelClass}>{label}</div>
        <div className={rowDetailClass}>{detail}</div>
      </div>
      <div
        className={`text-right font-mono text-xs leading-4 tabular-nums ${toneClass(tone)}`}
      >
        {value}
      </div>
    </div>
  )
}

function EndpointRow({
  detail,
  href,
  method,
}: Readonly<{ detail: string; href: string; method: string }>) {
  return (
    <a
      className="grid grid-cols-[3rem_minmax(0,1fr)] gap-2 border-t border-khala-border/60 py-2 text-khala-text-faint hover:text-khala-text"
      href={href}
    >
      <span className="font-mono text-[0.65rem] font-semibold uppercase leading-4 text-khala-text-faint">
        {method}
      </span>
      <span className="min-w-0">
        <span className="block truncate font-mono text-xs leading-4 text-current">
          {href}
        </span>
        <span className="block text-xs leading-4 text-khala-text-faint">
          {detail}
        </span>
      </span>
    </a>
  )
}

// The chart shell reused by the tokens-history bar chart and both mix
// panels — mirrors the Foldkit `historyChartShell` helper the retired
// `apps/web` page composed with.
function ChartShell({
  body,
  caption,
  chartId,
  live,
  metricToggle,
  title,
}: Readonly<{
  body: React.ReactNode
  caption: string
  chartId: string
  live: boolean
  metricToggle?: React.ReactNode
  title: string
}>) {
  return (
    <section
      className="flex h-full flex-col gap-3 border border-khala-border bg-khala-surface px-4 py-5"
      data-chart={chartId}
    >
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint">
        <div className="flex min-w-0 items-center gap-2">
          <StatusDot live={live} />
          <span className="truncate">{title}</span>
        </div>
        {metricToggle ?? null}
      </div>
      {body}
      <p className="m-0 text-xs leading-4 text-khala-text-faint">{caption}</p>
    </section>
  )
}

function ChartPlaceholder({ label }: Readonly<{ label: string }>) {
  return (
    <div
      className="flex h-24 items-center justify-center text-xs text-khala-text-faint"
      role="status"
    >
      {label}
    </div>
  )
}

function HistoryMetricToggle({
  metric,
  onSelect,
}: Readonly<{
  metric: HistoryMetric
  onSelect: (metric: HistoryMetric) => void
}>) {
  const buttonClass = (active: boolean): string =>
    active
      ? 'cursor-pointer bg-khala-surface-raised px-2 py-1.5 text-khala-text'
      : 'cursor-pointer px-2 py-1.5 text-khala-text-faint hover:bg-black/40 hover:text-khala-energy-soft'
  return (
    <div
      aria-label="Tokens served chart metric"
      className="inline-flex overflow-hidden border border-khala-border bg-black/50 text-xs font-semibold uppercase leading-none tracking-wide"
      role="group"
    >
      <button
        aria-pressed={metric === 'daily'}
        className={buttonClass(metric === 'daily')}
        onClick={() => onSelect('daily')}
        type="button"
      >
        Daily
      </button>
      <button
        aria-pressed={metric === 'cumulative'}
        className={buttonClass(metric === 'cumulative')}
        onClick={() => onSelect('cumulative')}
        type="button"
      >
        Cumulative
      </button>
    </div>
  )
}

function HistoryChartBody({
  metric,
  snapshot,
}: Readonly<{ metric: HistoryMetric; snapshot: TokensServedHistorySnapshot }>) {
  const bars = historyBars(snapshot, metric)
  if (bars.length === 0) {
    return <ChartPlaceholder label="No history rows in the current window." />
  }
  return (
    <div
      aria-label={`Tokens served per day, ${bars.length} days`}
      className="flex h-24 items-end gap-px"
      data-history-metric={metric}
      role="img"
    >
      {bars.map(bar => (
        <div
          className="min-w-0 flex-1 bg-khala-energy/70"
          data-history-day={bar.day}
          key={bar.day}
          style={{
            height: `${bar.heightPct}%`,
            minHeight: bar.tokens > 0 ? '1px' : '0',
          }}
          title={`${bar.day}: ${bar.tokens.toLocaleString('en-US')} tokens`}
        />
      ))}
    </div>
  )
}

function MixBody({
  emptyLabel,
  snapshot,
}: Readonly<{ emptyLabel: string; snapshot: MixSnapshot }>) {
  const rows = mixRows(snapshot)
  if (rows.length === 0) {
    return <ChartPlaceholder label={emptyLabel} />
  }
  return (
    <div className="grid content-start">
      {rows.map(row => (
        <div className={rowClass} key={row.label}>
          <div className="min-w-0">
            <div className={rowLabelClass}>{row.label}</div>
            <div className={rowDetailClass}>{row.detail}</div>
          </div>
          <div className="text-right font-mono text-xs leading-4 tabular-nums text-khala-energy-soft">
            {row.pct}
          </div>
        </div>
      ))}
    </div>
  )
}

const endpointRows = [
  {
    detail: 'Capability manifest for agents and operators.',
    href: '/.well-known/openagents.json',
    method: 'GET',
  },
  {
    detail: 'OpenAPI route contract.',
    href: '/api/openapi.json',
    method: 'GET',
  },
  {
    detail: 'Pylon heartbeat and receipt-gated accepted-work counters.',
    href: '/api/public/pylon-stats',
    method: 'GET',
  },
  {
    detail:
      'Versioned promise states: live, scoped, gated, degraded, and planned.',
    href: '/api/public/product-promises',
    method: 'GET',
  },
  {
    detail: 'Public Autopilot activity projection.',
    href: '/api/public/adjutant/activity',
    method: 'GET',
  },
] as const

const copyBoundaryRows = [
  { detail: 'Payer-side payment evidence only.', label: 'Tip sats paid' },
  { detail: 'Creator settlement evidence only.', label: 'Tip sats settled' },
  {
    detail: 'Receipt-backed Nexus/Treasury accepted-work payout evidence.',
    label: 'Accepted-work sats paid',
  },
  {
    detail: 'Asset-bound ledger projection, not a withdrawal promise.',
    label: 'Revshare',
  },
  { detail: 'Paid sats not yet settlement-backed.', label: 'Settlement gap' },
] as const

const UNAVAILABLE = 'Unavailable'

const loadableStatus = (state: Loadable<unknown>['state']): string =>
  state === 'ok' ? 'Live' : state === 'loading' ? 'Idle' : 'Unavailable'

// `openagents.com/stats` — public/anonymous variant only. The same URL also
// had a distinct authenticated Foldkit view (real account dashboards, private
// settlement detail); that view stays out of scope until Start has real
// session auth, same treatment as `/artanis/accounts`.
//
// Live-fetch wiring: on the client this route fetches the public no-auth
// endpoints listed in `-stats-data.ts` (tokens-served counter/history/mixes,
// pylon-stats, forum launch-status) and renders their real values. Server
// render and the pre-fetch first paint keep the honest idle placeholders,
// and any failed fetch degrades to the same Unavailable state — no
// fabricated counters, bars, or mix rows. Forum tip totals and revshare stay
// permanently Unavailable: `/api/forum/tip-leaderboards` is retired (HTTP 410
// `money_surface_retired`, 2026-07-14) and no public revshare projection
// endpoint exists.
export function StatsPage() {
  const [tokensServed, setTokensServed] = useState<
    Loadable<TokensServedSnapshot>
  >({
    state: 'loading',
  })
  const [history, setHistory] = useState<Loadable<TokensServedHistorySnapshot>>(
    {
      state: 'loading',
    },
  )
  const [modelMix, setModelMix] = useState<Loadable<MixSnapshot>>({
    state: 'loading',
  })
  const [channelMix, setChannelMix] = useState<Loadable<MixSnapshot>>({
    state: 'loading',
  })
  const [pylonStats, setPylonStats] = useState<Loadable<StatsPylonSnapshot>>({
    state: 'loading',
  })
  const [forumLaunch, setForumLaunch] = useState<
    Loadable<ForumLaunchStatusSnapshot>
  >({
    state: 'loading',
  })
  const [historyMetric, setHistoryMetric] = useState<HistoryMetric>('daily')

  useEffect(() => {
    let cancelled = false
    const guard =
      <T,>(set: (next: Loadable<T>) => void) =>
      (data: T | null): void => {
        if (!cancelled) set(toLoadable(data))
      }
    const pollCounter = async (): Promise<void> => {
      guard(setTokensServed)(
        await fetchPublicJson<TokensServedSnapshot>(TOKENS_SERVED_URL),
      )
    }
    void pollCounter()
    void fetchPublicJson<TokensServedHistorySnapshot>(
      TOKENS_SERVED_HISTORY_URL,
    ).then(guard(setHistory))
    void fetchPublicJson<MixSnapshot>(TOKENS_SERVED_MODEL_MIX_URL).then(
      guard(setModelMix),
    )
    void fetchPublicJson<MixSnapshot>(TOKENS_SERVED_CHANNEL_MIX_URL).then(
      guard(setChannelMix),
    )
    void fetchPublicJson<StatsPylonSnapshot>(STATS_PYLON_STATS_URL).then(
      guard(setPylonStats),
    )
    void fetchPublicJson<ForumLaunchStatusSnapshot>(
      FORUM_LAUNCH_STATUS_URL,
    ).then(guard(setForumLaunch))
    const counterTimer = setInterval(() => void pollCounter(), 20000)
    return () => {
      cancelled = true
      clearInterval(counterTimer)
    }
  }, [])

  const counter = tokensServedDisplay(tokensServed)
  const pylon =
    pylonStats.state === 'ok' ? pylonPanelValues(pylonStats.data) : null
  const accounting =
    pylonStats.state === 'ok' ? accountingPanelValues(pylonStats.data) : null
  const nostr =
    pylonStats.state === 'ok' ? nostrPanelValues(pylonStats.data) : null
  const forum =
    forumLaunch.state === 'ok' ? forumPanelValues(forumLaunch.data) : null

  return (
    <PublicPageShell dataRoute="stats">
      <main
        aria-label="Network Stats"
        className="min-h-dvh bg-khala-void text-khala-text"
      >
        <div className="mx-auto grid w-full max-w-7xl gap-3 px-3 py-8 sm:px-4 lg:px-6">
          <Card className="khala-panel grid gap-4 border-khala-border bg-khala-surface p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-stretch">
            <div className="flex min-w-0 flex-col justify-center">
              <h1 className="m-0 text-[1.4rem] font-semibold leading-[1.1] text-khala-text">
                Network Stats
              </h1>
              <p className="m-0 mt-1.5 max-w-[64ch] text-[0.74rem] leading-5 text-khala-text-muted">
                Live public-safe evidence: receipt-backed counters, launch
                gates, and claim boundaries. No dummy values; missing evidence
                is marked unavailable.
              </p>
            </div>
            <section
              className="flex min-w-[15rem] flex-col justify-center gap-2 border border-khala-border bg-black/40 px-4 py-3 text-left sm:h-full sm:items-end sm:text-right"
              data-counter="khala-tokens-served"
            >
              <div className="flex items-center gap-2 text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft sm:justify-end">
                <StatusDot live={counter.live} />
                <span>Tokens Served</span>
              </div>
              <p className="m-0 w-full min-w-0 max-w-full text-[1.28rem] font-semibold leading-none tabular-nums text-khala-text sm:text-[1.42rem]">
                <span
                  className="block w-full max-w-full whitespace-nowrap"
                  data-counter-display="khala-tokens-served"
                  data-status={counter.live ? 'ok' : tokensServed.state}
                  data-value={counter.value}
                >
                  {counter.value}
                </span>
              </p>
              <p className="m-0 text-[0.66rem] leading-4 text-khala-text-faint">
                All real input + output tokens served across OpenAgents
                products, including Khala API and opted-in direct local Codex
                usage.
              </p>
            </section>
          </Card>

          <div className="grid gap-3 lg:grid-cols-[minmax(0,1.75fr)_minmax(18rem,0.85fr)]">
            <ChartShell
              body={
                history.state === 'ok' ? (
                  <HistoryChartBody
                    metric={historyMetric}
                    snapshot={history.data}
                  />
                ) : (
                  <ChartPlaceholder
                    label={
                      history.state === 'loading'
                        ? 'Waiting for data…'
                        : 'History unavailable.'
                    }
                  />
                )
              }
              caption="Daily all-demand input + output tokens served across the network in America/Chicago."
              chartId="khala-tokens-served-history"
              live={history.state === 'ok'}
              metricToggle={
                <HistoryMetricToggle
                  metric={historyMetric}
                  onSelect={setHistoryMetric}
                />
              }
              title="Tokens Served / Day"
            />
            <section className="grid content-start gap-3">
              <ChartShell
                body={
                  modelMix.state === 'ok' ? (
                    <MixBody
                      emptyLabel="No model mix rows in the current window."
                      snapshot={modelMix.data}
                    />
                  ) : (
                    <ChartPlaceholder
                      label={
                        modelMix.state === 'loading'
                          ? 'Waiting for model mix…'
                          : 'Model mix unavailable.'
                      }
                    />
                  )
                }
                caption="Canonical model-family mix from all real aggregate token usage rows."
                chartId="khala-tokens-served-model-mix"
                live={modelMix.state === 'ok'}
                title="Model Family Mix"
              />
              <ChartShell
                body={
                  channelMix.state === 'ok' ? (
                    <MixBody
                      emptyLabel="No channel mix rows in the current window."
                      snapshot={channelMix.data}
                    />
                  ) : (
                    <ChartPlaceholder
                      label={
                        channelMix.state === 'loading'
                          ? 'Waiting for channel mix…'
                          : 'Channel mix unavailable.'
                      }
                    />
                  )
                }
                caption="Product-wide channel mix from the canonical token usage ledger."
                chartId="khala-tokens-served-channel-mix"
                live={channelMix.state === 'ok'}
                title="Channel Mix"
              />
            </section>
          </div>

          <div className="grid gap-3 xl:grid-cols-2">
            <section className="grid content-start gap-3">
              <Card
                className={panelClass}
                data-stats-pylon-panel={pylonStats.state}
              >
                <PanelHeader
                  meta={pylon?.meta ?? 'Heartbeat freshness unavailable.'}
                  status={loadableStatus(pylonStats.state)}
                  title="Pylon Stats"
                />
                <MetricRow
                  detail="Heartbeat window. Not payment or earning evidence."
                  label="Online now"
                  tone={pylon === null ? 'muted' : 'good'}
                  value={pylon?.onlineNow ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Seen in the last 24 hours."
                  label="Seen 24h"
                  value={pylon?.seen24h ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Pylons ready to accept assignments now."
                  label="Assigned now"
                  value={pylon?.assignedNow ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Public earning copy gate from the pylon-stats projection."
                  label="Earning gate"
                  tone={pylon?.earningGateReady === true ? 'good' : 'muted'}
                  value={pylon?.earningGate ?? UNAVAILABLE}
                />
              </Card>

              <Card
                className={panelClass}
                data-stats-forum-panel={forumLaunch.state}
              >
                <PanelHeader
                  meta={
                    forum?.meta ??
                    'Tip rows separate payer-side payment evidence from creator settlement.'
                  }
                  status={loadableStatus(forumLaunch.state)}
                  title="Forum Stats"
                />
                <MetricRow
                  detail="Forum tip endpoints are retired (money_surface_retired); no public tip totals."
                  label="Tip sats paid"
                  tone="muted"
                  value={UNAVAILABLE}
                />
                <MetricRow
                  detail="Forum tip endpoints are retired; settlement evidence is not published."
                  label="Tip sats settled"
                  tone="muted"
                  value={UNAVAILABLE}
                />
                <MetricRow
                  detail="No public tip rows exist to compute a settlement gap from."
                  label="Settlement gap"
                  tone="muted"
                  value={UNAVAILABLE}
                />
                <MetricRow
                  detail="Public tipping launch gates from the forum launch-status projection."
                  label="Tip gate"
                  tone={forum?.tipGateReady === true ? 'good' : 'warn'}
                  value={forum?.tipGate ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Retired tip endpoints publish no global tip count."
                  label="Tip count"
                  value={UNAVAILABLE}
                />
                <MetricRow
                  detail="Active orange check badges bought by registered agents. Participation signal, not identity verification."
                  label="Orange checks sold"
                  tone="muted"
                  value={forum?.orangeChecksSold ?? UNAVAILABLE}
                />
              </Card>

              <Card
                className={panelClass}
                data-stats-accounting-panel={pylonStats.state}
              >
                <PanelHeader
                  meta="No dummy money values. Missing public-safe evidence is marked unavailable."
                  status="Evidence"
                  title="Accounting Strip"
                />
                <MetricRow
                  detail="Receipt-backed Nexus/Treasury accepted-work payout evidence only."
                  label="Accepted-work sats paid"
                  tone={accounting === null ? 'muted' : 'good'}
                  value={accounting?.acceptedWorkSatsPaid ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Accepted-work settlement gate from the pylon-stats projection."
                  label="Accepted-work gate"
                  tone={
                    accounting?.acceptedWorkGateReady === true ? 'good' : 'warn'
                  }
                  value={accounting?.acceptedWorkGate ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Settled receipt references, not a sats amount."
                  label="Settlement refs"
                  value={accounting?.settlementRefs ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Asset-bound ledger projection. Not a withdrawal promise or settled payout. No public endpoint publishes it."
                  label="Revshare"
                  value={UNAVAILABLE}
                />
                <MetricRow
                  detail="Forum tip endpoints are retired; no paid-versus-settled rows exist."
                  label="Forum paid vs settled"
                  value={UNAVAILABLE}
                />
              </Card>
            </section>

            <section className="grid content-start gap-3">
              <Card className={panelClass} data-stats-copy-boundary-panel="">
                <PanelHeader
                  meta="Public copy boundaries for money and earning claims."
                  title="Claim Boundary"
                />
                {copyBoundaryRows.map(row => (
                  <MetricRow
                    detail={row.detail}
                    key={row.label}
                    label={row.label}
                    value="Bounded"
                  />
                ))}
              </Card>

              <Card
                className={panelClass}
                data-stats-endpoint-manifest-panel=""
              >
                <PanelHeader
                  meta="Public reads are no-store. Mutations require the authority named by the route."
                  status="Public"
                  title="Endpoint Manifest"
                />
                {endpointRows.map(row => (
                  <EndpointRow
                    detail={row.detail}
                    href={row.href}
                    key={row.href}
                    method={row.method}
                  />
                ))}
              </Card>

              <Card
                className={panelClass}
                data-stats-nostr-relay-panel={pylonStats.state}
              >
                <PanelHeader
                  meta="Pylon registrations may publish relay URLs and short public keys."
                  status={loadableStatus(pylonStats.state)}
                  title="Nostr Relay Configuration"
                />
                <MetricRow
                  detail="Hosted relay plus registered Pylon relays."
                  label="Relay URLs"
                  value={nostr?.relayUrls ?? UNAVAILABLE}
                />
                <MetricRow
                  detail="Short public-key labels from recent Pylon registrations."
                  label="Pubkeys"
                  value={nostr?.pubkeys ?? UNAVAILABLE}
                />
                <div className="border-t border-khala-border/60 pt-2">
                  <p className={panelMetaClass}>
                    No relay endpoint list is public in the current response.
                  </p>
                </div>
              </Card>
            </section>
          </div>

          <Card className="grid gap-3 border-khala-border bg-khala-surface p-4 sm:p-5">
            <p className={eyebrowClass}>Live surface</p>
            <p className="m-0 max-w-3xl text-sm/6 text-khala-text-muted">
              This route fetches live public data on the client: the Tokens
              Served counter, daily token history, model-family and channel
              mixes, Pylon counters, accepted-work settlement evidence, and
              Forum launch gates. Forum tip totals and revshare stay unavailable
              — the public tip endpoints are retired and no public revshare
              projection exists. Failed fetches degrade to the same unavailable
              state, never a dummy value.
            </p>
            <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
              <div>
                <span className="block text-khala-text-faint">
                  Tokens served
                </span>
                <code className={codeClass}>
                  /api/public/khala-tokens-served
                </code>
              </div>
              <div>
                <span className="block text-khala-text-faint">
                  History and mixes
                </span>
                <code className={codeClass}>
                  /api/public/khala-tokens-served/history
                </code>
              </div>
              <div>
                <span className="block text-khala-text-faint">Pylon stats</span>
                <code className={codeClass}>/api/public/pylon-stats</code>
              </div>
              <div>
                <span className="block text-khala-text-faint">
                  Forum launch status
                </span>
                <code className={codeClass}>/api/forum/launch-status</code>
              </div>
            </div>
          </Card>
        </div>
      </main>
    </PublicPageShell>
  )
}
