import type {
  AutopilotWorkProjection,
  AutopilotWorkRemoteSessionBridge,
  AutopilotWorkRemoteSessionBridgeEntry,
  AutopilotWorkRemoteSessionBridgeFreshness,
  AutopilotWorkRemoteSessionBridgeState,
} from '../model'

export type ForgeRemoteSessionBridgeStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'reconnecting'
  | 'stale'
  | 'unknown'

export type ForgeRemoteSessionBridgeAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  logStreamingAuthority: false
  publicClaimAuthority: false
  remoteCommandAuthority: false
  remoteHostInspectAuthority: false
  remoteSessionControlAuthority: false
  remoteSessionOpenAuthority: false
  remoteSessionReconnectAuthority: false
  remoteSessionTerminateAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeRemoteSessionBridgeItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  bridgeRef: string
  controllerRefs: ReadonlyArray<string>
  freshness: AutopilotWorkRemoteSessionBridgeFreshness
  heartbeatRefs: ReadonlyArray<string>
  permissionRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  protocolRefs: ReadonlyArray<string>
  reconnectRefs: ReadonlyArray<string>
  sessionRefs: ReadonlyArray<string>
  state: AutopilotWorkRemoteSessionBridgeState
  transportRefs: ReadonlyArray<string>
}>

export type ForgeRemoteSessionBridgeInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkRemoteSessionBridgeEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeRemoteSessionBridgeCounts = Readonly<{
  blocked: number
  connected: number
  ready: number
  reconnecting: number
  total: number
}>

export type ForgeRemoteSessionBridgeView = Readonly<{
  authority: ForgeRemoteSessionBridgeAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeRemoteSessionBridgeCounts
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeRemoteSessionBridgeStatus
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
const PRIVATE_REMOTE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:bridge|command|content|file|host|input|log|output|protocol|remote|session|shell|source|stderr|stdout|transport|transcript)/i,
  /private[-_ ](?:bridge|command|content|file|host|remote|session|source|transport|workspace)/i,
  /remote[-_ ](?:command|host|log|output|payload|shell|stdout|stderr)/i,
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

const authority: ForgeRemoteSessionBridgeAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  logStreamingAuthority: false,
  publicClaimAuthority: false,
  remoteCommandAuthority: false,
  remoteHostInspectAuthority: false,
  remoteSessionControlAuthority: false,
  remoteSessionOpenAuthority: false,
  remoteSessionReconnectAuthority: false,
  remoteSessionTerminateAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_REMOTE_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-remote-session-bridge-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkRemoteSessionBridgeEntry,
): Readonly<{
  entry: ForgeRemoteSessionBridgeItem | null
  omittedUnsafeRefCount: number
}> => {
  const bridgeRef = safeOptionalRef(entry.bridgeRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const controllerRefs = safeRefs(entry.controllerRefs)
  const heartbeatRefs = safeRefs(entry.heartbeatRefs)
  const permissionRefs = safeRefs(entry.permissionRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const protocolRefs = safeRefs(entry.protocolRefs)
  const reconnectRefs = safeRefs(entry.reconnectRefs)
  const sessionRefs = safeRefs(entry.sessionRefs)
  const transportRefs = safeRefs(entry.transportRefs)
  const omittedUnsafeRefCount =
    bridgeRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    controllerRefs.omittedUnsafeRefCount +
    heartbeatRefs.omittedUnsafeRefCount +
    permissionRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    protocolRefs.omittedUnsafeRefCount +
    reconnectRefs.omittedUnsafeRefCount +
    sessionRefs.omittedUnsafeRefCount +
    transportRefs.omittedUnsafeRefCount

  return bridgeRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          bridgeRef: bridgeRef.ref,
          controllerRefs: controllerRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          heartbeatRefs: heartbeatRefs.refs,
          permissionRefs: permissionRefs.refs,
          policyRefs: policyRefs.refs,
          protocolRefs: protocolRefs.refs,
          reconnectRefs: reconnectRefs.refs,
          sessionRefs: sessionRefs.refs,
          state: entry.state,
          transportRefs: transportRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
): ForgeRemoteSessionBridgeCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  connected: entries.filter(entry => entry.state === 'connected').length,
  ready: entries.filter(entry => entry.state === 'ready').length,
  reconnecting: entries.filter(entry => entry.state === 'reconnecting').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-remote-bridge-evidence:${entry.bridgeRef}`))

const readinessBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'ready' || entry.state === 'connected') &&
        entry.blockerRefs.length === 0 &&
        (entry.transportRefs.length === 0 ||
          entry.protocolRefs.length === 0 ||
          entry.policyRefs.length === 0),
    )
    .map(entry => blockerRef(workOrderRef, `remote-bridge-readiness-missing:${entry.bridgeRef}`))

const reconnectBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'reconnecting' &&
        entry.reconnectRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `reconnect-ref-missing:${entry.bridgeRef}`))

const controllerBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.controllerRefs.length > 0 &&
        entry.permissionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `controller-permission-ref-missing:${entry.bridgeRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeRemoteSessionBridgeItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeRemoteSessionBridgeStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'reconnecting')) {
    return 'reconnecting'
  }

  return entries.every(entry => entry.state === 'ready' || entry.state === 'connected')
    ? 'ready'
    : 'unknown'
}

export const projectForgeRemoteSessionBridge = (
  input: ForgeRemoteSessionBridgeInput,
): ForgeRemoteSessionBridgeView => {
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
        left.bridgeRef.localeCompare(right.bridgeRef),
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
      ...readinessBlockers(input.workOrderRef, entries),
      ...reconnectBlockers(input.workOrderRef, entries),
      ...controllerBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-remote-session-bridge-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-remote-session-bridge-material-omitted')]),
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

export const buildForgeRemoteSessionBridgeInput = (
  work: AutopilotWorkProjection,
): ForgeRemoteSessionBridgeInput => {
  const source: AutopilotWorkRemoteSessionBridge | undefined =
    work.remoteSessionBridge

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
