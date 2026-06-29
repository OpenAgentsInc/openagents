import {
  type ForgeRetrievalCandidateInput,
  type ForgeRetrievalFreshness,
  type ForgeRetrievalPlanInput,
  type ForgeRetrievalPlanView,
  type ForgeRetrievalSkipReason,
  type ForgeRetrievalSkippedCandidateInput,
  isSafeForgeRetrievalRef,
  projectForgeRetrievalPlan,
} from './retrieval-plan'

export type ForgeExactRetrievalSourceKind =
  | 'documentation'
  | 'file'
  | 'repository'

export type ForgeExactRetrievalFixtureSourceKind =
  | ForgeExactRetrievalSourceKind
  | 'unsupported'

export type ForgeExactRetrievalFixtureInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  candidateRef: string
  exactRefs?: ReadonlyArray<string>
  freshness?: ForgeRetrievalFreshness
  provenanceRefs?: ReadonlyArray<string>
  skipReason?: ForgeRetrievalSkipReason | null
  sourceKind: ForgeExactRetrievalFixtureSourceKind
  sourceRef?: string | null
}>

export type ForgeExactRetrievalAdapterInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  fixtures?: ReadonlyArray<ForgeExactRetrievalFixtureInput>
  freshness?: ForgeRetrievalFreshness
  generatedAt: string
  minimumScore?: number
  planRef: string
  queryRefs?: ReadonlyArray<string>
  requestRef: string
  sourceRefs?: ReadonlyArray<string>
}>

type ScoredFixture = Readonly<{
  candidate: ForgeRetrievalCandidateInput
  candidateRef: string
  score: number
  sourceKind: ForgeExactRetrievalFixtureSourceKind
}>

const SOURCE_KIND_PRIORITY: Readonly<Record<ForgeExactRetrievalSourceKind, number>> = {
  file: 0,
  documentation: 1,
  repository: 2,
}

const SOURCE_KIND_REFS: Readonly<Record<ForgeExactRetrievalSourceKind, string>> = {
  documentation: 'retrieval-source-kind.documentation',
  file: 'retrieval-source-kind.file',
  repository: 'retrieval-source-kind.repository',
}

const DEFAULT_MINIMUM_SCORE = 0.5

const unique = <T>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  Array.from(new Set(values))

const sourceKindPriority = (
  sourceKind: ForgeExactRetrievalFixtureSourceKind,
): number =>
  sourceKind === 'unsupported'
    ? Number.MAX_SAFE_INTEGER
    : SOURCE_KIND_PRIORITY[sourceKind]

const safeQuerySet = (queryRefs: ReadonlyArray<string> | undefined): Set<string> =>
  new Set((queryRefs ?? []).filter(isSafeForgeRetrievalRef))

const scoreFixture = (
  queryRefs: ReadonlySet<string>,
  fixture: ForgeExactRetrievalFixtureInput,
): number => {
  const sourceRef =
    fixture.sourceRef === null || fixture.sourceRef === undefined
      ? []
      : [fixture.sourceRef]
  const candidateRefs = [fixture.candidateRef]
  const exactRefs = fixture.exactRefs ?? []
  const provenanceRefs = fixture.provenanceRefs ?? []
  const scoreByRef = [
    ...candidateRefs.map(ref => ({
      ref,
      score: 1,
    })),
    ...sourceRef.map(ref => ({
      ref,
      score: 0.95,
    })),
    ...exactRefs.map(ref => ({
      ref,
      score: 0.85,
    })),
    ...provenanceRefs.map(ref => ({
      ref,
      score: 0.6,
    })),
  ]

  return scoreByRef.reduce(
    (bestScore, scoredRef) =>
      isSafeForgeRetrievalRef(scoredRef.ref) && queryRefs.has(scoredRef.ref)
        ? Math.max(bestScore, scoredRef.score)
        : bestScore,
    0,
  )
}

