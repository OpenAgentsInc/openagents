import { Context, Effect } from "effect"

import type { LndRpcRequest, LndRpcResponse } from "../contracts/rpc.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndTransportApi = Readonly<{
  readonly send: (request: LndRpcRequest) => Effect.Effect<LndRpcResponse, LndServiceUnavailableError>
}>

export class LndTransportService extends Context.Tag("@openagents/lnd-effect/LndTransportService")<
  LndTransportService,
  LndTransportApi
>() {}
