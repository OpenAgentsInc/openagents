import type {
  AutopilotWorkProjection,
  AutopilotWorkUpdateReleaseChannel,
  AutopilotWorkUpdateReleaseEntry,
  AutopilotWorkUpdateReleaseFreshness,
  AutopilotWorkUpdateReleaseStatus,
} from '../model'

export type ForgeUpdateReleaseStatus =
  | 'blocked'
  | 'current'
  | 'empty'
  | 'failed'
  | 'required'
  | 'unknown'
  | 'update_available'

export type ForgeUpdateReleaseAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  channelPinMutationAuthority: false
  deploymentAuthority: false
  installerAuthority: false
  managedPolicyMutationAuthority: false
  manifestFetchAuthority: false
  manifestVerificationAuthority: false
  migrationAuthority: false
  publicClaimAuthority: false
  rollbackAuthority: false
  settlementAuthority: false
  smokeExecutionAuthority: false
  updateCheckNetworkAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeUpdateReleaseItem = Readonly<{
  activeRunRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  channel: AutopilotWorkUpdateReleaseChannel
  channelRefs: ReadonlyArray<string>
  checksumRefs: ReadonlyArray<string>
  compatibilityRefs: ReadonlyArray<string>
  deprecationRefs: ReadonlyArray<string>
  freshness: AutopilotWorkUpdateReleaseFreshness
  knownBlockerRefs: ReadonlyArray<string>
  managedOverride: boolean
  managedPinRefs: ReadonlyArray<string>
  manifestRefs: ReadonlyArray<string>
  migrationRefs: ReadonlyArray<string>
  migrationRequired: boolean
  platformRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  releaseNoteRefs: ReadonlyArray<string>
  releaseRef: string
  restorePointRefs: ReadonlyArray<string>
  rollbackRefs: ReadonlyArray<string>
  rolloutRefs: ReadonlyArray<string>
  runtimeRequirementRefs: ReadonlyArray<string>
  safeUpdateWindowRefs: ReadonlyArray<string>
  signatureRefs: ReadonlyArray<string>
  smokeReceiptRefs: ReadonlyArray<string>
  status: AutopilotWorkUpdateReleaseStatus
  supportRefs: ReadonlyArray<string>
  versionRef: string
}>

export type ForgeUpdateReleaseInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkUpdateReleaseEntry>
  generatedAt: string
  manifestRefs?: ReadonlyArray<string>
  policyRefs?: ReadonlyArray<string>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeUpdateReleaseCounts = Readonly<{
  available: number
  blocked: number
  current: number
  entries: number
  managed: number
  required: number
  stale: number
}>

