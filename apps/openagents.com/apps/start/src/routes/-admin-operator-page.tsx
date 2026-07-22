import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

import {
  fetchFleetRuns,
  fetchFullAutoRuns,
  fetchOperatorOverview,
  fetchOpsHealth,
  type AgentChain,
  type OverviewResult,
  type OverviewSnapshot,
  type SideResult,
} from './-admin-operator-fetch'

const REFRESH_MS = 12_000

// ---------------------------------------------------------------------------
// Small presentation helpers
// ---------------------------------------------------------------------------

const numberFormat = new Intl.NumberFormat('en-US')
const fmt = (value: number): string => numberFormat.format(value)

const relativeTime = (iso: string | null | undefined, nowMs: number): string => {
  if (iso === null || iso === undefined) return '—'
  const then = Date.parse(iso)
  if (!Number.isFinite(then)) return iso
  const deltaSec = Math.round((nowMs - then) / 1000)
  if (deltaSec < 0) return 'in the future'
  if (deltaSec < 60) return `${deltaSec}s ago`
  if (deltaSec < 3600) return `${Math.floor(deltaSec / 60)}m ago`
  if (deltaSec < 86400) return `${Math.floor(deltaSec / 3600)}h ago`
  return `${Math.floor(deltaSec / 86400)}d ago`
}

const stateVariant = (
  state: string,
): 'ready' | 'running' | 'warning' | 'danger' | 'default' => {
  switch (state) {
    case 'running':
    case 'accepted':
    case 'offered':
      return 'running'
    case 'proof_submitted':
    case 'closeout_submitted':
    case 'accepted_work':
      return 'ready'
    case 'blocked':
    case 'stale':
      return 'warning'
    case 'rejected':
    case 'quarantined':
      return 'danger'
    default:
      return 'default'
  }
}

const labelClass =
  'm-0 font-mono text-[0.625rem] font-semibold uppercase leading-none tracking-[0.14em] text-khala-text-faint'

