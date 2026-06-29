import { describe, expect, test } from "bun:test"
import {
  buildSmokeReceipt,
  checkProofBoundary,
  type SmokeClaim,
  type SmokeReceipt,
} from "../src/tas/smoke-proof"

const receiptKeys: Array<keyof SmokeReceipt> = [
  "evidenceRefs",
  "ok",
  "unbackedCount",
]

describe("smoke proof boundary", () => {
  test("accepts all-backed smoke claims", () => {
    const claims: SmokeClaim[] = [
      {
        claim: "CI-safe smoke verified adapter startup",
        evidenceRef: "evidence.fixture.adapter_startup",
      },
      {
        claim: "Workspace smoke produced a public-safe receipt",
        evidenceRef: "evidence.fixture.workspace_receipt",
      },
    ]

    expect(checkProofBoundary(claims)).toEqual({
      ok: true,
      unbacked: [],
    })
  })

  test("rejects and lists an unbacked claim", () => {
    const claims: SmokeClaim[] = [
      {
        claim: "CI-safe smoke verified adapter startup",
        evidenceRef: "evidence.fixture.adapter_startup",
      },
      {
        claim: "Live smoke proves broad provider readiness",
      },
    ]

    expect(checkProofBoundary(claims)).toEqual({
      ok: false,
      unbacked: ["Live smoke proves broad provider readiness"],
    })
  })

  test("builds refs-only smoke receipts", () => {
    const privateClaim = "Read /Users/example/private/repo and proved live deployment"
    const claims: SmokeClaim[] = [
      {
        claim: privateClaim,
        evidenceRef: "evidence.fixture.live_deploy",
      },
      {
        claim: "Broad claim without a proof boundary",
      },
    ]

    const receipt = buildSmokeReceipt(claims)

    expect(receipt).toEqual({
      ok: false,
      evidenceRefs: ["evidence.fixture.live_deploy"],
      unbackedCount: 1,
    })
    expect(Object.keys(receipt).sort()).toEqual([...receiptKeys].sort())
    expect(JSON.stringify(receipt)).not.toContain(privateClaim)
    expect(JSON.stringify(receipt)).not.toContain("Broad claim without a proof boundary")
  })
})
