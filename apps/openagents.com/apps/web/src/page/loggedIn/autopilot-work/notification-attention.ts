import type {
  AutopilotWorkAttentionEntry,
  AutopilotWorkAttentionFreshness,
  AutopilotWorkAttentionSeverity,
  AutopilotWorkAttentionState,
  AutopilotWorkNotificationAttention,
  AutopilotWorkProjection,
} from '../model'

export type ForgeNotificationAttentionStatus =
  | 'attention'
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeNotificationAttentionAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  approvalRequestAuthority: false
  attentionResolutionAuthority: false
  decisionActionAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  notificationSendAuthority: false
  notificationSubscriptionAuthority: false
  providerAuthority: false
  publicClaimAuthority: false
  runStateMutationAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeAttentionItem = Readonly<{
  actionRefs: ReadonlyArray<string>
  attentionRef: string
  blockerRefs: ReadonlyArray<string>
  channelRefs: ReadonlyArray<string>
  decisionRefs: ReadonlyArray<string>
  deliveryRefs: ReadonlyArray<string>
  dedupeRefs: ReadonlyArray<string>
  freshness: AutopilotWorkAttentionFreshness
  invalidationRefs: ReadonlyArray<string>
  notificationRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  resolutionRefs: ReadonlyArray<string>
  severity: AutopilotWorkAttentionSeverity
  state: AutopilotWorkAttentionState
}>

export type ForgeNotificationAttentionInput = Readonly<{
  attention?: ReadonlyArray<AutopilotWorkAttentionEntry>
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeNotificationAttentionCounts = Readonly<{
  active: number
  critical: number
  delivered: number
  total: number
  waiting: number
}>

export type ForgeNotificationAttentionView = Readonly<{
  attention: ReadonlyArray<ForgeAttentionItem>
  authority: ForgeNotificationAttentionAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeNotificationAttentionCounts
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeNotificationAttentionStatus
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
const PRIVATE_ATTENTION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /notification[-_ ](?:body|content|message|text)/i,
  /attention[-_ ](?:body|content|message|text)/i,
  /raw[-_ ](?:body|command|content|diagnostic|file|input|key|log|memory|notification|output|payload|prompt|provider|request|shell|source|stderr|stdout|terminal|trace|transcript)/i,
  /private[-_ ](?:command|content|diagnostic|input|notification|prompt|repo|source|terminal|transcript|workspace)/i,
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

const authority: ForgeNotificationAttentionAuthority = {
  acceptedOutcomeAuthority: false,
  approvalRequestAuthority: false,
  attentionResolutionAuthority: false,
  decisionActionAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  notificationSendAuthority: false,
  notificationSubscriptionAuthority: false,
  providerAuthority: false,
  publicClaimAuthority: false,
  runStateMutationAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_ATTENTION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-notification-attention-blocker:${workOrderRef}:${suffix}`

const normalizeAttention = (
  entry: AutopilotWorkAttentionEntry,
): Readonly<{
  entry: ForgeAttentionItem | null
  omittedUnsafeRefCount: number
}> => {
  const attentionRef = safeOptionalRef(entry.attentionRef)
  const actionRefs = safeRefs(entry.actionRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const channelRefs = safeRefs(entry.channelRefs)
  const decisionRefs = safeRefs(entry.decisionRefs)
  const deliveryRefs = safeRefs(entry.deliveryRefs)
  const dedupeRefs = safeRefs(entry.dedupeRefs)
  const invalidationRefs = safeRefs(entry.invalidationRefs)
  const notificationRefs = safeRefs(entry.notificationRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const resolutionRefs = safeRefs(entry.resolutionRefs)
  const omittedUnsafeRefCount =
    attentionRef.omittedUnsafeRefCount +
    actionRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    channelRefs.omittedUnsafeRefCount +
    decisionRefs.omittedUnsafeRefCount +
    deliveryRefs.omittedUnsafeRefCount +
    dedupeRefs.omittedUnsafeRefCount +
    invalidationRefs.omittedUnsafeRefCount +
    notificationRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    resolutionRefs.omittedUnsafeRefCount

  return attentionRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          actionRefs: actionRefs.refs,
          attentionRef: attentionRef.ref,
          blockerRefs: blockerRefs.refs,
          channelRefs: channelRefs.refs,
          decisionRefs: decisionRefs.refs,
          deliveryRefs: deliveryRefs.refs,
          dedupeRefs: dedupeRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          invalidationRefs: invalidationRefs.refs,
          notificationRefs: notificationRefs.refs,
          policyRefs: policyRefs.refs,
          resolutionRefs: resolutionRefs.refs,
          severity: entry.severity,
          state: entry.state,
        },
        omittedUnsafeRefCount,
      }
}

const attentionCounts = (
  entries: ReadonlyArray<ForgeAttentionItem>,
): ForgeNotificationAttentionCounts => ({
  active: entries.filter(entry => entry.state === 'active').length,
  critical: entries.filter(entry => entry.severity === 'critical').length,
  delivered: entries.filter(entry => entry.deliveryRefs.length > 0).length,
  total: entries.length,
  waiting: entries.filter(entry => entry.state === 'waiting').length,
})

const staleAttentionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeAttentionItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-attention-evidence:${entry.attentionRef}`))

const policyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeAttentionItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'active' || entry.state === 'waiting') &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `attention-policy-missing:${entry.attentionRef}`))

const deliveryBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeAttentionItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.notificationRefs.length > 0 &&
        entry.deliveryRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `notification-delivery-missing:${entry.attentionRef}`))

const actionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeAttentionItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'waiting' &&
        entry.decisionRefs.length === 0 &&
        entry.actionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `decision-action-ref-missing:${entry.attentionRef}`))

const closeoutBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeAttentionItem>,
): ReadonlyArray<string> =>
  entries.flatMap(entry => [
    ...(entry.state === 'resolved' && entry.resolutionRefs.length === 0
      ? [blockerRef(workOrderRef, `resolution-ref-missing:${entry.attentionRef}`)]
      : []),
    ...(entry.state === 'invalidated' && entry.invalidationRefs.length === 0
      ? [blockerRef(workOrderRef, `invalidation-ref-missing:${entry.attentionRef}`)]
      : []),
  ])

const statusForView = (
  entries: ReadonlyArray<ForgeAttentionItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeNotificationAttentionStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'active' || entry.state === 'waiting')) {
    return 'attention'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeNotificationAttention = (
  input: ForgeNotificationAttentionInput,
): ForgeNotificationAttentionView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedAttention = (input.attention ?? []).map(normalizeAttention)
  const attention = normalizedAttention
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.severity.localeCompare(right.severity) ||
        left.attentionRef.localeCompare(right.attentionRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedAttention.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...attention.flatMap(entry => entry.blockerRefs),
      ...staleAttentionBlockers(input.workOrderRef, attention),
      ...policyBlockers(input.workOrderRef, attention),
      ...deliveryBlockers(input.workOrderRef, attention),
      ...actionBlockers(input.workOrderRef, attention),
      ...closeoutBlockers(input.workOrderRef, attention),
      ...(input.attention !== undefined && input.attention.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-attention-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-notification-attention-material-omitted')]),
    ]),
  )

  return {
    attention,
    authority,
    blockerRefs,
    counts: attentionCounts(attention),
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(attention, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeNotificationAttentionInput = (
  work: AutopilotWorkProjection,
): ForgeNotificationAttentionInput => {
  const source: AutopilotWorkNotificationAttention | undefined =
    work.notificationAttention

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.attention === undefined ? {} : { attention: source.attention }),
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
