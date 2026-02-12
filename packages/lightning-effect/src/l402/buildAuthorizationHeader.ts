import { Effect } from "effect"

import type { L402AuthorizationHeaderStrategy, L402Credential } from "../contracts/l402.js"
import { decodeL402Credential } from "../contracts/l402.js"
import { AuthorizationSerializeError } from "../errors/lightningErrors.js"

const quoteHeaderValue = (value: string): string => value.replaceAll("\\", "\\\\").replaceAll("\"", '\\"')

export const defaultAuthorizationHeaderStrategy: L402AuthorizationHeaderStrategy =
  "macaroon_preimage_params"

export const buildAuthorizationHeader = (
  credential: L402Credential,
  strategy: L402AuthorizationHeaderStrategy = defaultAuthorizationHeaderStrategy,
): string => {
  if (strategy === "macaroon_preimage_colon") {
    return `L402 ${credential.macaroon}:${credential.preimageHex}`
  }
  return `L402 macaroon="${quoteHeaderValue(credential.macaroon)}", preimage="${quoteHeaderValue(credential.preimageHex)}"`
}

export const serializeAuthorizationHeader = Effect.fn("l402.serializeAuthorizationHeader")(function* (
  input: unknown,
  strategy: L402AuthorizationHeaderStrategy = defaultAuthorizationHeaderStrategy,
) {
  const credential = yield* decodeL402Credential(input).pipe(
    Effect.mapError(() =>
      AuthorizationSerializeError.make({
        reason: "Authorization header requires a valid L402Credential value",
      }),
    ),
  )

  return buildAuthorizationHeader(credential, strategy)
})
