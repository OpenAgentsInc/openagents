import type {
  AutopilotWorkCompanionSurface,
  AutopilotWorkCompanionSurfaceEntry,
  AutopilotWorkCompanionSurfaceFreshness,
  AutopilotWorkCompanionSurfaceState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeCompanionSurfaceStatus =
  | 'blocked'
  | 'empty'
  | 'lagged'
  | 'read_only'
  | 'ready'
  | 'stale'
  | 'unknown'
  | 'waiting'

export type ForgeCompanionSurfaceAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  approvalResolveAuthority: false
  cancelRunAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  instructionQueueAuthority: false
  interruptRunAuthority: false
  notificationSendAuthority: false
  offlineActionQueueAuthority: false
  pauseRunAuthority: false
  privateLogStreamingAuthority: false
  publicClaimAuthority: false
  resumeRunAuthority: false
  sessionMutationAuthority: false
  settlementAuthority: false
  spawnRunAuthority: false
  terminalOpenAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeCompanionSurfaceItem = Readonly<{
  actionRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  closeoutRefs: ReadonlyArray<string>
  companionRef: string
  cursorRefs: ReadonlyArray<string>
  decisionRefs: ReadonlyArray<string>
  deliveryTierRefs: ReadonlyArray<string>
  freshness: AutopilotWorkCompanionSurfaceFreshness
  idempotencyRefs: ReadonlyArray<string>
  lagRefs: ReadonlyArray<string>
  notificationRefs: ReadonlyArray<string>
  pairingRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  progressRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  runRefs: ReadonlyArray<string>
  sessionRefs: ReadonlyArray<string>
  state: AutopilotWorkCompanionSurfaceState
  streamRefs: ReadonlyArray<string>
  surfaceRefs: ReadonlyArray<string>
}>

export type ForgeCompanionSurfaceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkCompanionSurfaceEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeCompanionSurfaceCounts = Readonly<{
  blocked: number
  offline: number
  readOnly: number
  ready: number
  total: number
  waiting: number
}>

export type ForgeCompanionSurfaceView = Readonly<{
  authority: ForgeCompanionSurfaceAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeCompanionSurfaceCounts
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeCompanionSurfaceStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_COMPANION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:action|artifact|audio|command|content|decision|file|input|log|notification|output|payload|progress|prompt|session|shell|source|stream|terminal|transcript)/i,
  /private[-_ ](?:action|artifact|content|decision|file|log|payload|progress|prompt|session|source|stream|terminal|workspace)/i,
  /companion[-_ ](?:body|payload|private|prompt|session)/i,
  /mobile[-_ ](?:payload|private|prompt|session|token)/i,
  /terminal[-_ ](?:output|payload|session|stream|transcript)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)[A-Za-z][A-Za-z0-9+.-]*:\/\//,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeCompanionSurfaceAuthority = {
  acceptedOutcomeAuthority: false,
  approvalResolveAuthority: false,
  cancelRunAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  instructionQueueAuthority: false,
  interruptRunAuthority: false,
  notificationSendAuthority: false,
  offlineActionQueueAuthority: false,
  pauseRunAuthority: false,
  privateLogStreamingAuthority: false,
  publicClaimAuthority: false,
  resumeRunAuthority: false,
  sessionMutationAuthority: false,
  settlementAuthority: false,
  spawnRunAuthority: false,
  terminalOpenAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_COMPANION_MARKERS.some(marker => marker.test(trimmed))
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

