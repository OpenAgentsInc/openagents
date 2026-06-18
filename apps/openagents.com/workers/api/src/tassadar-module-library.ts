import { Schema as S } from 'effect'

import type { TassadarCompiledWeightModuleListing } from '@openagentsinc/tassadar-executor/linked-dense-module'
import type { ArtanisWorkDirectionRequest } from './artanis-work-directions'
import type { MarketplaceMarginMemoryProjection } from './marketplace-margin-memory'

export type TassadarDemandPriceSignalInput = Readonly<{
  baselineBudgetSats: number
  baselineListingValueCents: number
  capabilityRef: string
  dataTraceValuationRefs?: ReadonlyArray<string> | undefined
  demandCount: number
  directionRef: string
  marginRankingScoreBps: number
  sourceRefs: ReadonlyArray<string>
  usageCount: number
}>

export type TassadarDemandPriceSignal = Readonly<{
  baselineBudgetSats: number
  baselineListingValueCents: number
  budgetLiftBps: number
  capabilityRef: string
  dataTraceValuationRefs: ReadonlyArray<string>
  demandCount: number
  demandScoreBps: number
  directionRef: string
  listingValueLiftBps: number
  marginRankingScoreBps: number
  recommendedBudgetSats: number
  recommendedListingValueCents: number
  signalRef: string
  sourceRefs: ReadonlyArray<string>
  usageCount: number
}>

export type TassadarModuleLibraryEntryInput = Readonly<{
  acceptedUseRefs?: ReadonlyArray<string> | undefined
  dedupeKey: string
  demandSignal: TassadarDemandPriceSignal
  entryRef: string
  listing: TassadarCompiledWeightModuleListing
  marginMemory: MarketplaceMarginMemoryProjection
  usageCount: number
}>

export type TassadarRankedModuleLibraryEntry = Readonly<{
  acceptedUseRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  canonical: boolean
  collapsedDuplicateEntryRefs: ReadonlyArray<string>
  dedupeKey: string
  demandRank: number
  demandScoreBps: number
  demandSignalRefs: ReadonlyArray<string>
  entryRef: string
  libraryScoreBps: number
  linkedModuleDigest: string
  listingRef: string
  marginRankingScoreBps: number
  moduleKind: string
  recommendedBudgetSats: number
  recommendedListingValueCents: number
  usageCount: number
  usageScoreBps: number
  verificationReceiptRefs: ReadonlyArray<string>
}>

export type TassadarDemandRankedModuleLibraryProjection = Readonly<{
  authority: Readonly<{
    demandPriceMutationAuthority: false
    listingMutationAuthority: false
    rankingMutationAuthority: false
    requestBudgetMutationAuthority: false
    settlementMutationAuthority: false
  }>
  caveatRefs: ReadonlyArray<string>
  collapsedDuplicateCount: number
  duplicateGroupCount: number
  entries: ReadonlyArray<TassadarRankedModuleLibraryEntry>
  generatedBy: 'tassadar_module_library_ranker.v1'
  libraryRef: 'library.public.tassadar_compiled_modules.demand_ranked.v1'
}>

export class TassadarModuleLibraryUnsafe extends S.TaggedErrorClass<TassadarModuleLibraryUnsafe>()(
  'TassadarModuleLibraryUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeModuleLibraryPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|dataset\.(private|raw)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private([._-]|$)|provider[_-]?(account|credential|grant|payload|secret|token)|raw([._-]|$)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[._-]?(archive|private|raw)|token|trace[._-]?(raw|full|private|payload)|wallet)/i

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0)
    .sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref =>
    !safeRefPattern.test(ref) || unsafeModuleLibraryPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new TassadarModuleLibraryUnsafe({
      reason: `${label} must be public-safe refs without raw/private, provider, customer, wallet, payment, or credential material.`,
    })
  }

  return normalized
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new TassadarModuleLibraryUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const clampBps = (value: number): number =>
  Math.max(0, Math.min(10_000, Math.round(value)))

const demandScoreBps = (
  demandCount: number,
  usageCount: number,
  marginRankingScoreBps: number,
): number =>
  clampBps(demandCount * 900 + usageCount * 450 + marginRankingScoreBps * 0.25)

const usageScoreBps = (usageCount: number): number =>
  clampBps(usageCount * 900)

const liftedValue = (
  baseline: number,
  liftBps: number,
): number => Math.max(baseline, Math.round((baseline * liftBps) / 10_000))

const shortRefSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9]+/g, '_').slice(0, 80)

