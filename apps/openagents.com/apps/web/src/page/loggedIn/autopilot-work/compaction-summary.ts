import type {
  AutopilotWorkCompaction,
  AutopilotWorkCompactionBoundary,
  AutopilotWorkCompactionEstimate,
  AutopilotWorkCompactionState,
  AutopilotWorkCompactionStrategy,
  AutopilotWorkCompactionToolPair,
  AutopilotWorkCompactionTrigger,
  AutopilotWorkProjection,
} from '../model'

export type ForgeCompactionSummaryStatus =
  | 'blocked'
  | 'cancelled'
  | 'compacted'
  | 'empty'
  | 'failed'
  | 'pending'

export type ForgeCompactionAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  automaticCompactionAuthority: false
  deploymentAuthority: false
  modelSummarizationAuthority: false
  publicClaimAuthority: false
  runtimeRetryAuthority: false
  settlementAuthority: false
  transcriptMutationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeCompactionEstimate = Readonly<{
  contextWindow: number | null
  estimateRef: string | null
  messageCount: number | null
  tokenCount: number | null
}>

export type ForgeCompactionToolPair = Readonly<{
  requestRef: string
  resultRef: string | null
  summaryRef: string | null
}>

export type ForgeCompactionBoundaryItem = Readonly<{
  automaticFailureCount: number
  blockerRefs: ReadonlyArray<string>
  boundaryRef: string
  failureRefs: ReadonlyArray<string>
  generatedAt: string
  hookRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  postEstimate: ForgeCompactionEstimate | null
  preEstimate: ForgeCompactionEstimate | null
  preservedAdapterRefs: ReadonlyArray<string>
  preservedPlanRefs: ReadonlyArray<string>
  preservedRecentMessageRefs: ReadonlyArray<string>
  preservedTaskRefs: ReadonlyArray<string>
  preservedToolPairs: ReadonlyArray<ForgeCompactionToolPair>
  publicMessage: string | null
  restoredAdapterRefs: ReadonlyArray<string>
  restoredFileRefs: ReadonlyArray<string>
  restoredPlanRefs: ReadonlyArray<string>
  restoredSkillRefs: ReadonlyArray<string>
  restoredTaskRefs: ReadonlyArray<string>
  retryRefs: ReadonlyArray<string>
  state: AutopilotWorkCompactionState
  strategy: AutopilotWorkCompactionStrategy
  summarySourceRefs: ReadonlyArray<string>
  trigger: AutopilotWorkCompactionTrigger
}>

export type ForgeCompactionSummaryInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  boundaries?: ReadonlyArray<AutopilotWorkCompactionBoundary>
  compactionRef?: string
  generatedAt: string
  workOrderRef: string
}>

