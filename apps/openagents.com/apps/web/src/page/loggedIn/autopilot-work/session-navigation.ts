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
export type ForgeSessionActionAvailability =
  | 'available'
  | 'blocked'
  | 'stale'
  | 'unavailable'
export type ForgeSessionControlFreshness = 'fresh' | 'stale'
export type ForgeSessionControlOutcome = 'applied' | 'blocked' | 'queued' | 'stale'

export type ForgeSessionActionState = Readonly<{
  action: ForgeSessionNavigationAction
  authorityRefs: ReadonlyArray<string>
  availability: ForgeSessionActionAvailability
  blockerRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  requestRef: string
}>

export type ForgeSessionSummaryInput = Readonly<{
  artifactRefs?: ReadonlyArray<string>
  bridgeRefs?: ReadonlyArray<string>
  checkpointRefs?: ReadonlyArray<string>
  controlAuthorityRefs?: ReadonlyArray<string>
  controlBlockerRefs?: ReadonlyArray<string>
  controlFreshness?: ForgeSessionControlFreshness
  controlPolicyRefs?: ReadonlyArray<string>
  eventRefs?: ReadonlyArray<string>
  observedAt?: string | null
  sessionRef: string
  state?: ForgeSessionState
  supportedControlActions?: ReadonlyArray<ForgeSessionNavigationAction>
  title?: string | null
}>

export type ForgeSessionControlReceiptInput = Readonly<{
  action: ForgeSessionNavigationAction
  actorRef: string
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  outcome: ForgeSessionControlOutcome
  provenanceRefs?: ReadonlyArray<string>
  publicSafe: boolean
  receiptRef: string
  requestRef: string
  sessionRef: string
}>

export type ForgeSessionNavigationInput = Readonly<{
  bridgeSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  claudeSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  codexSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  controlReceipts?: ReadonlyArray<ForgeSessionControlReceiptInput>
  generatedAt: string
  localPylonSessions?: ReadonlyArray<ForgeSessionSummaryInput>
  workOrderRef: string
}>

export type ForgeSessionNavigationItem = Readonly<{
  actions: Readonly<Record<ForgeSessionNavigationAction, ForgeSessionActionState>>
  artifactRefs: ReadonlyArray<string>
  bridgeRefs: ReadonlyArray<string>
  checkpointRefs: ReadonlyArray<string>
  controlAuthorityRefs: ReadonlyArray<string>
  controlBlockerRefs: ReadonlyArray<string>
  controlPolicyRefs: ReadonlyArray<string>
  eventRefs: ReadonlyArray<string>
  observedAt: string | null
  sessionRef: string
  source: ForgeSessionSource
  state: ForgeSessionState
  title: string
}>

export type ForgeSessionControlReceiptItem = Readonly<{
  action: ForgeSessionNavigationAction
  actorRef: string
  blockerRefs: ReadonlyArray<string>
  generatedAt: string
  outcome: ForgeSessionControlOutcome
  provenanceRefs: ReadonlyArray<string>
  publicSafe: true
  receiptRef: string
  requestRef: string
  sessionRef: string
}>

export type ForgeSessionNavigationView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  controlReceipts: ReadonlyArray<ForgeSessionControlReceiptItem>
  generatedAt: string
  items: ReadonlyArray<ForgeSessionNavigationItem>
  omittedUnsafeRefCount: number
  status: ForgeSessionNavigationStatus
  workOrderRef: string
}>

export type ForgeSessionSummaryReceiptItem = Readonly<{
  artifactRefs: ReadonlyArray<string>
  bridgeRefs: ReadonlyArray<string>
  checkpointRefs: ReadonlyArray<string>
  controlBlockerRefs: ReadonlyArray<string>
  eventRefs: ReadonlyArray<string>
  observedAt: string | null
  sessionRef: string
  source: ForgeSessionSource
  state: ForgeSessionState
}>

