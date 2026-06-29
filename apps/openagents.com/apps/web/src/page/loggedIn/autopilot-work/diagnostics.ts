import type {
  AutopilotWorkDiagnosticEntry,
  AutopilotWorkDiagnosticSeverity,
  AutopilotWorkDiagnostics,
  AutopilotWorkDiagnosticsFreshness,
  AutopilotWorkProjection,
} from '../model'

export type ForgeDiagnosticsStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeDiagnosticsAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  codeActionAuthority: false
  deploymentAuthority: false
  diagnosticsExecutionAuthority: false
  editAuthority: false
  fileReadAuthority: false
  lspConfigurationAuthority: false
  lspProcessAuthority: false
  providerAuthority: false
  publicClaimAuthority: false
  retrievalRoutingAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeDiagnosticEntryItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  diagnosticRef: string
  freshness: AutopilotWorkDiagnosticsFreshness
  languageServerRef: string | null
  policyRefs: ReadonlyArray<string>
  remediationRefs: ReadonlyArray<string>
  severity: AutopilotWorkDiagnosticSeverity
  sourceRefs: ReadonlyArray<string>
}>

export type ForgeDiagnosticsInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  diagnosticRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkDiagnosticEntry>
  freshness?: AutopilotWorkDiagnosticsFreshness
  generatedAt: string
  indexedAt?: string | null
  indexedAtRef?: string | null
  languageServerRefs?: ReadonlyArray<string>
  policyRefs?: ReadonlyArray<string>
  remediationRefs?: ReadonlyArray<string>
  severityCounts?: Readonly<{
    errorCount?: number | null
    hintCount?: number | null
    infoCount?: number | null
    warningCount?: number | null
  }>
  skippedDiagnosticRefs?: ReadonlyArray<string>
  snapshotRef?: string
  sourceRefs?: ReadonlyArray<string>
  versionRef?: string | null
  workOrderRef: string
  workspaceBoundaryRefs?: ReadonlyArray<string>
}>

export type ForgeDiagnosticsCounts = Readonly<{
  errors: number
  hints: number
  info: number
  total: number
  warnings: number
}>

