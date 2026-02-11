import { Context, Effect } from "effect"

import type { LndRpcRequest, LndRpcResponse } from "../contracts/rpc.js"
import type {
  LndAuthenticationError,
  LndResponseDecodeError,
  LndTransportError,
} from "../errors/lndErrors.js"

export type LndTransportServiceError =
  | LndTransportError
  | LndAuthenticationError
  | LndResponseDecodeError

export type LndTransportApi = Readonly<{
  readonly send: (request: LndRpcRequest) => Effect.Effect<LndRpcResponse, LndTransportServiceError>
}>

export class LndTransportService extends Context.Tag("@openagents/lnd-effect/LndTransportService")<
  LndTransportService,
  LndTransportApi
>() {}
