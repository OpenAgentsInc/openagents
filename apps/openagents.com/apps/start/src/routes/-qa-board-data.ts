export const QA_BOARD_URL = '/api/public/qa-board'

export type QaSeverity = 'critical' | 'high' | 'medium' | 'low' | 'unclassified'
export type QaCheckStatus = 'pass' | 'drift' | 'unrunnable'
export type QaSourceState = 'ok' | 'empty' | 'unavailable'

export type QaObserverCheck = Readonly<{
  consecutiveDriftRuns: number
  durationMs: number
  id: string
  severityOnDrift: QaSeverity
  status: QaCheckStatus
  surface: string
}>

export type QaBoardFinding = Readonly<{
  issueNumber: number | null
  issueState: 'open' | 'closed' | 'unavailable'
  issueUrl: string | null
  severity: QaSeverity
  summary: string
  surface: string
}>

export type QaSwarmLane = Readonly<{
  id: string
  surface: string
  verdict: 'pass' | 'finding'
}>

export type QaBoardProjection = Readonly<{
  schema: 'openagents.qa.board.v1'
  servedAt: string
  sources: Readonly<{
    issues: QaSourceState
    observer: QaSourceState
    swarm: QaSourceState
  }>
  observer: Readonly<{
    runAt: string
    checks: ReadonlyArray<QaObserverCheck>
    summary: Readonly<{
      drift: number
      pass: number
      total: number
      unrunnable: number
    }>
  }> | null
  swarm: Readonly<{
    baseSha: string
    completedAt: string
    lanes: ReadonlyArray<QaSwarmLane>
    runRef: string
    verdict: 'pass' | 'findings'
  }> | null
  findings: ReadonlyArray<QaBoardFinding>
}>

export type Loadable<T> =
  | { readonly state: 'loading' }
  | { readonly state: 'ok'; readonly data: T }
  | { readonly state: 'unavailable'; readonly detail: string }

export const fetchQaBoard = async (
  fetchFn: typeof fetch = fetch,
): Promise<Loadable<QaBoardProjection>> => {
  try {
    const response = await fetchFn(QA_BOARD_URL, {
      headers: { accept: 'application/json' },
    })
    if (!response.ok) {
      return {
        state: 'unavailable',
        detail: `QA evidence endpoint returned HTTP ${response.status}.`,
      }
    }
    const value = (await response.json()) as Partial<QaBoardProjection>
    if (value.schema !== 'openagents.qa.board.v1') {
      return {
        state: 'unavailable',
        detail: 'QA evidence endpoint returned an unsupported projection.',
      }
    }
    return { state: 'ok', data: value as QaBoardProjection }
  } catch (error) {
    return {
      state: 'unavailable',
      detail:
        error instanceof Error
          ? error.message
          : 'QA evidence endpoint is unreachable.',
    }
  }
}

export const freshnessLabel = (
  timestamp: string,
  nowMs: number = Date.now(),
): string => {
  const observedMs = Date.parse(timestamp)
  if (!Number.isFinite(observedMs)) return 'Freshness unavailable'
  const minutes = Math.max(0, Math.floor((nowMs - observedMs) / 60_000))
  if (minutes < 1) return 'Updated less than a minute ago'
  if (minutes < 60) return `Updated ${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 48) return `Updated ${hours}h ago`
  return `Updated ${Math.floor(hours / 24)}d ago`
}
