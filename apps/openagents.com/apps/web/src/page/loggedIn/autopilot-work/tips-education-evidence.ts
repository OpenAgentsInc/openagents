import type {
  AutopilotWorkProjection,
  AutopilotWorkTipsEducationEntry,
  AutopilotWorkTipsEducationFreshness,
  AutopilotWorkTipsEducationStatus,
  AutopilotWorkTipsEducationTopic,
} from '../model'

export type ForgeTipsEducationStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unsupported'
  | 'unknown'

export type ForgeTipsEducationAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  approvalPromptDismissalAuthority: false
  capabilityEnablementAuthority: false
  dismissalMutationAuthority: false
  docsReadAuthority: false
  helpSearchAuthority: false
  paymentActivationAuthority: false
  policyCaveatDismissalAuthority: false
  productClaimMutationAuthority: false
  providerActivationAuthority: false
  settlementActivationAuthority: false
  settlementAuthority: false
  tipRenderingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeTipsEducationItem = Readonly<{
  audienceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  dismissalReceiptRefs: ReadonlyArray<string>
  docsRefs: ReadonlyArray<string>
  expirationRefs: ReadonlyArray<string>
  freshness: AutopilotWorkTipsEducationFreshness
  helpTopicRefs: ReadonlyArray<string>
  liveStateRefs: ReadonlyArray<string>
  nonInteractiveDocsRefs: ReadonlyArray<string>
  nonInteractiveModeRefs: ReadonlyArray<string>
  requiredWarningRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  status: AutopilotWorkTipsEducationStatus
  tipRef: string
  topic: AutopilotWorkTipsEducationTopic
  triggerRefs: ReadonlyArray<string>
  unsupportedClaimRefs: ReadonlyArray<string>
  versionRefs: ReadonlyArray<string>
}>

export type ForgeTipsEducationInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkTipsEducationEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeTipsEducationCounts = Readonly<{
  caveats: number
  dismissed: number
  ready: number
  requiredWarnings: number
  stale: number
  tips: number
  unsupported: number
}>

export type ForgeTipsEducationView = Readonly<{
  authority: ForgeTipsEducationAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTipsEducationCounts
  entries: ReadonlyArray<ForgeTipsEducationItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeTipsEducationStatus
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
const PRIVATE_TIPS_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:copy|data|docs|help|payload|provider|run|secret|tip|token)/i,
  /private[-_ ](?:content|data|docs|payload|provider|ref|run|tip|workspace)/i,
  /docs[-_ ]content/i,
  /help[-_ ]payload/i,
  /payment[-_ ]payload/i,
  /provider[-_ ]payload/i,
  /raw[-_ ]run[-_ ]data/i,
  /secret[-_ ]bearing[-_ ]help/i,
  /tip[-_ ]copy/i,
  /unsupported[-_ ]capability[-_ ]copy/i,
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

const CAVEAT_TOPICS: ReadonlySet<AutopilotWorkTipsEducationTopic> = new Set([
  'payment',
  'payout',
  'provider',
  'settlement',
])

const CAPABILITY_TOPICS: ReadonlySet<AutopilotWorkTipsEducationTopic> = new Set([
  'capability',
  'command',
  'payment',
  'payout',
  'provider',
  'settlement',
  'workflow',
])