export const buildTassadarDemandPriceSignal = (
  input: TassadarDemandPriceSignalInput,
): TassadarDemandPriceSignal => {
  assertNonNegativeInteger('baselineBudgetSats', input.baselineBudgetSats)
  assertNonNegativeInteger(
    'baselineListingValueCents',
    input.baselineListingValueCents,
  )
  assertNonNegativeInteger('demandCount', input.demandCount)
  assertNonNegativeInteger('usageCount', input.usageCount)
  assertNonNegativeInteger('marginRankingScoreBps', input.marginRankingScoreBps)
  const sourceRefs = assertSafeRefs('Tassadar demand source refs', input.sourceRefs)
  const dataTraceValuationRefs = assertSafeRefs(
    'Tassadar demand data-trace valuation refs',
    input.dataTraceValuationRefs ?? [],
  )
  assertSafeRefs('Tassadar demand identity refs', [
    input.capabilityRef,
    input.directionRef,
  ])

  const score = demandScoreBps(
    input.demandCount,
    input.usageCount,
    input.marginRankingScoreBps,
  )
  const budgetLiftBps = 10_000 + Math.min(20_000, Math.round(score * 1.2))
  const listingValueLiftBps = 10_000 + Math.min(30_000, Math.round(score * 1.5))

  return {
    baselineBudgetSats: input.baselineBudgetSats,
    baselineListingValueCents: input.baselineListingValueCents,
    budgetLiftBps,
    capabilityRef: input.capabilityRef,
    dataTraceValuationRefs,
    demandCount: input.demandCount,
    demandScoreBps: score,
    directionRef: input.directionRef,
    listingValueLiftBps,
    marginRankingScoreBps: input.marginRankingScoreBps,
    recommendedBudgetSats: liftedValue(input.baselineBudgetSats, budgetLiftBps),
    recommendedListingValueCents: liftedValue(
      input.baselineListingValueCents,
      listingValueLiftBps,
    ),
    signalRef: `demand_price.public.tassadar.${shortRefSuffix(input.directionRef)}`,
    sourceRefs,
    usageCount: input.usageCount,
  }
}

export const applyTassadarDemandPriceToArtanisRequest = (
  request: ArtanisWorkDirectionRequest,
  signal: TassadarDemandPriceSignal,
): ArtanisWorkDirectionRequest => ({
  ...request,
  budgetSats: Math.max(request.budgetSats, signal.recommendedBudgetSats),
  sourceRefs: uniqueRefs([
    ...request.sourceRefs,
    signal.signalRef,
    signal.directionRef,
    signal.capabilityRef,
  ]),
})

const listingVerified = (listing: TassadarCompiledWeightModuleListing): boolean =>
  listing.replayVerificationCleared &&
  listing.compositionVerificationCleared &&
  listing.linkCompatibilityVerified

const libraryScoreBps = (
  input: Readonly<{
    demandScoreBps: number
    marginRankingScoreBps: number
    usageScoreBps: number
    verified: boolean
  }>,
): number =>
  clampBps(
    input.demandScoreBps * 0.45 +
      input.marginRankingScoreBps * 0.35 +
      input.usageScoreBps * 0.2 +
      (input.verified ? 750 : -1500),
  )

const entryCandidate = (
  input: TassadarModuleLibraryEntryInput,
): Omit<TassadarRankedModuleLibraryEntry, 'demandRank'> => {
  assertSafeRefs('Tassadar module library entry refs', [
    input.entryRef,
    input.dedupeKey,
    input.listing.listingRef,
    input.listing.moduleKind,
    input.demandSignal.signalRef,
    input.marginMemory.marketMemoryRef,
    ...(input.acceptedUseRefs ?? []),
  ])
  assertNonNegativeInteger('usageCount', input.usageCount)
  const verified = listingVerified(input.listing)
  const usageScore = usageScoreBps(input.usageCount)
  const blockerRefs = uniqueRefs([
    ...input.listing.blockerRefs,
    ...(!verified
      ? ['blocker.public.tassadar_module_library.verification_not_canonical']
      : []),
  ])

  return {
    acceptedUseRefs: uniqueRefs(input.acceptedUseRefs ?? []),
    blockerRefs,
    canonical: false,
    collapsedDuplicateEntryRefs: [],
    dedupeKey: input.dedupeKey,
    demandScoreBps: input.demandSignal.demandScoreBps,
    demandSignalRefs: [input.demandSignal.signalRef],
    entryRef: input.entryRef,
    libraryScoreBps: libraryScoreBps({
      demandScoreBps: input.demandSignal.demandScoreBps,
      marginRankingScoreBps: input.marginMemory.rankingScoreBps,
      usageScoreBps: usageScore,
      verified,
    }),
    linkedModuleDigest: input.listing.linkedModuleDigest,
    listingRef: input.listing.listingRef,
    marginRankingScoreBps: input.marginMemory.rankingScoreBps,
    moduleKind: input.listing.moduleKind,
    recommendedBudgetSats: input.demandSignal.recommendedBudgetSats,
    recommendedListingValueCents: input.demandSignal.recommendedListingValueCents,
    usageCount: input.usageCount,
    usageScoreBps: usageScore,
    verificationReceiptRefs: uniqueRefs([
      ...input.listing.replayReceiptRefs,
      ...input.listing.compositionReceiptRefs,
      ...input.listing.linkCompatibilityReceiptRefs,
    ]),
  }
}

