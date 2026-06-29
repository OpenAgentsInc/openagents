export type ForgeRetrievalMode =
  | 'exact'
  | 'hybrid'
  | 'model_selected'
  | 'semantic'
  | 'structured'

export type ForgeRetrievalFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeRetrievalPlanStatus = 'blocked' | 'empty' | 'ready' | 'stale'

export type ForgeRetrievalSkipReason =
  | 'duplicate'
  | 'filtered_private'
  | 'low_score'
  | 'missing_source'
  | 'stale'
  | 'unsupported_mode'

export type ForgeRetrievalCandidateInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  candidateRef: string
  freshness?: ForgeRetrievalFreshness
  mode?: ForgeRetrievalMode
  provenanceRefs?: ReadonlyArray<string>
  rank?: number | null
  score?: number | null
  sourceRef?: string | null
}>

export type ForgeRetrievalSkippedCandidateInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  candidateRef: string
  reason: ForgeRetrievalSkipReason
  sourceRef?: string | null
}>

export type ForgeRetrievalPlanInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  candidates?: ReadonlyArray<ForgeRetrievalCandidateInput>
  freshness?: ForgeRetrievalFreshness
  generatedAt: string
  mode: ForgeRetrievalMode
  planRef: string
  queryRefs?: ReadonlyArray<string>
  requestRef: string
  skippedCandidates?: ReadonlyArray<ForgeRetrievalSkippedCandidateInput>
  sourceRefs?: ReadonlyArray<string>
}>

export type ForgeRetrievalCandidate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  candidateRef: string
  freshness: ForgeRetrievalFreshness
  mode: ForgeRetrievalMode
  provenanceRefs: ReadonlyArray<string>
  rank: number | null
  score: number | null
  sourceRef: string | null
}>

export type ForgeRetrievalSkippedCandidate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  candidateRef: string
  reason: ForgeRetrievalSkipReason
  sourceRef: string | null
}>

export type ForgeRetrievalResultSet = Readonly<{
  selectedCandidateRefs: ReadonlyArray<string>
  skippedCandidateRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  totalSelected: number
  totalSkipped: number
}>

export type ForgeRetrievalPlanView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  candidates: ReadonlyArray<ForgeRetrievalCandidate>
  freshness: ForgeRetrievalFreshness
  generatedAt: string
  mode: ForgeRetrievalMode
  omittedUnsafeRefCount: number
  planRef: string
  queryRefs: ReadonlyArray<string>
  requestRef: string
  resultSet: ForgeRetrievalResultSet
  skippedCandidates: ReadonlyArray<ForgeRetrievalSkippedCandidate>
  sourceRefs: ReadonlyArray<string>
  status: ForgeRetrievalPlanStatus
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_RETRIEVAL_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_RETRIEVAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|transcript|test)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?)/i,
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

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_RETRIEVAL_REF_PATTERN.test(trimmed) &&
    !PRIVATE_RETRIEVAL_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

export const isSafeForgeRetrievalRef = (value: string): boolean =>
  safeRef(value) !== null

const safeRefs = (refs: ReadonlyArray<string> | undefined): RefBundle => {
  const sanitized = (refs ?? []).reduce<Readonly<{ omitted: number; refs: string[] }>>(
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

const blockerRef = (scopeRef: string, suffix: string): string =>
  `forge-retrieval-plan-blocker:${scopeRef}:${suffix}`

const safeRank = (value: number | null | undefined): number | null =>
  value === null || value === undefined || !Number.isSafeInteger(value) || value < 0
    ? null
    : value

const safeScore = (value: number | null | undefined): number | null =>
  value === null || value === undefined || !Number.isFinite(value) || value < 0
    ? null
    : Math.min(1, value)

const normalizeCandidate = (
  planMode: ForgeRetrievalMode,
  candidate: ForgeRetrievalCandidateInput,
): Readonly<{ candidate: ForgeRetrievalCandidate | null; omitted: number }> => {
  const candidateRef = safeOptionalRef(candidate.candidateRef)
  const sourceRef = safeOptionalRef(candidate.sourceRef)
  const provenanceRefs = safeRefs(candidate.provenanceRefs)
  const blockerRefs = safeRefs(candidate.blockerRefs)
  const omitted =
    candidateRef.omittedUnsafeRefCount +
    sourceRef.omittedUnsafeRefCount +
    provenanceRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    candidate:
      candidateRef.ref === null
        ? null
        : {
            blockerRefs: blockerRefs.refs,
            candidateRef: candidateRef.ref,
            freshness: candidate.freshness ?? 'unknown',
            mode: candidate.mode ?? planMode,
            provenanceRefs: provenanceRefs.refs,
            rank: safeRank(candidate.rank),
            score: safeScore(candidate.score),
            sourceRef: sourceRef.ref,
          },
    omitted,
  }
}

const normalizeSkippedCandidate = (
  candidate: ForgeRetrievalSkippedCandidateInput,
): Readonly<{ candidate: ForgeRetrievalSkippedCandidate | null; omitted: number }> => {
  const candidateRef = safeOptionalRef(candidate.candidateRef)
  const sourceRef = safeOptionalRef(candidate.sourceRef)
  const blockerRefs = safeRefs(candidate.blockerRefs)
  const omitted =
    candidateRef.omittedUnsafeRefCount +
    sourceRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    candidate:
      candidateRef.ref === null
        ? null
        : {
            blockerRefs: blockerRefs.refs,
            candidateRef: candidateRef.ref,
            reason: candidate.reason,
            sourceRef: sourceRef.ref,
          },
    omitted,
  }
}

