import { Effect } from "effect"

import type { L402Credential } from "../contracts/l402.js"
import { decodeL402Credential } from "../contracts/l402.js"
import { AuthorizationSerializeError } from "../errors/lightningErrors.js"

const quoteHeaderValue = (value: string): string => value.replaceAll("\\", "\\\\").replaceAll("\"", '\\"')

export const buildAuthorizationHeader = (credential: L402Credential): string =>
  `L402 macaroon="${quoteHeaderValue(credential.macaroon)}", preimage="${quoteHeaderValue(credential.preimageHex)}"`

export const serializeAuthorizationHeader = Effect.fn("l402.serializeAuthorizationHeader")(function* (
  input: unknown,
) {
  const credential = yield* decodeL402Credential(input).pipe(
    Effect.mapError(() =>
      AuthorizationSerializeError.make({
        reason: "Authorization header requires a valid L402Credential value",
      }),
    ),
  )

  return buildAuthorizationHeader(credential)
})