export type ForgeSessionSummaryReceipt = Readonly<{
  authority: Readonly<{
    cancelControlAuthority: false
    forkControlAuthority: false
    resumeControlAuthority: false
    rewindControlAuthority: false
    runtimeExecutionProof: false
  }>
  blockerRefs: ReadonlyArray<string>
  controlBlockerRefs: ReadonlyArray<string>
  countsBySource: Readonly<Record<ForgeSessionSource, number>>
  countsByState: Readonly<Record<ForgeSessionState, number>>
  exportedAt: string
  generatedAt: string
  itemCount: number
  items: ReadonlyArray<ForgeSessionSummaryReceiptItem>
  omittedUnsafeRefCount: number
  provenance: 'refs_only_session_summary'
  publicSafe: true
  receiptKind: 'forge_session_summary_export.v1'
  receiptRef: string
  safeEvidenceRefs: ReadonlyArray<string>
  safeSessionRefs: ReadonlyArray<string>
  status: ForgeSessionNavigationStatus
  workOrderRef: string
}>

export type ForgeSessionSummaryReceiptInput = Readonly<{
  exportedAt: string
  view: ForgeSessionNavigationView
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SESSION_SOURCES: ReadonlyArray<ForgeSessionSource> = [
  'bridge',
  'claude',
  'codex',
  'pylon',
]
const SESSION_STATES: ReadonlyArray<ForgeSessionState> = [
  'cancelled',
  'completed',
  'failed',
  'queued',
  'running',
  'unknown',
]
const SESSION_CONTROL_ACTIONS: ReadonlyArray<ForgeSessionNavigationAction> = [
  'cancel',
  'fork',
  'resume',
  'rewind',
]
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

const safeTitle = (value: string | null | undefined): string | null => {
  const trimmed = value?.trim()

  if (trimmed === undefined || trimmed === '') {
    return null
  }

  return PRIVATE_SESSION_MARKERS.some(marker => marker.test(trimmed))
    ? null
    : trimmed.slice(0, 120)
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
  control: Readonly<{
    authorityRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    freshness: ForgeSessionControlFreshness
    policyRefs: ReadonlyArray<string>
    supportedActions: ReadonlyArray<ForgeSessionNavigationAction>
  }>,
): ForgeSessionActionState => ({
  action,
  authorityRefs: control.authorityRefs,
  availability: control.supportedActions.includes(action)
    ? control.freshness === 'stale'
      ? 'stale'
      : control.authorityRefs.length === 0 ||
          control.policyRefs.length === 0 ||
          control.blockerRefs.length > 0
        ? 'blocked'
        : 'available'
    : 'unavailable',
  blockerRefs: Array.from(
    new Set([
      ...(!control.supportedActions.includes(action)
        ? [blockerRef(sessionRef, `${action}-control-verb-unavailable`)]
        : []),
      ...(control.supportedActions.includes(action) && control.freshness === 'stale'
        ? [blockerRef(sessionRef, `${action}-stale-session-ref`)]
        : []),
      ...(control.supportedActions.includes(action) &&
      control.authorityRefs.length === 0
        ? [blockerRef(sessionRef, `${action}-missing-authority-ref`)]
        : []),
      ...(control.supportedActions.includes(action) && control.policyRefs.length === 0
        ? [blockerRef(sessionRef, `${action}-missing-policy-ref`)]
        : []),
      ...(control.supportedActions.includes(action) ? control.blockerRefs : []),
    ]),
  ),
  policyRefs: control.policyRefs,
  requestRef: `forge-session-control-request:${sessionRef}:${action}`,
})

const controlActions = (
  sessionRef: string,
  control: Readonly<{
    authorityRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    freshness: ForgeSessionControlFreshness
    policyRefs: ReadonlyArray<string>
    supportedActions: ReadonlyArray<ForgeSessionNavigationAction>
  }>,
): Readonly<Record<ForgeSessionNavigationAction, ForgeSessionActionState>> => ({
  cancel: controlActionState(sessionRef, 'cancel', control),
  fork: controlActionState(sessionRef, 'fork', control),
  resume: controlActionState(sessionRef, 'resume', control),
  rewind: controlActionState(sessionRef, 'rewind', control),
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
      const title = safeTitle(session.title)
      const artifactRefs = safeRefs(session.artifactRefs)
      const bridgeRefs = safeRefs(session.bridgeRefs)
      const checkpointRefs = safeRefs(session.checkpointRefs)
      const eventRefs = safeRefs(session.eventRefs)
      const controlAuthorityRefs = safeRefs(session.controlAuthorityRefs)
      const controlBlockerRefs = safeRefs(session.controlBlockerRefs)
      const controlPolicyRefs = safeRefs(session.controlPolicyRefs)
      const supportedControlActions = Array.from(
        new Set(
          (session.supportedControlActions ?? []).filter(action =>
            SESSION_CONTROL_ACTIONS.includes(action),
          ),
        ),
      )
      const omitted =
        artifactRefs.omittedUnsafeRefCount +
        bridgeRefs.omittedUnsafeRefCount +
        checkpointRefs.omittedUnsafeRefCount +
        eventRefs.omittedUnsafeRefCount +
        controlAuthorityRefs.omittedUnsafeRefCount +
        controlBlockerRefs.omittedUnsafeRefCount +
        controlPolicyRefs.omittedUnsafeRefCount +
        (sessionRef === null ? 1 : 0) +
        (session.title !== undefined &&
        session.title !== null &&
        session.title.trim() !== '' &&
        title === null
          ? 1
          : 0)

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
            actions: controlActions(sessionRef, {
              authorityRefs: controlAuthorityRefs.refs,
              blockerRefs: controlBlockerRefs.refs,
              freshness: session.controlFreshness ?? 'fresh',
              policyRefs: controlPolicyRefs.refs,
              supportedActions: supportedControlActions,
            }),
            artifactRefs: artifactRefs.refs,
            bridgeRefs: bridgeRefs.refs,
            checkpointRefs: checkpointRefs.refs,
            controlAuthorityRefs: controlAuthorityRefs.refs,
            controlBlockerRefs: controlBlockerRefs.refs,
            controlPolicyRefs: controlPolicyRefs.refs,
            eventRefs: eventRefs.refs,
            observedAt: session.observedAt ?? null,
            sessionRef,
            source,
            state: session.state ?? 'unknown',
            title: title ?? sourceTitle(source),
          },
        ],
        omitted: state.omitted + omitted,
      }
    },
    { items: [], omitted: 0 },
  )

