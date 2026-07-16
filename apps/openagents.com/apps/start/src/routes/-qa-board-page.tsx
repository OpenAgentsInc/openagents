import { PublicPageShell } from '@/components/public-page-shell'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { useCallback, useEffect, useState } from 'react'

import {
  type Loadable,
  type QaBoardProjection,
  type QaCheckStatus,
  type QaSeverity,
  fetchQaBoard,
  freshnessLabel,
} from './-qa-board-data'

const severityVariant = (
  value: QaSeverity,
): 'danger' | 'warning' | 'outline' =>
  value === 'critical' || value === 'high'
    ? 'danger'
    : value === 'medium'
      ? 'warning'
      : 'outline'

const statusVariant = (value: QaCheckStatus): 'ready' | 'danger' | 'warning' =>
  value === 'pass' ? 'ready' : value === 'drift' ? 'danger' : 'warning'

function LoadingBoard() {
  return (
    <div aria-label="Loading QA evidence" className="grid gap-3" role="status">
      {[0, 1, 2].map(row => (
        <div
          className="h-20 animate-pulse border border-khala-border bg-khala-surface-raised/40 motion-reduce:animate-none"
          key={row}
        />
      ))}
    </div>
  )
}

function SourceState({
  label,
  state,
}: Readonly<{
  label: string
  state: 'ok' | 'empty' | 'unavailable'
}>) {
  return (
    <div className="flex items-center justify-between gap-3 border-t border-khala-border/60 py-2 first:border-t-0">
      <span className="text-xs text-khala-text-muted">{label}</span>
      <Badge
        variant={
          state === 'ok' ? 'ready' : state === 'empty' ? 'outline' : 'danger'
        }
      >
        {state}
      </Badge>
    </div>
  )
}

function SummaryStrip({
  projection,
}: Readonly<{ projection: QaBoardProjection }>) {
  const observer = projection.observer
  const openFindings = projection.findings.filter(
    finding => finding.issueState === 'open',
  ).length
  const issueStateUnavailable = projection.findings.filter(
    finding => finding.issueState === 'unavailable',
  ).length
  return (
    <div className="grid border border-khala-border bg-black/30 sm:grid-cols-3">
      <div className="px-4 py-3 sm:border-r sm:border-khala-border">
        <p className="m-0 text-xs text-khala-text-faint">Observer checks</p>
        <p className="m-0 mt-1 font-mono text-lg font-semibold tabular-nums text-khala-text">
          {observer === null
            ? 'Unavailable'
            : `${observer.summary.pass}/${observer.summary.total} passing`}
        </p>
      </div>
      <div className="border-t border-khala-border px-4 py-3 sm:border-r sm:border-t-0">
        <p className="m-0 text-xs text-khala-text-faint">Latest swarm</p>
        <p className="m-0 mt-1 font-mono text-lg font-semibold text-khala-text">
          {projection.swarm?.verdict ?? 'Unavailable'}
        </p>
      </div>
      <div className="border-t border-khala-border px-4 py-3 sm:border-t-0">
        <p className="m-0 text-xs text-khala-text-faint">
          Issue-linked findings
        </p>
        <p className="m-0 mt-1 font-mono text-lg font-semibold tabular-nums text-khala-text">
          {projection.sources.issues === 'unavailable'
            ? 'Unavailable'
            : `${openFindings} open${issueStateUnavailable > 0 ? ` · ${issueStateUnavailable} unknown` : ''}`}
        </p>
      </div>
    </div>
  )
}

function FindingsPanel({
  projection,
}: Readonly<{ projection: QaBoardProjection }>) {
  return (
    <Card className="min-w-0 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="m-0 text-sm font-semibold text-khala-text">
            Open findings
          </h2>
          <p className="m-0 mt-1 text-xs leading-5 text-khala-text-faint">
            Confirmed QA-1 findings enriched from the live GitHub issue ledger.
          </p>
        </div>
        <Badge
          variant={projection.sources.issues === 'ok' ? 'ready' : 'warning'}
        >
          issue ledger {projection.sources.issues}
        </Badge>
      </div>
      {projection.findings.length === 0 ? (
        <div
          className="mt-4 border border-khala-border bg-black/25 px-4 py-5"
          data-qa-empty="findings"
        >
          <p className="m-0 text-sm font-medium text-khala-text">
            No open issue-linked findings.
          </p>
          <p className="m-0 mt-1 max-w-[65ch] text-xs leading-5 text-khala-text-faint">
            This means the latest durable QA report has no finding still known
            open; it does not claim that unobserved surfaces are healthy.
          </p>
        </div>
      ) : (
        <div className="mt-3 divide-y divide-khala-border/60">
          {projection.findings.map(finding => (
            <article
              className="grid gap-2 py-3 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-start"
              key={`${finding.surface}-${finding.issueNumber ?? 'unknown'}`}
            >
              <Badge variant={severityVariant(finding.severity)}>
                {finding.severity}
              </Badge>
              <div className="min-w-0">
                <h3 className="m-0 text-sm font-medium text-khala-text">
                  {finding.surface}
                </h3>
                <p className="m-0 mt-1 text-xs leading-5 text-khala-text-faint">
                  {finding.summary}
                </p>
              </div>
              {finding.issueUrl === null ? (
                <Badge variant="warning">link unavailable</Badge>
              ) : (
                <a
                  className="khala-focus text-xs font-semibold text-khala-energy-soft underline underline-offset-4 hover:text-khala-energy-cyan"
                  href={finding.issueUrl}
                  rel="noreferrer"
                  target="_blank"
                >
                  #{finding.issueNumber} · {finding.issueState}
                </a>
              )}
            </article>
          ))}
        </div>
      )}
    </Card>
  )
}

