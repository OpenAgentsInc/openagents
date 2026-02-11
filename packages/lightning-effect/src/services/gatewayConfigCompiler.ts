import { Context, Effect } from "effect"

import type { GatewayConfigCompileResult, PaywallDefinition } from "../contracts/seller.js"

export type GatewayConfigCompilerApi = Readonly<{
  readonly compilePaywalls: (
    paywalls: ReadonlyArray<PaywallDefinition>,
  ) => Effect.Effect<GatewayConfigCompileResult>
}>

export class GatewayConfigCompilerService extends Context.Tag(
  "@openagents/lightning-effect/GatewayConfigCompilerService",
)<GatewayConfigCompilerService, GatewayConfigCompilerApi>() {}
