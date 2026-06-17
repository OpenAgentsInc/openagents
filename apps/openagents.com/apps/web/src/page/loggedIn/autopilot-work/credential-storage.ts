import type {
  AutopilotWorkCredentialFreshness,
  AutopilotWorkCredentialKind,
  AutopilotWorkCredentialRedactionClass,
  AutopilotWorkCredentialState,
  AutopilotWorkCredentialStorage,
  AutopilotWorkCredentialStorageEntry,
  AutopilotWorkProjection,
} from '../model'

export type ForgeCredentialStorageStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeCredentialStorageAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  authenticationAuthority: false
  credentialMintAuthority: false
  credentialReadAuthority: false
  credentialRefreshAuthority: false
  credentialRevokeAuthority: false
  credentialRotateAuthority: false
  credentialWriteAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  providerAccountAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolExecutionAuthority: false
  toolRoutingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeCredentialStorageItem = Readonly<{
  accountRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  credentialRef: string
  entitlementRefs: ReadonlyArray<string>
  freshness: AutopilotWorkCredentialFreshness
  kind: AutopilotWorkCredentialKind
  leaseRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkCredentialRedactionClass
  redactionRefs: ReadonlyArray<string>
  revocationRefs: ReadonlyArray<string>
  rotationRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  sessionRefs: ReadonlyArray<string>
  state: AutopilotWorkCredentialState
  storageBackendRefs: ReadonlyArray<string>
  validationRefs: ReadonlyArray<string>
}>

export type ForgeCredentialStorageInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkCredentialStorageEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeCredentialStorageCounts = Readonly<{
  blocked: number
  expired: number
  missing: number
  revoked: number
  total: number
  usable: number
}>

export type ForgeCredentialStorageView = Readonly<{
  authority: ForgeCredentialStorageAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeCredentialStorageCounts
  entries: ReadonlyArray<ForgeCredentialStorageItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeCredentialStorageStatus
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
const PRIVATE_CREDENTIAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /credential[-_ ](?:body|content|material|payload|raw|secret|text|value)/i,
  /raw[-_ ](?:api[-_ ]?key|auth|body|command|content|credential|env|file|key|log|mnemonic|output|password|payload|preimage|secret|session|shell|source|stderr|stdout|token|trace|transcript|wallet)/i,
  /private[-_ ](?:api[-_ ]?key|auth|content|credential|env|key|repo|secret|session|source|token|wallet|workspace)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeCredentialStorageAuthority = {
  acceptedOutcomeAuthority: false,
  authenticationAuthority: false,
  credentialMintAuthority: false,
  credentialReadAuthority: false,
  credentialRefreshAuthority: false,
  credentialRevokeAuthority: false,
  credentialRotateAuthority: false,
  credentialWriteAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  providerAccountAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolExecutionAuthority: false,
  toolRoutingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_CREDENTIAL_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-credential-storage-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkCredentialStorageEntry,
): Readonly<{
  entry: ForgeCredentialStorageItem | null
  omittedUnsafeRefCount: number
}> => {
  const credentialRef = safeOptionalRef(entry.credentialRef)
  const accountRefs = safeRefs(entry.accountRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const entitlementRefs = safeRefs(entry.entitlementRefs)
  const leaseRefs = safeRefs(entry.leaseRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const redactionRefs = safeRefs(entry.redactionRefs)
  const revocationRefs = safeRefs(entry.revocationRefs)
  const rotationRefs = safeRefs(entry.rotationRefs)
  const scopeRefs = safeRefs(entry.scopeRefs)
  const sessionRefs = safeRefs(entry.sessionRefs)
  const storageBackendRefs = safeRefs(entry.storageBackendRefs)
  const validationRefs = safeRefs(entry.validationRefs)
  const omittedUnsafeRefCount =
    credentialRef.omittedUnsafeRefCount +
    accountRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    entitlementRefs.omittedUnsafeRefCount +
    leaseRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    redactionRefs.omittedUnsafeRefCount +
    revocationRefs.omittedUnsafeRefCount +
    rotationRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    sessionRefs.omittedUnsafeRefCount +
    storageBackendRefs.omittedUnsafeRefCount +
    validationRefs.omittedUnsafeRefCount

  return credentialRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          accountRefs: accountRefs.refs,
          blockerRefs: blockerRefs.refs,
          credentialRef: credentialRef.ref,
          entitlementRefs: entitlementRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          kind: entry.kind,
          leaseRefs: leaseRefs.refs,
          policyRefs: policyRefs.refs,
          redactionClass: entry.redactionClass ?? 'public',
          redactionRefs: redactionRefs.refs,
          revocationRefs: revocationRefs.refs,
          rotationRefs: rotationRefs.refs,
          scopeRefs: scopeRefs.refs,
          sessionRefs: sessionRefs.refs,
          state: entry.state,
          storageBackendRefs: storageBackendRefs.refs,
          validationRefs: validationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ForgeCredentialStorageCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  expired: entries.filter(entry => entry.state === 'expired').length,
  missing: entries.filter(entry => entry.state === 'missing').length,
  revoked: entries.filter(entry => entry.state === 'revoked').length,
  total: entries.length,
  usable: entries.filter(entry => entry.state === 'usable').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-credential-evidence:${entry.credentialRef}`))

const usablePolicyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'usable' &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `credential-policy-ref-missing:${entry.credentialRef}`))

const usableValidationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'usable' &&
        entry.validationRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `credential-validation-ref-missing:${entry.credentialRef}`),
    )

const usableStorageBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'usable' &&
        entry.storageBackendRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `credential-storage-ref-missing:${entry.credentialRef}`),
    )

const closeoutBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'expired' || entry.state === 'revoked') &&
        entry.rotationRefs.length === 0 &&
        entry.revocationRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `credential-closeout-ref-missing:${entry.credentialRef}`),
    )

const redactionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.redactionClass !== 'public' &&
        entry.redactionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `credential-redaction-ref-missing:${entry.credentialRef}`),
    )

const statusForView = (
  entries: ReadonlyArray<ForgeCredentialStorageItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeCredentialStorageStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.every(entry => entry.state === 'usable')) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'expired' || entry.state === 'revoked')
    ? 'warning'
    : 'unknown'
}

export const projectForgeCredentialStorage = (
  input: ForgeCredentialStorageInput,
): ForgeCredentialStorageView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.kind.localeCompare(right.kind) ||
        left.credentialRef.localeCompare(right.credentialRef),
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
      ...usablePolicyBlockers(input.workOrderRef, entries),
      ...usableValidationBlockers(input.workOrderRef, entries),
      ...usableStorageBlockers(input.workOrderRef, entries),
      ...closeoutBlockers(input.workOrderRef, entries),
      ...redactionBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-credential-storage-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-credential-storage-material-omitted')]),
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

export const buildForgeCredentialStorageInput = (
  work: AutopilotWorkProjection,
): ForgeCredentialStorageInput => {
  const source: AutopilotWorkCredentialStorage | undefined = work.credentialStorage

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