const chooseCanonical = (
  candidates: ReadonlyArray<Omit<TassadarRankedModuleLibraryEntry, 'demandRank'>>,
): Omit<TassadarRankedModuleLibraryEntry, 'demandRank'> =>
  [...candidates].sort((left, right) => {
    const verifiedDelta = Number(left.blockerRefs.length === 0) -
      Number(right.blockerRefs.length === 0)
    if (verifiedDelta !== 0) {
      return -verifiedDelta
    }

    if (right.libraryScoreBps !== left.libraryScoreBps) {
      return right.libraryScoreBps - left.libraryScoreBps
    }

    return left.entryRef.localeCompare(right.entryRef)
  })[0]!

export const rankTassadarCompiledModuleLibrary = (
  input: Readonly<{
    entries: ReadonlyArray<TassadarModuleLibraryEntryInput>
  }>,
): TassadarDemandRankedModuleLibraryProjection => {
  const candidates = input.entries.map(entryCandidate)
  const groups = new Map<
    string,
    ReadonlyArray<Omit<TassadarRankedModuleLibraryEntry, 'demandRank'>>
  >()

  for (const candidate of candidates) {
    groups.set(candidate.dedupeKey, [
      ...(groups.get(candidate.dedupeKey) ?? []),
      candidate,
    ])
  }

  const deduped = [...groups.values()].map(group => {
    const canonical = chooseCanonical(group)
    const duplicateRefs = group
      .filter(candidate => candidate.entryRef !== canonical.entryRef)
      .map(candidate => candidate.entryRef)

    return {
      ...canonical,
      canonical: true,
      collapsedDuplicateEntryRefs: duplicateRefs.sort(),
    }
  })
  const ranked = deduped
    .sort((left, right) =>
      right.libraryScoreBps === left.libraryScoreBps
        ? left.entryRef.localeCompare(right.entryRef)
        : right.libraryScoreBps - left.libraryScoreBps
    )
    .map((entry, index) => ({ ...entry, demandRank: index + 1 }))
  const projection: TassadarDemandRankedModuleLibraryProjection = {
    authority: {
      demandPriceMutationAuthority: false,
      listingMutationAuthority: false,
      rankingMutationAuthority: false,
      requestBudgetMutationAuthority: false,
      settlementMutationAuthority: false,
    },
    caveatRefs: [
      'caveat.public.tassadar_module_library.ranking_is_projection_only',
      'caveat.public.tassadar_module_library.demand_price_requires_operator_requester_gate',
      'caveat.public.tassadar_module_library.purchase_is_not_settlement',
    ],
    collapsedDuplicateCount: candidates.length - ranked.length,
    duplicateGroupCount: [...groups.values()].filter(group => group.length > 1)
      .length,
    entries: ranked,
    generatedBy: 'tassadar_module_library_ranker.v1',
    libraryRef: 'library.public.tassadar_compiled_modules.demand_ranked.v1',
  }

  if (tassadarModuleLibraryProjectionHasPrivateMaterial(projection)) {
    throw new TassadarModuleLibraryUnsafe({
      reason: 'Tassadar module library projection contains unsafe material.',
    })
  }

  return projection
}

const projectionStrings = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStrings)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStrings)
  }

  return []
}

export const tassadarModuleLibraryProjectionHasPrivateMaterial = (
  projection: TassadarDemandRankedModuleLibraryProjection,
): boolean =>
  projectionStrings(projection).some(value =>
    unsafeModuleLibraryPattern.test(value)
  )
