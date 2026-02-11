import { Context, Effect } from "effect"

import type { L402TransportRequest, L402TransportResponse } from "../contracts/l402.js"
import type { L402TransportError } from "../errors/lightningErrors.js"

export type L402TransportApi = Readonly<{
  readonly send: (
    request: L402TransportRequest,
  ) => Effect.Effect<L402TransportResponse, L402TransportError>
}>

export class L402TransportService extends Context.Tag("@openagents/lightning-effect/L402TransportService")<
  L402TransportService,
  L402TransportApi
>() {}
