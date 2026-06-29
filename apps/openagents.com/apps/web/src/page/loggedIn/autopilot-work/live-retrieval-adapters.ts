import {
  type ForgeRetrievalCandidateInput,
  type ForgeRetrievalFreshness,
  type ForgeRetrievalMode,
  type ForgeRetrievalPlanInput,
  type ForgeRetrievalSkipReason,
  type ForgeRetrievalSkippedCandidateInput,
  isSafeForgeRetrievalRef,
} from './retrieval-plan'

export type ForgeLiveRetrievalSourceKind =
  | 'diagnostic'
  | 'documentation'
  | 'file'
  | 'unsupported'

export type ForgeLiveRetrievalSourceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  candidateRef: string
  exactRefs?: ReadonlyArray<string>
  freshness?: ForgeRetrievalFreshness
  provenanceRefs?: ReadonlyArray<string>
  score?: number | null
  sourceKind: ForgeLiveRetrievalSourceKind
  sourceRef?: string | null
}>

export type ForgeLiveRetrievalAdapterInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  freshness?: ForgeRetrievalFreshness
  generatedAt: string
  minimumScore?: number
  mode?: ForgeRetrievalMode
  planRef: string
  providerEvidenceRefs?: ReadonlyArray<string>
  queryRefs?: ReadonlyArray<string>
  requestRef: string
  sourceRefs?: ReadonlyArray<string>
  sources?: ReadonlyArray<ForgeLiveRetrievalSourceInput>
  workspaceBoundaryRefs?: ReadonlyArray<string>
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type ScoredLiveSource = Readonly<{
  candidate: ForgeRetrievalCandidateInput
  candidateRef: string
  score: number
  sourceKind: ForgeLiveRetrievalSourceKind
}>

const SOURCE_KIND_PRIORITY: Readonly<Record<Exclude<
  ForgeLiveRetrievalSourceKind,
  'unsupported'
>, number>> = {
  file: 0,
  documentation: 1,
  diagnostic: 2,
}

const SOURCE_KIND_REFS: Readonly<Record<Exclude<
  ForgeLiveRetrievalSourceKind,
  'unsupported'
>, string>> = {
  diagnostic: 'retrieval-source-kind.diagnostic',
  documentation: 'retrieval-source-kind.documentation',
  file: 'retrieval-source-kind.file',
}

const DEFAULT_MINIMUM_SCORE = 0.5

const unique = <T>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  Array.from(new Set(values))

const safeRef = (value: string | null | undefined): string | null =>
  value === null || value === undefined || !isSafeForgeRetrievalRef(value)
    ? null
    : value.trim()

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
    refs: unique(sanitized.refs),
  }
}

const blockerRef = (scopeRef: string, suffix: string): string =>
  `forge-live-retrieval-adapter-blocker:${scopeRef}:${suffix}`

const sourceKindPriority = (sourceKind: ForgeLiveRetrievalSourceKind): number =>
  sourceKind === 'unsupported'
    ? Number.MAX_SAFE_INTEGER
    : SOURCE_KIND_PRIORITY[sourceKind]

const boundedModeNeedsProviderEvidence = (mode: ForgeRetrievalMode): boolean =>
  mode === 'semantic' || mode === 'model_selected' || mode === 'hybrid'

const safeScore = (value: number | null | undefined): number | null =>
  value === null || value === undefined || !Number.isFinite(value) || value < 0
    ? null
    : Math.min(1, value)

const scoreSource = (
  queryRefs: ReadonlySet<string>,
  source: ForgeLiveRetrievalSourceInput,
  safeSourceRef: string | null,
  safeExactRefs: ReadonlyArray<string>,
  safeProvenanceRefs: ReadonlyArray<string>,
): number => {
  const explicitScore = safeScore(source.score)

  if (explicitScore !== null) {
    return explicitScore
  }

  const scoreByRef = [
    { ref: safeRef(source.candidateRef), score: 1 },
    { ref: safeSourceRef, score: 0.95 },
    ...safeExactRefs.map(ref => ({ ref, score: 0.85 })),
    ...safeProvenanceRefs.map(ref => ({ ref, score: 0.6 })),
  ]

  return scoreByRef.reduce(
    (bestScore, scoredRef) =>
      scoredRef.ref !== null && queryRefs.has(scoredRef.ref)
        ? Math.max(bestScore, scoredRef.score)
        : bestScore,
    0,
  )
}

