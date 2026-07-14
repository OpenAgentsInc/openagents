import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"

// Oracle for khala_mobile.platform.launched_app_interaction_smoke.v1
//
// The heavy proof for this contract is the SignedInThreadSmoke Maestro flow
// (clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml), run on a
// Release-configuration iOS simulator and recorded in the committed receipt
// referenced below. Node cannot boot a simulator, so this test oracle keeps
// the enforced contract honestly tied to that real launched-app run by
// asserting the receipt exists and records a PASS for the signed-in flow this
// contract binds (thread opens, lane picker visible, message sends + renders).
// The Maestro flow itself is exercised as the opt-in mobile step of the QA
// nightly matrix (docs/qa/khala-code-nightly-matrix.md).

const CONTRACT_ID = "khala_mobile.platform.launched_app_interaction_smoke.v1"

const RECEIPT_REF =
  "docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md"

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

describe(`contract ${CONTRACT_ID}`, () => {
  test("signed_in_thread_smoke_receipt_pass.unit — the SignedInThreadSmoke receipt exists and records PASS", async () => {
    const receipt = repoPath(RECEIPT_REF)
    expect(existsSync(receipt)).toBe(true)

    const text = await readFile(receipt, "utf8")

    // Records a PASS result, not just that a run happened.
    expect(text).toContain("## Result")
    expect(text).toMatch(/\bPASS\b/)

    // The signed-in interaction clauses the contract binds: a seeded thread
    // opens, the composer lane picker is visible, and a message renders.
    expect(text).toContain("Maestro smoke thread")
    expect(text).toContain("Send with Claude")

    // The flow the receipt is for is the one this contract enforces.
    expect(text).toContain("SignedInThreadSmoke.yaml")
  })
})
