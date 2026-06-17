import type {
  AutopilotWorkProjection,
  AutopilotWorkSettingsConfiguration,
  AutopilotWorkSettingsConfigurationEntry,
  AutopilotWorkSettingsConfigurationFreshness,
  AutopilotWorkSettingsConfigurationRedactionClass,
  AutopilotWorkSettingsConfigurationState,
} from '../model'

export type ForgeSettingsConfigurationStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeSettingsConfigurationAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  credentialAuthority: false
  deploymentAuthority: false
  effectiveConfigMutationAuthority: false
  fileReadAuthority: false
  publicClaimAuthority: false
  settingsActivationAuthority: false
  settingsReadAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolExecutionAuthority: false
  toolRoutingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeSettingsConfigurationItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  defaultRefs: ReadonlyArray<string>
  effectiveValueRefs: ReadonlyArray<string>
  freshness: AutopilotWorkSettingsConfigurationFreshness
  overrideRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkSettingsConfigurationRedactionClass
  redactionRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  settingRef: string
  sourceRefs: ReadonlyArray<string>
  state: AutopilotWorkSettingsConfigurationState
  validationRefs: ReadonlyArray<string>
}>

export type ForgeSettingsConfigurationInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkSettingsConfigurationEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeSettingsConfigurationCounts = Readonly<{
  blocked: number
  defaulted: number
  enabled: number
  overridden: number
  total: number
}>

export type ForgeSettingsConfigurationView = Readonly<{
  authority: ForgeSettingsConfigurationAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeSettingsConfigurationCounts
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeSettingsConfigurationStatus
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
const PRIVATE_SETTINGS_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|command|config|content|credential|file|input|key|log|output|payload|prompt|setting|settings|shell|source|stderr|stdout|trace|transcript|value)/i,
  /private[-_ ](?:config|content|credential|input|prompt|repo|setting|settings|source|transcript|value|workspace)/i,
  /settings?[-_ ](?:body|content|file|log|payload|text|value)/i,
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

const authority: ForgeSettingsConfigurationAuthority = {
  acceptedOutcomeAuthority: false,
  credentialAuthority: false,
  deploymentAuthority: false,
  effectiveConfigMutationAuthority: false,
  fileReadAuthority: false,
  publicClaimAuthority: false,
  settingsActivationAuthority: false,
  settingsReadAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolExecutionAuthority: false,
  toolRoutingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SETTINGS_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-settings-configuration-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkSettingsConfigurationEntry,
): Readonly<{
  entry: ForgeSettingsConfigurationItem | null
  omittedUnsafeRefCount: number
}> => {
  const settingRef = safeOptionalRef(entry.settingRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const defaultRefs = safeRefs(entry.defaultRefs)
  const effectiveValueRefs = safeRefs(entry.effectiveValueRefs)
  const overrideRefs = safeRefs(entry.overrideRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const redactionRefs = safeRefs(entry.redactionRefs)
  const scopeRefs = safeRefs(entry.scopeRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const validationRefs = safeRefs(entry.validationRefs)
  const omittedUnsafeRefCount =
    settingRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    defaultRefs.omittedUnsafeRefCount +
    effectiveValueRefs.omittedUnsafeRefCount +
    overrideRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    redactionRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    validationRefs.omittedUnsafeRefCount

  return settingRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          defaultRefs: defaultRefs.refs,
          effectiveValueRefs: effectiveValueRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          overrideRefs: overrideRefs.refs,
          policyRefs: policyRefs.refs,
          redactionClass: entry.redactionClass ?? 'public',
          redactionRefs: redactionRefs.refs,
          scopeRefs: scopeRefs.refs,
          settingRef: settingRef.ref,
          sourceRefs: sourceRefs.refs,
          state: entry.state,
          validationRefs: validationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
): ForgeSettingsConfigurationCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  defaulted: entries.filter(entry => entry.state === 'defaulted').length,
  enabled: entries.filter(entry => entry.state === 'enabled').length,
  overridden: entries.filter(entry => entry.state === 'overridden').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-settings-evidence:${entry.settingRef}`))

const policyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'enabled' || entry.state === 'overridden') &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `settings-policy-ref-missing:${entry.settingRef}`))

const validationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.effectiveValueRefs.length > 0 &&
        entry.validationRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `effective-value-validation-ref-missing:${entry.settingRef}`),
    )

const redactionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.redactionClass !== 'public' &&
        entry.redactionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `settings-redaction-ref-missing:${entry.settingRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeSettingsConfigurationItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeSettingsConfigurationStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.every(entry => entry.state === 'defaulted' || entry.state === 'enabled')) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'overridden') ? 'warning' : 'unknown'
}

export const projectForgeSettingsConfiguration = (
  input: ForgeSettingsConfigurationInput,
): ForgeSettingsConfigurationView => {
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
        left.settingRef.localeCompare(right.settingRef),
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
      ...policyBlockers(input.workOrderRef, entries),
      ...validationBlockers(input.workOrderRef, entries),
      ...redactionBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-settings-configuration-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-settings-configuration-material-omitted')]),
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

export const buildForgeSettingsConfigurationInput = (
  work: AutopilotWorkProjection,
): ForgeSettingsConfigurationInput => {
  const source: AutopilotWorkSettingsConfiguration | undefined =
    work.settingsConfiguration

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