const candidateProvenanceRefs = (
  source: ForgeLiveRetrievalSourceInput,
  workspaceBoundaryRefs: ReadonlyArray<string>,
  safeProvenanceRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  source.sourceKind === 'unsupported'
    ? safeProvenanceRefs
    : unique([
        SOURCE_KIND_REFS[source.sourceKind],
        ...workspaceBoundaryRefs,
        ...safeProvenanceRefs,
      ])

const selectedSort = (left: ScoredLiveSource, right: ScoredLiveSource): number =>
  right.score - left.score ||
  sourceKindPriority(left.sourceKind) - sourceKindPriority(right.sourceKind) ||
  left.candidateRef.localeCompare(right.candidateRef)

const safeCandidateRef = (
  source: ForgeLiveRetrievalSourceInput,
  index: number,
): string =>
  safeRef(source.candidateRef) ?? `retrieval-candidate.filtered_private.${index + 1}`

const skippedCandidate = (
  source: ForgeLiveRetrievalSourceInput,
  index: number,
  reason: ForgeRetrievalSkipReason,
  safeSourceRef: string | null,
  blockerRefs: ReadonlyArray<string>,
): ForgeRetrievalSkippedCandidateInput => ({
  blockerRefs,
  candidateRef: safeCandidateRef(source, index),
  reason,
  sourceRef: safeSourceRef,
})

export const buildForgeLiveRetrievalPlanInput = (
  input: ForgeLiveRetrievalAdapterInput,
): ForgeRetrievalPlanInput => {
  const mode = input.mode ?? 'exact'
  const safePlanRef = safeRef(input.planRef) ?? 'unsafe-live-retrieval-plan'
  const queryRefs = safeRefs(input.queryRefs)
  const inputSourceRefs = safeRefs(input.sourceRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const workspaceBoundaryRefs = safeRefs(input.workspaceBoundaryRefs)
  const providerEvidenceRefs = safeRefs(input.providerEvidenceRefs)
  const adapterBlockerRefs = [
    ...inputBlockerRefs.refs,
    ...(workspaceBoundaryRefs.refs.length === 0
      ? [blockerRef(safePlanRef, 'missing-workspace-boundary-ref')]
      : []),
    ...(boundedModeNeedsProviderEvidence(mode) && providerEvidenceRefs.refs.length === 0
      ? [blockerRef(safePlanRef, 'missing-provider-evidence-ref')]
      : []),
    ...(queryRefs.omittedUnsafeRefCount +
      inputSourceRefs.omittedUnsafeRefCount +
      inputBlockerRefs.omittedUnsafeRefCount +
      workspaceBoundaryRefs.omittedUnsafeRefCount +
      providerEvidenceRefs.omittedUnsafeRefCount >
    0
      ? [blockerRef(safePlanRef, 'unsafe-live-adapter-material-omitted')]
      : []),
  ]
  const querySet = new Set(queryRefs.refs)
  const minimumScore = input.minimumScore ?? DEFAULT_MINIMUM_SCORE
  const selected: ScoredLiveSource[] = []
  const skippedCandidates: ForgeRetrievalSkippedCandidateInput[] = []
  const seenCandidateRefs = new Set<string>()

  for (const [index, source] of (input.sources ?? []).entries()) {
    const safeSourceRef = safeRef(source.sourceRef)
    const safeExactRefs = safeRefs(source.exactRefs)
    const safeProvenanceRefs = safeRefs(source.provenanceRefs)
    const safeSourceBlockerRefs = safeRefs(source.blockerRefs)
    const safeCandidate = safeRef(source.candidateRef)
    const sourceUnsafeOmitted =
      safeExactRefs.omittedUnsafeRefCount +
      safeProvenanceRefs.omittedUnsafeRefCount +
      safeSourceBlockerRefs.omittedUnsafeRefCount +
      (safeCandidate === null ? 1 : 0) +
      (source.sourceRef !== null &&
      source.sourceRef !== undefined &&
      safeSourceRef === null
        ? 1
        : 0)
    const unsafeBlockers =
      sourceUnsafeOmitted === 0
        ? []
        : [blockerRef(safePlanRef, 'unsafe-live-source-material-omitted')]
    const sourceBlockers = unique([
      ...safeSourceBlockerRefs.refs,
      ...unsafeBlockers,
    ])

    if (safeCandidate === null) {
      skippedCandidates.push(
        skippedCandidate(source, index, 'filtered_private', safeSourceRef, sourceBlockers),
      )
      continue
    }

    if (source.sourceKind === 'unsupported') {
      skippedCandidates.push(
        skippedCandidate(source, index, 'unsupported_mode', safeSourceRef, sourceBlockers),
      )
      continue
    }

    if (safeSourceRef === null) {
      skippedCandidates.push(
        skippedCandidate(source, index, 'missing_source', null, sourceBlockers),
      )
      continue
    }

    if (source.freshness === 'stale') {
      skippedCandidates.push(
        skippedCandidate(source, index, 'stale', safeSourceRef, sourceBlockers),
      )
      continue
    }

    if (seenCandidateRefs.has(safeCandidate)) {
      skippedCandidates.push(
        skippedCandidate(source, index, 'duplicate', safeSourceRef, sourceBlockers),
      )
      continue
    }

    const score = scoreSource(
      querySet,
      source,
      safeSourceRef,
      safeExactRefs.refs,
      safeProvenanceRefs.refs,
    )

    if (score < minimumScore) {
      skippedCandidates.push(
        skippedCandidate(source, index, 'low_score', safeSourceRef, sourceBlockers),
      )
      continue
    }

    selected.push({
      candidate: {
        blockerRefs: sourceBlockers,
        candidateRef: safeCandidate,
        freshness: source.freshness ?? input.freshness ?? 'unknown',
        mode,
        provenanceRefs: candidateProvenanceRefs(
          source,
          workspaceBoundaryRefs.refs,
          safeProvenanceRefs.refs,
        ),
        score,
        sourceRef: safeSourceRef,
      },
      candidateRef: safeCandidate,
      score,
      sourceKind: source.sourceKind,
    })
    seenCandidateRefs.add(safeCandidate)
  }

  const candidates = selected
    .sort(selectedSort)
    .map<ForgeRetrievalCandidateInput>((source, index) => ({
      ...source.candidate,
      rank: index + 1,
    }))

  return {
    blockerRefs: unique(adapterBlockerRefs),
    candidates,
    generatedAt: input.generatedAt,
    mode,
    planRef: input.planRef,
    queryRefs: queryRefs.refs,
    requestRef: input.requestRef,
    skippedCandidates,
    sourceRefs: unique([
      ...inputSourceRefs.refs,
      ...workspaceBoundaryRefs.refs,
      ...providerEvidenceRefs.refs,
      ...candidates.flatMap(candidate =>
        candidate.sourceRef === null ||
        candidate.sourceRef === undefined ||
        !isSafeForgeRetrievalRef(candidate.sourceRef)
          ? []
          : [candidate.sourceRef],
      ),
    ]),
    ...(input.freshness === undefined ? {} : { freshness: input.freshness }),
  }
}
