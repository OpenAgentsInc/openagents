import { describe, expect, test } from "bun:test"
import { assertLaunchCopyAllowed, projectLaunchGateMatrix } from "../src/launch-gates"
import { assertPublicProjectionSafe } from "../src/state"
import { PYLON_VERSION } from "../src/version"

describe("Pylon launch gate copy guards", () => {
  test("allows stable package copy and exposes evidence refs", () => {
    const matrix = projectLaunchGateMatrix()

    expect(matrix.version).toBe(PYLON_VERSION)
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.v1_0.stable_package")?.state).toBe("allowed")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.optional_local_qwen_inference")?.state).toBe("allowed")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.paid_qwen_inference")?.state).toBe("blocked")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.psionic_training_boundary")?.state).toBe(
      "blocked",
    )
    expect(matrix.gates.every((gate) => !/Pylon v0\.3/i.test(gate.publicPhrase))).toBe(true)
    expect(matrix.supportsTraining).toBe(false)
    assertLaunchCopyAllowed(`@openagentsinc/pylon@${PYLON_VERSION} is the v1.0 stable release.`)
    assertPublicProjectionSafe(matrix)
  })

  test("blocks unsafe public launch phrases until evidence gates exist", () => {
    expect(() => assertLaunchCopyAllowed("Pylon v0.3.0 is stable and ready.")).toThrow("blocked public claim")
    expect(() => assertLaunchCopyAllowed("Pylon v0.3 is assignment-ready across the network.")).toThrow(
      "blocked public claim",
    )
    expect(() => assertLaunchCopyAllowed("Pylon v1.0 is assignment-ready across the network.")).toThrow(
      "blocked public claim",
    )
    expect(() => assertLaunchCopyAllowed("Paid Pylon work settles Bitcoin today.")).toThrow("blocked public claim")
    expect(() => assertLaunchCopyAllowed("Qwen is training on people's devices.")).toThrow("blocked public claim")
    expect(() => assertLaunchCopyAllowed("Paid Qwen inference is live on Pylons.")).toThrow("blocked public claim")
    expect(() => assertLaunchCopyAllowed("Pylons sell compute capacity live.")).toThrow("blocked public claim")
  })

  test("allows only bounded optional-local-inference launch language", () => {
    assertLaunchCopyAllowed(
      "Pylon can use optional local Qwen3.5 inference when the Psionic backend, model, and tool-call gates pass.",
    )
  })

  test("keeps secret-shaped material out of launch gate copy", () => {
    expect(() => assertLaunchCopyAllowed("Use bearer abc123 to inspect launch status.")).toThrow("private-data-shaped")
  })
})
