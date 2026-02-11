import { Context, Effect } from "effect"

import type { L402FetchRequest, L402FetchResult } from "../contracts/l402.js"
import type { LightningEffectError } from "../errors/lightningErrors.js"

export type L402ClientApi = Readonly<{
  readonly authorizeRequest: (
    request: L402FetchRequest,
  ) => Effect.Effect<L402FetchResult, LightningEffectError>
}>

export class L402ClientService extends Context.Tag("@openagents/lightning-effect/L402ClientService")<
  L402ClientService,
  L402ClientApi
>() {}
