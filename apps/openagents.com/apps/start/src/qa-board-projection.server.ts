import type {
  QaBoardFinding,
  QaBoardProjection,
  QaCheckStatus,
  QaObserverCheck,
  QaSeverity,
  QaSwarmLane,
} from './routes/-qa-board-data'

const observerModules = import.meta.glob(
  '../../../../../docs/qa/observer/results/qa-observer-run-*.json',
  { eager: true, import: 'default' },
) as Record<string, unknown>

const swarmModules = import.meta.glob(
  '../../../../../docs/qa/**/evidence/run.json',
  {
    eager: true,
    import: 'default',
  },
) as Record<string, unknown>

const reportModules = import.meta.glob('../../../../../docs/qa/**/README.md', {
  eager: true,
  import: 'default',
  query: '?raw',
}) as Record<string, string>

type RecordValue = Record<string, unknown>

const record = (value: unknown): RecordValue | null =>
  typeof value === 'object' && value !== null ? (value as RecordValue) : null

const text = (value: unknown): string | null =>
  typeof value === 'string' && value.length > 0 ? value : null

const finite = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

const severity = (value: unknown): QaSeverity =>
  value === 'critical' ||
  value === 'high' ||
  value === 'medium' ||
  value === 'low'
    ? value
    : 'unclassified'

const checkStatus = (value: unknown): QaCheckStatus | null =>
  value === 'pass' || value === 'drift' || value === 'unrunnable' ? value : null

const latestBy = (
  values: ReadonlyArray<unknown>,
  field: string,
): RecordValue | null =>
  values
    .map(record)
    .filter(
      (value): value is RecordValue =>
        value !== null && text(value[field]) !== null,
    )
    .toSorted((left, right) =>
      String(right[field]).localeCompare(String(left[field])),
    )[0] ?? null

const latestObserver = (): RecordValue | null =>
  latestBy(Object.values(observerModules), 'runAt')

const latestSwarm = (): RecordValue | null =>
  latestBy(
    Object.values(swarmModules).filter(
      value => record(value)?.schema === 'openagents.qa.six-lane-run.v1',
    ),
    'completedAt',
  )

const observerChecks = (
  artifact: RecordValue,
): ReadonlyArray<QaObserverCheck> =>
  (Array.isArray(artifact.results) ? artifact.results : []).flatMap(value => {
    const row = record(value)
    if (row === null) return []
    const id = text(row.id)
    const surface = text(row.surface)
    const status = checkStatus(row.status)
    if (id === null || surface === null || status === null) return []
    return [
      {
        consecutiveDriftRuns: finite(row.consecutiveDriftRuns) ?? 0,
        durationMs: finite(row.durationMs) ?? 0,
        id,
        severityOnDrift: severity(row.severityOnDrift),
        status,
        surface,
      },
    ]
  })

const swarmLanes = (artifact: RecordValue): ReadonlyArray<QaSwarmLane> =>
  (Array.isArray(artifact.lanes) ? artifact.lanes : []).flatMap(value => {
    const row = record(value)
    if (row === null) return []
    const id = text(row.id)
    const surface = text(row.surface)
    const verdict =
      row.verdict === 'pass' || row.verdict === 'finding' ? row.verdict : null
    return id === null || surface === null || verdict === null
      ? []
      : [{ id, surface, verdict }]
  })

type FindingDraft = Omit<QaBoardFinding, 'issueState'>

