export type ForgeSessionSource = 'bridge' | 'claude' | 'codex' | 'pylon'

export type ForgeSessionState =
  | 'cancelled'
  | 'completed'
  | 'failed'
  | 'queued'
  | 'running'
  | 'unknown'

export type ForgeSessionNavigationStatus =
  | 'active'
  | 'attention'
  | 'complete'
  | 'empty'
  | 'queued'

export type ForgeSessionNavigationAction = 'cancel' | 'fork' | 'resume' | 'rewind'

export type ForgeSessionActionState = Readonly<{
  action: ForgeSessionNavigationAction
  availability: 'unavailable'
  blockerRefs: ReadonlyArray<string>
}>

export type ForgeSessionSummaryInput = Readonly<{
  artifactRefs?: ReadonlyArray<string>
  bridgeRefs?: ReadonlyArray<string>
  checkpointRefs?: ReadonlyArray<string>
  eventRefs?: ReadonlyArray<string>
  observedAt?: string | null
  sessionRef: string
  state?: ForgeSessionState
  title?: string | null
}>

export type ForgeSessionNavigationInput = Readonly<{
  bridgeSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  claudeSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  codexSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  generatedAt: string
  localPylonSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  workOrderRef: string
}>

export type ForgeSessionNavigationItem = Readonly<{
  actions: Readonly<Record<ForgeSessionNavigationAction, ForgeSessionActionState>>
  artifactRefs: ReadonlyArray<string>
  bridgeRefs: ReadonlyArray<string>
  checkpointRefs: ReadonlyArray<string>
  eventRefs: ReadonlyArray<string>
  observedAt: string | null
  sessionRef: string
  source: ForgeSessionSource
  state: ForgeSessionState
  title: string
}>

export type ForgeSessionNavigationView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  generatedAt: string
  items: ReadonlyArray<ForgeSessionNavigationItem>
  omittedUnsafeRefCount: number
  status: ForgeSessionNavigationStatus
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SAFE_SESSION_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_SESSION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|transcript)/i,
  /private[-_ ](?:repo|content|source|transcript)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_SESSION_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SESSION_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const blockerRef = (scopeRef: string, suffix: string): string =>
  `forge-session-navigation-blocker:${scopeRef}:${suffix}`

const sourceTitle = (source: ForgeSessionSource): string =>
  source === 'pylon'
    ? 'Pylon session'
    : source === 'codex'
      ? 'Codex session'
      : source === 'claude'
        ? 'Claude session'
        : 'Bridge session'

const controlActionState = (
  sessionRef: string,
  action: ForgeSessionNavigationAction,
): ForgeSessionActionState => ({
  action,
  availability: 'unavailable',
  blockerRefs: [blockerRef(sessionRef, `${action}-control-verb-unavailable`)],
})

const controlActions = (
  sessionRef: string,
): Readonly<Record<ForgeSessionNavigationAction, ForgeSessionActionState>> => ({
  cancel: controlActionState(sessionRef, 'cancel'),
  fork: controlActionState(sessionRef, 'fork'),
  resume: controlActionState(sessionRef, 'resume'),
  rewind: controlActionState(sessionRef, 'rewind'),
})

const stateRank = (state: ForgeSessionState): number =>
  state === 'running'
    ? 0
    : state === 'queued'
      ? 1
      : state === 'failed' || state === 'cancelled'
        ? 2
        : state === 'completed'
          ? 3
          : 4

const observedTime = (observedAt: string | null): number => {
  const parsed = observedAt === null ? Number.NaN : Date.parse(observedAt)

  return Number.isFinite(parsed) ? parsed : 0
}

const statusForItems = (
  items: ReadonlyArray<ForgeSessionNavigationItem>,
): ForgeSessionNavigationStatus => {
  if (items.length === 0) {
    return 'empty'
  }

  if (items.some(item => item.state === 'running')) {
    return 'active'
  }

  if (items.some(item => item.state === 'failed' || item.state === 'cancelled')) {
    return 'attention'
  }

  if (items.some(item => item.state === 'queued')) {
    return 'queued'
  }

  return items.every(item => item.state === 'completed') ? 'complete' : 'attention'
}

const normalizeSessions = (
  source: ForgeSessionSource,
  sessions: ReadonlyArray<ForgeSessionSummaryInput> | undefined,
): Readonly<{ items: ReadonlyArray<ForgeSessionNavigationItem>; omitted: number }> =>
  (sessions ?? []).reduce<{
    items: ReadonlyArray<ForgeSessionNavigationItem>
    omitted: number
  }>(
    (state, session) => {
      const sessionRef = safeRef(session.sessionRef)
      const artifactRefs = safeRefs(session.artifactRefs)
      const bridgeRefs = safeRefs(session.bridgeRefs)
      const checkpointRefs = safeRefs(session.checkpointRefs)
      const eventRefs = safeRefs(session.eventRefs)
      const omitted =
        artifactRefs.omittedUnsafeRefCount +
        bridgeRefs.omittedUnsafeRefCount +
        checkpointRefs.omittedUnsafeRefCount +
        eventRefs.omittedUnsafeRefCount +
        (sessionRef === null ? 1 : 0)

      if (sessionRef === null) {
        return {
          items: state.items,
          omitted: state.omitted + omitted,
        }
      }

      return {
        items: [
          ...state.items,
          {
            actions: controlActions(sessionRef),
            artifactRefs: artifactRefs.refs,
            bridgeRefs: bridgeRefs.refs,
            checkpointRefs: checkpointRefs.refs,
            eventRefs: eventRefs.refs,
            observedAt: session.observedAt ?? null,
            sessionRef,
            source,
            state: session.state ?? 'unknown',
            title: session.title?.trim() || sourceTitle(source),
          },
        ],
        omitted: state.omitted + omitted,
      }
    },
    { items: [], omitted: 0 },
  )

export const projectForgeSessionNavigation = (
  input: ForgeSessionNavigationInput,
): ForgeSessionNavigationView => {
  const localPylon = normalizeSessions('pylon', input.localPylonSessions)
  const codex = normalizeSessions('codex', input.codexSessions)
  const claude = normalizeSessions('claude', input.claudeSessions)
  const bridge = normalizeSessions('bridge', input.bridgeSessions)
  const items = [
    ...localPylon.items,
    ...codex.items,
    ...claude.items,
    ...bridge.items,
  ].sort(
    (left, right) =>
      stateRank(left.state) - stateRank(right.state) ||
      observedTime(right.observedAt) - observedTime(left.observedAt) ||
      left.sessionRef.localeCompare(right.sessionRef),
  )
  const omittedUnsafeRefCount =
    localPylon.omitted + codex.omitted + claude.omitted + bridge.omitted
  const blockerRefs = safeRefs([
    ...(items.length === 0
      ? [blockerRef(input.workOrderRef, 'no-session-summaries')]
      : []),
    ...(omittedUnsafeRefCount > 0
      ? [blockerRef(input.workOrderRef, 'unsafe-session-material-omitted')]
      : []),
  ]).refs

  return {
    blockerRefs,
    generatedAt: input.generatedAt,
    items,
    omittedUnsafeRefCount,
    status: statusForItems(items),
    workOrderRef: input.workOrderRef,
  }
}