export type ForgeDiagnosticsView = Readonly<{
  authority: ForgeDiagnosticsAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeDiagnosticsCounts
  diagnosticRefs: ReadonlyArray<string>
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>
  freshness: AutopilotWorkDiagnosticsFreshness
  generatedAt: string
  indexedAt: string | null
  indexedAtRef: string | null
  languageServerRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  policyRefs: ReadonlyArray<string>
  publicSafe: true
  remediationRefs: ReadonlyArray<string>
  skippedDiagnosticRefs: ReadonlyArray<string>
  snapshotRef: string | null
  sourceRefs: ReadonlyArray<string>
  status: ForgeDiagnosticsStatus
  versionRef: string | null
  workOrderRef: string
  workspaceBoundaryRefs: ReadonlyArray<string>
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
const PRIVATE_DIAGNOSTIC_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /compiler[-_ ](?:output|stderr|stdout)/i,
  /diagnostic[-_ ](?:body|content|message|text)/i,
  /raw[-_ ](?:body|compiler|content|diagnostic|file|instruction|log|memory|payload|prompt|provider|request|shell|source|stderr|stdout|trace|transcript)/i,
  /private[-_ ](?:content|diagnostic|instruction|prompt|repo|source|transcript|workspace)/i,
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

const authority: ForgeDiagnosticsAuthority = {
  acceptedOutcomeAuthority: false,
  codeActionAuthority: false,
  deploymentAuthority: false,
  diagnosticsExecutionAuthority: false,
  editAuthority: false,
  fileReadAuthority: false,
  lspConfigurationAuthority: false,
  lspProcessAuthority: false,
  providerAuthority: false,
  publicClaimAuthority: false,
  retrievalRoutingAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_DIAGNOSTIC_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-diagnostics-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkDiagnosticEntry,
): Readonly<{
  entry: ForgeDiagnosticEntryItem | null
  omittedUnsafeRefCount: number
}> => {
  const diagnosticRef = safeOptionalRef(entry.diagnosticRef)
  const languageServerRef = safeOptionalRef(entry.languageServerRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const remediationRefs = safeRefs(entry.remediationRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const omittedUnsafeRefCount =
    diagnosticRef.omittedUnsafeRefCount +
    languageServerRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    remediationRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount

  return diagnosticRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          diagnosticRef: diagnosticRef.ref,
          freshness: entry.freshness ?? 'unknown',
          languageServerRef: languageServerRef.ref,
          policyRefs: policyRefs.refs,
          remediationRefs: remediationRefs.refs,
          severity: entry.severity,
          sourceRefs: sourceRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const entryCounts = (
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>,
): ForgeDiagnosticsCounts => ({
  errors: entries.filter(entry => entry.severity === 'error').length,
  hints: entries.filter(entry => entry.severity === 'hint').length,
  info: entries.filter(entry => entry.severity === 'info').length,
  total: entries.length,
  warnings: entries.filter(entry => entry.severity === 'warning').length,
})

const countsForView = (
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>,
  severityCounts: ForgeDiagnosticsInput['severityCounts'],
): ForgeDiagnosticsCounts => {
  const derived = entryCounts(entries)

  return severityCounts === undefined
    ? derived
    : {
        errors: severityCounts.errorCount ?? derived.errors,
        hints: severityCounts.hintCount ?? derived.hints,
        info: severityCounts.infoCount ?? derived.info,
        total:
          (severityCounts.errorCount ?? derived.errors) +
          (severityCounts.warningCount ?? derived.warnings) +
          (severityCounts.infoCount ?? derived.info) +
          (severityCounts.hintCount ?? derived.hints),
        warnings: severityCounts.warningCount ?? derived.warnings,
      }
}

const hasDiagnosticsEvidence = (input: Readonly<{
  diagnosticRefs: ReadonlyArray<string>
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>
  skippedDiagnosticRefs: ReadonlyArray<string>
}>): boolean =>
  input.diagnosticRefs.length > 0 ||
  input.entries.length > 0 ||
  input.skippedDiagnosticRefs.length > 0

const staleDiagnosticBlockers = (
  workOrderRef: string,
  freshness: AutopilotWorkDiagnosticsFreshness,
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>,
): ReadonlyArray<string> => [
  ...(freshness === 'stale'
    ? [blockerRef(workOrderRef, 'stale-diagnostics-refresh-evidence-missing')]
    : []),
  ...entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-diagnostic-refresh-evidence-missing:${entry.diagnosticRef}`)),
]

const remediationPolicyBlockers = (
  workOrderRef: string,
  policyRefs: ReadonlyArray<string>,
  remediationRefs: ReadonlyArray<string>,
  entries: ReadonlyArray<ForgeDiagnosticEntryItem>,
): ReadonlyArray<string> => [
  ...(remediationRefs.length > 0 && policyRefs.length === 0
    ? [blockerRef(workOrderRef, 'remediation-policy-missing')]
    : []),
  ...entries
    .filter(
      entry =>
        entry.remediationRefs.length > 0 &&
        entry.policyRefs.length === 0 &&
        policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `remediation-policy-missing:${entry.diagnosticRef}`)),
]

const statusForView = (
  input: Readonly<{
    blockerRefs: ReadonlyArray<string>
    evidencePresent: boolean
    freshness: AutopilotWorkDiagnosticsFreshness
  }>,
): ForgeDiagnosticsStatus => {
  if (input.blockerRefs.length > 0) {
    return 'blocked'
  }

  if (!input.evidencePresent) {
    return 'empty'
  }

  return input.freshness === 'fresh'
    ? 'ready'
    : input.freshness === 'stale'
      ? 'stale'
      : 'unknown'
}

export const projectForgeDiagnostics = (
  input: ForgeDiagnosticsInput,
): ForgeDiagnosticsView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const indexedAtRef = safeOptionalRef(input.indexedAtRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const diagnosticRefs = safeRefs(input.diagnosticRefs)
  const languageServerRefs = safeRefs(input.languageServerRefs)
  const policyRefs = safeRefs(input.policyRefs)
  const remediationRefs = safeRefs(input.remediationRefs)
  const skippedDiagnosticRefs = safeRefs(input.skippedDiagnosticRefs)
  const sourceRefs = safeRefs(input.sourceRefs)
  const workspaceBoundaryRefs = safeRefs(input.workspaceBoundaryRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.severity.localeCompare(right.severity) ||
        left.diagnosticRef.localeCompare(right.diagnosticRef),
    )
  const freshness = input.freshness ?? 'unknown'
  const evidencePresent = hasDiagnosticsEvidence({
    diagnosticRefs: diagnosticRefs.refs,
    entries,
    skippedDiagnosticRefs: skippedDiagnosticRefs.refs,
  })
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    indexedAtRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    diagnosticRefs.omittedUnsafeRefCount +
    languageServerRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    remediationRefs.omittedUnsafeRefCount +
    skippedDiagnosticRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    workspaceBoundaryRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const entryLanguageServerRefs = entries.flatMap(entry =>
    entry.languageServerRef === null ? [] : [entry.languageServerRef],
  )
  const allLanguageServerRefs = Array.from(
    new Set([...languageServerRefs.refs, ...entryLanguageServerRefs]),
  )
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleDiagnosticBlockers(input.workOrderRef, freshness, entries),
      ...remediationPolicyBlockers(
        input.workOrderRef,
        policyRefs.refs,
        remediationRefs.refs,
        entries,
      ),
      ...(evidencePresent && workspaceBoundaryRefs.refs.length === 0
        ? [blockerRef(input.workOrderRef, 'missing-workspace-boundary-ref')]
        : []),
      ...(evidencePresent && allLanguageServerRefs.length === 0
        ? [blockerRef(input.workOrderRef, 'missing-language-server-evidence')]
        : []),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-diagnostics-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-diagnostics-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: countsForView(entries, input.severityCounts),
    diagnosticRefs: diagnosticRefs.refs,
    entries,
    freshness,
    generatedAt: input.generatedAt,
    indexedAt: input.indexedAt ?? null,
    indexedAtRef: indexedAtRef.ref,
    languageServerRefs: allLanguageServerRefs,
    omittedUnsafeRefCount,
    policyRefs: policyRefs.refs,
    publicSafe: true,
    remediationRefs: remediationRefs.refs,
    skippedDiagnosticRefs: skippedDiagnosticRefs.refs,
    snapshotRef: snapshotRef.ref,
    sourceRefs: sourceRefs.refs,
    status: statusForView({ blockerRefs, evidencePresent, freshness }),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
    workspaceBoundaryRefs: workspaceBoundaryRefs.refs,
  }
}

export const buildForgeDiagnosticsInput = (
  work: AutopilotWorkProjection,
): ForgeDiagnosticsInput => {
  const source: AutopilotWorkDiagnostics | undefined = work.diagnostics

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
    ...(source.diagnosticRefs === undefined
      ? {}
      : { diagnosticRefs: source.diagnosticRefs }),
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    ...(source.freshness === undefined ? {} : { freshness: source.freshness }),
    ...(source.indexedAt === undefined ? {} : { indexedAt: source.indexedAt }),
    ...(source.indexedAtRef === undefined ? {} : { indexedAtRef: source.indexedAtRef }),
    ...(source.languageServerRefs === undefined
      ? {}
      : { languageServerRefs: source.languageServerRefs }),
    ...(source.policyRefs === undefined ? {} : { policyRefs: source.policyRefs }),
    ...(source.remediationRefs === undefined
      ? {}
      : { remediationRefs: source.remediationRefs }),
    ...(source.severityCounts === undefined
      ? {}
      : { severityCounts: source.severityCounts }),
    ...(source.skippedDiagnosticRefs === undefined
      ? {}
      : { skippedDiagnosticRefs: source.skippedDiagnosticRefs }),
    snapshotRef: source.snapshotRef,
    ...(source.sourceRefs === undefined ? {} : { sourceRefs: source.sourceRefs }),
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
    ...(source.workspaceBoundaryRefs === undefined
      ? {}
      : { workspaceBoundaryRefs: source.workspaceBoundaryRefs }),
  }
}
