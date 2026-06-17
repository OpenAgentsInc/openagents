import type {
  AutopilotWorkProjection,
  AutopilotWorkSessionMemory,
  AutopilotWorkSessionMemoryEntry,
  AutopilotWorkSessionMemoryFreshness,
  AutopilotWorkSessionMemoryKind,
  AutopilotWorkSessionMemoryLifecycleState,
  AutopilotWorkSessionMemoryRedactionClass,
  AutopilotWorkSessionMemoryRetentionClass,
  AutopilotWorkSessionMemoryScope,
} from '../model'

export type ForgeSessionMemoryStatus =
  | 'blocked'
  | 'conflicted'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeSessionMemoryAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  memoryCompactionAuthority: false
  memoryRetentionPolicyWriteAuthority: false
  memoryWriteAuthority: false
  modelCallAuthority: false
  promptAssemblyAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  skillCommandLoadAuthority: false
  toolGrantAuthority: false
  transcriptSummarizationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeSessionMemoryEntryItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  compactionRefs: ReadonlyArray<string>
  conflictRefs: ReadonlyArray<string>
  entryRef: string
  freshness: AutopilotWorkSessionMemoryFreshness
  kind: AutopilotWorkSessionMemoryKind
  lifecycleState: AutopilotWorkSessionMemoryLifecycleState
  policyRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkSessionMemoryRedactionClass
  retentionClass: AutopilotWorkSessionMemoryRetentionClass
  retrievalRefs: ReadonlyArray<string>
  scope: AutopilotWorkSessionMemoryScope
  sourceRefs: ReadonlyArray<string>
  summaryRefs: ReadonlyArray<string>
}>

export type ForgeSessionMemoryInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkSessionMemoryEntry>
  generatedAt: string
  projectionRef?: string | null
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeSessionMemoryCounts = Readonly<{
  active: number
  conflicted: number
  localOnly: number
  retained: number
  stale: number
  total: number
}>

export type ForgeSessionMemoryView = Readonly<{
  authority: ForgeSessionMemoryAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeSessionMemoryCounts
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  projectionRef: string | null
  publicSafe: true
  snapshotRef: string | null
  status: ForgeSessionMemoryStatus
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
const PRIVATE_MEMORY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /memory[-_ ](?:body|content|text)/i,
  /prompt[-_ ](?:body|content|text)/i,
  /raw[-_ ](?:body|content|diagnostic|file|instruction|log|memory|payload|prompt|provider|request|shell|source|trace|transcript)/i,
  /private[-_ ](?:content|diagnostic|instruction|memory|prompt|repo|source|transcript|workspace)/i,
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

const authority: ForgeSessionMemoryAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  memoryCompactionAuthority: false,
  memoryRetentionPolicyWriteAuthority: false,
  memoryWriteAuthority: false,
  modelCallAuthority: false,
  promptAssemblyAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  skillCommandLoadAuthority: false,
  toolGrantAuthority: false,
  transcriptSummarizationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MEMORY_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-session-memory-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkSessionMemoryEntry,
): Readonly<{
  entry: ForgeSessionMemoryEntryItem | null
  omittedUnsafeRefCount: number
}> => {
  const entryRef = safeOptionalRef(entry.entryRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const compactionRefs = safeRefs(entry.compactionRefs)
  const conflictRefs = safeRefs(entry.conflictRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const retrievalRefs = safeRefs(entry.retrievalRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const summaryRefs = safeRefs(entry.summaryRefs)
  const omittedUnsafeRefCount =
    entryRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    compactionRefs.omittedUnsafeRefCount +
    conflictRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    retrievalRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    summaryRefs.omittedUnsafeRefCount

  return entryRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          compactionRefs: compactionRefs.refs,
          conflictRefs: conflictRefs.refs,
          entryRef: entryRef.ref,
          freshness: entry.freshness ?? 'unknown',
          kind: entry.kind,
          lifecycleState: entry.lifecycleState,
          policyRefs: policyRefs.refs,
          redactionClass: entry.redactionClass ?? 'private_ref',
          retentionClass: entry.retentionClass ?? 'ephemeral',
          retrievalRefs: retrievalRefs.refs,
          scope: entry.scope,
          sourceRefs: sourceRefs.refs,
          summaryRefs: summaryRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const sessionMemoryCounts = (
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>,
): ForgeSessionMemoryCounts => ({
  active: entries.filter(entry => entry.lifecycleState === 'active').length,
  conflicted: entries.filter(
    entry => entry.lifecycleState === 'superseded' || entry.conflictRefs.length > 0,
  ).length,
  localOnly: entries.filter(entry => entry.redactionClass === 'local_only').length,
  retained: entries.filter(entry => entry.retentionClass !== 'ephemeral').length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
  total: entries.length,
})

const staleMemoryBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-memory-refresh-evidence-missing:${entry.entryRef}`))

const conflictEvidenceBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.lifecycleState === 'superseded' &&
        entry.conflictRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `conflict-evidence-missing:${entry.entryRef}`))

const retentionPolicyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.retentionClass !== 'ephemeral' &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `retention-policy-missing:${entry.entryRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeSessionMemoryEntryItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeSessionMemoryStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.lifecycleState === 'superseded')) {
    return 'conflicted'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeSessionMemory = (
  input: ForgeSessionMemoryInput,
): ForgeSessionMemoryView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const projectionRef = safeOptionalRef(input.projectionRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.scope.localeCompare(right.scope) ||
        left.kind.localeCompare(right.kind) ||
        left.entryRef.localeCompare(right.entryRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    projectionRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleMemoryBlockers(input.workOrderRef, entries),
      ...conflictEvidenceBlockers(input.workOrderRef, entries),
      ...retentionPolicyBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-session-memory-snapshot-ref')]
        : []),
      ...(projectionRef.ref !== null && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'projection-without-snapshot')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-session-memory-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: sessionMemoryCounts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    projectionRef: projectionRef.ref,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeSessionMemoryInput = (
  work: AutopilotWorkProjection,
): ForgeSessionMemoryInput => {
  const source: AutopilotWorkSessionMemory | undefined = work.sessionMemory

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
    ...(source.projectionRef === undefined ? {} : { projectionRef: source.projectionRef }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
