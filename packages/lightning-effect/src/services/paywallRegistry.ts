import { Context, Effect } from "effect"

import type { PaywallDefinition, PaywallStatus } from "../contracts/seller.js"

export type PaywallRegistryApi = Readonly<{
  readonly upsert: (definition: PaywallDefinition) => Effect.Effect<PaywallDefinition>
  readonly getById: (paywallId: string) => Effect.Effect<PaywallDefinition | null>
  readonly listByOwner: (ownerId: string) => Effect.Effect<ReadonlyArray<PaywallDefinition>>
  readonly setStatus: (
    paywallId: string,
    status: PaywallStatus,
  ) => Effect.Effect<PaywallDefinition | null>
}>

export class PaywallRegistryService extends Context.Tag(
  "@openagents/lightning-effect/PaywallRegistryService",
)<PaywallRegistryService, PaywallRegistryApi>() {}
