import type {
  AutopilotWorkProjection,
  AutopilotWorkTelemetryPrivacy,
  AutopilotWorkTelemetryPrivacyClass,
  AutopilotWorkTelemetryPrivacyClassKind,
  AutopilotWorkTelemetryPrivacyFreshness,
  AutopilotWorkTelemetryPrivacyMode,
  AutopilotWorkTelemetryPrivacyStatus,
} from '../model'

export type ForgeTelemetryPrivacyStatus =
  | 'blocked'
  | 'disabled'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeTelemetryPrivacyAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  diagnosticExportAuthority: false
  privacyFilterBypassAuthority: false
  publicClaimAuthority: false
  retentionDeletionAuthority: false
  settlementAuthority: false
  sinkActivationAuthority: false
  telemetryEmitAuthority: false
  telemetryModeWriteAuthority: false
  usageBillingMutationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeTelemetryPrivacyItem = Readonly<{
  aggregateRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  classKind: AutopilotWorkTelemetryPrivacyClassKind
  diagnosticBundleRefs: ReadonlyArray<string>
  deliveryRefs: ReadonlyArray<string>
  exportabilityRefs: ReadonlyArray<string>
  failureRefs: ReadonlyArray<string>
  freshness: AutopilotWorkTelemetryPrivacyFreshness
  mode: AutopilotWorkTelemetryPrivacyMode
  optOutRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  privacyFilterRefs: ReadonlyArray<string>
  redactionScanRefs: ReadonlyArray<string>
  retentionRefs: ReadonlyArray<string>
  sinkRefs: ReadonlyArray<string>
  status: AutopilotWorkTelemetryPrivacyStatus
  telemetryRef: string
  visibilityRefs: ReadonlyArray<string>
}>

export type ForgeTelemetryPrivacyInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  classes?: ReadonlyArray<AutopilotWorkTelemetryPrivacyClass>
  generatedAt: string
  modeRefs?: ReadonlyArray<string>
  optOutRefs?: ReadonlyArray<string>
  policyRefs?: ReadonlyArray<string>
  privacyFilterRefs?: ReadonlyArray<string>
  redactionScanRefs?: ReadonlyArray<string>
  retentionRefs?: ReadonlyArray<string>
  sinkRefs?: ReadonlyArray<string>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeTelemetryPrivacyCounts = Readonly<{
  disabled: number
  enabled: number
  failed: number
  product: number
  stale: number
  telemetryClasses: number
}>

