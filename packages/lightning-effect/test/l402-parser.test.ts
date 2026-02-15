import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { decodeL402Challenge } from "../src/contracts/l402.js"
import { decodeInvoicePaymentResult } from "../src/contracts/payment.js"
import { parseChallengeHeader } from "../src/l402/parseChallenge.js"
import {
  buildAuthorizationHeader,
  serializeAuthorizationHeader,
} from "../src/l402/buildAuthorizationHeader.js"

describe("l402 parser and serializer", () => {
  it.effect("parses a valid challenge with required and optional attributes", () =>
    Effect.gen(function* () {
      const challenge = yield* parseChallengeHeader(
        'L402 invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2500, issuer="aperture"',
      )

      expect(challenge.invoice).toBe("lnbcrt1invoice")
      expect(challenge.macaroon).toBe("AgEDbWFjYXJvb24=")
      expect(challenge.amountMsats).toBe(2500)
      expect(challenge.issuer).toBe("aperture")
    }),
  )

  it.effect("parses LSAT challenge scheme as L402-compatible", () =>
    Effect.gen(function* () {
      const challenge = yield* parseChallengeHeader(
        'LSAT invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2500',
      )

      expect(challenge.invoice).toBe("lnbcrt1invoice")
      expect(challenge.macaroon).toBe("AgEDbWFjYXJvb24=")
      expect(challenge.amountMsats).toBe(2500)
    }),
  )

  it.effect("parses combined auth header and prefers explicit L402 challenge", () =>
    Effect.gen(function* () {
      const challenge = yield* parseChallengeHeader(
        'LSAT macaroon="legacy", invoice="lnlegacy", L402 macaroon="mac_new", invoice="ln_new", amount_msats=700',
      )

      expect(challenge.invoice).toBe("ln_new")
      expect(challenge.macaroon).toBe("mac_new")
      expect(challenge.amountMsats).toBe(700)
    }),
  )

  it.effect("infers amount_msats from BOLT11 invoice when missing", () =>
    Effect.gen(function* () {
      const challenge = yield* parseChallengeHeader(
        'L402 invoice="lnbc2500n1exampleinvoice", macaroon="AgEDbWFjYXJvb24="',
      )

      // 2500n = 250 sat = 250,000 msats.
      expect(challenge.amountMsats).toBe(250_000)
    }),
  )

  it.effect("rejects malformed headers deterministically", () =>
    Effect.gen(function* () {
      const malformed = yield* Effect.either(
        parseChallengeHeader('L402 invoice="lnbcrt1invoice",, macaroon="AgEDbWFjYXJvb24="'),
      )
      expect(malformed._tag).toBe("Left")
      if (malformed._tag === "Left") {
        expect(malformed.left._tag).toBe("ChallengeParseError")
        expect(malformed.left.reason).toBe("Challenge contains an empty attribute entry")
      }

      const invalidAmount = yield* Effect.either(
        parseChallengeHeader(
          'L402 invoice="lnbcrt1invoice", macaroon="AgEDbWFjYXJvb24=", amount_msats=2.5',
        ),
      )
      expect(invalidAmount._tag).toBe("Left")
      if (invalidAmount._tag === "Left") {
        expect(invalidAmount.left._tag).toBe("ChallengeParseError")
        expect(invalidAmount.left.reason).toBe("amount_msats must be a non-negative integer")
      }

      const missingRequired = yield* Effect.either(
        parseChallengeHeader('L402 invoice="lnbcrt1invoice"'),
      )
      expect(missingRequired._tag).toBe("Left")
      if (missingRequired._tag === "Left") {
        expect(missingRequired.left._tag).toBe("ChallengeParseError")
        expect(missingRequired.left.reason).toBe(
          "Challenge must include invoice and macaroon attributes",
        )
      }
    }),
  )

  it.effect("serializes L402 authorization header in stable format", () =>
    Effect.gen(function* () {
      const direct = buildAuthorizationHeader({
        host: "api.example.com",
        macaroon: "AgEDbWFjYXJvb24=",
        preimageHex: "ab".repeat(32),
        amountMsats: 2500,
        issuedAtMs: 1_700_000_000_000,
      })
      expect(direct).toBe(
        `L402 macaroon="AgEDbWFjYXJvb24=", preimage="${"ab".repeat(32)}"`,
      )

      const serialized = yield* serializeAuthorizationHeader({
        host: "api.example.com",
        macaroon: "AgEDbWFjYXJvb24=",
        preimageHex: "ab".repeat(32),
        amountMsats: 2500,
        issuedAtMs: 1_700_000_000_000,
      })
      expect(serialized).toBe(
        `L402 macaroon="AgEDbWFjYXJvb24=", preimage="${"ab".repeat(32)}"`,
      )

      const directColon = buildAuthorizationHeader(
        {
          host: "sats4ai.com",
          macaroon: "mac_sats4ai",
          preimageHex: "cd".repeat(32),
          amountMsats: 2500,
          issuedAtMs: 1_700_000_000_000,
        },
        "macaroon_preimage_colon",
      )
      expect(directColon).toBe(`L402 mac_sats4ai:${"cd".repeat(32)}`)

      const serializedColon = yield* serializeAuthorizationHeader(
        {
          host: "sats4ai.com",
          macaroon: "mac_sats4ai",
          preimageHex: "cd".repeat(32),
          amountMsats: 2500,
          issuedAtMs: 1_700_000_000_000,
        },
        "macaroon_preimage_colon",
      )
      expect(serializedColon).toBe(`L402 mac_sats4ai:${"cd".repeat(32)}`)
    }),
  )

  it.effect("enforces required schema fields for challenge and payment models", () =>
    Effect.gen(function* () {
      const missingMacaroon = yield* Effect.either(
        decodeL402Challenge({ invoice: "lnbcrt1invoice" }),
      )
      expect(missingMacaroon._tag).toBe("Left")

      const missingPreimage = yield* Effect.either(
        decodeInvoicePaymentResult({
          paymentId: "pay_abc",
          amountMsats: 123,
          paidAtMs: 1_700_000_000_000,
        }),
      )
      expect(missingPreimage._tag).toBe("Left")
    }),
  )
})
