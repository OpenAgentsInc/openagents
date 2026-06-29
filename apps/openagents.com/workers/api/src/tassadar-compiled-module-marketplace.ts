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
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  buildTassadarDemandPriceSignal,
  rankTassadarCompiledModuleLibrary,
  type TassadarDemandPriceSignal,
  type TassadarDemandRankedModuleLibraryProjection,
} from './tassadar-module-library'

export const TASSADAR_COMPILED_MODULE_MARKETPLACE_ROUTE =
  '/api/public/tassadar/compiled-module-marketplace'
export const TASSADAR_COMPILED_MODULE_MARKETPLACE_SCHEMA_VERSION =
  'tassadar_compiled_module_marketplace.v1'

export type PublicTassadarCompiledModuleMarketplaceEnvelope = Readonly<{
  schemaVersion: typeof TASSADAR_COMPILED_MODULE_MARKETPLACE_SCHEMA_VERSION
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  marketRef: 'market.public.tassadar_compiled_weight_modules.v1'
  claimBoundary: string
  listings: ReadonlyArray<TassadarCompiledWeightModuleListing>
  demandSignals: ReadonlyArray<TassadarDemandPriceSignal>
  moduleLibrary: TassadarDemandRankedModuleLibraryProjection
  authority: Readonly<{
    demandPriceMutationAuthority: false
    listingMutationAuthority: false
    purchaseMutationAuthority: false
    rankingMutationAuthority: false
    requestBudgetMutationAuthority: false
    settlementMutationAuthority: false
    realSettlementEnabled: false
  }>
  caveatRefs: ReadonlyArray<string>
}>

export const buildPublicTassadarCompiledModuleMarketplaceEnvelope = async (
  generatedAt: string = currentIsoTimestamp(),
): Promise<PublicTassadarCompiledModuleMarketplaceEnvelope> => {
  const listing = await projectTassadarCompiledWeightModuleListing({
    fixture: tassadarLinkedDenseProgramFixture,
  })
  const marginMemory = projectMarketplaceMarginMemory(
    {
      ...exampleMarketplaceMarginMemory(),
      capabilityRef: 'capability.openagents.tassadar.linked_dense_module',
      evidenceRefs: [
        'evidence.public.tassadar.linked_dense_module.marketplace_memory',
      ],
      id: 'marketplace_margin_memory.tassadar_linked_dense_module',
      marketMemoryRef: 'market_memory.tassadar_linked_dense_module',
      modeledMarketplaceValueRefs: [
        'modeled.marketplace_value.tassadar_linked_dense_module',
      ],
      moduleVersionRefs: [
        `module_version.tassadar.linked_dense.${listing.linkedModuleDigest.slice(0, 16)}`,
      ],
      packageRefs: ['developer_package.tassadar.linked_dense_module'],
      programSignatureRefs: [
        'program_signature.tassadar.linked_dense_module.v1',
      ],
      routeRefs: ['route:api.public.tassadar.compiled_module_marketplace'],
      sourceRefs: ['source.public.tassadar.linked_dense_fixture'],
      toolRefs: ['tool.tassadar.linked_dense_replay_verifier'],
      workClassRefs: ['work_class.tassadar.compiled_module_library'],
    },
    'public',
    generatedAt,
  )
  const demandSignal = buildTassadarDemandPriceSignal({
    baselineBudgetSats: 2_500,
    baselineListingValueCents: 12_500,
    capabilityRef: 'capability.openagents.tassadar.linked_dense_module',
    dataTraceValuationRefs: [
      'valuation.public.data_market.tassadar_linked_dense_module',
    ],
    demandCount: 7,
    directionRef: 'direction.public.tassadar.linked_dense_module_library',
    marginRankingScoreBps: marginMemory.rankingScoreBps,
    sourceRefs: [
      listing.listingRef,
      marginMemory.marketMemoryRef,
      'signal.public.artanis.work_direction.demand.linked_dense_module',
    ],
    usageCount: 3,
  })
  const moduleLibrary = rankTassadarCompiledModuleLibrary({
    entries: [
      {
        acceptedUseRefs: ['usage.public.tassadar.linked_dense.demo_001'],
        dedupeKey: 'spec.public.tassadar.linked_dense.w3_100m',
        demandSignal,
        entryRef: 'entry.public.tassadar_compiled_module.linked_dense_canonical',
        listing,
        marginMemory,
        usageCount: 3,
      },
    ],
  })

  return {
    authority: {
      demandPriceMutationAuthority: false,
      listingMutationAuthority: false,
      purchaseMutationAuthority: false,
      rankingMutationAuthority: false,
      requestBudgetMutationAuthority: false,
      realSettlementEnabled: false,
      settlementMutationAuthority: false,
    },
    caveatRefs: [
      'caveat.public.tassadar_compiled_module.marketplace_listing_only',
      'caveat.public.tassadar_compiled_module.demand_rank_projection_only',
      'caveat.public.tassadar_compiled_module.purchase_is_not_settlement',
      'caveat.public.tassadar_compiled_module.real_settlement_owner_gated',
    ],
    claimBoundary:
      'public read-only compiled-weight-module listing metadata plus demand-ranked/deduped library projection; this route exposes digest-pinned linked dense module evidence, replay gate state, demand-price refs, and ranking refs only, and it grants no purchase mutation, settlement mutation, real-money, serving, trained-weight, request-budget mutation, or ranking mutation authority',
    demandSignals: [demandSignal],
    generatedAt,
    listings: [listing],
    marketRef: 'market.public.tassadar_compiled_weight_modules.v1',
    moduleLibrary,
    schemaVersion: TASSADAR_COMPILED_MODULE_MARKETPLACE_SCHEMA_VERSION,
    staleness: liveAtReadStaleness([
      'tassadar_linked_dense_fixture_committed',
      'tassadar_linked_dense_replay_verified',
      'tassadar_compiled_weight_module_listing_read',
      'tassadar_compiled_module_library_demand_rank_read',
      'tassadar_compiled_module_demand_price_signal_read',
    ]),
  }
}
