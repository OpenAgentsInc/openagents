import { Context, Effect } from "effect"

import type { LndWalletState } from "../contracts/lnd.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndWalletApi = Readonly<{
  readonly getWalletState: () => Effect.Effect<LndWalletState, LndServiceUnavailableError>
}>

export class LndWalletService extends Context.Tag("@openagents/lnd-effect/LndWalletService")<
  LndWalletService,
  LndWalletApi
>() {}
