import { Effect, Schema } from "effect"

import { L402Challenge, L402Credential } from "../contracts/l402.js"
import { ChallengeParseError } from "../errors/lightningErrors.js"

const challengePrefix = /^L402\s+/i

const parseAttributes = (raw: string): Record<string, string> => {
  const out: Record<string, string> = {}
  const parts = raw
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)

  for (const part of parts) {
    const match = /^([A-Za-z0-9_\-]+)\s*=\s*("([^"]*)"|[^\s]+)$/.exec(part)
    if (!match) continue
    const key = match[1]?.trim().toLowerCase()
    const value = (match[3] ?? match[2] ?? "").replace(/^"|"$/g, "").trim()
    if (key && value.length > 0) out[key] = value
  }

  return out
}

const decodeChallenge = Schema.decodeUnknown(L402Challenge)

export const parseChallengeHeader = Effect.fn("l402.parseChallengeHeader")(function* (header: string) {
  const trimmed = header.trim()
  if (!trimmed || !challengePrefix.test(trimmed)) {
    return yield* ChallengeParseError.make({
      header,
      reason: "Expected an L402 challenge header",
    })
  }

  const attrs = parseAttributes(trimmed.replace(challengePrefix, ""))
  const invoice = attrs.invoice
  const macaroon = attrs.macaroon
  const amountMsatsRaw = attrs.amount_msats ?? attrs.amountmsats

  if (!invoice || !macaroon) {
    return yield* ChallengeParseError.make({
      header,
      reason: "Challenge must include invoice and macaroon attributes",
    })
  }

  const amountMsats =
    amountMsatsRaw && Number.isFinite(Number(amountMsatsRaw))
      ? Math.max(0, Math.floor(Number(amountMsatsRaw)))
      : undefined

  return yield* decodeChallenge({
    invoice,
    macaroon,
    ...(amountMsats !== undefined ? { amountMsats } : {}),
    ...(attrs.issuer ? { issuer: attrs.issuer } : {}),
  }).pipe(
    Effect.mapError(
      () =>
        ChallengeParseError.make({
          header,
          reason: "L402 challenge failed schema validation",
        }),
    ),
  )
})

export const buildAuthorizationHeader = (credential: L402Credential): string =>
  `L402 macaroon="${credential.macaroon}", preimage="${credential.preimageHex}"`
