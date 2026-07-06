import { useEffect, useState } from 'react'

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { RecentUsersPanel } from '@/dashboard/recent-users-panel'

import { fetchOpsHealth, fetchOpsRuns, type OpsHealthCheck, type OpsRun } from './ops-api-client'

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

export function OpsConsole() {
  return (
    <div className="grid gap-6">
      <HealthStrip />
      <RecentUsersPanel />
      <RunsPanel />
    </div>
  )
}
