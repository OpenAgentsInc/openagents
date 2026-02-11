import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { buildAuthorizationHeader, parseChallengeHeader } from "../src/l402/challenge.js"

describe("l402 challenge helpers", () => {
  it.effect("parses a valid challenge header with amount", () =>
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

  it.effect("builds an authorization header from credential", () =>
    Effect.gen(function* () {
      const header = buildAuthorizationHeader({
        host: "api.example.com",
        macaroon: "AgEDbWFjYXJvb24=",
        preimageHex: "ab".repeat(32),
        amountMsats: 2500,
        issuedAtMs: 1_700_000_000_000,
      })

      expect(header).toContain('macaroon="AgEDbWFjYXJvb24="')
      expect(header).toContain(`preimage="${"ab".repeat(32)}"`)
    }),
  )
})
