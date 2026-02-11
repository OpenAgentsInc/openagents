import { Effect, Layer } from "effect"

import type { L402FetchRequest } from "../contracts/l402.js"
import { fetchWithL402 } from "../l402/fetchWithL402.js"
import { CredentialCacheService } from "../services/credentialCache.js"
import { InvoicePayerService } from "../services/invoicePayer.js"
import { L402ClientService } from "../services/l402Client.js"
import { L402TransportService } from "../services/l402Transport.js"
import { SpendPolicyService } from "../services/spendPolicy.js"

export const L402ClientLiveLayer = Layer.effect(
  L402ClientService,
  Effect.gen(function* () {
    const credentialCache = yield* CredentialCacheService
    const payer = yield* InvoicePayerService
    const policy = yield* SpendPolicyService
    const transport = yield* L402TransportService

    const fetchFn = Effect.fn("L402Client.fetchWithL402")(function* (request: L402FetchRequest) {
      return yield* fetchWithL402(request, {
        credentialCache,
        payer,
        policy,
        transport,
      })
    })

    const authorizeFn = Effect.fn("L402Client.authorizeRequest")(function* (request: L402FetchRequest) {
      return yield* fetchFn(request)
    })

    return L402ClientService.of({
      fetchWithL402: fetchFn,
      authorizeRequest: authorizeFn,
    })
  }),
)
