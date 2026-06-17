import type {
  AutopilotWorkMcpServerEntry,
  AutopilotWorkMcpServerExport,
  AutopilotWorkMcpServerExposureState,
  AutopilotWorkMcpServerFreshness,
  AutopilotWorkProjection,
} from '../model'

export type ForgeMcpServerExportStatus =
  | 'blocked'
  | 'disabled'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeMcpServerExportAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  credentialAuthority: false
  deploymentAuthority: false
  effectiveConfigMutationAuthority: false
  fileReadAuthority: false
  publicClaimAuthority: false
  remoteInvocationAuthority: false
  serverHostingAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolExecutionAuthority: false
  toolGrantAuthority: false
  toolRoutingAuthority: false
  transportExposureAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeMcpServerExportItem = Readonly<{
  audienceRefs: ReadonlyArray<string>
  authPolicyRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  exportedPromptRefs: ReadonlyArray<string>
  exportedResourceRefs: ReadonlyArray<string>
  exportedToolRefs: ReadonlyArray<string>
  freshness: AutopilotWorkMcpServerFreshness
  invocationReceiptRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  schemaRefs: ReadonlyArray<string>
  serverRef: string
  sourceRefs: ReadonlyArray<string>
  state: AutopilotWorkMcpServerExposureState
  transportRefs: ReadonlyArray<string>
  trustTierRefs: ReadonlyArray<string>
}>

export type ForgeMcpServerExportInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkMcpServerEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeMcpServerExportCounts = Readonly<{
  blocked: number
  disabled: number
  exposed: number
  internalOnly: number
  planned: number
  total: number
}>

export type ForgeMcpServerExportView = Readonly<{
  authority: ForgeMcpServerExportAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeMcpServerExportCounts
  entries: ReadonlyArray<ForgeMcpServerExportItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeMcpServerExportStatus
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
const PRIVATE_MCP_SERVER_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|capability|command|config|content|credential|file|input|log|mcp|output|payload|prompt|resource|schema|server|shell|socket|source|stderr|stdout|tool|trace|transport|transcript)/i,
  /private[-_ ](?:capability|config|content|credential|mcp|prompt|repo|resource|schema|server|socket|source|tool|transport|workspace)/i,
  /server[-_ ](?:body|content|log|output|payload|text)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeMcpServerExportAuthority = {
  acceptedOutcomeAuthority: false,
  credentialAuthority: false,
  deploymentAuthority: false,
  effectiveConfigMutationAuthority: false,
  fileReadAuthority: false,
  publicClaimAuthority: false,
  remoteInvocationAuthority: false,
  serverHostingAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolExecutionAuthority: false,
  toolGrantAuthority: false,
  toolRoutingAuthority: false,
  transportExposureAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MCP_SERVER_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-mcp-server-export-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkMcpServerEntry,
): Readonly<{
  entry: ForgeMcpServerExportItem | null
  omittedUnsafeRefCount: number
}> => {
  const serverRef = safeOptionalRef(entry.serverRef)
  const audienceRefs = safeRefs(entry.audienceRefs)
  const authPolicyRefs = safeRefs(entry.authPolicyRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const capabilityRefs = safeRefs(entry.capabilityRefs)
  const exportedPromptRefs = safeRefs(entry.exportedPromptRefs)
  const exportedResourceRefs = safeRefs(entry.exportedResourceRefs)
  const exportedToolRefs = safeRefs(entry.exportedToolRefs)
  const invocationReceiptRefs = safeRefs(entry.invocationReceiptRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const schemaRefs = safeRefs(entry.schemaRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const transportRefs = safeRefs(entry.transportRefs)
  const trustTierRefs = safeRefs(entry.trustTierRefs)
  const omittedUnsafeRefCount =
    serverRef.omittedUnsafeRefCount +
    audienceRefs.omittedUnsafeRefCount +
    authPolicyRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    exportedPromptRefs.omittedUnsafeRefCount +
    exportedResourceRefs.omittedUnsafeRefCount +
    exportedToolRefs.omittedUnsafeRefCount +
    invocationReceiptRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    schemaRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    transportRefs.omittedUnsafeRefCount +
    trustTierRefs.omittedUnsafeRefCount

  return serverRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          audienceRefs: audienceRefs.refs,
          authPolicyRefs: authPolicyRefs.refs,
          blockerRefs: blockerRefs.refs,
          capabilityRefs: capabilityRefs.refs,
          exportedPromptRefs: exportedPromptRefs.refs,
          exportedResourceRefs: exportedResourceRefs.refs,
          exportedToolRefs: exportedToolRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          invocationReceiptRefs: invocationReceiptRefs.refs,
          policyRefs: policyRefs.refs,
          schemaRefs: schemaRefs.refs,
          serverRef: serverRef.ref,
          sourceRefs: sourceRefs.refs,
          state: entry.state,
          transportRefs: transportRefs.refs,
          trustTierRefs: trustTierRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ForgeMcpServerExportCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  disabled: entries.filter(entry => entry.state === 'disabled').length,
  exposed: entries.filter(entry => entry.state === 'exposed').length,
  internalOnly: entries.filter(entry => entry.state === 'internal_only').length,
  planned: entries.filter(entry => entry.state === 'planned').length,
  total: entries.length,
})

const hasExportEvidence = (entry: ForgeMcpServerExportItem): boolean =>
  entry.state === 'exposed' ||
  entry.state === 'internal_only' ||
  entry.state === 'planned' ||
  entry.capabilityRefs.length > 0 ||
  entry.exportedPromptRefs.length > 0 ||
  entry.exportedResourceRefs.length > 0 ||
  entry.exportedToolRefs.length > 0

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-mcp-server-evidence:${entry.serverRef}`))

const schemaBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        hasExportEvidence(entry) &&
        entry.schemaRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `export-schema-ref-missing:${entry.serverRef}`))

const policyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        hasExportEvidence(entry) &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `export-policy-ref-missing:${entry.serverRef}`))

const remoteAuthBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'exposed' &&
        entry.blockerRefs.length === 0 &&
        (entry.authPolicyRefs.length === 0 ||
          entry.audienceRefs.length === 0 ||
          entry.trustTierRefs.length === 0),
    )
    .map(entry => blockerRef(workOrderRef, `remote-auth-ref-missing:${entry.serverRef}`))

const invocationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.invocationReceiptRefs.length > 0 &&
        entry.capabilityRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `invocation-capability-ref-missing:${entry.serverRef}`),
    )

const statusForView = (
  entries: ReadonlyArray<ForgeMcpServerExportItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeMcpServerExportStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.every(entry => entry.state === 'disabled')) {
    return 'disabled'
  }

  if (entries.every(entry => entry.state === 'exposed' || entry.state === 'internal_only')) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'planned') ? 'warning' : 'unknown'
}

export const projectForgeMcpServerExport = (
  input: ForgeMcpServerExportInput,
): ForgeMcpServerExportView => {
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
        left.serverRef.localeCompare(right.serverRef),
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
      ...schemaBlockers(input.workOrderRef, entries),
      ...policyBlockers(input.workOrderRef, entries),
      ...remoteAuthBlockers(input.workOrderRef, entries),
      ...invocationBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-mcp-server-export-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-mcp-server-export-material-omitted')]),
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

export const buildForgeMcpServerExportInput = (
  work: AutopilotWorkProjection,
): ForgeMcpServerExportInput => {
  const source: AutopilotWorkMcpServerExport | undefined = work.mcpServerExport

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
