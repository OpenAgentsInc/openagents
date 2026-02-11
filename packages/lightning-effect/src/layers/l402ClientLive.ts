import { Effect, Layer, Option } from "effect"

import type { L402Credential, L402FetchRequest, L402FetchResult } from "../contracts/l402.js"
import { ChallengeParseError, CredentialMissingError } from "../errors/lightningErrors.js"
import { buildAuthorizationHeader, parseChallengeHeader } from "../l402/challenge.js"
import { CredentialCacheService } from "../services/credentialCache.js"
import { InvoicePayerService } from "../services/invoicePayer.js"
import { L402ClientService } from "../services/l402Client.js"
import { SpendPolicyService } from "../services/spendPolicy.js"

const hostFromUrl = (url: string): Effect.Effect<string, ChallengeParseError> =>
  Effect.try({
    try: () => {
      const parsed = new URL(url)
      return parsed.host.toLowerCase()
    },
    catch: () =>
      ChallengeParseError.make({
        header: url,
        reason: "Request URL must be absolute and parseable",
      }),
  })

const proofReference = (preimageHex: string): string =>
  `preimage:${preimageHex.slice(0, 16)}`

const toCachedResult = (request: L402FetchRequest, host: string, credential: L402Credential): L402FetchResult => ({
  url: request.url,
  host,
  authorizationHeader: buildAuthorizationHeader(credential),
  fromCache: true,
  amountMsats: credential.amountMsats,
  paymentId: null,
  proofReference: proofReference(credential.preimageHex),
})

export const L402ClientLiveLayer = Layer.effect(
  L402ClientService,
  Effect.gen(function* () {
    const credentialCache = yield* CredentialCacheService
    const payer = yield* InvoicePayerService
    const policy = yield* SpendPolicyService

    const authorizeRequest = Effect.fn("L402Client.authorizeRequest")(function* (
      request: L402FetchRequest,
    ) {
      const host = yield* hostFromUrl(request.url)

      const cached = yield* credentialCache.getByHost(host)
      if (!request.forceRefresh && Option.isSome(cached)) {
        return toCachedResult(request, host, cached.value)
      }

      if (!request.challengeHeader || request.challengeHeader.trim().length === 0) {
        return yield* CredentialMissingError.make({
          host,
          reason: "Challenge header is required when no cached credential exists",
        })
      }

      const challenge = yield* parseChallengeHeader(request.challengeHeader)
      const quotedAmountMsats = challenge.amountMsats ?? request.maxSpendMsats

      yield* policy.ensureRequestAllowed({
        host,
        quotedAmountMsats,
        maxSpendMsats: request.maxSpendMsats,
      })

      const payment = yield* payer.payInvoice({
        invoice: challenge.invoice,
        maxAmountMsats: request.maxSpendMsats,
        host,
      })

      const credential: L402Credential = {
        host,
        macaroon: challenge.macaroon,
        preimageHex: payment.preimageHex,
        amountMsats: payment.amountMsats,
        issuedAtMs: payment.paidAtMs,
      }

      yield* credentialCache.putByHost(host, credential)

      const result: L402FetchResult = {
        url: request.url,
        host,
        authorizationHeader: buildAuthorizationHeader(credential),
        fromCache: false,
        amountMsats: payment.amountMsats,
        paymentId: payment.paymentId,
        proofReference: proofReference(payment.preimageHex),
      }
      return result
    })

    return L402ClientService.of({ authorizeRequest })
  }),
)
