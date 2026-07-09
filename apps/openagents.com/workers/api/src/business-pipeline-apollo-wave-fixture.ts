/**
 * OB-2 (#8559) fixture tier for Apollo segment waves.
 *
 * Builds ≥100 synthetic public-safe prospects so operators and tests can prove:
 * - volume ingest into `business_pipeline_rows`
 * - suppression-listed subjects never enter the queue
 * - replaying a wave is idempotent (pipelineRef + subjectRef)
 * - a later wave with new pipelineRefs for the same subjectRefs is also deduped
 *
 * No names, domains, emails, raw Apollo payloads, or private contact data.
 * Apollo remains a mirror; the pipeline is the system of record (BF-9.2).
 */

import { Schema as S } from 'effect'

export const OB2_APOLLO_WAVE_FIXTURE_MIN_COUNT = 100 as const
export const OB2_APOLLO_WAVE_FIXTURE_MAX_COUNT = 500 as const

export class Ob2ApolloWaveFixtureValidationError extends S.TaggedErrorClass<Ob2ApolloWaveFixtureValidationError>()(
  'Ob2ApolloWaveFixtureValidationError',
  { message: S.String },
) {}

export type Ob2ApolloWaveSegmentKey =
  | 'agencies_seo'
  | 'legal_small_firm'
  | 'home_services'
  | 'own_your_ai'

export type Ob2ApolloWaveSegmentDefinition = Readonly<{
  /** Opaque segment config ref used on the ingest payload. */
  segmentRef: string
  /** LG-6 source attribution token (must pass decodeBusinessSourceRef). */
  sourceRef: string
  /** Public-safe vertical descriptor. */
  vertical: string
  /** pipelineRef prefix, e.g. `biz-pipe-agency`. */
  pipelinePrefix: string
  /** subjectRef prefix, e.g. `prospect.agency`. */
  subjectPrefix: string
  /** Default quoted band for audit-first intake. */
  quotedBand: Readonly<{
    label: string
    maxUsdCents: number
    minUsdCents: number
  }>
}>

/**
 * Written segment set from the Apollo outbound plan + #8559 scope.
 * Keys are stable operator handles; values are public-safe only.
 */
export const OB2_APOLLO_WAVE_SEGMENTS: Readonly<
  Record<Ob2ApolloWaveSegmentKey, Ob2ApolloWaveSegmentDefinition>
> = {
  agencies_seo: {
    pipelinePrefix: 'biz-pipe-agency',
    quotedBand: {
      label: 'audit first',
      maxUsdCents: 500_000,
      minUsdCents: 150_000,
    },
    segmentRef: 'segment.apollo.agencies_seo',
    sourceRef: 'apollo_agent_readiness_agency',
    subjectPrefix: 'prospect.agency',
    vertical: 'agency',
  },
  home_services: {
    pipelinePrefix: 'biz-pipe-homesvc',
    quotedBand: {
      label: 'audit first',
      maxUsdCents: 500_000,
      minUsdCents: 150_000,
    },
    segmentRef: 'segment.apollo.home_services',
    sourceRef: 'apollo_agent_readiness_marketplace',
    subjectPrefix: 'prospect.homesvc',
    vertical: 'home services',
  },
  legal_small_firm: {
    pipelinePrefix: 'biz-pipe-legal',
    quotedBand: {
      label: 'audit first',
      maxUsdCents: 500_000,
      minUsdCents: 150_000,
    },
    segmentRef: 'segment.apollo.legal_small_firm',
    sourceRef: 'apollo_model_custody',
    subjectPrefix: 'prospect.legal',
    vertical: 'regulated legal',
  },
  own_your_ai: {
    pipelinePrefix: 'biz-pipe-oya',
    quotedBand: {
      label: 'model custody',
      maxUsdCents: 2_500_000,
      minUsdCents: 500_000,
    },
    segmentRef: 'segment.apollo.own_your_ai',
    sourceRef: 'own_your_ai',
    subjectPrefix: 'prospect.oya',
    vertical: 'model custody',
  },
}

/** API body prospect shape for POST /api/operator/business/pipeline/apollo-waves */
export type Ob2ApolloWaveFixtureProspect = Readonly<{
  pipelineRef: string
  quotedBandLabel: string
  quotedMaxUsdCents: number
  quotedMinUsdCents: number
  subjectRef: string
  vertical: string
}>

export type Ob2ApolloWaveFixture = Readonly<{
  count: number
  prospects: ReadonlyArray<Ob2ApolloWaveFixtureProspect>
  segmentKey: Ob2ApolloWaveSegmentKey
  segmentRef: string
  sourceRef: string
  /**
   * Subject refs that the fixture expects operators/tests to suppress
   * before ingest when proving the suppression gate.
   */
  suppressedSubjectRefs: ReadonlyArray<string>
  vertical: string
  waveRef: string
}>

export type BuildOb2ApolloWaveFixtureInput = Readonly<{
  /** Prospect count. Defaults to 100 (exit-gate floor). Max 500 (route cap). */
  count?: number
  /**
   * When true, pipelineRefs get a `-${waveId}` suffix so a second wave can
   * reuse the same subjectRefs under fresh pipelineRefs (subject-level dedupe).
   */
  distinctPipelineRefs?: boolean
  segmentKey: Ob2ApolloWaveSegmentKey
  /**
   * 1-based indexes of prospects to mark as suppressed for the fixture
   * receipt. Defaults to [50] when count >= 50 so a mid-wave suppression
   * is always present for ≥100 waves.
   */
  suppressIndexes?: ReadonlyArray<number>
  /** Wave id fragment, e.g. `20260709a`. Defaults to `fixture`. */
  waveId?: string
}>