const candidateSort = (
  left: ForgeRetrievalCandidate,
  right: ForgeRetrievalCandidate,
): number =>
  (left.rank ?? Number.MAX_SAFE_INTEGER) -
    (right.rank ?? Number.MAX_SAFE_INTEGER) ||
  (right.score ?? -1) - (left.score ?? -1) ||
  left.candidateRef.localeCompare(right.candidateRef)

const skippedSort = (
  left: ForgeRetrievalSkippedCandidate,
  right: ForgeRetrievalSkippedCandidate,
): number =>
  left.reason.localeCompare(right.reason) ||
  left.candidateRef.localeCompare(right.candidateRef)

const planStatus = (input: Readonly<{
  blockerRefs: ReadonlyArray<string>
  candidates: ReadonlyArray<ForgeRetrievalCandidate>
  freshness: ForgeRetrievalFreshness
}>): ForgeRetrievalPlanStatus => {
  if (input.blockerRefs.length > 0) {
    return 'blocked'
  }

  if (input.freshness === 'stale') {
    return 'stale'
  }

  return input.candidates.length === 0 ? 'empty' : 'ready'
}

export const projectForgeRetrievalPlan = (
  input: ForgeRetrievalPlanInput,
): ForgeRetrievalPlanView => {
  const requestRef = safeOptionalRef(input.requestRef)
  const planRef = safeOptionalRef(input.planRef)
  const queryRefs = safeRefs(input.queryRefs)
  const sourceRefs = safeRefs(input.sourceRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedCandidates = (input.candidates ?? []).map(candidate =>
    normalizeCandidate(input.mode, candidate),
  )
  const normalizedSkippedCandidates = (input.skippedCandidates ?? []).map(candidate =>
    normalizeSkippedCandidate(candidate),
  )
  const candidates = normalizedCandidates
    .flatMap(result => (result.candidate === null ? [] : [result.candidate]))
    .sort(candidateSort)
  const skippedCandidates = normalizedSkippedCandidates
    .flatMap(result => (result.candidate === null ? [] : [result.candidate]))
    .sort(skippedSort)
  const candidateSourceRefs = candidates.flatMap(candidate =>
    candidate.sourceRef === null ? [] : [candidate.sourceRef],
  )
  const skippedSourceRefs = skippedCandidates.flatMap(candidate =>
    candidate.sourceRef === null ? [] : [candidate.sourceRef],
  )
  const freshness = input.freshness ?? 'unknown'
  const omittedUnsafeRefCount =
    requestRef.omittedUnsafeRefCount +
    planRef.omittedUnsafeRefCount +
    queryRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedCandidates.reduce((sum, result) => sum + result.omitted, 0) +
    normalizedSkippedCandidates.reduce((sum, result) => sum + result.omitted, 0)
  const safeScopeRef = planRef.ref ?? requestRef.ref ?? 'unsafe-retrieval-plan'
  const derivedBlockers = [
    ...(requestRef.ref === null ? [blockerRef(safeScopeRef, 'missing-request-ref')] : []),
    ...(planRef.ref === null ? [blockerRef(safeScopeRef, 'missing-plan-ref')] : []),
    ...(queryRefs.refs.length === 0
      ? [blockerRef(safeScopeRef, 'missing-query-ref')]
      : []),
    ...(omittedUnsafeRefCount === 0
      ? []
      : [blockerRef(safeScopeRef, 'unsafe-retrieval-material-omitted')]),
  ]
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...candidates.flatMap(candidate => candidate.blockerRefs),
      ...skippedCandidates.flatMap(candidate => candidate.blockerRefs),
      ...derivedBlockers,
    ]),
  )
  const selectedCandidateRefs = candidates.map(candidate => candidate.candidateRef)
  const skippedCandidateRefs = skippedCandidates.map(candidate => candidate.candidateRef)
  const allSourceRefs = Array.from(
    new Set([...sourceRefs.refs, ...candidateSourceRefs, ...skippedSourceRefs]),
  )

  return {
    blockerRefs,
    candidates,
    freshness,
    generatedAt: input.generatedAt,
    mode: input.mode,
    omittedUnsafeRefCount,
    planRef: planRef.ref ?? 'unsafe-plan-ref-omitted',
    queryRefs: queryRefs.refs,
    requestRef: requestRef.ref ?? 'unsafe-request-ref-omitted',
    resultSet: {
      selectedCandidateRefs,
      skippedCandidateRefs,
      sourceRefs: allSourceRefs,
      totalSelected: selectedCandidateRefs.length,
      totalSkipped: skippedCandidateRefs.length,
    },
    skippedCandidates,
    sourceRefs: allSourceRefs,
    status: planStatus({ blockerRefs, candidates, freshness }),
  }
}