const candidateProvenanceRefs = (
  fixture: ForgeExactRetrievalFixtureInput,
): ReadonlyArray<string> =>
  fixture.sourceKind === 'unsupported'
    ? fixture.provenanceRefs ?? []
    : unique([SOURCE_KIND_REFS[fixture.sourceKind], ...(fixture.provenanceRefs ?? [])])

const skippedCandidate = (
  fixture: ForgeExactRetrievalFixtureInput,
  reason: ForgeRetrievalSkipReason,
): ForgeRetrievalSkippedCandidateInput => ({
  blockerRefs: fixture.blockerRefs ?? [],
  candidateRef: fixture.candidateRef,
  reason,
  sourceRef: fixture.sourceRef ?? null,
})

const skipReasonForFixture = (
  fixture: ForgeExactRetrievalFixtureInput,
  score: number,
  minimumScore: number,
  seenCandidateRefs: ReadonlySet<string>,
): ForgeRetrievalSkipReason | null => {
  if (fixture.skipReason !== null && fixture.skipReason !== undefined) {
    return fixture.skipReason
  }

  if (fixture.sourceKind === 'unsupported') {
    return 'unsupported_mode'
  }

  if (
    fixture.sourceRef === null ||
    fixture.sourceRef === undefined ||
    !isSafeForgeRetrievalRef(fixture.sourceRef)
  ) {
    return 'missing_source'
  }

  if (seenCandidateRefs.has(fixture.candidateRef)) {
    return 'duplicate'
  }

  return score < minimumScore ? 'low_score' : null
}

const selectedSort = (left: ScoredFixture, right: ScoredFixture): number =>
  right.score - left.score ||
  sourceKindPriority(left.sourceKind) - sourceKindPriority(right.sourceKind) ||
  left.candidateRef.localeCompare(right.candidateRef)

export const buildForgeExactRetrievalPlanInput = (
  input: ForgeExactRetrievalAdapterInput,
): ForgeRetrievalPlanInput => {
  const queryRefs = safeQuerySet(input.queryRefs)
  const minimumScore = input.minimumScore ?? DEFAULT_MINIMUM_SCORE
  const selected: ScoredFixture[] = []
  const skippedCandidates: ForgeRetrievalSkippedCandidateInput[] = []
  const seenCandidateRefs = new Set<string>()

  for (const fixture of input.fixtures ?? []) {
    const score = scoreFixture(queryRefs, fixture)
    const skipReason = skipReasonForFixture(
      fixture,
      score,
      minimumScore,
      seenCandidateRefs,
    )

    if (skipReason !== null) {
      skippedCandidates.push(skippedCandidate(fixture, skipReason))
      continue
    }

    selected.push({
      candidate: {
        blockerRefs: fixture.blockerRefs ?? [],
        candidateRef: fixture.candidateRef,
        freshness: fixture.freshness ?? input.freshness ?? 'unknown',
        mode: 'exact',
        provenanceRefs: candidateProvenanceRefs(fixture),
        score,
        sourceRef: fixture.sourceRef ?? null,
      },
      candidateRef: fixture.candidateRef,
      score,
      sourceKind: fixture.sourceKind,
    })
    seenCandidateRefs.add(fixture.candidateRef)
  }

  const candidates = selected
    .sort(selectedSort)
    .map<ForgeRetrievalCandidateInput>((fixture, index) => ({
      ...fixture.candidate,
      rank: index + 1,
    }))

  return {
    candidates,
    generatedAt: input.generatedAt,
    mode: 'exact',
    planRef: input.planRef,
    requestRef: input.requestRef,
    skippedCandidates,
    ...(input.blockerRefs === undefined ? {} : { blockerRefs: input.blockerRefs }),
    ...(input.freshness === undefined ? {} : { freshness: input.freshness }),
    ...(input.queryRefs === undefined ? {} : { queryRefs: input.queryRefs }),
    ...(input.sourceRefs === undefined ? {} : { sourceRefs: input.sourceRefs }),
  }
}

export const projectForgeExactRetrievalPlan = (
  input: ForgeExactRetrievalAdapterInput,
): ForgeRetrievalPlanView =>
  projectForgeRetrievalPlan(buildForgeExactRetrievalPlanInput(input))