const authority: ForgeTipsEducationAuthority = {
  acceptedOutcomeAuthority: false,
  approvalPromptDismissalAuthority: false,
  capabilityEnablementAuthority: false,
  dismissalMutationAuthority: false,
  docsReadAuthority: false,
  helpSearchAuthority: false,
  paymentActivationAuthority: false,
  policyCaveatDismissalAuthority: false,
  productClaimMutationAuthority: false,
  providerActivationAuthority: false,
  settlementActivationAuthority: false,
  settlementAuthority: false,
  tipRenderingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_TIPS_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-tips-education-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkTipsEducationEntry,
): Readonly<{
  item: ForgeTipsEducationItem | null
  omittedUnsafeRefCount: number
}> => {
  const audienceRefs = safeRefs(item.audienceRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const capabilityRefs = safeRefs(item.capabilityRefs)
  const caveatRefs = safeRefs(item.caveatRefs)
  const dismissalReceiptRefs = safeRefs(item.dismissalReceiptRefs)
  const docsRefs = safeRefs(item.docsRefs)
  const expirationRefs = safeRefs(item.expirationRefs)
  const helpTopicRefs = safeRefs(item.helpTopicRefs)
  const liveStateRefs = safeRefs(item.liveStateRefs)
  const nonInteractiveDocsRefs = safeRefs(item.nonInteractiveDocsRefs)
  const nonInteractiveModeRefs = safeRefs(item.nonInteractiveModeRefs)
  const requiredWarningRefs = safeRefs(item.requiredWarningRefs)
  const scopeRefs = safeRefs(item.scopeRefs)
  const tipRef = safeOptionalRef(item.tipRef)
  const triggerRefs = safeRefs(item.triggerRefs)
  const unsupportedClaimRefs = safeRefs(item.unsupportedClaimRefs)
  const versionRefs = safeRefs(item.versionRefs)
  const omittedUnsafeRefCount =
    audienceRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    caveatRefs.omittedUnsafeRefCount +
    dismissalReceiptRefs.omittedUnsafeRefCount +
    docsRefs.omittedUnsafeRefCount +
    expirationRefs.omittedUnsafeRefCount +
    helpTopicRefs.omittedUnsafeRefCount +
    liveStateRefs.omittedUnsafeRefCount +
    nonInteractiveDocsRefs.omittedUnsafeRefCount +
    nonInteractiveModeRefs.omittedUnsafeRefCount +
    requiredWarningRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    tipRef.omittedUnsafeRefCount +
    triggerRefs.omittedUnsafeRefCount +
    unsupportedClaimRefs.omittedUnsafeRefCount +
    versionRefs.omittedUnsafeRefCount

  return tipRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          audienceRefs: audienceRefs.refs,
          blockerRefs: blockerRefs.refs,
          capabilityRefs: capabilityRefs.refs,
          caveatRefs: caveatRefs.refs,
          dismissalReceiptRefs: dismissalReceiptRefs.refs,
          docsRefs: docsRefs.refs,
          expirationRefs: expirationRefs.refs,
          freshness: item.freshness ?? 'unknown',
          helpTopicRefs: helpTopicRefs.refs,
          liveStateRefs: liveStateRefs.refs,
          nonInteractiveDocsRefs: nonInteractiveDocsRefs.refs,
          nonInteractiveModeRefs: nonInteractiveModeRefs.refs,
          requiredWarningRefs: requiredWarningRefs.refs,
          scopeRefs: scopeRefs.refs,
          status: item.status,
          tipRef: tipRef.ref,
          topic: item.topic,
          triggerRefs: triggerRefs.refs,
          unsupportedClaimRefs: unsupportedClaimRefs.refs,
          versionRefs: versionRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeTipsEducationItem>,
): ForgeTipsEducationCounts => ({
  caveats: entries.filter(entry => entry.caveatRefs.length > 0).length,
  dismissed: entries.filter(entry => entry.status === 'dismissed').length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  requiredWarnings: entries.filter(entry => entry.requiredWarningRefs.length > 0)
    .length,
  stale: entries.filter(
    entry =>
      entry.status === 'expired' ||
      entry.freshness === 'stale' ||
      entry.freshness === 'expired',
  ).length,
  tips: entries.length,
  unsupported: entries.filter(
    entry => entry.status === 'unsupported' || entry.unsupportedClaimRefs.length > 0,
  ).length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeTipsEducationItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (
    item.status === 'expired' ||
    item.freshness === 'stale' ||
    item.freshness === 'expired'
  ) {
    blockers.push(blockerRef(workOrderRef, `stale-or-expired-tip:${item.tipRef}`))
  }

  if (
    item.status === 'ready' &&
    CAPABILITY_TOPICS.has(item.topic) &&
    (item.capabilityRefs.length === 0 || item.liveStateRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `capability-tip-live-state-missing:${item.tipRef}`),
    )
  }

  if (item.status === 'dismissed' && item.requiredWarningRefs.length > 0) {
    blockers.push(blockerRef(workOrderRef, `required-warning-dismissed:${item.tipRef}`))
  }

  if (
    item.status === 'dismissed' &&
    item.requiredWarningRefs.length === 0 &&
    item.dismissalReceiptRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `optional-tip-dismissal-receipt-missing:${item.tipRef}`),
    )
  }

  if (CAVEAT_TOPICS.has(item.topic) && item.caveatRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `payment-provider-caveat-missing:${item.tipRef}`),
    )
  }

  if (
    item.nonInteractiveModeRefs.length > 0 &&
    item.nonInteractiveDocsRefs.length === 0 &&
    item.docsRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `non-interactive-doc-ref-missing:${item.tipRef}`),
    )
  }

  if (item.status === 'unsupported' || item.unsupportedClaimRefs.length > 0) {
    blockers.push(
      blockerRef(workOrderRef, `unsupported-capability-education:${item.tipRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeTipsEducationItem>,
  blockers: ReadonlyArray<string>,
): ForgeTipsEducationStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.status === 'unsupported')) {
    return 'unsupported'
  }

  if (
    entries.some(
      entry =>
        entry.status === 'expired' ||
        entry.freshness === 'stale' ||
        entry.freshness === 'expired',
    )
  ) {
    return 'stale'
  }

  if (entries.every(entry => entry.status === 'ready' || entry.status === 'dismissed')) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeTipsEducationEvidence = (
  input: ForgeTipsEducationInput,
): ForgeTipsEducationView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
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
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(blockerRef(input.workOrderRef, 'missing-tips-education-snapshot-ref'))
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(blockerRef(input.workOrderRef, 'unsafe-tips-education-material-omitted'))
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

export const buildForgeTipsEducationInput = (
  work: AutopilotWorkProjection,
): ForgeTipsEducationInput => {
  const evidence = work.tipsEducationEvidence

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
