import { describe, expect, test } from "vite-plus/test"

import {
  INDEPENDENT_REVIEW_SCHEMA_ID,
  decodeIndependentReviewReceipt,
  independentReviewAdmits,
  verifyIndependentReviewSignature,
} from "../src/independent-review.ts"

const REVIEWER = "0326d8f9eb5abea63d9613ac90451dfce62ca2e9855144b5a71d8e8569932974"
const PRODUCER = "a".repeat(64)
const OTHER_PRODUCER = "b".repeat(64)
const digest = (seed: string) => `sha256:${seed.repeat(64).slice(0, 64)}`

const reproduction = (check: string, agrees = true) => ({
  check,
  command: `python3 script/${check}`,
  observedDigest: digest("c"),
  agreesWithProducer: agrees,
})

const receipt = (overrides: Record<string, unknown> = {}) => ({
  schema: INDEPENDENT_REVIEW_SCHEMA_ID,
  reviewerPubkey: REVIEWER,
  producerPubkeys: [PRODUCER],
  candidateDigest: digest("d"),
  obligationRef: "omega#16",
  reviewedAt: "2026-07-25T00:00:00Z",
  outcome: "accepted",
  reproductions: [reproduction("identity-matrix"), reproduction("tripwires")],
  disagreements: [],
  evidenceSha256: digest("e"),
  reviewerSignature: "f".repeat(128),
  ...overrides,
})

// The refusal path is tested first, deliberately. A reviewer built
// acceptance-first tends to grow an accept-shaped hole.
describe("a review can refuse", () => {
  test("refuses when a reproduction disagrees with the producer", () => {
    const decoded = decodeIndependentReviewReceipt(
      receipt({
        outcome: "refused",
        reproductions: [reproduction("identity-matrix"), reproduction("tripwires", false)],
      }),
    )
    expect(decoded.outcome).toBe("refused")
    expect(independentReviewAdmits(decoded)).toBe(false)
  })

  test("refuses with a named criterion and both sides of the disagreement", () => {
    const decoded = decodeIndependentReviewReceipt(
      receipt({
        outcome: "refused",
        disagreements: [
          {
            criterion: "no public surface presents Zed as the product",
            producerClaim: "binary is clean",
            reviewerObservation: "strings found 'Review .zed/settings.json' twice",
          },
        ],
      }),
    )
    expect(decoded.disagreements[0]?.criterion).toContain("Zed")
    expect(independentReviewAdmits(decoded)).toBe(false)
  })

  test("reports inconclusive without that reading as approval", () => {
    // A reviewer that cannot reproduce something must be able to say so. If
    // the only alternative to `accepted` were `refused`, a missing tool would
    // push an honest reviewer toward one of two wrong answers.
    const decoded = decodeIndependentReviewReceipt(
      receipt({
        outcome: "inconclusive",
        reproductions: [reproduction("identity-matrix", false)],
        disagreements: [
          {
            criterion: "installed tripwires",
            producerClaim: "6 surfaces clean",
            reviewerObservation: "collector unavailable on this host",
          },
        ],
      }),
    )
    expect(independentReviewAdmits(decoded)).toBe(false)
  })

  test("rejects a refusal that gives no reason", () => {
    expect(() =>
      decodeIndependentReviewReceipt(receipt({ outcome: "refused" })),
    ).toThrow(/without a recorded reason/)
  })
})

describe("acceptance is hard to reach", () => {
  test("accepts only when every reproduction agrees and nothing is disputed", () => {
    const decoded = decodeIndependentReviewReceipt(receipt())
    expect(independentReviewAdmits(decoded)).toBe(true)
  })

  test("refuses to accept while a reproduction disagreed", () => {
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({ reproductions: [reproduction("identity-matrix", false)] }),
      ),
    ).toThrow(/reproduction disagreed/)
  })

  test("refuses to accept while a disagreement is recorded", () => {
    // There is no partial acceptance.
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({
          disagreements: [
            { criterion: "c", producerClaim: "p", reviewerObservation: "o" },
          ],
        }),
      ),
    ).toThrow(/accepted with recorded disagreements/)
  })

  test("refuses a review that reproduced nothing", () => {
    // Reading the producer's JSON and confirming it parses is not review.
    expect(() => decodeIndependentReviewReceipt(receipt({ reproductions: [] }))).toThrow(
      /no reproduction was recorded/,
    )
  })

  test("refuses a review that counts one check twice", () => {
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({ reproductions: [reproduction("identity-matrix"), reproduction("identity-matrix")] }),
      ),
    ).toThrow(/reproduced more than once/)
  })
})

describe("independence is structural, not declared", () => {
  test("refuses a review the producer signed", () => {
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({ reviewerPubkey: PRODUCER, producerPubkeys: [PRODUCER] }),
      ),
    ).toThrow(/reviewer key is also a producer key/)
  })

  test("refuses when the reviewer is one of several producers", () => {
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({ producerPubkeys: [OTHER_PRODUCER, REVIEWER] }),
      ),
    ).toThrow(/reviewer key is also a producer key/)
  })

  test("refuses an obligation with no named producer", () => {
    // Independence from nobody is not independence.
    expect(() => decodeIndependentReviewReceipt(receipt({ producerPubkeys: [] }))).toThrow(
      /no producer to be independent of/,
    )
  })
})

describe("no reviewer shopping", () => {
  test("allows superseding a prior review with a stated reason", () => {
    const decoded = decodeIndependentReviewReceipt(
      receipt({ supersedes: digest("f"), supersedesReason: "reviewer host lacked notarytool" }),
    )
    expect(decoded.supersedes).toBe(digest("f"))
  })

  test("refuses a silent supersede", () => {
    expect(() =>
      decodeIndependentReviewReceipt(receipt({ supersedes: digest("f") })),
    ).toThrow(/without a stated reason/)
  })

  test("refuses a reason with nothing to supersede", () => {
    expect(() =>
      decodeIndependentReviewReceipt(receipt({ supersedesReason: "retry" })),
    ).toThrow(/without naming the review it replaces/)
  })

  test("refuses a review that supersedes itself", () => {
    expect(() =>
      decodeIndependentReviewReceipt(
        receipt({ supersedes: digest("e"), supersedesReason: "loop" }),
      ),
    ).toThrow(/cannot supersede itself/)
  })
})

describe("the reviewer identity is checkable, not asserted", () => {
  test("verifies the signature over the evidence digest with the reviewer key", () => {
    const decoded = decodeIndependentReviewReceipt(receipt())
    const seen: Array<string> = []
    const ok = verifyIndependentReviewSignature(decoded, (signature, message, pubkey) => {
      seen.push(signature, message, pubkey)
      return true
    })
    expect(ok).toBe(true)
    // It must sign the evidence digest with the reviewer key, not something else.
    expect(seen).toEqual(["f".repeat(128), digest("e"), REVIEWER])
  })

  test("reports a bad signature rather than trusting the stated pubkey", () => {
    // Without this, a producer could write the reviewer's public key into a
    // document it authored itself and the receipt would look independent.
    const decoded = decodeIndependentReviewReceipt(receipt())
    expect(verifyIndependentReviewSignature(decoded, () => false)).toBe(false)
  })

  test("refuses a receipt with a malformed signature", () => {
    expect(() => decodeIndependentReviewReceipt(receipt({ reviewerSignature: "nope" }))).toThrow()
  })
})
