import { Clock, Effect } from "effect"

import type {
  L402CacheStatus,
  L402Credential,
  L402FetchRequest,
  L402FetchResult,
  L402TransportRequest,
  L402TransportResponse,
} from "../contracts/l402.js"
import { CredentialMissingError } from "../errors/lightningErrors.js"
import { buildAuthorizationHeader } from "./buildAuthorizationHeader.js"
import { parseChallengeHeader } from "./parseChallenge.js"
import type { CredentialCacheApi, CredentialCacheLookup } from "../services/credentialCache.js"
import type { InvoicePayerApi } from "../services/invoicePayer.js"
import type { L402TransportApi } from "../services/l402Transport.js"
import type { SpendPolicyServiceApi } from "../services/spendPolicy.js"

export type FetchWithL402Deps = Readonly<{
  readonly credentialCache: CredentialCacheApi
  readonly payer: InvoicePayerApi
  readonly policy: SpendPolicyServiceApi
  readonly transport: L402TransportApi
}>

const defaultScope = "default"

const hostFromUrl = Effect.fn("l402.hostFromUrl")(function* (url: string) {
  return yield* Effect.try({
    try: () => {
      const parsed = new URL(url)
      return parsed.host.toLowerCase()
    },
    catch: () =>
      CredentialMissingError.make({
        host: url,
        reason: "Request URL must be absolute and parseable",
      }),
  })
})

const getHeaderCaseInsensitive = (headers: Record<string, string>, name: string): string | null => {
  const target = name.trim().toLowerCase()
  for (const [key, value] of Object.entries(headers)) {
    if (key.toLowerCase() === target) return value
  }
  return null
}

const withAuthorizationHeader = (
  request: L402FetchRequest,
  authorizationHeader: string | null,
): L402TransportRequest => {
  const nextHeaders: Record<string, string> = {
    ...(request.headers ?? {}),
  }

  if (authorizationHeader) {
    nextHeaders.Authorization = authorizationHeader
  }

  return {
    url: request.url,
    ...(request.method ? { method: request.method } : {}),
    ...(Object.keys(nextHeaders).length > 0 ? { headers: nextHeaders } : {}),
    ...(request.body !== undefined ? { body: request.body } : {}),
  }
}

const proofReference = (preimageHex: string): string =>
  `preimage:${preimageHex.slice(0, 16)}`

const amountFromLookup = (lookup: CredentialCacheLookup): number =>
  lookup._tag === "hit" || lookup._tag === "stale" ? lookup.credential.amountMsats : 0

const authorizationFromLookup = (lookup: CredentialCacheLookup): string | null =>
  lookup._tag === "hit" ? buildAuthorizationHeader(lookup.credential) : null

const resolveChallengeResponse = Effect.fn("l402.resolveChallengeResponse")(function* (
  response: L402TransportResponse,
  unauthenticatedRequest: L402TransportRequest,
  transport: L402TransportApi,
) {
  if (response.status === 402) return response
  return yield* transport.send(unauthenticatedRequest)
})

const buildResult = (input: {
  readonly request: L402FetchRequest
  readonly host: string
  readonly scope: string
  readonly response: L402TransportResponse
  readonly authorizationHeader: string | null
  readonly cacheStatus: L402CacheStatus
  readonly fromCache: boolean
  readonly paid: boolean
  readonly amountMsats: number
  readonly paymentId: string | null
  readonly proofRef: string
}): L402FetchResult => ({
  url: input.request.url,
  host: input.host,
  scope: input.scope,
  statusCode: input.response.status,
  authorizationHeader: input.authorizationHeader,
  cacheStatus: input.cacheStatus,
  fromCache: input.fromCache,
  paid: input.paid,
  amountMsats: input.amountMsats,
  paymentId: input.paymentId,
  proofReference: input.proofRef,
  ...(input.response.body !== undefined ? { responseBody: input.response.body } : {}),
})