export type ForgeTelemetryPrivacyView = Readonly<{
  authority: ForgeTelemetryPrivacyAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTelemetryPrivacyCounts
  generatedAt: string
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>
  modeRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  optOutRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  privacyFilterRefs: ReadonlyArray<string>
  publicSafe: true
  redactionScanRefs: ReadonlyArray<string>
  retentionRefs: ReadonlyArray<string>
  sinkRefs: ReadonlyArray<string>
  snapshotRef: string | null
  status: ForgeTelemetryPrivacyStatus
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
const PRIVATE_TELEMETRY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:command|content|event|file|invoice|log|output|payload|prompt|provider|shell|telemetry|transcript|wallet)/i,
  /private[-_ ](?:code|content|file|invoice|log|payload|prompt|repo|source|telemetry|transcript|workspace)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /wallet[-_ ](?:data|material|mnemonic|private)/i,
  /customer[-_ ](?:data|private|payload|record)/i,
  /invoice[-_ ](?:data|raw|record)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeTelemetryPrivacyAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  diagnosticExportAuthority: false,
  privacyFilterBypassAuthority: false,
  publicClaimAuthority: false,
  retentionDeletionAuthority: false,
  settlementAuthority: false,
  sinkActivationAuthority: false,
  telemetryEmitAuthority: false,
  telemetryModeWriteAuthority: false,
  usageBillingMutationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_TELEMETRY_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-telemetry-privacy-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkTelemetryPrivacyClass,
): Readonly<{
  item: ForgeTelemetryPrivacyItem | null
  omittedUnsafeRefCount: number
}> => {
  const aggregateRefs = safeRefs(item.aggregateRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const diagnosticBundleRefs = safeRefs(item.diagnosticBundleRefs)
  const deliveryRefs = safeRefs(item.deliveryRefs)
  const exportabilityRefs = safeRefs(item.exportabilityRefs)
  const failureRefs = safeRefs(item.failureRefs)
  const optOutRefs = safeRefs(item.optOutRefs)
  const policyRefs = safeRefs(item.policyRefs)
  const privacyFilterRefs = safeRefs(item.privacyFilterRefs)
  const redactionScanRefs = safeRefs(item.redactionScanRefs)
  const retentionRefs = safeRefs(item.retentionRefs)
  const sinkRefs = safeRefs(item.sinkRefs)
  const telemetryRef = safeOptionalRef(item.telemetryRef)
  const visibilityRefs = safeRefs(item.visibilityRefs)
  const omittedUnsafeRefCount =
    aggregateRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    diagnosticBundleRefs.omittedUnsafeRefCount +
    deliveryRefs.omittedUnsafeRefCount +
    exportabilityRefs.omittedUnsafeRefCount +
    failureRefs.omittedUnsafeRefCount +
    optOutRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    privacyFilterRefs.omittedUnsafeRefCount +
    redactionScanRefs.omittedUnsafeRefCount +
    retentionRefs.omittedUnsafeRefCount +
    sinkRefs.omittedUnsafeRefCount +
    telemetryRef.omittedUnsafeRefCount +
    visibilityRefs.omittedUnsafeRefCount

  return telemetryRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          aggregateRefs: aggregateRefs.refs,
          blockerRefs: blockerRefs.refs,
          classKind: item.classKind,
          diagnosticBundleRefs: diagnosticBundleRefs.refs,
          deliveryRefs: deliveryRefs.refs,
          exportabilityRefs: exportabilityRefs.refs,
          failureRefs: failureRefs.refs,
          freshness: item.freshness ?? 'unknown',
          mode: item.mode,
          optOutRefs: optOutRefs.refs,
          policyRefs: policyRefs.refs,
          privacyFilterRefs: privacyFilterRefs.refs,
          redactionScanRefs: redactionScanRefs.refs,
          retentionRefs: retentionRefs.refs,
          sinkRefs: sinkRefs.refs,
          status: item.status,
          telemetryRef: telemetryRef.ref,
          visibilityRefs: visibilityRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ForgeTelemetryPrivacyCounts => ({
  disabled: items.filter(item => item.status === 'disabled').length,
  enabled: items.filter(item => item.status === 'enabled').length,
  failed: items.filter(item => item.status === 'failed').length,
  product: items.filter(item => item.classKind === 'product_metrics').length,
  stale: items.filter(item => item.freshness === 'stale').length,
  telemetryClasses: items.length,
})

const staleBlockers = (
  workOrderRef: string,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(item => item.freshness === 'stale' && item.blockerRefs.length === 0)
    .map(item =>
      blockerRef(workOrderRef, `stale-telemetry-evidence:${item.telemetryRef}`),
    )

const productOptOutBlockers = (
  workOrderRef: string,
  globalOptOutRefs: ReadonlyArray<string>,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(
      item =>
        item.classKind === 'product_metrics' &&
        item.status === 'enabled' &&
        (item.optOutRefs.length > 0 || globalOptOutRefs.length > 0) &&
        item.blockerRefs.length === 0,
    )
    .map(item =>
      blockerRef(workOrderRef, `product-telemetry-opted-out:${item.telemetryRef}`),
    )

const enabledSinkBlockers = (
  workOrderRef: string,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(
      item =>
        item.status === 'enabled' &&
        (item.sinkRefs.length === 0 || item.policyRefs.length === 0) &&
        item.blockerRefs.length === 0,
    )
    .map(item =>
      blockerRef(workOrderRef, `enabled-telemetry-sink-policy-missing:${item.telemetryRef}`),
    )

const classMetadataBlockers = (
  workOrderRef: string,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(
      item =>
        (item.visibilityRefs.length === 0 ||
          item.retentionRefs.length === 0 ||
          item.exportabilityRefs.length === 0) &&
        item.blockerRefs.length === 0,
    )
    .map(item =>
      blockerRef(workOrderRef, `telemetry-class-metadata-missing:${item.telemetryRef}`),
    )

const diagnosticExportBlockers = (
  workOrderRef: string,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(
      item =>
        item.diagnosticBundleRefs.length > 0 &&
        (item.redactionScanRefs.length === 0 || item.retentionRefs.length === 0) &&
        item.blockerRefs.length === 0,
    )
    .map(item =>
      blockerRef(
        workOrderRef,
        `diagnostic-export-redaction-retention-missing:${item.telemetryRef}`,
      ),
    )

const deliveryFailureBlockers = (
  workOrderRef: string,
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
): ReadonlyArray<string> =>
  items
    .filter(
      item =>
        item.status === 'failed' &&
        item.failureRefs.length === 0 &&
        item.blockerRefs.length === 0,
    )
    .map(item =>
      blockerRef(workOrderRef, `telemetry-delivery-failure-missing:${item.telemetryRef}`),
    )

const statusForView = (
  items: ReadonlyArray<ForgeTelemetryPrivacyItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeTelemetryPrivacyStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (items.length === 0) {
    return 'empty'
  }

  if (items.every(item => item.status === 'disabled')) {
    return 'disabled'
  }

  if (items.some(item => item.freshness === 'stale')) {
    return 'stale'
  }

  return items.every(item => item.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeTelemetryPrivacy = (
  input: ForgeTelemetryPrivacyInput,
): ForgeTelemetryPrivacyView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const modeRefs = safeRefs(input.modeRefs)
  const optOutRefs = safeRefs(input.optOutRefs)
  const policyRefs = safeRefs(input.policyRefs)
  const privacyFilterRefs = safeRefs(input.privacyFilterRefs)
  const redactionScanRefs = safeRefs(input.redactionScanRefs)
  const retentionRefs = safeRefs(input.retentionRefs)
  const sinkRefs = safeRefs(input.sinkRefs)
  const normalizedItems = (input.classes ?? []).map(normalizeItem)
  const items = normalizedItems
    .flatMap(result => (result.item === null ? [] : [result.item]))
    .sort(
      (left, right) =>
        left.classKind.localeCompare(right.classKind) ||
        left.telemetryRef.localeCompare(right.telemetryRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    modeRefs.omittedUnsafeRefCount +
    optOutRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    privacyFilterRefs.omittedUnsafeRefCount +
    redactionScanRefs.omittedUnsafeRefCount +
    retentionRefs.omittedUnsafeRefCount +
    sinkRefs.omittedUnsafeRefCount +
    normalizedItems.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const hasEntries = (input.classes ?? []).length > 0
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...items.flatMap(item => item.blockerRefs),
      ...staleBlockers(input.workOrderRef, items),
      ...productOptOutBlockers(input.workOrderRef, optOutRefs.refs, items),
      ...enabledSinkBlockers(input.workOrderRef, items),
      ...classMetadataBlockers(input.workOrderRef, items),
      ...diagnosticExportBlockers(input.workOrderRef, items),
      ...deliveryFailureBlockers(input.workOrderRef, items),
      ...(hasEntries && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-telemetry-privacy-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-telemetry-privacy-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(items),
    generatedAt: input.generatedAt,
    items,
    modeRefs: modeRefs.refs,
    omittedUnsafeRefCount,
    optOutRefs: optOutRefs.refs,
    policyRefs: policyRefs.refs,
    privacyFilterRefs: privacyFilterRefs.refs,
    publicSafe: true,
    redactionScanRefs: redactionScanRefs.refs,
    retentionRefs: retentionRefs.refs,
    sinkRefs: sinkRefs.refs,
    snapshotRef: snapshotRef.ref,
    status: statusForView(items, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeTelemetryPrivacyInput = (
  work: AutopilotWorkProjection,
): ForgeTelemetryPrivacyInput => {
  const source: AutopilotWorkTelemetryPrivacy | undefined = work.telemetryPrivacy

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
    ...(source.classes === undefined ? {} : { classes: source.classes }),
    ...(source.modeRefs === undefined ? {} : { modeRefs: source.modeRefs }),
    ...(source.optOutRefs === undefined ? {} : { optOutRefs: source.optOutRefs }),
    ...(source.policyRefs === undefined ? {} : { policyRefs: source.policyRefs }),
    ...(source.privacyFilterRefs === undefined
      ? {}
      : { privacyFilterRefs: source.privacyFilterRefs }),
    ...(source.redactionScanRefs === undefined
      ? {}
      : { redactionScanRefs: source.redactionScanRefs }),
    ...(source.retentionRefs === undefined
      ? {}
      : { retentionRefs: source.retentionRefs }),
    ...(source.sinkRefs === undefined ? {} : { sinkRefs: source.sinkRefs }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
