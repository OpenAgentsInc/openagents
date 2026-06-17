import { describe, expect, test } from "bun:test"
import { assertLaunchCopyAllowed, projectLaunchGateMatrix } from "../src/launch-gates"
import { assertPublicProjectionSafe } from "../src/state"

describe("Pylon launch gate copy guards", () => {
  test("allows rc package copy and exposes evidence refs", () => {
    const matrix = projectLaunchGateMatrix()

    expect(matrix.version).toBe("1.0.0-rc.12")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.v1_0.rc_package")?.state).toBe("allowed")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.optional_local_qwen_inference")?.state).toBe("allowed")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.paid_qwen_inference")?.state).toBe("blocked")
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.psionic_training_boundary")?.state).toBe(
      "blocked",
    )
    expect(matrix.supportsTraining).toBe(false)
    expect(matrix.gates.find((gate) => gate.claimRef === "claim.pylon.v0_3.stable")?.state).toBe("blocked")
    assertLaunchCopyAllowed("@openagentsinc/pylon@1.0.0-rc.12 is the v1.0 release candidate.")
    assertPublicProjectionSafe(matrix)
  })

  test("blocks unsafe public launch phrases until evidence gates exist", () => {
    expect(() => assertLaunchCopyAllowed("Pylon v0.3.0 is stable and ready.")).toThrow("blocked public claim")
    expect(() => assertLaunchCopyAllowed("Pylon v0.3 is assignment-ready across the network.")).toThrow(
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
