import type {
  AutopilotWorkProjection,
  AutopilotWorkThemeVisualEntry,
  AutopilotWorkThemeVisualFreshness,
  AutopilotWorkThemeVisualStatus,
  AutopilotWorkThemeVisualSurface,
} from '../model'

export type ForgeThemeVisualStatus = 'blocked' | 'empty' | 'ready' | 'stale' | 'unknown'

export type ForgeThemeVisualAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  cssInjectionAuthority: false
  managedPolicyMutationAuthority: false
  productClaimMutationAuthority: false
  preferenceWriteAuthority: false
  remoteThemeExecutionAuthority: false
  rendererMutationAuthority: false
  runtimeStatusMutationAuthority: false
  settlementAuthority: false
  themeInstallAuthority: false
  visualSnapshotAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeThemeVisualItem = Readonly<{
  attentionColorRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  contrastCheckRefs: ReadonlyArray<string>
  crossSurfaceRefs: ReadonlyArray<string>
  densityRefs: ReadonlyArray<string>
  diffColorRefs: ReadonlyArray<string>
  focusRingRefs: ReadonlyArray<string>
  freshness: AutopilotWorkThemeVisualFreshness
  highContrastRefs: ReadonlyArray<string>
  managedPolicyRefs: ReadonlyArray<string>
  monochromeRefs: ReadonlyArray<string>
  progressColorRefs: ReadonlyArray<string>
  reducedMotionRefs: ReadonlyArray<string>
  runtimeReceiptRefs: ReadonlyArray<string>
  snapshotRefs: ReadonlyArray<string>
  status: AutopilotWorkThemeVisualStatus
  statusIconRefs: ReadonlyArray<string>
  statusLabelRefs: ReadonlyArray<string>
  statusVisualRefs: ReadonlyArray<string>
  surface: AutopilotWorkThemeVisualSurface
  themeRef: string
  tokenRefs: ReadonlyArray<string>
  typographyRefs: ReadonlyArray<string>
  warningPreservationRefs: ReadonlyArray<string>
}>

export type ForgeThemeVisualInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkThemeVisualEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeThemeVisualCounts = Readonly<{
  highContrast: number
  managed: number
  ready: number
  reducedMotion: number
  stale: number
  surfaces: number
}>

export type ForgeThemeVisualView = Readonly<{
  authority: ForgeThemeVisualAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeThemeVisualCounts
  entries: ReadonlyArray<ForgeThemeVisualItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeThemeVisualStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{ omittedUnsafeRefCount: number; refs: ReadonlyArray<string> }>
type OptionalRefBundle = Readonly<{ omittedUnsafeRefCount: number; ref: string | null }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_THEME_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:css|file|theme|token|visual|snapshot|payload|code)/i,
  /private[-_ ](?:branding|content|data|file|payload|snapshot|theme|workspace)/i,
  /executable[-_ ]theme/i,
  /plugin[-_ ]theme[-_ ]code/i,
  /remote[-_ ]theme[-_ ]code/i,
  /unsupported[-_ ](?:green|success)[-_ ]claim/i,
  /visual[-_ ]snapshot[-_ ]content/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret)\b/i,
  /\b(?:admin|auth|bearer|operator|private|refresh|secret|service|session)[_-]?token\b/i,
]

const SURFACE_COUNT = 4