function Stat({
  label,
  value,
  detail,
}: Readonly<{ label: string; value: string; detail?: string }>) {
  return (
    <div className="grid min-h-24 content-between gap-2 border border-khala-border bg-khala-surface p-3">
      <div className="truncate text-[0.625rem] uppercase tracking-wide text-khala-text-faint">
        {label}
      </div>
      <div className="text-2xl font-semibold leading-none tabular-nums text-khala-text">
        {value}
      </div>
      {detail === undefined ? null : (
        <div className="truncate text-[0.6875rem] text-khala-text-faint">
          {detail}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

function AgentChainRow({
  chain,
  nowMs,
}: Readonly<{ chain: AgentChain; nowMs: number }>) {
  const objective =
    chain.projection !== null &&
    typeof chain.projection === 'object' &&
    'objective' in (chain.projection as Record<string, unknown>)
      ? String((chain.projection as Record<string, unknown>).objective)
      : undefined

  return (
    <div
      className="grid gap-2 border border-khala-border bg-khala-void p-3"
      data-chain-state={chain.state}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Badge variant={stateVariant(chain.state)}>{chain.state}</Badge>
          <span className="truncate font-mono text-xs text-khala-text-muted">
            {chain.jobKind}
          </span>
        </div>
        <span className="shrink-0 text-[0.6875rem] text-khala-text-faint">
          {relativeTime(chain.updatedAt, nowMs)}
        </span>
      </div>
      <div className="truncate font-mono text-[0.6875rem] text-khala-text-faint">
        {chain.assignmentRef} · {chain.pylonRef}
      </div>
      {objective === undefined ? null : (
        <div className="line-clamp-2 text-xs text-khala-text-muted">
          {objective}
        </div>
      )}
      {chain.events.length === 0 ? null : (
        <ol className="m-0 grid list-none gap-1 p-0">
          {chain.events.slice(0, 5).map(event => (
            <li
              key={event.eventRef}
              className="flex items-center justify-between gap-2 text-[0.6875rem] text-khala-text-faint"
            >
              <span className="truncate">
                <span className="text-khala-text-muted">{event.eventKind}</span>
                {' · '}
                {event.status}
              </span>
              <span className="shrink-0">
                {relativeTime(event.createdAt, nowMs)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}

export function OverviewDashboard({
  snapshot,
  nowMs,
  side,
  lastRefreshedAt,
  refreshing,
}: Readonly<{
  snapshot: OverviewSnapshot
  nowMs: number
  side: SideState
  lastRefreshedAt: number
  refreshing: boolean
}>) {
  const { agentChains, tokens, traces, fleet, cloudHealth } = snapshot

  return (
    <main
      className="grid min-h-dvh gap-4 bg-khala-void px-4 py-6 text-khala-text sm:px-6"
      data-route="admin-operator"
    >
      <header className="flex flex-wrap items-end justify-between gap-3 border-b border-khala-border pb-4">
        <div className="grid gap-1">
          <p className={labelClass}>OpenAgents · admin</p>
          <h1 className="m-0 text-xl font-semibold text-khala-text">
            Operator dashboard
          </h1>
        </div>
        <div className="flex items-center gap-2 text-[0.6875rem] text-khala-text-faint">
          <span
            aria-hidden
            className={`inline-block h-2 w-2 rounded-full ${
              refreshing ? 'bg-khala-energy' : 'bg-khala-success'
            }`}
          />
          <span>
            {refreshing ? 'refreshing' : 'live'} · updated{' '}
            {relativeTime(new Date(lastRefreshedAt).toISOString(), nowMs)}
          </span>
        </div>
      </header>

      {/* Top stat strip */}
      <section className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
        <Stat
          label="Active chains"
          value={fmt(agentChains.activeCount)}
          detail={`${fmt(agentChains.recentCount)} recent`}
        />
        <Stat
          label="Pylons online"
          value={`${fmt(fleet.onlineCount)}/${fmt(fleet.totalCount)}`}
          detail="registered fleet"
        />
        <Stat
          label="Tokens 24h"
          value={fmt(tokens.last24h.tokens)}
          detail={`${fmt(tokens.last24h.events)} turns`}
        />
        <Stat
          label="Tokens total"
          value={fmt(tokens.total.tokens)}
          detail={`${fmt(tokens.total.events)} turns`}
        />
        <Stat label="Traces" value={fmt(traces.length)} detail="recent" />
        <Stat
          label="Last cloud turn"
          value={
            cloudHealth.lastOrgCloudTurnAt?.status === 'ok'
              ? relativeTime(cloudHealth.lastOrgCloudTurnAt.value, nowMs)
              : '—'
          }
          detail="org-cloud runtime"
        />
      </section>

      <div className="grid gap-4 lg:grid-cols-3">
        {/* Agent chains — the core "everything going on" column */}
        <Card className="lg:col-span-2">
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle>Agent chains</CardTitle>
            <Badge variant="outline">{fmt(agentChains.recentCount)}</Badge>
          </CardHeader>
          <CardContent className="grid gap-2">
            {agentChains.chains.length === 0 ? (
              <p className="m-0 text-sm text-khala-text-faint">
                No recent Pylon/Codex assignments.
              </p>
            ) : (
              agentChains.chains.map(chain => (
                <AgentChainRow
                  key={chain.assignmentRef}
                  chain={chain}
                  nowMs={nowMs}
                />
              ))
            )}
          </CardContent>
        </Card>

        <div className="grid content-start gap-4">
          {/* Fleet */}
          <Card>
            <CardHeader>
              <CardTitle>Fleet</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-2">
              {fleet.pylons.length === 0 ? (
                <p className="m-0 text-sm text-khala-text-faint">
                  No registered Pylons.
                </p>
              ) : (
                fleet.pylons.map(pylon => (
                  <div
                    key={pylon.pylonRef}
                    className="flex items-center justify-between gap-2 border border-khala-border bg-khala-void p-2"
                  >
                    <div className="grid min-w-0 gap-0.5">
                      <span className="truncate text-xs text-khala-text">
                        {pylon.displayName}
                      </span>
                      <span className="truncate font-mono text-[0.625rem] text-khala-text-faint">
                        {pylon.pylonRef}
                      </span>
                    </div>
                    <Badge
                      variant={pylon.status === 'online' ? 'ready' : 'outline'}
                    >
                      {pylon.status}
                    </Badge>
                  </div>
                ))
              )}
            </CardContent>
          </Card>

          {/* Cloud health */}
          <Card>
            <CardHeader>
              <CardTitle>Cloud health</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-2">
              <HealthRow
                label="Ops health strip"
                side={side.opsHealth}
                nowMs={nowMs}
              />
              {Object.entries(cloudHealth).map(([key, signal]) => (
                <div
                  key={key}
                  className="flex items-center justify-between gap-2 text-xs"
                >
                  <span className="truncate text-khala-text-muted">{key}</span>
                  <span className="shrink-0 text-khala-text-faint">
                    {signal.status === 'ok'
                      ? relativeTime(signal.value, nowMs)
                      : signal.status}
                  </span>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        {/* Token usage */}
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Token usage · last 24h</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-4 sm:grid-cols-2">
            <div className="grid gap-1">
              <p className={labelClass}>By demand source</p>
              {tokens.byDemandSource.length === 0 ? (
                <p className="m-0 text-xs text-khala-text-faint">No usage.</p>
              ) : (
                tokens.byDemandSource.map(row => (
                  <div
                    key={row.demandSource}
                    className="flex items-center justify-between gap-2 text-xs"
                  >
                    <span className="truncate font-mono text-khala-text-muted">
                      {row.demandSource}
                    </span>
                    <span className="shrink-0 tabular-nums text-khala-text">
                      {fmt(row.tokens)}
                    </span>
                  </div>
                ))
              )}
            </div>
            <div className="grid gap-1">
              <p className={labelClass}>By provider</p>
              {tokens.byProvider.length === 0 ? (
                <p className="m-0 text-xs text-khala-text-faint">No usage.</p>
              ) : (
                tokens.byProvider.map(row => (
                  <div
                    key={row.provider}
                    className="flex items-center justify-between gap-2 text-xs"
                  >
                    <span className="truncate font-mono text-khala-text-muted">
                      {row.provider}
                    </span>
                    <span className="shrink-0 tabular-nums text-khala-text">
                      {fmt(row.tokens)}
                    </span>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        {/* Recent traces */}
        <Card>
          <CardHeader>
            <CardTitle>Recent traces</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-2">
            {traces.length === 0 ? (
              <p className="m-0 text-sm text-khala-text-faint">No traces.</p>
            ) : (
              traces.slice(0, 10).map(trace => (
                <a
                  key={trace.traceUuid}
                  className="grid gap-0.5 border border-khala-border bg-khala-void p-2 no-underline transition-colors hover:border-khala-border-strong"
                  href={`/trace/${trace.traceUuid}`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate font-mono text-[0.6875rem] text-khala-text-muted">
                      {trace.agentRef}
                    </span>
                    <Badge variant="outline">{trace.visibility}</Badge>
                  </div>
                  <span className="text-[0.625rem] text-khala-text-faint">
                    {trace.stepCount} steps ·{' '}
                    {relativeTime(trace.createdAt, nowMs)}
                  </span>
                </a>
              ))
            )}
          </CardContent>
        </Card>
      </div>

      {/* Full Auto + Fleet runs (composed client-side from owner-scoped
          endpoints; honest raw fallback) */}
      <div className="grid gap-4 lg:grid-cols-2">
        <SideCard title="Full Auto runs (owner-scoped)" side={side.fullAuto} />
        <SideCard title="Fleet runs (owner-scoped)" side={side.fleetRuns} />
      </div>

      <footer className="border-t border-khala-border pt-3 text-[0.625rem] text-khala-text-faint">
        Admin-only. Agent chains, tokens, traces, and fleet are the redacted
        server snapshot. Full Auto and Fleet runs are the signed-in owner&apos;s
        own live projections. Auto-refreshes every {REFRESH_MS / 1000}s.
      </footer>
    </main>
  )
}

function HealthRow({
  label,
  side,
  nowMs: _nowMs,
}: Readonly<{ label: string; side: SideResult | undefined; nowMs: number }>) {
  const ok = side?.tag === 'ok'
  return (
    <div className="flex items-center justify-between gap-2 text-xs">
      <span className="truncate text-khala-text-muted">{label}</span>
      <Badge variant={ok ? 'ready' : 'outline'}>
        {side === undefined
          ? '…'
          : ok
            ? 'reachable'
            : `unavailable${side.status ? ` (${side.status})` : ''}`}
      </Badge>
    </div>
  )
}

function SideCard({
  title,
  side,
}: Readonly<{ title: string; side: SideResult | undefined }>) {
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>{title}</CardTitle>
        <Badge variant={side?.tag === 'ok' ? 'ready' : 'outline'}>
          {side === undefined
            ? 'loading'
            : side.tag === 'ok'
              ? 'live'
              : side.status === 403
                ? 'not owner'
                : 'unavailable'}
        </Badge>
      </CardHeader>
      <CardContent>
        {side === undefined ? (
          <p className="m-0 text-xs text-khala-text-faint">Loading…</p>
        ) : side.tag === 'error' ? (
          <p className="m-0 text-xs text-khala-text-faint">
            No live data from this endpoint for the signed-in owner.
          </p>
        ) : (
          <pre className="m-0 max-h-56 overflow-auto whitespace-pre-wrap break-words font-mono text-[0.6875rem] leading-relaxed text-khala-text-muted">
            {JSON.stringify(side.value, null, 2)}
          </pre>
        )}
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Status views
// ---------------------------------------------------------------------------

function StatusView({
  title,
  message,
  busy,
}: Readonly<{ title: string; message: string; busy?: boolean }>) {
  return (
    <main
      aria-busy={busy ? 'true' : undefined}
      className="grid min-h-dvh place-items-center bg-khala-void px-4 py-12 text-khala-text"
      data-route="admin-operator"
    >
      <div className="grid max-w-[min(100%,32rem)] gap-3 border border-khala-border bg-khala-surface p-6">
        <p className={labelClass}>OpenAgents · admin</p>
        <h1 className="m-0 text-lg font-medium text-khala-text">{title}</h1>
        <p className="m-0 text-sm/6 text-khala-text-muted">{message}</p>
        {busy ? null : (
          <a
            className="khala-focus inline-flex min-h-10 w-fit items-center border border-khala-text bg-khala-text px-4 text-[0.8125rem] text-black hover:bg-white"
            href="/login"
          >
            Sign in
          </a>
        )}
      </div>
    </main>
  )
}

// ---------------------------------------------------------------------------
// Page container
// ---------------------------------------------------------------------------

type SideState = Readonly<{
  opsHealth?: SideResult
  fullAuto?: SideResult
  fleetRuns?: SideResult
}>

type LoadState =
  | Readonly<{ tag: 'loading' }>
  | Readonly<{ tag: 'result'; result: OverviewResult }>

export function AdminOperatorPage() {
  const [state, setState] = useState<LoadState>({ tag: 'loading' })
  const [side, setSide] = useState<SideState>({})
  const [refreshing, setRefreshing] = useState(false)
  const [lastRefreshedAt, setLastRefreshedAt] = useState(() => Date.now())
  const [nowMs, setNowMs] = useState(() => Date.now())
  const firstLoad = useRef(true)

  const load = useCallback(async () => {
    if (!firstLoad.current) setRefreshing(true)
    const result = await fetchOperatorOverview()
    setState({ tag: 'result', result })
    setLastRefreshedAt(Date.now())
    setRefreshing(false)
    firstLoad.current = false

    if (result.tag === 'loaded') {
      // Best-effort side reads; failures degrade to honest per-card markers.
      const [opsHealth, fullAuto, fleetRuns] = await Promise.all([
        fetchOpsHealth(),
        fetchFullAutoRuns(),
        fetchFleetRuns(),
      ])
      setSide({ opsHealth, fullAuto, fleetRuns })
    }
  }, [])

  useEffect(() => {
    void load()
    const interval = setInterval(() => void load(), REFRESH_MS)
    return () => clearInterval(interval)
  }, [load])

  // Keep relative timestamps fresh between polls.
  useEffect(() => {
    const tick = setInterval(() => setNowMs(Date.now()), 1000)
    return () => clearInterval(tick)
  }, [])

  const view = useMemo(() => {
    if (state.tag === 'loading') {
      return (
        <StatusView
          busy
          title="Loading operator dashboard"
          message="Reading the live operator snapshot."
        />
      )
    }
    const { result } = state
    switch (result.tag) {
      case 'forbidden':
        return (
          <StatusView
            title="Admins only"
            message="This dashboard is restricted to OpenAgents admin accounts. Your account is signed in but not authorized."
          />
        )
      case 'unauthorized':
        return (
          <StatusView
            busy={false}
            title="Sign in required"
            message="Sign in with an OpenAgents admin account to view the operator dashboard."
          />
        )
      case 'failed':
        return (
          <StatusView title="Dashboard unavailable" message={result.error} />
        )
      case 'loaded':
        return (
          <OverviewDashboard
            snapshot={result.snapshot}
            nowMs={nowMs}
            side={side}
            lastRefreshedAt={lastRefreshedAt}
            refreshing={refreshing}
          />
        )
    }
  }, [state, nowMs, side, lastRefreshedAt, refreshing])

  return view
}
