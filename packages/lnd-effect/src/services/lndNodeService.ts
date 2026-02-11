import { Context, Effect } from "effect"

import type { LndNodeInfo } from "../contracts/lnd.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndNodeApi = Readonly<{
  readonly getNodeInfo: () => Effect.Effect<LndNodeInfo, LndServiceUnavailableError>
}>

export class LndNodeService extends Context.Tag("@openagents/lnd-effect/LndNodeService")<
  LndNodeService,
  LndNodeApi
>() {}
