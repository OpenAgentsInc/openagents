// Applied regression guard: the SHIPPED README platform copy must stay narrowed.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.windows_wsl_consumer_install_coverage_missing
//
// `consumer-install-platform-support.test.ts` (in src/) exercises the verifier
// against synthetic claims. This test binds it to the ACTUAL apps/pylon/README.md
// copy so a future edit that drops the narrowing sentence or reintroduces a
// Windows/WSL/any-platform claim fails here — catching real copy drift, not a
// hypothetical fixture. It does NOT clear the blocker or change promise state.
import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

import {
  auditReadmePlatformCopy,
  README_NARROWED_PLATFORM_SENTENCE,
} from "../src/consumer-install-platform-support"

const readme = readFileSync(join(import.meta.dir, "../README.md"), "utf8")

describe("README consumer-install platform copy guard", () => {
  const audit = auditReadmePlatformCopy(readme)

  test("README still carries the narrowing sentence", () => {
    expect(audit.narrowedClaimPresent).toBe(true)
  })

  test("README contains no over-promise platform phrases", () => {
    expect(audit.overpromisePhraseRefs).toEqual([])
  })

  test("derived claim does not over-promise per the verifier", () => {
    expect(audit.claimVerification.overpromises).toBe(false)
    expect(audit.claimVerification.valid).toBe(true)
  })

  test("overall shipped copy is honest", () => {
    expect(audit.copyHonest).toBe(true)
  })

  test("auditor flags a drifted copy that reintroduces any-platform / Windows", () => {
    const drifted =
      readme +
      "\n\nPylon runs on any platform, and Windows is fully supported.\n"
    const driftAudit = auditReadmePlatformCopy(drifted)
    expect(driftAudit.copyHonest).toBe(false)
    expect(driftAudit.overpromisePhraseRefs).toContain("any-platform-copy")
    expect(driftAudit.overpromisePhraseRefs).toContain("windows-supported-copy")
    expect(driftAudit.claimVerification.overpromises).toBe(true)
  })

  test("auditor catches verb-first drift the old verb-after-only patterns missed", () => {
    // Regression: previously the windows/wsl detectors required the coverage
    // verb to appear AFTER the platform word, so these common phrasings (verb
    // first) silently passed the guard. They must now be caught.
    for (const drift of [
      "Pylon also works on Windows now.",
      "Now runs on Windows too.",
      "We support Windows.",
      "Fully supported on WSL.",
      "Pylon runs on all platforms.",
    ]) {
      const driftAudit = auditReadmePlatformCopy(readme + "\n\n" + drift + "\n")
      expect(driftAudit.copyHonest).toBe(false)
    }
  })

  test("auditor still accepts honest macOS/Linux-only mentions (no false positive)", () => {
    // The coverage verbs must only fire when paired with an out-of-scope
    // platform token; honest macOS/Linux copy must stay honest.
    const honest =
      readme +
      "\n\nPylon works great and runs on macOS and Linux laptops.\n"
    expect(auditReadmePlatformCopy(honest).copyHonest).toBe(true)
  })

  test("auditor fails if the narrowing sentence is removed", () => {
    // The sentence wraps in the file, so normalize first (as the auditor does)
    // before removing it, to confirm removal is actually detected.
    const stripped = readme
      .replace(/\s+/g, " ")
      .replace(README_NARROWED_PLATFORM_SENTENCE, "")
    const strippedAudit = auditReadmePlatformCopy(stripped)
    expect(strippedAudit.narrowedClaimPresent).toBe(false)
    expect(strippedAudit.copyHonest).toBe(false)
  })
})
