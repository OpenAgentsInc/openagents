import { Context, Effect } from "effect"

import type {
  LndPaymentListResult,
  LndPaymentRecord,
  LndPaymentSendRequest,
  LndPaymentTrackRequest,
} from "../contracts/rpc.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndPaymentApi = Readonly<{
  readonly sendPayment: (
    request: LndPaymentSendRequest,
  ) => Effect.Effect<LndPaymentRecord, LndServiceUnavailableError>
  readonly trackPayment: (
    request: LndPaymentTrackRequest,
  ) => Effect.Effect<LndPaymentRecord, LndServiceUnavailableError>
  readonly listPayments: (input?: {
    readonly limit?: number
    readonly offset?: number
  }) => Effect.Effect<LndPaymentListResult, LndServiceUnavailableError>
}>

export class LndPaymentService extends Context.Tag("@openagents/lnd-effect/LndPaymentService")<
  LndPaymentService,
  LndPaymentApi
>() {}
