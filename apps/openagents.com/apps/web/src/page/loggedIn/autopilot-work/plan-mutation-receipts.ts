import type {
  AutopilotWorkPlanMutationAction,
  AutopilotWorkPlanMutationReceipt,
  AutopilotWorkPlanMutationRequest,
  AutopilotWorkPlanMutationState,
  AutopilotWorkProjection,
} from '../model'

export type ForgePlanMutationReceiptsStatus =
  | 'applied'
  | 'blocked'
  | 'empty'
  | 'requested'
  | 'stale'

export type ForgePlanMutationReceiptAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  runCompletionAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgePlanMutationReceiptItem = Readonly<{
  action: AutopilotWorkPlanMutationAction
  actorRef: string
  authority: ForgePlanMutationReceiptAuthority
  blockerRefs: ReadonlyArray<string>
  generatedAt: string
  itemRef: string
  provenanceRefs: ReadonlyArray<string>
  receiptRef: string | null
  requestRef: string
  state: AutopilotWorkPlanMutationState
}>

export type ForgePlanMutationReceiptsView = Readonly<{
  authority: ForgePlanMutationReceiptAuthority
  blockerRefs: ReadonlyArray<string>
  generatedAt: string
  items: ReadonlyArray<ForgePlanMutationReceiptItem>
  omittedUnsafeRefCount: number
  publicSafe: true
  status: ForgePlanMutationReceiptsStatus
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_PLAN_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|plan|todo)/i,
  /private[-_ ](?:repo|content|source|workspace|plan|todo)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice)\b/i,
]

const authority: ForgePlanMutationReceiptAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  runCompletionAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_PLAN_MARKERS.some(marker => marker.test(trimmed))
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
  value: string,
): Readonly<{ omittedUnsafeRefCount: number; ref: string | null }> => {
  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-plan-mutation-blocker:${workOrderRef}:${suffix}`

const completionStateNeedsCloseout = (work: AutopilotWorkProjection): boolean =>
  work.state === 'accepted' ||
  work.state === 'accepted_free_slice' ||
  work.state === 'delivered' ||
  work.state === 'rejected' ||
  work.state === 'revision_required'

const completionConsistencyBlockers = (
  work: AutopilotWorkProjection,
  action: AutopilotWorkPlanMutationAction,
  state: AutopilotWorkPlanMutationState,
): ReadonlyArray<string> =>
  action === 'complete' &&
  state === 'applied' &&
  completionStateNeedsCloseout(work) &&
  work.executionCloseout === null
    ? [blockerRef(work.workOrderRef, 'plan-complete-without-closeout-evidence')]
    : []

const requestedItem = (
  work: AutopilotWorkProjection,
  request: AutopilotWorkPlanMutationRequest,
): Readonly<{
  item: ForgePlanMutationReceiptItem | null
  omittedUnsafeRefCount: number
}> => {
  if (!request.publicSafe) {
    return { item: null, omittedUnsafeRefCount: 1 }
  }

  const actorRef = safeOptionalRef(request.actorRef)
  const itemRef = safeOptionalRef(request.itemRef)
  const requestRef = safeOptionalRef(request.requestRef)
  const provenanceRefs = safeRefs(request.provenanceRefs)
  const omittedUnsafeRefCount =
    actorRef.omittedUnsafeRefCount +
    itemRef.omittedUnsafeRefCount +
    requestRef.omittedUnsafeRefCount +
    provenanceRefs.omittedUnsafeRefCount

  if (actorRef.ref === null || itemRef.ref === null || requestRef.ref === null) {
    return { item: null, omittedUnsafeRefCount }
  }

  return {
    item: {
      action: request.action,
      actorRef: actorRef.ref,
      authority,
      blockerRefs: [],
      generatedAt: request.generatedAt,
      itemRef: itemRef.ref,
      provenanceRefs: provenanceRefs.refs,
      receiptRef: null,
      requestRef: requestRef.ref,
      state: 'requested',
    },
    omittedUnsafeRefCount,
  }
}

const receiptItem = (
  work: AutopilotWorkProjection,
  receipt: AutopilotWorkPlanMutationReceipt,
): Readonly<{
  item: ForgePlanMutationReceiptItem | null
  omittedUnsafeRefCount: number
}> => {
  if (!receipt.publicSafe) {
    return { item: null, omittedUnsafeRefCount: 1 }
  }

  const actorRef = safeOptionalRef(receipt.actorRef)
  const itemRef = safeOptionalRef(receipt.itemRef)
  const receiptRef = safeOptionalRef(receipt.receiptRef)
  const requestRef = safeOptionalRef(receipt.requestRef)
  const provenanceRefs = safeRefs(receipt.provenanceRefs)
  const blockerRefs = safeRefs(receipt.blockerRefs)
  const consistencyBlockers = safeRefs(
    completionConsistencyBlockers(work, receipt.action, receipt.state),
  )
  const omittedUnsafeRefCount =
    actorRef.omittedUnsafeRefCount +
    itemRef.omittedUnsafeRefCount +
    receiptRef.omittedUnsafeRefCount +
    requestRef.omittedUnsafeRefCount +
    provenanceRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    consistencyBlockers.omittedUnsafeRefCount

  if (
    actorRef.ref === null ||
    itemRef.ref === null ||
    receiptRef.ref === null ||
    requestRef.ref === null
  ) {
    return { item: null, omittedUnsafeRefCount }
  }

  return {
    item: {
      action: receipt.action,
      actorRef: actorRef.ref,
      authority,
      blockerRefs: [...blockerRefs.refs, ...consistencyBlockers.refs],
      generatedAt: receipt.generatedAt,
      itemRef: itemRef.ref,
      provenanceRefs: provenanceRefs.refs,
      receiptRef: receiptRef.ref,
      requestRef: requestRef.ref,
      state: receipt.state,
    },
    omittedUnsafeRefCount,
  }
}

const itemSortKey = (item: ForgePlanMutationReceiptItem): string =>
  `${item.generatedAt}|${item.receiptRef ?? item.requestRef}|${item.itemRef}`

const viewStatus = (
  items: ReadonlyArray<ForgePlanMutationReceiptItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgePlanMutationReceiptsStatus => {
  if (blockerRefs.length > 0 || items.some(item => item.state === 'blocked')) {
    return 'blocked'
  }

  if (items.some(item => item.state === 'stale')) {
    return 'stale'
  }

  if (items.some(item => item.state === 'requested')) {
    return 'requested'
  }

  return items.length === 0 ? 'empty' : 'applied'
}

export const projectForgePlanMutationReceipts = (
  work: AutopilotWorkProjection,
): ForgePlanMutationReceiptsView => {
  const requested = (work.planMutationRequests ?? []).map(request =>
    requestedItem(work, request)
  )
  const receipts = (work.planMutationReceipts ?? []).map(receipt =>
    receiptItem(work, receipt)
  )
  const omittedUnsafeRefCount = [...requested, ...receipts].reduce(
    (count, result) => count + result.omittedUnsafeRefCount,
    0,
  )
  const items = [...requested, ...receipts]
    .flatMap(result => (result.item === null ? [] : [result.item]))
    .sort((a, b) => itemSortKey(a).localeCompare(itemSortKey(b)))
  const itemBlockers = items.flatMap(item => item.blockerRefs)
  const blockerRefs = [
    ...itemBlockers,
    ...(omittedUnsafeRefCount === 0
      ? []
      : [blockerRef(work.workOrderRef, 'unsafe-plan-mutation-material-omitted')]),
  ]

  return {
    authority,
    blockerRefs: Array.from(new Set(blockerRefs)),
    generatedAt: work.generatedAt,
    items,
    omittedUnsafeRefCount,
    publicSafe: true,
    status: viewStatus(items, blockerRefs),
    workOrderRef: work.workOrderRef,
  }
}
