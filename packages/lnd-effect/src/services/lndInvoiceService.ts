import { Context, Effect } from "effect"

import type {
  LndInvoiceCreateRequest,
  LndInvoiceListResult,
  LndInvoiceLookupRequest,
  LndInvoiceRecord,
} from "../contracts/rpc.js"
import type { LndServiceUnavailableError } from "../errors/lndErrors.js"

export type LndInvoiceApi = Readonly<{
  readonly createInvoice: (
    input: LndInvoiceCreateRequest,
  ) => Effect.Effect<LndInvoiceRecord, LndServiceUnavailableError>
  readonly getInvoice: (
    input: LndInvoiceLookupRequest,
  ) => Effect.Effect<LndInvoiceRecord | null, LndServiceUnavailableError>
  readonly listInvoices: (input?: {
    readonly limit?: number
    readonly offset?: number
  }) => Effect.Effect<LndInvoiceListResult, LndServiceUnavailableError>
}>

export class LndInvoiceService extends Context.Tag("@openagents/lnd-effect/LndInvoiceService")<
  LndInvoiceService,
  LndInvoiceApi
>() {}