export type ForgeCompactionSummaryView = Readonly<{
  authority: ForgeCompactionAuthority
  blockerRefs: ReadonlyArray<string>
  boundaries: ReadonlyArray<ForgeCompactionBoundaryItem>
  compactionRef: string
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  status: ForgeCompactionSummaryStatus
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

type MessageBundle = Readonly<{
  message: string | null
  omittedUnsafeRefCount: number
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_COMPACTION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:content|diagnostic|document|dropped|file|log|patch|payload|prompt|provider|shell|source|stderr|stdout|summary|trace|transcript)/i,
  /private[-_ ](?:content|diagnostic|repo|source|summary|transcript|workspace)/i,
  /provider[-_ ]payload/i,
  /shell[-_ ](?:log|output|transcript)/i,
  /transcript[-_ ](?:body|content|raw)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeCompactionAuthority = {
  acceptedOutcomeAuthority: false,
  automaticCompactionAuthority: false,
  deploymentAuthority: false,
  modelSummarizationAuthority: false,
  publicClaimAuthority: false,
  runtimeRetryAuthority: false,
  settlementAuthority: false,
  transcriptMutationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_COMPACTION_MARKERS.some(marker => marker.test(trimmed))
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

const safePublicMessage = (
  value: string | null | undefined,
): MessageBundle => {
  if (value === null || value === undefined) {
    return { message: null, omittedUnsafeRefCount: 0 }
  }

  const trimmed = value.trim()
  const safe =
    trimmed.length > 0 &&
    trimmed.length <= 220 &&
    !/[\r\n]/.test(trimmed) &&
    !PRIVATE_COMPACTION_MARKERS.some(marker => marker.test(trimmed))

  return safe
    ? { message: trimmed, omittedUnsafeRefCount: 0 }
    : { message: null, omittedUnsafeRefCount: 1 }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-compaction-blocker:${workOrderRef}:${suffix}`

const fallbackCompactionRef = (workOrderRef: string): string =>
  `compaction.public.${workOrderRef}.derived`

const normalizeEstimate = (
  estimate: AutopilotWorkCompactionEstimate | undefined,
): Readonly<{
  estimate: ForgeCompactionEstimate | null
  omittedUnsafeRefCount: number
}> => {
  if (estimate === undefined) {
    return { estimate: null, omittedUnsafeRefCount: 0 }
  }

  const estimateRef = safeOptionalRef(estimate.estimateRef)

  return {
    estimate: {
      contextWindow: estimate.contextWindow ?? null,
      estimateRef: estimateRef.ref,
      messageCount: estimate.messageCount ?? null,
      tokenCount: estimate.tokenCount ?? null,
    },
    omittedUnsafeRefCount: estimateRef.omittedUnsafeRefCount,
  }
}

const normalizeToolPair = (
  pair: AutopilotWorkCompactionToolPair,
): Readonly<{
  omittedUnsafeRefCount: number
  pair: ForgeCompactionToolPair | null
}> => {
  const requestRef = safeOptionalRef(pair.requestRef)
  const resultRef = safeOptionalRef(pair.resultRef)
  const summaryRef = safeOptionalRef(pair.summaryRef)
  const omittedUnsafeRefCount =
    requestRef.omittedUnsafeRefCount +
    resultRef.omittedUnsafeRefCount +
    summaryRef.omittedUnsafeRefCount

  return requestRef.ref === null
    ? { omittedUnsafeRefCount, pair: null }
    : {
        omittedUnsafeRefCount,
        pair: {
          requestRef: requestRef.ref,
          resultRef: resultRef.ref,
          summaryRef: summaryRef.ref,
        },
      }
}

const normalizeBoundary = (
  boundary: AutopilotWorkCompactionBoundary,
): Readonly<{
  boundary: ForgeCompactionBoundaryItem | null
  omittedUnsafeRefCount: number
}> => {
  if (!boundary.publicSafe) {
    return { boundary: null, omittedUnsafeRefCount: 1 }
  }

  const boundaryRef = safeOptionalRef(boundary.boundaryRef)
  const blockerRefs = safeRefs(boundary.blockerRefs)
  const failureRefs = safeRefs(boundary.failureRefs)
  const hookRefs = safeRefs(boundary.hookRefs)
  const policyRefs = safeRefs(boundary.policyRefs)
  const preservedAdapterRefs = safeRefs(boundary.preservedAdapterRefs)
  const preservedPlanRefs = safeRefs(boundary.preservedPlanRefs)
  const preservedRecentMessageRefs = safeRefs(boundary.preservedRecentMessageRefs)
  const preservedTaskRefs = safeRefs(boundary.preservedTaskRefs)
  const restoredAdapterRefs = safeRefs(boundary.restoredAdapterRefs)
  const restoredFileRefs = safeRefs(boundary.restoredFileRefs)
  const restoredPlanRefs = safeRefs(boundary.restoredPlanRefs)
  const restoredSkillRefs = safeRefs(boundary.restoredSkillRefs)
  const restoredTaskRefs = safeRefs(boundary.restoredTaskRefs)
  const retryRefs = safeRefs(boundary.retryRefs)
  const summarySourceRefs = safeRefs(boundary.summarySourceRefs)
  const preEstimate = normalizeEstimate(boundary.preEstimate)
  const postEstimate = normalizeEstimate(boundary.postEstimate)
  const publicMessage = safePublicMessage(boundary.publicMessage)
  const preservedToolPairs = (boundary.preservedToolPairs ?? []).map(
    normalizeToolPair,
  )
  const omittedUnsafeRefCount =
    boundaryRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    failureRefs.omittedUnsafeRefCount +
    hookRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    preservedAdapterRefs.omittedUnsafeRefCount +
    preservedPlanRefs.omittedUnsafeRefCount +
    preservedRecentMessageRefs.omittedUnsafeRefCount +
    preservedTaskRefs.omittedUnsafeRefCount +
    restoredAdapterRefs.omittedUnsafeRefCount +
    restoredFileRefs.omittedUnsafeRefCount +
    restoredPlanRefs.omittedUnsafeRefCount +
    restoredSkillRefs.omittedUnsafeRefCount +
    restoredTaskRefs.omittedUnsafeRefCount +
    retryRefs.omittedUnsafeRefCount +
    summarySourceRefs.omittedUnsafeRefCount +
    preEstimate.omittedUnsafeRefCount +
    postEstimate.omittedUnsafeRefCount +
    publicMessage.omittedUnsafeRefCount +
    preservedToolPairs.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)

  return boundaryRef.ref === null
    ? { boundary: null, omittedUnsafeRefCount }
    : {
        boundary: {
          automaticFailureCount: boundary.automaticFailureCount ?? 0,
          blockerRefs: blockerRefs.refs,
          boundaryRef: boundaryRef.ref,
          failureRefs: failureRefs.refs,
          generatedAt: boundary.generatedAt,
          hookRefs: hookRefs.refs,
          policyRefs: policyRefs.refs,
          postEstimate: postEstimate.estimate,
          preEstimate: preEstimate.estimate,
          preservedAdapterRefs: preservedAdapterRefs.refs,
          preservedPlanRefs: preservedPlanRefs.refs,
          preservedRecentMessageRefs: preservedRecentMessageRefs.refs,
          preservedTaskRefs: preservedTaskRefs.refs,
          preservedToolPairs: preservedToolPairs.flatMap(result =>
            result.pair === null ? [] : [result.pair]
          ),
          publicMessage: publicMessage.message,
          restoredAdapterRefs: restoredAdapterRefs.refs,
          restoredFileRefs: restoredFileRefs.refs,
          restoredPlanRefs: restoredPlanRefs.refs,
          restoredSkillRefs: restoredSkillRefs.refs,
          restoredTaskRefs: restoredTaskRefs.refs,
          retryRefs: retryRefs.refs,
          state: boundary.state,
          strategy: boundary.strategy,
          summarySourceRefs: summarySourceRefs.refs,
          trigger: boundary.trigger,
        },
        omittedUnsafeRefCount,
      }
}

const unmatchedToolPairBlockers = (
  workOrderRef: string,
  boundary: ForgeCompactionBoundaryItem,
): ReadonlyArray<string> =>
  boundary.preservedToolPairs
    .filter(pair => pair.resultRef === null && pair.summaryRef === null)
    .map(pair =>
      blockerRef(workOrderRef, `unmatched-tool-pair:${pair.requestRef}`)
    )

const stateConsistencyBlockers = (
  workOrderRef: string,
  boundary: ForgeCompactionBoundaryItem,
): ReadonlyArray<string> => [
  ...(boundary.state === 'failed' && boundary.postEstimate !== null
    ? [blockerRef(workOrderRef, `failed-compaction-has-post-state:${boundary.boundaryRef}`)]
    : []),
  ...(boundary.state === 'cancelled' && boundary.postEstimate !== null
    ? [
        blockerRef(
          workOrderRef,
          `cancelled-compaction-has-post-state:${boundary.boundaryRef}`,
        ),
      ]
    : []),
  ...(boundary.trigger === 'automatic' &&
  boundary.state === 'failed' &&
  boundary.automaticFailureCount >= 2
    ? [
        blockerRef(
          workOrderRef,
          `automatic-compaction-circuit-breaker:${boundary.boundaryRef}`,
        ),
      ]
    : []),
]

const statusForBoundaries = (
  boundaries: ReadonlyArray<ForgeCompactionBoundaryItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeCompactionSummaryStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (boundaries.length === 0) {
    return 'empty'
  }

  if (boundaries.some(boundary => boundary.state === 'failed')) {
    return 'failed'
  }

  if (boundaries.some(boundary => boundary.state === 'cancelled')) {
    return 'cancelled'
  }

  if (boundaries.some(boundary => boundary.state === 'pending')) {
    return 'pending'
  }

  return boundaries.some(boundary => boundary.state === 'compacted')
    ? 'compacted'
    : 'empty'
}

export const projectForgeCompactionSummary = (
  input: ForgeCompactionSummaryInput,
): ForgeCompactionSummaryView => {
  const compactionRef = safeOptionalRef(
    input.compactionRef ?? fallbackCompactionRef(input.workOrderRef),
  )
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedBoundaries = (input.boundaries ?? []).map(normalizeBoundary)
  const boundaries = normalizedBoundaries
    .flatMap(result => (result.boundary === null ? [] : [result.boundary]))
    .sort((left, right) => left.generatedAt.localeCompare(right.generatedAt))
  const omittedUnsafeRefCount =
    compactionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedBoundaries.reduce(
      (sum, result) => sum + result.omittedUnsafeRefCount,
      0,
    )
  const invariantBlockers = boundaries.flatMap(boundary => [
    ...boundary.blockerRefs,
    ...unmatchedToolPairBlockers(input.workOrderRef, boundary),
    ...stateConsistencyBlockers(input.workOrderRef, boundary),
  ])
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...invariantBlockers,
      ...(compactionRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-compaction-ref')]
        : []),
      ...(input.boundaries !== undefined && input.boundaries.length > 0
        ? []
        : input.compactionRef === undefined && input.blockerRefs === undefined
          ? []
          : [blockerRef(input.workOrderRef, 'missing-compaction-boundary-evidence')]),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-compaction-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    boundaries,
    compactionRef: compactionRef.ref ?? `unsafe-compaction.${input.workOrderRef}`,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    status: statusForBoundaries(boundaries, blockerRefs),
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeCompactionSummaryInput = (
  work: AutopilotWorkProjection,
): ForgeCompactionSummaryInput => {
  const source: AutopilotWorkCompaction | undefined = work.compaction

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.boundaries === undefined ? {} : { boundaries: source.boundaries }),
    ...(source.compactionRef === undefined
      ? { compactionRef: fallbackCompactionRef(work.workOrderRef) }
      : { compactionRef: source.compactionRef }),
  }
}
