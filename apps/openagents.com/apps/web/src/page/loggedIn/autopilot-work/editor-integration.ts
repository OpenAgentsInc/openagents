import type {
  AutopilotWorkEditorIntegration,
  AutopilotWorkEditorIntegrationEntry,
  AutopilotWorkEditorIntegrationFreshness,
  AutopilotWorkEditorIntegrationState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeEditorIntegrationStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeEditorIntegrationAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  editorAutomationAuthority: false
  editorCommandAuthority: false
  extensionInstallAuthority: false
  fileOpenAuthority: false
  fileReadAuthority: false
  fileWriteAuthority: false
  publicClaimAuthority: false
  selectionReadAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolExecutionAuthority: false
  toolRoutingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeEditorIntegrationItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  commandRefs: ReadonlyArray<string>
  deepLinkRefs: ReadonlyArray<string>
  diagnosticHandoffRefs: ReadonlyArray<string>
  diagnosticRefs: ReadonlyArray<string>
  editorRefs: ReadonlyArray<string>
  extensionRefs: ReadonlyArray<string>
  fileOpenRefs: ReadonlyArray<string>
  freshness: AutopilotWorkEditorIntegrationFreshness
  integrationRef: string
  policyRefs: ReadonlyArray<string>
  selectionRefs: ReadonlyArray<string>
  state: AutopilotWorkEditorIntegrationState
  statusRefs: ReadonlyArray<string>
  workspaceRefs: ReadonlyArray<string>
}>

export type ForgeEditorIntegrationInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkEditorIntegrationEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeEditorIntegrationCounts = Readonly<{
  blocked: number
  connected: number
  disconnected: number
  ready: number
  total: number
}>

export type ForgeEditorIntegrationView = Readonly<{
  authority: ForgeEditorIntegrationAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeEditorIntegrationCounts
  entries: ReadonlyArray<ForgeEditorIntegrationItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeEditorIntegrationStatus
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
const PRIVATE_EDITOR_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|buffer|command|content|diagnostic|editor|extension|file|input|log|output|path|prompt|selection|shell|source|stderr|stdout|trace|transcript|workspace)/i,
  /private[-_ ](?:buffer|command|content|diagnostic|editor|file|path|repo|selection|source|workspace)/i,
  /editor[-_ ](?:body|buffer|content|payload|selection|text)/i,
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

const authority: ForgeEditorIntegrationAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  editorAutomationAuthority: false,
  editorCommandAuthority: false,
  extensionInstallAuthority: false,
  fileOpenAuthority: false,
  fileReadAuthority: false,
  fileWriteAuthority: false,
  publicClaimAuthority: false,
  selectionReadAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolExecutionAuthority: false,
  toolRoutingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_EDITOR_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-editor-integration-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkEditorIntegrationEntry,
): Readonly<{
  entry: ForgeEditorIntegrationItem | null
  omittedUnsafeRefCount: number
}> => {
  const integrationRef = safeOptionalRef(entry.integrationRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const commandRefs = safeRefs(entry.commandRefs)
  const deepLinkRefs = safeRefs(entry.deepLinkRefs)
  const diagnosticHandoffRefs = safeRefs(entry.diagnosticHandoffRefs)
  const diagnosticRefs = safeRefs(entry.diagnosticRefs)
  const editorRefs = safeRefs(entry.editorRefs)
  const extensionRefs = safeRefs(entry.extensionRefs)
  const fileOpenRefs = safeRefs(entry.fileOpenRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const selectionRefs = safeRefs(entry.selectionRefs)
  const statusRefs = safeRefs(entry.statusRefs)
  const workspaceRefs = safeRefs(entry.workspaceRefs)
  const omittedUnsafeRefCount =
    integrationRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    commandRefs.omittedUnsafeRefCount +
    deepLinkRefs.omittedUnsafeRefCount +
    diagnosticHandoffRefs.omittedUnsafeRefCount +
    diagnosticRefs.omittedUnsafeRefCount +
    editorRefs.omittedUnsafeRefCount +
    extensionRefs.omittedUnsafeRefCount +
    fileOpenRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    selectionRefs.omittedUnsafeRefCount +
    statusRefs.omittedUnsafeRefCount +
    workspaceRefs.omittedUnsafeRefCount

  return integrationRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          commandRefs: commandRefs.refs,
          deepLinkRefs: deepLinkRefs.refs,
          diagnosticHandoffRefs: diagnosticHandoffRefs.refs,
          diagnosticRefs: diagnosticRefs.refs,
          editorRefs: editorRefs.refs,
          extensionRefs: extensionRefs.refs,
          fileOpenRefs: fileOpenRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          integrationRef: integrationRef.ref,
          policyRefs: policyRefs.refs,
          selectionRefs: selectionRefs.refs,
          state: entry.state,
          statusRefs: statusRefs.refs,
          workspaceRefs: workspaceRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
): ForgeEditorIntegrationCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  connected: entries.filter(entry => entry.state === 'connected').length,
  disconnected: entries.filter(entry => entry.state === 'disconnected').length,
  ready: entries.filter(entry => entry.state === 'ready').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry =>
      blockerRef(workOrderRef, `stale-editor-integration-evidence:${entry.integrationRef}`),
    )

const readyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'ready' || entry.state === 'connected') &&
        entry.blockerRefs.length === 0 &&
        (entry.workspaceRefs.length === 0 || entry.policyRefs.length === 0),
    )
    .map(entry =>
      blockerRef(workOrderRef, `editor-readiness-evidence-missing:${entry.integrationRef}`),
    )

const deepLinkBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.deepLinkRefs.length > 0 &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `deep-link-policy-ref-missing:${entry.integrationRef}`))

const diagnosticHandoffBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.diagnosticHandoffRefs.length > 0 &&
        entry.diagnosticRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `diagnostic-handoff-ref-missing:${entry.integrationRef}`),
    )

const statusForView = (
  entries: ReadonlyArray<ForgeEditorIntegrationItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeEditorIntegrationStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.every(entry => entry.state === 'ready' || entry.state === 'connected')) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'disconnected') ? 'warning' : 'unknown'
}

export const projectForgeEditorIntegration = (
  input: ForgeEditorIntegrationInput,
): ForgeEditorIntegrationView => {
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
        left.integrationRef.localeCompare(right.integrationRef),
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
      ...readyBlockers(input.workOrderRef, entries),
      ...deepLinkBlockers(input.workOrderRef, entries),
      ...diagnosticHandoffBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-editor-integration-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-editor-integration-material-omitted')]),
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

export const buildForgeEditorIntegrationInput = (
  work: AutopilotWorkProjection,
): ForgeEditorIntegrationInput => {
  const source: AutopilotWorkEditorIntegration | undefined = work.editorIntegration

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