const padIndex = (index: number): string => String(index).padStart(3, '0')

const assertCount = (count: number): number => {
  if (
    !Number.isInteger(count) ||
    count < 1 ||
    count > OB2_APOLLO_WAVE_FIXTURE_MAX_COUNT
  ) {
    throw new Ob2ApolloWaveFixtureValidationError({
      message: `apollo wave fixture count must be an integer 1-${OB2_APOLLO_WAVE_FIXTURE_MAX_COUNT}`,
    })
  }
  return count
}

const defaultSuppressIndexes = (count: number): ReadonlyArray<number> =>
  count >= 50 ? [50] : count >= 1 ? [1] : []

/**
 * Build a public-safe synthetic Apollo wave payload at volume.
 * Does not call Apollo or the network.
 */
export const buildOb2ApolloWaveFixture = (
  input: BuildOb2ApolloWaveFixtureInput,
): Ob2ApolloWaveFixture => {
  const segment = OB2_APOLLO_WAVE_SEGMENTS[input.segmentKey]
  const count = assertCount(input.count ?? OB2_APOLLO_WAVE_FIXTURE_MIN_COUNT)
  const waveId = (input.waveId ?? 'fixture').trim() || 'fixture'
  const suppressIndexes = [
    ...new Set(
      (input.suppressIndexes ?? defaultSuppressIndexes(count)).filter(
        index => Number.isInteger(index) && index >= 1 && index <= count,
      ),
    ),
  ].sort((a, b) => a - b)

  const pipelineSuffix =
    input.distinctPipelineRefs === true ? `-${waveId}` : ''

  const prospects: Array<Ob2ApolloWaveFixtureProspect> = Array.from(
    { length: count },
    (_, zeroBased) => {
      const n = padIndex(zeroBased + 1)
      return {
        pipelineRef: `${segment.pipelinePrefix}-${n}${pipelineSuffix}`,
        quotedBandLabel: segment.quotedBand.label,
        quotedMaxUsdCents: segment.quotedBand.maxUsdCents,
        quotedMinUsdCents: segment.quotedBand.minUsdCents,
        subjectRef: `${segment.subjectPrefix}.${n}`,
        vertical: segment.vertical,
      }
    },
  )

  const suppressedSubjectRefs = suppressIndexes.map(
    index => `${segment.subjectPrefix}.${padIndex(index)}`,
  )

  return {
    count,
    prospects,
    segmentKey: input.segmentKey,
    segmentRef: segment.segmentRef,
    sourceRef: segment.sourceRef,
    suppressedSubjectRefs,
    vertical: segment.vertical,
    waveRef: `apollo.wave.${input.segmentKey}.${waveId}`,
  }
}

/** Operator POST body for the Apollo wave ingest route. */
export const apolloWaveIngestBodyFromFixture = (
  fixture: Ob2ApolloWaveFixture,
): Readonly<{
  prospects: ReadonlyArray<Ob2ApolloWaveFixtureProspect>
  segmentRef: string
  sourceRef: string
  waveRef: string
}> => ({
  prospects: fixture.prospects,
  segmentRef: fixture.segmentRef,
  sourceRef: fixture.sourceRef,
  waveRef: fixture.waveRef,
})

/**
 * Expected first-pass ingest counts when suppressions for
 * `fixture.suppressedSubjectRefs` are already present in store.
 */
export const expectedFirstPassApolloWaveCounts = (
  fixture: Ob2ApolloWaveFixture,
): Readonly<{
  acceptedCount: number
  duplicateCount: number
  suppressedCount: number
}> => {
  const suppressedCount = fixture.suppressedSubjectRefs.length
  return {
    acceptedCount: fixture.count - suppressedCount,
    duplicateCount: 0,
    suppressedCount,
  }
}

/**
 * Expected replay / re-wave counts after a successful first pass with the
 * same subject set already in the pipeline (idempotent re-wave).
 */
export const expectedReplayApolloWaveCounts = (
  fixture: Ob2ApolloWaveFixture,
): Readonly<{
  acceptedCount: number
  duplicateCount: number
  suppressedCount: number
}> => {
  const suppressedCount = fixture.suppressedSubjectRefs.length
  return {
    acceptedCount: 0,
    duplicateCount: fixture.count - suppressedCount,
    suppressedCount,
  }
}

/**
 * Second wave with distinct pipelineRefs for the same subjects (subject-level
 * dedupe). Suppressions still apply; none of the subjects should insert.
 */
export const expectedSubjectDedupeApolloWaveCounts = (
  fixture: Ob2ApolloWaveFixture,
): Readonly<{
  acceptedCount: number
  duplicateCount: number
  suppressedCount: number
}> => expectedReplayApolloWaveCounts(fixture)

/** Two default live-wave segment keys for the #8559 exit gate. */
export const OB2_LIVE_WAVE_SEGMENT_PAIR: ReadonlyArray<Ob2ApolloWaveSegmentKey> =
  ['agencies_seo', 'legal_small_firm']
