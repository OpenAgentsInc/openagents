import { describe, expect, test } from "bun:test"
import path from "node:path"

import { mvpProofRequiredSteps, resolveMvpProofConfig } from "./mvp-proof.ts"

describe("ProductSpec-native MVP proof contract", () => {
  test("requires the exact execution, child, verification, and owner-gate journey", () => {
    expect(mvpProofRequiredSteps).toEqual([
      "shell",
      "product-spec-open",
      "plan-accepted",
      "root-packet-turn",
      "root-packet-verified",
      "child-packet-turn",
      "child-transcript",
      "child-packet-verified",
      "owner-gate-pending",
    ])
  })

  test("is double-gated, mutually exclusive, and refuses unsafe spec paths", () => {
    const userData = "/tmp/openagents-mvp-user-data"
    expect(resolveMvpProofConfig({}, userData).enabled).toBe(false)
    const valid = resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: "specs/mvp.product-spec.md",
    }, userData)
    expect(valid).toEqual({
      enabled: true,
      conflict: false,
      outDir: path.join(userData, "mvp-proof"),
      specPath: "specs/mvp.product-spec.md",
    })
    for (const specPath of ["", "../escape.product-spec.md", "/tmp/x.product-spec.md", "specs/not-a-spec.md"]) {
      expect(resolveMvpProofConfig({
        OPENAGENTS_DESKTOP_MVP_PROOF: "1",
        OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
        OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: specPath,
      }, userData).conflict).toBe(true)
    }
    expect(resolveMvpProofConfig({
      OPENAGENTS_DESKTOP_MVP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_SMOKE: "1",
      OPENAGENTS_DESKTOP_MVP_PROOF_SPEC_PATH: "specs/mvp.product-spec.md",
    }, userData).conflict).toBe(true)
  })
})
