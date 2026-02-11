import { Context, Effect } from "effect"

import type { LndInvoice } from "../contracts/rpc.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndInvoiceApi = Readonly<{
  readonly createInvoice: (input: { readonly amountSat: number }) => Effect.Effect<LndInvoice, LndServiceUnavailableError>
}>

export class LndInvoiceService extends Context.Tag("@openagents/lnd-effect/LndInvoiceService")<
  LndInvoiceService,
  LndInvoiceApi
>() {}