const normalizeControlReceipts = (
  receipts: ReadonlyArray<ForgeSessionControlReceiptInput> | undefined,
): Readonly<{
  items: ReadonlyArray<ForgeSessionControlReceiptItem>
  omitted: number
}> =>
  (receipts ?? []).reduce<{
    items: ReadonlyArray<ForgeSessionControlReceiptItem>
    omitted: number
  }>(
    (state, receipt) => {
      const sessionRef = safeRef(receipt.sessionRef)
      const requestRef = safeRef(receipt.requestRef)
      const receiptRef = safeRef(receipt.receiptRef)
      const actorRef = safeRef(receipt.actorRef)
      const blockerRefs = safeRefs(receipt.blockerRefs)
      const provenanceRefs = safeRefs(receipt.provenanceRefs)
      const omitted =
        blockerRefs.omittedUnsafeRefCount +
        provenanceRefs.omittedUnsafeRefCount +
        (sessionRef === null ? 1 : 0) +
        (requestRef === null ? 1 : 0) +
        (receiptRef === null ? 1 : 0) +
        (actorRef === null ? 1 : 0) +
        (receipt.publicSafe ? 0 : 1)

      if (
        !receipt.publicSafe ||
        sessionRef === null ||
        requestRef === null ||
        receiptRef === null ||
        actorRef === null
      ) {
        return {
          items: state.items,
          omitted: state.omitted + omitted,
        }
      }

      return {
        items: [
          ...state.items,
          {
            action: receipt.action,
            actorRef,
            blockerRefs: blockerRefs.refs,
            generatedAt: receipt.generatedAt,
            outcome: receipt.outcome,
            provenanceRefs: provenanceRefs.refs,
            publicSafe: true,
            receiptRef,
            requestRef,
            sessionRef,
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
  const controlReceipts = normalizeControlReceipts(input.controlReceipts)
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
    localPylon.omitted +
    codex.omitted +
    claude.omitted +
    bridge.omitted +
    controlReceipts.omitted
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
    controlReceipts: Array.from(controlReceipts.items).sort(
      (left, right) =>
        observedTime(right.generatedAt) - observedTime(left.generatedAt) ||
        left.receiptRef.localeCompare(right.receiptRef),
    ),
    generatedAt: input.generatedAt,
    items,
    omittedUnsafeRefCount,
    status: statusForItems(items),
    workOrderRef: input.workOrderRef,
  }
}

const actionBlockerRefs = (
  item: ForgeSessionNavigationItem,
): ReadonlyArray<string> =>
  Array.from(
    new Set(
      Object.values(item.actions).flatMap(actionState => actionState.blockerRefs),
    ),
  )

const evidenceRefs = (item: {
  artifactRefs: ReadonlyArray<string>
  bridgeRefs: ReadonlyArray<string>
  checkpointRefs: ReadonlyArray<string>
  eventRefs: ReadonlyArray<string>
}): ReadonlyArray<string> => [
  ...item.artifactRefs,
  ...item.bridgeRefs,
  ...item.checkpointRefs,
  ...item.eventRefs,
]

const slugRefPart = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '') || 'unknown'

const emptySourceCounts = (): Record<ForgeSessionSource, number> => ({
  bridge: 0,
  claude: 0,
  codex: 0,
  pylon: 0,
})

const emptyStateCounts = (): Record<ForgeSessionState, number> => ({
  cancelled: 0,
  completed: 0,
  failed: 0,
  queued: 0,
  running: 0,
  unknown: 0,
})

export const projectForgeSessionSummaryReceipt = ({
  exportedAt,
  view,
}: ForgeSessionSummaryReceiptInput): ForgeSessionSummaryReceipt => {
  const safeWorkOrderRef = safeRef(view.workOrderRef) ?? 'unsafe-work-order-ref-omitted'
  const countsBySource = emptySourceCounts()
  const countsByState = emptyStateCounts()
  const items = view.items.map(item => {
    countsBySource[item.source] += 1
    countsByState[item.state] += 1

    return {
      artifactRefs: item.artifactRefs,
      bridgeRefs: item.bridgeRefs,
      checkpointRefs: item.checkpointRefs,
      controlBlockerRefs: actionBlockerRefs(item),
      eventRefs: item.eventRefs,
      observedAt: item.observedAt,
      sessionRef: item.sessionRef,
      source: item.source,
      state: item.state,
    }
  })
  const safeSessionRefs = Array.from(new Set(items.map(item => item.sessionRef)))
  const safeEvidenceRefs = Array.from(new Set(items.flatMap(evidenceRefs)))
  const controlBlockerRefs = Array.from(
    new Set(items.flatMap(item => item.controlBlockerRefs)),
  )
  const blockerRefs = Array.from(new Set([...view.blockerRefs, ...controlBlockerRefs]))
  const omittedUnsafeRefCount =
    view.omittedUnsafeRefCount + (safeWorkOrderRef === view.workOrderRef ? 0 : 1)

  SESSION_SOURCES.forEach(source => {
    countsBySource[source] = countsBySource[source] ?? 0
  })
  SESSION_STATES.forEach(state => {
    countsByState[state] = countsByState[state] ?? 0
  })

  return {
    authority: {
      cancelControlAuthority: false,
      forkControlAuthority: false,
      resumeControlAuthority: false,
      rewindControlAuthority: false,
      runtimeExecutionProof: false,
    },
    blockerRefs,
    controlBlockerRefs,
    countsBySource,
    countsByState,
    exportedAt,
    generatedAt: view.generatedAt,
    itemCount: items.length,
    items,
    omittedUnsafeRefCount,
    provenance: 'refs_only_session_summary',
    publicSafe: true,
    receiptKind: 'forge_session_summary_export.v1',
    receiptRef: `forge.session_summary_export.${slugRefPart(safeWorkOrderRef)}.${slugRefPart(exportedAt)}`,
    safeEvidenceRefs,
    safeSessionRefs,
    status: view.status,
    workOrderRef: safeWorkOrderRef,
  }
}
