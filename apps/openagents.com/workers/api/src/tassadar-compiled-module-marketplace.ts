import {
  projectTassadarCompiledWeightModuleListing,
  tassadarLinkedDenseProgramFixture,
  type TassadarCompiledWeightModuleListing,
} from '@openagentsinc/tassadar-executor/linked-dense-module'

import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

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
  authority: Readonly<{
    listingMutationAuthority: false
    purchaseMutationAuthority: false
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

  return {
    authority: {
      listingMutationAuthority: false,
      purchaseMutationAuthority: false,
      realSettlementEnabled: false,
      settlementMutationAuthority: false,
    },
    caveatRefs: [
      'caveat.public.tassadar_compiled_module.marketplace_listing_only',
      'caveat.public.tassadar_compiled_module.purchase_is_not_settlement',
      'caveat.public.tassadar_compiled_module.real_settlement_owner_gated',
    ],
    claimBoundary:
      'public read-only compiled-weight-module listing metadata; this route exposes digest-pinned linked dense module evidence and replay gate state only, and it grants no purchase mutation, settlement mutation, real-money, serving, or trained-weight authority',
    generatedAt,
    listings: [listing],
    marketRef: 'market.public.tassadar_compiled_weight_modules.v1',
    schemaVersion: TASSADAR_COMPILED_MODULE_MARKETPLACE_SCHEMA_VERSION,
    staleness: liveAtReadStaleness([
      'tassadar_linked_dense_fixture_committed',
      'tassadar_linked_dense_replay_verified',
      'tassadar_compiled_weight_module_listing_read',
    ]),
  }
}
