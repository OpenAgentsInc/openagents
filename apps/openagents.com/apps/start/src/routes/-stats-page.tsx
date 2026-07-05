import type * as React from 'react'

import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

type RowTone = 'good' | 'muted' | 'warn'

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint'
const panelTitleClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none text-khala-text'
const panelMetaClass = 'm-0 mt-1 font-mono text-[0.68rem] leading-4 text-khala-text-faint'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-khala-border/60 py-2'
const rowLabelClass = 'min-w-0 font-mono text-xs font-medium leading-4 text-khala-text'
const rowDetailClass = 'mt-1 font-mono text-[0.66rem] leading-4 text-khala-text-faint'
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
        live ? 'bg-khala-energy shadow-[0_0_8px_rgba(58,123,255,0.85)]' : 'bg-khala-text-faint/60'
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
      <div className={`text-right font-mono text-xs leading-4 tabular-nums ${toneClass(tone)}`}>
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
        <span className="block truncate font-mono text-xs leading-4 text-current">{href}</span>
        <span className="block font-mono text-[0.66rem] leading-4 text-khala-text-faint">
          {detail}
        </span>
      </span>
    </a>
  )
}

// The chart shell reused by the tokens-history bar chart and both mix
// panels — mirrors the Foldkit `historyChartShell` helper
// (apps/web/src/page/loggedOut/page/home.ts) that every one of them
// composes with. Every prior TS-6 Start route has stayed static/SSR-only, so
// this renders exactly the Idle first-paint state each Foldkit chart shows
// before its client fetch resolves — no fabricated bars or mix rows.
function ChartShell({
  body,
  caption,
  live,
  metricToggle,
  title,
}: Readonly<{
  body: React.ReactNode
  caption: string
  live: boolean
  metricToggle?: React.ReactNode
  title: string
}>) {
  return (
    <section
      className="flex h-full flex-col gap-3 border border-khala-border bg-khala-surface px-4 py-5"
      data-chart="khala-tokens-served-history"
    >
      <div className="flex flex-wrap items-center justify-between gap-2 font-mono text-xs uppercase leading-none tracking-wide text-khala-text-faint">
        <div className="flex min-w-0 items-center gap-2">
          <StatusDot live={live} />
          <span className="truncate">{title}</span>
        </div>
        {metricToggle ?? null}
      </div>
      {body}
      <p className="m-0 font-mono text-[0.66rem] leading-4 text-khala-text-faint">{caption}</p>
    </section>
  )
}

function ChartPlaceholder({ label }: Readonly<{ label: string }>) {
  return (
    <div
      className="flex h-24 items-center justify-center font-mono text-[0.7rem] text-khala-text-faint"
      role="status"
    >
      {label}
    </div>
  )
}

function HistoryMetricToggle() {
  return (
    <div
      aria-label="Tokens served chart metric"
      className="inline-flex overflow-hidden border border-khala-border bg-black/50 font-mono text-[0.58rem] font-semibold uppercase leading-none tracking-wide"
      role="group"
    >
      <button
        aria-pressed="true"
        className="cursor-pointer bg-khala-surface-raised px-2 py-1.5 text-khala-text"
        type="button"
      >
        Daily
      </button>
      <button
        aria-pressed="false"
        className="cursor-pointer px-2 py-1.5 text-khala-text-faint hover:bg-black/40 hover:text-khala-energy-soft"
        type="button"
      >
        Cumulative
      </button>
    </div>
  )
}

