import { describe, expect, test } from 'vitest'

import {
  projectTassadarCompiledWeightModuleListing,
  tassadarLinkedDenseProgramFixture,
  type TassadarCompiledWeightModuleListing,
} from '@openagentsinc/tassadar-executor/linked-dense-module'
import {
  exampleMarketplaceMarginMemory,
  projectMarketplaceMarginMemory,
} from './marketplace-margin-memory'
import {
  applyTassadarDemandPriceToArtanisRequest,
  buildTassadarDemandPriceSignal,
  rankTassadarCompiledModuleLibrary,
  TassadarModuleLibraryUnsafe,
  tassadarModuleLibraryProjectionHasPrivateMaterial,
} from './tassadar-module-library'
import type { ArtanisWorkDirectionRequest } from './artanis-work-directions'

const nowIso = '2026-06-18T12:00:00.000Z'

const marginMemory = () =>
  projectMarketplaceMarginMemory(
    {
      ...exampleMarketplaceMarginMemory(),
      capabilityRef: 'capability.openagents.tassadar.linked_dense_module',
      evidenceRefs: [
        'evidence.public.tassadar.linked_dense_module.marketplace_memory',
      ],
      id: 'marketplace_margin_memory.tassadar_linked_dense_module',
      marketMemoryRef: 'market_memory.tassadar_linked_dense_module',
      moduleVersionRefs: ['module_version.tassadar.linked_dense.v1'],
      sourceRefs: ['source.public.tassadar.linked_dense_fixture'],
      workClassRefs: ['work_class.tassadar.compiled_module_library'],
    },
    'public',
    nowIso,
  )

const demandSignal = (
  overrides: Partial<Parameters<typeof buildTassadarDemandPriceSignal>[0]> = {},
) =>
  buildTassadarDemandPriceSignal({
    baselineBudgetSats: 2_500,
    baselineListingValueCents: 12_500,
    capabilityRef: 'capability.openagents.tassadar.linked_dense_module',
    dataTraceValuationRefs: [
      'valuation.public.data_market.tassadar_linked_dense_module',
    ],
    demandCount: 7,
    directionRef: 'direction.public.tassadar.linked_dense_module_library',
    marginRankingScoreBps: marginMemory().rankingScoreBps,
    sourceRefs: ['source.public.tassadar.linked_dense_demand'],
    usageCount: 3,
    ...overrides,
  })

const artanisDatasetRequest = (): ArtanisWorkDirectionRequest => ({
  budgetSats: 2_500,
  corpusRef: 'corpus.tassadar_trace.linked_dense_library_v1',
  deadlineRef: 'deadline.public.artanis.work_direction.soon',
  directionKind: 'dataset_curation',
  objectiveRef: 'objective.public.artanis.dataset_curate.linked_dense_library',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  sourceRefs: ['source.public.artanis.work_direction.e2'],
  title: 'Curate linked dense module library demand evidence',
  verificationClass: 'v3_data_correctness',
  verificationCommandRef:
    'command.public.openagents.data_contribution.v3_correctness',
})