const safeOptionalRef = (
  value: string | null | undefined,
): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-companion-surface-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkCompanionSurfaceEntry,
): Readonly<{
  entry: ForgeCompanionSurfaceItem | null
  omittedUnsafeRefCount: number
}> => {
  const actionRefs = safeRefs(entry.actionRefs)
  const artifactRefs = safeRefs(entry.artifactRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const budgetRefs = safeRefs(entry.budgetRefs)
  const capabilityRefs = safeRefs(entry.capabilityRefs)
  const closeoutRefs = safeRefs(entry.closeoutRefs)
  const companionRef = safeOptionalRef(entry.companionRef)
  const cursorRefs = safeRefs(entry.cursorRefs)
  const decisionRefs = safeRefs(entry.decisionRefs)
  const deliveryTierRefs = safeRefs(entry.deliveryTierRefs)
  const idempotencyRefs = safeRefs(entry.idempotencyRefs)
  const lagRefs = safeRefs(entry.lagRefs)
  const notificationRefs = safeRefs(entry.notificationRefs)
  const pairingRefs = safeRefs(entry.pairingRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const progressRefs = safeRefs(entry.progressRefs)
  const receiptRefs = safeRefs(entry.receiptRefs)
  const runRefs = safeRefs(entry.runRefs)
  const sessionRefs = safeRefs(entry.sessionRefs)
  const streamRefs = safeRefs(entry.streamRefs)
  const surfaceRefs = safeRefs(entry.surfaceRefs)
  const omittedUnsafeRefCount =
    actionRefs.omittedUnsafeRefCount +
    artifactRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    closeoutRefs.omittedUnsafeRefCount +
    companionRef.omittedUnsafeRefCount +
    cursorRefs.omittedUnsafeRefCount +
    decisionRefs.omittedUnsafeRefCount +
    deliveryTierRefs.omittedUnsafeRefCount +
    idempotencyRefs.omittedUnsafeRefCount +
    lagRefs.omittedUnsafeRefCount +
    notificationRefs.omittedUnsafeRefCount +
    pairingRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    progressRefs.omittedUnsafeRefCount +
    receiptRefs.omittedUnsafeRefCount +
    runRefs.omittedUnsafeRefCount +
    sessionRefs.omittedUnsafeRefCount +
    streamRefs.omittedUnsafeRefCount +
    surfaceRefs.omittedUnsafeRefCount

  return companionRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          actionRefs: actionRefs.refs,
          artifactRefs: artifactRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetRefs: budgetRefs.refs,
          capabilityRefs: capabilityRefs.refs,
          closeoutRefs: closeoutRefs.refs,
          companionRef: companionRef.ref,
          cursorRefs: cursorRefs.refs,
          decisionRefs: decisionRefs.refs,
          deliveryTierRefs: deliveryTierRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          idempotencyRefs: idempotencyRefs.refs,
          lagRefs: lagRefs.refs,
          notificationRefs: notificationRefs.refs,
          pairingRefs: pairingRefs.refs,
          policyRefs: policyRefs.refs,
          progressRefs: progressRefs.refs,
          receiptRefs: receiptRefs.refs,
          runRefs: runRefs.refs,
          sessionRefs: sessionRefs.refs,
          state: entry.state,
          streamRefs: streamRefs.refs,
          surfaceRefs: surfaceRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>,
): ForgeCompanionSurfaceCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  offline: entries.filter(entry => entry.state === 'offline').length,
  readOnly: entries.filter(entry => entry.state === 'read_only').length,
  ready: entries.filter(entry => entry.state === 'ready').length,
  total: entries.length,
  waiting: entries.filter(entry => entry.state === 'waiting').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.freshness === 'stale' || entry.freshness === 'lagged') &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(
        workOrderRef,
        `${entry.freshness}-companion-evidence:${entry.companionRef}`,
      ),
    )

const streamCursorBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.streamRefs.length > 0 &&
        entry.cursorRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `companion-stream-cursor-missing:${entry.companionRef}`),
    )

const actionBoundaryBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>,
): ReadonlyArray<string> =>
  entries.flatMap(entry => {
    if (entry.actionRefs.length === 0 || entry.blockerRefs.length > 0) {
      return []
    }

    const missingBoundary =
      entry.capabilityRefs.length === 0 ||
      entry.policyRefs.length === 0 ||
      entry.pairingRefs.length === 0 ||
      entry.idempotencyRefs.length === 0
    const missingReceipt = entry.receiptRefs.length === 0

    return [
      ...(missingBoundary
        ? [
            blockerRef(
              workOrderRef,
              `companion-action-boundary-missing:${entry.companionRef}`,
            ),
          ]
        : []),
      ...(missingReceipt
        ? [
            blockerRef(
              workOrderRef,
              `companion-action-receipt-missing:${entry.companionRef}`,
            ),
          ]
        : []),
    ]
  })

const statusForView = (
  entries: ReadonlyArray<ForgeCompanionSurfaceItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeCompanionSurfaceStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'lagged')) {
    return 'lagged'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'waiting')) {
    return 'waiting'
  }

  if (entries.every(entry => entry.state === 'read_only')) {
    return 'read_only'
  }

  return entries.every(entry => entry.state === 'ready' || entry.state === 'read_only')
    ? 'ready'
    : 'unknown'
}

export const projectForgeCompanionSurface = (
  input: ForgeCompanionSurfaceInput,
): ForgeCompanionSurfaceView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.freshness.localeCompare(right.freshness) ||
        left.companionRef.localeCompare(right.companionRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...streamCursorBlockers(input.workOrderRef, entries),
      ...actionBoundaryBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-companion-surface-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-companion-surface-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeCompanionSurfaceInput = (
  work: AutopilotWorkProjection,
): ForgeCompanionSurfaceInput => {
  const source: AutopilotWorkCompanionSurface | undefined = work.companionSurface

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