const reportFindingDrafts = (): ReadonlyArray<FindingDraft> => {
  const pattern =
    /^\|\s*(critical|high|medium|low)\s*\|\s*([^|]+?)\s*\|[^\n]*?\[#(\d+)\]\((https:\/\/github\.com\/OpenAgentsInc\/openagents\/issues\/\d+)\)[^\n]*$/gimu
  return Object.values(reportModules).flatMap(markdown =>
    [...markdown.matchAll(pattern)].map(match => ({
      issueNumber: Number(match[3]),
      issueUrl: match[4] ?? null,
      severity: severity(match[1]?.toLowerCase()),
      summary: `Confirmed ${match[1]?.toLowerCase()}-severity finding on ${match[2]?.trim() ?? 'unknown surface'}.`,
      surface: match[2]?.trim() ?? 'Unknown surface',
    })),
  )
}

const issueState = async (
  draft: FindingDraft,
  fetchFn: typeof fetch,
): Promise<QaBoardFinding> => {
  if (draft.issueNumber === null) return { ...draft, issueState: 'unavailable' }
  try {
    const response = await fetchFn(
      `https://api.github.com/repos/OpenAgentsInc/openagents/issues/${draft.issueNumber}`,
      {
        headers: {
          accept: 'application/vnd.github+json',
          'user-agent': 'openagents-qa-board',
        },
      },
    )
    if (!response.ok) return { ...draft, issueState: 'unavailable' }
    const body = record(await response.json())
    return {
      ...draft,
      issueState:
        body?.state === 'open' || body?.state === 'closed'
          ? body.state
          : 'unavailable',
    }
  } catch {
    return { ...draft, issueState: 'unavailable' }
  }
}

export async function buildQaBoardProjection(
  fetchFn: typeof fetch = fetch,
  servedAt: string = new Date().toISOString(),
): Promise<QaBoardProjection> {
  const observerArtifact = latestObserver()
  const swarmArtifact = latestSwarm()
  const checks =
    observerArtifact === null ? [] : observerChecks(observerArtifact)
  const lanes = swarmArtifact === null ? [] : swarmLanes(swarmArtifact)
  const findings = await Promise.all(
    reportFindingDrafts().map(draft => issueState(draft, fetchFn)),
  )

  const runAt = observerArtifact === null ? null : text(observerArtifact.runAt)
  const completedAt =
    swarmArtifact === null ? null : text(swarmArtifact.completedAt)
  const observerSummary =
    observerArtifact === null ? null : record(observerArtifact.summary)

  return {
    schema: 'openagents.qa.board.v1',
    servedAt,
    sources: {
      issues:
        findings.length === 0
          ? 'empty'
          : findings.every(finding => finding.issueState === 'unavailable')
            ? 'unavailable'
            : 'ok',
      observer:
        observerArtifact === null
          ? 'unavailable'
          : checks.length === 0
            ? 'empty'
            : 'ok',
      swarm:
        swarmArtifact === null
          ? 'unavailable'
          : lanes.length === 0
            ? 'empty'
            : 'ok',
    },
    observer:
      observerArtifact === null || runAt === null
        ? null
        : {
            runAt,
            checks,
            summary: {
              drift:
                finite(observerSummary?.drift) ??
                checks.filter(check => check.status === 'drift').length,
              pass:
                finite(observerSummary?.pass) ??
                checks.filter(check => check.status === 'pass').length,
              total: finite(observerSummary?.checksTotal) ?? checks.length,
              unrunnable:
                finite(observerSummary?.unrunnable) ??
                checks.filter(check => check.status === 'unrunnable').length,
            },
          },
    swarm:
      swarmArtifact === null || completedAt === null
        ? null
        : {
            baseSha: text(swarmArtifact.baseSha) ?? 'unknown',
            completedAt,
            lanes,
            runRef: text(swarmArtifact.runRef) ?? 'unknown',
            verdict: swarmArtifact.verdict === 'pass' ? 'pass' : 'findings',
          },
    findings: findings.filter(finding => finding.issueState !== 'closed'),
  }
}

export async function routeQaBoardRequest(
  request: Request,
  fetchFn: typeof fetch = fetch,
): Promise<Response | undefined> {
  const url = new URL(request.url)
  if (url.pathname !== '/api/public/qa-board') return undefined
  if (request.method !== 'GET') {
    return Response.json(
      { error: 'method_not_allowed' },
      { status: 405, headers: { allow: 'GET' } },
    )
  }
  return Response.json(await buildQaBoardProjection(fetchFn), {
    headers: {
      'cache-control': 'public, max-age=30, stale-while-revalidate=120',
    },
  })
}
