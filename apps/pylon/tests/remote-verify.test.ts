import { describe, expect, test } from "bun:test"

import { planRemoteVerify } from "../src/remote-verify"
import { normalizeSshTarget } from "../src/ssh-target"

describe("planRemoteVerify", () => {
  test("plans static SSH verification without artifact collection", () => {
    const target = normalizeSshTarget({ host: "example.test" })
    const plan = planRemoteVerify({
      target,
      verify: ["bun", "test"],
    })

    expect(plan.providerKind).toBe("static_ssh")
    expect(plan.verifyRef.startsWith("verify.")).toBe(true)
    expect(plan.steps.map((step) => step.step)).toEqual([
      "materialize",
      "sync",
      "run",
      "release",
    ])
    expect(plan.steps.map((step) => step.detailRef)).toEqual([
      "remote_verify.materialize",
      "remote_verify.sync",
      "remote_verify.run",
      "remote_verify.release",
    ])
  })

  test("plans artifact collection when required artifacts are present", () => {
    const target = normalizeSshTarget({ host: "example.test" })
    const plan = planRemoteVerify({
      target,
      verify: ["bun", "test"],
      requiredArtifacts: ["coverage/lcov.info"],
    })

    expect(plan.providerKind).toBe("static_ssh")
    expect(plan.verifyRef.startsWith("verify.")).toBe(true)
    expect(plan.steps.map((step) => step.step)).toEqual([
      "materialize",
      "sync",
      "run",
      "collect_artifacts",
      "release",
    ])
  })
})
