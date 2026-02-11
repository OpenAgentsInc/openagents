import { Context, Effect } from "effect"

import type { LndPayment } from "../contracts/rpc.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndPaymentApi = Readonly<{
  readonly trackPayment: (paymentHash: string) => Effect.Effect<LndPayment, LndServiceUnavailableError>
}>

export class LndPaymentService extends Context.Tag("@openagents/lnd-effect/LndPaymentService")<
  LndPaymentService,
  LndPaymentApi
>() {}