export const fetchWithL402 = Effect.fn("l402.fetchWithL402")(function* (
  request: L402FetchRequest,
  deps: FetchWithL402Deps,
) {
  const scope = request.scope ?? defaultScope
  const host = yield* hostFromUrl(request.url)
  const nowMs = yield* Clock.currentTimeMillis

  const lookup = request.forceRefresh
    ? ({ _tag: "miss" } as const)
    : yield* deps.credentialCache.getByHost(host, scope, nowMs)

  const cacheStatus: L402CacheStatus =
    lookup._tag === "hit" ? "hit" : lookup._tag === "stale" ? "stale" : "miss"

  if (lookup._tag === "stale") {
    yield* deps.credentialCache.markInvalid(host, scope)
  }

  const unauthenticatedRequest = withAuthorizationHeader(request, null)
  const cachedAuthorization = authorizationFromLookup(lookup)
  const initialRequest = withAuthorizationHeader(request, cachedAuthorization)
  const initialResponse = yield* deps.transport.send(initialRequest)

  const canUseCachedCredential =
    lookup._tag === "hit" && initialResponse.status !== 402 && initialResponse.status !== 401
  if (canUseCachedCredential) {
    const cachedCredential = lookup.credential
    return buildResult({
      request,
      host,
      scope,
      response: initialResponse,
      authorizationHeader: cachedAuthorization,
      cacheStatus: "hit",
      fromCache: true,
      paid: false,
      amountMsats: amountFromLookup(lookup),
      paymentId: null,
      proofRef: proofReference(cachedCredential.preimageHex),
    })
  }

  const cacheWasInvalid = lookup._tag === "hit" && (initialResponse.status === 402 || initialResponse.status === 401)
  if (cacheWasInvalid) {
    yield* deps.credentialCache.markInvalid(host, scope)
  }

  if (!cacheWasInvalid && initialResponse.status !== 402) {
    return buildResult({
      request,
      host,
      scope,
      response: initialResponse,
      authorizationHeader: cachedAuthorization,
      cacheStatus,
      fromCache: false,
      paid: false,
      amountMsats: amountFromLookup(lookup),
      paymentId: null,
      proofRef: "none",
    })
  }

  const maybeChallengeResponse = cacheWasInvalid
    ? yield* resolveChallengeResponse(
        initialResponse,
        unauthenticatedRequest,
        deps.transport,
      )
    : initialResponse

  if (maybeChallengeResponse.status !== 402) {
    return buildResult({
      request,
      host,
      scope,
      response: maybeChallengeResponse,
      authorizationHeader: cachedAuthorization,
      cacheStatus: cacheWasInvalid ? "invalid" : cacheStatus,
      fromCache: false,
      paid: false,
      amountMsats: cacheWasInvalid ? 0 : amountFromLookup(lookup),
      paymentId: null,
      proofRef: "none",
    })
  }

  const challengeHeader = getHeaderCaseInsensitive(maybeChallengeResponse.headers, "www-authenticate")
  if (!challengeHeader) {
    return yield* CredentialMissingError.make({
      host,
      reason: "Missing WWW-Authenticate challenge on 402 response",
    })
  }

  const challenge = yield* parseChallengeHeader(challengeHeader)
  const quotedAmountMsats = challenge.amountMsats ?? request.maxSpendMsats

  yield* deps.policy.ensureRequestAllowed({
    host,
    quotedAmountMsats,
    maxSpendMsats: request.maxSpendMsats,
  })

  const payment = yield* deps.payer.payInvoice({
    invoice: challenge.invoice,
    maxAmountMsats: request.maxSpendMsats,
    host,
  })

  const credential: L402Credential = {
    host,
    scope,
    macaroon: challenge.macaroon,
    preimageHex: payment.preimageHex,
    amountMsats: payment.amountMsats,
    issuedAtMs: payment.paidAtMs,
  }

  yield* deps.credentialCache.putByHost(host, scope, credential, {
    ...(request.cacheTtlMs !== undefined ? { ttlMs: request.cacheTtlMs } : {}),
  })

  const authorizationHeader = buildAuthorizationHeader(credential)
  const retryResponse = yield* deps.transport.send(
    withAuthorizationHeader(request, authorizationHeader),
  )

  if (retryResponse.status === 402 || retryResponse.status === 401) {
    yield* deps.credentialCache.markInvalid(host, scope)
    return yield* CredentialMissingError.make({
      host,
      reason: "Credential rejected on post-payment retry",
    })
  }

  return buildResult({
    request,
    host,
    scope,
    response: retryResponse,
    authorizationHeader,
    cacheStatus: cacheWasInvalid ? "invalid" : cacheStatus,
    fromCache: false,
    paid: true,
    amountMsats: payment.amountMsats,
    paymentId: payment.paymentId,
    proofRef: proofReference(payment.preimageHex),
  })
})