const authority: ForgeThemeVisualAuthority = {
  acceptedOutcomeAuthority: false,
  cssInjectionAuthority: false,
  managedPolicyMutationAuthority: false,
  productClaimMutationAuthority: false,
  preferenceWriteAuthority: false,
  remoteThemeExecutionAuthority: false,
  rendererMutationAuthority: false,
  runtimeStatusMutationAuthority: false,
  settlementAuthority: false,
  themeInstallAuthority: false,
  visualSnapshotAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_THEME_MARKERS.some(marker => marker.test(trimmed))
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

const safeOptionalRef = (value: string | null | undefined): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-theme-visual-blocker:${workOrderRef}:${suffix}`

const hasRole = (refs: ReadonlyArray<string>, pattern: RegExp): boolean =>
  refs.some(ref => pattern.test(ref))

const normalizeItem = (
  item: AutopilotWorkThemeVisualEntry,
): Readonly<{ item: ForgeThemeVisualItem | null; omittedUnsafeRefCount: number }> => {
  const attentionColorRefs = safeRefs(item.attentionColorRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const contrastCheckRefs = safeRefs(item.contrastCheckRefs)
  const crossSurfaceRefs = safeRefs(item.crossSurfaceRefs)
  const densityRefs = safeRefs(item.densityRefs)
  const diffColorRefs = safeRefs(item.diffColorRefs)
  const focusRingRefs = safeRefs(item.focusRingRefs)
  const highContrastRefs = safeRefs(item.highContrastRefs)
  const managedPolicyRefs = safeRefs(item.managedPolicyRefs)
  const monochromeRefs = safeRefs(item.monochromeRefs)
  const progressColorRefs = safeRefs(item.progressColorRefs)
  const reducedMotionRefs = safeRefs(item.reducedMotionRefs)
  const runtimeReceiptRefs = safeRefs(item.runtimeReceiptRefs)
  const snapshotRefs = safeRefs(item.snapshotRefs)
  const statusIconRefs = safeRefs(item.statusIconRefs)
  const statusLabelRefs = safeRefs(item.statusLabelRefs)
  const statusVisualRefs = safeRefs(item.statusVisualRefs)
  const themeRef = safeOptionalRef(item.themeRef)
  const tokenRefs = safeRefs(item.tokenRefs)
  const typographyRefs = safeRefs(item.typographyRefs)
  const warningPreservationRefs = safeRefs(item.warningPreservationRefs)
  const omittedUnsafeRefCount =
    attentionColorRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    contrastCheckRefs.omittedUnsafeRefCount +
    crossSurfaceRefs.omittedUnsafeRefCount +
    densityRefs.omittedUnsafeRefCount +
    diffColorRefs.omittedUnsafeRefCount +
    focusRingRefs.omittedUnsafeRefCount +
    highContrastRefs.omittedUnsafeRefCount +
    managedPolicyRefs.omittedUnsafeRefCount +
    monochromeRefs.omittedUnsafeRefCount +
    progressColorRefs.omittedUnsafeRefCount +
    reducedMotionRefs.omittedUnsafeRefCount +
    runtimeReceiptRefs.omittedUnsafeRefCount +
    snapshotRefs.omittedUnsafeRefCount +
    statusIconRefs.omittedUnsafeRefCount +
    statusLabelRefs.omittedUnsafeRefCount +
    statusVisualRefs.omittedUnsafeRefCount +
    themeRef.omittedUnsafeRefCount +
    tokenRefs.omittedUnsafeRefCount +
    typographyRefs.omittedUnsafeRefCount +
    warningPreservationRefs.omittedUnsafeRefCount

  return themeRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          attentionColorRefs: attentionColorRefs.refs,
          blockerRefs: blockerRefs.refs,
          contrastCheckRefs: contrastCheckRefs.refs,
          crossSurfaceRefs: crossSurfaceRefs.refs,
          densityRefs: densityRefs.refs,
          diffColorRefs: diffColorRefs.refs,
          focusRingRefs: focusRingRefs.refs,
          freshness: item.freshness ?? 'unknown',
          highContrastRefs: highContrastRefs.refs,
          managedPolicyRefs: managedPolicyRefs.refs,
          monochromeRefs: monochromeRefs.refs,
          progressColorRefs: progressColorRefs.refs,
          reducedMotionRefs: reducedMotionRefs.refs,
          runtimeReceiptRefs: runtimeReceiptRefs.refs,
          snapshotRefs: snapshotRefs.refs,
          status: item.status,
          statusIconRefs: statusIconRefs.refs,
          statusLabelRefs: statusLabelRefs.refs,
          statusVisualRefs: statusVisualRefs.refs,
          surface: item.surface,
          themeRef: themeRef.ref,
          tokenRefs: tokenRefs.refs,
          typographyRefs: typographyRefs.refs,
          warningPreservationRefs: warningPreservationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (entries: ReadonlyArray<ForgeThemeVisualItem>): ForgeThemeVisualCounts => ({
  highContrast: entries.filter(entry => entry.highContrastRefs.length > 0).length,
  managed: entries.filter(entry => entry.managedPolicyRefs.length > 0).length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  reducedMotion: entries.filter(entry => entry.reducedMotionRefs.length > 0).length,
  stale: entries.filter(entry => entry.freshness === 'stale' || entry.status === 'stale').length,
  surfaces: entries.length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeThemeVisualItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const successVisual = hasRole(item.statusVisualRefs, /success|green|positive/i)
  const warningVisual = hasRole(item.statusVisualRefs, /warning|failure|waiting|danger|error/i)

  if (item.freshness === 'stale' || item.status === 'stale') {
    blockers.push(blockerRef(workOrderRef, `stale-theme-visual-evidence:${item.themeRef}`))
  }

  if (successVisual && item.runtimeReceiptRefs.length === 0) {
    blockers.push(blockerRef(workOrderRef, `success-visual-runtime-receipt-missing:${item.themeRef}`))
  }

  if (
    warningVisual &&
    (item.statusLabelRefs.length === 0 ||
      item.statusIconRefs.length === 0 ||
      item.contrastCheckRefs.length === 0 ||
      item.monochromeRefs.length === 0)
  ) {
    blockers.push(blockerRef(workOrderRef, `warning-visual-accessibility-evidence-missing:${item.themeRef}`))
  }

  if (item.managedPolicyRefs.length > 0 && item.highContrastRefs.length === 0) {
    blockers.push(blockerRef(workOrderRef, `managed-theme-high-contrast-evidence-missing:${item.themeRef}`))
  }

  if (warningVisual && item.densityRefs.length > 0 && item.warningPreservationRefs.length === 0) {
    blockers.push(blockerRef(workOrderRef, `critical-warning-preservation-missing:${item.themeRef}`))
  }

  if (item.status === 'ready' && item.crossSurfaceRefs.length < SURFACE_COUNT) {
    blockers.push(blockerRef(workOrderRef, `cross-surface-visual-consistency-missing:${item.themeRef}`))
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeThemeVisualItem>,
  blockers: ReadonlyArray<string>,
): ForgeThemeVisualStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }
  if (entries.length === 0) {
    return 'empty'
  }
  if (entries.some(entry => entry.freshness === 'stale' || entry.status === 'stale')) {
    return 'stale'
  }
  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }
  return 'unknown'
}

export const projectForgeThemeVisualEvidence = (
  input: ForgeThemeVisualInput,
): ForgeThemeVisualView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result => (result.item === null ? [] : [result.item]))
  const normalizedOmissions = normalized.reduce(
    (total, result) => total + result.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(blockerRef(input.workOrderRef, 'missing-theme-visual-snapshot-ref'))
  }
  if (omittedUnsafeRefCount > 0) {
    blockers.push(blockerRef(input.workOrderRef, 'unsafe-theme-visual-material-omitted'))
  }

  const uniqueBlockers = Array.from(new Set(blockers))

  return {
    authority,
    blockerRefs: uniqueBlockers,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeThemeVisualInput = (
  work: AutopilotWorkProjection,
): ForgeThemeVisualInput => {
  const evidence = work.themeVisualEvidence

  return {
    generatedAt: evidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(evidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: evidence.blockerRefs }),
    ...(evidence?.entries === undefined ? {} : { entries: evidence.entries }),
    ...(evidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: evidence.snapshotRef }),
    ...(evidence?.versionRef === undefined
      ? {}
      : { versionRef: evidence.versionRef }),
  }
}