const endpointRows = [
  {
    detail: 'Capability manifest for agents and operators.',
    href: '/.well-known/openagents.json',
    method: 'GET',
  },
  { detail: 'OpenAPI route contract.', href: '/api/openapi.json', method: 'GET' },
  {
    detail: 'Pylon heartbeat and receipt-gated accepted-work counters.',
    href: '/api/public/pylon-stats',
    method: 'GET',
  },
  {
    detail: 'Versioned promise states: live, scoped, gated, degraded, and planned.',
    href: '/api/public/product-promises',
    method: 'GET',
  },
  {
    detail: 'Public tip paid and settled evidence rows.',
    href: '/api/forum/tip-leaderboards',
    method: 'GET',
  },
  {
    detail: 'Forum posting and tipping launch gates.',
    href: '/api/forum/launch-status',
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

// `openagents.com/stats` — public/anonymous variant only. The same URL also
// has a distinct authenticated `loggedIn/page/stats.ts` view (real account
// dashboards, private settlement detail); that view is out of scope here,
// same "public-safe default until Start has real session auth" treatment as
// `/artanis/accounts`.
//
// The Foldkit original (apps/web/src/page/loggedOut/page/stats.ts) composes
// nine shared panel functions from home.ts, every one of them fed by a
// client-fetched model union (Idle / Loading / Loaded / Failed against
// /api/public/pylon-stats, /api/forum/tip-leaderboards,
// /api/forum/launch-status, and the Khala-tokens-served counter/history/mix
// endpoints). No prior TS-6 Start route has wired a live client fetch on a
// standalone page, so this port renders exactly the Idle first-paint state
// every one of those nine panels already shows before its fetch resolves —
// the hero counter's `—` placeholder, the history chart's "Waiting for
// data…" body, both mix panels' "Waiting for … mix…" bodies, and every
// metric row's "Unavailable" value — rather than fabricating counters,
// chart bars, or mix rows. The always-static panels (Claim Boundary,
// Endpoint Manifest) are ported in full since none of their content is
// behind a fetch in the Foldkit original.
export function StatsPage() {
  return (
    <main className="min-h-dvh bg-black text-khala-text" data-route="stats">
      <div className="mx-auto grid w-full max-w-7xl gap-3 px-3 py-4 font-mono sm:px-4 lg:px-6">
        <div className="flex">
          <a
            className="khala-focus inline-flex items-center border border-khala-border bg-khala-surface px-2.5 py-2 font-mono text-[0.65rem] uppercase leading-none text-khala-text-faint hover:border-khala-border-strong hover:text-khala-text"
            href="/"
          >
            Home
          </a>
        </div>

        <Card className="khala-panel grid gap-4 border-khala-border bg-khala-surface p-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-stretch">
          <div className="flex min-w-0 flex-col justify-center">
            <h1 className="m-0 text-[1.4rem] font-semibold leading-[1.1] text-khala-text">
              Network Stats
            </h1>
            <p className="m-0 mt-1.5 max-w-[64ch] text-[0.74rem] leading-5 text-khala-text-muted">
              Live public-safe evidence: receipt-backed counters, launch gates,
              and claim boundaries. No dummy values; missing evidence is
              marked unavailable.
            </p>
          </div>
          <section
            className="flex min-w-[15rem] flex-col justify-center gap-2 border border-khala-border bg-black/40 px-4 py-3 text-left sm:h-full sm:items-end sm:text-right"
            data-counter="khala-tokens-served"
          >
            <div className="flex items-center gap-2 font-mono text-[0.62rem] uppercase leading-none tracking-wide text-khala-energy-soft sm:justify-end">
              <StatusDot live={false} />
              <span>Tokens Served</span>
            </div>
            <p className="m-0 w-full min-w-0 max-w-full text-[1.28rem] font-semibold leading-none tabular-nums text-khala-text sm:text-[1.42rem]">
              <span
                className="block w-full max-w-full whitespace-nowrap"
                data-counter-display="khala-tokens-served"
                data-value="—"
              >
                —
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
            body={<ChartPlaceholder label="Waiting for data…" />}
            caption="Daily all-demand input + output tokens served across the network in America/Chicago."
            live={false}
            metricToggle={<HistoryMetricToggle />}
            title="Tokens Served / Day"
          />
          <section className="grid content-start gap-3">
            <ChartShell
              body={<ChartPlaceholder label="Waiting for model mix…" />}
              caption="Canonical model-family mix from all real aggregate token usage rows."
              live={false}
              title="Model Family Mix"
            />
            <ChartShell
              body={<ChartPlaceholder label="Waiting for channel mix…" />}
              caption="Product-wide channel mix from the canonical token usage ledger."
              live={false}
              title="Channel Mix"
            />
          </section>
        </div>

        <div className="grid gap-3 xl:grid-cols-2">
          <section className="grid content-start gap-3">
            <Card className={panelClass} data-stats-pylon-panel="">
              <PanelHeader meta="Heartbeat freshness unavailable." status="Idle" title="Pylon Stats" />
              <MetricRow
                detail="Heartbeat window. Not payment or earning evidence."
                label="Online now"
                tone="muted"
                value="Unavailable"
              />
              <MetricRow detail="Seen in the last 24 hours." label="Seen 24h" value="Unavailable" />
              <MetricRow
                detail="Pylons idle and waiting for assignments."
                label="Assigned now"
                value="Unavailable"
              />
              <MetricRow
                detail="Earning copy gate not loaded."
                label="Earning gate"
                tone="muted"
                value="Unavailable"
              />
            </Card>

            <Card className={panelClass} data-stats-forum-panel="">
              <PanelHeader
                meta="Tip rows separate payer-side payment evidence from creator settlement."
                status="Partial"
                title="Forum Stats"
              />
              <MetricRow
                detail="Payer-side payment evidence only; shown top creator rows."
                label="Tip sats paid"
                tone="muted"
                value="Unavailable"
              />
              <MetricRow
                detail="Creator settlement evidence only; not inferred from payment."
                label="Tip sats settled"
                tone="muted"
                value="Unavailable"
              />
              <MetricRow
                detail="Paid sats not yet settlement-backed in shown rows."
                label="Settlement gap"
                tone="muted"
                value="Unavailable"
              />
              <MetricRow
                detail="Forum tip gate not loaded."
                label="Tip gate"
                tone="warn"
                value="Unavailable"
              />
              <MetricRow
                detail="Endpoint returned creator rows, not global forum totals."
                label="Tip count"
                value="Unavailable"
              />
              <MetricRow
                detail="Active \ orange check badges bought by registered agents. Participation signal, not identity verification."
                label="Orange checks sold"
                tone="muted"
                value="Unavailable"
              />
            </Card>

            <Card className={panelClass} data-stats-accounting-panel="">
              <PanelHeader
                meta="No dummy money values. Missing public-safe evidence is marked unavailable."
                status="Evidence"
                title="Accounting Strip"
              />
              <MetricRow
                detail="Receipt-backed Nexus/Treasury accepted-work payout evidence only."
                label="Accepted-work sats paid"
                tone="muted"
                value="Unavailable"
              />
              <MetricRow
                detail="Accepted-work gate unavailable."
                label="Accepted-work gate"
                tone="warn"
                value="Unavailable"
              />
              <MetricRow
                detail="Settled receipt references, not a sats amount."
                label="Settlement refs"
                value="Unavailable"
              />
              <MetricRow
                detail="Asset-bound ledger projection. Not a withdrawal promise or settled payout."
                label="Revshare"
                value="Unavailable"
              />
              <MetricRow
                detail="Forum paid minus settled sats in shown leaderboard rows."
                label="Forum paid vs settled"
                value="Unavailable"
              />
            </Card>
          </section>

          <section className="grid content-start gap-3">
            <Card className={panelClass} data-stats-copy-boundary-panel="">
              <PanelHeader meta="Public copy boundaries for money and earning claims." title="Claim Boundary" />
              {copyBoundaryRows.map(row => (
                <MetricRow detail={row.detail} key={row.label} label={row.label} value="Bounded" />
              ))}
            </Card>

            <Card className={panelClass} data-stats-endpoint-manifest-panel="">
              <PanelHeader
                meta="Public reads are no-store. Mutations require the authority named by the route."
                status="Public"
                title="Endpoint Manifest"
              />
              {endpointRows.map(row => (
                <EndpointRow detail={row.detail} href={row.href} key={row.href} method={row.method} />
              ))}
            </Card>

            <Card className={panelClass} data-stats-nostr-relay-panel="">
              <PanelHeader
                meta="Pylon registrations may publish relay URLs and short public keys."
                status="Idle"
                title="Nostr Relay Configuration"
              />
              <MetricRow
                detail="Hosted relay plus registered Pylon relays."
                label="Relay URLs"
                value="Unavailable"
              />
              <MetricRow
                detail="Short public-key labels from recent Pylon registrations."
                label="Pubkeys"
                value="Unavailable"
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
            Live counters, chart bars, mix rows, and evidence panels stay on
            the existing Foldkit page until this route carries a real fetch
            against the endpoints below.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
            <div>
              <span className="block text-khala-text-faint">Pylon stats</span>
              <code className={codeClass}>/api/public/pylon-stats</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Tip leaderboards</span>
              <code className={codeClass}>/api/forum/tip-leaderboards</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Forum launch status</span>
              <code className={codeClass}>/api/forum/launch-status</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Product promises</span>
              <code className={codeClass}>/api/public/product-promises</code>
            </div>
          </div>
        </Card>
      </div>
    </main>
  )
}