function ObserverPanel({
  projection,
}: Readonly<{ projection: QaBoardProjection }>) {
  const observer = projection.observer
  return (
    <Card className="min-w-0 overflow-hidden">
      <div className="flex flex-wrap items-start justify-between gap-3 p-4">
        <div>
          <h2 className="m-0 text-sm font-semibold text-khala-text">
            Observer checks
          </h2>
          <p className="m-0 mt-1 text-xs leading-5 text-khala-text-faint">
            QA-2 drift checks from the latest committed observer run.
          </p>
        </div>
        <Badge
          variant={projection.sources.observer === 'ok' ? 'ready' : 'danger'}
        >
          observer {projection.sources.observer}
        </Badge>
      </div>
      {observer === null ? (
        <p
          className="m-0 border-t border-khala-border px-4 py-5 text-sm text-khala-danger"
          role="status"
        >
          Observer evidence is unavailable. No check is being represented as
          passing.
        </p>
      ) : observer.checks.length === 0 ? (
        <p
          className="m-0 border-t border-khala-border px-4 py-5 text-sm text-khala-text-muted"
          role="status"
        >
          The latest observer artifact contains no checks.
        </p>
      ) : (
        <div className="border-t border-khala-border">
          <div className="divide-y divide-khala-border/60 sm:hidden">
            {observer.checks.map(check => (
              <article className="grid gap-2 px-4 py-3" key={check.id}>
                <div className="flex min-w-0 items-start justify-between gap-3">
                  <a
                    className="min-w-0 truncate font-mono text-xs text-khala-text hover:text-khala-energy-soft"
                    href={check.surface}
                  >
                    {check.id}
                  </a>
                  <Badge variant={statusVariant(check.status)}>
                    {check.status}
                  </Badge>
                </div>
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2 text-xs text-khala-text-faint">
                    <span>Drift:</span>
                    <Badge variant={severityVariant(check.severityOnDrift)}>
                      {check.severityOnDrift}
                    </Badge>
                  </div>
                  <span className="font-mono text-xs tabular-nums text-khala-text-muted">
                    {check.durationMs}ms
                  </span>
                </div>
              </article>
            ))}
          </div>
          <div className="hidden overflow-x-auto sm:block">
            <table className="w-full min-w-[44rem] border-collapse text-left text-xs">
              <caption className="sr-only">
                Latest QA observer check results
              </caption>
              <thead className="bg-black/30 text-khala-text-faint">
                <tr>
                  <th className="px-4 py-2 font-medium" scope="col">
                    Check
                  </th>
                  <th className="px-3 py-2 font-medium" scope="col">
                    State
                  </th>
                  <th className="px-3 py-2 font-medium" scope="col">
                    Drift severity
                  </th>
                  <th className="px-4 py-2 text-right font-medium" scope="col">
                    Duration
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-khala-border/60">
                {observer.checks.map(check => (
                  <tr key={check.id}>
                    <th className="px-4 py-3 font-normal" scope="row">
                      <span className="block font-mono text-khala-text">
                        {check.id}
                      </span>
                      <a
                        className="mt-1 block max-w-[52ch] truncate text-khala-text-faint hover:text-khala-energy-soft"
                        href={check.surface}
                      >
                        {check.surface}
                      </a>
                    </th>
                    <td className="px-3 py-3">
                      <Badge variant={statusVariant(check.status)}>
                        {check.status}
                      </Badge>
                    </td>
                    <td className="px-3 py-3">
                      <Badge variant={severityVariant(check.severityOnDrift)}>
                        {check.severityOnDrift}
                      </Badge>
                    </td>
                    <td className="px-4 py-3 text-right font-mono tabular-nums text-khala-text-muted">
                      {check.durationMs}ms
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </Card>
  )
}

function SwarmPanel({
  projection,
}: Readonly<{ projection: QaBoardProjection }>) {
  const swarm = projection.swarm
  return (
    <Card className="min-w-0 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="m-0 text-sm font-semibold text-khala-text">
            Latest six-lane swarm
          </h2>
          <p className="m-0 mt-1 text-xs leading-5 text-khala-text-faint">
            QA-1 current-main contracts and read-only production probes.
          </p>
        </div>
        <Badge
          variant={
            swarm?.verdict === 'pass'
              ? 'ready'
              : swarm === null
                ? 'danger'
                : 'warning'
          }
        >
          {swarm?.verdict ?? 'unavailable'}
        </Badge>
      </div>
      {swarm === null ? (
        <p
          className="m-0 mt-4 border border-khala-border bg-black/25 px-4 py-5 text-sm text-khala-danger"
          role="status"
        >
          Swarm evidence is unavailable. No lane result is being inferred.
        </p>
      ) : (
        <>
          <div className="mt-3 divide-y divide-khala-border/60">
            {swarm.lanes.map(lane => (
              <div
                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 py-2.5"
                key={lane.id}
              >
                <div className="min-w-0">
                  <p className="m-0 font-mono text-xs text-khala-text">
                    {lane.id}
                  </p>
                  <p className="m-0 mt-0.5 truncate text-xs text-khala-text-faint">
                    {lane.surface}
                  </p>
                </div>
                <Badge variant={lane.verdict === 'pass' ? 'ready' : 'danger'}>
                  {lane.verdict}
                </Badge>
              </div>
            ))}
          </div>
          <p className="m-0 mt-3 border-t border-khala-border pt-3 font-mono text-[0.68rem] leading-5 text-khala-text-faint">
            {swarm.runRef} · {swarm.baseSha.slice(0, 12)} ·{' '}
            {freshnessLabel(swarm.completedAt)}
          </p>
        </>
      )}
    </Card>
  )
}

export function QaBoardPage() {
  const [snapshot, setSnapshot] = useState<Loadable<QaBoardProjection>>({
    state: 'loading',
  })
  const refresh = useCallback(async () => {
    setSnapshot({ state: 'loading' })
    setSnapshot(await fetchQaBoard())
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return (
    <PublicPageShell dataRoute="qa">
      <main
        aria-label="QA board"
        className="min-h-dvh bg-khala-void text-khala-text"
      >
        <div className="mx-auto grid w-full max-w-7xl gap-3 px-3 py-8 sm:px-4 lg:px-6">
          <header className="grid gap-4 border border-khala-border bg-khala-surface p-4 lg:grid-cols-[minmax(0,1fr)_18rem] lg:items-end">
            <div>
              <div className="flex items-center gap-2">
                <span
                  aria-hidden="true"
                  className="h-2 w-2 rounded-full bg-khala-energy shadow-[0_0_8px_rgba(58,123,255,0.85)]"
                />
                <span className="font-mono text-xs font-semibold text-khala-energy-soft">
                  QA control room
                </span>
              </div>
              <h1 className="m-0 mt-3 text-[1.55rem] font-semibold leading-tight text-khala-text">
                Live QA board
              </h1>
              <p className="m-0 mt-2 max-w-[70ch] text-sm leading-6 text-khala-text-muted">
                Durable observer checks, issue-linked findings, and the latest
                six-lane swarm run. Missing evidence stays unavailable—never
                green by default.
              </p>
            </div>
            <div className="flex items-center justify-between gap-3 lg:justify-end">
              <p
                aria-live="polite"
                className="m-0 text-xs text-khala-text-faint"
              >
                {snapshot.state === 'ok'
                  ? freshnessLabel(snapshot.data.servedAt)
                  : snapshot.state === 'loading'
                    ? 'Refreshing evidence…'
                    : 'Evidence unavailable'}
              </p>
              <Button
                disabled={snapshot.state === 'loading'}
                onClick={() => void refresh()}
                size="sm"
                variant="secondary"
              >
                Refresh
              </Button>
            </div>
          </header>

          {snapshot.state === 'loading' ? (
            <LoadingBoard />
          ) : snapshot.state === 'unavailable' ? (
            <Card
              className="border-khala-danger/50 bg-khala-surface p-5"
              data-qa-state="unavailable"
              role="alert"
            >
              <Badge variant="danger">Unavailable</Badge>
              <h2 className="m-0 mt-3 text-base font-semibold text-khala-text">
                QA evidence could not be loaded
              </h2>
              <p className="m-0 mt-2 max-w-[70ch] text-sm leading-6 text-khala-text-muted">
                {snapshot.detail}
              </p>
            </Card>
          ) : (
            <div className="grid gap-3" data-qa-state="ok">
              <SummaryStrip projection={snapshot.data} />
              <div className="grid gap-3 lg:grid-cols-[minmax(0,1.65fr)_minmax(18rem,0.85fr)]">
                <ObserverPanel projection={snapshot.data} />
                <div className="grid content-start gap-3">
                  <Card className="p-4">
                    <h2 className="m-0 text-sm font-semibold text-khala-text">
                      Evidence sources
                    </h2>
                    <div className="mt-2">
                      <SourceState
                        label="QA-2 observer registry"
                        state={snapshot.data.sources.observer}
                      />
                      <SourceState
                        label="QA-1 swarm registry"
                        state={snapshot.data.sources.swarm}
                      />
                      <SourceState
                        label="GitHub issue ledger"
                        state={snapshot.data.sources.issues}
                      />
                    </div>
                  </Card>
                  <SwarmPanel projection={snapshot.data} />
                </div>
              </div>
              <FindingsPanel projection={snapshot.data} />
            </div>
          )}
        </div>
      </main>
    </PublicPageShell>
  )
}