export type ForgeUpdateReleaseView = Readonly<{
  authority: ForgeUpdateReleaseAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeUpdateReleaseCounts
  entries: ReadonlyArray<ForgeUpdateReleaseItem>
  generatedAt: string
  manifestRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  policyRefs: ReadonlyArray<string>
  publicSafe: true
  snapshotRef: string | null
  status: ForgeUpdateReleaseStatus
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
const PRIVATE_UPDATE_RELEASE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|binary|command|content|detail|file|log|manifest|note|output|package|payload|provider|release|shell|transcript)/i,
  /private[-_ ](?:artifact|code|command|content|detail|file|log|manifest|note|payload|provider|repo|release|source|workspace)/i,
  /artifact[-_ ]payload/i,
  /manifest[-_ ]body/i,
  /provider[-_ ]payload/i,
  /release[-_ ]note[-_ ]body/i,
  /shell[-_ ]log/i,
  /customer[-_ ](?:data|private|payload|record)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeUpdateReleaseAuthority = {
  acceptedOutcomeAuthority: false,
  channelPinMutationAuthority: false,
  deploymentAuthority: false,
  installerAuthority: false,
  managedPolicyMutationAuthority: false,
  manifestFetchAuthority: false,
  manifestVerificationAuthority: false,
  migrationAuthority: false,
  publicClaimAuthority: false,
  rollbackAuthority: false,
  settlementAuthority: false,
  smokeExecutionAuthority: false,
  updateCheckNetworkAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_UPDATE_RELEASE_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-update-release-blocker:${workOrderRef}:${suffix}`

const isUpdateClaim = (status: AutopilotWorkUpdateReleaseStatus): boolean =>
  status === 'available' || status === 'recommended' || status === 'required'

const normalizeItem = (
  item: AutopilotWorkUpdateReleaseEntry,
): Readonly<{
  item: ForgeUpdateReleaseItem | null
  omittedUnsafeRefCount: number
}> => {
  const activeRunRefs = safeRefs(item.activeRunRefs)
  const artifactRefs = safeRefs(item.artifactRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const channelRefs = safeRefs(item.channelRefs)
  const checksumRefs = safeRefs(item.checksumRefs)
  const compatibilityRefs = safeRefs(item.compatibilityRefs)
  const deprecationRefs = safeRefs(item.deprecationRefs)
  const knownBlockerRefs = safeRefs(item.knownBlockerRefs)
  const managedPinRefs = safeRefs(item.managedPinRefs)
  const manifestRefs = safeRefs(item.manifestRefs)
  const migrationRefs = safeRefs(item.migrationRefs)
  const platformRefs = safeRefs(item.platformRefs)
  const policyRefs = safeRefs(item.policyRefs)
  const releaseNoteRefs = safeRefs(item.releaseNoteRefs)
  const releaseRef = safeOptionalRef(item.releaseRef)
  const restorePointRefs = safeRefs(item.restorePointRefs)
  const rollbackRefs = safeRefs(item.rollbackRefs)
  const rolloutRefs = safeRefs(item.rolloutRefs)
  const runtimeRequirementRefs = safeRefs(item.runtimeRequirementRefs)
  const safeUpdateWindowRefs = safeRefs(item.safeUpdateWindowRefs)
  const signatureRefs = safeRefs(item.signatureRefs)
  const smokeReceiptRefs = safeRefs(item.smokeReceiptRefs)
  const supportRefs = safeRefs(item.supportRefs)
  const versionRef = safeOptionalRef(item.versionRef)
  const omittedUnsafeRefCount =
    activeRunRefs.omittedUnsafeRefCount +
    artifactRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    channelRefs.omittedUnsafeRefCount +
    checksumRefs.omittedUnsafeRefCount +
    compatibilityRefs.omittedUnsafeRefCount +
    deprecationRefs.omittedUnsafeRefCount +
    knownBlockerRefs.omittedUnsafeRefCount +
    managedPinRefs.omittedUnsafeRefCount +
    manifestRefs.omittedUnsafeRefCount +
    migrationRefs.omittedUnsafeRefCount +
    platformRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    releaseNoteRefs.omittedUnsafeRefCount +
    releaseRef.omittedUnsafeRefCount +
    restorePointRefs.omittedUnsafeRefCount +
    rollbackRefs.omittedUnsafeRefCount +
    rolloutRefs.omittedUnsafeRefCount +
    runtimeRequirementRefs.omittedUnsafeRefCount +
    safeUpdateWindowRefs.omittedUnsafeRefCount +
    signatureRefs.omittedUnsafeRefCount +
    smokeReceiptRefs.omittedUnsafeRefCount +
    supportRefs.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount

  return releaseRef.ref === null || versionRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          activeRunRefs: activeRunRefs.refs,
          artifactRefs: artifactRefs.refs,
          blockerRefs: blockerRefs.refs,
          channel: item.channel,
          channelRefs: channelRefs.refs,
          checksumRefs: checksumRefs.refs,
          compatibilityRefs: compatibilityRefs.refs,
          deprecationRefs: deprecationRefs.refs,
          freshness: item.freshness ?? 'unknown',
          knownBlockerRefs: knownBlockerRefs.refs,
          managedOverride: managedPinRefs.refs.length > 0,
          managedPinRefs: managedPinRefs.refs,
          manifestRefs: manifestRefs.refs,
          migrationRefs: migrationRefs.refs,
          migrationRequired: item.migrationRequired ?? false,
          platformRefs: platformRefs.refs,
          policyRefs: policyRefs.refs,
          releaseNoteRefs: releaseNoteRefs.refs,
          releaseRef: releaseRef.ref,
          restorePointRefs: restorePointRefs.refs,
          rollbackRefs: rollbackRefs.refs,
          rolloutRefs: rolloutRefs.refs,
          runtimeRequirementRefs: runtimeRequirementRefs.refs,
          safeUpdateWindowRefs: safeUpdateWindowRefs.refs,
          signatureRefs: signatureRefs.refs,
          smokeReceiptRefs: smokeReceiptRefs.refs,
          status: item.status,
          supportRefs: supportRefs.refs,
          versionRef: versionRef.ref,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeUpdateReleaseItem>,
): ForgeUpdateReleaseCounts => ({
  available: entries.filter(entry =>
    entry.status === 'available' || entry.status === 'recommended',
  ).length,
  blocked: entries.filter(entry => entry.status === 'blocked').length,
  current: entries.filter(entry => entry.status === 'current').length,
  entries: entries.length,
  managed: entries.filter(entry => entry.managedOverride).length,
  required: entries.filter(entry => entry.status === 'required').length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeUpdateReleaseItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs, ...item.knownBlockerRefs]
  const claim = isUpdateClaim(item.status)

  if (item.freshness === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-release-evidence:${item.releaseRef}`),
    )
  }

  if (claim && item.manifestRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `update-claim-missing-manifest:${item.releaseRef}`),
    )
  }

  if (
    claim &&
    (item.signatureRefs.length === 0 || item.checksumRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `update-claim-missing-integrity:${item.releaseRef}`),
    )
  }

  if (claim && item.platformRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `update-claim-missing-platform:${item.releaseRef}`),
    )
  }

  if (claim && item.compatibilityRefs.length === 0) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `update-claim-missing-compatibility:${item.releaseRef}`,
      ),
    )
  }

  if (claim && item.smokeReceiptRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `update-claim-missing-smoke:${item.releaseRef}`),
    )
  }

  if (
    item.migrationRequired &&
    item.restorePointRefs.length === 0 &&
    item.rollbackRefs.length === 0
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `migration-without-restore-or-rollback:${item.releaseRef}`,
      ),
    )
  }

  if (
    claim &&
    item.activeRunRefs.length > 0 &&
    item.safeUpdateWindowRefs.length === 0
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `active-run-update-without-safe-window:${item.releaseRef}`,
      ),
    )
  }

  if (item.managedPinRefs.length > 0 && item.policyRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `managed-pin-policy-missing:${item.releaseRef}`),
    )
  }

  if (item.releaseNoteRefs.length > 0 && item.smokeReceiptRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `release-notes-without-receipts:${item.releaseRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeUpdateReleaseItem>,
  blockers: ReadonlyArray<string>,
): ForgeUpdateReleaseStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.status === 'required')) {
    return 'required'
  }

  if (
    entries.some(
      entry => entry.status === 'available' || entry.status === 'recommended',
    )
  ) {
    return 'update_available'
  }

  if (entries.some(entry => entry.status === 'failed')) {
    return 'failed'
  }

  if (entries.every(entry => entry.status === 'current')) {
    return 'current'
  }

  return 'unknown'
}

export const projectForgeUpdateRelease = (
  input: ForgeUpdateReleaseInput,
): ForgeUpdateReleaseView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const manifestRefs = safeRefs(input.manifestRefs)
  const policyRefs = safeRefs(input.policyRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result =>
    result.item === null ? [] : [result.item],
  )
  const normalizedOmissions = normalized.reduce(
    (total, result) => total + result.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    manifestRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(
      blockerRef(input.workOrderRef, 'missing-update-release-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-update-release-material-omitted'),
    )
  }

  const uniqueBlockers = Array.from(new Set(blockers))

  return {
    authority,
    blockerRefs: uniqueBlockers,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    manifestRefs: manifestRefs.refs,
    omittedUnsafeRefCount,
    policyRefs: policyRefs.refs,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeUpdateReleaseInput = (
  work: AutopilotWorkProjection,
): ForgeUpdateReleaseInput => {
  const updateRelease = work.updateRelease

  return {
    generatedAt: updateRelease?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(updateRelease?.blockerRefs === undefined
      ? {}
      : { blockerRefs: updateRelease.blockerRefs }),
    ...(updateRelease?.entries === undefined
      ? {}
      : { entries: updateRelease.entries }),
    ...(updateRelease?.manifestRefs === undefined
      ? {}
      : { manifestRefs: updateRelease.manifestRefs }),
    ...(updateRelease?.policyRefs === undefined
      ? {}
      : { policyRefs: updateRelease.policyRefs }),
    ...(updateRelease?.snapshotRef === undefined
      ? {}
      : { snapshotRef: updateRelease.snapshotRef }),
    ...(updateRelease?.versionRef === undefined
      ? {}
      : { versionRef: updateRelease.versionRef }),
  }
}
