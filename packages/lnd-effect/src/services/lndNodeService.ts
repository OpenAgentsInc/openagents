import { Context, Effect } from "effect"

import type { LndBalanceSummary, LndChannelSummary, LndNodeInfo, LndNodeSnapshot } from "../contracts/lnd.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndNodeApi = Readonly<{
  readonly getNodeInfo: () => Effect.Effect<LndNodeInfo, LndServiceUnavailableError>
  readonly getBalanceSummary: () => Effect.Effect<LndBalanceSummary, LndServiceUnavailableError>
  readonly getChannelSummary: () => Effect.Effect<LndChannelSummary, LndServiceUnavailableError>
  readonly getNodeSnapshot: () => Effect.Effect<LndNodeSnapshot, LndServiceUnavailableError>
}>

export class LndNodeService extends Context.Tag("@openagents/lnd-effect/LndNodeService")<
  LndNodeService,
  LndNodeApi
>() {}