describe('Tassadar module library demand ranking', () => {
  test('raises budgets and listing value for a wanted direction', () => {
    const signal = demandSignal()
    const boosted = applyTassadarDemandPriceToArtanisRequest(
      artanisDatasetRequest(),
      signal,
    )

    expect(signal.recommendedBudgetSats).toBeGreaterThan(
      signal.baselineBudgetSats,
    )
    expect(signal.recommendedListingValueCents).toBeGreaterThan(
      signal.baselineListingValueCents,
    )
    expect(boosted.budgetSats).toBe(signal.recommendedBudgetSats)
    expect(boosted.sourceRefs).toEqual(
      expect.arrayContaining([
        signal.signalRef,
        'direction.public.tassadar.linked_dense_module_library',
      ]),
    )
  })

  test('dedupes near-duplicate authored modules to the verified canonical entry and ranks by demand/value/usage', async () => {
    const listing = await projectTassadarCompiledWeightModuleListing({
      fixture: tassadarLinkedDenseProgramFixture,
    })
    const duplicateListing: TassadarCompiledWeightModuleListing = {
      ...listing,
      blockerRefs: [
        'blocker.public.tassadar_compiled_module.replay_verification_missing',
      ],
      compositionReceiptRefs: [],
      compositionVerificationCleared: false,
      linkCompatibilityReceiptRefs: [],
      linkCompatibilityVerified: false,
      replayReceiptRefs: [],
      replayVerificationCleared: false,
      state: 'blocked',
    }
    const projection = rankTassadarCompiledModuleLibrary({
      entries: [
        {
          acceptedUseRefs: ['usage.public.tassadar.linked_dense.demo_001'],
          dedupeKey: 'spec.public.tassadar.linked_dense.w3_100m',
          demandSignal: demandSignal({ demandCount: 10, usageCount: 4 }),
          entryRef: 'entry.public.tassadar.linked_dense.canonical',
          listing,
          marginMemory: marginMemory(),
          usageCount: 4,
        },
        {
          dedupeKey: 'spec.public.tassadar.linked_dense.w3_100m',
          demandSignal: demandSignal({ demandCount: 9, usageCount: 3 }),
          entryRef: 'entry.public.tassadar.linked_dense.near_duplicate',
          listing: duplicateListing,
          marginMemory: marginMemory(),
          usageCount: 3,
        },
        {
          dedupeKey: 'spec.public.tassadar.memory_roundtrip.w1',
          demandSignal: demandSignal({
            demandCount: 1,
            directionRef:
              'direction.public.tassadar.memory_roundtrip_module_library',
            usageCount: 1,
          }),
          entryRef: 'entry.public.tassadar.memory_roundtrip.secondary',
          listing,
          marginMemory: marginMemory(),
          usageCount: 1,
        },
      ],
    })

    expect(projection.entries).toHaveLength(2)
    expect(projection.collapsedDuplicateCount).toBe(1)
    expect(projection.duplicateGroupCount).toBe(1)
    expect(projection.entries[0]).toMatchObject({
      canonical: true,
      collapsedDuplicateEntryRefs: [
        'entry.public.tassadar.linked_dense.near_duplicate',
      ],
      demandRank: 1,
      dedupeKey: 'spec.public.tassadar.linked_dense.w3_100m',
      entryRef: 'entry.public.tassadar.linked_dense.canonical',
    })
    expect(projection.entries[0]!.libraryScoreBps).toBeGreaterThan(
      projection.entries[1]!.libraryScoreBps,
    )
    expect(tassadarModuleLibraryProjectionHasPrivateMaterial(projection))
      .toBe(false)
    expect(projection.authority).toEqual({
      demandPriceMutationAuthority: false,
      listingMutationAuthority: false,
      rankingMutationAuthority: false,
      requestBudgetMutationAuthority: false,
      settlementMutationAuthority: false,
    })
  })

  test('rejects unsafe demand and usage refs before projecting the library', async () => {
    const listing = await projectTassadarCompiledWeightModuleListing({
      fixture: tassadarLinkedDenseProgramFixture,
    })

    expect(() =>
      buildTassadarDemandPriceSignal({
        baselineBudgetSats: 2_500,
        baselineListingValueCents: 12_500,
        capabilityRef: 'capability.openagents.tassadar.linked_dense_module',
        demandCount: 1,
        directionRef: 'direction.public.tassadar.linked_dense_module_library',
        marginRankingScoreBps: marginMemory().rankingScoreBps,
        sourceRefs: ['source.raw_private_trace'],
        usageCount: 1,
      }),
    ).toThrow(TassadarModuleLibraryUnsafe)

    expect(() =>
      rankTassadarCompiledModuleLibrary({
        entries: [
          {
            acceptedUseRefs: ['usage.raw_private_trace'],
            dedupeKey: 'spec.public.tassadar.linked_dense.w3_100m',
            demandSignal: demandSignal(),
            entryRef: 'entry.public.tassadar.linked_dense.canonical',
            listing,
            marginMemory: marginMemory(),
            usageCount: 1,
          },
        ],
      }),
    ).toThrow(TassadarModuleLibraryUnsafe)
  })
})
