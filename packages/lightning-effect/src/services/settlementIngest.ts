import { Context, Effect } from "effect"

import type { SettlementRecord } from "../contracts/seller.js"

export type SettlementIngestApi = Readonly<{
  readonly ingest: (record: SettlementRecord) => Effect.Effect<SettlementRecord>
  readonly getBySettlementId: (settlementId: string) => Effect.Effect<SettlementRecord | null>
  readonly listByPaywall: (paywallId: string) => Effect.Effect<ReadonlyArray<SettlementRecord>>
}>

export class SettlementIngestService extends Context.Tag(
  "@openagents/lightning-effect/SettlementIngestService",
)<SettlementIngestService, SettlementIngestApi>() {}
