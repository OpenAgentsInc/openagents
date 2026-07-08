import { useEffect, useState } from 'react'

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { RecentUsersPanel } from '@/dashboard/recent-users-panel'

import {
  fetchDailySalesLedger,
  fetchOpsHealth,
  fetchOpsRuns,
  type DailySalesLedger,
  type OpsHealthCheck,
  type OpsRun,
} from './ops-api-client'

function RunsPanel() {
  const [runs, setRuns] = useState<ReadonlyArray<OpsRun>>([])
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchOpsRuns()
      if (cancelled) return
      if (!result.ok) {
        setStatus('error')
        return
      }
      setRuns(result.value.runs)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent org-cloud runs</CardTitle>
        <CardDescription>
          Exact usage receipts per turn, read directly from{' '}
          <code>token_usage_events</code>. Not yet a live Khala Sync feed —
          see the closing note on #8501.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {status === 'loading' && <p className="text-sm text-khala-text-faint">Loading...</p>}
        {status === 'error' && <p className="text-sm text-khala-danger">Failed to load.</p>}
        <ul className="grid gap-1" data-testid="ops-runs-list">
          {status === 'ready' && runs.length === 0 && (
            <li className="text-sm text-khala-text-faint" data-testid="ops-runs-empty">
              No org-cloud runs recorded yet.
            </li>
          )}
          {runs.map((run, index) => (
            <li
              className="flex items-center justify-between border-b border-khala-border py-1 text-sm text-khala-text"
              // Runs have no single stable id in this v1 read; index is
              // acceptable since the list is a point-in-time snapshot, not
              // reordered/animated.
              key={`${run.observedAt}-${index}`}
            >
              <span>
                {run.userId ?? 'unknown user'} — {run.provider}/{run.model} (
                {run.threadId ?? '—'})
              </span>
              <span>{run.totalTokens.toLocaleString()} tok</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

function HealthCheckRow({ label, check }: { label: string; check: OpsHealthCheck }) {
  const dotClass =
    check.status === 'ok'
      ? 'bg-khala-success'
      : check.status === 'error'
        ? 'bg-khala-danger'
        : 'bg-khala-text-faint'
  const detail =
    check.status === 'ok'
      ? check.value
      : check.status === 'error'
        ? check.messageSafe
        : 'not measured'

  return (
    <li className="flex items-center justify-between border-b border-khala-border py-1.5 text-sm text-khala-text">
      <span className="flex items-center gap-2">
        <span aria-hidden="true" className={`h-2 w-2 rounded-full ${dotClass}`} />
        {label}
      </span>
      <span className="text-khala-text-muted">{detail}</span>
    </li>
  )
}

function HealthStrip() {
  const [checks, setChecks] = useState<
    | Readonly<{
        lastOrgCloudTurnCompletedAt: OpsHealthCheck
        pushDeviceTokensRegistered: OpsHealthCheck
        khalaPublicStatsReachable: OpsHealthCheck
      }>
    | undefined
  >(undefined)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchOpsHealth()
      if (cancelled) return
      if (!result.ok) {
        setStatus('error')
        return
      }
      setChecks(result.value.checks)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Card>
      <CardHeader>
        <CardTitle>Health strip</CardTitle>
        <CardDescription>A simple green/red operating snapshot.</CardDescription>
      </CardHeader>
      <CardContent>
        {status === 'loading' && <p className="text-sm text-khala-text-faint">Loading...</p>}
        {status === 'error' && <p className="text-sm text-khala-danger">Failed to load.</p>}
        {checks !== undefined && (
          <ul className="grid gap-0" data-testid="ops-health-list">
            <HealthCheckRow
              check={checks.lastOrgCloudTurnCompletedAt}
              label="Last org-cloud turn completed"
            />
            <HealthCheckRow
              check={checks.pushDeviceTokensRegistered}
              label="Push device tokens registered"
            />
            <HealthCheckRow
              check={checks.khalaPublicStatsReachable}
              label="Khala public stats reachable"
            />
          </ul>
        )}
      </CardContent>
    </Card>
  )
}

const healthDotClass = (health: string): string =>
  health === 'healthy'
    ? 'bg-khala-success'
    : health === 'at_risk'
      ? 'bg-khala-warning'
      : health === 'breach'
        ? 'bg-khala-danger'
        : 'bg-khala-text-faint'

function DailySalesLedgerPanel() {
  const [ledger, setLedger] = useState<DailySalesLedger | undefined>(undefined)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchDailySalesLedger()
      if (cancelled) return
      if (!result.ok) {
        setStatus('error')
        return
      }
      setLedger(result.value.ledger)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Card>
      <CardHeader>
        <CardTitle>Daily sales ledger</CardTitle>
        <CardDescription>
          Outbound funnel (sourced/drafted/approved/sent/quoted/closed) and
          deliverability health, read from the real event tables OB-2/OB-3/
          OB-4 already write to. Metrics surface only — OB-1 owns the actual
          ramp enforcement. See #8563.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {status === 'loading' && <p className="text-sm text-khala-text-faint">Loading...</p>}
        {status === 'error' && <p className="text-sm text-khala-danger">Failed to load.</p>}
        {ledger !== undefined && (
          <div className="grid gap-3" data-testid="daily-sales-ledger">
            <p className="text-sm text-khala-text" data-testid="daily-sales-ledger-digest">
              {ledger.digestLine}
            </p>
            <div className="text-sm text-khala-text-muted">
              Window {ledger.since} → {ledger.until}. Totals: sourced{' '}
              {ledger.totals.sourced}, drafted {ledger.totals.drafted}, approved{' '}
              {ledger.totals.approved}, sent {ledger.totals.sent}, delivered{' '}
              {ledger.totals.delivered}, bounced {ledger.totals.bounced}, complained{' '}
              {ledger.totals.complained}, opt-outs {ledger.totals.optOuts}, quoted{' '}
              {ledger.totals.quoted}, closed won {ledger.totals.closedWon}, closed lost{' '}
              {ledger.totals.closedLost}.
            </div>
            <ul className="grid gap-0" data-testid="daily-sales-ledger-deliverability">
              {ledger.deliverabilityDays.map(day => (
                <li
                  className="flex items-center justify-between border-b border-khala-border py-1.5 text-sm text-khala-text"
                  key={day.date}
                >
                  <span className="flex items-center gap-2">
                    <span
                      aria-hidden="true"
                      className={`h-2 w-2 rounded-full ${healthDotClass(day.health)}`}
                    />
                    {day.date}
                  </span>
                  <span className="text-khala-text-muted">
                    delivered {day.delivered} / bounced {day.bounced} / complained{' '}
                    {day.complained} / opt-outs {day.optOuts} ({day.health})
                  </span>
                </li>
              ))}
            </ul>
            {ledger.notMeasured.length > 0 && (
              <p className="text-xs text-khala-text-faint" data-testid="daily-sales-ledger-gaps">
                Not yet measured: {ledger.notMeasured.map(entry => entry.field).join(', ')}.
              </p>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

export function OpsConsole() {
  return (
    <div className="grid gap-6">
      <HealthStrip />
      <RecentUsersPanel />
      <RunsPanel />
      <DailySalesLedgerPanel />
    </div>
  )
}
